//! M25/T25.1: the frame a widget's look would be measured in.
//!
//! **Why this exists rather than `crossover`.** M25's pilot migrates
//! `Toggle`, and `crossover`'s trees hold labels, fixed boxes and
//! spacers — so a toggle's look could be arbitrarily expensive and that
//! frame would not move. `docs/plan-appearance.md` §5 records the review
//! finding: a kill switch measured on a tree that does not contain the
//! widget it judges cannot fire. This tree is toggles on the table, where
//! `Style` is per node, laid out and painted in full every iteration.
//!
//! It also prints the host sizes an arena row is made of, because a
//! `List` or `Repeat` shares one style row and its per-row cost is
//! `NodeState` plus the instance, not `NodeState` plus `Style`.
//!
//! Run: `cargo run --release -p benches --bin appearance`

use std::hint::black_box;
use std::mem::size_of;

use benches::bench;
use lumen_static::assemble::Assembled;
use lumen_static::ops::{Button, Label};
use lumen_static::{CheckBox, Constraints, NodeState, Radio, StaticTree, Style, Switch, Toggle, Widget};

const ROOT_C: Constraints = Constraints { min_w: 0.0, max_w: 1920.0, min_h: 0.0, max_h: 100_000.0 };

/// Ten toggles in a row, all three shapes: 11 nodes.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
pub struct Toggles {
    a: CheckBox<()>,
    b: Radio<()>,
    c: Switch<()>,
    d: CheckBox<()>,
    e: Radio<()>,
    f: Switch<()>,
    g: CheckBox<()>,
    h: Radio<()>,
    i: Switch<()>,
    j: CheckBox<()>,
}

fn main() {
    black_box(());
    const ROWS: usize = 100;
    let mut slab = Assembled::<Toggles>::new(ROWS);
    // Half of them on, so both paint paths run.
    println!(
        "appearance: full layout+paint per iteration; {} rows x {} nodes = {} table nodes\n",
        ROWS,
        Toggles::NODE_COUNT,
        ROWS * Toggles::NODE_COUNT
    );
    bench("1k-toggles/static", 20_000, || {
        slab.layout(ROOT_C);
        let mut out = lumen_static::DisplayList::new();
        slab.paint(&mut out);
        out.len() as u64
    });

    println!("\nhost sizes, bytes (an arena row is NodeState + one instance):");
    for (name, s) in [
        ("NodeState", size_of::<NodeState>()),
        ("Style", size_of::<Style>()),
        ("Label<String>", size_of::<Label<String>>()),
        ("Button<(), String>", size_of::<Button<(), String>>()),
        ("Toggle<()>", size_of::<Toggle<()>>()),
    ] {
        println!("  {name:<20} {s:>4}");
    }
}
