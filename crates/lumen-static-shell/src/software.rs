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

use lumen_static::desc::Point;
use lumen_static::display::{Cmd, Color, DisplayList, Renderer, Stops};

/// An RGBA8 pixel buffer.
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    /// A real face, when one is available. Without it, text falls back
    /// to one filled block per glyph — enough to prove the pipeline,
    /// which is what the headless oracle needs, and not enough to read.
    #[cfg(feature = "truetype")]
    pub font: Option<crate::text::FontStack>,
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
/// they do not overlap. (The threading itself is not built yet; this is
/// the shape that makes it possible.)
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
struct Surface<'a> {
    px: &'a mut [u8],
    width: u32,
    /// Absolute y of `px`'s first row.
    y0: i64,
    /// Rows in `px`.
    rows: i64,
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
    #[cfg(feature = "truetype")]
    fn draw_run(&mut self, font: &crate::text::FontStack, run: TextRun<'_>, clip: &[[f32; 4]]) {
        let TextRun { rect, text, size, weight, color } = run;
        let (glyphs, _) = font.glyphs(text, size, weight, rect[2] - rect[0]);
        for g in glyphs {
            let gx = (rect[0] + g.x).round();
            let gy = (rect[1] + g.y).round();
            let Some(r) = clipped([gx, gy, gx + g.w as f32, gy + g.h as f32], clip) else {
                continue;
            };
            let (w, h) = (self.width as i64, self.y_hi());
            let (x0, y0) = ((r[0] as i64).max(0), (r[1] as i64).max(self.y0));
            let (x1, y1) = ((r[2] as i64).min(w), (r[3] as i64).min(h));
            for y in y0..y1 {
                let row = (y - gy as i64) as usize;
                for x in x0..x1 {
                    let col = (x - gx as i64) as usize;
                    let cov = g.cov[row * g.w + col];
                    if cov != 0 {
                        self.blend_cov(x, y, color, cov);
                    }
                }
            }
        }
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

    /// Fill `dst` with a linear gradient at `angle`, limited to `area`.
    fn fill_gradient(&mut self, dst: [f32; 4], area: [f32; 4], stops: Stops, angle: f32) {
        let (cx, cy) = ((dst[0] + dst[2]) / 2.0, (dst[1] + dst[3]) / 2.0);
        let (ux, uy) = (angle.cos(), angle.sin());
        // Half-extent of the box along the gradient's axis, so t spans
        // 0..1 across the whole rect whatever the angle.
        let half = ((dst[2] - dst[0]) * ux).abs() / 2.0 + ((dst[3] - dst[1]) * uy).abs() / 2.0;
        let span = if half > 0.0 { half * 2.0 } else { 1.0 };
        for y in (area[1].floor() as i64).max(self.y0)..(area[3].ceil() as i64).min(self.y_hi()) {
            for x in area[0].floor().max(0.0) as i64..(area[2].ceil() as i64).min(self.width as i64)
            {
                let (px, py) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let t = (px * ux + py * uy) / span + 0.5;
                self.blend(x, y, stops.sample(t));
            }
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
    fn render(
        &mut self,
        list: &DisplayList,
        damage: Option<[f32; 4]>,
        #[cfg(feature = "truetype")] font: Option<&crate::text::FontStack>,
        images: Option<&dyn lumen_static::ImageSource>,
    ) {
        let mut clip: Vec<[f32; 4]> = Vec::new();
        if let Some(d) = damage {
            clip.push(d);
        }
        for c in &list.cmds {
            match c {
                Cmd::Rect { rect, color } => {
                    if let Some(r) = clipped(*rect, &clip) {
                        self.fill(r, *color);
                    }
                }
                Cmd::RoundedRect { rect, color, radius } => {
                    if let Some(r) = clipped(*rect, &clip) {
                        // Corners are rounded against the *original*
                        // rect and the fill is limited to the clipped
                        // one, so a clipped card does not square off.
                        self.fill_rounded_within(*rect, r, *color, *radius);
                    }
                }
                Cmd::Shadow { rect, color, radius, spread } => {
                    let reach =
                        [rect[0] - spread, rect[1] - spread, rect[2] + spread, rect[3] + spread];
                    if let Some(r) = clipped(reach, &clip) {
                        self.shadow_within(*rect, r, *color, *radius, *spread);
                    }
                }
                Cmd::Text { rect, text, size, weight, color } => {
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
                    let pad = *size;
                    let reach = [rect[0] - pad, rect[1] - pad, rect[2] + pad, rect[3] + pad];
                    if clipped(reach, &clip).is_none() {
                        continue;
                    }
                    let s = list.str_of(*text);
                    #[cfg(feature = "truetype")]
                    if let Some(font) = font {
                        let run = TextRun { rect, text: s, size: *size, weight: *weight, color: *color };
                        self.draw_run(font, run, &clip);
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
                        if let Some(r) = clipped(g, &clip) {
                            self.fill(r, *color);
                        }
                    }
                }
                Cmd::Line { a, b, width, color } => {
                    let w = width / 2.0;
                    let bounds =
                        [a.x.min(b.x) - w, a.y.min(b.y) - w, a.x.max(b.x) + w, a.y.max(b.y) + w];
                    if clipped(bounds, &clip).is_some() {
                        self.stroke_line(*a, *b, *width, *color, &clip);
                    }
                }
                Cmd::Gradient { rect, stops, angle } => {
                    if let Some(r) = clipped(*rect, &clip) {
                        self.fill_gradient(*rect, r, *stops, *angle);
                    }
                }
                Cmd::Image { rect, id, tint } => {
                    // Culled before anything is fetched, like every
                    // other command and like text since the 46x fix
                    // (`docs/results-competitive.md` §6.1).
                    let Some(r) = clipped(*rect, &clip) else { continue };
                    let Some(src) = images else { continue };
                    // A missing asset draws nothing rather than taking
                    // the frame down: `ImageSource` returns `Option` for
                    // exactly that.
                    let Some((iw, ih, px)) = src.image(*id) else { continue };
                    if iw == 0 || ih == 0 || px.len() < (iw as usize * ih as usize * 4) {
                        continue;
                    }
                    self.blit_image(*rect, r, iw, ih, px, *tint);
                }
                Cmd::PushClip { rect } => clip.push(*rect),
                Cmd::PopClip => {
                    // Never pop the damage scope: it is not part of the
                    // list's own balance, and an unbalanced list (which
                    // `tests/paint_containment.rs` forbids, but a
                    // renderer must not trust) would otherwise let a
                    // later command paint outside the region.
                    if clip.len() > damage.is_some() as usize {
                        clip.pop();
                    }
                }
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
            #[cfg(feature = "truetype")]
            font: None,
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
        }
    }

    fn render_impl(&mut self, list: &DisplayList, damage: Option<[f32; 4]>) {
        #[cfg(feature = "truetype")]
        let font = self.font.clone();
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
    /// disjoint rows, and each one draws *every* command whose ink
    /// reaches those rows, in list order — the same rule that makes a
    /// partial redraw equal a full one. Nothing is skipped and nothing
    /// is reordered, so there is no interleaving for two threads to get
    /// wrong.
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
        // core trait for a case nothing in this workspace exercises —
        // `Canvas::images` has no setter outside tests — and images are
        // not what a large repaint is made of.
        if images.is_some() {
            return false;
        }
        let stride = (self.width * 4) as usize;
        if stride == 0 {
            return false;
        }
        let h = self.height as i64;
        let area = damage.unwrap_or([0.0, 0.0, self.width as f32, self.height as f32]);
        let y_lo = (area[1].floor() as i64).clamp(0, h) as usize;
        let y_hi = (area[3].ceil() as i64).clamp(0, h) as usize;
        let rows = y_hi.saturating_sub(y_lo);

        let n = bands_for(rows * self.width as usize);
        if n < 2 {
            return false;
        }
        let band_rows = rows.div_ceil(n);
        let width = self.width;
        let slice = &mut self.pixels[y_lo * stride..y_hi * stride];

        std::thread::scope(|sc| {
            for (k, chunk) in slice.chunks_mut(band_rows * stride).enumerate() {
                let y0 = (y_lo + k * band_rows) as i64;
                let nrows = (chunk.len() / stride) as i64;
                sc.spawn(move || {
                    let mut band = Surface { px: chunk, width, y0, rows: nrows };
                    band.render(
                        list,
                        damage,
                        #[cfg(feature = "truetype")]
                        font,
                        None,
                    );
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

/// Intersect a rect with every clip on the stack; `None` if nothing
/// survives.
///
/// Honouring the clip stack is not cosmetic. A `Repeat` places rows
/// outside its viewport by construction and `hit` rejects points there,
/// so a renderer that ignores clips draws content no click can reach —
/// the probe-vs-click class of bug, one layer down. `tests/clip_test.rs`
/// pins it at the pixel level.
/// One `Cmd::Text` as the glyph blitter wants it: the fields of the
/// command, borrowed, so the call site does not spell out six scalars.
#[cfg(feature = "truetype")]
struct TextRun<'a> {
    rect: &'a [f32; 4],
    text: &'a str,
    size: f32,
    weight: f32,
    color: Color,
}

/// How many bands to split `px` damaged pixels into — T17.4.
///
/// **The threshold is the point of this function, not the arithmetic.**
/// A thread costs 5–10 µs to wake, and a damaged tally frame rasterises
/// in 12 µs, so threading one is a straight loss. Only frames big enough
/// to amortise a wake-up are split, and the rest take the single-band
/// path with one comparison spent deciding.
///
/// 0.25 Mpx is roughly an eighth of a 1080p surface and about eight
/// times the tally's whole damaged region. `docs/results-threading.md`
/// measured the band overhead itself at 1–6% across sixteen bands, so
/// the split is nearly free once it is worth doing at all.
#[cfg(feature = "threads")]
fn bands_for(px: usize) -> usize {
    const MIN_PX: usize = 250_000;
    if px < MIN_PX {
        return 1;
    }
    // `LUMEN_BANDS` overrides the count, for tuning and for the sweep in
    // `docs/results-threading.md`. Read once: `available_parallelism` is
    // a syscall and this is on the frame path.
    static CAP: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    let cap = *CAP.get_or_init(|| {
        std::env::var("LUMEN_BANDS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|n| *n > 0)
            .unwrap_or_else(|| {
                // **Capped at 8, measured** (`docs/results-threading.md`).
                // A frame ends at a barrier -- every band must finish --
                // so it runs at the speed of the slowest one. On a
                // heterogeneous CPU that means the slowest *core*: on
                // the reference 13900K (8 performance cores, 16
                // efficiency cores, 32 threads) a 4K redraw is 8.91 ms
                // at 8 bands and 9.92 at 32, because past the
                // performance cores the extra bands land somewhere
                // slower and the whole frame waits for them.
                //
                // The right cap is the performance-core count, which
                // `std` cannot report. 8 is that number here and a
                // reasonable ceiling elsewhere; `LUMEN_BANDS` overrides
                // it for anyone whose machine disagrees.
                std::thread::available_parallelism().map_or(1, |n| n.get()).min(8)
            })
    });
    // Never more bands than there is work to justify them.
    cap.min(px / MIN_PX).max(1)
}

/// One row's run of RGBA -> `0RGB`, shared by both blit paths (T17.3).
///
/// `dst` is the destination row; `base` is that row's first pixel index
/// into `pixels`, and `x0..x1` are pixel columns.
#[inline]
fn blit_run(pixels: &[u8], dst: &mut [u32], base: usize, x0: usize, x1: usize) {
    #[cfg(feature = "simd")]
    {
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
