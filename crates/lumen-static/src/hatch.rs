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

use crate::cx::LayoutCx;
use crate::display::DisplayList;
use crate::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use crate::hit::Hit;
use crate::style::Style;
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
impl<W: Widget> Widget for Option<W> {
    const NODE_COUNT: usize = W::NODE_COUNT;
    const DEPTH: usize = W::DEPTH;
    const IS_RIGID: bool = W::IS_RIGID;
    const SUB_TABLE: &'static [NodeDesc] = W::SUB_TABLE;
}

impl<W: Widget> Node for Option<W> {
    fn kind_name(&self) -> &'static str {
        match self {
            Some(w) => w.kind_name(),
            None => "option",
        }
    }

    fn defaults(&self) -> Style {
        match self {
            Some(w) => w.defaults(),
            None => Style::default(),
        }
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

    fn paint(&self, st: &[NodeState], i: usize, out: &mut DisplayList) {
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
    fn handler(
        &self,
        sel: crate::event::NodeSel,
    ) -> Option<fn(&mut S, &crate::event::EventCtx)> {
        self.as_ref().and_then(|w| w.handler(sel))
    }
}

/// `Either<A, B>` — one of two subtrees, **both resident**.
///
/// A struct rather than a Rust `enum`, deliberately: §6 wants the hidden
/// branch to keep its state, so toggling a tab strip does not lose a
/// scroll position. An enum would drop it.
///
/// **Deviation from §6 — and the reason given for it was wrong.** The
/// handoff expects `NODE_COUNT = A + B`, both branches spliced into the
/// parent's table. This impl contributes **one** table node and keeps
/// its branches off-table with their own state arenas (`Repeat`'s
/// mechanism), and the original comment here said a merged table "needs
/// `generic_const_exprs`". It does not. `[NodeDesc; A::N + B::N]` *as a
/// type* does — but `SUB_TABLE` is `&'static [NodeDesc]`, a slice, so
/// the length is a value: build `[NodeDesc; MAX]` in const-eval with
/// `splice_child`, `split_at(N)`, and `assert!` the bound. Verified on
/// stable 1.94 during the 2026-09-02 review, nested three deep, spliced
/// into a derived struct with tree-absolute indices; exceeding `MAX` is
/// a compile error.
///
/// What this shape costs for that mistaken reason: branch internals get
/// branch-relative agent ids instead of table indices, and each `Either`
/// holds four heap `Vec`s — two state arenas, two style arenas — which
/// a merged table would not need. The decision is **REOPENED** in
/// `docs/plan-architecture-v2.md` M4 and `docs/either-reopened.md`;
/// this impl stands until it is re-decided.
pub struct Either<A: Widget, B: Widget> {
    pub a: A,
    pub b: B,
    showing: Side,
    a_state: Vec<NodeState>,
    b_state: Vec<NodeState>,
    a_style: Vec<Style>,
    b_style: Vec<Style>,
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
        Either {
            a,
            b,
            showing: Side::A,
            a_state: vec![NodeState::default(); A::NODE_COUNT],
            b_state: vec![NodeState::default(); B::NODE_COUNT],
            a_style: Vec::new(),
            b_style: Vec::new(),
        }
    }

    pub fn showing(&self) -> Side {
        self.showing
    }

    /// Toggle. Free: both branches are resident and keep their state.
    pub fn show(&mut self, side: Side) {
        self.showing = side;
    }

    /// Layout state of the active branch, for tests and tooling.
    pub fn active_state(&self, node: usize) -> NodeState {
        match self.showing {
            Side::A => self.a_state[node],
            Side::B => self.b_state[node],
        }
    }

    fn ensure_styles(&mut self) {
        if self.a_style.is_empty() {
            self.a_style = (0..A::NODE_COUNT).map(|j| self.a.node(j).defaults()).collect();
        }
        if self.b_style.is_empty() {
            self.b_style = (0..B::NODE_COUNT).map(|j| self.b.node(j).defaults()).collect();
        }
    }
}

impl<A: Widget, B: Widget> Widget for Either<A, B> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1 + if A::DEPTH > B::DEPTH { A::DEPTH } else { B::DEPTH };
    const IS_RIGID: bool = A::IS_RIGID && B::IS_RIGID;
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::CONTAINER)];
}

impl<A: Widget, B: Widget> Node for Either<A, B> {
    fn kind_name(&self) -> &'static str {
        "either"
    }

    fn defaults(&self) -> Style {
        match self.showing {
            Side::A => self.a.defaults(),
            Side::B => self.b.defaults(),
        }
    }

    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        _style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        self.ensure_styles();
        let size = match self.showing {
            Side::A => self.a.layout(cx, limits, &self.a_style, &mut self.a_state, 0),
            Side::B => self.b.layout(cx, limits, &self.b_style, &mut self.b_state, 0),
        };
        st[i].constraints = limits;
        st[i].size = size;
        size
    }

    fn place(&mut self, at: Point, _style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = at;
        match self.showing {
            Side::A => self.a.place(at, &self.a_style, &mut self.a_state, 0),
            Side::B => self.b.place(at, &self.b_style, &mut self.b_state, 0),
        }
    }

    fn paint(&self, _st: &[NodeState], _i: usize, out: &mut DisplayList) {
        match self.showing {
            Side::A => self.a.paint(&self.a_state, 0, out),
            Side::B => self.b.paint(&self.b_state, 0, out),
        }
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit> {
        if !crate::hit::contains(&st[i], x, y) {
            return None;
        }
        let inner = match self.showing {
            Side::A => self.a.hit(&self.a_state, 0, x, y),
            Side::B => self.b.hit(&self.b_state, 0, x, y),
        };
        Some(Hit {
            node: i as u32,
            // Branch nodes are off-table, so they are addressed the way
            // repeater rows are: row 0 for A, row 1 for B.
            inner: inner.map(|h| crate::hit::Inner {
                row: match self.showing {
                    Side::A => 0,
                    Side::B => 1,
                },
                node: h.node,
            }),
            local: inner.map_or(
                Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
                |h| h.local,
            ),
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
        // Same synthetic id scheme as the repeater: the branch is "row"
        // 0 or 1, so ids are stable across toggles.
        let row: u64 = match self.showing {
            Side::A => 0,
            Side::B => 1,
        };
        let id_of = |t: usize| (1u64 << 63) | (id << 44) | (row << 12) | t as u64;
        match self.showing {
            Side::A => {
                crate::observe::emit_subtree(&self.a, &self.a_state, &id_of, extra);
            }
            Side::B => {
                crate::observe::emit_subtree(&self.b, &self.b_state, &id_of, extra);
            }
        }
        node.push_child(accesskit::NodeId(id_of(0)));
    }
}

impl<A, B, S> crate::event::Handlers<S> for Either<A, B>
where
    A: Widget + crate::event::Handlers<S>,
    B: Widget + crate::event::Handlers<S>,
{
    fn handler(
        &self,
        sel: crate::event::NodeSel,
    ) -> Option<fn(&mut S, &crate::event::EventCtx)> {
        let (row, node) = sel.inner?;
        let inner = crate::event::NodeSel { i: node as usize, inner: None };
        match (self.showing, row) {
            (Side::A, 0) => self.a.handler(inner),
            (Side::B, 1) => self.b.handler(inner),
            _ => None,
        }
    }
}

/// `Vec<W>` as a widget — plan T4.3.
///
/// The non-virtualized sibling of `Repeat`: every element is
/// materialized, and arity comes from the data. One table node, like
/// `Either`, so `NODE_COUNT` stays independent of how many items there
/// are — §6's scaling law applied to a plain list.
pub struct List<W: Widget> {
    pub items: Vec<W>,
    /// Gap between items along the column.
    pub spacing: f32,
    states: Vec<NodeState>,
    style: Vec<Style>,
}

impl<W: Widget> Default for List<W> {
    fn default() -> Self {
        List { items: Vec::new(), spacing: 0.0, states: Vec::new(), style: Vec::new() }
    }
}

impl<W: Widget> List<W> {
    pub fn new(items: Vec<W>) -> Self {
        List { items, spacing: 0.0, states: Vec::new(), style: Vec::new() }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Layout state of item `k`'s node `node`.
    pub fn item_state(&self, k: usize, node: usize) -> NodeState {
        self.states[k * W::NODE_COUNT + node]
    }

    fn ensure(&mut self) {
        let want = self.items.len() * W::NODE_COUNT;
        if self.states.len() != want {
            self.states.resize(want, NodeState::default());
        }
        if self.style.is_empty() && !self.items.is_empty() {
            self.style = (0..W::NODE_COUNT).map(|j| self.items[0].node(j).defaults()).collect();
        }
    }

    fn slice(&self, k: usize) -> &[NodeState] {
        &self.states[k * W::NODE_COUNT..(k + 1) * W::NODE_COUNT]
    }
}

impl<W: Widget> Widget for List<W> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1 + W::DEPTH;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::CONTAINER)];
}

impl<W: Widget> Node for List<W> {
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
        let List { items, states, style: istyle, .. } = self;
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
        let List { items, states, style: istyle, .. } = self;
        let mut y = at.y;
        for (k, item) in items.iter_mut().enumerate() {
            item.place(Point { x: at.x, y }, istyle, &mut states[k * n..(k + 1) * n], 0);
            y += states[k * n].size.h + spacing;
        }
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut DisplayList) {
        // Items live in an arena this node places; `resolve_self` can
        // size the node below their sum (a `Fixed` height), and `hit`
        // gates on that size. Clip to the same rect so paint agrees —
        // the invariant is scoped to the clipping ancestor
        // (`docs/text-and-scrolling-decisions.md` A1).
        out.push_clip(crate::ops::rect_of(&st[i]));
        for k in 0..self.items.len() {
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
            let id_of = move |t: usize| (1u64 << 63) | (id << 44) | ((k as u64) << 12) | t as u64;
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

impl<W, S> crate::event::Handlers<S> for List<W>
where
    W: Widget + crate::event::Handlers<S>,
{
    fn handler(
        &self,
        sel: crate::event::NodeSel,
    ) -> Option<fn(&mut S, &crate::event::EventCtx)> {
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
pub struct Dyn {
    inner: Box<dyn Node>,
    table: &'static [NodeDesc],
    states: Vec<NodeState>,
    style: Vec<Style>,
}

impl Dyn {
    pub fn new<W: Widget + 'static>(w: W) -> Self {
        let style = (0..W::NODE_COUNT).map(|j| w.node(j).defaults()).collect();
        Dyn {
            inner: Box::new(w),
            table: W::SUB_TABLE,
            states: vec![NodeState::default(); W::NODE_COUNT],
            style,
        }
    }

    /// Swap the island's content at runtime — the point of the hatch.
    pub fn replace<W: Widget + 'static>(&mut self, w: W) {
        *self = Dyn::new(w);
    }

    pub fn inner_state(&self, node: usize) -> NodeState {
        self.states[node]
    }

    pub fn node_count(&self) -> usize {
        self.table.len()
    }
}

impl Default for Dyn {
    fn default() -> Self {
        Dyn::new(crate::ops::Spacer)
    }
}

impl Widget for Dyn {
    const NODE_COUNT: usize = 1;
    /// The island's content is opaque at compile time, so its depth
    /// cannot be counted. It is charged the maximum, which is what keeps
    /// `MAX_DEPTH` a sound bound rather than an optimistic one.
    const DEPTH: usize = 2;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::CONTAINER)];
}

impl Node for Dyn {
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
        let size = self.inner.layout(cx, limits, &self.style, &mut self.states, 0);
        let size = crate::ops::resolve_self(limits, &style[i], size);
        st[i].constraints = limits;
        st[i].size = size;
        size
    }

    fn place(&mut self, at: Point, _style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = at;
        self.inner.place(at, &self.style, &mut self.states, 0);
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut DisplayList) {
        // Same shape as `List`: the island's content may exceed the box
        // `resolve_self` gave it; `hit` stops at the box, so paint must.
        out.push_clip(crate::ops::rect_of(&st[i]));
        self.inner.paint(&self.states, 0, out);
        out.pop_clip();
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit> {
        if !crate::hit::contains(&st[i], x, y) {
            return None;
        }
        let inner = self.inner.hit(&self.states, 0, x, y);
        Some(Hit {
            node: i as u32,
            inner: inner.map(|h| crate::hit::Inner { row: 0, node: h.node }),
            local: inner.map_or(
                Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
                |h| h.local,
            ),
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
        let id_of = |t: usize| (1u64 << 63) | (id << 44) | t as u64;
        crate::observe::emit_table(self.table, &*self.inner, &self.states, &id_of, extra);
        node.push_child(accesskit::NodeId(id_of(0)));
    }
}

/// An island's content cannot carry typed handlers: its type is erased,
/// so it cannot prove `Handlers<S>` for the app's state. Islands are
/// display content; interactive plugin content needs a message channel
/// (`docs/events-decision.md`'s async seam), which is the honest place
/// for it.
impl<S> crate::event::Handlers<S> for Dyn {
    fn handler(
        &self,
        _sel: crate::event::NodeSel,
    ) -> Option<fn(&mut S, &crate::event::EventCtx)> {
        None
    }
}
