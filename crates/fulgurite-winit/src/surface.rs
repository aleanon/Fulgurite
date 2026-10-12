//! The pixels half: canvas, damage, blit, present —
//! `docs/plan-desktop.md` T14.0.
//!
//! Everything here was inside `examples/counter-window` for three
//! milestones, which meant a second application copied it or got it
//! wrong. Two of the comments below record bugs that cost a debugging
//! session each, and both are the kind an app author would hit again.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Instant;

use fulgurite::Color;
use fulgurite_shell::backend::{Frame, WindowHandles};
use fulgurite_shell::session::FrameSource;
use fulgurite_shell::software::Canvas;
use fulgurite_shell::text::FontStack;

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

/// Grow or shrink `canvas` to match the window. Cheap and idempotent
/// when the size has not changed; **whether it changed**, because the
/// host must then be told too (`Host::resize`, or `Session::resize`).
pub fn sync_size(canvas: &mut Canvas, font: Option<&FontStack>, width: u32, height: u32) -> bool {
    if (canvas.width, canvas.height) == (width, height) {
        return false;
    }
    // **In place.** This was `*canvas = canvas(w, h, font)` until
    // 2026-09-13, which rebuilt the whole struct and therefore dropped
    // every field the constructor does not set — including the
    // `ImageSource` a `Config::images` had just installed. The symptom
    // was a window with no picture in it on both backends while the
    // headless render at the same size was correct, because only the
    // window resizes.
    let _ = font;
    canvas.resize(width, height);
    true
}

/// The softbuffer half: what the canvas is blitted into and presented
/// through.
///
/// It does **not** own the canvas. An application may need to read the
/// canvas while the surface does not exist — an agent endpoint may
/// answer `ui.screenshot` before `resumed` has run — and a `Surface`
/// that owned it would make that a `None`.
/// The window is held in an `Arc`, not an `Rc`, and the reason is the
/// GPU backend: wgpu's *safe* surface constructor wants a `Send + Sync`
/// window handle, and `crates/*/src` may contain no `unsafe`
/// (`scripts/check_no_unsafe.sh`), so `create_surface_unsafe` is not
/// available to reach for. softbuffer is generic over the handle and
/// does not care either way. **As raw handles** since M83/T83.1
/// ([`WindowHandles`]), so a backend built from a `BackendTarget` never
/// names winit's `Window`; a winit window coerces to one.
pub struct Surface {
    surface: softbuffer::Surface<Arc<dyn WindowHandles>, Arc<dyn WindowHandles>>,
}

impl Surface {
    pub fn new(window: Arc<dyn WindowHandles>) -> Self {
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
    pub fn present(
        &mut self,
        host: &mut dyn FrameSource,
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
        // (`crates/fulgurite/src/retain.rs`).
        let viewport = [0.0, 0.0, width as f32, height as f32];
        let damage = host.take_damage_for_age(buf.age());

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
        // *raster*, which is the expensive half: the render and present
        // p95s are in `docs/results-frame-cost.md`, "Correction, same
        // day" (this comment restated them, and drifted -- R8-F9).
        //
        // **Every region, not their bounding box** — M18. A box union of
        // two changes in opposite corners is the whole window, which is
        // how a scrollbar plus a toolbar used to cost a full repaint.
        // Regions may overlap; clearing and replaying each independently
        // writes an overlapped pixel twice with the same value, so the
        // only cost of an overlap is the second write.
        let t = Instant::now();
        let mut damaged_px = 0u64;
        let mut rendered = false;
        // The frame's store first, then the canvas's own (M84/T84.6).
        let images = host.images();
        for area in damage.regions(viewport) {
            canvas.clear_within(area, page);
            canvas.render_within_from(host.list(), area, images);
            damaged_px +=
                ((area[2] - area[0]) as f64 * (area[3] - area[1]) as f64).max(0.0) as u64;
            rendered = true;
        }
        let render_ns = if rendered { t.elapsed().as_nanos() as u64 } else { 0 };

        let t = Instant::now();
        // Blit only what changed, but present all of it. The two halves
        // answer different questions: the *buffer* only needs the damage
        // written into it, because at `age >= 1` it already holds the
        // frame presented last — and at age 0 the damage is `All` by
        // construction, so this is a full blit anyway. The *present* has
        // to be whole, because this redraw may be an expose and the
        // display server has lost everything.
        for area in damage.regions(viewport) {
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
            // The sum over regions, so an overlap is counted twice --
            // which is what it costs.
            damaged_px,
            // softbuffer's present panics on failure; it never drops.
            dropped: false,
            atlas_give_ups: None,
        }
    }
}
