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

// `alloc`, not the std prelude: this crate is `no_std` when the
// `std` feature is off (M13/T13.1).
// Only the accesskit `extra` arg needs it, and that is observe-gated.
#[cfg(feature = "observe")]
use alloc::vec::Vec;

use crate::desc::{Constraints, NodeDesc, NodeState, Point, Size};
use crate::cx::LayoutCx;
use crate::display::Painter;
use crate::style::{Seed, Style};
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

    /// Emit drawing commands into whatever the caller supplies.
    ///
    /// `&mut dyn Painter`, not `&mut DisplayList`, since M10a: a widget
    /// describes *what* to draw and does not name where it goes. A
    /// desktop host collects into a [`crate::DisplayList`]; a host with
    /// no allocator implements [`crate::Painter`] on a rasteriser and
    /// no list is ever built. The renderer likewise never appears in a
    /// widget signature — it consumes the list afterwards.
    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter);

    /// Deepest node under `(x, y)`, or `None` if the point misses.
    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit>;

    /// How many off-table arenas this node owns.
    ///
    /// Zero for everything in the table. A repeater owns one (its
    /// window), an `Either` owns two (one per branch), `List` and `Dyn`
    /// own one each. See [`crate::arena::ArenaMut`] for why the
    /// cross-cutting walks need to know.
    /// Behind `alloc`: the only implementors are `Repeat`, `List` and
    /// `Dyn`, which are the escape hatches T13.4 gates. A tree with no
    /// allocator has no off-table content to reach.
    #[cfg(feature = "alloc")]
    fn arena_count(&self) -> usize {
        0
    }

    /// Arena `a`, for a walk that must reach off-table content.
    #[cfg(feature = "alloc")]
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

    /// This node's selected text, if it has one — `docs/plan-desktop.md`
    /// T14.5.
    ///
    /// What a copy asks the focused node for. `None` is both "I have no
    /// selection" and "I am not the kind of thing that has one", and a
    /// caller cannot tell them apart because it does not need to: there
    /// is nothing to copy either way.
    ///
    /// A borrowed `&str`, so the core never allocates to answer. The
    /// *host* owns the copy into a clipboard, because a clipboard is a
    /// platform, and this crate has no platform.
    fn selection(&self) -> Option<&str> {
        None
    }

    /// Close whatever this node has open, if anything —
    /// `docs/plan-capabilities.md` T15.1.
    ///
    /// Returns whether it closed something, so a caller knows whether
    /// to redraw. `false` from a node with nothing to close, which is
    /// what lets `dispatch_overlaid` offer dismissal to every open
    /// overlay without asking what kind of thing each one is.
    ///
    /// **This is the answer to the one input case `dispatch` could not
    /// express**: a click outside every node. It reaches no widget, so
    /// no widget can react to it; the pass has to.
    fn dismiss(&mut self) -> bool {
        false
    }

    /// Whether a press outside this overlay, which dismisses it, is
    /// **consumed** — M33/T33.3.
    ///
    /// `true` for a menu or a modal: the click that closes a dropdown
    /// must not also press what is under it (`tests/dismiss_click.rs`).
    /// `false` for a *passive* overlay — a tooltip — whose dismissal must
    /// not take the press from the control it was describing. Asked of
    /// the overlay node itself, by its own index, so this is a leaf's
    /// answer and never a wrapper's to forward.
    fn dismissal_consumes_press(&self) -> bool {
        true
    }

    /// Whether this node **owns the arrow keys** while it has focus —
    /// M44/T44.1 (`docs/plan-input-semantics.md` §2.1).
    ///
    /// `false` by default: an arrow on a focused control moves focus
    /// within its parent's group of focusable children, which is what a
    /// radio group, a toolbar or a menu row wants (T15.6). `true` for a
    /// widget that *uses* arrows — a text field's caret, a slider's
    /// value, a divider's position — and then the group rule does not run
    /// at all, whether or not the widget consumes the key. That is the
    /// platform convention: an arrow never leaves a text field, even from
    /// its first character or its first line; Tab and Shift+Tab do.
    ///
    /// Asked of the node at the focused index, like
    /// [`dismissal_consumes_press`](Node::dismissal_consumes_press), so it
    /// is a **leaf's answer and never a wrapper's to forward**. The one
    /// trap is a composite whose `node(i)` resolves to *itself* for an
    /// inner field's index — `NumberInput` does — which must answer for
    /// the field it wraps.
    fn owns_arrows(&self) -> bool {
        false
    }

    /// A number that changes whenever the **items** this node's rows
    /// stand for may have changed — M44/T44.2
    /// (`docs/plan-input-semantics.md` §2.2).
    ///
    /// A click is a press and a release on the same logical item. For a
    /// node without rows that is "the same node"; for a virtualizer a row
    /// index is only an index, and an insertion between press and release
    /// puts a different item under a pointer that never moved. The pass
    /// latches this at the press and compares it at the release, and a
    /// mismatch cancels the click. `0` by default, so every non-repeater
    /// keeps plain `(node, row)` semantics. Asked of the hit's table node,
    /// which for a row is the virtualizer itself.
    fn row_generation(&self) -> u64 {
        0
    }

    /// The pointer shape over template node `inner.node` of one of this
    /// node's rows — M44/T44.8.
    ///
    /// A row is not in the main table, so the host's style array has no
    /// entry for a `Button` inside a list row, and the cursor over it read
    /// as the list's own (R15-F7). `None` by default; a virtualizer
    /// answers from its rows' resolved styles. Asked of the hit's table
    /// node, like [`row_generation`](Node::row_generation).
    fn inner_cursor(&self, _inner: crate::hit::Inner) -> Option<crate::style::Cursor> {
        None
    }

    /// One step of an input-method composition (T14.8).
    ///
    /// `false` from a node that does not edit text, which is what lets
    /// `Host::compose` route to the focused node without asking what
    /// kind of thing it is.
    ///
    /// **The answer is also the mark** (M41/T41.1): `Host::compose`
    /// marks this node's subtree when it returns `true`, and nothing
    /// otherwise. A node that changes and answers `false` is laid out
    /// only by a frame something else marked, so under `Boundaries` it
    /// paints stale.
    fn compose(&mut self, c: crate::event::Composition<'_>) -> bool {
        let _ = c;
        false
    }

    /// Take a pasted string **whole**, as one edit — M45/T45.7.
    ///
    /// `Some(n)` from a node that edits text, where `n` is how many
    /// characters it **kept** — its own newline policy applied (a
    /// multi-line editor normalises line endings and keeps them, a
    /// single-line one strips them) and whatever its storage refused left
    /// out. `None` from anything else, and `Host::paste_with` then falls
    /// back to delivering the string as typing, one event per character
    /// with each line ending as `Key::Enter`, so a custom widget keeps
    /// working.
    ///
    /// Asked of `node_mut(i)` at the focused index, which resolves to the
    /// widget that owns the index — so a composite whose editable part is
    /// a node of its own (`SearchField`, `Combobox`, `RichTextEditor`)
    /// needs nothing. **A wrapper that shares its child's index must
    /// forward this**, as it forwards `compose` and `selection`: the
    /// provided `None` is right for a leaf and wrong for a wrapper, which
    /// would silently turn every paste into it back into typing
    /// (`NumberInput`'s shape, R10-F13 and R10-F15).
    ///
    /// **The answer is also the mark**, as for `compose`: `Some(n)` with
    /// `n > 0` marks this node's subtree.
    fn paste(&mut self, text: &str) -> Option<usize> {
        let _ = text;
        None
    }

    /// Whether this node derives how much of its content to materialize
    /// from the constraints it is given.
    ///
    /// `Repeat` does; nothing else does. It exists so
    /// `ScrollView` can refuse to contain
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

    /// Whether this container places its children **on top of each
    /// other** rather than beside them — M11/T11.6.
    ///
    /// The audit's `W1004` (sibling overlap) exists because overlapping
    /// siblings are normally a layout bug. In a `stack` they are the
    /// entire point, so a code that fired on every stack would be noise,
    /// and noise is how a diagnostic stops being read. This is the
    /// distinction, declared by the container rather than guessed from
    /// the geometry — geometry cannot tell an intended overlap from an
    /// accidental one.
    ///
    /// Answered by `#[widget(stack)]`'s generated impl and by nothing
    /// else. Not behind `observe`: a lint that only exists in dev builds
    /// still has to agree with a release build about what the tree is.
    fn overlaps_children(&self) -> bool {
        false
    }

    /// Measure under `limits` **without disturbing any memo, or anything
    /// paint reads**.
    ///
    /// The ambient audit probes every leaf twice per frame, at 1e6 and
    /// 2e6 px, to decide whether it is sized by its own content. Doing
    /// that through `layout` left each `Label`'s measurement cache keyed
    /// on the probe's constraints rather than the frame's, so the memo
    /// missed on every subsequent frame — measured: 0 text measurements
    /// per idle frame without the agent attached, 400 of 400 with it
    /// (`docs/plan-review-fixes.md` F2.1).
    ///
    /// **A widget whose `layout` writes state that paint reads must
    /// override this** — M41/T41.4, R1-F5. `TextInput` and `TextArea`
    /// computed their scroll offset in `layout` from the limits they were
    /// given, so the probe's 1e6 px box reset a scrolled field to zero;
    /// under `Everything` the next layout put it back, and under the
    /// `Boundaries` default an unrelated hover left the field unmarked
    /// and paint drew the probe's scroll. M41/T41.7 found five more with
    /// the live shadow oracle in the showcase: `Pagination` and `Strip`
    /// cache each label's measured width for paint to centre by,
    /// `RichText` places its words against the offered width, and
    /// `NumberInput` and `Markdown` are **wrappers**, for which this
    /// provided body lays the inner widget out instead of probing it —
    /// the rule that a provided method is wrong for a wrapper.
    /// `audit_is_inert.rs`'s sweep probes the leaf kinds the widget
    /// crate ships, seeded with content, **with a measurer unlike the
    /// layout's** (probing with the same one wrote back the value that
    /// was already there), and compares each leaf's paint before and
    /// after.
    ///
    /// The default *is* `layout`, which is right for a widget whose
    /// layout writes only its `NodeState` row. `Label`, `Repeat`,
    /// `TextInput`, `TextArea`, `Pagination`, `Strip`, `RichText`,
    /// `Markdown` and `NumberInput` override it.
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

    /// **This subtree's accessibility entries** — M35/T35.1
    /// (`docs/plan-observe-cost.md`).
    ///
    /// The default walks `table` through [`Node::node`], which is correct
    /// for any widget and only slower for a type used as a repeated row.
    /// `leaf_common!` and the `StaticTree` derive override it with a walk
    /// that calls each widget's `observe` statically, so a `List`'s or
    /// `Repeat`'s row widget is never coerced to `&dyn Node` and stays
    /// inlinable with `observe` compiled in. `table` is this subtree's
    /// own; indices are local, and `id_of` maps them.
    ///
    /// **Provided, not required**, unlike the plan's first answer: a
    /// provided default that *dropped* a wrapper's children would be
    /// wrong, and this one drops nothing. **And the fallback lives here,
    /// not in the caller**: a caller that coerced in a branch dead for
    /// every widget with a walk still kept `Label::paint` address-taken
    /// (`just observe-inlining`). A default body is compiled only for the
    /// types that use it.
    #[cfg(feature = "observe")]
    fn observe_walk(
        &self,
        table: &[crate::desc::NodeDesc],
        st: &[NodeState],
        id_of: &dyn Fn(usize) -> u64,
        out: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        for i in 0..table.len() {
            crate::observe::emit_node(table, st, i, id_of, out, |n, o| {
                self.node(i).observe(st, i, id_of(i), n, o)
            });
        }
    }

    /// Cheap scalar introspection for the per-frame audit.
    #[cfg(feature = "observe")]
    #[allow(unused_variables)]
    fn stats(&self, out: &mut crate::desc::NodeStats) {}}

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

    /// The kind name a [`crate::style::Selector::Kind`] rule matches.
    ///
    /// The type-level half of [`Node::kind_name`], which forwards here.
    const KIND: &'static str;

    /// This kind's own defaults, before any rule applies.
    ///
    /// The type-level half of [`Node::defaults`], which forwards here —
    /// and the home that method's contract always described. See
    /// [`crate::style::Seed`] for what reading it off an instance cost.
    fn type_defaults() -> Style {
        Style::default()
    }

    /// One [`crate::style::Seed`] per node of this subtree, in
    /// `SUB_TABLE` order — so `SEEDS.len() == NODE_COUNT`.
    ///
    /// **No default**, deliberately. A default correct for a leaf is
    /// silently wrong for a container, and a container that cascades as
    /// `"widget"` with `Style::default()` is precisely the failure this
    /// exists to end. Requiring it makes the compiler name every site.
    const SEEDS: &'static [Seed];
}

/// Copy a child's `SEEDS` into a parent's array being built. The seed
/// counterpart of [`splice_child`] — and simpler, because a seed holds
/// no indices to rebase.
#[doc(hidden)]
pub const fn splice_seeds(dst: &mut [Seed], write_at: usize, src: &[Seed]) {
    let mut j = 0;
    while j < src.len() {
        dst[write_at + j] = src[j];
        j += 1;
    }
}

/// Copy a child's `SUB_TABLE` into a parent table being built, rebasing
/// indices by `write_at`. The child's root is re-parented to
/// `parent_index`. Const, so derives run it at compile time; runtime
/// assembly (handoff §8) calls the same fn, and since M46/T46.3 also
/// checks its result with [`table_is_well_formed`] in debug builds and
/// refuses more instances than a `u16` child count holds
/// (`assemble::concat_table`).
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
///
/// **What it guarantees** is what [`crate::style::subtree_range`] and
/// every child-chain walk assume: the table is DFS pre-order — node
/// `j`'s parent is `j - 1` or an ancestor of `j - 1`, so a subtree is one
/// contiguous range — each parent's children are linked forward from
/// `first_child = parent + 1`, and **a last child's `next_sibling` is
/// 0**. Until M41/T41.6 it checked only that a parent *precedes* its
/// child, and let a last child point at "a later node", which accepted
/// four tables whose `subtree_range` is wrong or whose sibling chain
/// runs into an uncle (`crates/lumen-static/tests/table_invariants.rs`,
/// R2-F5). The reach of that gap grew when the `Boundaries` default made
/// `subtree_range` decide what a precise mark covers.
#[doc(hidden)]
pub const fn table_is_well_formed(t: &[NodeDesc]) -> bool {
    if t.is_empty() {
        return false;
    }
    if t[0].parent != crate::desc::NO_PARENT {
        return false;
    }

    // DFS pre-order: node j's parent is j - 1 or one of j - 1's
    // ancestors. "Parent precedes child" is weaker — it accepts a
    // child stored after its uncle, which splits its parent's subtree
    // into two ranges. The walk up is bounded by the depth, and each
    // step moves strictly toward the root, so it terminates on any
    // table that passed the precedes check on the way.
    // The declared child counts add up to "every node but the root is
    // somebody's child".
    let mut declared = 0usize;
    let mut i = 0;
    while i < t.len() {
        if i > 0 {
            let p = t[i].parent as usize;
            if p >= i {
                return false;
            }
            let mut a = i - 1;
            while a != p {
                if a == 0 {
                    return false;
                }
                a = t[a].parent as usize;
            }
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
                } else if next != 0 {
                    // The last child terminates with 0. The allowance
                    // this replaced — "or points at a later node, its
                    // parent's own next sibling" — let a chain walk run
                    // on into the uncle (M41/T41.6).
                    return false;
                }
                k = next;
            }
        }
        n += 1;
    }
    true
}
