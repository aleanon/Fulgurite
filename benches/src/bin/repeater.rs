//! Step 3 scaling benchmark (§6): a virtualized `Repeat<Item>` vs a
//! fully materialized tree of the same rows. The claim under test is
//! that frame cost and resident size track the *viewport*, not the data.
//!
//! Run: `cargo run --release -p benches --bin repeater`

use benches::{bench, Item};
use lumen_static::assemble::Assembled;
use lumen_static::desc::NodeDesc;
use lumen_static::repeat::Repeat;
use lumen_static::{pass, Constraints, NodeState, Widget};

const ROOT_C: Constraints = Constraints { min_w: 0.0, max_w: 1920.0, min_h: 0.0, max_h: 800.0 };

fn bind_icon(item: &mut Item, row: usize) {
    item.icon.w = (row % 50) as f32 + 1.0;
    item.icon.h = 10.0;
}

fn repeat_arm(rows: usize, iters: usize) {
    let mut list = Repeat::<Item>::new(rows, 24.0, bind_icon);
    let mut st = [NodeState::default(); 1];
    let sty = pass::default_styles(&list);
    bench(&format!("repeat/{rows}-rows"), iters, || {
        pass::layout(&mut list, &sty, &mut st, ROOT_C);
        let mut out = 0u64;
        pass::paint(&list, &st, &mut out);
        out
    });
    println!(
        "    window {} rows, resident nodes {} (table {} B, states {} B)",
        list.window_len(),
        1 + list.window_len() * Item::NODE_COUNT,
        std::mem::size_of::<NodeDesc>(),
        (1 + list.window_len() * Item::NODE_COUNT) * size_of::<NodeState>(),
    );
}

fn materialized_arm(rows: usize, iters: usize) {
    let mut a = Assembled::<Item>::new(rows);
    for (row, item) in a.instances.iter_mut().enumerate() {
        bind_icon(item, row);
    }
    bench(&format!("materialized/{rows}-rows"), iters, || {
        a.layout(ROOT_C);
        let mut out = 0u64;
        a.paint(&mut out);
        out
    });
    println!(
        "    resident nodes {} (table {} B, states {} B)",
        a.table.len(),
        std::mem::size_of_val(a.table.as_slice()),
        std::mem::size_of_val(a.states.as_slice()),
    );
}

fn main() {
    println!("repeater scaling: full layout+paint per iteration, 800px viewport\n");
    for rows in [1_000usize, 100_000, 1_000_000, 10_000_000] {
        repeat_arm(rows, 20_000);
    }
    println!();
    materialized_arm(1_000, 20_000);
    materialized_arm(100_000, 300);
    materialized_arm(1_000_000, 20);
}
