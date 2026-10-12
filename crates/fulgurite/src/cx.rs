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
///
/// **The contract, from M81/T81.2** (`docs/plan-shaped-paint.md`):
/// clusters arrive in **source order** (rising [`Cluster::byte`]), and
/// [`Cluster::x`] is the cluster's **leading** edge — the edge reading
/// starts from, which is the left edge in a left-to-right run and the
/// right edge in a right-to-left one, as [`Cluster::rtl`] says. So in a
/// right-to-left run x *falls* as the byte rises. `ComplexText` used to
/// report visual order with left edges, which broke "in source order";
/// every measurer here now keeps it. A measurer that shapes no
/// right-to-left text (`BakedFace`, `FontMetrics`) reports `rtl: false`
/// throughout, and for it nothing changed.
///
/// **A direction change is two insertion points** (M81/T81.5): where a
/// run ends and the next character, or the text's end, runs the other
/// way, the offset between them is reported twice with one byte — first
/// the ending run's *trailing* edge (its `rtl` and line), then the next
/// cluster's leading edge. No leading edge is that boundary, so without
/// it a caret, a click or a selection's tint could not reach it. A reader
/// wanting one place per offset takes the **last** cluster with that
/// byte; a run that never changes direction reports no pair.
///
/// 12 bytes: `rtl` takes a byte of what was padding after `line`
/// (`docs/results-embedded.md`, M81/T81.2, prints the size).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cluster {
    /// Byte offset into the source string. Always on a `char` boundary.
    pub byte: u32,
    /// X from the run's origin, in px — the cluster's leading edge: its
    /// left edge when `rtl` is clear, its right edge when it is set.
    pub x: f32,
    /// Which line the cluster landed on, 0-based. Multiply by
    /// [`TextMeasure::line_height`] for its y.
    pub line: u16,
    /// Whether the cluster sits in a right-to-left run, so `x` is its
    /// right edge and its trailing edge lies to the left. The trailing
    /// insertion point (`byte` = the text's length) carries the
    /// paragraph's direction.
    pub rtl: bool,
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
    /// Record one cluster boundary. Called in source order, with `x` the
    /// leading edge ([`Cluster`]'s contract), whatever the run's
    /// direction — and twice for one byte at a direction change, the
    /// first time with the ending run's trailing edge.
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

/// Where [`TextMeasure::outline`] writes a run's glyph outlines —
/// M82/T82.7 (`docs/plan-paint-vocabulary.md`, "Vector text is outlines
/// from the measurer").
///
/// The outline arrives as [`PathEl`](crate::display::PathEl)s, the
/// vocabulary [`Painter::path`](crate::display::Painter::path) draws, so a
/// sink that collects them into a `Vec` holds a path ready to paint. A
/// sink, not a returned buffer, for the reason [`GlyphSink`] is one: the
/// measurer allocates nothing for the caller.
///
/// **Coordinates are the run's**, as [`TextMeasure::outline`] states them:
/// logical px, the run's top-left at the origin, y down.
pub trait PathSink {
    /// One element of the outline, in order. Each glyph's contours are
    /// whole subpaths — each starts with a `Move` and ends with a `Close`.
    fn el(&mut self, e: crate::display::PathEl);

    /// Called before glyph `index`'s elements (its id in the face that
    /// draws it), for a sink that wants to tell one glyph from the next —
    /// a test that holds each glyph's outline to its pen. A glyph with no
    /// outline (a space) is announced and adds no element. The default
    /// ignores it.
    ///
    /// **A sink that wraps another must forward this**, the provided-method
    /// hazard `AGENT.md` records for `Painter`.
    fn glyph(&mut self, index: u16) {
        let _ = index;
    }
}

/// The obvious growable sink: the elements, in order, ready for
/// [`Painter::path`](crate::display::Painter::path).
#[cfg(feature = "alloc")]
impl PathSink for Vec<crate::display::PathEl> {
    fn el(&mut self, e: crate::display::PathEl) {
        self.push(e);
    }
}

/// Counts the lines a shaped run landed on — M80/T80.2
/// (`docs/plan-text-fit.md`, "Measurement needs no `TextMeasure` change").
///
/// A widget with a line height set shapes into this and takes
/// `lines × pitch` ([`pitched_height`]), because every measurer that
/// shapes already reports each cluster's [`Cluster::line`]. A measurer
/// that reports no clusters (`BlockMetrics`, the test stand-ins) leaves
/// it at 0, and the caller keeps that measurer's own height — the
/// existing contract of [`TextMeasure::shape`]. Allocates nothing: one
/// running maximum.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LineCount {
    lines: u32,
}

impl LineCount {
    pub const fn new() -> Self {
        LineCount { lines: 0 }
    }

    /// The highest line any cluster landed on, plus one; 0 when no
    /// cluster was reported.
    pub const fn lines(&self) -> u32 {
        self.lines
    }
}

impl GlyphSink for LineCount {
    fn cluster(&mut self, c: Cluster) {
        let n = c.line as u32 + 1;
        if n > self.lines {
            self.lines = n;
        }
    }
}

/// The distance between lines at `line_height` sixteenths of `px` —
/// [`crate::Style::line_height`]'s pitch. 0 for `line_height` 0, which
/// means "the measurer's own spacing", not a pitch.
#[inline]
pub fn pitch(line_height: u8, px: f32) -> f32 {
    line_height as f32 / 16.0 * px
}

/// The height of `lines` lines at a pitch — M80/T80.2: `lines × pitch +
/// max(0, nls − pitch)`, where `nls` is the face's own new-line size.
///
/// At or above the face's spacing that is exactly `lines × pitch`, CSS's
/// and iced's figure. Below it the box grows by the deficit, because the
/// renderer shifts every line down by half of it so line 0's ascenders
/// and the last line's descenders stay inside the run's rect
/// (`FontStack::place_run_into` in `fulgurite-shell`, which this must
/// agree with: nothing clips a run, and `Recording` damages its rect).
#[inline]
pub fn pitched_height(lines: u32, line_height: u8, px: f32, nls: f32) -> f32 {
    let p = pitch(line_height, px);
    lines as f32 * p + (nls - p).max(0.0)
}

/// `text` measured as a node with [`crate::Style::line_height`] set lays
/// it out — M80/T80.2: shaped into a [`LineCount`], and `lines × pitch`
/// tall ([`pitched_height`]) where the measurer reported clusters.
///
/// `nls` is the measurer's [`TextMeasure::line_height`] at the display
/// scale, rounded up in physical pixels as fontdue rounds its new-line
/// size, so this box and the renderer's agree at every scale. A measurer
/// that reports no clusters keeps its own height. `line_height` 0 is the
/// plain [`TextMeasure::measure_weighted`], byte for byte.
pub fn measure_spaced(
    text: &mut dyn TextMeasure,
    scale: f32,
    s: &str,
    px: f32,
    weight: f32,
    line_height: u8,
    max_w: f32,
) -> Size {
    if line_height == 0 {
        return text.measure_weighted(s, px, weight, max_w);
    }
    let mut count = LineCount::new();
    let run = text.shape(s, px, weight, max_w, &mut count);
    if count.lines() == 0 {
        return run;
    }
    let k = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    let nls = crate::num::ceil(text.line_height(px) * k) / k;
    Size { w: run.w, h: pitched_height(count.lines(), line_height, px, nls) }
}

/// Measures a run of text. Implemented by whatever shaping stack the
/// host composes in — `fulgurite-shell`'s `FontMetrics` (fontdue)
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

    /// Write `text`'s glyph outlines at `size` px and `weight` to `sink`,
    /// laid out as [`measure_weighted`](Self::measure_weighted) lays it out
    /// on one line, and answer whether it did — M82/T82.7
    /// (`docs/plan-paint-vocabulary.md`, "Vector text is outlines from the
    /// measurer").
    ///
    /// The outline is in the run's frame: logical px, the run's top-left
    /// at the origin, y down — the frame a painter places the run's
    /// glyphs in, so an outline painted at the run's rect lands where the
    /// glyphs would. Each glyph sits at the pen the measurer advances by,
    /// so the outline's advances are the measured width's. This is what a
    /// rotated run is drawn from, since a glyph bitmap cannot take a
    /// rotation.
    ///
    /// **The default answers `false` and writes nothing**: a measurer with
    /// no outlines — a baked bitmap face, a block stand-in — says so, and
    /// the caller draws the string as ordinary text instead. A real
    /// measurer overrides it: `fulgurite-shell`'s `FontMetrics` under
    /// its `vector-text` feature, and `fulgurite-text`'s `ComplexText`.
    ///
    /// **A wrapper must forward this**, like [`generation`](Self::generation):
    /// the provided `false` is right for a leaf and wrong for a wrapper,
    /// which would report every wrapped stack as having no outlines. None
    /// of the measurers in this workspace wraps another.
    fn outline(&mut self, text: &str, size: f32, weight: f32, sink: &mut dyn PathSink) -> bool {
        let _ = (text, size, weight, sink);
        false
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
    /// `text.generation()`, read once when the context is built. M45's
    /// memo key reads it for every text node, and through
    /// `&mut dyn TextMeasure` that was a virtual call in the hot layout
    /// loop (`docs/results-text.md`, "The memo key's generation"). A
    /// measurer's generation changes only through `&mut self` on the
    /// concrete type, which the borrow here excludes for the whole pass;
    /// code that replaces `text` must build a new context.
    generation: u32,
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
    /// The window, in logical px — M57/A18 (`docs/plan-widget-fixes.md`).
    ///
    /// What an out-of-flow popup must stay inside. `layout` is handed only
    /// its parent's limits, which for a dropdown in a form is the form, not
    /// the screen; so a tooltip near the right edge ran off it and a long
    /// option list ran off the bottom (`docs/widget-audit.md`, Tooltip,
    /// FilePicker). Infinite unless the caller knows the window: the host
    /// and [`crate::pass::layout_with`] set it from the root's limits.
    pub viewport: crate::desc::Size,
}

impl<'a> LayoutCx<'a> {
    pub fn new(text: &'a mut dyn TextMeasure) -> Self {
        let generation = text.generation();
        LayoutCx {
            text,
            generation,
            fill_span: None,
            scale: 1.0,
            changed: crate::invalidate::Changed::ALL,
            palette: &crate::style::Palette::LIGHT,
            viewport: crate::desc::Size { w: f32::INFINITY, h: f32::INFINITY },
        }
    }

    /// The same context, knowing the window is `viewport` (M57/A18).
    pub fn with_viewport(mut self, viewport: crate::desc::Size) -> Self {
        self.viewport = viewport;
        self
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

    /// The measurer's generation, as read when this context was built.
    pub fn generation(&self) -> u32 {
        self.generation
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
        let generation = text.generation();
        LayoutCx {
            text,
            generation,
            fill_span: None,
            scale: 1.0,
            changed,
            palette: &crate::style::Palette::LIGHT,
            viewport: crate::desc::Size { w: f32::INFINITY, h: f32::INFINITY },
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
