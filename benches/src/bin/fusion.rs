//! Step 6 benchmark: the `#[fuse]` erasure dial against Step 2's arms.
//! Full layout + paint per iteration, same shapes and constraints as the
//! crossover benchmark, so numbers are directly comparable.
//!
//! Run: `cargo run --release -p benches --bin fusion`

use benches::{bench, d_assembled, d_ten, Card, DynWidget, FusedCard, FusedTen, Ten};
use lumen_static::assemble::Assembled;
use lumen_static::{pass, Constraints, NodeState, Point, Widget};

const ROOT_C: Constraints = Constraints { min_w: 0.0, max_w: 1920.0, min_h: 0.0, max_h: 100_000.0 };

fn static_root<W: Widget + Default>(label: &str, iters: usize) {
    let w = W::default();
    let base = &w as *const W as *const u8;
    let mut st = vec![NodeState::default(); W::NODE_COUNT];
    bench(label, iters, || {
        pass::layout(base, W::SUB_TABLE, W::KIND_LIST, &mut st, ROOT_C);
        let mut out = 0u64;
        pass::paint(base, W::SUB_TABLE, W::KIND_LIST, &st, &mut out);
        out
    });
}

fn assembled<C: Widget + Default>(label: &str, iters: usize, count: usize) {
    let mut a = Assembled::<C>::new(count);
    let base = a.base();
    let (table, kinds) = (a.table.as_slice(), a.kinds.as_slice());
    let st = &mut a.states;
    bench(label, iters, || {
        pass::layout(base, table, kinds, st, ROOT_C);
        let mut out = 0u64;
        pass::paint(base, table, kinds, st, &mut out);
        out
    });
}

fn dyn_arm(label: &str, iters: usize, mut tree: Box<dyn DynWidget>) {
    bench(label, iters, || {
        tree.layout(ROOT_C);
        tree.place(Point::default());
        let mut out = 0u64;
        tree.paint(&mut out);
        out
    });
}

fn main() {
    println!("fusion: full layout+paint per iteration; logical sizes as in crossover\n");

    // The Step 2 gap: 10 logical nodes, where dyn beat the table.
    static_root::<Ten>("10/static-table", 200_000);
    static_root::<FusedTen>("10/fused", 200_000);
    dyn_arm("10/dyn", 200_000, d_ten());
    println!();

    // 45 cards = 991 logical nodes; fused table holds 46 entries.
    assembled::<Card>("1k/static-table", 20_000, 45);
    assembled::<FusedCard>("1k/fused-cards", 20_000, 45);
    dyn_arm("1k/dyn", 20_000, d_assembled(45));
    println!();

    // 4545 cards = 99,991 logical nodes; fused table holds 4,546 entries.
    assembled::<Card>("100k/static-table", 300, 4545);
    assembled::<FusedCard>("100k/fused-cards", 300, 4545);
    dyn_arm("100k/dyn", 300, d_assembled(4545));
}
