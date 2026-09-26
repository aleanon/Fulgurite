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
#[derive(Default)]
pub struct Label<S: TextStorage = &'static str> {
    pub text: Dirty<S>,
    measure: Measured,
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
        Label { text: Dirty::new(text), measure: Measured::new() }
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

    /// Whether the memo would serve a layout under `limits` and `style`
    /// without re-measuring.
    ///
    /// Takes the style since M26/T26.3, and not for symmetry: a memo that
    /// is warm at 16 px is cold at 32, so "is it warm" has no answer
    /// without the style being asked about. A version that assumed the
    /// default would have been the defect again, in the test API.
    pub fn measure_is_warm(&self, limits: Constraints, style: &Style, scale: f32) -> bool {
        !self.text.is_dirty() && self.measure.is_warm(MeasureKey::of(limits, style, scale))
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
        ])).with_roles(lumen_static::style::SeedRoles::NONE.color(lumen_static::style::Role::Text))];
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
        let text = self.text.get().as_ref();
        let dirty = self.text.is_dirty();
        let px = style[i].font_size;
        let key = MeasureKey::of(limits, &style[i], cx.scale());
        let content = self.measure.get_or(key, dirty, || {
            cx.text.measure_weighted(text, px, style[i].font_weight, limits.max_w)
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
        let content =
            cx.text.measure_weighted(text, style[i].font_size, style[i].font_weight, limits.max_w);
        let pad = style[i].padding;
        let boxed = Size { w: content.w + pad.horizontal(), h: content.h + pad.vertical() };
        record_painted(st, i, limits, resolve_self(limits, &style[i], boxed), &style[i])
    }
    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let r = rect_of(&st[i]);
        out.rounded_rect(r, st[i].bg, st[i].radius);
        border(st, i, out);
        // Text sits inside the padding box; `layout` sized the node to
        // include it.
        let p = st[i].pad;
        out.text_weighted(
            [r[0] + p.0, r[1] + p.1, r[2] - p.2, r[3] - p.3],
            self.text.get().as_ref(),
            st[i].font_size,
            st[i].font_weight,
            st[i].fg,
        );
    }
    lumen_static::leaf_common!(Label<S>, 0);

    #[cfg(feature = "observe")]
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
    /// Keyed on a [`MeasureKey`] and a fold of the label's bytes. The
    /// fold stands in for the dirty bit a `Label` has and a `Button` does
    /// not. This was a private `ButtonKey` holding the same three fields
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
                width: lumen_static::style::Length::FILL,
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
        let key = MeasureKey::of(limits, &style[i], cx.scale());
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

    #[cfg(feature = "observe")]
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
