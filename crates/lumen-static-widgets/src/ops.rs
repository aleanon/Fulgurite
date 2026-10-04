//! The widget vocabulary — `Label`, `Fixed`, `Spacer`, `Button`.
//!
//! These lived in `crate::ops` until M30, alongside the layout
//! helpers the derive calls. That module's rustdoc called the pairing a
//! wart and named the fix — *"splitting the two into a widgets module
//! and a hidden private one is the honest end state
//! (`docs/plan-review-fixes.md` F6.1)"* — and deferred it for churn.
//! Moving every widget into this crate made the split compulsory, so
//! F6.1 is closed here: the machinery stayed in `crate::ops`,
//! and this is the vocabulary.
//!
//! The name is kept rather than improved. `ops` is where a reader who
//! knows this codebase will look for `Label`, and a rename would cost
//! every `use` line in the workspace a second edit for no gain that a
//! re-export could not give.

use lumen_static::cx::LayoutCx;
use lumen_static::desc::{Constraints, NodeDesc, NodeState, Size, NO_PARENT};
use lumen_static::display::{Color, Painter};
use lumen_static::event::{EventCtx, Handlers, NodeSel};
use lumen_static::ops::{border, record_painted, rect_of, resolve_self};
use lumen_static::style::{Length, Seed, Style};
use lumen_static::text::{Dirty, MeasureKey, Measured, TextStorage};
use lumen_static::widget::{Node, Widget};

/// A run of text.
///
/// Generic over storage so each profile can choose (`docs/
/// layout-and-storage-decisions.md` §1); defaults to `&'static str` so
/// the `no_std` profile is the path of least resistance. `text` carries
/// a dirty bit and `measure` memoizes, because shaping is the expensive
/// part of a real frame — the reason text lives here rather than in a
/// sibling field reached by a getter.
///
/// **The text is a presentation copy.** The application owns the string
/// and `#[bind]` writes it here through `TextTarget`; the widget never
/// changes it on its own. `TextInput` is the opposite case, for the same
/// rule: there the text originates at the widget, because the user typed
/// it.
///
/// **It reads `Style::line_height`** (M80/T80.2): at 0 it measures and
/// paints at the face's own spacing, as it always did; set, it measures
/// `lines × pitch` (`lumen_static::cx::measure_spaced`) and hands the byte
/// to `Painter::text_spaced`, so the renderer places the lines on the same
/// pitch the box was measured at.
///
/// **It reads `Style::overflow`** (M80/T80.3): under `Clip` or `Ellipsis`
/// it is one line, cut in layout at a grapheme boundary by
/// [`crate::fit::cut`], and paints the prefix (then `…`; before it, at the
/// left, when the measurer lays the prefix out right to left — M81/T81.3,
/// [`crate::fit::rtl`]). The cut is made
/// inside the memo's miss, so a still frame cuts nothing, and against the
/// width the label will **occupy** — its own resolved width less its
/// padding, not the width its parent offered (a `Fixed(90)` label in a
/// 200 px column measured at 200 before M80/T80.3, and painted past its
/// box). `observe` still publishes the whole text: a reader is not short
/// of room. **Without an allocator it reads every mode as `Wrap`** (K80b's
/// fallback; see `measure_label`).
pub struct Label<S: TextStorage = &'static str> {
    pub text: Dirty<S>,
    measure: Measured,
    /// The cut, from layout — M80/T80.3: the prefix's end byte
    /// (`u32::MAX` = paint the whole text, as before `overflow` existed)
    /// and where the `…` starts from the content box's left edge, which
    /// is negative under `Clip` (no mark), and at or below `RTL_MARK`
    /// for a right-to-left cut whose mark sits at the left (M81/T81.3,
    /// `cut_places`). **+8 B per `Label`** where
    /// there is an allocator; **absent in the leaf tier**, which reads
    /// every `TextOverflow` as `Wrap` (K80b's fallback,
    /// `docs/results-embedded.md` M80/T80.3).
    #[cfg(feature = "alloc")]
    fit: (u32, f32),
}

impl<S: TextStorage + Default> Default for Label<S> {
    fn default() -> Self {
        Label::new(S::default())
    }
}

/// Whether `Label` reads [`Style::overflow`]: where there is an
/// allocator. The leaf tier reads every mode as `Wrap` (K80b's fallback),
/// so its seed does not declare the bit and the sheet lint reports an
/// `overflow` rule there as the no-op it is.
const LABEL_OVERFLOW: lumen_static::style::Fields =
    if cfg!(feature = "alloc") { lumen_static::style::Fields::OVERFLOW } else { lumen_static::style::Fields::NONE };

/// `Label::fit` when nothing is cut.
const UNCUT: (u32, f32) = (u32::MAX, -1.0);

/// `Label::fit`'s mark offset at and below which the cut is right to left
/// (M81/T81.3): the `…` sits at the content box's left edge and the prefix
/// starts `RTL_MARK - offset` px in, the mark's width. `-1.0` is `Clip`'s
/// no-mark, and a left-to-right mark is never negative.
const RTL_MARK: f32 = -2.0;

/// Where a cut's prefix and `…` start from the content box's left edge,
/// from `Label::fit`'s mark offset: `(prefix, Some(mark))`, or no mark
/// under `Clip`.
#[inline]
fn cut_places(mark: f32) -> (f32, Option<f32>) {
    if mark <= RTL_MARK {
        (RTL_MARK - mark, Some(0.0))
    } else if mark >= 0.0 {
        (0.0, Some(mark))
    } else {
        (0.0, None)
    }
}

/// The width a label's text is measured and cut against — M80/T80.3: the
/// width the node will occupy (its `width` resolved under `limits`, which
/// for `Shrink` is the offer and for `Fixed` its own) less its padding.
/// Until then `Label` measured against the offer and ignored its padding,
/// so a fixed-width label wrapped as wide as its parent and painted past
/// its own box (`docs/plan-text-fit.md`, "What building found", T80.2).
///
/// **The width half of `resolve_self`, written out**: only `Fixed` differs
/// from the offer (`Shrink` and `Fill` resolve to it, a hidden label lays
/// out to nothing whatever it measured), and this runs on every layout,
/// memo hit included. Calling `resolve_self` here cost the
/// "layout, memo warm" arm of `--bin bindings` about 1.1 ns of a 7.8 ns hit,
/// outside the parent's spread (K80c, `docs/results-text.md` M80/T80.3).
///
/// `pub(crate)` for `Selectable` (M80/T80.4), which shapes its clusters
/// against the same room so a cluster's line is the painted run's.
#[inline]
pub(crate) fn text_room(limits: Constraints, style: &Style) -> f32 {
    let own = match style.width {
        Length::Fixed(v) => lumen_static::num::clamp(v, limits.min_w, limits.max_w),
        _ => limits.max_w,
    };
    (own - style.padding.left - style.padding.right).max(0.0)
}

/// The limits the memo keys on: the offer, with its width replaced by the
/// room the text gets, so a padding or width restyle under the same offer
/// re-measures.
fn text_limits(limits: Constraints, style: &Style) -> Constraints {
    Constraints { max_w: text_room(limits, style), ..limits }
}

/// The label's content box and its cut, measured — one function so
/// `layout` and `probe` cannot measure different labels.
///
/// **The leaf tier reads every `TextOverflow` as `Wrap`** — K80b's own
/// fallback (`docs/plan-text-fit.md`): the cut cost the paint-only
/// firmware about 2 KB of `.text` against M80a's 1,024 B, so with no
/// allocator a `Clip` or `Ellipsis` label wraps as it did before the field
/// existed (`docs/results-embedded.md` M80/T80.3).
fn measure_label(cx: &mut LayoutCx, text: &str, s: &Style, room: f32) -> (Size, (u32, f32)) {
    let scale = cx.scale();
    let (px, wt, lh) = (s.font_size, s.font_weight, s.line_height);
    #[cfg(not(feature = "alloc"))]
    {
        (lumen_static::cx::measure_spaced(cx.text, scale, text, px, wt, lh, room), UNCUT)
    }
    #[cfg(feature = "alloc")]
    measure_cut(cx, text, s, room, scale, px, wt, lh)
}

/// [`measure_label`] where there is an allocator: `Wrap`, or the cut.
#[cfg(feature = "alloc")]
#[allow(clippy::too_many_arguments)]
fn measure_cut(cx: &mut LayoutCx, text: &str, s: &Style, room: f32, scale: f32, px: f32, wt: f32, lh: u8) -> (Size, (u32, f32)) {
    use lumen_static::style::TextOverflow;
    let mark = match s.overflow {
        TextOverflow::Wrap => {
            return (lumen_static::cx::measure_spaced(cx.text, scale, text, px, wt, lh, room), UNCUT);
        }
        TextOverflow::Clip => false,
        TextOverflow::Ellipsis => true,
    };
    match crate::fit::cut(cx.text, text, px, wt, room, mark) {
        // One line that fits: measured exactly as `Wrap` measures it, so an
        // untruncated `Ellipsis` label paints what a `Wrap` one does.
        None => (lumen_static::cx::measure_spaced(cx.text, scale, text, px, wt, lh, room), UNCUT),
        Some((end, w)) => {
            // One line tall: the prefix's height and the mark's, whichever
            // inks taller, at the pitch.
            let prefix = text.get(..end).unwrap_or("");
            let line = lumen_static::cx::measure_spaced(cx.text, scale, prefix, px, wt, lh, f32::INFINITY);
            let e = lumen_static::cx::measure_spaced(cx.text, scale, crate::fit::ELLIPSIS, px, wt, lh, f32::INFINITY);
            let h = line.h.max(e.h);
            if mark {
                // The mark at the paragraph direction's end (M81/T81.3):
                // left of a right-to-left prefix, which then starts after
                // it — encoded as `RTL_MARK - e.w` (see `cut_places`).
                let rtl = !prefix.is_empty() && crate::fit::rtl(cx.text, prefix, px, wt);
                (Size { w: w + e.w, h }, (end as u32, if rtl { RTL_MARK - e.w } else { w }))
            } else {
                (Size { w, h }, (end as u32, -1.0))
            }
        }
    }
}

impl<S: lumen_static::text::TextBuffer> lumen_static::bind::TextTarget for Label<S> {
    /// The binding seam — M30. `#[bind]` writes through this rather
    /// than through the inherent method, because core cannot name this
    /// type since the widgets moved out.
    fn set_str(&mut self, text: &str) -> bool {
        Label::set_str(self, text)
    }
}

impl<S: TextStorage> Label<S> {
    pub fn new(text: S) -> Self {
        Label {
            text: Dirty::new(text),
            measure: Measured::new(),
            #[cfg(feature = "alloc")]
            fit: UNCUT,
        }
    }

    /// Replace the text, marking it for re-measurement.
    pub fn set(&mut self, text: S) {
        self.text.set(text);
    }

    /// Replace the text from a borrowed string, reusing the storage's
    /// allocation where it can. **Compares first**: identical text does
    /// not touch the dirty bit, so the measurement memo survives.
    /// Returns whether the text changed.
    ///
    /// The compare used to live only in `lumen_static::bind`, which meant the
    /// obvious way to write a repeater's bind function —
    /// `row.label.set_str(..)` — dirtied every visible row on every
    /// layout and defeated the memo entirely: 62 needless text
    /// measurements per *idle* frame for a 31-row window. Nothing in
    /// `fn(&mut T, usize)` steered an author away from it. Comparing
    /// here is the only sane default for a memoized field
    /// (`docs/plan-review-fixes.md` F4.6).
    ///
    /// Requires [`TextBuffer`](lumen_static::text::TextBuffer), so
    /// `Label<&'static str>` does not have this method: that storage can
    /// only accept a new value by leaking.
    pub fn set_str(&mut self, text: &str) -> bool
    where
        S: lumen_static::text::TextBuffer,
    {
        if self.text.get().as_ref() == text {
            return false;
        }
        self.text.get_mut().set_from_str(text);
        true
    }

    pub fn as_str(&self) -> &str {
        self.text.get().as_ref()
    }

    /// The part of the text the last layout painted, and whether it was
    /// cut — M80/T80.3, the same answer `AppBar::shown_title` gives. The
    /// whole text and `false` under `Wrap`, or when it fits.
    pub fn shown(&self) -> (&str, bool) {
        let t = self.text.get().as_ref();
        let end = self.fit().0;
        if end == u32::MAX {
            return (t, false);
        }
        (t.get(..end as usize).unwrap_or(""), true)
    }

    /// The label's ground: its background and border — the first half of
    /// [`Node::paint`], split out (M80/T80.4) so `Selectable` can paint its
    /// selection tint between the ground and the run. **`pub(crate)` and
    /// inlined**: the split must compile to the same leaf code, which
    /// `just firmware-delta` holds at 0 B on all four binaries.
    #[inline]
    pub(crate) fn paint_ground(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let r = rect_of(&st[i]);
        out.rounded_rect(r, st[i].bg, st[i].radius);
        border(st, i, out);
    }

    /// The label's run, inside its padding box — the second half of
    /// [`Node::paint`] (see [`Label::paint_ground`]).
    #[inline]
    pub(crate) fn paint_run(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let r = rect_of(&st[i]);
        // Text sits inside the padding box; `layout` sized the node to
        // include it. At the pitch `record_painted` stamped (M80/T80.2):
        // the renderer places the lines, so the box `layout` measured and
        // the lines painted come from one rule.
        //
        // Cut (M80/T80.3): the prefix layout kept, then the `…` where
        // layout measured the prefix to end — two ordinary runs, as
        // `AppBar` paints its title, so each is damaged by its own rect
        // and neither reaches past the node.
        let p = st[i].pad;
        let content = [r[0] + p.0, r[1] + p.1, r[2] - p.2, r[3] - p.3];
        let t = self.text.get().as_ref();
        let (px, wt, lh, fg) = (st[i].font_size, st[i].font_weight, st[i].line_height, st[i].fg);
        let (end, mark) = self.fit();
        if end == u32::MAX {
            out.text_spaced(content, t, px, wt, lh, fg);
            return;
        }
        // `get`, not an index: a reload restores the text without the dirty
        // bit, and a cut from the old text must not panic before the next
        // layout recomputes it.
        let (at, mark) = cut_places(mark);
        out.text_spaced([content[0] + at, content[1], content[2], content[3]], t.get(..end as usize).unwrap_or(""), px, wt, lh, fg);
        if let Some(m) = mark {
            out.text_spaced([content[0] + m, content[1], content[2], content[3]], crate::fit::ELLIPSIS, px, wt, lh, fg);
        }
    }

    /// The cut, or [`UNCUT`] in the leaf tier, which never cuts.
    #[inline]
    fn fit(&self) -> (u32, f32) {
        #[cfg(feature = "alloc")]
        {
            self.fit
        }
        #[cfg(not(feature = "alloc"))]
        {
            UNCUT
        }
    }

    /// Whether the memo would serve a layout under `limits` and `style`
    /// without re-measuring.
    ///
    /// Takes the style since M26/T26.3, and not for symmetry: a memo that
    /// is warm at 16 px is cold at 32, so "is it warm" has no answer
    /// without the style being asked about. A version that assumed the
    /// default would have been the defect again, in the test API.
    ///
    /// And the measurer since M45/T45.2: pass the generation of the one
    /// the frame measured with (`TextMeasure::generation`, 0 for a
    /// stand-in).
    pub fn measure_is_warm(&self, limits: Constraints, style: &Style, scale: f32, generation: u32) -> bool {
        !self.text.is_dirty()
            && self.measure.is_warm(MeasureKey::of(text_limits(limits, style), style, scale, generation))
    }
}

impl<S: TextStorage> Widget for Label<S> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];

    const KIND: &'static str = "label";

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, lumen_static::style::Fields::of(&[
            lumen_static::style::Fields::WIDTH,
            lumen_static::style::Fields::HEIGHT,
            lumen_static::style::Fields::PADDING,
            lumen_static::style::Fields::CURSOR,
            lumen_static::style::Fields::VISIBLE,
            lumen_static::style::Fields::COLOR,
            lumen_static::style::Fields::BACKGROUND,
            lumen_static::style::Fields::BORDER,
            lumen_static::style::Fields::RADIUS,
            lumen_static::style::Fields::FONT_SIZE,
            lumen_static::style::Fields::FONT_WEIGHT,
            lumen_static::style::Fields::PRESSED_DIM,
            lumen_static::style::Fields::HOVER_DIM,
            lumen_static::style::Fields::LINE_HEIGHT,
        ]).union(LABEL_OVERFLOW)).with_roles(lumen_static::style::SeedRoles::NONE.color(lumen_static::style::Role::Text))];
}

impl<S: TextStorage> Node for Label<S> {
    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        // The memo is the whole reason text is owned here: with real
        // shaping (M5) this call is the expensive one, and it must not
        // run for text that has not changed under limits that have not
        // changed.
        //
        // The key holds the size and weight as well as the limits
        // (M26/T26.3). Until then only the limits were in it, and the size
        // and weight were read in the closure below — so a restyle that
        // changed either, under the same limits, was served the old box.
        //
        // **At a line height, `lines × pitch`** — M80/T80.2. The key holds
        // the byte (`MeasureKey`'s `flow`), and the measurement shapes into
        // a line counter rather than taking the face's own spacing; 0 is
        // `measure_weighted` exactly, so a label that sets nothing measures
        // as it always did.
        //
        // **And the cut** — M80/T80.3: under `Clip`/`Ellipsis` the miss
        // also decides where the line ends, so it is keyed exactly as the
        // box is (the overflow byte is in `flow`) and a still frame cuts
        // nothing. Both are measured against the width the label occupies,
        // not the width it was offered (`text_room`).
        let text = self.text.get().as_ref();
        let dirty = self.text.is_dirty();
        let tl = text_limits(limits, &style[i]);
        let key = MeasureKey::new(cx, tl, &style[i]);
        #[cfg(feature = "alloc")]
        let fit = &mut self.fit;
        let content = self.measure.get_or(key, dirty, || {
            let (size, _cut) = measure_label(cx, text, &style[i], tl.max_w);
            #[cfg(feature = "alloc")]
            {
                *fit = _cut;
            }
            size
        });
        self.text.clear_dirty();
        let pad = style[i].padding;
        let boxed = Size { w: content.w + pad.horizontal(), h: content.h + pad.vertical() };
        record_painted(st, i, limits, resolve_self(limits, &style[i], boxed), &style[i])
    }

    /// Measure without touching the memo — see [`Node::probe`]. The
    /// dirty bit is left alone too: a probe is a question, not a frame.
    #[cfg(feature = "observe")]
    fn probe(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        // The same measurement `layout` makes — size, weight, padding —
        // or the audit compares the frame against a different label.
        // Until 2026-09-03 this measured at 16 px regular with no
        // padding, so a probe of the tally's 76 px value asked for a
        // box a fifth its size.
        let text = self.text.get().as_ref();
        let (content, _cut) = measure_label(cx, text, &style[i], text_room(limits, &style[i]));
        let pad = style[i].padding;
        let boxed = Size { w: content.w + pad.horizontal(), h: content.h + pad.vertical() };
        record_painted(st, i, limits, resolve_self(limits, &style[i], boxed), &style[i])
    }
    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        self.paint_ground(st, i, out);
        self.paint_run(st, i, out);
    }
    lumen_static::leaf_common!(Label<S>, 0);

    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::Label);
        node.set_value(self.text.get().as_ref());
    }
}

/// Fixed-size box: the rigid leaf (§2's relayout boundary, const-known).
pub struct Fixed {
    pub w: f32,
    pub h: f32,
}

impl Default for Fixed {
    fn default() -> Self {
        Fixed { w: 24.0, h: 24.0 }
    }
}

impl Widget for Fixed {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::RIGID)];

    const KIND: &'static str = "fixed";

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, lumen_static::style::Fields::of(&[
            lumen_static::style::Fields::WIDTH,
            lumen_static::style::Fields::HEIGHT,
            lumen_static::style::Fields::CURSOR,
            lumen_static::style::Fields::VISIBLE,
            lumen_static::style::Fields::COLOR,
        ]))];
}

impl Node for Fixed {
    fn layout(
        &mut self,
        _cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let content = Size { w: self.w, h: self.h };
        record_painted(st, i, limits, resolve_self(limits, &style[i], content), &style[i])
    }
    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        out.rect(rect_of(&st[i]), st[i].fg);
    }
    lumen_static::leaf_common!(Fixed, NodeDesc::RIGID);

    /// **Hidden from a reader on purpose** — M57/D7. It is layout, not
    /// content; published as a nameless `GenericContainer` it read as a
    /// control that says nothing, and the live accessibility sweep
    /// flagged every filler in the showcase's grid rows.
    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut alloc::vec::Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        node.set_hidden();
    }
}

/// Fills whatever its parent hands it.
#[derive(Default)]
pub struct Spacer;

impl Widget for Spacer {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];

    const KIND: &'static str = "spacer";

    /// The one leaf that is filler rather than content.
    fn type_defaults() -> Style {
        Style { width: Length::FILL, height: Length::FILL, ..Style::default() }
    }

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, lumen_static::style::Fields::of(&[
            lumen_static::style::Fields::WIDTH,
            lumen_static::style::Fields::HEIGHT,
            lumen_static::style::Fields::VISIBLE,
        ]))];
}

impl Node for Spacer {
    fn layout(
        &mut self,
        _cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        // A spacer is filler by default; its style says so, and
        // `resolve_self` turns that into "take the whole limit".
        record_painted(st, i, limits, resolve_self(limits, &style[i], Size::default()), &style[i])
    }
    fn paint(&self, _st: &[NodeState], _i: usize, _out: &mut dyn Painter) {
        // A spacer is empty space; it draws nothing.
    }

    // `nohit`: a spacer is not a click target.
    // Hit testing follows painting — you cannot click what was never
    // drawn. A row or a column hides the difference, because its
    // children cannot overlap and claiming a transparent gap only ever
    // returns the parent. A `stack` does not: a `Spacer` at `FILL` in a
    // layer above the content is invisible, covers the whole window,
    // and is hit *first* because a stack walks its children in reverse.
    // It swallowed every click and every wheel event in
    // `examples/glass` (found 2026-09-11).
    lumen_static::leaf_common!(Spacer, 0, nohit);

    /// **Hidden from a reader on purpose** — M57/D7. It is layout, not
    /// content; published as a nameless `GenericContainer` it read as a
    /// control that says nothing, and the live accessibility sweep
    /// flagged every filler in the showcase's grid rows.
    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut alloc::vec::Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        node.set_hidden();
    }
}

/// A clickable leaf. `on_click` is a plain `fn` pointer, never a closure:
/// it cannot capture, so `fresh()` re-supplies it across a hot reload
/// (`docs/events-decision.md`). Payload a closure would have captured
/// arrives in [`EventCtx`] — notably `ctx.row` inside a repeater.
pub struct Button<S: 'static, T: TextStorage = &'static str, const ROW: bool = false> {
    pub label: T,
    pub on_click: fn(&mut S, &EventCtx),
    /// The label's measured box and what it was measured under. `label`
    /// is a plain field, not a `Dirty<_>`, so the key carries a content
    /// hash instead of a dirty bit — a few bytes per frame against a
    /// shaping call that allocates. Until 2026-09-03 a button never
    /// measured at all: width was `0.62 em × chars`, the text box was
    /// `font_size` tall, and every label sat 4 px low
    /// (`docs/results-frame-cost.md`).
    ///
    /// Keyed on a [`MeasureKey`] and a hash of the label's bytes
    /// (`lumen_static::display::hash_bytes`; until M46/T46.1 a linear
    /// fold, on which `"x"` and `"\0x"` shared a width). The hash stands
    /// in for the dirty bit a `Label` has and a `Button` does not. This was a private `ButtonKey` holding the same three fields
    /// the label's memo left out, so the right key and the wrong one sat
    /// in the same file until M26/T26.3 gave both widgets the one type.
    measured: lumen_static::text::FoldedText,
}

pub fn ignore_click<S>(_: &mut S, _: &EventCtx) {}

impl<S, T: TextStorage, const ROW: bool> Default for Button<S, T, ROW> {
    fn default() -> Self {
        Button {
            label: T::default(),
            on_click: ignore_click::<S>,
            measured: lumen_static::text::FoldedText::new(),
        }
    }
}

impl<S: 'static, T: TextStorage, const ROW: bool> Button<S, T, ROW> {}

impl<S: 'static, T: TextStorage, const ROW: bool> Widget for Button<S, T, ROW> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::FOCUSABLE)];

    /// `ROW` is a **menu row** — `Select`'s — M57/C1. The same control,
    /// with its own kind and look: full width, flat, a hover tint over the
    /// menu's own colour. The showcase had to style menu rows with a
    /// `Kind("button")` rule, which also reached every button it did not
    /// name field by field — and set `hover_dim(0.0)` on all of them, so
    /// no demo button showed hover (`docs/widget-audit.md`, Button). A row
    /// with its own kind is styled by its widget, and a sheet that wants
    /// to restyle rows says `Kind("selectrow")`.
    const KIND: &'static str = if ROW { "selectrow" } else { "button" };

    fn type_defaults() -> Style {
        if ROW {
            return Style {
                // The whole row: the repeater's row extent is the height.
                width: lumen_static::style::Length::FILL,
                height: lumen_static::style::Length::FILL,
                padding: lumen_static::style::Edges { left: 14.0, right: 10.0, top: 6.0, bottom: 6.0 },
                pressed_dim: 0.18,
                hover_dim: 0.08,
                cursor: lumen_static::style::Cursor::Pointer,
                background: Color(255, 255, 255, 255),
                color: Color(15, 23, 42, 255),
                font_size: 14.0,
                ..Style::default()
            };
        }
        Style {
            padding: lumen_static::style::Edges { left: 14.0, right: 14.0, top: 9.0, bottom: 9.0 },
            // A control that does not change while held reads as broken,
            // and it is the one GUI-theory check `scripts/ui_check.py`
            // could not make before C6.
            pressed_dim: 0.18,
            // 0.12, not 0.07 (M57/B10): at least 1.2:1 against the rest
            // shade on the lightest body and on the accent, where 0.07
            // was a hover nobody could see.
            hover_dim: 0.12,
            // A hand over something clickable (T14.6).
            cursor: lumen_static::style::Cursor::Pointer,
            background: Color(226, 232, 240, 255),
            color: Color(15, 23, 42, 255),
            radius: 8.0,
            ..Style::default()
        }
    }

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, lumen_static::style::Fields::of(&[
            lumen_static::style::Fields::WIDTH,
            lumen_static::style::Fields::HEIGHT,
            lumen_static::style::Fields::PADDING,
            lumen_static::style::Fields::CURSOR,
            lumen_static::style::Fields::FOCUS_RING,
            lumen_static::style::Fields::VISIBLE,
            lumen_static::style::Fields::COLOR,
            lumen_static::style::Fields::BACKGROUND,
            lumen_static::style::Fields::BORDER,
            lumen_static::style::Fields::RADIUS,
            lumen_static::style::Fields::FONT_SIZE,
            lumen_static::style::Fields::FONT_WEIGHT,
            lumen_static::style::Fields::PRESSED_DIM,
            lumen_static::style::Fields::HOVER_DIM,
        ]))
        // A row sits on its menu's raised panel; a button on the page.
        .with_roles(lumen_static::style::SeedRoles::NONE.color(lumen_static::style::Role::Text).background(
            if ROW { lumen_static::style::Role::Raised } else { lumen_static::style::Role::Surface },
        ))];
}

impl<S: 'static, T: TextStorage, const ROW: bool> Node for Button<S, T, ROW> {
    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let label = self.label.as_ref();
        let px = style[i].font_size;
        let weight = style[i].font_weight;
        let key = MeasureKey::new(cx, limits, &style[i]);
        // The fold stands in for the dirty bit a `Label` has and a
        // `Button` does not — in `FoldedText` since M26/T26.8, because
        // `Badge` and `Chip` needed the same thing and had none.
        let text = self.measured.get_or(key, label, || {
            cx.text.measure_weighted(label, px, weight, limits.max_w)
        });
        let pad = style[i].padding;
        let content = Size { w: text.w + pad.horizontal(), h: text.h + pad.vertical() };
        record_painted(st, i, limits, resolve_self(limits, &style[i], content), &style[i])
    }
    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let r = rect_of(&st[i]);
        out.rounded_rect(r, st[i].bg, st[i].radius);
        border(st, i, out);
        // Centre the *measured* line box in the button. The line box,
        // not the ink: that is what every toolkit centres, and it puts
        // capitals a hair below centre in faces with a deep descent
        // (Noto Sans: 0.03 em) — the same hair a browser leaves.
        let text = self.measured.size();
        let x = r[0] + ((r[2] - r[0]) - text.w).max(0.0) / 2.0;
        let y = r[1] + ((r[3] - r[1]) - text.h).max(0.0) / 2.0;
        out.text_weighted(
            [x, y, (x + text.w).max(r[2]), y + text.h],
            self.label.as_ref(),
            st[i].font_size,
            st[i].font_weight,
            st[i].fg,
        );
    }
    /// Measure without touching the memo — see [`Node::probe`].
    #[cfg(feature = "observe")]
    fn probe(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let text = cx.text.measure_weighted(
            self.label.as_ref(),
            style[i].font_size,
            style[i].font_weight,
            limits.max_w,
        );
        let pad = style[i].padding;
        let content = Size { w: text.w + pad.horizontal(), h: text.h + pad.vertical() };
        record_painted(st, i, limits, resolve_self(limits, &style[i], content), &style[i])
    }
    lumen_static::leaf_common!(Button<S, T, ROW>, NodeDesc::FOCUSABLE);

    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::Button);
        node.set_label(self.label.as_ref());
        node.add_action(accesskit::Action::Click);
    }
}

macro_rules! no_handlers {
    (Label<S2>) => {
        impl<S, S2: TextStorage> Handlers<S> for Label<S2> {
            fn handler(&self, _sel: NodeSel) -> Option<fn(&mut S, &EventCtx)> {
                None
            }
        }
    };
    ($ty:ty) => {
        impl<S> Handlers<S> for $ty {
            fn handler(&self, _sel: NodeSel) -> Option<fn(&mut S, &EventCtx)> {
                None
            }
        }
    };
}

no_handlers!(Label<S2>);
no_handlers!(Fixed);
no_handlers!(Spacer);

/// Only for its *own* state type — a `Button<A>` inside a `B`-rooted
/// tree fails to satisfy the derive's `Handlers<S>` bound, which is the
/// compile-time form of the old dispatch safety contract.
impl<S: 'static, T: TextStorage, const ROW: bool> Handlers<S> for Button<S, T, ROW> {
    fn handler(&self, _sel: NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        Some(self.on_click)
    }
}
