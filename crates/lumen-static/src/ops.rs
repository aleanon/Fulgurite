//! Container layout math (row/column) and the built-in leaf widgets.
//!
//! The derive bakes a struct's axis and spacing into thin wrapper fns
//! around [`row_down`]/[`row_up`]/… (fn pointers can't capture, and for
//! Step 2 spacing is a `#[widget(...)]` attribute constant rather than a
//! field — noted as a deviation in `docs/step2-design-notes.md`).
//!
//! Container fns are dispatched once per container per phase and walk
//! their children inline via `first_child`/`next_sibling`; the inner loops
//! are dispatch-free (see the dispatch-granularity note in `desc.rs`).
//!
//! Layout model, kept deliberately boring (§1 non-goals): a container
//! splits its max constraint equally among children along its main axis,
//! sums child sizes plus spacing on that axis, takes the max crosswise.

use crate::desc::{leaf_nop, Constraints, NodeDesc, NodeState, Point, Size, WidgetOps};
use crate::widget::Widget;

#[inline]
pub(crate) fn fold(out: &mut u64, v: u64) {
    *out = out.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(v);
}

#[inline]
fn fold_geom(out: &mut u64, d: &NodeDesc, st: &NodeState) {
    fold(out, d.kind as u64);
    fold(out, st.size.w.to_bits() as u64 ^ ((st.size.h.to_bits() as u64) << 32));
    fold(out, st.pos.x.to_bits() as u64 ^ ((st.pos.y.to_bits() as u64) << 32));
}

// ---- containers ------------------------------------------------------------

#[inline]
pub const fn spaced(n: u16, spacing: f32) -> f32 {
    if n > 1 { spacing * (n - 1) as f32 } else { 0.0 }
}

/// Walk `i`'s children: `f(child_index)` per child. Caller guarantees
/// `child_count > 0`.
macro_rules! for_children {
    ($table:expr, $i:expr, $ch:ident, $body:block) => {
        let mut $ch = $table[$i].first_child as usize;
        loop {
            $body
            let ns = $table[$ch].next_sibling;
            if ns == 0 {
                break;
            }
            $ch = ns as usize;
        }
    };
}

#[inline]
pub fn row_down(table: &[NodeDesc], st: &mut [NodeState], i: usize, spacing: f32) {
    let d = &table[i];
    let c = st[i].constraints;
    let n = d.child_count as f32;
    let cc = Constraints {
        min_w: 0.0,
        max_w: ((c.max_w - spaced(d.child_count, spacing)) / n).max(0.0),
        min_h: 0.0,
        max_h: c.max_h,
    };
    for_children!(table, i, ch, {
        st[ch].constraints = cc;
    });
}

#[inline]
pub fn row_up(table: &[NodeDesc], st: &[NodeState], i: usize, spacing: f32) -> Size {
    let mut sum = 0.0f32;
    let mut max = 0.0f32;
    for_children!(table, i, ch, {
        sum += st[ch].size.w;
        max = max.max(st[ch].size.h);
    });
    st[i].constraints.clamp(Size { w: sum + spaced(table[i].child_count, spacing), h: max })
}

#[inline]
pub fn row_place(table: &[NodeDesc], st: &mut [NodeState], i: usize, spacing: f32) {
    let mut cur = st[i].pos;
    for_children!(table, i, ch, {
        st[ch].pos = cur;
        cur = Point { x: cur.x + st[ch].size.w + spacing, y: cur.y };
    });
}

#[inline]
pub fn col_down(table: &[NodeDesc], st: &mut [NodeState], i: usize, spacing: f32) {
    let d = &table[i];
    let c = st[i].constraints;
    let n = d.child_count as f32;
    let cc = Constraints {
        min_w: 0.0,
        max_w: c.max_w,
        min_h: 0.0,
        max_h: ((c.max_h - spaced(d.child_count, spacing)) / n).max(0.0),
    };
    for_children!(table, i, ch, {
        st[ch].constraints = cc;
    });
}

#[inline]
pub fn col_up(table: &[NodeDesc], st: &[NodeState], i: usize, spacing: f32) -> Size {
    let mut sum = 0.0f32;
    let mut max = 0.0f32;
    for_children!(table, i, ch, {
        sum += st[ch].size.h;
        max = max.max(st[ch].size.w);
    });
    st[i].constraints.clamp(Size { w: max, h: sum + spaced(table[i].child_count, spacing) })
}

#[inline]
pub fn col_place(table: &[NodeDesc], st: &mut [NodeState], i: usize, spacing: f32) {
    let mut cur = st[i].pos;
    for_children!(table, i, ch, {
        st[ch].pos = cur;
        cur = Point { x: cur.x, y: cur.y + st[ch].size.h + spacing };
    });
}

pub fn container_paint(_base: *const u8, d: &NodeDesc, st: &NodeState, out: &mut u64) {
    fold_geom(out, d, st);
}

/// Ops for a container whose values are *not* addressable through the tree
/// base — the synthetic root used by runtime assembly (handoff §8's
/// concat-at-startup path). Spacing 0, column axis.
pub const VIRTUAL_COLUMN_OPS: WidgetOps = WidgetOps::new(
    "virtual-column",
    |_b, t, st, i| col_down(t, st, i, 0.0),
    |_b, t, st, i| col_up(t, st, i, 0.0),
    |_b, t, st, i| col_place(t, st, i, 0.0),
    container_paint,
);

// ---- leaves ----------------------------------------------------------------

/// Content-sized text stand-in. `&'static str` keeps the leaf allocation-
/// free for Step 2; real text handling is composed in later (§11).
#[derive(Default)]
pub struct Label {
    pub text: &'static str,
}

#[cfg(feature = "observe")]
fn label_observe(
    b: *const u8,
    d: &NodeDesc,
    _st: &NodeState,
    _id: u64,
    node: &mut accesskit::Node,
    _extra: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
) {
    let l: &Label = unsafe { &*(b.add(d.offset as usize) as *const Label) };
    node.set_role(accesskit::Role::Label);
    node.set_value(l.text);
}

const LABEL_OPS: WidgetOps = {
    let ops = WidgetOps::new(
        "label",
        leaf_nop,
        |b, t, st, i| {
            let l: &Label = unsafe { &*(b.add(t[i].offset as usize) as *const Label) };
            st[i].constraints.clamp(Size { w: 8.0 * l.text.len() as f32, h: 16.0 })
        },
        leaf_nop,
        |b, d, st, out| {
            let l: &Label = unsafe { &*(b.add(d.offset as usize) as *const Label) };
            fold_geom(out, d, st);
            for &byte in l.text.as_bytes() {
                fold(out, byte as u64);
            }
        },
    );
    #[cfg(feature = "observe")]
    let ops = ops.with_observe(label_observe);
    ops
};

impl Widget for Label {
    const NODE_COUNT: usize = 1;
    const KIND_COUNT: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, 0, 0, 0, crate::desc::NO_PARENT, 0, 0)];
    const KIND_LIST: &'static [&'static WidgetOps] = &[&LABEL_OPS];
}

/// Fixed-size box: the rigid leaf (§2's relayout boundary, known at
/// compile time).
pub struct Fixed {
    pub w: f32,
    pub h: f32,
}

impl Default for Fixed {
    fn default() -> Self {
        Fixed { w: 24.0, h: 24.0 }
    }
}

const FIXED_OPS: WidgetOps = WidgetOps::new(
    "fixed",
    leaf_nop,
    |b, t, st, i| {
        let f: &Fixed = unsafe { &*(b.add(t[i].offset as usize) as *const Fixed) };
        st[i].constraints.clamp(Size { w: f.w, h: f.h })
    },
    leaf_nop,
    |b, d, st, out| {
        let f: &Fixed = unsafe { &*(b.add(d.offset as usize) as *const Fixed) };
        fold_geom(out, d, st);
        fold(out, f.w.to_bits() as u64 ^ f.h.to_bits() as u64);
    },
);

impl Widget for Fixed {
    const NODE_COUNT: usize = 1;
    const KIND_COUNT: usize = 1;
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, 0, 0, 0, crate::desc::NO_PARENT, 0, NodeDesc::RIGID)];
    const KIND_LIST: &'static [&'static WidgetOps] = &[&FIXED_OPS];
}

/// Fills whatever its parent hands it.
#[derive(Default)]
pub struct Spacer;

const SPACER_OPS: WidgetOps = WidgetOps::new(
    "spacer",
    leaf_nop,
    |_b, _t, st, i| Size { w: st[i].constraints.max_w, h: st[i].constraints.max_h },
    leaf_nop,
    |_b, d, st, out| fold_geom(out, d, st),
);

impl Widget for Spacer {
    const NODE_COUNT: usize = 1;
    const KIND_COUNT: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, 0, 0, 0, crate::desc::NO_PARENT, 0, 0)];
    const KIND_LIST: &'static [&'static WidgetOps] = &[&SPACER_OPS];
}
