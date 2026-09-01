//! Container layout math and the built-in leaf widgets.
//!
//! Layout is recursive: a container calls each child's `layout` and gets
//! its final size back, so nothing is estimated and corrected later. The
//! shared helpers below are the per-container math the derive's emitted
//! bodies call — one function per axis per phase, not per widget.

use crate::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use crate::event::EventCtx;
use crate::hit::{contains, Hit};
use crate::cx::LayoutCx;
use crate::display::{Color, DisplayList};
use crate::style::{Align, Length, Style};
use crate::text::{Dirty, Measured, TextStorage};
use crate::widget::{Node, Widget};

#[inline]
pub(crate) fn fold(out: &mut u64, v: u64) {
    *out = out.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(v);
}

/// Public so derive-emitted bodies can fold identically to the leaves.
#[inline]
pub fn fold_pub(out: &mut u64, v: u64) {
    fold(out, v)
}

/// A node's absolute box, for emitting commands.
#[inline]
pub fn rect_of(st: &NodeState) -> crate::display::Rect {
    [st.pos.x, st.pos.y, st.pos.x + st.size.w, st.pos.y + st.size.h]
}

/// Containers and structural nodes draw nothing by themselves; this
/// keeps the call site uniform and gives a renderer a hook later.
#[inline]
pub fn paint_container(_out: &mut DisplayList, _st: &NodeState) {}

/// Space consumed by gaps between `n` children.
#[inline]
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
    let w = match style.width {
        Length::Fixed(v) => v,
        Length::Shrink => content.w,
        Length::FillPortion(_) => limits.max_w,
    };
    let h = match style.height {
        Length::Fixed(v) => v,
        Length::Shrink => content.h,
        Length::FillPortion(_) => limits.max_h,
    };
    limits.clamp(Size { w, h })
}

/// Cross-axis offset for a child inside a container of `cross` extent.
#[inline]
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
    /// allocation where it can. Marks for re-measurement unconditionally
    /// — callers that want the write skipped when nothing changed go
    /// through `crate::bind`, which compares first.
    pub fn set_str(&mut self, text: &str) {
        self.text.get_mut().set_from_str(text);
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
        let content =
            self.measure.get_or(limits, dirty, || cx.text.measure(text, 16.0, limits.max_w));
        self.text.clear_dirty();
        record(st, i, limits, resolve_self(limits, &style[i], content))
    }
    fn paint(&self, st: &[NodeState], i: usize, out: &mut DisplayList) {
        out.text(rect_of(&st[i]), self.text.get().as_ref(), 16.0, Color::BLACK);
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
        record(st, i, limits, resolve_self(limits, &style[i], content))
    }
    fn paint(&self, st: &[NodeState], i: usize, out: &mut DisplayList) {
        out.rect(rect_of(&st[i]), Color::BLACK);
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
        record(st, i, limits, resolve_self(limits, &style[i], Size::default()))
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
pub struct Button<S: 'static> {
    pub label: &'static str,
    pub on_click: fn(&mut S, &EventCtx),
}

pub fn ignore_click<S>(_: &mut S, _: &EventCtx) {}

impl<S> Default for Button<S> {
    fn default() -> Self {
        Button { label: "", on_click: ignore_click::<S> }
    }
}

impl<S: 'static> Button<S> {
    const PAD: f32 = 8.0;
}

impl<S: 'static> Widget for Button<S> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::FOCUSABLE)];
}

impl<S: 'static> Node for Button<S> {
    fn kind_name(&self) -> &'static str {
        "button"
    }

    fn layout(
        &mut self,
        _cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let content = Size {
            w: 8.0 * self.label.len() as f32 + 2.0 * Self::PAD,
            h: 16.0 + Self::PAD,
        };
        record(st, i, limits, resolve_self(limits, &style[i], content))
    }
    fn paint(&self, st: &[NodeState], i: usize, out: &mut DisplayList) {
        let r = rect_of(&st[i]);
        out.rect(r, Color(220, 220, 220, 255));
        out.text(r, self.label, 16.0, Color::BLACK);
    }
    leaf_common!(Button<S>, NodeDesc::FOCUSABLE);

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
        node.set_label(self.label);
        node.add_action(accesskit::Action::Click);
    }
}

/// The synthetic column root runtime assembly puts above its instances.
#[derive(Default)]
pub struct VirtualColumn;

// ---- handler routing ---------------------------------------------------

use crate::event::{Handlers, NodeSel};

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
impl<S: 'static> Handlers<S> for Button<S> {
    fn handler(&self, _sel: NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        Some(self.on_click)
    }
}
