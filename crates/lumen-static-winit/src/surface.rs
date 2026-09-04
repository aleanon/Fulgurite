//! The pixels half: canvas, damage, blit, present —
//! `docs/plan-desktop.md` T14.0.
//!
//! Everything here was inside `examples/counter-window` for three
//! milestones, which meant a second application copied it or got it
//! wrong. Two of the comments below record bugs that cost a debugging
//! session each, and both are the kind an app author would hit again.

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::Instant;

use lumen_static::event::App;
use lumen_static::Color;
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::FontStack;
use lumen_static_shell::Host;
use winit::window::Window;

/// What one presented frame cost, in nanoseconds.
///
/// The same fields `lumen_static_agent::FrameTiming` wants, without
/// depending on the agent crate — a window should not have to link the
/// agent to time itself, and the example maps one to the other.
#[derive(Clone, Copy, Debug, Default)]
pub struct Frame {
    pub layout_ns: u64,
    pub paint_ns: u64,
    pub render_ns: u64,
    pub present_ns: u64,
    pub frame_ns: u64,
    /// Pixels the damage rect covered, or 0 for a frame that rendered
    /// nothing and was presented anyway (see [`Surface::present`]).
    pub damaged_px: u64,
}

/// A canvas of `width` x `height` that rasterises with `font`.
///
/// `font` must be the same face layout measures with, or boxes are
/// sized for one face and filled with another — the bug that made the
/// first window wrap "TALLY" to "TAL / LY".
pub fn canvas(width: u32, height: u32, font: Option<&FontStack>) -> Canvas {
    match font {
        Some(f) => Canvas::with_font(width, height, f.clone()),
        None => Canvas::new(width, height),
    }
}

/// Grow or shrink `canvas` to match the window, and tell the host.
/// Cheap and idempotent when the size has not changed.
pub fn sync_size<A: App>(
    canvas: &mut Canvas,
    host: &mut Host<A>,
    font: Option<&FontStack>,
    width: u32,
    height: u32,
) {
    if (canvas.width, canvas.height) == (width, height) {
        return;
    }
    *canvas = self::canvas(width, height, font);
    host.resize(width, height);
}

/// The softbuffer half: what the canvas is blitted into and presented
/// through.
///
/// It does **not** own the canvas. An application may need to read the
/// canvas while the surface does not exist — the tally's agent endpoint
/// answers `ui.screenshot` before `resumed` has run — and a `Surface`
/// that owned it would make that a `None`.
pub struct Surface {
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
}

impl Surface {
    pub fn new(window: Rc<Window>) -> Self {
        let ctx = softbuffer::Context::new(window.clone()).expect("softbuffer context");
        let surface = softbuffer::Surface::new(&ctx, window).expect("softbuffer surface");
        Surface { surface }
    }

    /// Read the presented window back from the display server, as RGBA8.
    ///
    /// `softbuffer::Surface::fetch` is unimplemented on Wayland and
    /// macOS and needs a mapped window on X11, so a failure is reported
    /// as `None` — "not checkable" — rather than guessed at. This is the
    /// one view that sees the blit *and* the present, which is how
    /// `ui.screenshot { source: "surface" }` catches a frame that was
    /// rendered and never shown.
    pub fn fetch_rgba(&mut self, width: u32, height: u32) -> Option<(u32, u32, Vec<u8>)> {
        let px = self.surface.fetch().ok()?;
        let n = (width * height) as usize;
        if px.len() < n {
            return None;
        }
        // 0RGB back to RGBA8, which is what the PNG encoder wants.
        let mut rgba = Vec::with_capacity(n * 4);
        for v in &px[..n] {
            rgba.extend_from_slice(&[(v >> 16) as u8, (v >> 8) as u8, *v as u8, 255]);
        }
        Some((width, height, rgba))
    }

    /// Render the damaged region and put the frame on screen.
    ///
    /// The host must already have pumped: this draws what `host.list`
    /// holds and asks the host what changed.
    pub fn present<A: App>(
        &mut self,
        host: &mut Host<A>,
        canvas: &mut Canvas,
        page: Color,
        frame_start: Instant,
    ) -> Frame {
        let (width, height) = (canvas.width, canvas.height);
        let (Some(w), Some(h)) = (NonZeroU32::new(width), NonZeroU32::new(height)) else {
            return Frame::default();
        };
        let (layout_ns, paint_ns) = host.timings();

        self.surface.resize(w, h).expect("resize surface");
        let mut buf = self.surface.buffer_mut().expect("acquire surface buffer");

        // What *this buffer* needs redrawn, which is not `host.damage()`.
        // The surface is multi-buffered, and the one just handed back may
        // hold the frame from two presents ago or nothing at all. It says
        // which through `age`, and drawing one frame's difference onto a
        // two-frame-old base is the classic double-buffered smear
        // (`crates/lumen-static/src/retain.rs`).
        let viewport = [0.0, 0.0, width as f32, height as f32];
        let area = host.take_damage_for_age(buf.age()).clip_to(viewport);

        // Render only what changed — but *always* present.
        //
        // T8b.4 said "no damage, no frame", and the first implementation
        // took that literally and returned without presenting. That is
        // wrong, and it emptied the window: `RedrawRequested` is not only
        // ours. X11 sends it on **expose** — when the window is mapped,
        // uncovered or moved — and an expose means the *server* lost the
        // content, which the app's own damage knows nothing about.
        // Skipping the present left whatever was in the shared memory
        // segment on screen, which is another window's pixels.
        //
        // winit 0.30 does not say which kind of `RedrawRequested` this
        // is, and guessing wrong is invisible in every headless test, so
        // the frame is always presented. What T8b.4 actually saves is the
        // *raster*, which is the expensive half: render p95 0.089 ms
        // against present p95 0.044 ms (`docs/results-frame-cost.md`).
        let t = Instant::now();
        let render_ns = match area {
            Some(area) => {
                canvas.clear_within(area, page);
                canvas.render_within(&host.list, area);
                t.elapsed().as_nanos() as u64
            }
            None => 0,
        };

        let t = Instant::now();
        // Blit only what changed, but present all of it. The two halves
        // answer different questions: the *buffer* only needs the damage
        // written into it, because at `age >= 1` it already holds the
        // frame presented last — and at age 0 the damage is `All` by
        // construction, so this is a full blit anyway. The *present* has
        // to be whole, because this redraw may be an expose and the
        // display server has lost everything.
        if let Some(area) = area {
            canvas.blit_0rgb_within(&mut buf, area);
        }
        // `present`, not `present_with_damage`: the partial form would
        // tell the server to copy only the damaged rows, which is exactly
        // wrong when the redraw is an expose. (`present` is itself
        // `present_with_damage` over the full rect in every softbuffer
        // backend, so only the hint is given up.)
        buf.present().expect("present");
        let present_ns = t.elapsed().as_nanos() as u64;

        Frame {
            layout_ns,
            paint_ns,
            render_ns,
            present_ns,
            frame_ns: frame_start.elapsed().as_nanos() as u64,
            damaged_px: area.map_or(0, |a| {
                ((a[2] - a[0]) as f64 * (a[3] - a[1]) as f64).max(0.0) as u64
            }),
        }
    }
}
