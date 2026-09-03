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

use lumen_static::display::{Cmd, Color, DisplayList, Renderer};

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
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Self {
        Canvas {
            width,
            height,
            pixels: vec![255; (width * height * 4) as usize],
            #[cfg(feature = "truetype")]
            font: None,
        }
    }

    /// A canvas that draws real glyphs.
    #[cfg(feature = "truetype")]
    pub fn with_font(width: u32, height: u32, font: crate::text::FontStack) -> Self {
        let mut c = Self::new(width, height);
        c.font = Some(font);
        c
    }

    /// Blit one text run with real glyphs.
    ///
    /// The run's box is its *layout* box, so glyphs are placed from its
    /// top-left and clipped to whatever clip scopes are open — the same
    /// treatment a rect gets, which is what keeps text inside a scroller.
    /// Each glyph is clipped once as a rect; the first version clipped
    /// every pixel and that was most of the text cost.
    #[cfg(feature = "truetype")]
    fn draw_run(&mut self, font: crate::text::FontStack, run: TextRun<'_>, clip: &[[f32; 4]]) {
        let TextRun { rect, text, size, weight, color } = run;
        let (glyphs, _) = font.glyphs(text, size, weight, rect[2] - rect[0]);
        for g in glyphs {
            let gx = (rect[0] + g.x).round();
            let gy = (rect[1] + g.y).round();
            let Some(r) = clipped([gx, gy, gx + g.w as f32, gy + g.h as f32], clip) else {
                continue;
            };
            let (w, h) = (self.width as i64, self.height as i64);
            let (x0, y0) = ((r[0] as i64).max(0), (r[1] as i64).max(0));
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
        let (w, h) = (self.width as i64, self.height as i64);
        let (x0, y0) = ((bounds[0].floor() as i64).max(0), (bounds[1].floor() as i64).max(0));
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
        let (w, h) = (self.width as i64, self.height as i64);
        let (x0, y0) = ((bounds[0].floor() as i64).max(0), (bounds[1].floor() as i64).max(0));
        let (x1, y1) = ((bounds[2].ceil() as i64).min(w), (bounds[3].ceil() as i64).min(h));

        // The shape's inset-by-`rad` rect is entirely inside it — the
        // rounded rect *is* that rect dilated by `rad` — so every pixel
        // in it is at distance <= -rad and contributes nothing. Skipping
        // it structurally rather than computing a distance and throwing
        // it away is most of this function: for the tally's card that is
        // 131k of 158k pixels, and the shadow went from 72% of a frame's
        // render to a tenth of it (`docs/results-frame-cost.md`).
        let skip = rad >= 1.0;
        let (ix0, iy0) = ((shape[0] + rad).ceil() as i64, (shape[1] + rad).ceil() as i64);
        let (ix1, iy1) = ((shape[2] - rad).floor() as i64, (shape[3] - rad).floor() as i64);

        for y in y0..y1 {
            let qy = (y as f32 + 0.5 - cy).abs() - hy;
            let inner_row = skip && y >= iy0 && y < iy1;
            let mut x = x0;
            while x < x1 {
                if inner_row && x >= ix0 && x < ix1 {
                    x = ix1;
                    continue;
                }
                let qx = (x as f32 + 0.5 - cx).abs() - hx;
                // Signed distance to a rounded rect (Quilez).
                let (ax, ay) = (qx.max(0.0), qy.max(0.0));
                let d = (ax * ax + ay * ay).sqrt() + qx.max(qy).min(0.0) - rad;
                if d >= -1.0 {
                    let t = 1.0 - (d / spread).clamp(0.0, 1.0);
                    let a = (c.3 as f32 * t * t) as u8;
                    if a > 0 {
                        self.blend(x, y, Color(c.0, c.1, c.2, a));
                    }
                }
                x += 1;
            }
        }
    }

    pub fn clear(&mut self, c: Color) {
        for px in self.pixels.chunks_exact_mut(4) {
            px.copy_from_slice(&[c.0, c.1, c.2, c.3]);
        }
    }

    /// Clear only `r`. A partial redraw clears the region it is about
    /// to repaint and leaves the rest of the frame standing — clearing
    /// the whole surface first would make the "partial" half pointless.
    pub fn clear_within(&mut self, r: [f32; 4], c: Color) {
        let opaque = Color(c.0, c.1, c.2, 255);
        self.fill(r, opaque);
    }

    /// One row, `[x0, x1)`, already clamped to the surface. Opaque
    /// colours are written, not blended: a card's background is most of
    /// the pixels in a frame.
    fn span(&mut self, y: i64, x0: i64, x1: i64, c: Color) {
        if x0 >= x1 || y < 0 || y >= self.height as i64 || c.3 == 0 {
            return;
        }
        if c.3 == 255 {
            let start = ((y as u32 * self.width + x0 as u32) * 4) as usize;
            let end = ((y as u32 * self.width + x1 as u32) * 4) as usize;
            for px in self.pixels[start..end].chunks_exact_mut(4) {
                px.copy_from_slice(&[c.0, c.1, c.2, 255]);
            }
        } else {
            for x in x0..x1 {
                self.blend(x, y, c);
            }
        }
    }

    fn blend(&mut self, x: i64, y: i64, c: Color) {
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 || c.3 == 0 {
            return;
        }
        let i = ((y as u32 * self.width + x as u32) * 4) as usize;
        let a = c.3 as u32;
        let inv = 255 - a;
        for (k, src) in [c.0, c.1, c.2].into_iter().enumerate() {
            self.pixels[i + k] = ((src as u32 * a + self.pixels[i + k] as u32 * inv) / 255) as u8;
        }
        self.pixels[i + 3] = 255;
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
        let (w, h) = (self.width as i64, self.height as i64);
        let (x0, y0) = ((r[0].floor() as i64).max(0), (r[1].floor() as i64).max(0));
        let (x1, y1) = ((r[2].ceil() as i64).min(w), (r[3].ceil() as i64).min(h));
        for y in y0..y1 {
            self.span(y, x0, x1, c);
        }
    }

    /// Copy the frame into a `0RGB` `u32` buffer — softbuffer's format.
    /// Lives here rather than in the shell binding so it is compiled
    /// with the rasteriser's optimisation level: in the example it was
    /// 2 ms of a 3 ms frame.
    pub fn blit_0rgb(&self, dst: &mut [u32]) {
        for (d, px) in dst.iter_mut().zip(self.pixels.chunks_exact(4)) {
            *d = (px[0] as u32) << 16 | (px[1] as u32) << 8 | px[2] as u32;
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
            for x in x0..x1 {
                let i = (row + x) * 4;
                let px = &self.pixels[i..i + 3];
                dst[row + x] = (px[0] as u32) << 16 | (px[1] as u32) << 8 | px[2] as u32;
            }
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

    fn render_impl(&mut self, list: &DisplayList, damage: Option<[f32; 4]>) {
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
                    let s = list.str_of(*text);
                    #[cfg(feature = "truetype")]
                    if let Some(font) = self.font.clone() {
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
