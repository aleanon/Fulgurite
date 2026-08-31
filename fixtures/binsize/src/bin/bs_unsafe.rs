//! Code-size probe: the unsafe base+offset path over a 90-node tree.
//! Shapes are defined locally so the binary links *only* this path.
use lumen_static::ops::{Fixed, Label, Spacer};
use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};

#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
struct Item { icon: Fixed, name: Label, gap: Spacer, size: Label }

#[derive(StaticTree, Default)]
#[widget(column, spacing = 2.0)]
struct Card { title: Label, r1: Item, r2: Item, r3: Item, r4: Item }

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Block { title: Label, c1: Card, c2: Card, c3: Card, c4: Card }

fn main() {
    let t = Block::default();
    let base = &t as *const Block as *const u8;
    let mut st = [NodeState::default(); Block::NODE_COUNT];
    let c = Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 600.0 };
    pass::layout(base, Block::SUB_TABLE, Block::KIND_LIST, &mut st, c);
    let mut out = 0u64;
    pass::paint(base, Block::SUB_TABLE, Block::KIND_LIST, &st, &mut out);
    println!("{out:#x} {}", Block::NODE_COUNT);
}
