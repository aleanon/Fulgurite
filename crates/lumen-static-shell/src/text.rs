//! A real text stack for the software renderer.
//!
//! `BlockMetrics` — the core's stand-in — reports `char_count × advance`
//! and the renderer draws one filled square per glyph. That is enough to
//! prove the pipeline end to end without a font, which is why it exists,
//! and it is why a window showed a row of black boxes rather than a
//! counter.
//!
//! This is the same seam M7's `lumen-text` adoption will use: a
//! [`TextMeasure`] for layout and a rasterizer for paint, both behind
//! traits the widgets never name. Feature-gated (`truetype`), so the
//! headless oracle and every test still run on the stand-in — a
//! deterministic measurer is what makes golden geometry assertions
//! possible at all.

use fontdue::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle};
use fontdue::Font;
use lumen_static::cx::TextMeasure;
use lumen_static::Size;

/// A loaded font, shared by the measurer and the rasterizer so the two
/// cannot disagree about advances.
#[derive(Clone)]
pub struct FontStack {
    font: std::sync::Arc<Font>,
}

/// Where to look for a sans-serif face, in order.
const CANDIDATES: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu/DejaVuSans.ttf",
    "/System/Library/Fonts/Helvetica.ttc",
    "C:\\Windows\\Fonts\\segoeui.ttf",
];

impl FontStack {
    /// Load a face from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        Font::from_bytes(bytes, fontdue::FontSettings::default())
            .ok()
            .map(|font| FontStack { font: std::sync::Arc::new(font) })
    }

    /// Find a system sans-serif face.
    ///
    /// Returns `None` rather than panicking: a caller can fall back to
    /// the block stand-in and still show *something*, which is better
    /// than a window that fails to open on a machine with unusual font
    /// paths.
    pub fn system() -> Option<Self> {
        CANDIDATES
            .iter()
            .find_map(|p| std::fs::read(p).ok())
            .and_then(|b| Self::from_bytes(&b))
    }

    /// Lay out one run and return each glyph's raster and position.
    pub(crate) fn glyphs(&self, text: &str, px: f32, max_w: f32) -> (Vec<Glyph>, Size) {
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            max_width: if max_w.is_finite() && max_w > 0.0 { Some(max_w) } else { None },
            ..LayoutSettings::default()
        });
        layout.append(&[self.font.as_ref()], &TextStyle::new(text, px, 0));

        let mut out = Vec::with_capacity(text.len());
        let (mut w, mut h) = (0.0f32, 0.0f32);
        for g in layout.glyphs() {
            // Width from the *advance*, not the ink box. Wrapping is
            // done on advances, so measuring ink under-reports by the
            // last glyph's right side bearing — and `paint` then re-wraps
            // inside a box a hair too narrow and drops that glyph.
            // "POSITIVE" came out "POSITIV" exactly this way.
            let adv = self.font.metrics_indexed(g.key.glyph_index, px).advance_width;
            w = w.max(g.x + adv.max(g.width as f32));
            h = h.max(g.y + g.height as f32);
            if g.width == 0 || g.height == 0 {
                continue;
            }
            let (_, bitmap) = self.font.rasterize_config(g.key);
            out.push(Glyph { x: g.x, y: g.y, w: g.width, h: g.height, cov: bitmap });
        }
        // Ceiled: `layout` sizes the box from this, and `paint` re-wraps
        // inside it. A width rounded *down* by a fraction of a pixel
        // drops the last glyph — which is how "POSITIVE" came out as
        // "POSITIV".
        (out, Size { w: w.ceil(), h: h.max(layout.height()).ceil() })
    }
}

/// One rasterized glyph: an 8-bit coverage mask and where to put it.
pub(crate) struct Glyph {
    pub x: f32,
    pub y: f32,
    pub w: usize,
    pub h: usize,
    pub cov: Vec<u8>,
}

/// [`TextMeasure`] backed by a real face — what `layout` asks.
pub struct FontMetrics {
    stack: FontStack,
}

impl FontMetrics {
    pub fn new(stack: FontStack) -> Self {
        FontMetrics { stack }
    }
}

impl TextMeasure for FontMetrics {
    fn measure(&mut self, text: &str, size: f32, max_w: f32) -> Size {
        if text.is_empty() {
            return Size { w: 0.0, h: 0.0 };
        }
        self.stack.glyphs(text, size, max_w).1
    }
}
