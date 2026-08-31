//! Code-size probe: the safe &dyn accessor path over the same shape.
use lumen_static::ops::{Fixed, Label, Spacer};
use lumen_static::safe::{self, Tree};
use lumen_static::{Constraints, NodeState, SafeTree, StaticTree};

#[derive(StaticTree, SafeTree, Default)]
#[widget(row, spacing = 4.0)]
struct Item { icon: Fixed, name: Label, gap: Spacer, size: Label }

#[derive(StaticTree, SafeTree, Default)]
#[widget(column, spacing = 2.0)]
struct Card { title: Label, r1: Item, r2: Item, r3: Item, r4: Item }

#[derive(StaticTree, SafeTree, Default)]
#[widget(column, spacing = 0.0)]
struct Block { title: Label, c1: Card, c2: Card, c3: Card, c4: Card }

fn main() {
    let t = Block::default();
    let mut st = [NodeState::default(); Block::COUNT];
    let c = Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 600.0 };
    safe::layout(&t, &mut st, c);
    let mut out = 0u64;
    safe::paint(&t, &st, &mut out);
    println!("{out:#x} {}", Block::COUNT);
}
