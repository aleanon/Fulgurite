//! Runtime table assembly: the large-app path from handoff §8.
//!
//! A homogeneous slab of component instances (`Vec<C>`) under one
//! synthetic column root, with the root table built at startup by running
//! the *same* const splice fns the derive uses at compile time. The kind
//! list is shared across instances — kinds are per widget kind, so 1,000
//! instances contribute `C::KIND_COUNT` entries once, not 1,000 times.

use crate::desc::{NodeDesc, NodeState, WidgetOps, NO_PARENT};
use crate::ops::VIRTUAL_COLUMN_OPS;
use crate::widget::{splice_child, Widget};

pub struct Assembled<C> {
    pub instances: Vec<C>,
    pub table: Vec<NodeDesc>,
    pub kinds: Vec<&'static WidgetOps>,
    pub states: Vec<NodeState>,
}

/// Build the concatenated table: a synthetic column root followed by
/// `count` copies of `sub`, rebased. `stride` is the byte distance
/// between instances — the safe prototype (`crate::safe::Slab`) passes 0
/// because it addresses instances by index rather than by offset, and
/// otherwise builds byte-identical structure.
pub fn concat_table(
    count: usize,
    node_count: usize,
    sub: &[NodeDesc],
    stride: usize,
) -> Vec<NodeDesc> {
    let n = 1 + count * node_count;
    let mut table = vec![NodeDesc::EMPTY; n];
    table[0] = NodeDesc::new(
        0,
        0,
        if count > 0 { 1 } else { 0 },
        count as u16,
        NO_PARENT,
        0,
        if count > 0 { NodeDesc::CONTAINER } else { 0 },
    );
    let mut at = 1usize;
    let mut prev_root = 0usize;
    for i in 0..count {
        splice_child(&mut table, at, sub, 0, (i * stride) as u32, 1);
        if prev_root != 0 {
            table[prev_root].next_sibling = at as u32;
        }
        prev_root = at;
        at += node_count;
    }
    table
}

impl<C: Widget + Default> Assembled<C> {
    pub fn new(count: usize) -> Self {
        let n = 1 + count * C::NODE_COUNT;
        let mut table = vec![NodeDesc::EMPTY; n];
        table[0] = NodeDesc::new(
            0,
            0,
            if count > 0 { 1 } else { 0 },
            count as u16,
            NO_PARENT,
            0,
            if count > 0 { NodeDesc::CONTAINER } else { 0 },
        );

        let mut kinds: Vec<&'static WidgetOps> = Vec::with_capacity(1 + C::KIND_COUNT);
        kinds.push(&VIRTUAL_COLUMN_OPS);
        kinds.extend_from_slice(C::KIND_LIST);

        let stride = core::mem::size_of::<C>();
        let mut at = 1usize;
        let mut prev_root = 0usize;
        for i in 0..count {
            splice_child(&mut table, at, C::SUB_TABLE, 0, (i * stride) as u32, 1);
            if prev_root != 0 {
                table[prev_root].next_sibling = at as u32;
            }
            prev_root = at;
            at += C::NODE_COUNT;
        }

        Assembled {
            instances: (0..count).map(|_| C::default()).collect(),
            table,
            kinds,
            states: vec![NodeState::default(); n],
        }
    }

    /// Base pointer for pass dispatch: node offsets address into the
    /// instance slab (instance `i` at `i * size_of::<C>()`).
    pub fn base(&self) -> *const u8 {
        self.instances.as_ptr() as *const u8
    }
}
