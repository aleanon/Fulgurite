//! The [`Node`] and [`Widget`] traits, and the const splicing helpers
//! the derive uses.
//!
//! Two traits, split by what they can carry. [`Node`] holds the methods
//! and is object-safe, so a container can reach a child as `&mut dyn
//! Node`. [`Widget`] adds the associated consts — `NODE_COUNT`, `DEPTH`,
//! `SUB_TABLE` — which is exactly what a trait object cannot have. Every
//! widget implements both; the split is what lets compile-time facts and
//! runtime traversal coexist.
//!
//! Traversal is recursive descent: the vtable arrives with the child
//! reference, so there is no per-node kind lookup to pay. That replaced
//! a flat table walk over a static array of `fn` pointers, and was
//! measured 34–39% faster (`docs/results-traversal.md`).
//!
//! Code is emitted per widget *kind*, not per tree position — §4's trap
//! is avoided exactly as §5 intended, by the `&dyn` coercion rather than
//! by a table. `#[widget(fuse)]` emits the same bodies with static calls,
//! which is the monomorphization dial and nothing more.

use crate::desc::{Constraints, NodeDesc, NodeState, Point, Size};
use crate::cx::LayoutCx;
use crate::display::DisplayList;
use crate::style::Style;
use crate::hit::Hit;

/// The object-safe half: everything traversal calls.
///
/// Split from [`Widget`] because associated consts are not
/// dyn-compatible, and children are reached as `&mut dyn Node`.
pub trait Node {
    /// Lay out this subtree under `limits`, writing into `st` from index
    /// `i`, and return this node's size.
    ///
    /// `&mut self` because each node is visited exactly once, which makes
    /// exclusive access free and lets stateful widgets (a repeater's
    /// window, a measure cache) mutate without interior mutability.
    ///
    /// `style` is the parallel computed-style array (plan M1), read for
    /// this node's `Length`s, padding, spacing and alignment.
    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size;

    /// Position this subtree with its origin at `at`.
    ///
    /// `&mut self` for the same reason as `layout`: one visit per node,
    /// and a widget owning off-table geometry (a repeater's window) must
    /// write it without interior mutability.
    fn place(&mut self, at: Point, style: &[Style], st: &mut [NodeState], i: usize);

    /// Emit drawing commands. The renderer consumes the list *after*
    /// the pass, so it never appears in a widget signature.
    fn paint(&self, st: &[NodeState], i: usize, out: &mut DisplayList);

    /// Deepest node under `(x, y)`, or `None` if the point misses.
    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit>;

    /// This kind's own style defaults, before any rule applies.
    fn defaults(&self) -> Style {
        Style::default()
    }

    /// This widget kind's name, for agent selectors and diagnostics.
    /// Replaces the name field the old per-kind ops table carried.
    fn kind_name(&self) -> &'static str {
        "widget"
    }

    /// The widget at table index `i` of this subtree (`i == 0` is self).
    /// Index addressing for the observability surface only.
    fn node(&self, i: usize) -> &dyn Node;

    /// Mutable counterpart, for tools that must call `layout` on a single
    /// node (the audit's intrinsic probe).
    fn node_mut(&mut self, i: usize) -> &mut dyn Node;

    /// Contribute to the accessibility/agent tree.
    #[cfg(feature = "observe")]
    #[allow(unused_variables)]
    fn observe(
        &self,
        st: &[NodeState],
        i: usize,
        id: u64,
        node: &mut accesskit::Node,
        extra: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
    }

    /// Cheap scalar introspection for the per-frame audit.
    #[cfg(feature = "observe")]
    #[allow(unused_variables)]
    fn stats(&self, out: &mut crate::desc::NodeStats) {}
}

/// The compile-time half: facts about a widget's shape.
pub trait Widget: Node {
    /// Nodes in this widget's subtree, itself included (handoff §3).
    const NODE_COUNT: usize;
    /// Deepest chain of nested widgets, itself included. Asserted against
    /// [`crate::desc::MAX_DEPTH`] by the derive, so a statically-typed
    /// tree that could overflow the stack does not compile.
    ///
    /// **One exception:** [`crate::hatch::Dyn`] erases its content's
    /// type, so the content's depth cannot reach this arithmetic. `Dyn`
    /// reports a constant and checks its content at construction
    /// instead; a shallow island nested deeply is not caught. See its
    /// `DEPTH` for what that does and does not cover.
    const DEPTH: usize;
    /// Size provably independent of content (handoff §2's relayout boundary).
    const IS_RIGID: bool;
    /// Structural descriptors for this subtree — index addressing for the
    /// agent surface, journal and audit. Not read during layout.
    const SUB_TABLE: &'static [NodeDesc];
}

/// Copy a child's `SUB_TABLE` into a parent table being built, rebasing
/// indices by `write_at`. The child's root is re-parented to
/// `parent_index`. Const, so derives run it at compile time; runtime
/// assembly (handoff §8) calls the same fn.
pub const fn splice_child(
    dst: &mut [NodeDesc],
    write_at: usize,
    src: &[NodeDesc],
    parent_index: u32,
) {
    let mut j = 0;
    while j < src.len() {
        let mut d = src[j];
        if d.child_count > 0 {
            d.first_child += write_at as u32;
        }
        if d.next_sibling != 0 {
            d.next_sibling += write_at as u32;
        }
        d.parent = if j == 0 { parent_index } else { d.parent + write_at as u32 };
        dst[write_at + j] = d;
        j += 1;
    }
    // The spliced root's sibling link is the caller's to set.
    dst[write_at].next_sibling = 0;
}

/// Structural well-formedness of a table, as a `const fn` so the derive
/// can assert it at compile time for every table it generates —
/// proof-carrying tables (`docs/verification-notes.md`, Tier 0).
pub const fn table_is_well_formed(t: &[NodeDesc]) -> bool {
    if t.is_empty() {
        return false;
    }
    if t[0].parent != crate::desc::NO_PARENT {
        return false;
    }
    let mut i = 1;
    while i < t.len() {
        // Parent precedes child (DFS pre-order).
        if t[i].parent as usize >= i {
            return false;
        }
        i += 1;
    }
    // Every declared child count is matched by that many nodes claiming
    // the node as parent.
    let mut n = 0;
    while n < t.len() {
        let mut seen = 0;
        let mut k = 1;
        while k < t.len() {
            if t[k].parent as usize == n {
                seen += 1;
            }
            k += 1;
        }
        if seen != t[n].child_count as usize {
            return false;
        }
        if t[n].child_count > 0 && t[n].first_child as usize != n + 1 {
            return false;
        }
        n += 1;
    }
    true
}
