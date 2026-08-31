//! Hit testing over the flat table — plan E1.
//!
//! **Why "last containing node wins" is correct here.** All nodes that
//! contain a point form a single ancestor chain, because this layout
//! model guarantees two invariants: children never escape their parent
//! (constraints flow down and every node clamps to them) and siblings
//! never overlap (containers place them sequentially by measured size).
//! In DFS pre-order the deepest member of that chain is the last one, so
//! one forward scan finds it — no depth array, no allocation.
//!
//! Both invariants are pinned by
//! `lumen-static-agent/tests/lint.rs::overflow_is_impossible_here_...`,
//! which is also why the audit reserves `W1003`/`W1004` rather than
//! shipping them. The first construct that breaks them — a scrollable
//! content region, absolute positioning — must revisit this function at
//! the same time it makes those codes implementable.

use crate::desc::{Inner, NodeDesc, NodeState, Point, WidgetOps};

/// Where a point landed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// Main-table node index.
    pub node: u32,
    /// Virtualized content the node owns, when the point landed inside it.
    pub inner: Option<Inner>,
    /// Point relative to the receiving node's (or inner node's) origin.
    pub local: Point,
}

#[inline]
fn contains(st: &NodeState, x: f32, y: f32) -> bool {
    x >= st.pos.x
        && y >= st.pos.y
        && x < st.pos.x + st.size.w
        && y < st.pos.y + st.size.h
}

/// Deepest node under `(x, y)`, descending into off-table content
/// (repeater rows) through each kind's `hit` op.
pub fn hit_test(
    base: *const u8,
    table: &[NodeDesc],
    kinds: &[&'static WidgetOps],
    st: &[NodeState],
    x: f32,
    y: f32,
) -> Option<Hit> {
    let mut found: Option<usize> = None;
    for (i, _) in table.iter().enumerate() {
        if contains(&st[i], x, y) {
            found = Some(i);
        }
    }
    let i = found?;
    let d = &table[i];
    let inner = (kinds[d.kind as usize].hit)(base, d, &st[i], x, y);
    let origin = match &inner {
        Some(inn) => inn.origin,
        None => st[i].pos,
    };
    Some(Hit {
        node: i as u32,
        inner,
        local: Point { x: x - origin.x, y: y - origin.y },
    })
}

/// The same scan restricted to one (instance) table — the helper a
/// virtualized widget's `hit` op uses over its window states.
pub fn deepest(table: &[NodeDesc], st: &[NodeState], x: f32, y: f32) -> Option<u32> {
    let mut found = None;
    for (i, _) in table.iter().enumerate() {
        if contains(&st[i], x, y) {
            found = Some(i as u32);
        }
    }
    found
}
