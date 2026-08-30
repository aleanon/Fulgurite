//! The §10 Step 2 crossover benchmark: static node-table traversal vs
//! naive `Box<dyn Widget>` recursion at 10 / 1k / 100k nodes, plus the
//! rigid/flexible partitioned pass. One full layout + paint per iteration.
//!
//! Run: `cargo run --release -p benches --bin crossover`

use std::hint::black_box;
use std::time::Instant;

use benches::{d_assembled, d_ten, DynWidget, Ten};
use lumen_static::assemble::Assembled;
use lumen_static::widget::{collect_partition, count_partition};
use lumen_static::{pass, Constraints, NodeState, Point, Widget};

const ROOT_C: Constraints = Constraints { min_w: 0.0, max_w: 1920.0, min_h: 0.0, max_h: 100_000.0 };

fn bench(label: &str, iters: usize, mut f: impl FnMut() -> u64) {
    // warmup
    for _ in 0..iters.div_ceil(10).max(3) {
        black_box(f());
    }
    let mut samples: Vec<u64> = Vec::with_capacity(iters);
    for _ in 0..iters {
        let t0 = Instant::now();
        black_box(f());
        samples.push(t0.elapsed().as_nanos() as u64);
    }
    samples.sort_unstable();
    let med = samples[samples.len() / 2];
    let p10 = samples[samples.len() / 10];
    let p90 = samples[samples.len() * 9 / 10];
    println!("{label:<32} median {med:>12} ns   p10 {p10:>12}   p90 {p90:>12}");
}

fn static_arm(label: &str, iters: usize, node_count: usize, card_count: usize) {
    if card_count == 0 {
        // Fully const path: table and states are compile-time-sized.
        let tree = Ten::default();
        let base = &tree as *const Ten as *const u8;
        let mut st = [NodeState::default(); Ten::NODE_COUNT];
        assert_eq!(Ten::SUB_TABLE.len(), node_count);
        bench(&format!("{label}/static"), iters, || {
            pass::layout(base, Ten::SUB_TABLE, Ten::KIND_LIST, &mut st, ROOT_C);
            let mut out = 0u64;
            pass::paint(base, Ten::SUB_TABLE, Ten::KIND_LIST, &st, &mut out);
            out
        });
        return;
    }

    let mut a = Assembled::<benches::Card>::new(card_count);
    assert_eq!(a.table.len(), node_count);
    let base = a.base();
    let (table, kinds) = (a.table.as_slice(), a.kinds.as_slice());
    let st = &mut a.states;
    bench(&format!("{label}/static"), iters, || {
        pass::layout(base, table, kinds, st, ROOT_C);
        let mut out = 0u64;
        pass::paint(base, table, kinds, st, &mut out);
        out
    });

    // Partitioned pass (rigid caches warm from the loop above).
    let n_rigid = count_partition(table, true);
    let flex: Vec<u32> = (0..table.len() as u32).filter(|&i| !table[i as usize].is_rigid()).collect();
    bench(
        &format!("{label}/static-part({}% rigid)", 100 * n_rigid / table.len()),
        iters,
        || {
            pass::layout_partitioned(base, table, kinds, st, ROOT_C, &flex);
            let mut out = 0u64;
            pass::paint(base, table, kinds, st, &mut out);
            out
        },
    );
}

fn dyn_arm(label: &str, iters: usize, mut tree: Box<dyn DynWidget>) {
    bench(&format!("{label}/dyn"), iters, || {
        tree.layout(ROOT_C);
        tree.place(Point::default());
        let mut out = 0u64;
        tree.paint(&mut out);
        out
    });
}

fn main() {
    // Compile-time partition proof at a concrete const site (handoff §5):
    // the array sizes come from the table itself, at compile time.
    const TEN_FLEX_N: usize = count_partition(Ten::SUB_TABLE, false);
    const TEN_FLEX: [u32; TEN_FLEX_N] = collect_partition(Ten::SUB_TABLE, false);
    assert_eq!(TEN_FLEX.len(), 7); // 10 nodes - 3 Fixed = 7 flexible

    println!("crossover: full layout+paint per iteration; sizes 10 / 1k / 100k nodes\n");

    static_arm("10", 200_000, 10, 0);
    dyn_arm("10", 200_000, d_ten());
    println!();

    static_arm("1k", 20_000, 1 + 45 * 22, 45); // 991 nodes
    dyn_arm("1k", 20_000, d_assembled(45));
    println!();

    static_arm("100k", 300, 1 + 4545 * 22, 4545); // 99,991 nodes
    dyn_arm("100k", 300, d_assembled(4545));
    println!();

    partition_probe("1k-rigid", 20_000, 45);
    partition_probe("100k-rigid", 300, 4545);
}

/// The rigid/flexible partition on a ~90%-rigid tree: whole `RigidItem`
/// subtrees collapse out of the down/up loops.
fn partition_probe(label: &str, iters: usize, card_count: usize) {
    let mut a = Assembled::<benches::RigidCard>::new(card_count);
    let base = a.base();
    let (table, kinds) = (a.table.as_slice(), a.kinds.as_slice());
    let st = &mut a.states;
    bench(&format!("{label}/static"), iters, || {
        pass::layout(base, table, kinds, st, ROOT_C);
        let mut out = 0u64;
        pass::paint(base, table, kinds, st, &mut out);
        out
    });
    let n_rigid = count_partition(table, true);
    let flex: Vec<u32> = (0..table.len() as u32).filter(|&i| !table[i as usize].is_rigid()).collect();
    bench(
        &format!("{label}/static-part({}% rigid)", 100 * n_rigid / table.len()),
        iters,
        || {
            pass::layout_partitioned(base, table, kinds, st, ROOT_C, &flex);
            let mut out = 0u64;
            pass::paint(base, table, kinds, st, &mut out);
            out
        },
    );
    // Decomposition: layout-only (both variants) and paint-only, so the
    // partition saving can be attributed - paint always walks every node.
    bench(&format!("{label}/layout-full"), iters, || {
        pass::layout(base, table, kinds, st, ROOT_C);
        0
    });
    bench(&format!("{label}/layout-part"), iters, || {
        pass::layout_partitioned(base, table, kinds, st, ROOT_C, &flex);
        0
    });
    bench(&format!("{label}/paint-only"), iters, || {
        let mut out = 0u64;
        pass::paint(base, table, kinds, st, &mut out);
        out
    });
}
