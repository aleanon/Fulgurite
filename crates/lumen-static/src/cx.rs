//! Layout context — plan T5.2.
//!
//! Some measurements need a service a widget cannot own: real text needs
//! a font system and a shaper. Threading that as a *type parameter*
//! would be viral through every signature and would spend §8's
//! single-type-axis budget, so it arrives as a concrete context struct
//! with `dyn` inside — the shape recorded in the renderer/executor
//! answer (`docs/plan-agent-parity.md`, appendix).

// `alloc`, not the std prelude: this crate is `no_std` when the
// `std` feature is off (M13/T13.1), and absent entirely without the
// `alloc` feature (T13.4).
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

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
#[cfg(feature = "alloc")]
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
    /// **Once per edit is once per edit of the whole text.** A `TextArea`
    /// reshapes its whole value on every keystroke, which is linear —
    /// about 5 ms at 100 KB with `FontMetrics` since M45/T45.10 removed two
    /// quadratics from it (`docs/results-text.md` T45.10) — and is what a
    /// paragraph cache would bound, if a document size ever needs one.
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

    /// Which measurer is answering, and in which state — M45/T45.2.
    ///
    /// Part of every [`MeasureKey`](crate::text::MeasureKey): two answers
    /// with different generations are two different measurers' answers
    /// and must not be served for each other. A measurer that changes
    /// what it measures — a font stack that loaded a face — returns a new
    /// value; one that never changes, and every stand-in, answers the
    /// default 0.
    ///
    /// **Why the measurer reports it and the host does not**: a host is
    /// handed a `&mut dyn TextMeasure` every frame and cannot tell a new
    /// measurer from the old one except by address, and a lazily loaded
    /// fallback face changes measurement with no change of measurer at
    /// all (`docs/plan-text-correctness.md` §2.3).
    ///
    /// **A wrapper must forward this.** The provided 0 is right for a
    /// leaf and wrong for a wrapper, which would report every wrapped
    /// stack as the stand-in — the provided-method hazard `AGENT.md`
    /// records for `Painter`. None of the measurers in this workspace
    /// wraps another today.
    fn generation(&self) -> u32 {
        0
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
        let per_line = crate::num::floor(max_w / advance).max(1.0);
        let lines = crate::num::ceil(text.len() as f32 / per_line);
        Size { w: max_w, h: size * lines }
    }
}

/// Services layout may need. Passed by `&mut` through every `layout`
/// call; concrete, so no type parameter goes viral.
pub struct LayoutCx<'a> {
    pub text: &'a mut dyn TextMeasure,
    /// What `Fill` resolves against on an axis whose maximum is not a
    /// real bound.
    ///
    /// A scroll region measures its content against an **unbounded**
    /// main axis — that is the only way `Shrink` content can exceed the
    /// viewport, because `resolve_self` clamps every node to the limits
    /// it was given. But `Fill` cannot mean "infinity": `used += ∞` then
    /// `free = (∞ − ∞).max(0.0)` is **NaN**, and NaN sizes reach
    /// `NodeState`, make every `contains` false, hash by bit pattern
    /// into the journal and slip past the audit's `EPS` comparisons.
    ///
    /// So the scroller publishes its viewport here for the duration of
    /// its content's layout, and `Fill` inside means *the viewport* —
    /// never the content. That is A5 of
    /// `docs/text-and-scrolling-decisions.md`, and it is the reason no
    /// infinity ever reaches a size.
    ///
    /// `None` outside a scroller, where the constraint's own maximum is
    /// the answer.
    fill_span: Option<Size>,
    /// The display scale the measurer is answering at — M26/T26.17
    /// Phase D #12.
    ///
    /// Part of every [`MeasureKey`](crate::text::MeasureKey), because a
    /// hinted face's advances are not linear in size and the measurer
    /// therefore returns a different logical width at a different scale
    /// for the same text, size and limits. `1.0` unless a host says
    /// otherwise, which is correct for every headless caller: the bug
    /// needs a scale that *changes*, and only a windowed host has one.
    scale: f32,
    /// Which subtrees changed this frame — `docs/plan-invalidation.md`
    /// T16.7.
    ///
    /// A container may skip a child whose range this reports clean. It
    /// rides in the context for the same reason `text` does: threading
    /// it as a type parameter would be viral through every `layout`
    /// signature and would spend §8's single-type-axis budget, and
    /// adding a parameter to `Node::layout` would change every widget
    /// in the workspace to say something most of them do not care
    /// about.
    ///
    /// [`crate::invalidate::Changed::ALL`] unless a caller says otherwise, so
    /// [`LayoutCx::new`] behaves exactly as it did before T16.7 and
    /// every existing caller is unaffected.
    pub changed: crate::invalidate::Changed<'a>,
    /// The host's palette — M25/T25.3 — for a widget that wants a colour
    /// no stylesheet rule sets. A rule that names a role does not need it:
    /// the cascade already resolved that role into the node's `Style`
    /// (M25/T25.11).
    ///
    /// Here for the reason `changed` is: a parameter on `Node::layout`
    /// would change every widget to say something most do not use. The
    /// host sets it with [`LayoutCx::with_palette`];
    /// [`crate::style::Palette::LIGHT`] otherwise, which is the colours the
    /// widgets' defaults already are, so every existing caller is
    /// unaffected.
    pub palette: &'a crate::style::Palette,
}

impl<'a> LayoutCx<'a> {
    pub fn new(text: &'a mut dyn TextMeasure) -> Self {
        LayoutCx {
            text,
            fill_span: None,
            scale: 1.0,
            changed: crate::invalidate::Changed::ALL,
            palette: &crate::style::Palette::LIGHT,
        }
    }

    /// The same context, measuring for a display with this scale factor
    /// (M26/T26.17 Phase D #12).
    ///
    /// A shell calls this with its own scale; nothing else needs to.
    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
        self
    }

    /// The display scale in force — [`MeasureKey::new`](crate::text::MeasureKey::new)
    /// reads it.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// The same context, resolving looks against `palette` (M25/T25.3).
    pub fn with_palette(mut self, palette: &'a crate::style::Palette) -> Self {
        self.palette = palette;
        self
    }

    /// A context that knows which subtrees changed, so containers may
    /// skip the ones that did not (T16.7).
    ///
    /// The whole-tree walk is what [`LayoutCx::new`] gives and what the
    /// oracle uses; this is the bounded one.
    pub fn with_changed(
        text: &'a mut dyn TextMeasure,
        changed: crate::invalidate::Changed<'a>,
    ) -> Self {
        LayoutCx {
            text,
            fill_span: None,
            scale: 1.0,
            changed,
            palette: &crate::style::Palette::LIGHT,
        }
    }

    /// Run `f` with `span` as the `Fill` reference, restoring whatever
    /// was there before.
    ///
    /// Restoring matters: scrollers nest, and an inner one must not
    /// leave its viewport in place for the outer one's remaining
    /// children.
    pub fn with_fill_span<R>(&mut self, span: Size, f: impl FnOnce(&mut Self) -> R) -> R {
        let saved = self.fill_span.replace(span);
        let out = f(self);
        self.fill_span = saved;
        out
    }

    /// The extent `Fill` resolves against along one axis: the
    /// constraint's own maximum when that is a real bound, else the
    /// enclosing scroller's viewport, else zero.
    ///
    /// Zero rather than infinity for the last case: a `Fill` with
    /// nothing to fill against occupies nothing, which is visible and
    /// inert. An infinity would propagate into `NodeState`.
    #[inline]
    pub fn fill_extent(&self, bound: f32, vertical: bool) -> f32 {
        if bound.is_finite() {
            return bound;
        }
        match self.fill_span {
            Some(s) if vertical => s.h,
            Some(s) => s.w,
            None => 0.0,
        }
    }
}
