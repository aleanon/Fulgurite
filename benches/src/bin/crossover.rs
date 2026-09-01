//! The §10 crossover benchmark: the framework's recursive traversal
//! against a naive `Box<dyn Widget>` tree at 10 / 1k / 100k nodes. One
//! full layout + paint per iteration.
//!
//! Run: `cargo run --release -p benches --bin crossover`

use std::hint::black_box;

use benches::{bench, d_assembled, d_ten, Card, DynWidget, Ten};
use lumen_static::assemble::Assembled;
use lumen_static::{pass, Constraints, NodeState, Point, Widget};

const ROOT_C: Constraints = Constraints { min_w: 0.0, max_w: 1920.0, min_h: 0.0, max_h: 100_000.0 };

fn static_root<W: Widget + Default>(label: &str, iters: usize) {
    let mut w = W::default();
    let mut st = vec![NodeState::default(); W::NODE_COUNT];
    let sty = pass::default_styles(&w);
    bench(label, iters, || {
        pass::layout(&mut w, &sty, &mut st, ROOT_C);
        let mut out = lumen_static::DisplayList::new();
        pass::paint(&w, &st, &mut out);
        out.len() as u64
    });
}

fn static_slab<C: Widget + Default + 'static>(label: &str, iters: usize, count: usize) {
    let mut a = Assembled::<C>::new(count);
    bench(label, iters, || {
        a.layout(ROOT_C);
        let mut out = lumen_static::DisplayList::new();
        a.paint(&mut out);
        out.len() as u64
    });
}

fn dyn_arm(label: &str, iters: usize, mut tree: Box<dyn DynWidget>) {
    bench(label, iters, || {
        tree.layout(ROOT_C);
        tree.place(Point::default());
        // The oracle keeps its own checksum paint.
        let mut out = 0u64;
        tree.paint(&mut out);
        out
    });
}

fn main() {
    black_box(());
    println!("crossover: full layout+paint per iteration; sizes 10 / 1k / 100k nodes\n");

    static_root::<Ten>("10/static", 200_000);
    dyn_arm("10/dyn", 200_000, d_ten());
    println!();

    static_slab::<Card>("1k/static", 20_000, 45);
    dyn_arm("1k/dyn", 20_000, d_assembled(45));
    println!();

    static_slab::<Card>("100k/static", 300, 4545);
    dyn_arm("100k/dyn", 300, d_assembled(4545));
}
