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
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Self {
        Canvas { width, height, pixels: vec![255; (width * height * 4) as usize] }
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
        let (x0, y0) = (r[0].floor() as i64, r[1].floor() as i64);
        let (x1, y1) = (r[2].ceil() as i64, r[3].ceil() as i64);
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
        // Clipping is tracked but not yet applied per-pixel; the stack is
        // here so the command vocabulary is exercised end to end.
        let mut clip: Vec<[f32; 4]> = Vec::new();
        for c in &list.cmds {
            match c {
                Cmd::Rect { rect, color } => self.fill(*rect, *color),
                Cmd::Text { rect, text, size, color } => {
                    let s = list.str_of(*text);
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
                        self.fill([x + 0.5, rect[1] + 2.0, x + advance - 0.5, rect[1] + size - 2.0], *color);
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
