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
        if table[i].is_container() {
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
        if table[i].is_container() {
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
        if table[i].is_container() {
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
        if table[i].is_container() {
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

/// Dispatch one input event against a laid-out tree — plan E2.
///
/// Pointer events hit-test; keyboard events go to `focus`. The event is
/// offered to the hit node's kind and then to each ancestor until one
/// consumes it (leaf → root), so a container can act as a fallback for
/// content that ignored the event.
///
/// # Contract (extends `WidgetOps`'s)
/// `state` and `base` must be derived from the same live borrow of the
/// application state — the widget tree is normally a field of it — and
/// `st` must **not** live inside that state (layout output is a separate
/// array in every shape this framework supports). No reference into
/// either is held across a handler call: each event op copies its `fn`
/// pointer out before materializing `&mut S`.
pub fn dispatch(
    state: *mut u8,
    base: *const u8,
    table: &[NodeDesc],
    kinds: &[&'static WidgetOps],
    st: &[NodeState],
    focus: &mut Option<u32>,
    ev: &crate::event::Event,
) -> bool {
    use crate::event::{Event, EventCtx};

    let (start, ctx) = match ev.position() {
        Some(p) => {
            let Some(h) = crate::hit::hit_test(base, table, kinds, st, p.x, p.y) else {
                return false;
            };
            // Pointer-down moves focus to the deepest focusable node on
            // the hit chain (or clears it, which is what clicking the
            // background should do).
            if matches!(ev, Event::PointerDown { .. }) {
                let mut f = None;
                let mut i = h.node as usize;
                loop {
                    if table[i].is_focusable() {
                        f = Some(i as u32);
                        break;
                    }
                    if table[i].parent == crate::desc::NO_PARENT {
                        break;
                    }
                    i = table[i].parent as usize;
                }
                *focus = f;
            }
            let ctx = EventCtx {
                node: h.node,
                row: h.inner.map(|i| i.row),
                inner: h.inner.map_or(0, |i| i.node),
                pos: p,
                local: h.local,
            };
            (h.node as usize, ctx)
        }
        None => match *focus {
            Some(f) => (f as usize, EventCtx::at(f)),
            None => return false,
        },
    };

    let mut i = start;
    loop {
        let d = &table[i];
        let mut ctx = ctx;
        ctx.node = i as u32;
        if i != start {
            // Only the node actually hit owns the inner/row detail.
            ctx.row = None;
            ctx.inner = 0;
        }
        if (kinds[d.kind as usize].event)(base, state, d, &st[i], ev, &ctx) {
            return true;
        }
        if d.parent == crate::desc::NO_PARENT {
            return false;
        }
        i = d.parent as usize;
    }
}
