//! Off-table state, owned in one shape.
//!
//! Five constructs keep node state outside the parallel arrays that the
//! table indexes: [`Repeat`](crate::repeat::Repeat) (a virtualized
//! window), [`List`](crate::hatch::List) (every item),
//! [`Dyn`](crate::hatch::Dyn) (a type-erased island), the enum derive,
//! and [`Assembled`](crate::assemble::Assembled).
//!
//! It was six until 2026-09-04: `Either` kept one arena per branch until
//! T8.0 put both branches on the merged table and deleted them
//! (`docs/either-reopened.md`).
//! Each had hand-rolled the same three things — a `Vec<NodeState>`, a
//! `Vec<Style>` seeded from `defaults()`, and a `slice(k)` accessor —
//! with five separately-written `ensure` bodies between them.
//!
//! This is the storage half, and only the storage half. Placement is
//! genuinely different for each: a repeater positions a scrolled window,
//! a list stacks with spacing, `Dyn` has exactly one child, and
//! `Assembled` rebases into a real table index. Nothing here tries to unify that.
//!
//! `docs/plan-review-fixes.md` F4.1.

// `alloc`, not the std prelude: this crate is `no_std` when the
// `std` feature is off (M13/T13.1), and absent entirely without the
// `alloc` feature (T13.4).
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::desc::NodeState;
use crate::style::Style;
use crate::widget::Node;

/// State and style for `items` instances of a template with
/// `node_count` nodes each, laid end to end.
///
/// Keyed on a **runtime** node count rather than a type parameter, so
/// [`Dyn`](crate::hatch::Dyn) — whose content is erased — can hold one
/// too.
///
/// **Requires the `alloc` feature** (default-on). One of the escape
/// hatches M13/T13.4 moved behind it, so that "what does dynamism cost
/// me" is answerable in the type system rather than in prose.
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

    /// Build the style row from `probe` if it is not already sized.
    ///
    /// The cascade runs *before* the first layout, so the row must exist
    /// by then; `ensure` sizes it alongside the instances, which is too
    /// late. Separated because the cascade has no instance count to give.
    pub fn ensure_style(&mut self, probe: &dyn Node) {
        if self.style.len() != self.node_count {
            self.style.clear();
            self.style.extend((0..self.node_count).map(|j| probe.node(j).defaults()));
        }
    }

    /// The style row, mutably — for the cascade.
    #[inline]
    pub fn style_mut(&mut self) -> &mut [Style] {
        &mut self.style
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

/// One off-table arena, exposed for the cross-cutting walks.
///
/// The cascade, the AccessKit emitter, the journal and the audit all
/// walk the table — and every construct that keeps children *off* it was
/// therefore invisible to all four. A stylesheet rule could not reach a
/// repeater's template row, an author id inside one was dead for both
/// styling and agent selectors, and the journal collapsed a whole window
/// into the repeater's single checksum
/// (`docs/plan-review-fixes.md` F4.4, F4.5).
///
/// This is the handle those walks need: the template's own sub-table to
/// walk, something to ask for defaults and kind names, and the shared
/// style row to write.
pub struct ArenaMut<'a> {
    /// The template's own table — what a walk recurses over.
    pub table: &'static [crate::desc::NodeDesc],
    /// An instance to ask for `defaults()` and `kind_name()`. One
    /// instance answers for all, since `Node::defaults` depends on the
    /// type (F3.2). Only read when `seeds` is `None`.
    pub template: &'a dyn Node,
    /// This arena's style seeds, when the content's type is known.
    ///
    /// `None` for a [`crate::hatch::Dyn`] island and nothing else:
    /// erasing the content's type is what an island is *for*, so its
    /// kinds cannot reach a const array and the cascade falls back to
    /// asking `template`. That fallback is sound where the main table's
    /// was not — a template is a live instance by construction, whereas
    /// the node the main cascade needed might be hidden (M24/T24.1).
    pub seeds: Option<&'static [crate::style::Seed]>,
    /// The style row every instance in this arena shares.
    pub style: &'a mut [Style],
}
