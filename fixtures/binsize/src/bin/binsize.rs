//! Code-size probe: one 90-node tree, so `nm` can attribute framework
//! bytes. Shapes are local so the binary links only what it uses.
use lumen_static_widgets::ops::{Fixed, Label, Spacer};
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
    let mut t = Block::default();
    let mut st = [NodeState::default(); Block::NODE_COUNT];
    let sty = pass::default_styles(&t);
    let c = Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 600.0 };
    pass::layout(&mut t, &sty, &mut st, c);
    let mut out = lumen_static::DisplayList::new();
    pass::paint(&t, &st, &mut out);
    println!("cmds={} nodes={} depth={}", out.len(), Block::NODE_COUNT, Block::DEPTH);
}
