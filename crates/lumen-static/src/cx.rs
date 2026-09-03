//! Layout context — plan T5.2.
//!
//! Some measurements need a service a widget cannot own: real text needs
//! a font system and a shaper. Threading that as a *type parameter*
//! would be viral through every signature and would spend §8's
//! single-type-axis budget, so it arrives as a concrete context struct
//! with `dyn` inside — the shape recorded in the renderer/executor
//! answer (`docs/plan-agent-parity.md`, appendix).

use crate::desc::Size;

/// One cluster boundary in a shaped run: a place an insertion point may
/// sit, and where that place is on screen.
///
/// A cluster, not a glyph: the unit a caret may sit between and a hit
/// test may land on. For Latin they coincide; for a ligature or a
/// combining sequence one glyph carries several source bytes and only
/// the boundaries are addressable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cluster {
    /// Byte offset into the source string. Always on a `char` boundary.
    pub byte: u32,
    /// X from the run's origin, in px — the left edge of the cluster.
    pub x: f32,
    /// Which line the cluster landed on, 0-based. Multiply by
    /// [`TextMeasure::line_height`] for its y.
    pub line: u16,
}

/// Receives cluster boundaries from [`TextMeasure::shape`].
///
/// A trait rather than a `&mut Vec<Cluster>` because the buffer belongs
/// to the text storage, and which buffer that is depends on the storage
/// (`docs/text-and-scrolling-decisions.md` B9: `String` pairs with a
/// `Vec`, an inline buffer with an array). `shape` itself never
/// allocates; a sink with a fixed capacity simply stops recording, and
/// says so through [`GlyphSink::is_full`] so a caller can tell a
/// truncated run from a short one.
pub trait GlyphSink {
    /// Record one cluster boundary. Called in source order.
    fn cluster(&mut self, c: Cluster);

    /// Whether further clusters would be dropped. `shape` checks this
    /// and stops early; the default never fills.
    fn is_full(&self) -> bool {
        false
    }
}

/// The obvious growable sink. `String` storage pairs with this one
/// (B9); a fixed-capacity storage supplies its own and overrides
/// `is_full`.
impl GlyphSink for Vec<Cluster> {
    fn cluster(&mut self, c: Cluster) {
        self.push(c);
    }
}

/// Discards every cluster — what [`TextMeasure::shape`] is given when
/// the caller wants only the size.
pub struct NoSink;

impl GlyphSink for NoSink {
    fn cluster(&mut self, _: Cluster) {}
}

/// Measures a run of text. Implemented by whatever shaping stack the
/// host composes in — `lumen-static-shell`'s `FontMetrics` (fontdue)
/// is the one that ships; the default below keeps the framework
/// runnable with none. Lumen's parley stack was measured and declined
/// (`docs/text-render-adoption-decision.md`).
pub trait TextMeasure {
    /// Size of `text` at `size` px, wrapped to at most `max_w`.
    fn measure(&mut self, text: &str, size: f32, max_w: f32) -> Size;

    /// `measure`, with a weight. The default ignores it, which is right
    /// for a stand-in with one face; a real stack overrides.
    fn measure_weighted(&mut self, text: &str, size: f32, weight: f32, max_w: f32) -> Size {
        let _ = weight;
        self.measure(text, size, max_w)
    }

    /// Measure *and* report every cluster boundary to `sink`, returning
    /// the size `measure_weighted` would.
    ///
    /// This is the method a caret, a selection rectangle and a
    /// click-to-offset hit test are built from, and the one M8's shaped
    /// cache will store the result of (`docs/text-and-scrolling-
    /// decisions.md` B2) — it exists so a text field shapes once per
    /// edit rather than once per frame per query. Reporting into a sink
    /// rather than returning a buffer is what keeps it allocation-free
    /// (T7.2b).
    ///
    /// The default measures and reports nothing, so a stand-in stays
    /// usable: a caller that gets no clusters falls back to the run's
    /// size, which is what `BlockMetrics` deserves.
    fn shape(
        &mut self,
        text: &str,
        size: f32,
        weight: f32,
        max_w: f32,
        sink: &mut dyn GlyphSink,
    ) -> Size {
        let _ = sink;
        self.measure_weighted(text, size, weight, max_w)
    }

    /// Distance between baselines at `size` px. A caret's height and a
    /// wrapped run's line spacing both come from here.
    fn line_height(&mut self, size: f32) -> f32 {
        size * 1.2
    }
}

/// The stand-in used until a shaping stack is composed in: a fixed
/// advance per byte and a single line. Deliberately crude, and the only
/// thing that has to change for real text to work.
#[derive(Default, Debug)]
pub struct BlockMetrics;

impl TextMeasure for BlockMetrics {
    fn measure(&mut self, text: &str, size: f32, max_w: f32) -> Size {
        let advance = size * 0.5;
        let natural = advance * text.len() as f32;
        if max_w <= 0.0 || natural <= max_w {
            return Size { w: natural, h: size };
        }
        // Wrap by character count — enough to make wrapping *happen*, so
        // the layout model is exercised; a real shaper breaks on grapheme
        // and word boundaries.
        let per_line = (max_w / advance).floor().max(1.0);
        let lines = (text.len() as f32 / per_line).ceil();
        Size { w: max_w, h: size * lines }
    }
}

/// Services layout may need. Passed by `&mut` through every `layout`
/// call; concrete, so no type parameter goes viral.
pub struct LayoutCx<'a> {
    pub text: &'a mut dyn TextMeasure,
}

impl<'a> LayoutCx<'a> {
    pub fn new(text: &'a mut dyn TextMeasure) -> Self {
        LayoutCx { text }
    }
}
