//! What a point landed on.
//!
//! Hit testing recurses with layout (`Widget::hit`): a container that
//! contains the point asks its children, and the first child that claims
//! it wins. Because containers place children sequentially by measured
//! size and children never escape their parents, at most one child can
//! claim a point — so "first claimant" *is* "deepest node", with no
//! depth bookkeeping.

use crate::desc::Point;

/// Off-table content a node owns, resolved from a point: a repeater row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Inner {
    /// Which row of virtualized content.
    pub row: u64,
    /// Node index within that row's template table.
    pub node: u32,
}

/// Where a point landed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// Main-table node index.
    pub node: u32,
    /// Virtualized content the node owns, when the point was inside it.
    pub inner: Option<Inner>,
    /// Point relative to the receiving node's origin.
    pub local: Point,
}

/// Does `(x, y)` fall inside this node's laid-out box?
#[inline]
pub fn contains(st: &crate::desc::NodeState, x: f32, y: f32) -> bool {
    x >= st.pos.x && y >= st.pos.y && x < st.pos.x + st.size.w && y < st.pos.y + st.size.h
}
