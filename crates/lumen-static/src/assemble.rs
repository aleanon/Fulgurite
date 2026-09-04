//! Runtime table assembly — the large-app path from handoff §8.
//!
//! A homogeneous slab of component instances under one synthetic column
//! root. The table is concatenated at startup for *index addressing*
//! (agent surface, journal, audit); traversal recurses over the `Vec`.

// `alloc`, not the std prelude: this crate is `no_std` when the
// `std` feature is off (M13/T13.1), and absent entirely without the
// `alloc` feature (T13.4).
#[cfg(feature = "alloc")]
use alloc::{vec, vec::Vec};

use crate::cx::LayoutCx;
use crate::display::Painter;
use crate::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use crate::hit::{contains, Hit};
use crate::style::Style;
use crate::widget::{splice_child, Node, Widget};

/// Build the concatenated table: a synthetic column root followed by
/// `count` copies of `sub`, rebased.
pub fn concat_table(count: usize, node_count: usize, sub: &[NodeDesc]) -> Vec<NodeDesc> {
    let n = 1 + count * node_count;
    let mut table = vec![NodeDesc::EMPTY; n];
    table[0] = NodeDesc::new(
        if count > 0 { 1 } else { 0 },
        NO_PARENT,
        0,
        count as u16,
        if count > 0 { NodeDesc::CONTAINER } else { 0 },
    );
    let mut at = 1usize;
    let mut prev_root = 0usize;
    for _ in 0..count {
        splice_child(&mut table, at, sub, 0);
        if prev_root != 0 {
            table[prev_root].next_sibling = at as u32;
        }
        prev_root = at;
        at += node_count;
    }
    table
}

/// **Not an [`Arena`](crate::arena::Arena) owner, deliberately.** The
/// other five off-table constructs hold `k` instances of one template
/// and index them `k * n + node`. This holds a *table* — a synthetic
/// root at index 0 followed by the instances, indexed `1 + k * n + node`
/// — because it is a runtime-assembled tree rather than a widget with
/// hidden children, and `hit` rebases into a real table index instead of
/// returning an `Inner`. Sharing `Arena` would mean carving an exception
/// into it for the one member whose addressing is different.
/// `docs/plan-review-fixes.md` F4.2.
/// `count` instances of `C` under a column root.
pub struct Assembled<C: Widget + Default + 'static> {
    pub instances: Vec<C>,
    pub table: Vec<NodeDesc>,
    pub states: Vec<NodeState>,
    pub styles: Vec<Style>,
}

impl<C: Widget + Default + 'static> Assembled<C> {
    pub fn new(count: usize) -> Self {
        Assembled {
            instances: (0..count).map(|_| C::default()).collect(),
            table: concat_table(count, C::NODE_COUNT, C::SUB_TABLE),
            states: vec![NodeState::default(); 1 + count * C::NODE_COUNT],
            styles: Self::default_styles(count),
        }
    }

    fn default_styles(count: usize) -> Vec<Style> {
        let probe = C::default();
        let mut v = Vec::with_capacity(1 + count * C::NODE_COUNT);
        v.push(Style::default());
        for _ in 0..count {
            for j in 0..C::NODE_COUNT {
                v.push(probe.node(j).defaults());
            }
        }
        v
    }

    /// Lay out and position the whole slab under `limits`. The synthetic
    /// root is a column that gives each instance the space it asks for.
    pub fn layout_with(&mut self, cx: &mut LayoutCx, limits: Constraints) {
        let n = C::NODE_COUNT;
        // The synthetic root behaves as a column: each instance is
        // offered whatever main-axis space is left, exactly as a derived
        // container's first scan does.
        let mut sum = 0.0f32;
        let mut mx = 0.0f32;
        for (k, item) in self.instances.iter_mut().enumerate() {
            let cc = Constraints {
                min_w: 0.0,
                max_w: limits.max_w,
                min_h: 0.0,
                max_h: (limits.max_h - sum).max(0.0),
            };
            let d: &mut dyn Node = item;
            let s = d.layout(
                cx,
                cc,
                &self.styles[1 + k * n..1 + (k + 1) * n],
                &mut self.states[1 + k * n..1 + (k + 1) * n],
                0,
            );
            sum += s.h;
            mx = mx.max(s.w);
        }
        self.states[0].constraints = limits;
        self.states[0].size = limits.clamp(Size { w: mx, h: sum });

        self.states[0].pos = Point::default();
        let mut cur = Point::default();
        for (k, item) in self.instances.iter_mut().enumerate() {
            let d: &mut dyn Node = item;
            d.place(
                cur,
                &self.styles[1 + k * n..1 + (k + 1) * n],
                &mut self.states[1 + k * n..1 + (k + 1) * n],
                0,
            );
            cur.y += self.states[1 + k * n].size.h;
        }
    }

    /// Lay out with the built-in metrics.
    pub fn layout(&mut self, limits: Constraints) {
        let mut m = crate::cx::BlockMetrics;
        let mut cx = LayoutCx::new(&mut m);
        self.layout_with(&mut cx, limits);
    }

    pub fn paint(&self, out: &mut dyn Painter) {
        // `hit` gates on `states[0]`; paint clips to it for the same
        // reason `List` and `Repeat` do (A1's scoped invariant).
        out.push_clip(crate::ops::rect_of(&self.states[0]));
        let n = C::NODE_COUNT;
        for (k, item) in self.instances.iter().enumerate() {
            item.paint(&self.states[1 + k * n..1 + (k + 1) * n], 0, out);
        }
        out.pop_clip();
    }

    pub fn hit(&self, x: f32, y: f32) -> Option<Hit> {
        if !contains(&self.states[0], x, y) {
            return None;
        }
        let n = C::NODE_COUNT;
        for (k, item) in self.instances.iter().enumerate() {
            if let Some(h) = item.hit(&self.states[1 + k * n..1 + (k + 1) * n], 0, x, y) {
                return Some(Hit { node: 1 + (k * n) as u32 + h.node, ..h });
            }
        }
        Some(Hit { node: 0, inner: None, local: Point { x, y } })
    }

    /// The widget at a whole-slab index, for the observability surface.
    pub fn node(&self, i: usize) -> Option<&dyn Node> {
        if i == 0 {
            return None;
        }
        let j = i - 1;
        self.instances.get(j / C::NODE_COUNT).map(|c| c.node(j % C::NODE_COUNT))
    }
}
