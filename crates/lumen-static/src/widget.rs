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

    /// How many off-table arenas this node owns.
    ///
    /// Zero for everything in the table. A repeater owns one (its
    /// window), an `Either` owns two (one per branch), `List` and `Dyn`
    /// own one each. See [`crate::arena::ArenaMut`] for why the
    /// cross-cutting walks need to know.
    fn arena_count(&self) -> usize {
        0
    }

    /// Arena `a`, for a walk that must reach off-table content.
    fn arena_mut(&mut self, _a: usize) -> Option<crate::arena::ArenaMut<'_>> {
        None
    }

    /// This kind's own style defaults, before any rule applies.
    ///
    /// **Must depend only on the type, never on `self`'s current
    /// state.** The cascade calls this when a stylesheet is set, not per
    /// frame, and the arena containers snapshot it once per template —
    /// so a state-dependent answer is frozen at whatever the state
    /// happened to be, and never revisited. `Option::defaults` returned
    /// `Style::default()` while `None`, which meant an `Option` empty at
    /// construction had its slot styled `Shrink` permanently: setting it
    /// to `Some(Spacer)` later laid the spacer out at 0x0 instead of
    /// filling, with nothing to indicate why.
    ///
    /// Runtime state belongs in `layout` — which runs every frame — not
    /// here. `Option` hides its subtree when empty; that is the same
    /// information expressed where it can change.
    /// `docs/plan-review-fixes.md` F3.2.
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
    /// Measure under `limits` **without disturbing any memo**.
    ///
    /// The ambient audit probes every leaf twice per frame to decide
    /// whether it is sized by its own content. Doing that through
    /// `layout` left each `Label`'s measurement cache keyed on the
    /// probe's constraints rather than the frame's, so the memo missed
    /// on every subsequent frame — measured: 0 text measurements per
    /// idle frame without the agent attached, 400 of 400 with it. Under
    /// a real shaper that is the whole shaping cost paid three times per
    /// frame, in the one component whose module doc names "re-shaping
    /// every text node" as a scar it learned from.
    ///
    /// The default *is* `layout`, which is right for every widget that
    /// has nothing to cache. `Label` overrides it.
    /// `docs/plan-review-fixes.md` F2.1.
    /// Handle an event addressed to this node, returning whether it was
    /// consumed (C3, C4 of `docs/text-and-scrolling-decisions.md`).
    ///
    /// The seam a widget edits *itself* through. `Handlers` carries the
    /// application's `fn(&mut S, &EventCtx)` — the click that changes
    /// app state — and cannot reach widget state, because from `&mut S`
    /// a widget is only reachable as `&mut dyn Node`. That is why a
    /// scroller could not consume a wheel event and a text field could
    /// not accept a keystroke, however well the events were routed.
    ///
    /// `dispatch` offers the event to each node on the chain, deepest
    /// first, and stops at the first `true`. **The `bool` is what makes
    /// scroll chaining work** (C4): a scroller at its end returns
    /// `false` and the event continues to its parent, which is what
    /// "nested scrollers, at my end take it" means. Returning `true`
    /// from a node that did nothing silently swallows input.
    ///
    /// Runs *before* the application's activation handler, so a widget
    /// may claim a key the app would otherwise treat as a click.
    fn on_event(&mut self, ev: &crate::event::Event, cx: &crate::event::EventCtx) -> bool {
        let _ = (ev, cx);
        false
    }

    /// Whether this node derives how much of its content to materialize
    /// from the constraints it is given.
    ///
    /// `Repeat` does; nothing else does. It exists so
    /// [`ScrollView`](crate::scroll::ScrollView) can refuse to contain
    /// one (A7 of `docs/text-and-scrolling-decisions.md`): a scroller
    /// hands its content the viewport, a `Repeat` reads that as "show
    /// this much", and a repeater inside a scroller therefore shows a
    /// viewport's worth *of the scroller's* — the two disagree about who
    /// is windowing. A `Repeat` is already a scroller.
    ///
    /// Not behind `observe`, deliberately: it guards an invariant, and
    /// an invariant that only holds in dev builds is not one.
    fn virtualizes(&self) -> bool {
        false
    }

    #[cfg(feature = "observe")]
    fn probe(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        self.layout(cx, limits, style, st, i)
    }

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
#[doc(hidden)]
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
#[doc(hidden)]
pub const fn table_is_well_formed(t: &[NodeDesc]) -> bool {
    if t.is_empty() {
        return false;
    }
    if t[0].parent != crate::desc::NO_PARENT {
        return false;
    }

    // Parent precedes child (DFS pre-order), and the declared child
    // counts add up to "every node but the root is somebody's child".
    let mut declared = 0usize;
    let mut i = 0;
    while i < t.len() {
        if i > 0 && t[i].parent as usize >= i {
            return false;
        }
        declared += t[i].child_count as usize;
        i += 1;
    }
    if declared != t.len() - 1 {
        return false;
    }

    // Each parent's declared children are real, linked, and claim it.
    // Walking the sibling chains visits every non-root node exactly
    // once, so this whole loop is O(n) rather than the O(n^2) rescan it
    // replaces — which took 20 s of const-eval at 3.2k nodes and tripped
    // `long_running_const_eval` at 10k, i.e. a tree that large could not
    // be compiled at all (`docs/plan-review-fixes.md` F5.1).
    let mut n = 0;
    while n < t.len() {
        let c = t[n].child_count as usize;
        if c > 0 {
            if t[n].first_child as usize != n + 1 {
                return false;
            }
            let mut k = n + 1;
            let mut seen = 0;
            while seen < c {
                if k >= t.len() || t[k].parent as usize != n {
                    return false;
                }
                seen += 1;
                let next = t[k].next_sibling as usize;
                if seen < c {
                    // More to come: the link must move forward and stay
                    // inside the table.
                    if next <= k || next >= t.len() {
                        return false;
                    }
                } else if next != 0 && next <= k {
                    // The last child terminates with 0, or points at a
                    // later node (its parent's own next sibling).
                    return false;
                }
                k = next;
            }
        }
        n += 1;
    }
    true
}
