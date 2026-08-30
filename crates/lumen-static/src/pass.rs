//! Iterative layout and paint passes over the flat table (handoff §5).
//!
//! Single-pass layout decomposed into three stack-free loops over the
//! DFS-ordered table:
//!   1. constraints down — forward over containers; a container's own
//!      constraints are final before it is visited (root pre-seeded,
//!      children written by their parent's `down`, parents precede
//!      children);
//!   2. sizes up — reverse over all nodes; a container's children follow
//!      it in the table, so their sizes are already final when it is
//!      visited;
//!   3. positions down — forward over containers; parent positions are
//!      final before a container places its own children.
//!
//! No node ever reads its own final position during layout (§2's
//! single-pass property), no loop recurses, and dispatch is once per
//! container per phase (leaves dispatch only in the up and paint phases).

use crate::desc::{Constraints, NodeDesc, NodeState, Point, WidgetOps};

/// Full layout: every node visited. `base` is the tree value struct;
/// `table`/`kinds` must be the matching derive/assembly output.
pub fn layout(
    base: *const u8,
    table: &[NodeDesc],
    kinds: &[&'static WidgetOps],
    st: &mut [NodeState],
    root_c: Constraints,
) {
    assert_eq!(table.len(), st.len());

    // 1. constraints down
    st[0].constraints = root_c;
    for i in 0..table.len() {
        if table[i].child_count > 0 {
            (kinds[table[i].kind as usize].down)(base, table, st, i);
        }
    }

    // 2. sizes up
    for i in (0..table.len()).rev() {
        let s = (kinds[table[i].kind as usize].up)(base, table, st, i);
        st[i].size = s;
    }

    // 3. positions down
    st[0].pos = Point::default();
    for i in 0..table.len() {
        if table[i].child_count > 0 {
            (kinds[table[i].kind as usize].place)(base, table, st, i);
        }
    }
}

/// Partitioned layout: rigid subtrees are neither constrained nor
/// re-measured — parents read their cached sizes (handoff §5's
/// rigid/flexible partition). Requires one prior full [`layout`] with the
/// same `root_c` family so rigid caches are warm. Positions are always
/// recomputed (they depend on siblings).
///
/// `flexible` holds the non-rigid table indices in ascending order. A
/// container's rigidity is the AND of its children's (the derive
/// guarantees it), so every flexible node's ancestors are flexible too and
/// rigid subtrees are never re-entered.
pub fn layout_partitioned(
    base: *const u8,
    table: &[NodeDesc],
    kinds: &[&'static WidgetOps],
    st: &mut [NodeState],
    root_c: Constraints,
    flexible: &[u32],
) {
    st[0].constraints = root_c;

    // 1. constraints down, flexible containers only (a rigid child gets its
    // constraints written but never consumes them).
    for &fi in flexible {
        let i = fi as usize;
        if table[i].child_count > 0 {
            (kinds[table[i].kind as usize].down)(base, table, st, i);
        }
    }

    // 2. sizes up, flexible only — container `up` reads child sizes from
    // `st`, which for rigid children is the warm cache.
    for &fi in flexible.iter().rev() {
        let i = fi as usize;
        let s = (kinds[table[i].kind as usize].up)(base, table, st, i);
        st[i].size = s;
    }

    // 3. positions down — every container, rigid subtrees included (their
    // internal offsets shift with their parents).
    st[0].pos = Point::default();
    for i in 0..table.len() {
        if table[i].child_count > 0 {
            (kinds[table[i].kind as usize].place)(base, table, st, i);
        }
    }
}

/// Paint pass: forward loop, fold every node into a checksum.
pub fn paint(
    base: *const u8,
    table: &[NodeDesc],
    kinds: &[&'static WidgetOps],
    st: &[NodeState],
    out: &mut u64,
) {
    for (d, s) in table.iter().zip(st.iter()) {
        (kinds[d.kind as usize].paint)(base, d, s, out);
    }
}
