//! Step 6's `#[fuse]` dial, on the recursive path: fusion is now the
//! *same* generated body with static calls instead of `&dyn` calls, so
//! this measures monomorphization alone.
//!
//! Run: `cargo run --release -p benches --bin fusion`

use benches::{bench, d_assembled, d_ten, Card, DynWidget, FusedCard, FusedTen, Ten};
use lumen_static::assemble::Assembled;
use lumen_static::{pass, Constraints, NodeState, Point, Widget};

const ROOT_C: Constraints = Constraints { min_w: 0.0, max_w: 1920.0, min_h: 0.0, max_h: 100_000.0 };

fn static_root<W: Widget + Default>(label: &str, iters: usize) {
    let mut w = W::default();
    let mut st = vec![NodeState::default(); W::NODE_COUNT];
    let sty = pass::default_styles(&w);
    bench(label, iters, || {
        pass::layout(&mut w, &sty, &mut st, ROOT_C);
        let mut out = 0u64;
        pass::paint(&w, &st, &mut out);
        out
    });
}

fn slab<C: Widget + Default + 'static>(label: &str, iters: usize, count: usize) {
    let mut a = Assembled::<C>::new(count);
    bench(label, iters, || {
        a.layout(ROOT_C);
        let mut out = 0u64;
        a.paint(&mut out);
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
    println!("fusion: recursive &dyn vs recursive static calls (same bodies)\n");

    static_root::<Ten>("10/dyn-calls", 200_000);
    static_root::<FusedTen>("10/fused", 200_000);
    dyn_arm("10/naive-dyn", 200_000, d_ten());
    println!();

    slab::<Card>("1k/dyn-calls", 20_000, 45);
    slab::<FusedCard>("1k/fused", 20_000, 45);
    dyn_arm("1k/naive-dyn", 20_000, d_assembled(45));
    println!();

    slab::<Card>("100k/dyn-calls", 300, 4545);
    slab::<FusedCard>("100k/fused", 300, 4545);
    dyn_arm("100k/naive-dyn", 300, d_assembled(4545));
}
