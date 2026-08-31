//! Container layout math and the built-in leaf widgets.
//!
//! Layout is recursive: a container calls each child's `layout` and gets
//! its final size back, so nothing is estimated and corrected later. The
//! shared helpers below are the per-container math the derive's emitted
//! bodies call — one function per axis per phase, not per widget.

use crate::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use crate::event::EventCtx;
use crate::hit::{contains, Hit};
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

#[inline]
pub fn fold_geom(out: &mut u64, tag: u64, st: &NodeState) {
    fold(out, tag);
    fold(out, st.size.w.to_bits() as u64 ^ ((st.size.h.to_bits() as u64) << 32));
    fold(out, st.pos.x.to_bits() as u64 ^ ((st.pos.y.to_bits() as u64) << 32));
}

/// Space consumed by gaps between `n` children.
#[inline]
pub const fn spaced(n: usize, spacing: f32) -> f32 {
    if n > 1 { spacing * (n - 1) as f32 } else { 0.0 }
}

/// The limits a row hands each of its `n` children.
#[inline]
pub fn row_child_limits(limits: Constraints, n: usize, spacing: f32) -> Constraints {
    Constraints {
        min_w: 0.0,
        max_w: ((limits.max_w - spaced(n, spacing)) / n.max(1) as f32).max(0.0),
        min_h: 0.0,
        max_h: limits.max_h,
    }
}

#[inline]
pub fn col_child_limits(limits: Constraints, n: usize, spacing: f32) -> Constraints {
    Constraints {
        min_w: limits.min_w.min(limits.max_w),
        max_w: limits.max_w,
        min_h: 0.0,
        max_h: ((limits.max_h - spaced(n, spacing)) / n.max(1) as f32).max(0.0),
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
        fn place(&mut self, at: Point, st: &mut [NodeState], i: usize) {
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

/// Content-sized text stand-in. `&'static str` until M3 makes storage
/// generic over `AsRef<str>`.
#[derive(Default)]
pub struct Label {
    pub text: &'static str,
}

impl Widget for Label {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];
}

impl Node for Label {
    fn kind_name(&self) -> &'static str {
        "label"
    }

    fn layout(&mut self, limits: Constraints, st: &mut [NodeState], i: usize) -> Size {
        let s = limits.clamp(Size { w: 8.0 * self.text.len() as f32, h: 16.0 });
        record(st, i, limits, s)
    }
    fn paint(&self, st: &[NodeState], i: usize, out: &mut u64) {
        fold_geom(out, 1, &st[i]);
        for &b in self.text.as_bytes() {
            fold(out, b as u64);
        }
    }
    leaf_common!(Label, 0);

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
        node.set_value(self.text);
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

    fn layout(&mut self, limits: Constraints, st: &mut [NodeState], i: usize) -> Size {
        let s = limits.clamp(Size { w: self.w, h: self.h });
        record(st, i, limits, s)
    }
    fn paint(&self, st: &[NodeState], i: usize, out: &mut u64) {
        fold_geom(out, 2, &st[i]);
        fold(out, self.w.to_bits() as u64 ^ self.h.to_bits() as u64);
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

    fn layout(&mut self, limits: Constraints, st: &mut [NodeState], i: usize) -> Size {
        let s = Size { w: limits.max_w, h: limits.max_h };
        record(st, i, limits, s)
    }
    fn paint(&self, st: &[NodeState], i: usize, out: &mut u64) {
        fold_geom(out, 3, &st[i]);
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

    fn layout(&mut self, limits: Constraints, st: &mut [NodeState], i: usize) -> Size {
        let s = limits.clamp(Size {
            w: 8.0 * self.label.len() as f32 + 2.0 * Self::PAD,
            h: 16.0 + Self::PAD,
        });
        record(st, i, limits, s)
    }
    fn paint(&self, st: &[NodeState], i: usize, out: &mut u64) {
        fold_geom(out, 4, &st[i]);
        for &b in self.label.as_bytes() {
            fold(out, b as u64);
        }
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

impl VirtualColumn {
    pub fn paint_self(&self, st: &[NodeState], i: usize, out: &mut u64) {
        fold_geom(out, 0, &st[i]);
    }
}

// ---- handler routing ---------------------------------------------------

use crate::event::{Handlers, NodeSel};

macro_rules! no_handlers {
    ($ty:ty) => {
        impl<S> Handlers<S> for $ty {
            fn handler(&self, _sel: NodeSel) -> Option<fn(&mut S, &EventCtx)> {
                None
            }
        }
    };
}

no_handlers!(Label);
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
