//! Runtime table assembly — the large-app path from handoff §8.
//!
//! A homogeneous slab of component instances under one synthetic column
//! root. The table is concatenated at startup for *index addressing*
//! (agent surface, journal, audit); traversal recurses over the `Vec`.

use crate::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use crate::hit::{contains, Hit};
use crate::ops::{col_child_limits, fold_geom};
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

/// `count` instances of `C` under a column root.
pub struct Assembled<C: Widget + Default + 'static> {
    pub instances: Vec<C>,
    pub table: Vec<NodeDesc>,
    pub states: Vec<NodeState>,
}

impl<C: Widget + Default + 'static> Assembled<C> {
    pub fn new(count: usize) -> Self {
        Assembled {
            instances: (0..count).map(|_| C::default()).collect(),
            table: concat_table(count, C::NODE_COUNT, C::SUB_TABLE),
            states: vec![NodeState::default(); 1 + count * C::NODE_COUNT],
        }
    }

    /// Lay out and position the whole slab under `limits`.
    pub fn layout(&mut self, limits: Constraints) {
        let n = C::NODE_COUNT;
        let cc = col_child_limits(limits, self.instances.len(), 0.0);
        let mut sum = 0.0f32;
        let mut mx = 0.0f32;
        for (k, item) in self.instances.iter_mut().enumerate() {
            let d: &mut dyn Node = item;
            let s = d.layout(cc, &mut self.states[1 + k * n..1 + (k + 1) * n], 0);
            sum += s.h;
            mx = mx.max(s.w);
        }
        self.states[0].constraints = limits;
        self.states[0].size = limits.clamp(Size { w: mx, h: sum });

        self.states[0].pos = Point::default();
        let mut cur = Point::default();
        for (k, item) in self.instances.iter_mut().enumerate() {
            let d: &mut dyn Node = item;
            d.place(cur, &mut self.states[1 + k * n..1 + (k + 1) * n], 0);
            cur.y += self.states[1 + k * n].size.h;
        }
    }

    pub fn paint(&self, out: &mut u64) {
        fold_geom(out, 0, &self.states[0]);
        let n = C::NODE_COUNT;
        for (k, item) in self.instances.iter().enumerate() {
            item.paint(&self.states[1 + k * n..1 + (k + 1) * n], 0, out);
        }
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
