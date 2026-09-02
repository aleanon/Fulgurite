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

/// The node's box as a rect.
#[inline]
pub fn contains_rect(st: &crate::desc::NodeState) -> crate::display::Rect {
    crate::ops::rect_of(st)
}

/// Whether two rects share any area.
///
/// Used where a construct must report only what is actually on screen —
/// a repeater's window overruns its viewport by construction, and those
/// rows are clipped from paint and rejected by hit, so they must not be
/// announced to AccessKit or the agent either (F4.7).
#[inline]
pub fn rects_overlap(a: crate::display::Rect, b: crate::display::Rect) -> bool {
    a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
}
