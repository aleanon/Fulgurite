//! A software renderer — enough to see the frame without a GPU or a
//! shaping stack.
//!
//! Deliberately small: filled and rounded rectangles with anti-aliased
//! corners, one-pass soft shadows, and text — real glyphs behind the
//! `truetype` feature, otherwise one block per character at the advance
//! `BlockMetrics` assumes, so what is drawn matches what was measured.
//! Its job is to make the pipeline *verifiable* headlessly; a real
//! rasterizer (parity Phase A adopts Lumen's `lumen-render`) replaces it
//! without touching a widget. Per-stage costs: `docs/results-frame-cost.md`.
//!
//! # Which pixels a command covers
//!
//! **Every command covers `floor(x0)..ceil(x1)` × `floor(y0)..ceil(y1)`
//! of its clipped rect**, and `Canvas::clear_within` rounds outward the
//! same way — a pixel the edge touches at all is the command's. That one
//! convention is what makes a partial redraw equal a full one when a
//! damage region's edge is fractional, which since M40/T40.1 is the
//! ordinary case: damage is physical, and every edge of a layout at 1.25
//! or 1.5 lands between pixels. A command that rounded any other way
//! would leave the row it disagreed about cleared and not redrawn. The
//! glyph blit was that command until M40/T40.2. Damage regions are
//! deliberately *not* snapped as well: one rule in one place is enough.

use lumen_static::desc::Point;
use lumen_static::display::{Cmd, Color, DisplayList, GradAxis, GradKind, Renderer, Stops};

/// An RGBA8 pixel buffer.
/// **Keeps its name; M27's widget is PaintBox** (T27.2, 2026-09-18).
///
/// Lumen has a `Canvas` widget — a user draw callback — and this is the
/// raster backend's pixel buffer. Renaming *this* to free the widget
/// name would be the tail wagging the dog: it is cited from
/// `lumen-static-shell`'s front page, from `lumen-static-wgpu`'s shader
/// comments, and from AGENT.md itself (`Canvas::bands_for`).
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    /// The blur's working buffers, kept across frames — M26/T26.13.
    /// Private: it is scratch, and a caller that resized it would be
    /// resizing something `glass_within` re-fits on every use anyway.
    glass: GlassScratch,
    /// The path rasteriser's buffers, kept across frames like `glass`'s
    /// (M82/T82.5, [`PathScratch`]).
    paths: PathScratch,
    /// A real face, when one is available. Without it, text falls back
    /// to one filled block per glyph — enough to prove the pipeline,
    /// which is what the headless oracle needs, and not enough to read.
    #[cfg(feature = "truetype")]
    pub font: Option<crate::text::FontStack>,
    /// The shaping scratch and the glyph coverage cache — M50/T50.2.
    /// Private, like `glass`: scratch the canvas owns because it is the
    /// one thing in a frame holding `&mut` (`docs/plan-raster-allocation.md`
    /// §2.1), which keeps [`crate::text::FontStack`] immutable and `Sync`.
    #[cfg(feature = "truetype")]
    text: TextState,
    /// Where `Cmd::Image` is resolved to pixels (T15.2).
    ///
    /// `None` draws nothing, which is what a host that ships no images
    /// wants and costs it one `Option` check per image command — of
    /// which it has none.
    pub images: Option<Box<dyn lumen_static::ImageSource>>,
}

/// A horizontal band of a [`Canvas`], and the only thing that draws.
///
/// **Every rasterising method lives here rather than on `Canvas`, and
/// the reason is threads** (M17/T17.4). A band borrows `&mut [u8]` — a
/// slice of whole rows — plus the absolute `y` that slice starts at, so
/// N disjoint bands of one surface can be handed to N threads with no
/// aliasing and no `unsafe`, because a safe split of the pixel buffer
/// into row groups is what produces them and the borrow checker proves
/// they do not overlap. `Canvas::render_banded` is the threaded caller
/// (M17/T17.4), behind `threads`; text reaches a band already shaped, from
/// the frame thread's `TextState::prepare` (M50/T50.3).
///
/// A whole-surface render is the `y0 = 0`, `rows = height` band, so
/// there is **one** implementation of the pixel arithmetic and not two.
/// Forking the rasteriser for a threaded copy was considered and
/// rejected: `damage_oracle.rs`, `clip_test.rs`, `wide_stores.rs` and
/// `simd_shadow.rs` all assert byte-identity against a single
/// implementation, and two copies that must stay pixel-identical
/// forever is the maintenance shape this crate avoids everywhere else.
///
/// Coordinates stay **absolute** throughout — a command's rect means
/// the same thing in every band. Only indexing subtracts `y0`, which is
/// what keeps the arithmetic in the drawing code unchanged from when it
/// addressed a whole surface.
/// The blur's three working buffers, kept across frames.
///
/// **`glass_within` built these per glass command** — a capture, its
/// accumulator, and the ping-pong target — which the allocation gate
/// measured at 3.00 allocations per panel (M26/T26.13). They only ever
/// grow: a window is resized rarely and a panel's source rect is
/// bounded by it, so the steady state after the first frame is zero.
#[derive(Default)]
pub(crate) struct GlassScratch {
    a: Vec<u16>,
    b: Vec<u16>,
    acc: Vec<u32>,
}

impl GlassScratch {
    /// Size the three buffers for a `bw * bh` downsampled capture, and
    /// zero what will be read. `resize` keeps the capacity it has.
    fn fit(&mut self, bw: usize, bh: usize) {
        self.a.clear();
        self.a.resize(bw * bh * 3, 0);
        self.b.clear();
        self.b.resize(bw * bh * 3, 0);
        self.acc.clear();
        self.acc.resize(bw * bh * 4, 0);
    }
}

/// A canvas's text scratch: one layout and placement buffer, the
/// coverage cache (M50/T50.2), and this frame's shaped runs (T50.3).
#[cfg(feature = "truetype")]
#[derive(Default)]
pub(crate) struct TextState {
    shape: crate::text::ShapeScratch,
    cache: crate::text::GlyphCache,
    /// The frame's visible runs, in command order: (command index, range
    /// into `placed`).
    runs: Vec<(u32, u32, u32)>,
    /// Every visible run's placements, back to back.
    placed: Vec<crate::text::Placed>,
}

#[cfg(feature = "truetype")]
impl TextState {
    /// **Shape each visible run once, on the frame thread, before any
    /// band draws** — M50/T50.3, R8-F2.
    ///
    /// A band culled a run against its own rows padded by one em and then
    /// shaped the whole run, so a run whose padded reach straddled a strip
    /// boundary was shaped once per strip it touched: 2.27 times per
    /// visible run at 1080p and 3.63 at 4K (`tests/banded.rs`). This walks
    /// the list once with the same clip stack and the same cull, against
    /// the frame's outer clip — which contains every band's, so every run
    /// a band will draw is here — and fills the cache for every glyph that
    /// meets the clip. The bands then only read: placements and coverage
    /// through a shared `&`, which is what lets them run on threads without
    /// borrowing the cache mutably (`docs/plan-raster-allocation.md` §2.1).
    ///
    /// Without `threads` the same pass runs and the single band reads it,
    /// so there is one text path and `scripts/check_banded.sh` compares
    /// like with like.
    fn prepare(&mut self, list: &DisplayList, outer: [f32; 4], font: &crate::text::FontStack) {
        self.runs.clear();
        self.placed.clear();
        let mut clip = ClipStack::new(outer);
        for (i, c) in list.cmds.iter().enumerate() {
            match c {
                Cmd::PushClip { rect } => clip.push(*rect),
                Cmd::PopClip => clip.pop(),
                // The same scope the raster loop gives a bracket (M83/T83.3),
                // so a fallback's runs are culled as they will be drawn.
                #[cfg(lumen_shader)]
                Cmd::Shader { rect, .. } => clip.push(*rect),
                #[cfg(lumen_shader)]
                Cmd::ShaderEnd => clip.pop(),
                Cmd::Text { rect, text, size, weight, line_height, .. } => {
                    let pad = *size;
                    let reach = [rect[0] - pad, rect[1] - pad, rect[2] + pad, rect[3] + pad];
                    if clipped(reach, clip.as_slice()).is_none() {
                        continue;
                    }
                    font.place_run_into(
                        &mut self.shape,
                        list.str_of(*text),
                        *size,
                        *weight,
                        *line_height,
                        rect[2] - rect[0],
                    );
                    let start = self.placed.len();
                    for p in self.shape.placed() {
                        // The column and the phase from one shell function
                        // (`text::pen_of`, M81/T81.8), as the bands and the
                        // GPU decide them.
                        let pen = crate::text::pen_of(rect[0] + p.x, p);
                        let gy = (rect[1] + p.y).round();
                        // Only a glyph that meets the clip is rasterised
                        // (R8-F3's out-of-clip half, T50.2).
                        if clipped([pen.x, gy, pen.x + pen.w as f32, gy + p.h as f32], clip.as_slice()).is_some() {
                            self.cache.fill(font, pen.id);
                        }
                        self.placed.push(*p);
                    }
                    self.runs.push((i as u32, start as u32, self.placed.len() as u32));
                }
                _ => {}
            }
        }
    }
}

struct Surface<'a> {
    px: &'a mut [u8],
    width: u32,
    /// Absolute y of `px`'s first row.
    y0: i64,
    /// Rows in `px`.
    rows: i64,
    /// The blur's buffers, owned by the `Canvas` (or by a worker's own
    /// band) and lent here for the frame.
    glass: &'a mut GlassScratch,
    /// The path rasteriser's buffers, lent the same way (M82/T82.5).
    paths: &'a mut PathScratch,
    /// The frame's shaped runs and the glyph cache, shared by every band
    /// — read-only, because the frame thread filled both before any band
    /// started (`TextState::prepare`, M50/T50.3).
    #[cfg(feature = "truetype")]
    text: &'a TextState,
}

impl Surface<'_> {
    /// One past the last absolute row this band owns. Every y clamp in
    /// the drawing code uses this where it used the surface height.
    #[inline]
    fn y_hi(&self) -> i64 {
        self.y0 + self.rows
    }

    /// Blit one text run with real glyphs.
    ///
    /// The run's box is its *layout* box, so glyphs are placed from its
    /// top-left and clipped to whatever clip scopes are open — the same
    /// treatment a rect gets, which is what keeps text inside a scroller.
    /// Each glyph is clipped once as a rect; the first version clipped
    /// every pixel and that was most of the text cost.
    ///
    /// **Blitted from placements and coverage the frame thread prepared**
    /// — M50/T50.2 and T50.3. This called `FontStack::glyphs`, which built
    /// two fontdue layouts, a placement `Vec`, a glyph `Vec` and a coverage
    /// `Vec` per glyph on every frame (R8-F1: 25.80 allocations a frame on
    /// the allocation gate's own fixture, `docs/results-allocation.md`),
    /// and in every band the run reached (R8-F2). Now `TextState::prepare`
    /// shapes each visible run once and fills the cache, and this only
    /// finds the run by its command index — `cursor` walks forward, since
    /// both are in list order — and blends. A glyph is clipped before its
    /// coverage is looked up, so a glyph outside the clip is never
    /// rasterised (R8-F3's second half).
    ///
    /// A glyph the cache does not hold — too large to keep, or lost to a
    /// reset during a frame that needed more than the cache holds — is
    /// rasterised here and drawn, allocating: never dropped (plan §2.1).
    #[cfg(feature = "truetype")]
    #[allow(clippy::too_many_arguments)]
    fn draw_run(
        &mut self,
        font: &crate::text::FontStack,
        index: u32,
        cursor: &mut usize,
        rect: &[f32; 4],
        color: Color,
        clip: &[[f32; 4]],
    ) {
        let text = self.text;
        while *cursor < text.runs.len() && text.runs[*cursor].0 < index {
            *cursor += 1;
        }
        let Some(&(at, a, b)) = text.runs.get(*cursor) else { return };
        if at != index {
            return;
        }
        for p in &text.placed[a as usize..b as usize] {
            let pen = crate::text::pen_of(rect[0] + p.x, p);
            let (gx, gy) = (pen.x, (rect[1] + p.y).round());
            let Some(r) = clipped([gx, gy, gx + pen.w as f32, gy + p.h as f32], clip) else {
                continue;
            };
            match text.cache.get(pen.id) {
                Some((cov, w, h)) => self.blit_glyph(cov, w, h, gx, gy, r, color),
                None => {
                    if let Some((w, h, cov)) = font.rasterize(pen.id) {
                        self.blit_glyph(&cov, w as usize, h as usize, gx, gy, r, color);
                    }
                }
            }
        }
    }

    /// Blend one glyph's coverage at `(gx, gy)`, limited to `r`, its rect
    /// already clipped.
    ///
    /// `floor..ceil`, the convention every command here follows (module
    /// docs). The glyph's own rect is whole-pixel, so only a clip edge can
    /// be fractional — and a damage region's far edge is one whenever the
    /// scale is not whole. This took the far edge with a bare `as i64`,
    /// which truncates: a region ending at 173.2 was cleared through row
    /// 173 and the glyphs were redrawn through row 172, leaving row 173
    /// blank until something damaged it again (R1-F8, M40/T40.2;
    /// `tests/partial_text_edge.rs`). Neither end can leave the glyph: its
    /// rect is integer, so `ceil` of a clip inside it stays inside it —
    /// and the loop is bounded by the coverage's own size as well, so a
    /// placement whose box disagrees with its bitmap cannot index past it.
    #[cfg(feature = "truetype")]
    #[allow(clippy::too_many_arguments)]
    fn blit_glyph(&mut self, cov: &[u8], gw: usize, gh: usize, gx: f32, gy: f32, r: [f32; 4], color: Color) {
        let (w, h) = (self.width as i64, self.y_hi());
        let (x0, y0) = ((r[0].floor() as i64).max(0), (r[1].floor() as i64).max(self.y0));
        let x1 = (r[2].ceil() as i64).min(w).min(gx as i64 + gw as i64);
        let y1 = (r[3].ceil() as i64).min(h).min(gy as i64 + gh as i64);
        for y in y0..y1 {
            let row = (y - gy as i64) as usize;
            for x in x0..x1 {
                let col = (x - gx as i64) as usize;
                let c = cov[row * gw + col];
                if c != 0 {
                    self.blend_cov(x, y, color, c);
                }
            }
        }
        crate::text::count_drawn(1);
    }

    /// Fill `bounds`, rounding against `shape`'s corners with
    /// anti-aliased edges.
    ///
    /// Only the four corner squares are sampled per pixel: coverage is
    /// the pixel centre's distance past the corner circle, clamped to
    /// one pixel of ramp. Every other row is a straight span. The first
    /// version tested every pixel of the box and drew hard-edged corners;
    /// this is both the faster and the smoother one
    /// (`docs/results-frame-cost.md`).
    fn fill_rounded_within(&mut self, shape: [f32; 4], bounds: [f32; 4], c: Color, radius: f32) {
        if radius <= 0.5 {
            self.fill(bounds, c);
            return;
        }
        let r = shape;
        if !r.iter().all(|v| v.is_finite()) || !bounds.iter().all(|v| v.is_finite()) {
            return;
        }
        let rad = radius.min((r[2] - r[0]) / 2.0).min((r[3] - r[1]) / 2.0);
        let (w, h) = (self.width as i64, self.y_hi());
        let (x0, y0) = ((bounds[0].floor() as i64).max(0), (bounds[1].floor() as i64).max(self.y0));
        let (x1, y1) = ((bounds[2].ceil() as i64).min(w), (bounds[3].ceil() as i64).min(h));
        // Corner bands, in pixel rows/columns.
        let (top, bottom) = ((r[1] + rad).ceil() as i64, (r[3] - rad).floor() as i64);
        let (left, right) = ((r[0] + rad).ceil() as i64, (r[2] - rad).floor() as i64);
        for y in y0..y1 {
            if y >= top && y < bottom {
                self.span(y, x0, x1, c);
                continue;
            }
            let py = y as f32 + 0.5;
            let dy = (r[1] + rad - py).max(py - (r[3] - rad)).max(0.0);
            // Straight middle of a corner row, then the two corners.
            self.span(y, x0.max(left), x1.min(right), c);
            for x in (x0..x1.min(left)).chain(x0.max(right)..x1) {
                let px = x as f32 + 0.5;
                let dx = (r[0] + rad - px).max(px - (r[2] - rad)).max(0.0);
                let cov = (rad + 0.5 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0);
                if cov > 0.0 {
                    self.blend_cov(x, y, c, (cov * 255.0) as u8);
                }
            }
        }
    }

    /// An **inset** stroked rounded rect — a border (M27/T27.1).
    ///
    /// The ring between two rounded rects: the node's own box, and the
    /// same box inset by `width` with its radius reduced to match, so the
    /// inner and outer curves stay concentric. Coverage is the difference
    /// of the two, which antialiases both edges with one subtraction and
    /// needs no second pass.
    ///
    /// **Not four `Line`s.** `lumen_static::ops::focus_ring` tried that
    /// and the live gate caught it inside a frame: a square outline over a
    /// rounded box covers the antialiased corner. Four lines also cost
    /// four ink rects against `MAX_REGIONS`'s four, so one bordered
    /// control changing would fill the region set on its own.
    ///
    /// Rows are split the way `fill_rounded_within` splits them: a row
    /// between the inner rect's corner bands can only have coverage at
    /// the two vertical edges, so the long middle is skipped rather than
    /// tested per pixel.
    fn stroke_rounded_within(
        &mut self,
        shape: [f32; 4],
        bounds: [f32; 4],
        c: Color,
        radius: f32,
        width: f32,
    ) {
        let r = shape;
        if !r.iter().all(|v| v.is_finite())
            || !bounds.iter().all(|v| v.is_finite())
            || !radius.is_finite()
            || !width.is_finite()
            || width <= 0.0
        {
            return;
        }
        let half_w = (r[2] - r[0]) / 2.0;
        let half_h = (r[3] - r[1]) / 2.0;
        // A border thicker than half the box is a filled box. Clamping
        // rather than refusing keeps `border_width: 255` from drawing
        // nothing at all, which reads as "the border is broken".
        let bw = width.min(half_w).min(half_h);
        let rad_o = radius.min(half_w).min(half_h).max(0.0);
        let rad_i = (rad_o - bw).max(0.0);
        let inner = [r[0] + bw, r[1] + bw, r[2] - bw, r[3] - bw];

        let (w, h) = (self.width as i64, self.y_hi());
        let (x0, y0) = ((bounds[0].floor() as i64).max(0), (bounds[1].floor() as i64).max(self.y0));
        let (x1, y1) = ((bounds[2].ceil() as i64).min(w), (bounds[3].ceil() as i64).min(h));
        // Rows strictly between the inner rect's corner bands: only the
        // two vertical edges can be covered.
        let (mid_top, mid_bot) = ((inner[1] + rad_i).ceil() as i64, (inner[3] - rad_i).floor() as i64);
        let left_end = (inner[0].ceil() as i64) + 1;
        let right_start = (inner[2].floor() as i64) - 1;

        for y in y0..y1 {
            let py = y as f32 + 0.5;
            let scan: &mut dyn Iterator<Item = i64> = if y >= mid_top && y < mid_bot {
                &mut (x0..x1.min(left_end)).chain(x0.max(right_start)..x1)
            } else {
                &mut (x0..x1)
            };
            for x in scan {
                let px = x as f32 + 0.5;
                let cov = rr_cov(px, py, r, rad_o) - rr_cov(px, py, inner, rad_i);
                if cov > 0.0 {
                    self.blend_cov(x, y, c, (cov.min(1.0) * 255.0) as u8);
                }
            }
        }
    }

    /// A backdrop filter — M19/T19.2.
    ///
    /// **The only method here that reads the destination before writing
    /// it.** Everything else composes forward; this captures what is
    /// already in the buffer under `shape`, blurs it, and composites the
    /// result back at the panel's own coverage.
    ///
    /// Three passes of a box blur approximate a Gaussian to within a few
    /// percent, and a box blur with a running sum is O(1) per pixel in
    /// the radius — which is what makes a 12 px blur cost the same as a
    /// 2 px one. The box width is the one SVG's `feGaussianBlur`
    /// specifies. **T19.3 owns the rest of the quality story**: the
    /// odd/even split, the downsample, `u16` intermediates. This is the
    /// correct-and-simple version those are measured against.
    ///
    /// Straight, non-premultiplied sRGB, and that is correct *here and
    /// only here*: `blend` and `blend_px` write `α = 255`
    /// unconditionally and `CLEAR` is opaque, so the destination alpha
    /// is constant and straight equals premultiplied. **A
    /// transparent-window path would invalidate this** and must
    /// premultiply before the box passes.
    fn glass_within(
        &mut self,
        shape: [f32; 4],
        bounds: [f32; 4],
        radius: f32,
        m: lumen_static::display::Material,
    ) {
        let blur = m.blur;
        // **Float, not `Color` — M22/T22.3.** The adaptive tint is a
        // continuous function of the backdrop, and rounding the blend
        // back to `u8` puts a staircase on it: one step of the alpha
        // moves the panel's interior by a whole output level, which was
        // measured at exactly the flicker bound. `docs/results-glass.md`
        // §0e.
        let tint = [m.tint.0 as f32, m.tint.1 as f32, m.tint.2 as f32, m.tint.3 as f32];
        if !shape.iter().all(|v| v.is_finite()) || !bounds.iter().all(|v| v.is_finite()) {
            return;
        }
        // The source region, from the *shared* expression the damage
        // recorder uses. If these two disagreed the panel would be stale
        // in its outermost ring and the oracle would not see it.
        let src = lumen_static::display::glass_source(shape, &m);
        let (w, h) = (self.width as i64, self.y_hi());
        let (k, d, _) = if blur > 0.0 { blur_plan(blur) } else { (1usize, 1i64, 0.0) };
        let ki = k as i64;
        // **Snapped to a multiple of `k` in *surface* coordinates**, so
        // the downsample grid is anchored to the surface and not to the
        // panel. Anchored to the capture, a panel at a fractional or
        // scrolled position samples a different phase every frame and
        // the blur shimmers -- and neither "stable frame to frame" nor
        // the incremental-vs-rebuild oracle can see it, because the
        // capture rect is identical in the two renders being compared.
        let snap_lo = |v: i64| v.div_euclid(ki) * ki;
        let snap_hi = |v: i64| v.div_euclid(ki) * ki + if v.rem_euclid(ki) != 0 { ki } else { 0 };
        let (sx0, sy0) = (snap_lo((src[0].floor() as i64).max(0)), snap_lo((src[1].floor() as i64).max(self.y0)));
        let (sx1, sy1) = (snap_hi((src[2].ceil() as i64).min(w)), snap_hi((src[3].ceil() as i64).min(h)));
        let (sx1, sy1) = (sx1.min(w), sy1.min(h));
        if sx1 <= sx0 || sy1 <= sy0 {
            return;
        }
        let (sw, sh) = ((sx1 - sx0) as usize, (sy1 - sy0) as usize);
        let stride = (self.width * 4) as usize;

        // Capture, downsampling by `k` in the same pass. RGB only:
        // alpha is the constant 255 above, so carrying it through three
        // box passes would blur a constant.
        let (bw, bh) = (sw.div_ceil(k), sh.div_ceil(k));
        // Borrowed, not built: see `GlassScratch`.
        // **Taken out and put back**, rather than borrowed in place:
        // the rest of this function calls `self.blend_px`, so a live
        // borrow of `self.glass` across it would not compile. `take`
        // moves the three `Vec`s, which costs three pointer triples and
        // no allocation.
        let mut scratch = core::mem::take(self.glass);
        scratch.fit(bw, bh);
        let GlassScratch { a, b, acc } = &mut scratch;
        if k == 1 {
            // **No accumulator at 1:1** (M26/T26.13, part three). Every
            // bucket holds exactly one pixel, so the loop below would
            // write four `u32`s per pixel and read them back to divide
            // by one — a full write-then-read of `bw * bh * 4` words
            // for a copy. `docs/results-glass.md` §1 prices the σ range
            // where this happens as the expensive one: below about 5
            // the search picks a single pass and all three passes run
            // at full resolution, so 1:1 is exactly where the frame can
            // least afford it.
            for y in sy0..sy1 {
                let row = (y - self.y0) as usize * stride;
                let by = (y - sy0) as usize;
                for x in sx0..sx1 {
                    let p = row + x as usize * 4;
                    let at = (by * bw + (x - sx0) as usize) * 3;
                    a[at] = self.px[p] as u16;
                    a[at + 1] = self.px[p + 1] as u16;
                    a[at + 2] = self.px[p + 2] as u16;
                }
            }
        } else {
            for y in sy0..sy1 {
                let row = (y - self.y0) as usize * stride;
                let by = ((y - sy0) / ki) as usize;
                for x in sx0..sx1 {
                    let p = row + x as usize * 4;
                    let bx = ((x - sx0) / ki) as usize;
                    let at = (by * bw + bx) * 4;
                    acc[at] += self.px[p] as u32;
                    acc[at + 1] += self.px[p + 1] as u32;
                    acc[at + 2] += self.px[p + 2] as u32;
                    acc[at + 3] += 1;
                }
            }
            for i in 0..bw * bh {
                let n = acc[i * 4 + 3].max(1);
                a[i * 3] = (acc[i * 4] / n) as u16;
                a[i * 3 + 1] = (acc[i * 4 + 1] / n) as u16;
                a[i * 3 + 2] = (acc[i * 4 + 2] / n) as u16;
            }
        }
        // Three passes each way, at the widths SVG specifies.
        if blur > 0.0 && d > 1 {
            for (l, r) in box_windows(d) {
                box_rows(a, b, bw, bh, l, r);
                box_cols(b, a, bw, bh, l, r);
            }
        }

        // **M22/T22.3: the tint follows the backdrop.**
        //
        // Reduced over the panel's own `shape`, intersected with the
        // capture — **never over `bounds`**. `bounds` is the clip, and
        // in a partial redraw the clip is the damage region: reduce
        // over it and the full and the partial render compute different
        // means and then differ on *every pixel of the panel*. That is
        // what the invalidation oracle exists to catch, and it is the
        // one hazard here a gate can see.
        //
        // Reducing over the dilated capture instead would be a quieter
        // bug: a mean that includes `3σ + refraction` of surrounding
        // backdrop is simply the wrong colour — equally wrong in both
        // renders, so every gate stays green. The panel's rect is the
        // only region with both properties.
        //
        // Safe today only because two rules elsewhere happen to put the
        // panel inside the clip; this pins it to neither.
        let tint = if m.adapt > 0 {
            let lo = |v: f32, s: i64| ((v.floor() as i64).max(s) - s).max(0) as usize / k;
            let hi = |v: f32, e: i64, s: i64| (((v.ceil() as i64).min(e) - s).max(0) as usize).div_ceil(k);
            let (bx0, by0) = (lo(shape[0], sx0), lo(shape[1], sy0));
            let (bx1, by1) = (hi(shape[2], sx1, sx0).min(bw), hi(shape[3], sy1, sy0).min(bh));
            if bx1 > bx0 && by1 > by0 {
                let mut acc = 0.0f32;
                for by in by0..by1 {
                    for bx in bx0..bx1 {
                        let i = (by * bw + bx) * 3;
                        // Rec. 601 luma, the same weights `Canvas`
                        // uses for the shadow's facing term.
                        acc += 0.299 * a[i] as f32
                            + 0.587 * a[i + 1] as f32
                            + 0.114 * a[i + 2] as f32;
                    }
                }
                let n = ((bx1 - bx0) * (by1 - by0)) as f32;
                let f = lumen_static::display::adapt_mix(acc / n / 255.0, m.adapt);
                let t2 = m.tint_over_light;
                let g = |x: f32, y: u8| x + (y as f32 - x) * f;
                [
                    g(tint[0], t2.0),
                    g(tint[1], t2.1),
                    g(tint[2], t2.2),
                    g(tint[3], t2.3),
                ]
            } else {
                tint
            }
        } else {
            tint
        };

        // Bilinear sample of the downsampled, blurred backdrop, in
        // surface coordinates.
        // **Float coordinates — M22/T22.1.**
        //
        // This took `i64` until then, and the refracted read truncated
        // to a whole surface pixel *before* getting here. Combined with
        // the downsample factor being 1 at the liquid blur — where `fx`
        // below is then already an integer and the four taps collapse
        // to one — the refraction was nearest-neighbour, and its rim
        // carried a staircase.
        //
        // It also made a colour fringe impossible: a sub-pixel
        // difference between three channels cannot survive truncation
        // to the same pixel, which is what T22.2 needs.
        let sample = |x: f32, y: f32| -> [f32; 3] {
            // Clamped to the capture: refraction displaces the read, and
            // a panel at the surface edge can push it past what was
            // captured. Clamp-to-edge is the same rule the box passes
            // use at their own boundary.
            // `x` and `y` are pixel **centres**, not indices — the
            // caller passes `i + 0.5`. The mapping therefore carries no
            // `+ 0.5` of its own; adding one shifted every read half a
            // pixel and slid the blur's centre of mass from 59.5 to
            // 59.0, which `glass_blur.rs` caught immediately.
            let x = x.clamp(sx0 as f32 + 0.5, sx1 as f32 - 0.5);
            let y = y.clamp(sy0 as f32 + 0.5, sy1 as f32 - 0.5);
            let fx = (x - sx0 as f32) / k as f32 - 0.5;
            let fy = (y - sy0 as f32) / k as f32 - 0.5;
            let x0 = fx.floor();
            let y0f = fy.floor();
            let tx = fx - x0;
            let ty = fy - y0f;
            let cx = |v: f32| (v.max(0.0) as usize).min(bw - 1);
            let cy = |v: f32| (v.max(0.0) as usize).min(bh - 1);
            let (ix0, ix1) = (cx(x0), cx(x0 + 1.0));
            let (iy0, iy1) = (cy(y0f), cy(y0f + 1.0));
            let g = |xi: usize, yi: usize, c: usize| a[(yi * bw + xi) * 3 + c] as f32;
            let mut out = [0.0f32; 3];
            for (c, o) in out.iter_mut().enumerate() {
                let top = g(ix0, iy0, c) * (1.0 - tx) + g(ix1, iy0, c) * tx;
                let bot = g(ix0, iy1, c) * (1.0 - tx) + g(ix1, iy1, c) * tx;
                *o = top * (1.0 - ty) + bot * ty;
            }
            out
        };

        // One channel, for the dispersive path — M22/T22.2.
        //
        // Calling the three-channel read three times would triple the
        // coordinate setup *and* do 36 loads where 12 suffice, because
        // it amortises one clamp-and-floor over all three. This is the
        // cost the plan claims: three setups, twelve loads.
        let sample_ch = |x: f32, y: f32, c: usize| -> f32 {
            let x = x.clamp(sx0 as f32 + 0.5, sx1 as f32 - 0.5);
            let y = y.clamp(sy0 as f32 + 0.5, sy1 as f32 - 0.5);
            let fx = (x - sx0 as f32) / k as f32 - 0.5;
            let fy = (y - sy0 as f32) / k as f32 - 0.5;
            let x0 = fx.floor();
            let y0f = fy.floor();
            let tx = fx - x0;
            let ty = fy - y0f;
            let cx = |v: f32| (v.max(0.0) as usize).min(bw - 1);
            let cy = |v: f32| (v.max(0.0) as usize).min(bh - 1);
            let (ix0, ix1) = (cx(x0), cx(x0 + 1.0));
            let (iy0, iy1) = (cy(y0f), cy(y0f + 1.0));
            let g = |xi: usize, yi: usize| a[(yi * bw + xi) * 3 + c] as f32;
            let top = g(ix0, iy0) * (1.0 - tx) + g(ix1, iy0) * tx;
            let bot = g(ix0, iy1) * (1.0 - tx) + g(ix1, iy1) * tx;
            top * (1.0 - ty) + bot * ty
        };

        // Composite. **Coverage keeps `fill_rounded_within`'s structure**
        // -- hard spans in the straight regions, the ramp only in the
        // corner bands -- so a panel and a rounded rect of the same
        // geometry have the same silhouette. What is new is the bevel:
        // within `rim` of the boundary the surface curves, and that band
        // needs a signed distance everywhere, not only in the corners.
        //
        // **M22/T22.5: `shape` is the *union* of both blobs.** The
        // primary is derived back out of it, and is bit-exactly `shape`
        // when there is no second blob — the offsets are zero and
        // nothing is added or subtracted.
        let morphing = lumen_static::display::morphing(&m);
        let prim = lumen_static::display::glass_primary(shape, &m);
        let (odx, ody) = lumen_static::display::morph_offset(&m);
        let rr = radius.min((prim[2] - prim[0]) / 2.0).min((prim[3] - prim[1]) / 2.0);
        let (cx, cy) = ((prim[0] + prim[2]) / 2.0, (prim[1] + prim[3]) / 2.0);
        let (hx, hy) = ((prim[2] - prim[0]) / 2.0 - rr, (prim[3] - prim[1]) / 2.0 - rr);
        let (x0, y0) = ((bounds[0].floor() as i64).max(0), (bounds[1].floor() as i64).max(self.y0));
        let (x1, y1) = ((bounds[2].ceil() as i64).min(w), (bounds[3].ceil() as i64).min(h));
        let (top, bottom) = ((prim[1] + rr).ceil() as i64, (prim[3] - rr).floor() as i64);
        let (left, right) = ((prim[0] + rr).ceil() as i64, (prim[2] - rr).floor() as i64);

        let rim = m.rim.max(0.0);
        let tv = tint[3] / 255.0;
        let sat = m.saturation as f32 / lumen_static::display::Material::NEUTRAL_SATURATION as f32;
        // **`shadow` is in the predicate**, and leaving it out would
        // have been a panel compared with itself: a material with only
        // an inner shadow would get no bevel, no normal and no dot
        // product, and every injection aimed at the shadow would pass
        // (`docs/plan-liquid.md` §2).
        let bevelled = rim > 0.0 && (m.refraction > 0.0 || m.specular > 0.0 || m.shadow > 0);
        // How wide the neck is, in px — M22/T22.5. Tied to `rim` so the
        // blend's scale follows the bevel's, and floored so a rimless
        // morph still merges rather than producing a hard union.
        //
        // **The multiplier is arithmetic, not taste.** The smooth
        // minimum undershoots by κ/4 where the fields meet, so at a
        // separation `s` the midpoint sits at `s/2 − κ/4`: the neck
        // closes below `s = κ/2`. At `κ = rim` that is 6.5 px on
        // `LIQUID` — the blobs would already be apart at *half* a rim
        // width, and a gate written at one rim width would read empty.
        // `κ = 3·rim` puts the closing point at 1.5 rim widths, between
        // the two separations `docs/plan-liquid.md` §9 gates on.
        let kappa = (rim * 3.0).max(4.0);
        // The signed distance to one rounded rect and its outward
        // normal, which for a true distance field is the unit gradient.
        //
        // **Used by both paths**, not duplicated for the morph: two
        // copies of this expression is the shape that let `Shadow` and
        // `Image` hash as the same tag for a year.
        let blob = |px: f32, pyf: f32, bcx: f32, bcy: f32| -> (f32, f32, f32) {
            let (qx, qy) = ((px - bcx).abs() - hx, (pyf - bcy).abs() - hy);
            let (mx, my) = (qx.max(0.0), qy.max(0.0));
            let outside = (mx * mx + my * my).sqrt();
            let d = outside + qx.max(qy).min(0.0) - rr;
            // **`sign(p - c)` is not optional**: `max(q, 0)` is
            // componentwise non-negative, so without it every normal
            // points into the +x/+y quadrant and the left and top rims
            // refract and light backwards.
            let (sx, sy) = (sign_of(px - bcx), sign_of(pyf - bcy));
            let (nx, ny) = if outside > 1.0e-4 {
                (sx * mx / outside, sy * my / outside)
            } else if qx > qy {
                (sx, 0.0)
            } else {
                (0.0, sy)
            };
            (d, nx, ny)
        };
        // The light, in 3D. Its elevation tilts it out of the screen so a flat
        // interior is lit at all; without it only the rim would be, and
        // the panel would read as an outline rather than a surface.
        let light = [m.light.cos() * LIGHT_XY, m.light.sin() * LIGHT_XY, LIGHT_Z];

        for y in y0..y1 {
            let corner_row = !(y >= top && y < bottom) && rr > 0.5;
            let py = y as f32 + 0.5;
            let dy = (shape[1] + rr - py).max(py - (shape[3] - rr)).max(0.0);
            for x in x0..x1 {
                let (px, pyf) = (x as f32 + 0.5, y as f32 + 0.5);
                // **M22/T22.5: the smooth minimum, or nothing.**
                //
                // `morph_d` is `None` for an ordinary panel, and the
                // whole block below it is then the code M19 shipped,
                // reached by the same route. That branch is on the
                // *command*, not the pixel: M19 records that glass keeps
                // `fill_rounded_within`'s hard-span coverage so a panel
                // is pixel-identical to a rounded rect of the same
                // geometry, and that a pure SDF ramp disagrees by up to
                // 254/255 on a fractional edge (`plan-glass.md` §4.1).
                let morph_d = if morphing {
                    let (da, gax, gay) = blob(px, pyf, cx, cy);
                    let (db, gbx, gby) = blob(px, pyf, cx + odx, cy + ody);
                    let hb = (0.5 + 0.5 * (db - da) / kappa).clamp(0.0, 1.0);
                    let d = db + (da - db) * hb - kappa * hb * (1.0 - hb);
                    // **The blended gradient is exact, and it is not
                    // unit.** Weighting the two gradients by the same
                    // `h` the value uses is not an approximation — the
                    // remaining term vanishes identically. But its
                    // length falls below 1 wherever the gradients
                    // disagree and reaches 0 where they oppose, and the
                    // bevel is built on the normal being unit *by
                    // construction*.
                    let (gx, gy) = (gbx + (gax - gbx) * hb, gby + (gay - gby) * hb);
                    let len = (gx * gx + gy * gy).sqrt();
                    if len > 1.0e-4 {
                        // **And a smooth minimum is not a distance
                        // field.** The gradient's length is below 1 in
                        // the neck, so `u` would reach 0 after fewer
                        // than `rim` pixels and the bevel would visibly
                        // pinch there. Dividing by the length restores
                        // it and shares the square root the
                        // normalisation already needed.
                        Some((d / len, gx / len, gy / len))
                    } else {
                        Some((d, 0.0, 0.0))
                    }
                } else {
                    None
                };
                let cov = match morph_d {
                    Some((d, _, _)) => (0.5 - d).clamp(0.0, 1.0),
                    None if !corner_row || (x >= left && x < right) => 1.0,
                    None => {
                        let dx = (prim[0] + rr - px).max(px - (prim[2] - rr)).max(0.0);
                        (rr + 0.5 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0)
                    }
                };
                if cov <= 0.0 {
                    continue;
                }

                let mut sample_at = (px, pyf);
                // The per-channel displacement, in the same units as
                // `sample_at`'s. Zero everywhere the bevel is not.
                let mut split = (0.0f32, 0.0f32);
                let mut spec = 0.0f32;
                // Extra tint strength from grazing incidence, and the
                // unlit rim's darkening. Both are rim-band only: `u` is
                // 0 in the interior and both fall out with it.
                let mut fres = 0.0f32;
                let mut dark = 0.0f32;
                if bevelled {
                    // Signed distance and the outward 2D normal --
                    // everywhere, not just in corners. The morph path
                    // has already produced both.
                    let (d, nx, ny) = match morph_d {
                        Some(v) => v,
                        None => blob(px, pyf, cx, cy),
                    };
                    // `u` is 1 at the boundary and 0 at `rim` px inside:
                    // the bevel's parameter.
                    let u = (1.0 + d / rim).clamp(0.0, 1.0);
                    if u > 0.0 {
                        // **The bevel is a spherical cap, and this is
                        // the whole of it.** `(u*n, sqrt(1 - u*u))` is
                        // already unit -- no division, no singularity at
                        // the rim -- and it is what gives the normal a
                        // *third* component. A 2D gradient is constant
                        // along a straight edge, so a highlight built on
                        // one is a uniform stripe; this one varies
                        // across the rim as well as around it, which is
                        // what reads as a curved surface.
                        let nz = (1.0 - u * u).max(0.0).sqrt();
                        if m.refraction > 0.0 {
                            let k = m.refraction * u * u;
                            sample_at = (px + nx * k, pyf + ny * k);
                            // **Blue deviates more than red.** Normal
                            // dispersion: the refractive index falls
                            // with wavelength. Getting it backwards
                            // flips which side of the rim the warm
                            // fringe lands on, which is the one thing
                            // an eye recognises as a lens.
                            if m.dispersion > 0 {
                                let e = k
                                    * (m.dispersion as f32 / 255.0)
                                    * lumen_static::display::Material::MAX_DISPERSION;
                                split = (nx * e, ny * e);
                            }
                        }
                        // **Fresnel — M22/T22.0, and the reason a thick
                        // edge reads as a bright band rather than a
                        // tinted one.** Real glass reflects far more at
                        // grazing incidence, and with the viewer on +z
                        // the cosine of the incidence angle *is* `nz`,
                        // already computed. Schlick's fifth power.
                        //
                        // Derived from `specular` rather than given a
                        // field: a bright rim and a highlight are the
                        // same property of the same edge, and a panel
                        // with no specular should have neither.
                        if m.specular > 0.0 {
                            let f = 1.0 - nz;
                            let f2 = f * f;
                            fres = f2 * f2 * f * m.specular;

                            let along = nx * u * light[0] + ny * u * light[1];
                            let dot = along + nz * light[2];
                            if dot > 0.0 {
                                let d2 = dot * dot;
                                let d8 = d2 * d2 * d2 * d2;
                                spec = d8 * d8 * d8 * m.specular;
                            }
                        }
                        // **The inner shadow rides u-squared, not the
                        // specular's negative half.** That half exists
                        // only where `u` exceeds the light's elevation
                        // — the outer 38% of the rim — so darkening it
                        // gives a hairline. A diffuse term on the
                        // horizontal components covers the whole unlit
                        // side and falls to zero in the interior, and it
                        // reuses two of the three products above.
                        if m.shadow > 0 {
                            let facing = -(nx * light[0] + ny * light[1]) / LIGHT_XY;
                            if facing > 0.0 {
                                dark = (m.shadow as f32 / 255.0) * u * u * facing;
                            }
                        }
                    }
                }

                let back = if split.0 != 0.0 || split.1 != 0.0 {
                    // **Fall back to the shared read the moment any of
                    // the three would clamp.** At the capture's edge
                    // three displacements clamp at three different
                    // distances, and an achromatic backdrop grows a
                    // coloured band indistinguishable from the feature.
                    // The fallback makes that case bit-identical to the
                    // pre-T22.2 renderer.
                    //
                    // The test is on the **extremes**, not on the green
                    // coordinate the plan named: green is the midpoint,
                    // so blue can clamp while green is still inside, and
                    // checking green alone would let exactly the band
                    // this guards against through on one side.
                    let cl = |v: f32, e: f32, lo: f32, hi: f32| {
                        let (a, b) = (v - e, v + e);
                        a.min(b) <= lo || a.max(b) >= hi
                    };
                    let edge = cl(sample_at.0, split.0, sx0 as f32 + 0.5, sx1 as f32 - 0.5)
                        || cl(sample_at.1, split.1, sy0 as f32 + 0.5, sy1 as f32 - 0.5);
                    if edge {
                        sample(sample_at.0, sample_at.1)
                    } else {
                        [
                            sample_ch(sample_at.0 - split.0, sample_at.1 - split.1, 0),
                            sample_ch(sample_at.0, sample_at.1, 1),
                            sample_ch(sample_at.0 + split.0, sample_at.1 + split.1, 2),
                        ]
                    }
                } else {
                    sample(sample_at.0, sample_at.1)
                };
                let back = saturate(back, sat);
                // Fresnel raises the tint's strength toward 1 at the
                // rim; `t` is unchanged in the interior where `fres` is
                // 0.
                let t = tv + (1.0 - tv) * fres;
                let mix = |c: f32, ti: f32| c + (ti - c) * t;
                // **Shadow multiplies, specular adds.** Subtracting
                // clips each channel at a different point and rotates
                // hue -- (30, 60, 220) less 200 is (0, 0, 20) and the
                // ratios are gone. A multiply cannot go below zero and
                // preserves them, which also stops the shadow undoing
                // the saturation boost in the same pixel.
                let shade = 1.0 - dark;
                let lit = |c: f32| (c * shade + spec * 255.0).clamp(0.0, 255.0);
                let c = Color(
                    lit(mix(back[0], tint[0])) as u8,
                    lit(mix(back[1], tint[1])) as u8,
                    lit(mix(back[2], tint[2])) as u8,
                    255,
                );
                self.blend_cov(x, y, c, (cov * 255.0) as u8);
            }
        }
        // **Put the buffers back**, or every call allocates three
        // fresh ones and the gate reports the same 3.00 per panel it
        // did before — which is exactly what the first attempt at
        // this did, and what the gate caught.
        *self.glass = scratch;
    }

    /// A soft shadow: `c` at the edge of the rounded rect `shape`,
    /// falling off to nothing `spread` px out, drawn in one pass from
    /// the signed distance to the shape. The falloff is quadratic — the
    /// cheap curve that reads as a blur. Pixels well inside the shape
    /// are skipped: whatever cast the shadow is painted over them next.
    fn shadow_within(
        &mut self,
        shape: [f32; 4],
        bounds: [f32; 4],
        c: Color,
        radius: f32,
        spread: f32,
    ) {
        if !shape.iter().all(|v| v.is_finite()) || !bounds.iter().all(|v| v.is_finite()) {
            return;
        }
        let rad = radius.clamp(0.0, (shape[2] - shape[0]).min(shape[3] - shape[1]) / 2.0);
        let (cx, cy) = ((shape[0] + shape[2]) / 2.0, (shape[1] + shape[3]) / 2.0);
        let (hx, hy) = ((shape[2] - shape[0]) / 2.0 - rad, (shape[3] - shape[1]) / 2.0 - rad);
        let (w, h) = (self.width as i64, self.y_hi());
        let (x0, y0) = ((bounds[0].floor() as i64).max(0), (bounds[1].floor() as i64).max(self.y0));
        let (x1, y1) = ((bounds[2].ceil() as i64).min(w), (bounds[3].ceil() as i64).min(h));

        // The shape's inset-by-`rad` rect is entirely inside it — the
        // rounded rect *is* that rect dilated by `rad` — so every pixel
        // in it is at distance <= -rad and contributes nothing. Skipping
        // it structurally rather than computing a distance and throwing
        // it away is most of this function: for the tally's card that is
        // 131k of 158k pixels, and it took a full render from 811 to
        // 478 us (`docs/results-frame-cost.md`).
        //
        // **This comment used to say the shadow went "to a tenth" of a
        // frame's render. It did not** (corrected 2026-09-09,
        // `docs/results-simd.md`). A tenth is true of the *damaged*
        // path, where the shadow costs 0-1 us. On a full redraw it is
        // still the largest single command by a wide margin: after the
        // skip, after T17.0's wide stores and after T17.1's vector
        // distance field, 180 us of a 270 us render -- 67%.
        let skip = rad >= 1.0;
        let (ix0, iy0) = ((shape[0] + rad).ceil() as i64, (shape[1] + rad).ceil() as i64);
        let (ix1, iy1) = ((shape[2] - rad).floor() as i64, (shape[3] - rad).floor() as i64);

        // Row-oriented since T17.1: the inner loop hands a whole run of
        // one row to `shadow_run`, which is where the signed-distance
        // field is evaluated. Two things follow from that shape and both
        // are the point of it — the per-pixel bounds check inside
        // `blend` becomes one slice bound for the run, and the distance
        // field becomes eight lanes wide under `feature = "simd"`.
        //
        // This is 74% of a full redraw of the tally: 255 us of 346, for
        // *one* command (`docs/results-simd.md`).
        let stride = (self.width * 4) as usize;
        for y in y0..y1 {
            let qy = (y as f32 + 0.5 - cy).abs() - hy;
            let inner_row = skip && y >= iy0 && y < iy1;
            let row = &mut self.px[(y - self.y0) as usize * stride..((y - self.y0) as usize + 1) * stride];
            if inner_row {
                // The interior contributes nothing, so the row is two
                // runs with a hole between them.
                shadow_run(row, x0, x1.min(ix0), qy, cx, hx, rad, spread, c);
                shadow_run(row, x0.max(ix1), x1, qy, cx, hx, rad, spread, c);
            } else {
                shadow_run(row, x0, x1, qy, cx, hx, rad, spread, c);
            }
        }
    }

    /// Clear only `r`. A partial redraw clears the region it is about
    /// to repaint and leaves the rest of the frame standing — clearing
    /// the whole surface first would make the "partial" half pointless.
    fn clear_within(&mut self, r: [f32; 4], c: Color) {
        let opaque = Color(c.0, c.1, c.2, 255);
        self.fill(r, opaque);
    }

    /// One row, `[x0, x1)`, already clamped to the surface. Opaque
    /// colours are written, not blended: a card's background is most of
    /// the pixels in a frame.
    fn span(&mut self, y: i64, x0: i64, x1: i64, c: Color) {
        if x0 >= x1 || y < self.y0 || y >= self.y_hi() || c.3 == 0 {
            return;
        }
        if c.3 == 255 {
            let r0 = (y - self.y0) as u32;
            let start = ((r0 * self.width + x0 as u32) * 4) as usize;
            let end = ((r0 * self.width + x1 as u32) * 4) as usize;
            fill_px(&mut self.px[start..end], [c.0, c.1, c.2, 255]);
        } else {
            for x in x0..x1 {
                self.blend(x, y, c);
            }
        }
    }

    /// Stroke a line, one span of the clip at a time.
    ///
    /// A distance test per pixel over the bounding box rather than a
    /// Bresenham walk: it gives a round cap and antialiasing for free,
    /// and a UI's lines are short. A chart with a million segments wants
    /// a different renderer, and `Renderer` is where one plugs in.
    fn stroke_line(&mut self, a: Point, b: Point, width: f32, c: Color, clip: &[[f32; 4]]) {
        let hw = (width / 2.0).max(0.5);
        let bounds = [a.x.min(b.x) - hw, a.y.min(b.y) - hw, a.x.max(b.x) + hw, a.y.max(b.y) + hw];
        let Some(r) = clipped(bounds, clip) else { return };
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len2 = dx * dx + dy * dy;
        for y in (r[1].floor() as i64).max(self.y0)..(r[3].ceil() as i64).min(self.y_hi()) {
            for x in r[0].floor().max(0.0) as i64..(r[2].ceil() as i64).min(self.width as i64) {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                // Distance from the segment, not the infinite line: a
                // line must end where it ends.
                let t = if len2 > 0.0 {
                    clamp01(((px - a.x) * dx + (py - a.y) * dy) / len2)
                } else {
                    0.0
                };
                let (qx, qy) = (a.x + dx * t, a.y + dy * t);
                let d = ((px - qx) * (px - qx) + (py - qy) * (py - qy)).sqrt();
                // One pixel of feather at the edge, which is what makes
                // a diagonal readable.
                let cov = clamp01(hw + 0.5 - d);
                if cov > 0.0 {
                    self.blend_cov(x, y, c, (cov * 255.0) as u8);
                }
            }
        }
    }

    /// Fill `dst` with a gradient, limited to `area` (M82/T82.2).
    ///
    /// The parameter comes from [`GradAxis`], the formula the GPU backend
    /// uploads too. With `corner > 0` each pixel's colour is multiplied
    /// by the rounded box's coverage from [`sd_round_box`] -- the
    /// expression `shader.wgsl`'s `KIND_GRADIENT` case applies to the same
    /// `params.x` -- so a rounded gradient's corner antialiases as a
    /// `RoundedRect`'s does. At `corner == 0` there is no coverage term at
    /// all: the fill stays hard-edged, byte-identical to the gradient
    /// before M82 (`tests/golden.rs`'s `gradient3`).
    fn fill_gradient(
        &mut self,
        dst: [f32; 4],
        area: [f32; 4],
        stops: Stops,
        kind: GradKind,
        geom: [f32; 3],
        corner: f32,
    ) {
        let axis = GradAxis::new(dst, kind, geom);
        // The corner clamps to half the shorter side, as a rounded rect's
        // does; past that `sd_round_box` would describe no box at all.
        let rad = corner.min((dst[2] - dst[0]) / 2.0).min((dst[3] - dst[1]) / 2.0);
        let rounded = rad > 0.0;
        for y in (area[1].floor() as i64).max(self.y0)..(area[3].ceil() as i64).min(self.y_hi()) {
            for x in area[0].floor().max(0.0) as i64..(area[2].ceil() as i64).min(self.width as i64)
            {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let c = stops.sample(axis.t(px, py));
                if rounded {
                    let cov = rr_cov(px, py, dst, rad);
                    if cov > 0.0 {
                        self.blend_cov(x, y, c, (cov * 255.0) as u8);
                    }
                } else {
                    self.blend(x, y, c);
                }
            }
        }
    }

    /// Fill and then stroke a path whose ink is `ink`, writing only inside
    /// `area` (its clipped part) — M82/T82.5, through [`rasterize_path`].
    ///
    /// **The mask's x origin is the ink's left edge on the surface, never
    /// the damage's.** Coverage is a running sum along each row, so a
    /// mask that started at a damage edge would add the same areas in a
    /// different order and could round a pixel differently from the full
    /// frame: a partial redraw must be byte-identical to a full one. Rows
    /// are independent (`rasterize_path`'s rustdoc), so the rows are
    /// limited to the area and the band.
    fn path_within(
        &mut self,
        els: &[lumen_static::display::PathEl],
        ink: [f32; 4],
        area: [f32; 4],
        fill: Color,
        stroke: Color,
        width: f32,
    ) {
        let x0 = (ink[0].floor() as i64).max(0);
        let x1 = (ink[2].ceil() as i64).min(self.width as i64);
        let ax0 = (area[0].floor() as i64).max(x0);
        let ax1 = (area[2].ceil() as i64).min(x1);
        let y0 = (area[1].floor() as i64).max(self.y0);
        let y1 = (area[3].ceil() as i64).min(self.y_hi());
        if ax0 >= ax1 || y0 >= y1 {
            return;
        }
        let (w, h) = ((x1 - x0) as usize, (y1 - y0) as usize);
        for (paint, cover) in [(fill, PathCoverage::Fill), (stroke, PathCoverage::Stroke(width))] {
            if paint.3 == 0 || matches!(cover, PathCoverage::Stroke(v) if v.is_nan() || v <= 0.0) {
                continue;
            }
            let mut scratch = core::mem::take(self.paths);
            let mask = rasterize_path(els, cover, [x0 as i32, y0 as i32], w, h, &mut scratch);
            for y in y0..y1 {
                let row = (y - y0) as usize * w;
                for x in ax0..ax1 {
                    let cov = mask[row + (x - x0) as usize];
                    if cov > 0 {
                        self.blend_cov(x, y, paint, cov);
                    }
                }
            }
            *self.paths = scratch;
        }
    }

    /// Blit an image into `dst`, limited to `area`.
    ///
    /// Nearest-neighbour: this is a software canvas whose whole point is
    /// predictable cost, and a bilinear resample is four fetches and
    /// three lerps per pixel for a smoothness nothing here has asked
    /// for. `Fit` already keeps the aspect, so the common case is a
    /// scale close to 1.
    fn blit_image(
        &mut self,
        dst: [f32; 4],
        area: [f32; 4],
        iw: u32,
        ih: u32,
        px: &[u8],
        tint: Color,
    ) {
        let (dw, dh) = (dst[2] - dst[0], dst[3] - dst[1]);
        if dw <= 0.0 || dh <= 0.0 {
            return;
        }
        let (x0, y0) = (area[0].floor().max(0.0) as i64, (area[1].floor() as i64).max(self.y0));
        let (x1, y1) = (
            (area[2].ceil() as i64).min(self.width as i64),
            (area[3].ceil() as i64).min(self.y_hi()),
        );
        for y in y0..y1 {
            let v = ((y as f32 + 0.5 - dst[1]) / dh * ih as f32) as i64;
            if v < 0 || v >= ih as i64 {
                continue;
            }
            for x in x0..x1 {
                let u = ((x as f32 + 0.5 - dst[0]) / dw * iw as f32) as i64;
                if u < 0 || u >= iw as i64 {
                    continue;
                }
                let o = ((v as u32 * iw + u as u32) * 4) as usize;
                let s = &px[o..o + 4];
                // Tint multiplies, so `Color::WHITE` is untinted and one
                // monochrome asset can serve several colours.
                let m = |a: u8, b: u8| ((a as u32 * b as u32) / 255) as u8;
                self.blend(x, y, Color(m(s[0], tint.0), m(s[1], tint.1), m(s[2], tint.2), m(s[3], tint.3)));
            }
        }
    }

    fn blend(&mut self, x: i64, y: i64, c: Color) {
        if x < 0 || y < self.y0 || x >= self.width as i64 || y >= self.y_hi() || c.3 == 0 {
            return;
        }
        let i = (((y - self.y0) as u32 * self.width + x as u32) * 4) as usize;
        let a = c.3 as u32;
        let inv = 255 - a;
        for (k, src) in [c.0, c.1, c.2].into_iter().enumerate() {
            self.px[i + k] = ((src as u32 * a + self.px[i + k] as u32 * inv) / 255) as u8;
        }
        self.px[i + 3] = 255;
    }

    /// Blend with a coverage mask applied to the colour's alpha — a
    /// glyph's or an edge's.
    fn blend_cov(&mut self, x: i64, y: i64, c: Color, cov: u8) {
        let a = (c.3 as u32 * cov as u32 / 255) as u8;
        self.blend(x, y, Color(c.0, c.1, c.2, a));
    }

    fn fill(&mut self, r: [f32; 4], c: Color) {
        // Clamp to the surface *before* iterating. `blend` rejects
        // out-of-range pixels, but a `[-1e8, -1e8, 1e8, 1e8]` rect — a
        // `Fill` node under an unbounded constraint — would still loop
        // ~10^16 times getting there. A non-finite rect draws nothing.
        if !r.iter().all(|v| v.is_finite()) {
            return;
        }
        let (w, h) = (self.width as i64, self.y_hi());
        let (x0, y0) = ((r[0].floor() as i64).max(0), (r[1].floor() as i64).max(self.y0));
        let (x1, y1) = ((r[2].ceil() as i64).min(w), (r[3].ceil() as i64).min(h));
        for y in y0..y1 {
            self.span(y, x0, x1, c);
        }
    }

    /// Walk a display list into this band.
    ///
    /// `font` and `images` arrive as parameters rather than fields
    /// because a band borrows only pixels (T17.4). That also removes the
    /// take-and-put-back dance the old `render_impl` needed to get
    /// around borrowing `self.images` while `&mut self` was live.
    // The command index finds a run's shaping (M50/T50.3), which only the
    // truetype build has.
    #[cfg_attr(not(feature = "truetype"), allow(clippy::unused_enumerate_index))]
    fn render(
        &mut self,
        list: &DisplayList,
        damage: Option<[f32; 4]>,
        #[cfg(feature = "truetype")] font: Option<&crate::text::FontStack>,
        images: Option<&dyn lumen_static::ImageSource>,
    ) {
        // **The band's own rows are a clip, and saying so is what makes
        // a strip cheap** (M17/T17.5).
        //
        // Every command is culled against the clip stack before any work
        // is done for it. For text this used to be what stood between the
        // arm and shaping and rasterising a whole run: without the band in
        // the stack a full redraw had an empty one, so every strip
        // re-shaped every run in the frame — at 4K with 8-row strips 270
        // strips x 192 runs, and the "parallel" render came out **11x
        // slower than one thread**. Since M50/T50.3 the frame thread
        // shapes each run once before any band starts (`TextState::
        // prepare`) and a band only blits, so for text this cull now saves
        // a lookup rather than a shaping.
        //
        // Intersecting the damage rect with the band gives one rect, so
        // the stack still starts one deep and `PopClip`'s floor below is
        // unchanged.
        let band = [0.0, self.y0 as f32, self.width as f32, self.y_hi() as f32];
        // **The damage is snapped outward to whole pixels** (M57/D6),
        // as `lumen-static-wgpu`'s `snap_out` does. `fill` rounds a rect
        // outward and the glyph blit truncates one, so a region ending
        // at 608.4 px — a popup's edge at scale 2 — had its row 608
        // cleared by `clear_within` and its glyph ink clipped away: one
        // stale row under the trigger's text on every partial repaint,
        // where a full repaint's integer band drew it.
        let outer = match damage {
            Some(d) => {
                let d = [d[0].floor(), d[1].floor(), d[2].ceil(), d[3].ceil()];
                [d[0].max(band[0]), d[1].max(band[1]), d[2].min(band[2]), d[3].min(band[3])]
            }
            None => band,
        };
        // **A fixed stack, not a `Vec`** (M26/T26.13). This allocated
        // once per rasterised frame — one heap call in the path this
        // repository's central invariant says has none, and which no
        // test could see until the allocation gate was extended to
        // cover a raster frame. The shape is `InputQueue`'s and
        // `Overlays`': a fixed depth, and an explicit answer when it is
        // reached.
        let mut clip = ClipStack::new(outer);
        // Where this band is in the frame's shaped runs (M50/T50.3).
        #[cfg(feature = "truetype")]
        let mut cursor = 0usize;
        for (_index, c) in list.cmds.iter().enumerate() {
            match c {
                Cmd::Rect { rect, color } => {
                    if let Some(r) = clipped(*rect, clip.as_slice()) {
                        self.fill(r, *color);
                    }
                }
                Cmd::RoundedRect { rect, color, radius } => {
                    if let Some(r) = clipped(*rect, clip.as_slice()) {
                        // Corners are rounded against the *original*
                        // rect and the fill is limited to the clipped
                        // one, so a clipped card does not square off.
                        self.fill_rounded_within(*rect, r, *color, *radius);
                    }
                }
                Cmd::StrokedRect { rect, color, radius, width } => {
                    if let Some(r) = clipped(*rect, clip.as_slice()) {
                        // Same split as `RoundedRect`: the ring is
                        // computed against the original box and written
                        // only inside the clipped one, so a clipped
                        // border does not square off at the clip edge.
                        self.stroke_rounded_within(*rect, r, *color, *radius, *width);
                    }
                }
                Cmd::Glass { rect, radius, material } => {
                    if let Some(r) = clipped(*rect, clip.as_slice()) {
                        self.glass_within(*rect, r, *radius, *material);
                    }
                }
                Cmd::Shadow { rect, color, radius, spread } => {
                    let reach =
                        [rect[0] - spread, rect[1] - spread, rect[2] + spread, rect[3] + spread];
                    if let Some(r) = clipped(reach, clip.as_slice()) {
                        self.shadow_within(*rect, r, *color, *radius, *spread);
                    }
                }
                // `line_height` reaches the glyphs through `prepare`, which
                // shaped this run already; the block-glyph path below draws
                // one line and has no pitch to apply.
                Cmd::Text { rect, text, size, weight, line_height: _, color } => {
                    // Cull the run before shaping it.
                    //
                    // Every other arm of this match tests `clipped` first
                    // and skips; text did not, so a run outside the damage
                    // rect was still shaped and rasterised and only its
                    // *glyphs* were clipped, one at a time. On a 3000-row
                    // list that is 3000 shaping calls per frame for the ~40
                    // rows a 800 px viewport can show — measured at 7.3 ms a
                    // frame before this check and 0.16 ms after
                    // (`docs/results-competitive.md`).
                    //
                    // Padded by `size`: a glyph may overhang its run's rect
                    // (an italic tail, a wide bearing), and culling on the
                    // bare rect would clip those at the boundary. One em is
                    // more than any face overhangs and costs nothing.
                    //
                    // **Per run, so it bounds nothing about a long run**: a
                    // run that meets the damage is shaped and rasterised
                    // whole, and its glyphs are clipped one by one below. A
                    // widget with a lot of text paints it as several runs —
                    // `TextArea` one per visible line since M45/T45.11,
                    // `RichText` one per word — so that this cull has
                    // something to cull.
                    let pad = *size;
                    let reach = [rect[0] - pad, rect[1] - pad, rect[2] + pad, rect[3] + pad];
                    if clipped(reach, clip.as_slice()).is_none() {
                        continue;
                    }
                    let s = list.str_of(*text);
                    #[cfg(feature = "truetype")]
                    if let Some(font) = font {
                        self.draw_run(font, _index as u32, &mut cursor, rect, *color, clip.as_slice());
                        continue;
                    }
                    let _ = weight;
                    let advance = size * 0.5;
                    for (k, ch) in s.chars().enumerate() {
                        if ch == ' ' {
                            continue;
                        }
                        let x = rect[0] + k as f32 * advance;
                        if x + advance > rect[2] {
                            break; // clipped by the node's own box
                        }
                        // One block per glyph, inset so runs stay legible.
                        let g = [x + 0.5, rect[1] + 2.0, x + advance - 0.5, rect[1] + size - 2.0];
                        if let Some(r) = clipped(g, clip.as_slice()) {
                            self.fill(r, *color);
                        }
                    }
                }
                Cmd::Line { a, b, width, color } => {
                    let w = width / 2.0;
                    let bounds =
                        [a.x.min(b.x) - w, a.y.min(b.y) - w, a.x.max(b.x) + w, a.y.max(b.y) + w];
                    if clipped(bounds, clip.as_slice()).is_some() {
                        self.stroke_line(*a, *b, *width, *color, clip.as_slice());
                    }
                }
                Cmd::Gradient { rect, stops, kind, geom, corner } => {
                    if let Some(r) = clipped(*rect, clip.as_slice()) {
                        self.fill_gradient(*rect, r, *stops, *kind, *geom, *corner);
                    }
                }
                Cmd::Image { rect, id, tint, frame } => {
                    // Culled before anything is fetched, like every
                    // other command and like text since the 46x fix
                    // (`docs/results-competitive.md` §6.1).
                    let Some(r) = clipped(*rect, clip.as_slice()) else { continue };
                    let Some(src) = images else { continue };
                    // A missing asset draws nothing rather than taking
                    // the frame down: `ImageSource` returns `Option` for
                    // exactly that. Each command is resolved at its own
                    // frame (M84/T84.2), so one id at two frames in one
                    // list draws both.
                    let Some((iw, ih, px)) = src.image(*id, *frame) else { continue };
                    if iw == 0 || ih == 0 || px.len() < (iw as usize * ih as usize * 4) {
                        continue;
                    }
                    self.blit_image(*rect, r, iw, ih, px, *tint);
                }
                Cmd::Path { rect, els, fill, stroke, width } => {
                    let Some(r) = clipped(*rect, clip.as_slice()) else { continue };
                    // A range outside the store draws nothing: a list from
                    // `DisplayList::path` cannot hold one, and `wire::decode`
                    // refuses one, but a renderer does not trust either.
                    let (at, n) = (els.0 as usize, els.1 as usize);
                    let Some(els) = list.paths.get(at..at + n) else { continue };
                    self.path_within(els, *rect, r, *fill, *stroke, *width);
                }
                Cmd::PushClip { rect } => clip.push(*rect),
                Cmd::PopClip => {
                    // Never pop the damage scope: it is not part of the
                    // list's own balance, and an unbalanced list (which
                    // `tests/paint_containment.rs` forbids, but a
                    // renderer must not trust) would otherwise let a
                    // later command paint outside the region.
                    // Never pop the band scope: it is not part of the
                    // list's own balance, and an unbalanced list (which
                    // `tests/paint_containment.rs` forbids, but a
                    // renderer must not trust) would otherwise let a
                    // later command paint outside this strip's rows.
                    // `ClipStack::pop` keeps the floor itself — slot 0
                    // is the band, never popped — and also declines the
                    // pops that match pushes it refused for depth.
                    clip.pop();
                }
                // **A shader bracket is a clip here** (M83/T83.3,
                // `docs/plan-gpu-extension.md`): the canvas runs no shader,
                // so it draws the fallback recorded inside the bracket,
                // clipped to the shader's rect. The end pops it, with
                // `PopClip`'s floor.
                #[cfg(lumen_shader)]
                Cmd::Shader { rect, .. } => clip.push(*rect),
                #[cfg(lumen_shader)]
                Cmd::ShaderEnd => clip.pop(),
            }
        }
    }
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Self {
        Canvas {
            width,
            height,
            pixels: vec![255; (width * height * 4) as usize],
            glass: GlassScratch::default(),
            paths: PathScratch::default(),
            #[cfg(feature = "truetype")]
            font: None,
            #[cfg(feature = "truetype")]
            text: TextState::default(),
            images: None,
        }
    }

    /// A canvas that draws real glyphs.
    #[cfg(feature = "truetype")]
    pub fn with_font(width: u32, height: u32, font: crate::text::FontStack) -> Self {
        let mut c = Self::new(width, height);
        c.font = Some(font);
        c
    }

    /// **Draw text with `font` from the next frame on** — M81/T81.4: how a
    /// window's software backend follows a swap that changed the
    /// program's `Program::shaping` (`Backend::set_font` in
    /// `lumen-static-winit`). The text scratch and the glyph cache go with
    /// the old stack: an id is only promised identical bitmaps from the
    /// stack that made it (`text::GlyphId`), so nothing cached is kept
    /// across a change of stack. What is already on the canvas is not
    /// redrawn here; the caller repaints all.
    #[cfg(feature = "truetype")]
    pub fn set_font(&mut self, font: crate::text::FontStack) {
        self.font = Some(font);
        self.text = TextState::default();
    }

    /// What this canvas's glyph cache has done — M50/T50.2. `uncached`
    /// is the count to watch: it must read 0 once a screen is warm
    /// ([`crate::text::CacheStats`]).
    #[cfg(feature = "truetype")]
    pub fn glyph_cache_stats(&self) -> crate::text::CacheStats {
        self.text.cache.stats()
    }

    /// Resize the pixel buffer **in place**, keeping everything else.
    ///
    /// **A method rather than a fresh `Canvas`, and that is the point.**
    /// `lumen-static-winit`'s `sync_size` used to do `*canvas =
    /// Canvas::new(w, h)`, which silently dropped every other field —
    /// and the moment one of those fields carried something a caller had
    /// installed, that became a bug nobody could see: `Config::images`
    /// reached the canvas at startup, the first frame resized it because
    /// a window manager had clamped the window, and the wallpaper
    /// vanished on both backends. A whole-struct replacement is a field
    /// list that nobody wrote down and the compiler will not check.
    ///
    /// Cheap and idempotent when the size has not changed.
    ///
    /// **Exact growth, and memory given back** — M50/T50.6, R8-F7. This
    /// cleared the `Vec` and `resize`d it, so a grow past capacity took
    /// `Vec`'s amortised doubling — one pixel wider at 4K reserved a second
    /// 4K frame, 66 MB for 33 — and nothing ever shrank it. Now a grow past
    /// capacity reserves exactly the new length, a shrink below half the
    /// capacity gives the rest back, and anything else reuses the buffer in
    /// place, so a drag of a pixel or two allocates only when it passes the
    /// largest size it has held. Both reallocating paths drop the old
    /// buffer before taking the new one: the old pixels are about to be
    /// overwritten, so copying them would be waste and holding both would
    /// double the peak. `tests/alloc_gate.rs`
    /// `resizing_the_canvas_grows_exactly_and_gives_memory_back` holds all
    /// three. A resize is not the steady state (`docs/results-allocation.md`).
    ///
    /// softbuffer keeps its own buffer, whose growth is outside this crate.
    pub fn resize(&mut self, width: u32, height: u32) {
        if (self.width, self.height) == (width, height) {
            return;
        }
        self.width = width;
        self.height = height;
        let n = (width as usize * height as usize) * 4;
        let cap = self.pixels.capacity();
        if n > cap || n < cap / 2 {
            self.pixels = Vec::new();
            self.pixels.reserve_exact(n);
        } else {
            self.pixels.clear();
        }
        self.pixels.resize(n, 255);
    }

    /// Copy the frame into a `0RGB` `u32` buffer — softbuffer's format.
    /// Lives here rather than in the shell binding so it is compiled
    /// with the rasteriser's optimisation level: in the example it was
    /// 2 ms of a 3 ms frame.
    pub fn blit_0rgb(&self, dst: &mut [u32]) {
        // **The swizzle is lane-uniform arithmetic, not a shuffle**
        // (M17/T17.2, `docs/results-simd.md`). RGBA little-endian is
        // `R | G<<8 | B<<16 | A<<24`; `0RGB` is `B | G<<8 | R<<16`. So
        // the conversion is three masks, two shifts and two ors — all
        // `u32` ops that vectorise eight lanes wide with no
        // deinterleaving, which is what made this one tractable when
        // the blend was not.
        //
        // T17.0 tried it four pixels at a time with `from_le_bytes` and
        // moved the needle 1.06x; the *same arithmetic* written scalar
        // is actually 0.94x, i.e. slower. It is the vector width that
        // pays here, not the spelling.
        #[cfg(feature = "simd")]
        {
            #[cfg(feature = "raster-counters")]
            VECTOR[0].fetch_add(1, core::sync::atomic::Ordering::Relaxed);
            use wide::u32x8;
            let (m_r, m_g, m_b) = (
                u32x8::splat(0x0000_00FF),
                u32x8::splat(0x0000_FF00),
                u32x8::splat(0x00FF_0000),
            );
            let mut d8 = dst.chunks_exact_mut(8);
            let mut s32 = self.pixels.chunks_exact(32);
            for (dq, sq) in (&mut d8).zip(&mut s32) {
                let mut arr = [0u32; 8];
                for (k, a) in arr.iter_mut().enumerate() {
                    *a = u32::from_le_bytes([
                        sq[k * 4],
                        sq[k * 4 + 1],
                        sq[k * 4 + 2],
                        sq[k * 4 + 3],
                    ]);
                }
                let v = u32x8::from(arr);
                let r: u32x8 = ((v & m_r) << 16) | (v & m_g) | ((v & m_b) >> 16);
                dq.copy_from_slice(&r.to_array());
            }
            let tail_src = s32.remainder();
            for (d, px) in d8.into_remainder().iter_mut().zip(tail_src.chunks_exact(4)) {
                *d = u32::from_le_bytes([px[2], px[1], px[0], 0]);
            }
        }
        #[cfg(not(feature = "simd"))]
        for (d, px) in dst.iter_mut().zip(self.pixels.chunks_exact(4)) {
            *d = u32::from_le_bytes([px[2], px[1], px[0], 0]);
        }
    }

    /// [`Canvas::blit_0rgb`] for one region — the rows a partial
    /// redraw touched. `dst` is the whole surface, not the region: the
    /// stride is the canvas width either way.
    pub fn blit_0rgb_within(&self, dst: &mut [u32], r: [f32; 4]) {
        let (w, h) = (self.width as i64, self.height as i64);
        let x0 = (r[0].floor() as i64).clamp(0, w) as usize;
        let y0 = (r[1].floor() as i64).clamp(0, h) as usize;
        let x1 = (r[2].ceil() as i64).clamp(0, w) as usize;
        let y1 = (r[3].ceil() as i64).clamp(0, h) as usize;
        let stride = self.width as usize;
        for y in y0..y1 {
            let row = y * stride;
            // **The same swizzle as `blit_0rgb`, and the same vector
            // form.** T17.2 vectorised only the whole-surface blit and
            // left this one scalar — which is backwards, because this
            // is the one a frame actually calls: the full blit runs on
            // a resize, this runs every damaged frame. `blit_within`
            // was 7 us of a 19 us damaged tally frame, 37% of it, and
            // measured 0.97 us in the per-stage profiler under both
            // builds, which is what exposed the omission (M17/T17.3).
            blit_run(&self.pixels, &mut dst[row..row + stride], row, x0, x1);
        }
    }

    /// Portable pixmap — no image dependency, and any viewer opens it.
    pub fn to_ppm(&self) -> Vec<u8> {
        let mut out = format!("P6\n{} {}\n255\n", self.width, self.height).into_bytes();
        for px in self.pixels.chunks_exact(4) {
            out.extend_from_slice(&px[..3]);
        }
        out
    }

    /// Fraction of pixels that are not the clear colour — a cheap
    /// "something was drawn" assertion for headless tests.
    pub fn ink_fraction(&self, clear: Color) -> f32 {
        let mut inked = 0usize;
        for px in self.pixels.chunks_exact(4) {
            if px[0] != clear.0 || px[1] != clear.1 || px[2] != clear.2 {
                inked += 1;
            }
        }
        inked as f32 / (self.width * self.height) as f32
    }
}

impl Canvas {
    /// Render only what falls inside `damage`.
    ///
    /// Every command is still walked, in order, and every command whose
    /// ink reaches the region is drawn — clipped to it. That is what
    /// makes the result identical to a full render of the same list:
    /// overlap is resolved by paint order exactly as before, and a card's
    /// shadow *is* repainted where it lies under a changed label,
    /// because it intersects. Skipping a command that intersects, or
    /// drawing them out of order, is what a per-node retained list would
    /// have to get right; walking the list does not have the problem.
    ///
    /// The clip stack is honoured throughout: `damage` is pushed as the
    /// outermost scope, so a command cannot escape it any more than it
    /// can escape a scroller.
    /// How many band workers a render of `damage` would use — 1 meaning
    /// "this frame will not be split at all".
    ///
    /// **This exists so a threading test can assert its own setup**, and
    /// it was added because one could not. `tests/banded.rs` claimed its
    /// five sizes were "large enough to cross the splitter's threshold
    /// at every one of them" and **six of its ten cases never split** —
    /// they compared a single-band render with itself across two builds,
    /// which is `AGENT.md`'s first vacuous shape and the seventh
    /// instance of it found on this repository. The claim was wrong by a
    /// factor of two: `workers_for` returns `px / 250_000`, so the
    /// threshold to *split* is 500 000 px, not the 250 000 floor.
    ///
    /// Always 1 without `threads`, which is the truth for that build.
    /// [`Canvas::bands_for`], but for a specific list — 1 when the list
    /// contains a backdrop filter, which cannot be banded (M19/T19.5).
    ///
    /// Exists so `tests/banded.rs` can **assert** the decline rather
    /// than infer it from pixels that happen to match. Shares
    /// `has_glass` with the splitter, so the predicate a test pins
    /// cannot drift from the decision the renderer makes.
    pub fn bands_for_list(&self, list: &DisplayList, damage: Option<[f32; 4]>) -> usize {
        if has_glass(list) {
            return 1;
        }
        self.bands_for(damage)
    }

    pub fn bands_for(&self, damage: Option<[f32; 4]>) -> usize {
        #[cfg(feature = "threads")]
        {
            let (_, _, rows) = self.band_rows(damage);
            let w = self.workers_for_damage(damage);
            if w < 2 || rows < 2 || (self.width * 4) == 0 {
                1
            } else {
                w
            }
        }
        #[cfg(not(feature = "threads"))]
        {
            let _ = damage;
            1
        }
    }

    pub fn render_within(&mut self, list: &DisplayList, damage: [f32; 4]) {
        self.render_impl(list, Some(damage));
    }

    /// Fill the whole surface.
    pub fn clear(&mut self, c: Color) {
        fill_px(&mut self.pixels, [c.0, c.1, c.2, c.3]);
    }

    /// Clear only `r`. A partial redraw clears the region it is about
    /// to repaint and leaves the rest of the frame standing — clearing
    /// the whole surface first would make the "partial" half pointless.
    pub fn clear_within(&mut self, r: [f32; 4], c: Color) {
        self.whole().clear_within(r, c);
    }

    /// The whole surface as one band.
    fn whole(&mut self) -> Surface<'_> {
        Surface {
            px: &mut self.pixels,
            width: self.width,
            y0: 0,
            rows: self.height as i64,
            glass: &mut self.glass,
            paths: &mut self.paths,
            #[cfg(feature = "truetype")]
            text: &self.text,
        }
    }

    fn render_impl(&mut self, list: &DisplayList, damage: Option<[f32; 4]>) {
        #[cfg(feature = "truetype")]
        let font = self.font.clone();
        // Shape every visible run once, here, before any band draws
        // (M50/T50.3). The outer clip is the one the whole-surface band
        // starts with, which contains every strip's.
        #[cfg(feature = "truetype")]
        if let Some(f) = font.as_ref() {
            let (w, h) = (self.width as f32, self.height as f32);
            let outer = match damage {
                Some(d) => [d[0].max(0.0), d[1].max(0.0), d[2].min(w), d[3].min(h)],
                None => [0.0, 0.0, w, h],
            };
            self.text.prepare(list, outer, f);
        }
        let images = self.images.take();
        {
            #[cfg(feature = "threads")]
            let split = self.render_banded(
                list,
                damage,
                #[cfg(feature = "truetype")]
                font.as_ref(),
                images.as_deref(),
            );
            #[cfg(not(feature = "threads"))]
            let split = false;

            if !split {
                let mut band = self.whole();
                band.render(
                    list,
                    damage,
                    #[cfg(feature = "truetype")]
                    font.as_ref(),
                    images.as_deref(),
                );
            }
        }
        self.images = images;
    }

    /// Rasterise the damaged rows across threads, one band each — T17.4.
    ///
    /// Returns whether it did; `false` means the caller should render
    /// the ordinary way. Refusing is the common case and is not a
    /// failure: below the threshold a thread costs more to wake than the
    /// rows cost to draw.
    ///
    /// **Byte-identical to the single band by construction.** Bands own
    /// The absolute row range a render of `damage` would touch, and its
    /// height. Shared by the splitter and by [`Canvas::bands_for`], so
    /// the predicate a test asserts on cannot drift from the decision
    /// the renderer makes.
    #[cfg(feature = "threads")]
    fn band_rows(&self, damage: Option<[f32; 4]>) -> (usize, usize, usize) {
        let h = self.height as i64;
        let area = damage.unwrap_or([0.0, 0.0, self.width as f32, self.height as f32]);
        let y_lo = (area[1].floor() as i64).clamp(0, h) as usize;
        let y_hi = (area[3].ceil() as i64).clamp(0, h) as usize;
        (y_lo, y_hi, y_hi.saturating_sub(y_lo))
    }

    #[cfg(feature = "threads")]
    fn workers_for_damage(&self, damage: Option<[f32; 4]>) -> usize {
        workers_for(self.band_rows(damage).2 * self.width as usize)
    }

    /// disjoint rows, and each one draws *every* command whose ink
    /// reaches those rows, in list order — the same rule that makes a
    /// partial redraw equal a full one. Nothing is skipped and nothing
    /// is reordered, so there is no interleaving for two threads to get
    /// wrong.
    ///
    /// **It allocates, and exactly this much: `std::thread::scope`'s 3 per
    /// worker plus 1, on the frame thread** — a recorded budget, not a
    /// zero (M50/T50.4, `docs/results-allocation.md`). The workers' own
    /// code allocates nothing: text arrives shaped and cached from the
    /// frame thread (T50.3). A persistent pool would remove the spawns,
    /// but without `unsafe` it cannot borrow this frame's list or rows and
    /// must copy the rows out and back — 615 µs a frame at 1080p against a
    /// 216 µs banded frame (`benches --bin banded_pool`).
    /// `tests/alloc_gate_threads.rs` holds both halves.
    #[cfg(feature = "threads")]
    fn render_banded(
        &mut self,
        list: &DisplayList,
        damage: Option<[f32; 4]>,
        #[cfg(feature = "truetype")] font: Option<&crate::text::FontStack>,
        images: Option<&dyn lumen_static::ImageSource>,
    ) -> bool {
        // `&dyn ImageSource` carries no `Sync` bound, so a frame with
        // images renders on one thread. Adding the bound would change a
        // core trait for a case nothing in this workspace exercises --
        // `Canvas::images` has no setter outside tests -- and images are
        // not what a large repaint is made of.
        if images.is_some() {
            return false;
        }
        // **A backdrop filter cannot run in a band** — M19/T19.5.
        //
        // A band owns disjoint rows and that is the whole safety
        // argument; `Cmd::Glass` captures a source region reaching
        // `3σ + 4` px past the panel on every side, into rows another
        // worker owns and is concurrently writing.
        //
        // The plan's T19.5 was to segment the command list at glass
        // boundaries and join between segments, reconstructing the clip
        // stack at each. Not built, and `docs/results-glass.md` §7 has
        // the measurement that decided it: a toolbar-sized panel is
        // 0.42 ms in total, so segmentation, a clip prefix per boundary
        // and a per-segment worker count buy nothing at the size a panel
        // actually is. The condition to revisit is a panel of a few
        // hundred thousand pixels, which §2 prices at 5 ms and up.
        //
        // `tests/banded.rs` asserts the decline through
        // [`Canvas::bands_for_list`], so this cannot silently start
        // threading.
        if has_glass(list) {
            return false;
        }
        let stride = (self.width * 4) as usize;
        if stride == 0 {
            return false;
        }
        let (y_lo, y_hi, rows) = self.band_rows(damage);

        let workers = self.workers_for_damage(damage);
        if workers < 2 || rows < 2 {
            return false;
        }
        let strip_rows = strip_rows_for(rows, workers);
        let width = self.width;
        #[cfg(feature = "truetype")]
        let text = &self.text;
        let slice = &mut self.pixels[y_lo * stride..y_hi * stride];

        // **A shared queue, not a fixed partition** -- M17/T17.5.
        //
        // T17.4 gave each thread one equal band, and the frame then ran
        // at the speed of the slowest *core*: 8 bands beat 32 on the
        // reference machine because past the performance cores the
        // extras landed somewhere slower and everyone waited at the
        // barrier. A cap of 8 fixed that for one CPU and would be the
        // wrong number on anybody else's.
        //
        // Over-decomposing and letting workers pull the next strip needs
        // to know nothing about the machine, and fixes the other
        // imbalance too: a strip containing a shadow costs far more than
        // an empty one, and a fixed partition cannot know which is which
        // either.
        //
        // Safe, with no `unsafe` and no dependency. `chunks_mut` proves
        // the strips disjoint; taking one out of the iterator *moves*
        // the `&mut`, so ownership transfers to the worker and nothing
        // aliases. A `Mutex` over the iterator is the whole protocol --
        // per-thread deques and a steal handshake are for tasks that
        // spawn subtasks, and these do not.
        let queue = std::sync::Mutex::new(slice.chunks_mut(strip_rows * stride).enumerate());

        std::thread::scope(|sc| {
            for _ in 0..workers {
                let queue = &queue;
                sc.spawn(move || {
                    #[cfg(feature = "raster-counters")]
                    IN_WORKER.with(|w| w.set(true));
                    loop {
                        // Held only long enough to take a strip, never while
                        // drawing one.
                        let next = queue.lock().map(|mut q| q.next()).unwrap_or(None);
                        let Some((k, chunk)) = next else { break };
                        let y0 = (y_lo + k * strip_rows) as i64;
                        let nrows = (chunk.len() / stride) as i64;
                        // **One scratch per worker, not per glass command.**
                        // A `Canvas`'s own buffers cannot be shared across
                        // threads, so a banded frame builds one set per band
                        // — bounded by the worker count, where it used to be
                        // three `Vec`s per panel per band (M26/T26.13).
                        let mut scratch = GlassScratch::default();
                        let mut paths = PathScratch::default();
                        let mut band = Surface {
                            px: chunk,
                            width,
                            y0,
                            rows: nrows,
                            glass: &mut scratch,
                            paths: &mut paths,
                            #[cfg(feature = "truetype")]
                            text,
                        };
                        band.render(
                            list,
                            damage,
                            #[cfg(feature = "truetype")]
                            font,
                            None,
                        );
                    }
                });
            }
        });
        true
    }

}

impl Renderer for Canvas {
    fn render(&mut self, list: &DisplayList, _w: u32, _h: u32) {
        self.render_impl(list, None);
    }
}

#[cfg(all(feature = "threads", feature = "raster-counters"))]
thread_local! {
    /// Set as a band worker's closure starts — M50/T50.4.
    static IN_WORKER: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}

/// Whether the calling thread is running a band worker's own code — for
/// `tests/alloc_gate_threads.rs`'s allocator, which must tell a worker's
/// allocations from the ones `std` makes starting the thread (its
/// stack-overflow handler's thread registry, a `BTreeMap` whose node
/// splits land on a spawned thread now and then). Test-only
/// (`raster-counters`).
#[cfg(all(feature = "threads", feature = "raster-counters"))]
pub fn in_band_worker() -> bool {
    IN_WORKER.try_with(|w| w.get()).unwrap_or(false)
}

/// Times each `simd` vector branch ran: `blit_0rgb`, `blit_run`,
/// `shadow_run` — M50/T50.5, R18-F10.
#[cfg(all(feature = "simd", feature = "raster-counters"))]
static VECTOR: [core::sync::atomic::AtomicU64; 3] = [const { core::sync::atomic::AtomicU64::new(0) }; 3];

/// How many times each `simd` vector branch has run in this process:
/// `[blit_0rgb, blit_run, shadow_run]` — M50/T50.5, R18-F10.
///
/// **A counter, not a constant**, because the vector code is chosen only
/// by `#[cfg]` and the scalar path produces the same pixels: compiling a
/// branch out (the review's I8, gating it on a target feature the default
/// target lacks) left every byte-identity test green. A `const` beside the
/// branch would have its own `cfg` and stay true; this increments inside
/// the branch, so a branch that is not compiled reads 0. Zeros without
/// `simd`. Test-only (`raster-counters`): the shipped `simd` build has no
/// counter in its hot loop.
#[cfg(feature = "raster-counters")]
pub fn vector_path_counts() -> [u64; 3] {
    #[cfg(feature = "simd")]
    {
        let r = core::sync::atomic::Ordering::Relaxed;
        [VECTOR[0].load(r), VECTOR[1].load(r), VECTOR[2].load(r)]
    }
    #[cfg(not(feature = "simd"))]
    {
        [0; 3]
    }
}

/// How many worker threads to run for `px` damaged pixels — T17.4.
///
/// **The threshold is the point, not the arithmetic.** A thread costs
/// 5–10 µs to wake, and a damaged tally frame rasterises in 12 µs, so
/// threading one is a straight loss. Only frames big enough to amortise
/// a wake-up are split; the rest take the single-band path having spent
/// one comparison deciding. 0.25 Mpx is about an eighth of a 1080p
/// surface and eight times the tally's whole damaged region.
///
/// Unlike T17.4's band count this is **not capped at 8**: with a work
/// queue a slow core simply takes fewer strips, so an extra worker
/// costs a spawn rather than stalling the frame at the barrier.
/// `LUMEN_WORKERS` overrides it.
///
/// **Both halves of this are leaving time on the table** — measured
/// 2026-09-09 against `vello_cpu`, `docs/results-vello.md` §3. Removing
/// the `px / MIN_PX` throttle is worth 2.4x at 720p and 1.8x at 1080p;
/// lowering the floor is worth 1.9x on the tally window. The heuristic
/// counts pixels because pixels are what a band splits, and cannot see
/// that a 720p frame with 400 cards has work for sixteen workers and is
/// being given three. Not changed, because the strip height was tuned
/// against a measured re-walk cost (`docs/results-threading.md`) and the
/// two interact.
#[cfg(feature = "threads")]
fn workers_for(px: usize) -> usize {
    const MIN_PX: usize = 250_000;
    if px < MIN_PX {
        return 1;
    }
    static CAP: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    let cap = *CAP.get_or_init(|| {
        std::env::var("LUMEN_WORKERS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|n| *n > 0)
            .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()))
    });
    cap.min(px / MIN_PX).max(1)
}

/// How tall each strip in the queue is — T17.5.
///
/// A product of two costs pulling opposite ways. **Finer strips balance
/// better**: the frame's tail is bounded by one strip rather than one
/// whole band. **Coarser strips cost less**: every strip re-walks the
/// command list, measured at 1.06x the single-strip cost at 8, 1.13x at
/// 32, 1.20x at 64 and 1.34x at 128 on a 1 633-command 4K scene
/// (`docs/results-threading.md`).
///
/// Four strips per worker bounds the imbalance to a quarter of one
/// worker's share while staying on the cheap part of that curve, and
/// never finer than 16 rows, below which the walk dominates whatever
/// the balance is worth. `LUMEN_STRIP_ROWS` overrides it.
#[cfg(feature = "threads")]
fn strip_rows_for(rows: usize, workers: usize) -> usize {
    static OVERRIDE: std::sync::OnceLock<Option<usize>> = std::sync::OnceLock::new();
    if let Some(n) = *OVERRIDE.get_or_init(|| {
        std::env::var("LUMEN_STRIP_ROWS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|n| *n > 0)
    }) {
        return n.clamp(1, rows);
    }
    rows.div_ceil(workers * 4).clamp(16, rows.max(1))
}

/// The light's elevation, as the z component of a unit direction.
///
/// A constant rather than a field: only the azimuth is authorable, which
/// is why animating `Material::light` rotates a highlight around a panel
/// and cannot sweep it across the rim (`docs/plan-liquid.md` §13).
const LIGHT_Z: f32 = 0.62;

/// …and the horizontal magnitude that goes with it.
const LIGHT_XY: f32 = 0.784_599_3;

/// Boost a colour's chroma about its luma — M22/T22.0.
///
/// **Luma, not luminance.** The renderer blurs in straight, gamma-encoded
/// sRGB and never linearises; Rec.709's coefficients are *linear-light*
/// and applied to gamma bytes they over-darken blue and over-brighten
/// green. 0.299/0.587/0.114 is the convention that matches the encoding
/// actually in the buffer.
///
/// **Clamps the scale, not the channels.** `lum + (c − lum)·s` leaves
/// gamut at both ends, and clipping each channel where it happens to
/// land rotates hue: (255, 120, 20) at 1.4 becomes (255, 108, 0) and the
/// red-to-green ratio moves. Reducing `s` until every channel fits
/// desaturates gracefully instead. The search runs only when a channel
/// would actually leave gamut, because unlike everything else in T22.0
/// this applies to the whole panel and not the rim band.
fn saturate(c: [f32; 3], s: f32) -> [f32; 3] {
    if s == 1.0 {
        return c;
    }
    let lum = 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
    let mut k = s;
    for v in c {
        let d = v - lum;
        if d > 0.0 {
            if lum + d * k > 255.0 {
                k = k.min((255.0 - lum) / d);
            }
        } else if d < 0.0 && lum + d * k < 0.0 {
            k = k.min(-lum / d);
        }
    }
    [lum + (c[0] - lum) * k, lum + (c[1] - lum) * k, lum + (c[2] - lum) * k]
}

/// `-1` or `+1`, and never `0` — a normal needs a direction even
/// exactly on the centre line.
#[inline]
fn sign_of(v: f32) -> f32 {
    if v < 0.0 {
        -1.0
    } else {
        1.0
    }
}

/// Whether a list contains a backdrop filter.
///
/// One place, shared by the splitter and by
/// [`Canvas::bands_for_list`] — a predicate a test asserts on must be
/// the predicate the renderer decides with, or the test pins nothing.
fn has_glass(list: &DisplayList) -> bool {
    list.cmds.iter().any(|c| matches!(c, lumen_static::Cmd::Glass { .. }))
}

/// SVG's box width for a Gaussian of this standard deviation.
///
/// <https://www.w3.org/TR/SVG11/filters.html#feGaussianBlurElement>:
/// `d = floor(s * 3 * sqrt(2*pi)/4 + 0.5)`.
fn box_width(sigma: f32) -> i64 {
    (sigma * 3.0 * (2.0 * core::f32::consts::PI).sqrt() / 4.0 + 0.5).floor() as i64
}

/// The standard deviation three box passes of width `d` actually
/// deliver — M19/T19.3.
///
/// A box of width `d` has variance `(d² − 1)/12`. Three of them add, and
/// the even case is the spec's: two boxes of `d` and one of `d + 1`.
fn box_sigma(d: i64) -> f32 {
    if d <= 1 {
        return 0.0;
    }
    let dd = d as f32;
    let var = if d % 2 == 1 {
        3.0 * (dd * dd - 1.0) / 12.0
    } else {
        (2.0 * (dd * dd - 1.0) + ((dd + 1.0) * (dd + 1.0) - 1.0)) / 12.0
    };
    var.sqrt()
}

/// The three `(left, right)` windows for a blur of width `d`.
///
/// **The even case is not three equal boxes**, and getting it wrong is
/// not a subtle quality difference: an even box has no centre pixel, so
/// three of them shift the image half a pixel per pass — 1.5 px in
/// total, a visible slide of the backdrop under the panel. The spec's
/// answer is two boxes of `d` centred on opposite pixel *boundaries*,
/// which is the asymmetric pair below, and one of `d + 1` centred on the
/// pixel. And the even case is the common one here: T19.3's `k` search
/// targets a per-pass sigma around 2, where `d` is 4.
pub fn box_windows(d: i64) -> [(usize, usize); 3] {
    let h = (d / 2) as usize;
    if d % 2 == 1 {
        [(h, h), (h, h), (h, h)]
    } else {
        [(h, h.saturating_sub(1)), (h.saturating_sub(1), h), (h, h)]
    }
}

/// Downsample factor, box width, and the sigma actually **delivered**.
///
/// Public so `benches --bin glass-cost` can print requested against delivered
/// without a second copy of the arithmetic. A results table computed
/// from a duplicate of the code it describes is a table that goes wrong
/// silently, which is what `AGENT.md`'s "numbers are stated once" rule
/// is about.
///
/// **Chosen by search, not by a formula** — M19/T19.3. The obvious rule,
/// `k = ceil(sigma/2)`, drops the per-pass sigma below the 2.0 the SVG
/// approximation is specified for, delivers a blur up to 24% away from
/// what was asked, and does it in plateaus: several requested sigmas
/// render identically and then one jumps 50%. Eight iterations of the
/// real cost function, once per command rather than per pixel, bound the
/// error at about 10% and remove the plateaus.
pub fn blur_plan(sigma: f32) -> (usize, i64, f32) {
    let mut best = (1usize, 1i64, f32::INFINITY);
    for k in 1..=8usize {
        let d = box_width(sigma / k as f32).max(1);
        let delivered = k as f32 * box_sigma(d);
        let cost = (delivered - sigma).abs();
        if cost < best.2 {
            best = (k, d, cost);
        }
    }
    (best.0, best.1, best.0 as f32 * box_sigma(best.1))
}

/// One box pass across rows, with a running sum — M19/T19.2.
///
/// Normalised by the number of *contributing* samples, so a box that
/// hangs off the edge does not darken the rim. Clamp-to-edge is the
/// other honest rule and is what a GPU sampler does; this is the same
/// answer without a branch per tap.
fn box_rows(src: &[u16], dst: &mut [u16], w: usize, h: usize, left: usize, right: usize) {
    for y in 0..h {
        let row = y * w * 3;
        for ch in 0..3 {
            let mut sum: u32 = 0;
            let mut n: u32 = 0;
            for x in 0..(right + 1).min(w) {
                sum += src[row + x * 3 + ch] as u32;
                n += 1;
            }
            for x in 0..w {
                dst[row + x * 3 + ch] = (sum / n.max(1)) as u16;
                if x >= left {
                    sum -= src[row + (x - left) * 3 + ch] as u32;
                    n -= 1;
                }
                let add = x + right + 1;
                if add < w {
                    sum += src[row + add * 3 + ch] as u32;
                    n += 1;
                }
            }
        }
    }
}

/// The same, down columns.
fn box_cols(src: &[u16], dst: &mut [u16], w: usize, h: usize, left: usize, right: usize) {
    for x in 0..w {
        for ch in 0..3 {
            let at = |y: usize| y * w * 3 + x * 3 + ch;
            let mut sum: u32 = 0;
            let mut n: u32 = 0;
            for y in 0..(right + 1).min(h) {
                sum += src[at(y)] as u32;
                n += 1;
            }
            for y in 0..h {
                dst[at(y)] = (sum / n.max(1)) as u16;
                if y >= left {
                    sum -= src[at(y - left)] as u32;
                    n -= 1;
                }
                let add = y + right + 1;
                if add < h {
                    sum += src[at(add)] as u32;
                    n += 1;
                }
            }
        }
    }
}

/// One row's run of RGBA -> `0RGB`, shared by both blit paths (T17.3).
///
/// `dst` is the destination row; `base` is that row's first pixel index
/// into `pixels`, and `x0..x1` are pixel columns.
#[inline]
fn blit_run(pixels: &[u8], dst: &mut [u32], base: usize, x0: usize, x1: usize) {
    #[cfg(feature = "simd")]
    {
        #[cfg(feature = "raster-counters")]
        VECTOR[1].fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        use wide::u32x8;
        let (m_r, m_g, m_b) = (
            u32x8::splat(0x0000_00FF),
            u32x8::splat(0x0000_FF00),
            u32x8::splat(0x00FF_0000),
        );
        let mut x = x0;
        while x + 8 <= x1 {
            let i = (base + x) * 4;
            let sq = &pixels[i..i + 32];
            let mut arr = [0u32; 8];
            for (k, a) in arr.iter_mut().enumerate() {
                *a = u32::from_le_bytes([
                    sq[k * 4],
                    sq[k * 4 + 1],
                    sq[k * 4 + 2],
                    sq[k * 4 + 3],
                ]);
            }
            let v = u32x8::from(arr);
            let r: u32x8 = ((v & m_r) << 16) | (v & m_g) | ((v & m_b) >> 16);
            dst[x..x + 8].copy_from_slice(&r.to_array());
            x += 8;
        }
        blit_run_scalar(pixels, dst, base, x, x1);
        }
    #[cfg(not(feature = "simd"))]
    blit_run_scalar(pixels, dst, base, x0, x1);
}

/// The scalar form, and the tail of the vector one.
#[inline]
fn blit_run_scalar(pixels: &[u8], dst: &mut [u32], base: usize, x0: usize, x1: usize) {
    if x0 >= x1 {
        return;
    }
    let src = &pixels[(base + x0) * 4..(base + x1) * 4];
    for (d, px) in dst[x0..x1].iter_mut().zip(src.chunks_exact(4)) {
        *d = u32::from_le_bytes([px[2], px[1], px[0], 0]);
    }
}

/// One row's run of a shadow's signed-distance field — T17.1.
///
/// `row` is the whole row; `xa..xb` are pixel indices into it, already
/// clamped to the surface by the caller, which is what lets the blend
/// index without a per-pixel bounds check.
///
/// Byte-identical between the scalar and SIMD forms **by
/// construction**: the vector form computes the same `f32` alpha in
/// eight lanes and hands it back as `f32`, and the `as u8` cast and the
/// blend stay scalar. So the only arithmetic that differs is the order
/// the same operations happen in, and `sqrt`, `max`, `min` and the
/// four arithmetic ops are all exactly specified for `f32`.
/// `tests/simd_shadow.rs` asserts it anyway, because "by construction"
/// is an argument and not a measurement.
#[allow(clippy::too_many_arguments)]
fn shadow_run(
    row: &mut [u8],
    xa: i64,
    xb: i64,
    qy: f32,
    cx: f32,
    hx: f32,
    rad: f32,
    spread: f32,
    c: Color,
) {
    if xa >= xb {
        return;
    }

    #[cfg(feature = "simd")]
    {
        #[cfg(feature = "raster-counters")]
        VECTOR[2].fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        use wide::{f32x8, CmpGe};
        let base = c.3 as f32;
        let (vhx, vrad, vsp) = (f32x8::splat(hx), f32x8::splat(rad), f32x8::splat(spread));
        let (vcx, vqy, vbase) = (f32x8::splat(cx), f32x8::splat(qy), f32x8::splat(base));
        let (zero, one, neg1) = (f32x8::splat(0.0), f32x8::splat(1.0), f32x8::splat(-1.0));
        let ay = vqy.max(zero);
        let mut x = xa;
        while x + 8 <= xb {
            let mut xs = [0.0f32; 8];
            for (k, v) in xs.iter_mut().enumerate() {
                *v = (x + k as i64) as f32 + 0.5;
            }
            let qx = (f32x8::from(xs) - vcx).abs() - vhx;
            let ax = qx.max(zero);
            let d = (ax * ax + ay * ay).sqrt() + qx.max(vqy).min(zero) - vrad;
            let t = one - (d / vsp).max(zero).min(one);
            // Pixels deeper than one unit inside the shape contribute
            // nothing; masking them to zero alpha is what the scalar
            // form's `if d >= -1.0` does.
            let a = (vbase * t * t) & d.cmp_ge(neg1);
            for (k, av) in a.to_array().iter().enumerate() {
                let a8 = *av as u8;
                if a8 > 0 {
                    blend_px(row, (x + k as i64) as usize, Color(c.0, c.1, c.2, a8));
                }
            }
            x += 8;
        }
        // The tail: fewer than eight pixels left over.
        shadow_run_scalar(row, x, xb, qy, cx, hx, rad, spread, c);
    }
    #[cfg(not(feature = "simd"))]
    shadow_run_scalar(row, xa, xb, qy, cx, hx, rad, spread, c);
}

/// The scalar form, and the tail of the vector one.
#[allow(clippy::too_many_arguments)]
fn shadow_run_scalar(
    row: &mut [u8],
    xa: i64,
    xb: i64,
    qy: f32,
    cx: f32,
    hx: f32,
    rad: f32,
    spread: f32,
    c: Color,
) {
    let base = c.3 as f32;
    let ay = qy.max(0.0);
    for x in xa..xb {
        let qx = (x as f32 + 0.5 - cx).abs() - hx;
        // Signed distance to a rounded rect (Quilez).
        let ax = qx.max(0.0);
        let d = (ax * ax + ay * ay).sqrt() + qx.max(qy).min(0.0) - rad;
        if d >= -1.0 {
            let t = 1.0 - (d / spread).clamp(0.0, 1.0);
            let a = (base * t * t) as u8;
            if a > 0 {
                blend_px(row, x as usize, Color(c.0, c.1, c.2, a));
            }
        }
    }
}

/// Source-over one pixel into a row, with no coordinate check.
///
/// The caller has already clamped the run to the surface, so the only
/// bound left is the slice's own. `Canvas::blend` re-checks four
/// comparisons per pixel, which is what a row-oriented loop exists to
/// stop paying.
#[inline]
/// Coverage of a rounded rect at a pixel centre, 0..=1.
///
/// One function for both the outer and the inner curve of a border, which
/// is what lets the ring be a subtraction rather than a second pass.
///
/// Two regimes, and the split matters: inside a corner *square* the
/// distance is to the corner circle, and everywhere else it is to the
/// nearest straight edge. Using the corner form everywhere is what
/// `fill_rounded_within` avoids by early-returning for `radius <= 0.5` —
/// with `rad` at 0 it reports half coverage for every pixel inside a
/// square box, because the circle it measures against has no radius.
fn rr_cov(px: f32, py: f32, r: [f32; 4], rad: f32) -> f32 {
    (0.5 - sd_round_box(px, py, r, rad)).clamp(0.0, 1.0)
}

/// Signed distance to a rounded box (Quilez): negative inside.
///
/// **Transcribed from `shader.wgsl`'s `sd_round_box`, deliberately, and
/// not re-derived.** `just gpu-parity` compares the two backends pixel
/// for pixel, and a border whose curve came from a different expression
/// on each side would differ at exactly the antialiased corner the whole
/// command exists to get right. The shadow path already shares this
/// function; the border now does too.
///
/// It is also correct at `rad == 0`, which the obvious
/// distance-past-the-corner form is not: with no radius that form reports
/// half coverage for every pixel inside a square box, which is why
/// `fill_rounded_within` early-returns instead of using it.
fn sd_round_box(px: f32, py: f32, r: [f32; 4], rad: f32) -> f32 {
    let (cx, cy) = ((r[0] + r[2]) * 0.5, (r[1] + r[3]) * 0.5);
    let (hx, hy) = ((r[2] - r[0]) * 0.5 - rad, (r[3] - r[1]) * 0.5 - rad);
    let qx = (px - cx).abs() - hx;
    let qy = (py - cy).abs() - hy;
    let out = (qx.max(0.0) * qx.max(0.0) + qy.max(0.0) * qy.max(0.0)).sqrt();
    out + qx.max(qy).min(0.0) - rad
}

fn blend_px(row: &mut [u8], x: usize, c: Color) {
    let i = x * 4;
    let px = &mut row[i..i + 4];
    let a = c.3 as u32;
    let inv = 255 - a;
    px[0] = ((c.0 as u32 * a + px[0] as u32 * inv) / 255) as u8;
    px[1] = ((c.1 as u32 * a + px[1] as u32 * inv) / 255) as u8;
    px[2] = ((c.2 as u32 * a + px[2] as u32 * inv) / 255) as u8;
    px[3] = 255;
}

/// Write one RGBA pixel value across `dst`, wide.
///
/// **The naive spelling is 5.6x slower and this is the whole of
/// M17/T17.0.** `for px in dst.chunks_exact_mut(4) { px.copy_from_slice(&p) }`
/// is what `clear` and `span` used, and compiled into the library it
/// stores four bytes at a time -- 0.896 ms to clear 1080p against
/// 0.159 for the same work done sixteen bytes at a time. The identical
/// loop written *inline in a test crate* vectorises fine, so this is
/// not about the loop's shape; it is about what survives being
/// compiled behind a crate boundary with no LTO.
///
/// Splatting the four bytes into a 16-byte pattern first gives the
/// optimiser something it will widen either way, which is why the fix
/// is a pattern rather than an `#[inline]`. `docs/results-wide-stores.md`.
///
/// Byte-identical to the loop it replaces: the remainder is written
/// four bytes at a time, and a canvas is always a whole number of
/// pixels, so the remainder is at most three pixels.
#[inline]
fn fill_px(dst: &mut [u8], p: [u8; 4]) {
    let mut pat = [0u8; 16];
    for k in 0..4 {
        pat[k * 4..k * 4 + 4].copy_from_slice(&p);
    }
    let mut it = dst.chunks_exact_mut(16);
    for ch in &mut it {
        ch.copy_from_slice(&pat);
    }
    for px in it.into_remainder().chunks_exact_mut(4) {
        px.copy_from_slice(&p);
    }
}

/// Slots in the raster clip stack: the damage band, plus eight scopes
/// from the list.
///
/// **Eight list scopes, and what happens at a ninth is stated rather
/// than undefined.** Slot 0 is the band and is never one of the
/// eight — an off-by-one the first version of this had, and the test
/// below found by reporting a 48×48 fill where a 20×20 scope was
/// supposed to be in force.
/// A `Repeat` inside a `Popup` inside a `Card` is three; the deepest
/// tree in this repository reaches four. A ninth `PushClip` is
/// *ignored* — the scope it would have opened is not entered, so the
/// content it would have bounded is clipped by the eight above it
/// instead. That is the conservative direction: too much clipping
/// draws less, never more, and drawing outside a clip is the bug a
/// renderer must not have (`tests/paint_containment.rs`).
const MAX_CLIPS: usize = 9;

/// The raster path's clip stack.
///
/// A fixed array, because this is per-frame and the frame path
/// allocates nothing — the same shape `InputQueue` and `Overlays` use,
/// and for the same reason (M26/T26.13).
struct ClipStack {
    stack: [[f32; 4]; MAX_CLIPS],
    /// Scopes actually entered. Never below 1: slot 0 is the damage
    /// band, which is not part of the list's own balance.
    n: usize,
    /// Pushes refused for depth, so `pop` can decline the matching
    /// number and keep the stack balanced. Without this a ninth push
    /// followed by its pop would pop a scope it never opened.
    refused: usize,
}

impl ClipStack {
    fn new(outer: [f32; 4]) -> ClipStack {
        let mut stack = [[0.0; 4]; MAX_CLIPS];
        stack[0] = outer;
        ClipStack { stack, n: 1, refused: 0 }
    }

    fn push(&mut self, rect: [f32; 4]) {
        if self.n < MAX_CLIPS {
            self.stack[self.n] = rect;
            self.n += 1;
        } else {
            self.refused += 1;
        }
    }

    /// Answers whether a scope was actually left, so the caller's floor
    /// check reads the same as it did with a `Vec`.
    fn pop(&mut self) {
        if self.refused > 0 {
            self.refused -= 1;
        } else if self.n > 1 {
            self.n -= 1;
        }
    }

    fn as_slice(&self) -> &[[f32; 4]] {
        &self.stack[..self.n]
    }
}

/// Intersect a rect with every clip on the stack; `None` if nothing
/// survives.
///
/// Honouring the clip stack is not cosmetic. A `Repeat` places rows
/// outside its viewport by construction and `hit` rejects points there,
/// so a renderer that ignores clips draws content no click can reach —
/// the probe-vs-click class of bug, one layer down.
///
/// `tests/clip_test.rs` pins it at the pixel level **for every drawing
/// command** since M50/T50.8: `Rect`, `RoundedRect`, `StrokedRect`,
/// `Shadow`, `Gradient`, `Line`, `Text` (block glyphs; the truetype
/// blit is not in that binary), `Image` and `Glass` (frosted and
/// liquid), each inside a `PushClip`, plus a shadow inside a
/// `ScrollView`'s viewport on a full redraw — the case `tests/damage_oracle.rs`
/// cannot see, because a full and a partial render escape identically.
/// Until T50.8 it covered the first three only. (This comment also sat
/// on the text-run struct M50/T50.3 removed, detached from the function
/// it describes.)
fn clipped(rect: [f32; 4], stack: &[[f32; 4]]) -> Option<[f32; 4]> {
    // Reject non-finite and empty input on entry. `f32::max`/`min` return
    // the non-NaN operand, so without this a NaN coordinate would be
    // silently *replaced by the clip bound* — a NaN rect inside a clip
    // scope filled the entire scope solid, while the same rect outside
    // one drew nothing.
    if !rect.iter().all(|v| v.is_finite()) || rect[0] >= rect[2] || rect[1] >= rect[3] {
        return None;
    }
    let mut r = rect;
    for c in stack {
        r = [r[0].max(c[0]), r[1].max(c[1]), r[2].min(c[2]), r[3].min(c[3])];
        if r[0] >= r[2] || r[1] >= r[3] {
            return None;
        }
    }
    Some(r)
}

/// `x` limited to 0..=1 — `lumen_static::num::clamp` without repeating
/// the bounds at every call, and total where `f32::clamp` panics.
fn clamp01(x: f32) -> f32 {
    lumen_static::num::clamp(x, 0.0, 1.0)
}

/// What [`rasterize_path`] covers: the path's non-zero fill, or its
/// stroke at a width in device px — M82/T82.5.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathCoverage {
    Fill,
    Stroke(f32),
}

/// The flattening tolerance, in device px: no flattened segment strays
/// further than this from its curve (`docs/plan-paint-vocabulary.md`,
/// Decisions). The elements reach [`rasterize_path`] after
/// `lumen_static::Scaled` and the path's own transform, so this is
/// physical whatever the scale.
pub const FLATTEN_TOLERANCE: f32 = 0.1;

/// The most segments one curve flattens to. Wang's bound asks for about
/// 12 per quarter of a 40 px circle; this is for a curve whose control
/// points are thousands of px apart, which should not cost unbounded
/// work.
const MAX_CURVE_SEGMENTS: usize = 256;

/// A round join or cap is a polygon of this many sides, inscribed in the
/// stroke's half-width.
const JOIN_SIDES: usize = 16;

/// [`rasterize_path`]'s buffers, kept between calls so a steady frame
/// allocates nothing (the `alloc_gate.rs` rule): the accumulation rows,
/// the flattened points, the subpaths, and the coverage it answers.
#[derive(Default)]
pub struct PathScratch {
    acc: Vec<f32>,
    pts: Vec<Point>,
    /// `(start, end, closed)` ranges into `pts`, one per subpath.
    subs: Vec<(u32, u32, bool)>,
    mask: Vec<u8>,
}

/// Rasterise a path's coverage into a `w × h` mask whose pixel `(i, j)`
/// is the device pixel `(origin[0] + i, origin[1] + j)` — M82/T82.5.
///
/// **Signed-area accumulation**: every edge adds the area it sweeps to
/// the cells it crosses, a running sum along each row turns that into
/// winding, and the coverage is `min(|sum|, 1)` — non-zero winding,
/// analytic rather than sampled, and no `unsafe`. The software canvas
/// draws [`lumen_static::Cmd::Path`] through it, and since M82/T82.6 the
/// GPU backend rasterises its path masks through the same function, at
/// the same origin, into its glyph atlas (`lumen_static_wgpu::MaskKey`),
/// so the two can only differ in how the mask is blended.
///
/// Curves are flattened at [`FLATTEN_TOLERANCE`] by Wang's bound. A
/// stroke is the non-zero union of one quad per flattened segment and a
/// `JOIN_SIDES`-gon (16) of the half-width at every vertex — round joins
/// and caps, drawn by the same accumulation; every polygon is emitted
/// with the same orientation so an overlap clamps instead of cancelling.
/// Miter joins, dashes and even-odd are not built.
///
/// **A row's coverage does not depend on which rows the mask holds**:
/// each row's crossing is computed from its edge's endpoints, never
/// stepped from the previous row, so a band of a frame rasterises
/// exactly the bytes the whole frame would (`tests/banded.rs`). An edge
/// left of the mask counts as a vertical edge at its left column, and
/// one right of it is dropped, so columns are exact too — but the canvas
/// still takes its x origin from the surface, not the damage, so a
/// partial redraw sums in the same order as a full one.
pub fn rasterize_path<'s>(
    els: &[lumen_static::display::PathEl],
    cover: PathCoverage,
    origin: [i32; 2],
    w: usize,
    h: usize,
    scratch: &'s mut PathScratch,
) -> &'s [u8] {
    let PathScratch { acc, pts, subs, mask } = scratch;
    let stride = w + 2;
    acc.clear();
    acc.resize(stride * h, 0.0);
    mask.clear();
    mask.resize(w * h, 0);
    if w == 0 || h == 0 {
        return mask;
    }
    flatten(els, pts, subs);
    let o = Point { x: origin[0] as f32, y: 0.0 };
    let mut acc = Acc { a: acc, stride, w: w as f32, y0: origin[1] as f32, h };
    match cover {
        PathCoverage::Fill => {
            for &(s, e, _) in subs.iter() {
                let p = &pts[s as usize..e as usize];
                for k in 0..p.len() {
                    // Every subpath closes for a fill.
                    let (a, b) = (p[k], p[(k + 1) % p.len()]);
                    acc.line(sub(a, o), sub(b, o));
                }
            }
        }
        PathCoverage::Stroke(width) => {
            let hw = width / 2.0;
            if !hw.is_finite() || hw <= 0.0 {
                return mask;
            }
            for &(s, e, closed) in subs.iter() {
                let p = &pts[s as usize..e as usize];
                let n = p.len();
                let segs = if closed && n > 2 { n } else { n.saturating_sub(1) };
                for k in 0..segs {
                    let (a, b) = (sub(p[k], o), sub(p[(k + 1) % n], o));
                    let (dx, dy) = (b.x - a.x, b.y - a.y);
                    let len = (dx * dx + dy * dy).sqrt();
                    if len > 0.0 {
                        let (nx, ny) = (-dy / len * hw, dx / len * hw);
                        acc.polygon(&[
                            Point { x: a.x + nx, y: a.y + ny },
                            Point { x: b.x + nx, y: b.y + ny },
                            Point { x: b.x - nx, y: b.y - ny },
                            Point { x: a.x - nx, y: a.y - ny },
                        ]);
                    }
                }
                // A join at every vertex and a cap at each open end: the
                // same disc. A closed path's last point is its first again
                // only when it said so with a `Line`, which costs one more
                // disc and changes no pixel.
                for q in p {
                    let c = sub(*q, o);
                    let mut ring = [Point::default(); JOIN_SIDES];
                    for (k, v) in ring.iter_mut().enumerate() {
                        let t = k as f32 * core::f32::consts::TAU / JOIN_SIDES as f32;
                        *v = Point { x: c.x + hw * t.cos(), y: c.y + hw * t.sin() };
                    }
                    acc.polygon(&ring);
                }
            }
        }
    }
    let a = acc.a;
    for row in 0..h {
        let mut s = 0.0f32;
        for x in 0..w {
            s += a[row * stride + x];
            mask[row * w + x] = (s.abs().min(1.0) * 255.0 + 0.5) as u8;
        }
    }
    mask
}

/// `p` in the mask's columns: x moved by the origin, **y kept absolute**.
/// A row's crossing is then computed from the same floats whichever row
/// the mask starts at — `y - y0` would round differently for each band's
/// `y0`, and a banded frame would differ from a whole one in the last bit
/// of an edge pixel (`tests/banded.rs`, M82/T82.5's first run).
#[inline]
fn sub(p: Point, o: Point) -> Point {
    Point { x: p.x - o.x, y: p.y }
}

/// Flatten `els` into polylines: `pts` holds every subpath's points back
/// to back, `subs` each one's range and whether it said `Close`. A
/// drawing element with no current point starts a subpath at its end
/// point, and a non-finite point ends the path there.
fn flatten(els: &[lumen_static::display::PathEl], pts: &mut Vec<Point>, subs: &mut Vec<(u32, u32, bool)>) {
    use lumen_static::display::PathEl;
    pts.clear();
    subs.clear();
    let mut start: Option<usize> = None;
    let finish = |pts: &mut Vec<Point>, subs: &mut Vec<(u32, u32, bool)>, start: &mut Option<usize>, closed: bool| {
        if let Some(s) = start.take() {
            if pts.len() - s >= 2 {
                subs.push((s as u32, pts.len() as u32, closed));
            } else {
                pts.truncate(s);
            }
        }
    };
    for e in els {
        let (n, ps) = e.points();
        if ps[..n].iter().any(|p| !(p.x.is_finite() && p.y.is_finite())) {
            break;
        }
        match *e {
            PathEl::Move(p) => {
                finish(pts, subs, &mut start, false);
                start = Some(pts.len());
                pts.push(p);
            }
            PathEl::Close => {
                // The current point returns to the subpath's start, where
                // a following element without a `Move` continues.
                let back = start.map(|s| pts[s]);
                finish(pts, subs, &mut start, true);
                if let Some(p) = back {
                    start = Some(pts.len());
                    pts.push(p);
                }
            }
            _ => {
                if start.is_none() {
                    start = Some(pts.len());
                    pts.push(ps[n - 1]);
                    continue;
                }
                let p0 = *pts.last().expect("a started subpath has a point");
                match *e {
                    PathEl::Line(p) => pts.push(p),
                    PathEl::Quad(c, p) => {
                        let m = len(p0.x - 2.0 * c.x + p.x, p0.y - 2.0 * c.y + p.y);
                        let k = segments(0.25 * m);
                        for i in 1..=k {
                            let t = i as f32 / k as f32;
                            let u = 1.0 - t;
                            pts.push(Point {
                                x: u * u * p0.x + 2.0 * u * t * c.x + t * t * p.x,
                                y: u * u * p0.y + 2.0 * u * t * c.y + t * t * p.y,
                            });
                        }
                    }
                    PathEl::Cubic(c1, c2, p) => {
                        let m = len(p0.x - 2.0 * c1.x + c2.x, p0.y - 2.0 * c1.y + c2.y)
                            .max(len(c1.x - 2.0 * c2.x + p.x, c1.y - 2.0 * c2.y + p.y));
                        let k = segments(0.75 * m);
                        for i in 1..=k {
                            let t = i as f32 / k as f32;
                            let u = 1.0 - t;
                            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                            pts.push(Point {
                                x: a * p0.x + b * c1.x + c * c2.x + d * p.x,
                                y: a * p0.y + b * c1.y + c * c2.y + d * p.y,
                            });
                        }
                    }
                    _ => unreachable!("Move and Close are handled above"),
                }
            }
        }
    }
    finish(pts, subs, &mut start, false);
}

#[inline]
fn len(x: f32, y: f32) -> f32 {
    (x * x + y * y).sqrt()
}

/// Wang's bound: a degree-`n` Bézier whose largest second difference is
/// `m` stays within the tolerance of its `k`-segment chord when
/// `k ≥ sqrt(n(n−1)/8 · m / tol)`; the caller passes `n(n−1)/8 · m`.
fn segments(scaled_m: f32) -> usize {
    let k = (scaled_m / FLATTEN_TOLERANCE).sqrt().ceil();
    if k.is_finite() {
        (k as usize).clamp(1, MAX_CURVE_SEGMENTS)
    } else {
        MAX_CURVE_SEGMENTS
    }
}

/// The accumulation rows of one [`rasterize_path`] call: x in mask
/// columns, y absolute, rows `y0..y0 + h`; `stride` is `w + 2` so a
/// crossing at the right edge has two cells to land in.
struct Acc<'a> {
    a: &'a mut [f32],
    stride: usize,
    w: f32,
    /// The first row's absolute y, a whole number.
    y0: f32,
    h: usize,
}

impl Acc<'_> {
    /// A closed polygon, emitted with positive orientation whatever order
    /// its points came in, so the stroke's pieces add instead of cancel.
    fn polygon(&mut self, p: &[Point]) {
        let n = p.len();
        let mut area = 0.0;
        for k in 0..n {
            let (a, b) = (p[k], p[(k + 1) % n]);
            area += a.x * b.y - b.x * a.y;
        }
        for k in 0..n {
            let (a, b) = if area >= 0.0 { (p[k], p[(k + 1) % n]) } else { (p[(k + 1) % n], p[k]) };
            self.line(a, b);
        }
    }

    /// One edge: split where it leaves `0..=w` in x, the part on the left
    /// moved onto column 0 (it still winds every pixel to its right) and
    /// the part on the right dropped (it winds nothing in the mask).
    fn line(&mut self, a: Point, b: Point) {
        if a.y == b.y {
            return;
        }
        let w = self.w;
        let at = |x: f32| {
            let t = (x - a.x) / (b.x - a.x);
            Point { x, y: a.y + (b.y - a.y) * t }
        };
        // Up to three pieces, in order along the edge.
        let mut cuts = [a, a, a, b];
        let mut n = 1;
        let (lo, hi) = if a.x <= b.x { (0.0, w) } else { (w, 0.0) };
        for x in [lo, hi] {
            if (a.x < x) != (b.x < x) && a.x != x && b.x != x {
                cuts[n] = at(x);
                n += 1;
            }
        }
        cuts[n] = b;
        for k in 0..n {
            let (p, q) = (cuts[k], cuts[k + 1]);
            let mid = (p.x + q.x) / 2.0;
            if mid > w {
                continue;
            }
            let cx = |v: Point| Point { x: v.x.clamp(0.0, w), y: v.y };
            self.cells(cx(p), cx(q));
        }
    }

    /// The cells one in-range edge crosses, row by row (font-rs's
    /// accumulation step, with each row's crossing taken from the edge's
    /// endpoints rather than stepped from the row above).
    fn cells(&mut self, p0: Point, p1: Point) {
        if p0.y == p1.y {
            return;
        }
        let (dir, a, b) = if p0.y < p1.y { (1.0, p0, p1) } else { (-1.0, p1, p0) };
        let dxdy = (b.x - a.x) / (b.y - a.y);
        let y_lo = a.y.max(self.y0);
        let y_hi = b.y.min(self.y0 + self.h as f32);
        if y_lo >= y_hi {
            return;
        }
        let first = (y_lo - self.y0).floor() as usize;
        let end = ((y_hi - self.y0).ceil() as usize).min(self.h);
        for row in first..end {
            // The row's edges in absolute y: whole numbers, exact in f32.
            let top = self.y0 + row as f32;
            let ya = top.max(a.y);
            let yb = (top + 1.0).min(b.y);
            let dy = yb - ya;
            if dy <= 0.0 {
                continue;
            }
            let xa = a.x + (ya - a.y) * dxdy;
            let xb = a.x + (yb - a.y) * dxdy;
            let d = dy * dir;
            let line = &mut self.a[row * self.stride..(row + 1) * self.stride];
            let (x0, x1) = if xa < xb { (xa, xb) } else { (xb, xa) };
            let x0f = x0.floor();
            let x0i = x0f as usize;
            let x1c = x1.ceil();
            let x1i = x1c as usize;
            if x1i <= x0i + 1 {
                // Inside one cell: its share left of the crossing's middle.
                let xm = 0.5 * (xa + xb) - x0f;
                line[x0i] += d - d * xm;
                line[x0i + 1] += d * xm;
            } else {
                let s = 1.0 / (x1 - x0);
                let x0r = x0 - x0f;
                let a0 = 0.5 * s * (1.0 - x0r) * (1.0 - x0r);
                let x1r = x1 - x1c + 1.0;
                let am = 0.5 * s * x1r * x1r;
                line[x0i] += d * a0;
                if x1i == x0i + 2 {
                    line[x0i + 1] += d * (1.0 - a0 - am);
                } else {
                    let a1 = s * (1.5 - x0r);
                    line[x0i + 1] += d * (a1 - a0);
                    for c in &mut line[x0i + 2..x1i - 1] {
                        *c += d * s;
                    }
                    let a2 = a1 + (x1i - x0i - 3) as f32 * s;
                    line[x1i - 1] += d * (1.0 - a2 - am);
                }
                line[x1i] += d * am;
            }
        }
    }
}
