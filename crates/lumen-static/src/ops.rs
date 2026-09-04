//! The built-in widget vocabulary, and the layout helpers the derive
//! calls.
//!
//! Two audiences in one module, which is a wart the name records: `ops`
//! is called that because it once held the per-kind op tables M0 deleted.
//! [`Label`], [`Fixed`], [`Spacer`] and [`Button`] are the vocabulary and
//! are re-exported from [`crate::prelude`]; the rest —
//! `inner_limits`, `loose_child`, `fill_child`, `align_offset`,
//! `spaced`, `hide_subtree` — exist so derive-generated code has a
//! public path to call and are `#[doc(hidden)]`. `resolve_self`,
//! `record` and `rect_of` are visible because tests and widgets outside
//! this crate genuinely use them.
//!
//! Splitting the two into a widgets module and a hidden private one is
//! the honest end state (`docs/plan-review-fixes.md` F6.1); hiding the
//! machinery is most of the benefit for a fraction of the churn.

use crate::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use crate::event::{EventCtx, Handlers, NodeSel};
use crate::hit::{contains, Hit};
use crate::cx::LayoutCx;
use crate::display::{Color, DisplayList};
use crate::style::{Align, Length, Style};
use crate::text::{Dirty, Measured, TextStorage};
use crate::widget::{Node, Widget};



/// A node's absolute box, for emitting commands.
#[inline]
/// Zero a whole subtree's geometry and return the zero size.
///
/// What `Style::visible = false` means, for every widget that honours
/// it. Zeroing only the node itself is not enough: descendants keep the
/// previous frame's `size` and `pos`, and `paint` walks into them
/// unconditionally, so a hidden container drew its children at their old
/// boxes. `hatch::hide` did this correctly for `Option`; the derive did
/// not, and the two mechanisms disagreed
/// (`docs/plan-review-fixes.md` F1.6).
///
/// A zero-area node paints nothing a renderer can see and fails every
/// `contains` test, which is what makes this sufficient without threading
/// `&[Style]` into `paint` and `hit`.
#[doc(hidden)]
pub fn hide_subtree(st: &mut [NodeState], i: usize, limits: Constraints, count: usize) -> Size {
    for s in st.iter_mut().skip(i).take(count) {
        s.size = Size::default();
        s.constraints = limits;
    }
    Size::default()
}

pub fn rect_of(st: &NodeState) -> crate::display::Rect {
    [st.pos.x, st.pos.y, st.pos.x + st.size.w, st.pos.y + st.size.h]
}


/// Space consumed by gaps between `n` children.
#[inline]
#[doc(hidden)]
pub const fn spaced(n: usize, spacing: f32) -> f32 {
    if n > 1 { spacing * (n - 1) as f32 } else { 0.0 }
}

/// A container's own axis: which of width/height is the main one.
#[derive(Clone, Copy, PartialEq)]
pub enum Axis {
    Row,
    Column,
}

impl Axis {
    #[inline]
    pub fn main(&self, s: Size) -> f32 {
        match self {
            Axis::Row => s.w,
            Axis::Column => s.h,
        }
    }
    #[inline]
    pub fn cross(&self, s: Size) -> f32 {
        match self {
            Axis::Row => s.h,
            Axis::Column => s.w,
        }
    }
    #[inline]
    pub fn size(&self, main: f32, cross: f32) -> Size {
        match self {
            Axis::Row => Size { w: main, h: cross },
            Axis::Column => Size { w: cross, h: main },
        }
    }
    /// The child's `Length` along this container's main axis.
    #[inline]
    pub fn main_len(&self, s: &Style) -> Length {
        match self {
            Axis::Row => s.width,
            Axis::Column => s.height,
        }
    }

    /// …and along the cross axis.
    #[inline]
    pub fn cross_len(&self, s: &Style) -> Length {
        match self {
            Axis::Row => s.height,
            Axis::Column => s.width,
        }
    }
}

/// The content box: the node's limits minus its own padding.
#[inline]
#[doc(hidden)]
pub fn inner_limits(limits: Constraints, pad: crate::style::Edges) -> Constraints {
    Constraints {
        min_w: (limits.min_w - pad.horizontal()).max(0.0),
        max_w: (limits.max_w - pad.horizontal()).max(0.0),
        min_h: (limits.min_h - pad.vertical()).max(0.0),
        max_h: (limits.max_h - pad.vertical()).max(0.0),
    }
}

/// Limits for a non-`Fill` child: it may take up to whatever is left.
#[inline]
#[doc(hidden)]
pub fn loose_child(axis: Axis, inner: Constraints, remaining_main: f32, align: Align) -> Constraints {
    let cross_min = if align == Align::Fill { axis.cross_of(inner) } else { 0.0 };
    match axis {
        Axis::Row => Constraints {
            min_w: 0.0,
            max_w: remaining_main.max(0.0),
            min_h: cross_min,
            max_h: inner.max_h,
        },
        Axis::Column => Constraints {
            min_w: cross_min,
            max_w: inner.max_w,
            min_h: 0.0,
            max_h: remaining_main.max(0.0),
        },
    }
}

/// Tight main-axis limits for a `Fill` child's share.
#[inline]
#[doc(hidden)]
pub fn fill_child(axis: Axis, inner: Constraints, share: f32, align: Align) -> Constraints {
    let cross_min = if align == Align::Fill { axis.cross_of(inner) } else { 0.0 };
    match axis {
        Axis::Row => Constraints {
            min_w: share.max(0.0),
            max_w: share.max(0.0),
            min_h: cross_min,
            max_h: inner.max_h,
        },
        Axis::Column => Constraints {
            min_w: cross_min,
            max_w: inner.max_w,
            min_h: share.max(0.0),
            max_h: share.max(0.0),
        },
    }
}

impl Axis {
    #[inline]
    pub fn main_of(&self, c: Constraints) -> f32 {
        match self {
            Axis::Row => c.max_w,
            Axis::Column => c.max_h,
        }
    }

    #[inline]
    fn cross_of(&self, c: Constraints) -> f32 {
        match self {
            Axis::Row => c.max_h,
            Axis::Column => c.max_w,
        }
    }
}

/// Resolve a container's own size from its content, honouring its own
/// `Length`s: `Fill` takes the whole limit, `Fixed` takes its value,
/// `Shrink` takes the content size — then clamped.
#[inline]
pub fn resolve_self(limits: Constraints, style: &Style, content: Size) -> Size {
    // Every leaf resolves its own box here, so this is the one place a
    // leaf can honour `visible`. Before 2026-09-02 no leaf read the flag
    // at all: `ops.rs` contained zero occurrences of it, so hiding a
    // `Label` changed nothing — it kept its size and kept painting
    // (`docs/plan-review-fixes.md` F1.6).
    if !style.visible {
        return Size::default();
    }
    // An unbounded axis is a scroll region's (A5). `Fill` there cannot
    // mean the maximum — it is infinite — so it means the content, and
    // the enclosing scroller's viewport reaches the *children* through
    // `LayoutCx::fill_extent`. Either way no infinity reaches a `Size`.
    let w = match style.width {
        Length::Fixed(v) => v,
        Length::Shrink => content.w,
        Length::FillPortion(_) if !limits.max_w.is_finite() => content.w,
        Length::FillPortion(_) => limits.max_w,
    };
    let h = match style.height {
        Length::Fixed(v) => v,
        Length::Shrink => content.h,
        Length::FillPortion(_) if !limits.max_h.is_finite() => content.h,
        Length::FillPortion(_) => limits.max_h,
    };
    limits.clamp(Size { w, h })
}

/// Whether an axis is the vertical one — for [`LayoutCx::fill_extent`].
#[inline]
#[doc(hidden)]
pub fn is_vertical(axis: Axis) -> bool {
    matches!(axis, Axis::Column)
}

/// Cross-axis offset for a child inside a container of `cross` extent.
#[inline]
#[doc(hidden)]
pub fn align_offset(align: Align, cross: f32, child_cross: f32) -> f32 {
    match align {
        Align::Start | Align::Fill => 0.0,
        Align::Center => ((cross - child_cross) / 2.0).max(0.0),
        Align::End => (cross - child_cross).max(0.0),
    }
}

/// Only the root of a subtree is stored by its parent; this records a
/// node's own laid-out box.
#[inline]
pub fn record(st: &mut [NodeState], i: usize, limits: Constraints, size: Size) -> Size {
    st[i].constraints = limits;
    st[i].size = size;
    size
}

/// `record`, plus the resolved paint attributes.
///
/// Every widget that draws calls this instead, so `paint` — which has no
/// `&[Style]` — can read the cascade's answer out of the state array it
/// already has.
pub fn record_painted(
    st: &mut [NodeState],
    i: usize,
    limits: Constraints,
    size: Size,
    style: &Style,
) -> Size {
    st[i].fg = style.color;
    st[i].bg = style.background;
    st[i].radius = style.radius;
    st[i].font_size = style.font_size;
    st[i].font_weight = style.font_weight;
    st[i].shadow = style.shadow;
    st[i].pad = (
        style.padding.left,
        style.padding.top,
        style.padding.right,
        style.padding.bottom,
    );
    record(st, i, limits, size)
}

// ---- leaves ----------------------------------------------------------------

/// A leaf's boilerplate: one node, no children, `node`/`node_mut` are
/// self, `place` writes a position, `hit` is a bounds test.
macro_rules! leaf_common {
    ($ty:ty, $flags:expr) => {
        fn place(&mut self, at: Point, _style: &[Style], st: &mut [NodeState], i: usize) {
            st[i].pos = at;
        }
        fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit> {
            if contains(&st[i], x, y) {
                Some(Hit {
                    node: i as u32,
                    inner: None,
                    local: Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
                })
            } else {
                None
            }
        }
        fn node(&self, _i: usize) -> &dyn Node {
            self
        }
        fn node_mut(&mut self, _i: usize) -> &mut dyn Node {
            self
        }
    };
}

/// A run of text.
///
/// Generic over storage so each profile can choose (`docs/
/// layout-and-storage-decisions.md` §1); defaults to `&'static str` so
/// the `no_std` profile is the path of least resistance. `text` carries
/// a dirty bit and `measure` memoizes, because shaping is the expensive
/// part of a real frame — the reason text lives here rather than in a
/// sibling field reached by a getter.
#[derive(Default)]
pub struct Label<S: TextStorage = &'static str> {
    pub text: Dirty<S>,
    measure: Measured,
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
    /// The compare used to live only in `crate::bind`, which meant the
    /// obvious way to write a repeater's bind function —
    /// `row.label.set_str(..)` — dirtied every visible row on every
    /// layout and defeated the memo entirely: 62 needless text
    /// measurements per *idle* frame for a 31-row window. Nothing in
    /// `fn(&mut T, usize)` steered an author away from it. Comparing
    /// here is the only sane default for a memoized field
    /// (`docs/plan-review-fixes.md` F4.6).
    ///
    /// Requires [`TextBuffer`](crate::text::TextBuffer), so
    /// `Label<&'static str>` does not have this method: that storage can
    /// only accept a new value by leaking.
    pub fn set_str(&mut self, text: &str) -> bool
    where
        S: crate::text::TextBuffer,
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

    /// Whether the memo would serve `limits` without re-measuring.
    pub fn measure_is_warm(&self, limits: Constraints) -> bool {
        !self.text.is_dirty() && self.measure.is_warm(limits)
    }
}

impl<S: TextStorage> Widget for Label<S> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];
}

impl<S: TextStorage> Node for Label<S> {
    fn kind_name(&self) -> &'static str {
        "label"
    }

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
        let text = self.text.get().as_ref();
        let dirty = self.text.is_dirty();
        let px = style[i].font_size;
        let content =
            self.measure.get_or(limits, dirty, || {
                cx.text.measure_weighted(text, px, style[i].font_weight, limits.max_w)
            });
        self.text.clear_dirty();
        let pad = style[i].padding;
        let boxed =
            Size { w: content.w + pad.horizontal(), h: content.h + pad.vertical() };
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
    fn paint(&self, st: &[NodeState], i: usize, out: &mut DisplayList) {
        let r = rect_of(&st[i]);
        out.rounded_rect(r, st[i].bg, st[i].radius);
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
    leaf_common!(Label<S>, 0);

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
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::RIGID)];
}

impl Node for Fixed {
    fn kind_name(&self) -> &'static str {
        "fixed"
    }

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
    fn paint(&self, st: &[NodeState], i: usize, out: &mut DisplayList) {
        out.rect(rect_of(&st[i]), st[i].fg);
    }
    leaf_common!(Fixed, NodeDesc::RIGID);
}

/// Fills whatever its parent hands it.
#[derive(Default)]
pub struct Spacer;

impl Widget for Spacer {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];
}

impl Node for Spacer {
    fn kind_name(&self) -> &'static str {
        "spacer"
    }

    /// The one leaf that is filler rather than content.
    fn defaults(&self) -> Style {
        Style { width: Length::FILL, height: Length::FILL, ..Style::default() }
    }

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
    fn paint(&self, _st: &[NodeState], _i: usize, _out: &mut DisplayList) {
        // A spacer is empty space; it draws nothing.
    }
    leaf_common!(Spacer, 0);
}

/// A clickable leaf. `on_click` is a plain `fn` pointer, never a closure:
/// it cannot capture, so `fresh()` re-supplies it across a hot reload
/// (`docs/events-decision.md`). Payload a closure would have captured
/// arrives in [`EventCtx`] — notably `ctx.row` inside a repeater.
pub struct Button<S: 'static, T: TextStorage = &'static str> {
    pub label: T,
    pub on_click: fn(&mut S, &EventCtx),
    /// The label's measured box and what it was measured under. `label`
    /// is a plain field, not a `Dirty<_>`, so the key carries a content
    /// hash instead of a dirty bit — a few bytes per frame against a
    /// shaping call that allocates. Until 2026-09-03 a button never
    /// measured at all: width was `0.62 em × chars`, the text box was
    /// `font_size` tall, and every label sat 4 px low
    /// (`docs/results-frame-cost.md`).
    measured: Option<(ButtonKey, Size)>,
}

/// What a button's measurement depends on.
#[derive(Clone, Copy, PartialEq)]
struct ButtonKey {
    limits: Constraints,
    px: u32,
    weight: u32,
    label: u64,
}

pub fn ignore_click<S>(_: &mut S, _: &EventCtx) {}

impl<S, T: TextStorage> Default for Button<S, T> {
    fn default() -> Self {
        Button { label: T::default(), on_click: ignore_click::<S>, measured: None }
    }
}

impl<S: 'static, T: TextStorage> Button<S, T> {
}

impl<S: 'static, T: TextStorage> Widget for Button<S, T> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::FOCUSABLE)];
}

impl<S: 'static, T: TextStorage> Node for Button<S, T> {
    fn defaults(&self) -> Style {
        Style {
            padding: crate::style::Edges { left: 14.0, right: 14.0, top: 9.0, bottom: 9.0 },
            background: Color(226, 232, 240, 255),
            color: Color(15, 23, 42, 255),
            radius: 8.0,
            ..Style::default()
        }
    }

    fn kind_name(&self) -> &'static str {
        "button"
    }

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
        let key = ButtonKey {
            limits,
            px: px.to_bits(),
            weight: weight.to_bits(),
            label: label.bytes().fold(0u64, |h, b| crate::display::fold(h, b as u64)),
        };
        let text = match self.measured {
            Some((k, s)) if k == key => s,
            _ => {
                let s = cx.text.measure_weighted(label, px, weight, limits.max_w);
                self.measured = Some((key, s));
                s
            }
        };
        let pad = style[i].padding;
        let content = Size { w: text.w + pad.horizontal(), h: text.h + pad.vertical() };
        record_painted(st, i, limits, resolve_self(limits, &style[i], content), &style[i])
    }
    fn paint(&self, st: &[NodeState], i: usize, out: &mut DisplayList) {
        let r = rect_of(&st[i]);
        out.rounded_rect(r, st[i].bg, st[i].radius);
        // Centre the *measured* line box in the button. The line box,
        // not the ink: that is what every toolkit centres, and it puts
        // capitals a hair below centre in faces with a deep descent
        // (Noto Sans: 0.03 em) — the same hair a browser leaves.
        let text = self.measured.map(|(_, s)| s).unwrap_or_default();
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
    leaf_common!(Button<S, T>, NodeDesc::FOCUSABLE);

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
impl<S: 'static, T: TextStorage> Handlers<S> for Button<S, T> {
    fn handler(&self, _sel: NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        Some(self.on_click)
    }
}
