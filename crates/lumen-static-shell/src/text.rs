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

use std::borrow::Cow;
use fontdue::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle};
use fontdue::Font;
use lumen_static::cx::{Cluster, GlyphSink, TextMeasure};
use lumen_static::Size;

/// A loaded font, shared by the measurer and the rasterizer so the two
/// cannot disagree about advances.
#[derive(Clone)]
pub struct FontStack {
    regular: std::sync::Arc<Font>,
    /// A bold face when one was found; otherwise `regular` serves both
    /// weights and the request is quietly honoured as best it can be.
    bold: Option<std::sync::Arc<Font>>,
}

/// Where to look for a sans-serif face, in order: (regular, bold).
const CANDIDATES: &[(&str, &str)] = &[
    (
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
    ),
    (
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    ),
    ("/usr/share/fonts/TTF/DejaVuSans.ttf", "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf"),
    ("/System/Library/Fonts/Helvetica.ttc", "/System/Library/Fonts/Helvetica.ttc"),
    ("C:\\Windows\\Fonts\\segoeui.ttf", "C:\\Windows\\Fonts\\segoeuib.ttf"),
];

impl FontStack {
    /// Load a regular face, and optionally a bold one, from bytes.
    pub fn from_bytes(regular: &[u8], bold: Option<&[u8]>) -> Option<Self> {
        let load = |b: &[u8]| {
            Font::from_bytes(b, fontdue::FontSettings::default()).ok().map(std::sync::Arc::new)
        };
        Some(FontStack { regular: load(regular)?, bold: bold.and_then(load) })
    }

    /// The face for a CSS-style weight: 600 and up is bold.
    fn face(&self, weight: f32) -> &Font {
        match (&self.bold, weight >= 600.0) {
            (Some(b), true) => b,
            _ => &self.regular,
        }
    }

    /// Find a system sans-serif face.
    ///
    /// Returns `None` rather than panicking: a caller can fall back to
    /// the block stand-in and still show *something*, which is better
    /// than a window that fails to open on a machine with unusual font
    /// paths.
    pub fn system() -> Option<Self> {
        CANDIDATES.iter().find_map(|(r, b)| {
            let regular = std::fs::read(r).ok()?;
            let bold = std::fs::read(b).ok();
            Self::from_bytes(&regular, bold.as_deref())
        })
    }

    /// Pick the face for `weight` and, if it still lacks a glyph,
    /// substitute per character.
    ///
    /// Tofu doctrine (Lumen's `verifying-apps` skill): a glyph the
    /// chosen face lacks must not render as a box if any face has it.
    /// Try the requested weight, then the other face; only then
    /// substitute — per character rather than per run, so one missing
    /// symbol does not box the whole label.
    ///
    /// Returns the face and the text to lay out. The returned text is
    /// borrowed when nothing was substituted, which is every steady-state
    /// call; substitution allocates, and is a fallback path.
    fn prepare<'t>(&self, text: &'t str, weight: f32) -> (&Font, Cow<'t, str>) {
        let wanted = self.face(weight);
        let has_all =
            |f: &Font| text.chars().all(|c| c.is_whitespace() || f.lookup_glyph_index(c) != 0);
        let other: &Font = if std::ptr::eq(wanted, &*self.regular) {
            self.bold.as_deref().unwrap_or(&self.regular)
        } else {
            &self.regular
        };
        let font = if has_all(wanted) {
            wanted
        } else if has_all(other) {
            other
        } else {
            wanted
        };
        if has_all(font) {
            return (font, Cow::Borrowed(text));
        }
        let out = text
            .chars()
            .map(|c| {
                if c.is_whitespace() || font.lookup_glyph_index(c) != 0 {
                    c
                } else {
                    match c {
                        '\u{2212}' | '\u{2013}' | '\u{2014}' => '-',
                        '\u{2018}' | '\u{2019}' => '\'',
                        '\u{201c}' | '\u{201d}' => '"',
                        _ => '?',
                    }
                }
            })
            .collect();
        (font, Cow::Owned(out))
    }

    /// Run fontdue's line layout for one styled run.
    fn run(font: &Font, text: &str, px: f32, max_w: f32) -> Layout {
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            max_width: if max_w.is_finite() && max_w > 0.0 { Some(max_w) } else { None },
            ..LayoutSettings::default()
        });
        layout.append(&[font], &TextStyle::new(text, px, 0));
        layout
    }

    /// Lay out one run and return each glyph's raster and position.
    pub(crate) fn glyphs(
        &self,
        text: &str,
        px: f32,
        weight: f32,
        max_w: f32,
    ) -> (Vec<Glyph>, Size) {
        let (font, prepared) = self.prepare(text, weight);
        let text: &str = &prepared;
        let layout = Self::run(font, text, px, max_w);

        let shift = cap_shift(font, px);

        let mut out = Vec::with_capacity(text.len());
        let (mut w, mut h) = (0.0f32, 0.0f32);
        for g in layout.glyphs() {
            // Width from the *advance*, not the ink box. Wrapping is
            // done on advances, so measuring ink under-reports by the
            // last glyph's right side bearing — and `paint` then re-wraps
            // inside a box a hair too narrow and drops that glyph.
            // "POSITIVE" came out "POSITIV" exactly this way.
            let adv = font.metrics_indexed(g.key.glyph_index, px).advance_width;
            w = w.max(g.x + adv.max(g.width as f32));
            h = h.max(g.y + g.height as f32);
            if g.width == 0 || g.height == 0 {
                continue;
            }
            let (_, bitmap) = font.rasterize_config(g.key);
            out.push(Glyph { x: g.x, y: g.y + shift, w: g.width, h: g.height, cov: bitmap });
        }
        // Ceiled: `layout` sizes the box from this, and `paint` re-wraps
        // inside it. A width rounded *down* by a fraction of a pixel
        // drops the last glyph — which is how "POSITIVE" came out as
        // "POSITIV".
        (out, Size { w: w.ceil(), h: h.max(layout.height()).ceil() })
    }

    /// The size of a run, **without rasterising it**.
    ///
    /// `measure` used to call [`Self::glyphs`] and keep only its `Size`,
    /// throwing away a `Vec` of freshly rasterised coverage bitmaps —
    /// so *measuring* text rasterised it, and layout paid for pixels no
    /// one looked at. fontdue has no glyph cache, so that was a real
    /// raster per glyph per measure.
    ///
    /// It cost **2.9 µs a label**, which at 3000 labels is 8.7 ms of a
    /// frame; `docs/results-competitive.md` has the before and after.
    /// The width and height arithmetic below is lifted verbatim from
    /// `glyphs`, so the two cannot disagree about a box — which they
    /// must not, because `layout` sizes from this and `paint` re-wraps
    /// inside it.
    pub(crate) fn measure_run(&self, text: &str, px: f32, weight: f32, max_w: f32) -> Size {
        let (font, prepared) = self.prepare(text, weight);
        let text: &str = &prepared;
        let layout = Self::run(font, text, px, max_w);
        let (mut w, mut h) = (0.0f32, 0.0f32);
        for g in layout.glyphs() {
            // Width from the *advance*, not the ink box — see `glyphs`.
            let adv = font.metrics_indexed(g.key.glyph_index, px).advance_width;
            w = w.max(g.x + adv.max(g.width as f32));
            h = h.max(g.y + g.height as f32);
        }
        Size { w: w.ceil(), h: h.max(layout.height()).ceil() }
    }

    /// Shape one run and report every cluster boundary to `sink`.
    ///
    /// No rasterization: this is the query a caret, a selection or a
    /// click-to-offset makes, and it must not cost a glyph cache. The
    /// trailing boundary (an insertion point after the last character)
    /// is reported too, so a run of `n` characters yields `n + 1`
    /// clusters — the caret positions a text field actually has.
    ///
    /// Byte offsets are into the *caller's* string. When a glyph was
    /// substituted the laid-out string differs from it, and since
    /// substitution is one char for one char the offsets are mapped back
    /// through the character correspondence rather than reported from
    /// the substituted copy — `\u{2212}` is three bytes and `-` is one,
    /// so reporting the copy's offsets would put a caret mid-character.
    pub(crate) fn shape_into(
        &self,
        text: &str,
        px: f32,
        weight: f32,
        max_w: f32,
        sink: &mut dyn GlyphSink,
    ) -> Size {
        let (font, prepared) = self.prepare(text, weight);
        let laid: &str = &prepared;
        let layout = Self::run(font, laid, px, max_w);

        // Offset in the laid-out copy → offset in the caller's string.
        // Identity in the common case; a lookup table only when the
        // fallback path ran.
        let remap = |b: usize| -> u32 {
            if matches!(prepared, Cow::Borrowed(_)) {
                return b as u32;
            }
            match laid.char_indices().position(|(i, _)| i == b) {
                Some(n) => text.char_indices().nth(n).map(|(i, _)| i as u32).unwrap_or(b as u32),
                None => text.len() as u32,
            }
        };

        let lines = layout.lines();
        let line_of = |gi: usize| -> u16 {
            match lines {
                Some(ls) => {
                    ls.iter().position(|l| gi >= l.glyph_start && gi <= l.glyph_end).unwrap_or(0)
                        as u16
                }
                None => 0,
            }
        };

        let (mut w, mut h) = (0.0f32, 0.0f32);
        let mut end = (0.0f32, 0u16);
        for (gi, g) in layout.glyphs().iter().enumerate() {
            let adv = font.metrics_indexed(g.key.glyph_index, px).advance_width;
            w = w.max(g.x + adv.max(g.width as f32));
            h = h.max(g.y + g.height as f32);
            let line = line_of(gi);
            if !sink.is_full() {
                sink.cluster(Cluster { byte: remap(g.byte_offset), x: g.x, line });
            }
            end = (g.x + adv, line);
        }
        // The insertion point after the last character.
        if !sink.is_full() {
            sink.cluster(Cluster { byte: text.len() as u32, x: end.0, line: end.1 });
        }
        Size { w: w.ceil(), h: h.max(layout.height()).ceil() }
    }
}

/// Divides cluster positions back to logical units on the way out.
struct LogicalClusters<'a> {
    inner: &'a mut dyn GlyphSink,
    scale: f32,
}

impl GlyphSink for LogicalClusters<'_> {
    fn cluster(&mut self, c: Cluster) {
        self.inner.cluster(Cluster { x: c.x / self.scale, ..c });
    }
    fn is_full(&self) -> bool {
        self.inner.is_full()
    }
}

/// How far to move a run so its capitals are centred in its line box.
///
/// The line box is what gets centred in a button, and its middle is not
/// where the eye puts the middle of a word: capitals and digits hang
/// from the ascent, and Noto's ascent is 1.07 em against a 0.71 em cap,
/// so line-box-centred text reads ~2 px low at 16 px and 6 px low at
/// 76 px. This is the "vertical trim" design tools apply. Measured from
/// where the layout actually puts an `H` — its rasterised rows, not the
/// face's cap-height figure — because the bitmap can sit a pixel below
/// the metric baseline, and `tests/glyph_placement.rs` checks ink, not
/// metrics. Descenders still hang below, as they should.
fn cap_shift(font: &Font, px: f32) -> f32 {
    let Some(line) = font.horizontal_line_metrics(px).map(|m| m.new_line_size) else {
        return 0.0;
    };
    let mut probe = Layout::new(CoordinateSystem::PositiveYDown);
    probe.reset(&LayoutSettings::default());
    probe.append(&[font], &TextStyle::new("H", px, 0));
    match probe.glyphs().first() {
        Some(h) if h.height > 0 => (line / 2.0 - (h.y + h.height as f32 / 2.0)).round(),
        _ => 0.0,
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
    /// Physical pixels per logical pixel (M10b).
    ///
    /// Layout speaks logical units, but glyphs are rasterised at
    /// physical ones, and a face's advances are **not** linear in size:
    /// hinting and rounding mean measuring at 16 px and doubling is not
    /// measuring at 32 px. So this measures at `size * scale` and
    /// divides the answer back down. Get it wrong and text drifts out of
    /// the box that was laid out for it — visible as a clipped last
    /// glyph, which is exactly how "POSITIVE" once became "POSITIV"
    /// (`docs/results-frame-cost.md`).
    scale: f32,
}

impl FontMetrics {
    /// A measurer for a 1:1 display.
    pub fn new(stack: FontStack) -> Self {
        FontMetrics { stack, scale: 1.0 }
    }

    /// A measurer that reports logical sizes for a display with this
    /// scale factor.
    pub fn with_scale(stack: FontStack, scale: f32) -> Self {
        let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
        FontMetrics { stack, scale }
    }

    /// Update the scale factor in place — a window can move between
    /// monitors.
    pub fn set_scale(&mut self, scale: f32) {
        if scale.is_finite() && scale > 0.0 {
            self.scale = scale;
        }
    }
}

impl TextMeasure for FontMetrics {
    fn measure(&mut self, text: &str, size: f32, max_w: f32) -> Size {
        self.measure_weighted(text, size, 400.0, max_w)
    }

    fn measure_weighted(&mut self, text: &str, size: f32, weight: f32, max_w: f32) -> Size {
        if text.is_empty() {
            return Size { w: 0.0, h: 0.0 };
        }
        let k = self.scale;
        // `measure_run`, not `glyphs`: measuring must not rasterise.
        let s = self.stack.measure_run(text, size * k, weight, max_w * k);
        Size { w: s.w / k, h: s.h / k }
    }

    fn shape(
        &mut self,
        text: &str,
        size: f32,
        weight: f32,
        max_w: f32,
        sink: &mut dyn GlyphSink,
    ) -> Size {
        if text.is_empty() {
            // One cluster: the caret position in an empty field.
            sink.cluster(Cluster { byte: 0, x: 0.0, line: 0 });
            return Size { w: 0.0, h: self.line_height(size) };
        }
        // Clusters are reported in logical x, because a caret is placed
        // by `layout` and `paint`, both of which speak logical.
        let k = self.scale;
        let mut scaled = LogicalClusters { inner: sink, scale: k };
        let s = self.stack.shape_into(text, size * k, weight, max_w * k, &mut scaled);
        Size { w: s.w / k, h: s.h / k }
    }

    fn line_height(&mut self, size: f32) -> f32 {
        let k = self.scale;
        self.stack
            .regular
            .horizontal_line_metrics(size * k)
            .map(|m| m.new_line_size / k)
            .unwrap_or(size * 1.2)
    }
}
