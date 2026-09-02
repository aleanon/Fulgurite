//! A software renderer — enough to see the frame without a GPU or a
//! shaping stack.
//!
//! Deliberately small: filled rectangles, and text drawn as one block
//! per character at the advance `BlockMetrics` assumes, so what is drawn
//! matches what was measured. Its job is to make the pipeline
//! *verifiable* headlessly; a real rasterizer (parity Phase A adopts
//! Lumen's `lumen-render`) replaces it without touching a widget.

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

    /// Fill a rect whose corners are rounded by `radius`.
    ///
    /// Coverage is sampled once per pixel against the corner circles —
    /// no anti-aliasing beyond that, which is the right amount of
    /// renderer for a reference implementation whose job is to be
    /// checkable rather than pretty.
    /// Blit one text run with real glyphs.
    ///
    /// The run's box is its *layout* box, so glyphs are placed from its
    /// top-left and clipped to whatever clip scopes are open — the same
    /// treatment a rect gets, which is what keeps text inside a scroller.
    #[cfg(feature = "truetype")]
    fn draw_run(
        &mut self,
        font: crate::text::FontStack,
        rect: &[f32; 4],
        text: &str,
        size: f32,
        color: Color,
        clip: &[[f32; 4]],
    ) {
        let (glyphs, _) = font.glyphs(text, size, rect[2] - rect[0]);
        for g in glyphs {
            for row in 0..g.h {
                for col in 0..g.w {
                    let cov = g.cov[row * g.w + col];
                    if cov == 0 {
                        continue;
                    }
                    let px = rect[0] + g.x + col as f32;
                    let py = rect[1] + g.y + row as f32;
                    // One-pixel rect through the same clip path as
                    // everything else, so a glyph cannot escape a scope
                    // that a rect could not.
                    if clipped([px, py, px + 1.0, py + 1.0], clip).is_none() {
                        continue;
                    }
                    let a = (color.3 as u32 * cov as u32 / 255) as u8;
                    self.blend(px as i64, py as i64, Color(color.0, color.1, color.2, a));
                }
            }
        }
    }

    /// Fill `bounds`, rounding against `shape`'s corners.
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
        for y in y0..y1 {
            for x in x0..x1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                // Distance past the inner rect, per axis: zero anywhere
                // but a corner.
                let dx = (r[0] + rad - px).max(px - (r[2] - rad)).max(0.0);
                let dy = (r[1] + rad - py).max(py - (r[3] - rad)).max(0.0);
                if dx * dx + dy * dy <= rad * rad {
                    self.blend(x, y, c);
                }
            }
        }
    }

    pub fn clear(&mut self, c: Color) {
        for px in self.pixels.chunks_exact_mut(4) {
            px.copy_from_slice(&[c.0, c.1, c.2, c.3]);
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
            for x in x0..x1 {
                self.blend(x, y, c);
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

impl Renderer for Canvas {
    fn render(&mut self, list: &DisplayList, _w: u32, _h: u32) {
        let mut clip: Vec<[f32; 4]> = Vec::new();
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
                Cmd::Text { rect, text, size, color } => {
                    let s = list.str_of(*text);
                    #[cfg(feature = "truetype")]
                    if let Some(font) = self.font.clone() {
                        self.draw_run(font, rect, s, *size, *color, &clip);
                        continue;
                    }
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
                    clip.pop();
                }
            }
        }
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
