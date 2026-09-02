//! Off-table state, owned in one shape.
//!
//! Six constructs keep node state outside the parallel arrays that the
//! table indexes: [`Repeat`](crate::repeat::Repeat) (a virtualized
//! window), [`List`](crate::hatch::List) (every item), [`Either`](crate::hatch::Either)
//! (two branches at once), [`Dyn`](crate::hatch::Dyn) (a type-erased
//! island), the enum derive, and [`Assembled`](crate::assemble::Assembled).
//! Each had hand-rolled the same three things — a `Vec<NodeState>`, a
//! `Vec<Style>` seeded from `defaults()`, and a `slice(k)` accessor —
//! with five separately-written `ensure` bodies between them.
//!
//! This is the storage half, and only the storage half. Placement is
//! genuinely different for each: a repeater positions a scrolled window,
//! a list stacks with spacing, an `Either` places one of two at the same
//! origin, `Dyn` has exactly one child, and `Assembled` rebases into a
//! real table index. Nothing here tries to unify that.
//!
//! `docs/plan-review-fixes.md` F4.1.

use crate::desc::NodeState;
use crate::style::Style;
use crate::widget::Node;

/// State and style for `items` instances of a template with
/// `node_count` nodes each, laid end to end.
///
/// Keyed on a **runtime** node count rather than a type parameter, so
/// [`Dyn`](crate::hatch::Dyn) — whose content is erased — can hold one
/// too.
#[derive(Default, Debug, Clone)]
pub struct Arena {
    node_count: usize,
    states: Vec<NodeState>,
    style: Vec<Style>,
}

impl Arena {
    pub fn new(node_count: usize) -> Self {
        Arena { node_count, states: Vec::new(), style: Vec::new() }
    }

    /// Nodes per instance.
    #[inline]
    pub fn node_count(&self) -> usize {
        self.node_count
    }

    /// Instances currently held.
    #[inline]
    pub fn len(&self) -> usize {
        if self.node_count == 0 {
            0
        } else {
            self.states.len() / self.node_count
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Size for `items` instances and (re)build the style row from
    /// `probe`.
    ///
    /// The style row is per *template*, not per instance, which is sound
    /// only because [`Node::defaults`] is required to depend on the type
    /// alone. It was not, and the three hand-written versions of this
    /// each snapshotted one instance's state and never rebuilt — a
    /// `List<Option<Spacer>>` whose first item was `None` styled every
    /// item `Shrink` forever (`docs/plan-review-fixes.md` F3.2).
    pub fn ensure(&mut self, items: usize, probe: &dyn Node) {
        let want = items * self.node_count;
        if self.states.len() != want {
            self.states.resize(want, NodeState::default());
        }
        if self.style.len() != self.node_count {
            self.style.clear();
            self.style.extend((0..self.node_count).map(|j| probe.node(j).defaults()));
        }
    }

    /// Drop every instance, keeping the allocation and the style row.
    pub fn clear_instances(&mut self) {
        self.states.clear();
    }

    /// Instance `k`'s states.
    #[inline]
    pub fn slice(&self, k: usize) -> &[NodeState] {
        &self.states[k * self.node_count..(k + 1) * self.node_count]
    }

    /// Instance `k`'s states, mutably.
    #[inline]
    pub fn slice_mut(&mut self, k: usize) -> &mut [NodeState] {
        &mut self.states[k * self.node_count..(k + 1) * self.node_count]
    }

    /// One node of one instance.
    #[inline]
    pub fn node_state(&self, k: usize, node: usize) -> NodeState {
        self.states[k * self.node_count + node]
    }

    /// The style row every instance shares.
    #[inline]
    pub fn style(&self) -> &[Style] {
        &self.style
    }

    /// Shared style row and exclusive states from one `&mut self`.
    ///
    /// The five owners each hand-wrote this destructuring so they could
    /// iterate instances while reading the style row; disjoint field
    /// borrows make it legal, and having it once means the pattern is
    /// stated once.
    #[inline]
    pub fn parts(&mut self) -> (&[Style], &mut [NodeState]) {
        (&self.style, &mut self.states)
    }
}
