//! The §6 dynamism escape hatches — plan M4.
//!
//! All four were marked **DECIDED** in the handoff and none existed;
//! only `Repeat` had been built. Every non-trivial UI needs conditional
//! subtrees, so this is load-bearing rather than nice to have.
//!
//! They share one mechanism: **visibility**. `Option` and `Either` both
//! reduce to "these table entries exist but this subtree is inactive
//! this frame". Inactive subtrees take zero size and are skipped by
//! paint and hit testing; they keep their state, which §6 correctly
//! calls a *win* — toggling is free and the hidden branch remembers
//! where it was.

// `alloc`, not the std prelude: this crate is `no_std` when the
// `std` feature is off (M13/T13.1), and absent entirely without the
// `alloc` feature (T13.4).
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::cx::LayoutCx;
use crate::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use crate::display::Painter;
use crate::hit::Hit;
use crate::style::{Seed, Style};
use crate::widget::{Node, Widget};

/// Lay out a subtree as absent: zero size, no descent.
#[inline]
fn hide(st: &mut [NodeState], i: usize, limits: Constraints, count: usize) -> Size {
    for s in st.iter_mut().skip(i).take(count) {
        s.size = Size::default();
        s.constraints = limits;
    }
    Size::default()
}

/// `Option<W>` — present or not, without changing `NODE_COUNT`.
///
/// The table always has `W`'s entries, so node indices, agent ids and
/// provenance are stable across a toggle. That is the property a
/// key-based reconciler has to work for.
impl<W: Widget + Default> Widget for Option<W> {
    const NODE_COUNT: usize = W::NODE_COUNT;
    const DEPTH: usize = W::DEPTH;
    const IS_RIGID: bool = W::IS_RIGID;
    const SUB_TABLE: &'static [NodeDesc] = W::SUB_TABLE;

    /// **The inner kind, present or not.** An `Option` adds no node of
    /// its own — `NODE_COUNT` and `SUB_TABLE` are `W`'s — so there is no
    /// node for an `"option"` kind to name. `kind_name` used to answer
    /// `"option"` while `None`, which made a `Kind` rule match a hidden
    /// subtree and stop matching when it was filled: state-dependent,
    /// which [`crate::style::Seed`] forbids for exactly this reason.
    const KIND: &'static str = W::KIND;

    fn type_defaults() -> Style {
        W::type_defaults()
    }

    const SEEDS: &'static [Seed] = W::SEEDS;
}

impl<W: Widget + Default> Node for Option<W> {
    fn kind_name(&self) -> &'static str {
        <Self as Widget>::KIND
    }

    /// The *inner kind's* defaults, whether or not there is a value.
    ///
    /// `defaults()` feeds the cascade, which runs when a stylesheet is
    /// set — not per frame — so a state-dependent answer is snapshotted
    /// once and never revisited. This returned `Style::default()` while
    /// `None`, so an `Option` empty at construction had its slot styled
    /// `Shrink` permanently: setting it to `Some(Spacer)` later laid the
    /// spacer out at 0x0 instead of filling, with nothing to say why.
    /// Absence is expressed by `layout` hiding the subtree, which is a
    /// per-frame decision and the right home for one.
    /// `docs/plan-review-fixes.md` F3.2.
    fn defaults(&self) -> Style {
        <Self as Widget>::type_defaults()
    }

    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        match self {
            Some(w) => w.layout(cx, limits, style, st, i),
            None => hide(st, i, limits, W::NODE_COUNT),
        }
    }

    fn place(&mut self, at: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        if let Some(w) = self {
            w.place(at, style, st, i);
        } else {
            for s in st.iter_mut().skip(i).take(W::NODE_COUNT) {
                s.pos = at;
            }
        }
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        if let Some(w) = self {
            w.paint(st, i, out);
        }
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit> {
        self.as_ref().and_then(|w| w.hit(st, i, x, y))
    }

    fn node(&self, i: usize) -> &dyn Node {
        match self {
            Some(w) => w.node(i),
            None => self,
        }
    }

    fn node_mut(&mut self, i: usize) -> &mut dyn Node {
        match self {
            Some(w) => w.node_mut(i),
            None => self,
        }
    }

    #[cfg(feature = "observe")]
    fn observe(
        &self,
        st: &[NodeState],
        i: usize,
        id: u64,
        node: &mut accesskit::Node,
        extra: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        match self {
            Some(w) => w.observe(st, i, id, node, extra),
            // An absent branch is announced as hidden rather than
            // silently reported with zero bounds, so an agent can tell
            // "not shown" from "shown but broken".
            None => node.set_hidden(),
        }
    }
}

impl<W, S> crate::event::Handlers<S> for Option<W>
where
    W: Widget + crate::event::Handlers<S>,
{
    fn handler(&self, sel: crate::event::NodeSel) -> Option<fn(&mut S, &crate::event::EventCtx)> {
        self.as_ref().and_then(|w| w.handler(sel))
    }
}

/// `Either<A, B>` — one of two subtrees, **both resident**.
///
/// A struct rather than a Rust `enum`, deliberately: §6 wants the hidden
/// branch to keep its state, so toggling a tab strip does not lose a
/// scroll position. An enum would drop it.
///
/// **Both branches are on the table** — `NODE_COUNT = 1 + A + B`, as §6
/// specified. M4/T4.2 shipped one table node with off-table arenas
/// instead, on the stated ground that a merged table "needs
/// `generic_const_exprs`". It does not: `[NodeDesc; A::N + B::N]` *as a
/// type* would, but `SUB_TABLE` is a slice, so the length is a value.
/// [`EitherTable`] builds `[NodeDesc; MAX_EITHER_NODES]` in const-eval
/// and hands out `split_at(N).0`; exceeding the bound is a compile
/// error. Re-decided 2026-09-04 (`docs/either-reopened.md`, Option A).
///
/// What the merged table buys, and the old shape could not:
/// - branch internals get **tree-absolute** indices, so an agent
///   selector, a lint finding and a `#[widget_id]` inside a branch all
///   address them the way they address anything else;
/// - a stylesheet reaches each branch node's own style slot, rather than
///   both branches sharing one;
/// - **no heap at all**: the old shape held four `Vec`s per `Either`
///   (two state arenas, two style arenas).
///
/// What it costs: `MAX_EITHER_NODES × size_of::<NodeDesc>()` of rodata
/// per distinct `Either<A, B>`, whatever the branches' real size.
/// Measured in `docs/either-reopened.md`: ~3 KB at 64 nodes.
///
/// The hidden branch is hidden the way an empty [`Option`] is: `layout`
/// zeroes its subtree every frame, so it holds no stale geometry for
/// `paint`, `hit`, the journal or the audit to read.
pub struct Either<A: Widget, B: Widget> {
    pub a: A,
    pub b: B,
    showing: Side,
}

/// Upper bound on `1 + A::NODE_COUNT + B::NODE_COUNT` for one
/// [`Either`].
///
/// The padding a merged const table costs. Exceeding it is a compile
/// error naming the type, not a runtime surprise. Raising it costs
/// rodata in every distinct `Either` instantiation — measured at ~3 KB
/// per instantiation at 64, and +138 KB across a probe's instantiations
/// when raised to 1024 (`docs/either-reopened.md`).
pub const MAX_EITHER_NODES: usize = 64;

/// The merged table for one `Either<A, B>`, built at compile time.
///
/// A helper type rather than a `const` inside the `Widget` impl because
/// the array must be an *associated* const to be generic in `A` and `B`
/// and still promotable to `&'static`.
pub struct EitherTable<A, B>(core::marker::PhantomData<(A, B)>);

impl<A: Widget, B: Widget> EitherTable<A, B> {
    const ARRAY: [NodeDesc; MAX_EITHER_NODES] = {
        assert!(
            1 + A::NODE_COUNT + B::NODE_COUNT <= MAX_EITHER_NODES,
            "Either<A, B> exceeds MAX_EITHER_NODES; raise it or split the branches"
        );
        let mut t = [NodeDesc::EMPTY; MAX_EITHER_NODES];
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 2, NodeDesc::CONTAINER);
        crate::widget::splice_child(&mut t, 1, A::SUB_TABLE, 0);
        crate::widget::splice_child(&mut t, 1 + A::NODE_COUNT, B::SUB_TABLE, 0);
        // A's root is B's root's previous sibling; B's is the last.
        t[1].next_sibling = (1 + A::NODE_COUNT) as u32;
        t
    };

    /// The used prefix.
    pub const SLICE: &'static [NodeDesc] =
        Self::ARRAY.split_at(1 + A::NODE_COUNT + B::NODE_COUNT).0;

    /// The seed counterpart of [`Self::ARRAY`], spliced the same way.
    const SEED_ARRAY: [Seed; MAX_EITHER_NODES] = {
        let mut s = [Seed::WIDGET; MAX_EITHER_NODES];
        s[0] = Seed::new("either", crate::style::plain_style, crate::style::Fields::NONE);
        crate::widget::splice_seeds(&mut s, 1, A::SEEDS);
        crate::widget::splice_seeds(&mut s, 1 + A::NODE_COUNT, B::SEEDS);
        s
    };
    pub const SEED_SLICE: &'static [Seed] =
        Self::SEED_ARRAY.split_at(1 + A::NODE_COUNT + B::NODE_COUNT).0;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    A,
    B,
}

impl<A: Widget + Default, B: Widget + Default> Default for Either<A, B> {
    fn default() -> Self {
        Either::new(A::default(), B::default())
    }
}

impl<A: Widget, B: Widget> Either<A, B> {
    pub fn new(a: A, b: B) -> Self {
        Either { a, b, showing: Side::A }
    }

    /// Table index of branch A's root, relative to this node.
    const A_AT: usize = 1;
    /// Table index of branch B's root, relative to this node.
    const B_AT: usize = 1 + A::NODE_COUNT;

    pub fn showing(&self) -> Side {
        self.showing
    }

    /// Toggle. Free: both branches are resident and keep their state.
    pub fn show(&mut self, side: Side) {
        self.showing = side;
    }

    /// Where the active branch's root sits, relative to this node.
    ///
    /// `1` for A, `1 + A::NODE_COUNT` for B — the same arithmetic
    /// `layout` uses, exposed because a caller holding a `NodeState`
    /// slice needs it to find the branch's geometry.
    pub fn active_at(&self) -> usize {
        match self.showing {
            Side::A => Self::A_AT,
            Side::B => Self::B_AT,
        }
    }

    /// Layout state of a node of the active branch, `node` being an
    /// index within that branch's own subtree.
    ///
    /// Takes the table slice now, where the arena version needed none:
    /// branch state lives in the caller's array since 2026-09-04.
    pub fn active_state(&self, st: &[NodeState], i: usize, node: usize) -> NodeState {
        st[i + self.active_at() + node]
    }
}

impl<A: Widget, B: Widget> Widget for Either<A, B> {
    const NODE_COUNT: usize = 1 + A::NODE_COUNT + B::NODE_COUNT;
    const DEPTH: usize = 1 + if A::DEPTH > B::DEPTH { A::DEPTH } else { B::DEPTH };
    const IS_RIGID: bool = A::IS_RIGID && B::IS_RIGID;
    const SUB_TABLE: &'static [NodeDesc] = EitherTable::<A, B>::SLICE;

    const KIND: &'static str = "either";

    const SEEDS: &'static [Seed] = EitherTable::<A, B>::SEED_SLICE;

    /// A plain container's defaults.
    ///
    /// Until 2026-09-04 this returned branch A's, because both branches
    /// shared one style slot and `defaults()` had to name a kind. With
    /// both branches on the table each node has its own slot and answers
    /// for itself, so this node answers for itself too.
    fn type_defaults() -> Style {
        Style::default()
    }
}

impl<A: Widget, B: Widget> Node for Either<A, B> {
    fn kind_name(&self) -> &'static str {
        <Self as Widget>::KIND
    }

    fn defaults(&self) -> Style {
        <Self as Widget>::type_defaults()
    }

    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        // The hidden branch is zeroed every frame, not merely skipped:
        // stale geometry left in the table is what `paint`, `hit`, the
        // journal and the audit would otherwise read, and an `Option`
        // that only zeroed its own node put a hidden panel on screen at
        // last frame's position (`docs/plan-review-fixes.md` F1.6).
        let size = match self.showing {
            Side::A => {
                hide(st, i + Self::B_AT, limits, B::NODE_COUNT);
                self.a.layout(cx, limits, style, st, i + Self::A_AT)
            }
            Side::B => {
                hide(st, i + Self::A_AT, limits, A::NODE_COUNT);
                self.b.layout(cx, limits, style, st, i + Self::B_AT)
            }
        };
        st[i].constraints = limits;
        st[i].size = size;
        size
    }

    fn place(&mut self, at: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = at;
        match self.showing {
            Side::A => self.a.place(at, style, st, i + Self::A_AT),
            Side::B => self.b.place(at, style, st, i + Self::B_AT),
        }
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        match self.showing {
            Side::A => self.a.paint(st, i + Self::A_AT, out),
            Side::B => self.b.paint(st, i + Self::B_AT, out),
        }
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit> {
        if !crate::hit::contains(&st[i], x, y) {
            return None;
        }
        // The hidden branch cannot be hit: its subtree is zeroed, so
        // `contains` rejects it — but ask only the showing one anyway,
        // so the answer does not depend on that.
        let inner = match self.showing {
            Side::A => self.a.hit(st, i + Self::A_AT, x, y),
            Side::B => self.b.hit(st, i + Self::B_AT, x, y),
        };
        inner.or(Some(Hit {
            node: i as u32,
            inner: None,
            local: Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
        }))
    }

    /// Delegate by index range. Branch nodes are ordinary table nodes
    /// now, so an agent selector, a lint finding and a stylesheet rule
    /// all reach them without a special case.
    fn node(&self, i: usize) -> &dyn Node {
        if i == 0 {
            self
        } else if i < Self::B_AT {
            self.a.node(i - Self::A_AT)
        } else {
            self.b.node(i - Self::B_AT)
        }
    }

    fn node_mut(&mut self, i: usize) -> &mut dyn Node {
        if i == 0 {
            self
        } else if i < Self::B_AT {
            self.a.node_mut(i - Self::A_AT)
        } else {
            self.b.node_mut(i - Self::B_AT)
        }
    }
}

impl<A, B, S> crate::event::Handlers<S> for Either<A, B>
where
    A: Widget + crate::event::Handlers<S>,
    B: Widget + crate::event::Handlers<S>,
{
    /// Delegate by index range, like [`Node::node`].
    ///
    /// Branch nodes are ordinary table nodes since 2026-09-04, so a
    /// `Hit` names one directly and no `(row, node)` pair is involved.
    /// The showing branch is not checked here: `hit` cannot produce an
    /// index inside the hidden one, because its subtree is zeroed.
    fn handler(&self, sel: crate::event::NodeSel) -> Option<fn(&mut S, &crate::event::EventCtx)> {
        if sel.i == 0 {
            return None;
        }
        if sel.i < 1 + A::NODE_COUNT {
            self.a.handler(crate::event::NodeSel { i: sel.i - 1, inner: sel.inner })
        } else {
            self.b
                .handler(crate::event::NodeSel { i: sel.i - (1 + A::NODE_COUNT), inner: sel.inner })
        }
    }
}

/// `Vec<W>` as a widget — plan T4.3.
///
/// The non-virtualized sibling of `Repeat`: every element is
/// materialized, and arity comes from the data. One table node — unlike
/// `Either`, whose branch count is a compile-time two and which
/// therefore merges both into its table (T8.0) — so `NODE_COUNT` stays
/// independent of how many items there are, which is §6's scaling law
/// applied to a plain list.
///
/// **Requires the `alloc` feature** (default-on). This is one of the
/// escape hatches M13/T13.4 moved behind it, so that "what does
/// dynamism cost me" is answerable in the type system rather than in
/// prose: a `--no-default-features` build simply does not have it, and
/// the compiler names the line that wanted one.
#[cfg(feature = "alloc")]
pub struct List<W: Widget> {
    pub items: Vec<W>,
    /// Gap between items along the column.
    pub spacing: f32,
    arena: crate::arena::Arena,
}

#[cfg(feature = "alloc")]
impl<W: Widget> Default for List<W> {
    fn default() -> Self {
        List { items: Vec::new(), spacing: 0.0, arena: crate::arena::Arena::new(W::NODE_COUNT) }
    }
}

#[cfg(feature = "alloc")]
impl<W: Widget> List<W> {
    pub fn new(items: Vec<W>) -> Self {
        List { items, spacing: 0.0, arena: crate::arena::Arena::new(W::NODE_COUNT) }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Layout state of item `k`'s node `node`.
    pub fn item_state(&self, k: usize, node: usize) -> NodeState {
        self.arena.node_state(k, node)
    }

    fn ensure(&mut self) {
        // Disjoint field borrows: `items` shared, `arena` exclusive.
        let List { items, arena, .. } = self;
        match items.first() {
            // One probe styles every item, which is sound only because
            // `Node::defaults` depends on the type rather than on this
            // instance's state (F3.2).
            Some(_) => arena.ensure(items.len(), W::SEEDS),
            None => arena.clear_instances(),
        }
    }

    fn slice(&self, k: usize) -> &[NodeState] {
        self.arena.slice(k)
    }
}

#[cfg(feature = "alloc")]
impl<W: Widget> Widget for List<W> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1 + W::DEPTH;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::CONTAINER)];

    const KIND: &'static str = "list";

    /// **One seed, for the list's own node.** Its rows are off-table —
    /// they live in an arena the main cascade cannot reach — and are
    /// styled by `cascade_arenas` from a template instance instead.
    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, crate::style::Fields::of(&[
            crate::style::Fields::WIDTH,
            crate::style::Fields::HEIGHT,
            crate::style::Fields::CURSOR,
            crate::style::Fields::VISIBLE,
        ]))];
}

#[cfg(feature = "alloc")]
impl<W: Widget> Node for List<W> {
    fn defaults(&self) -> Style {
        <Self as Widget>::type_defaults()
    }

    fn arena_count(&self) -> usize {
        1
    }

    fn arena_mut(&mut self, a: usize) -> Option<crate::arena::ArenaMut<'_>> {
        if a != 0 {
            return None;
        }
        let List { items, arena, .. } = self;
        let t = items.first()?;
        arena.ensure_style(W::SEEDS);
        let _ = t;
        Some(crate::arena::ArenaMut {
            table: W::SUB_TABLE,
            style: arena.style_mut(),
            seeds: W::SEEDS,
        })
    }

    fn kind_name(&self) -> &'static str {
        "list"
    }

    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        self.ensure();
        let n = W::NODE_COUNT;
        let spacing = self.spacing;
        let List { items, arena, .. } = self;
        let (istyle, states) = arena.parts();
        let last = items.len().saturating_sub(1);
        let mut sum = 0.0f32;
        let mut mx = 0.0f32;
        for (k, item) in items.iter_mut().enumerate() {
            let cc = Constraints {
                min_w: 0.0,
                max_w: limits.max_w,
                min_h: 0.0,
                max_h: (limits.max_h - sum).max(0.0),
            };
            let s = item.layout(cx, cc, istyle, &mut states[k * n..(k + 1) * n], 0);
            sum += s.h + if k < last { spacing } else { 0.0 };
            mx = mx.max(s.w);
        }
        let size = crate::ops::resolve_self(limits, &style[i], Size { w: mx, h: sum });
        st[i].constraints = limits;
        st[i].size = size;
        size
    }

    fn place(&mut self, at: Point, _style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = at;
        let n = W::NODE_COUNT;
        let spacing = self.spacing;
        let List { items, arena, .. } = self;
        let (istyle, states) = arena.parts();
        let mut y = at.y;
        for (k, item) in items.iter_mut().enumerate() {
            item.place(Point { x: at.x, y }, istyle, &mut states[k * n..(k + 1) * n], 0);
            y += states[k * n].size.h + spacing;
        }
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        // Items live in an arena this node places; `resolve_self` can
        // size the node below their sum (a `Fixed` height), and `hit`
        // gates on that size. Clip to the same rect so paint agrees —
        // the invariant is scoped to the clipping ancestor
        // (`docs/text-and-scrolling-decisions.md` A1).
        out.push_clip(crate::ops::rect_of(&st[i]));
        // **Cull to the surface** — `docs/plan-invalidation.md` T16.12.
        //
        // A `List` materializes every element, deliberately: that is
        // what makes it comparable with `GtkBox` and `QVBoxLayout`, and
        // `Repeat` is the virtualizing one. Materializing is a *layout*
        // property though, and painting ten thousand rows into an
        // 800 px viewport so the renderer can discard 9 960 of them one
        // command at a time is not comparability, it is waste. GTK and
        // Qt both cull in their paint paths.
        //
        // `clip_bounds` is the surface, not the enclosing clip stack —
        // a superset, so this discards less than it could and never
        // more than it may. `None` (a bare `DisplayList`, every test
        // and every benchmark arm that does not opt in) culls nothing
        // and emits exactly what it emitted before.
        let bounds = out.clip_bounds();
        for k in 0..self.items.len() {
            if let Some(b) = bounds {
                let r = crate::ops::rect_of(&self.slice(k)[0]);
                // Half-open in both axes, and `>=` rather than `>`: a
                // row whose bottom edge is exactly the surface's top
                // contributes no pixel.
                if r[2] <= b[0] || r[0] >= b[2] || r[3] <= b[1] || r[1] >= b[3] {
                    continue;
                }
            }
            self.items[k].paint(self.slice(k), 0, out);
        }
        out.pop_clip();
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit> {
        if !crate::hit::contains(&st[i], x, y) {
            return None;
        }
        for k in 0..self.items.len() {
            if let Some(h) = self.items[k].hit(self.slice(k), 0, x, y) {
                return Some(Hit {
                    node: i as u32,
                    inner: Some(crate::hit::Inner { row: k as u64, node: h.node }),
                    local: h.local,
                });
            }
        }
        Some(Hit {
            node: i as u32,
            inner: None,
            local: Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
        })
    }

    fn node(&self, _i: usize) -> &dyn Node {
        self
    }
    fn node_mut(&mut self, _i: usize) -> &mut dyn Node {
        self
    }

    #[cfg(feature = "observe")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        id: u64,
        node: &mut accesskit::Node,
        extra: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::List);
        node.set_size_of_set(self.items.len());
        for k in 0..self.items.len() {
            let id_of = move |t: usize| crate::observe::instance_id(id, k as u64, t);
            crate::observe::emit_subtree(&self.items[k], self.slice(k), &id_of, extra);
            node.push_child(accesskit::NodeId(id_of(0)));
        }
    }

    #[cfg(feature = "observe")]
    fn stats(&self, out: &mut crate::desc::NodeStats) {
        out.items = Some(self.items.len() as u64);
        out.materialized = Some(self.items.len() as u32);
    }
}

#[cfg(feature = "alloc")]
impl<W, S> crate::event::Handlers<S> for List<W>
where
    W: Widget + crate::event::Handlers<S>,
{
    fn handler(&self, sel: crate::event::NodeSel) -> Option<fn(&mut S, &crate::event::EventCtx)> {
        let (row, node) = sel.inner?;
        let item = self.items.get(row as usize)?;
        item.handler(crate::event::NodeSel { i: node as usize, inner: None })
    }
}

/// A `dyn` island — plan T4.5, the last of §6's hatches.
///
/// One table node owning a runtime-provided subtree: the escape hatch
/// for content whose shape is genuinely unknown at compile time
/// (plugins, server-driven fragments). §6 calls this "the one place
/// indirection is paid", and it is a generalization of machinery that
/// already exists — `Repeat` with a single, boxed instance.
///
/// The island captures its content's node count and table at
/// construction, since a `Box<dyn Node>` cannot supply associated
/// consts.
///
/// **Requires the `alloc` feature** (default-on). This is one of the
/// escape hatches M13/T13.4 moved behind it, so that "what does
/// dynamism cost me" is answerable in the type system rather than in
/// prose: a `--no-default-features` build simply does not have it, and
/// the compiler names the line that wanted one.
/// A [`Node`] that can also route a handler for `S` — M23.
///
/// **The supertrait that makes an island interactive.** `Dyn` boxes its
/// content, and a `Box<dyn Node>` has no `Handlers<S>`: the application
/// type is exactly what the erasure erased. So the island boxes *this*
/// instead, and the blanket impl below means any widget that already
/// answers `Handlers<S>` qualifies without writing anything.
///
/// It exists because the alternative shipped and was silently broken:
/// `examples/showcase` put its demos in an island, and every button,
/// checkbox and field in it could be hit-tested, hovered and pressed
/// while no handler ever ran.
#[cfg(feature = "alloc")]
pub trait NodeFor<S>: Node {
    fn handler_for(
        &self,
        sel: crate::event::NodeSel,
    ) -> Option<fn(&mut S, &crate::event::EventCtx)>;
}

#[cfg(feature = "alloc")]
impl<S, W: Node + crate::event::Handlers<S>> NodeFor<S> for W {
    fn handler_for(
        &self,
        sel: crate::event::NodeSel,
    ) -> Option<fn(&mut S, &crate::event::EventCtx)> {
        crate::event::Handlers::handler(self, sel)
    }
}

#[cfg(feature = "alloc")]
pub struct Dyn<S = ()> {
    content_depth: usize,
    inner: Box<dyn NodeFor<S>>,
    table: &'static [NodeDesc],
    /// The boxed content's seeds, captured at construction exactly as
    /// `table` is (M26/T26.10).
    ///
    /// **Type erasure is about the *instance*, not the type.** `Dyn::new`
    /// is generic over `W`, so `W::SEEDS` is in hand at exactly the
    /// moment `W::SUB_TABLE` is — the island only stops knowing `W`
    /// afterwards. Capturing both closes the one case where the cascade
    /// still read a live node for its defaults and its kind, which is
    /// the defect M24 was entirely about, in the last place it survived.
    seeds: &'static [crate::style::Seed],
    /// One instance's worth. `Arena` is keyed on a runtime node count,
    /// which is what lets a type-erased island share it with the other
    /// off-table owners (F4.1).
    arena: crate::arena::Arena,
}

#[cfg(feature = "alloc")]
impl<S> Dyn<S> {
    /// Box a widget as an island.
    ///
    /// # Panics
    ///
    /// If the content's own `DEPTH` does not fit [`MAX_DEPTH`](crate::desc::MAX_DEPTH).
    /// This is
    /// the only depth check an island can make: `W::DEPTH` is known
    /// here, but the island's *own* position in the tree is not, because
    /// the type erasure is the point. See `content_depth`.
    pub fn new<W: Widget + crate::event::Handlers<S> + 'static>(w: W) -> Self {
        assert!(
            W::DEPTH < crate::desc::MAX_DEPTH,
            "Dyn island content is {} deep, over MAX_DEPTH ({})",
            W::DEPTH,
            crate::desc::MAX_DEPTH,
        );
        let mut arena = crate::arena::Arena::new(W::NODE_COUNT);
        arena.ensure(1, W::SEEDS);
        Dyn {
            inner: Box::new(w),
            table: W::SUB_TABLE,
            seeds: W::SEEDS,
            arena,
            content_depth: W::DEPTH,
        }
    }

    /// How deep the boxed content is. Recorded because [`Widget::DEPTH`]
    /// cannot carry it: the ancestors' compile-time arithmetic sees only
    /// the constant below.
    pub fn content_depth(&self) -> usize {
        self.content_depth
    }

    /// Swap the island's content at runtime — the point of the hatch.
    pub fn replace<W: Widget + crate::event::Handlers<S> + 'static>(&mut self, w: W) {
        *self = Dyn::new(w);
    }

    pub fn inner_state(&self, node: usize) -> NodeState {
        self.arena.node_state(0, node)
    }

    pub fn node_count(&self) -> usize {
        self.table.len()
    }
}

/// What an island holds before anything is put in it — M30.
///
/// A zero-sized leaf that lays out to nothing and paints nothing.
/// `Dyn::default()` used `ops::Spacer`, and core cannot name that since
/// the widgets moved to `lumen-static-widgets`.
///
/// **Private, and not a widget.** It exists only so `Dyn<S>` can be
/// `Default` — which the app derives need, because an application
/// struct holding an island derives `Default` like any other. `Dyn`
/// type-erases its content, so nothing outside this module can observe
/// the type, and adding a *user-placeable* widget back into core to
/// solve this would have undone what M30 was for.
///
/// It is `IS_RIGID` and zero-sized where `Spacer` filled its parent:
/// an empty island should occupy nothing, and the old default silently
/// claimed the whole box until something was put in it.
#[cfg(feature = "alloc")]
#[derive(Default)]
struct Vacant;

#[cfg(feature = "alloc")]
impl Widget for Vacant {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];
    const KIND: &'static str = "vacant";
    const SEEDS: &'static [crate::style::Seed] = &[crate::style::Seed::new(
        Self::KIND,
        Self::type_defaults,
        crate::style::Fields::of(&[crate::style::Fields::VISIBLE]),
    )];
}

#[cfg(feature = "alloc")]
impl Node for Vacant {
    fn layout(
        &mut self,
        _cx: &mut crate::cx::LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        crate::ops::record_painted(st, i, limits, Size::default(), &style[i])
    }
    fn paint(&self, _st: &[NodeState], _i: usize, _out: &mut dyn Painter) {}

    // `nohit`: an empty island is not a click target. It has no box to
    // speak of, but saying so is what keeps a `Dyn` that has not been
    // filled from swallowing a press meant for whatever is under it.
    crate::leaf_common!(Vacant, NodeDesc::RIGID, nohit);
}

#[cfg(feature = "alloc")]
impl<S> crate::event::Handlers<S> for Vacant {
    fn handler(&self, _: crate::event::NodeSel) -> Option<fn(&mut S, &crate::EventCtx)> {
        None
    }
}

#[cfg(feature = "alloc")]
impl<S> Default for Dyn<S> {
    fn default() -> Self {
        Dyn::new(Vacant)
    }
}

#[cfg(feature = "alloc")]
impl<S> Widget for Dyn<S> {
    const NODE_COUNT: usize = 1;
    /// **A lower bound, and the one hole in the `MAX_DEPTH` guarantee.**
    ///
    /// An island's content is type-erased, which is its whole purpose, so
    /// its depth cannot reach an ancestor's compile-time arithmetic. This
    /// constant used to be documented as "charged the maximum, which is
    /// what keeps `MAX_DEPTH` a sound bound" — it is a constant `2`, and
    /// the claim was false: a 40-deep tree inside a `Dyn` compiled and
    /// recursed 40 deep (found by review, 2026-09-02).
    ///
    /// Charging the real maximum is not available either: `DEPTH` for a
    /// container is `1 + max(children)`, so `Dyn::DEPTH = MAX_DEPTH`
    /// would make an island fail the derive's assert inside *any*
    /// container, i.e. everywhere it is used.
    ///
    /// What is checked instead, in [`Dyn::new`]: the content's own depth
    /// fits `MAX_DEPTH`. That catches a deep tree being boxed — the
    /// realistic failure — and does not catch a shallow island nested
    /// deeply, which needs a depth counter threaded through `layout` and
    /// is left for the recursion budget work. `docs/plan-review-fixes.md`
    /// F1.1.
    const DEPTH: usize = 2;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::CONTAINER)];

    const KIND: &'static str = "dyn";

    /// **One seed, and the island's content gets none.** Type erasure is
    /// the point: the content's kinds are not knowable to this type, so
    /// they cannot be seeds. `cascade_arenas` styles them from the boxed
    /// instance, which is the one place they *are* reachable.
    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, crate::style::Fields::of(&[
            crate::style::Fields::WIDTH,
            crate::style::Fields::HEIGHT,
            crate::style::Fields::CURSOR,
            crate::style::Fields::VISIBLE,
        ]))];
}

#[cfg(feature = "alloc")]
impl<S> Node for Dyn<S> {
    fn defaults(&self) -> Style {
        <Self as Widget>::type_defaults()
    }

    fn arena_count(&self) -> usize {
        1
    }

    fn arena_mut(&mut self, a: usize) -> Option<crate::arena::ArenaMut<'_>> {
        if a != 0 {
            return None;
        }
        let Dyn { inner, table, seeds, arena, .. } = self;
        let t: &dyn Node = &**inner;
        arena.ensure_style(seeds);
        // **`seeds: Some`, since M26/T26.10.** This said `None` — "the
        // island's content is type-erased, so its kinds cannot reach a
        // const array" — and the erasure happens *after* `Dyn::new`,
        // which is generic over `W` and therefore holds `W::SEEDS` at
        // exactly the moment it holds `W::SUB_TABLE`. With `None` the
        // cascade fell back to reading the live instance, and the lint
        // reported every field as read for an island because it had no
        // mask to consult.
        let _ = t;
        Some(crate::arena::ArenaMut { table, style: arena.style_mut(), seeds })
    }

    fn kind_name(&self) -> &'static str {
        "dyn"
    }

    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let Dyn { inner, arena, .. } = self;
        let (istyle, states) = arena.parts();
        let size = inner.layout(cx, limits, istyle, states, 0);
        let size = crate::ops::resolve_self(limits, &style[i], size);
        st[i].constraints = limits;
        st[i].size = size;
        size
    }

    fn place(&mut self, at: Point, _style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = at;
        let Dyn { inner, arena, .. } = self;
        let (istyle, states) = arena.parts();
        inner.place(at, istyle, states, 0);
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        // Same shape as `List`: the island's content may exceed the box
        // `resolve_self` gave it; `hit` stops at the box, so paint must.
        out.push_clip(crate::ops::rect_of(&st[i]));
        self.inner.paint(self.arena.slice(0), 0, out);
        out.pop_clip();
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit> {
        if !crate::hit::contains(&st[i], x, y) {
            return None;
        }
        let inner = self.inner.hit(self.arena.slice(0), 0, x, y);
        Some(Hit {
            node: i as u32,
            inner: inner.map(|h| crate::hit::Inner { row: 0, node: h.node }),
            local: inner.map_or(Point { x: x - st[i].pos.x, y: y - st[i].pos.y }, |h| h.local),
        })
    }

    fn node(&self, _i: usize) -> &dyn Node {
        self
    }
    fn node_mut(&mut self, _i: usize) -> &mut dyn Node {
        self
    }

    #[cfg(feature = "observe")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        id: u64,
        node: &mut accesskit::Node,
        extra: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        // Row 0: a `Dyn` island has exactly one instance, so the row
        // field is unused rather than absent — one layout, one meaning.
        let id_of = |t: usize| crate::observe::instance_id(id, 0, t);
        crate::observe::emit_table(self.table, &*self.inner, self.arena.slice(0), &id_of, extra);
        node.push_child(accesskit::NodeId(id_of(0)));
    }
}

/// An island's content cannot carry typed handlers: its type is erased,
/// so it cannot prove `Handlers<S>` for the app's state. Islands are
/// display content; interactive plugin content needs a message channel
/// (`docs/events-decision.md`'s async seam), which is the honest place
/// for it.
#[cfg(feature = "alloc")]
/// **Routes into the island** — M23.
///
/// This returned `None` unconditionally until the supertrait above
/// existed, and the comment explaining why was correct about the cause
/// and wrong about it being unavoidable: nothing can be forwarded to a
/// `Box<dyn Node>`, so the island boxes a `dyn NodeFor<S>` instead.
impl<S> crate::event::Handlers<S> for Dyn<S> {
    fn handler(&self, sel: crate::event::NodeSel) -> Option<fn(&mut S, &crate::event::EventCtx)> {
        self.inner.handler_for(sel)
    }
}
