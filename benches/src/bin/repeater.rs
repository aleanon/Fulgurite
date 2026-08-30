//! Step 3 scaling benchmark (§6, §10): a virtualized `Repeat<Item>` list
//! vs a fully materialized tree of the same rows. Full layout + paint per
//! iteration, fixed 800px viewport.
//!
//! The claim under test: with the repeater, frame cost and resident tree
//! size track the *viewport*, not the data. Run:
//! `cargo run --release -p benches --bin repeater`

use benches::{bench, Item};
use lumen_static::assemble::Assembled;
use lumen_static::repeat::Repeat;
use lumen_static::{pass, Constraints, NodeState, Widget};

const ROOT_C: Constraints = Constraints { min_w: 0.0, max_w: 1920.0, min_h: 0.0, max_h: 800.0 };

fn bind_icon(item: &mut Item, row: usize) {
    item.icon.w = (row % 50) as f32 + 1.0;
    item.icon.h = 10.0;
}

fn repeat_arm(rows: usize, iters: usize) {
    let list = Repeat::<Item>::new(rows, 24.0, bind_icon);
    let base = &list as *const Repeat<Item> as *const u8;
    let mut st = [NodeState::default(); 1];
    let (table, kinds) = (Repeat::<Item>::SUB_TABLE, Repeat::<Item>::KIND_LIST);
    bench(&format!("repeat/{rows}-rows"), iters, || {
        pass::layout(base, table, kinds, &mut st, ROOT_C);
        let mut out = 0u64;
        pass::paint(base, table, kinds, &st, &mut out);
        out
    });
    let resident = 1 + list.window_len() * Item::NODE_COUNT;
    println!(
        "    window {} rows, resident nodes {} (table {} B, states {} B)",
        list.window_len(),
        resident,
        std::mem::size_of_val(table),
        (st.len() + list.window_len() * Item::NODE_COUNT) * size_of::<NodeState>(),
    );
}

fn materialized_arm(rows: usize, iters: usize) {
    let mut a = Assembled::<Item>::new(rows);
    for (row, item) in a.instances.iter_mut().enumerate() {
        bind_icon(item, row);
    }
    let base = a.base();
    let (table, kinds) = (a.table.as_slice(), a.kinds.as_slice());
    let st = &mut a.states;
    bench(&format!("materialized/{rows}-rows"), iters, || {
        pass::layout(base, table, kinds, st, ROOT_C);
        let mut out = 0u64;
        pass::paint(base, table, kinds, st, &mut out);
        out
    });
    println!(
        "    resident nodes {} (table {} B, states {} B)",
        table.len(),
        std::mem::size_of_val(table),
        st.len() * size_of::<NodeState>(),
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
