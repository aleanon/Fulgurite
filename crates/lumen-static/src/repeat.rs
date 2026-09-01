//! `Repeat<T>` — the repeater from handoff §6: one main-table entry, a
//! const template, a runtime instance count, only the visible window
//! materialized.
//!
//! > `NODE_COUNT` grows with how much UI you **wrote**, not how much
//! > data you **display**.
//!
//! The window is plain `Vec` fields, not `RefCell`: with recursive
//! traversal `layout` takes `&mut self`, so a repeater mutates its own
//! window directly. Interior mutability existed only because the old
//! flat passes handed ops a *shared* base pointer.

use crate::cx::LayoutCx;
use crate::display::DisplayList;
use crate::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use crate::hit::{contains, Hit, Inner};
use crate::style::{Length, Style};
use crate::widget::{Node, Widget};

pub fn noop_bind<T>(_: &mut T, _: usize) {}

pub struct Repeat<T: Widget + Default + 'static> {
    /// Logical row count — the data size. Never reflected in any table.
    pub count: usize,
    /// Scroll offset in px along the main (vertical) axis.
    pub scroll: f32,
    /// Uniform per-row extent along the main axis.
    pub item_extent: f32,
    /// Configure a recycled template instance for a row. A plain `fn`
    /// pointer: it cannot capture, so a hot reload re-supplies it.
    pub bind: fn(&mut T, usize),
    window: Vec<T>,
    states: Vec<NodeState>,
    first: usize,
    /// Template default styles, built once. Instances are off-table, so
    /// the cascade cannot reach them; this is their style source until
    /// per-row styling exists.
    row_style: Vec<Style>,
}

impl<T: Widget + Default> Default for Repeat<T> {
    fn default() -> Self {
        Self::new(0, 1.0, noop_bind::<T>)
    }
}

impl<T: Widget + Default> Repeat<T> {
    pub fn new(count: usize, item_extent: f32, bind: fn(&mut T, usize)) -> Self {
        Repeat {
            count,
            scroll: 0.0,
            item_extent,
            bind,
            window: Vec::new(),
            states: Vec::new(),
            first: 0,
            row_style: Vec::new(),
        }
    }

    /// Materialized instances (visible window), for tests/inspection.
    pub fn window_len(&self) -> usize {
        self.window.len()
    }

    /// First materialized row.
    pub fn first_row(&self) -> usize {
        self.first
    }

    /// Copy of instance `k`'s state for template node `node`.
    pub fn instance_state(&self, k: usize, node: usize) -> NodeState {
        self.states[k * T::NODE_COUNT + node]
    }

    /// Instance `k`'s widget, for routing and observability.
    pub fn instance(&self, k: usize) -> &T {
        &self.window[k]
    }

    fn visible_range(&self, viewport_h: f32) -> (usize, usize) {
        if self.count == 0 || self.item_extent <= 0.0 {
            return (0, 0);
        }
        let first = ((self.scroll / self.item_extent).floor().max(0.0) as usize)
            .min(self.count - 1);
        let vis = ((viewport_h / self.item_extent).ceil() as usize + 1).min(self.count - first);
        (first, vis)
    }

    fn ensure_row_style(&mut self) {
        if self.row_style.is_empty() {
            let probe = T::default();
            self.row_style = (0..T::NODE_COUNT).map(|j| probe.node(j).defaults()).collect();
        }
    }

    /// Geometry of materialized row `k`'s root node — for tests.
    pub fn row_state(&self, k: usize) -> NodeState {
        self.states[k * T::NODE_COUNT]
    }

    fn slice(&self, k: usize) -> &[NodeState] {
        &self.states[k * T::NODE_COUNT..(k + 1) * T::NODE_COUNT]
    }
}

impl<T: Widget + Default + 'static> Widget for Repeat<T> {
    const NODE_COUNT: usize = 1;
    /// The template's depth counts: a repeater recurses into instances.
    const DEPTH: usize = 1 + T::DEPTH;
    const IS_RIGID: bool = false;
    /// One entry: no *table* children (the template is not spliced in),
    /// but CONTAINER, because it places off-table content.
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::CONTAINER)];
}

impl<T: Widget + Default + 'static> Node for Repeat<T> {
    fn kind_name(&self) -> &'static str {
        "repeat"
    }

    fn defaults(&self) -> Style {
        // A list fills its container and scrolls inside it.
        Style { width: Length::FILL, height: Length::FILL, ..Style::default() }
    }

    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        // The repeater's own style is read via `resolve_self` below;
        // instances use the template's defaults, since the cascade is
        // keyed by table index and instances are off-table.
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let (first, vis) = self.visible_range(limits.max_h);
        self.first = first;
        self.window.resize_with(vis, T::default);
        self.states.resize(vis * T::NODE_COUNT, NodeState::default());
        let bind = self.bind;
        for (k, item) in self.window.iter_mut().enumerate() {
            bind(item, first + k);
        }

        let row_limits = Constraints {
            min_w: 0.0,
            max_w: limits.max_w,
            min_h: 0.0,
            max_h: self.item_extent,
        };
        let n = T::NODE_COUNT;
        // Instances use the template's own default styles: the cascade
        // is keyed by *table* index, and instances are not in the table
        // (`docs/layout-and-storage-decisions.md` §3's recorded limit).
        self.ensure_row_style();
        let Repeat { window, states, row_style, .. } = self;
        for (k, item) in window.iter_mut().enumerate() {
            item.layout(cx, row_limits, row_style, &mut states[k * n..(k + 1) * n], 0);
        }

        let content = Size { w: limits.max_w, h: self.count as f32 * self.item_extent };
        let s = crate::ops::resolve_self(limits, &style[i], content);
        st[i].constraints = limits;
        st[i].size = s;
        s
    }

    fn place(&mut self, at: Point, _style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = at;
        // Instance geometry lives in this repeater's own arena, so the
        // window is positioned here rather than by the caller.
        let n = T::NODE_COUNT;
        let (first, extent, scroll) = (self.first, self.item_extent, self.scroll);
        let Repeat { window, states, row_style, .. } = self;
        for k in 0..window.len() {
            let dy = (first + k) as f32 * extent - scroll;
            let origin = Point { x: at.x, y: at.y + dy };
            window[k].place(origin, row_style, &mut states[k * n..(k + 1) * n], 0);
        }
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut DisplayList) {
        // A repeater is a viewport, and its window deliberately overruns
        // it in both directions: `place` puts the first row at
        // `first * extent - scroll`, negative for any scroll off an item
        // boundary, and `visible_range` materializes one row past the
        // bottom because a viewport is rarely an exact multiple of the
        // item extent. Both are required by virtualization rather than
        // incidental to it.
        //
        // `hit` has always gated on this same rect before examining a
        // row, so without this clip paint and hit disagree: a row drawn
        // up to a full item outside the list is visible but not
        // clickable. Clipping here is what makes the two agree.
        //
        // When `Style.overflow` lands (A2 of
        // `docs/text-and-scrolling-decisions.md`) this becomes the
        // generic mechanism and `Repeat` keeps it only as its
        // `defaults()`.
        out.push_clip(crate::ops::rect_of(&st[i]));
        for k in 0..self.window.len() {
            self.window[k].paint(self.slice(k), 0, out);
        }
        out.pop_clip();
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit> {
        if !contains(&st[i], x, y) {
            return None;
        }
        for k in 0..self.window.len() {
            if let Some(h) = self.window[k].hit(self.slice(k), 0, x, y) {
                return Some(Hit {
                    node: i as u32,
                    inner: Some(Inner { row: (self.first + k) as u64, node: h.node }),
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
        node.set_size_of_set(self.count);
        for k in 0..self.window.len() {
            let row = (self.first + k) as u64;
            // Row-stable synthetic ids: `1<<63 | node<<44 | row<<12 | template`.
            let id_of = |t: usize| (1u64 << 63) | (id << 44) | (row << 12) | t as u64;
            crate::observe::emit_subtree(&self.window[k], self.slice(k), &id_of, extra);
            node.push_child(accesskit::NodeId(id_of(0)));
        }
    }

    #[cfg(feature = "observe")]
    fn stats(&self, out: &mut crate::desc::NodeStats) {
        out.items = Some(self.count as u64);
        out.materialized = Some(self.window.len() as u32);
    }
}

/// Routes to the template's handlers with the hit row attached, so a
/// handler declared on a template widget receives `ctx.row` as an
/// argument — never as a capture (`docs/events-decision.md`).
impl<T, S> crate::event::Handlers<S> for Repeat<T>
where
    T: Widget + Default + 'static + crate::event::Handlers<S>,
{
    fn handler(
        &self,
        sel: crate::event::NodeSel,
    ) -> Option<fn(&mut S, &crate::event::EventCtx)> {
        let (row, node) = sel.inner?;
        let k = (row as usize).checked_sub(self.first)?;
        let item = self.window.get(k)?;
        item.handler(crate::event::NodeSel { i: node as usize, inner: None })
    }
}
