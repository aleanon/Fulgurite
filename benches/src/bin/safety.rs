//! Does removing the `unsafe` cost anything? — `docs/verification-notes.md`.
//!
//! Two arms over **identical shapes and a byte-identical const table**;
//! the only difference is how a node's data is reached:
//!
//! - `unsafe`: `base.add(desc.offset) as *const W`, dispatched through
//!   `KIND_LIST[desc.kind]`;
//! - `safe`: a derive-generated `node(i) -> &dyn Node` range match, with
//!   the vtable riding along on the `&dyn`.
//!
//! Run: `cargo run --release -p benches --bin safety`

use benches::{bench, Card, SafeCard, SafeTen, Ten};
use lumen_static::assemble::Assembled;
use lumen_static::safe::{self, RNode, RSlab, Slab, Tree};
use lumen_static::{pass, Constraints, NodeState, Widget};

const ROOT_C: Constraints = Constraints { min_w: 0.0, max_w: 1920.0, min_h: 0.0, max_h: 100_000.0 };

fn unsafe_root<W: Widget + Default>(label: &str, iters: usize) {
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

fn safe_root<T: Tree + Default>(label: &str, iters: usize) {
    let t = T::default();
    let mut st = vec![NodeState::default(); T::COUNT];
    bench(label, iters, || {
        safe::layout(&t, &mut st, ROOT_C);
        let mut out = 0u64;
        safe::paint(&t, &st, &mut out);
        out
    });
}

/// Same safe arm, but the accessor chain is walked once per frame
/// instead of once per pass.
fn safe_indexed<T: Tree + Default>(label: &str, iters: usize) {
    let t = T::default();
    let mut st = vec![NodeState::default(); T::COUNT];
    bench(label, iters, || {
        let nodes = safe::resolve(&t);
        safe::layout_indexed(&nodes, T::TABLE, &mut st, ROOT_C);
        let mut out = 0u64;
        safe::paint_indexed(&nodes, T::TABLE, &st, &mut out);
        out
    });
}

fn safe_slab_indexed<C: Tree + Default + 'static>(label: &str, iters: usize, count: usize) {
    let slab = Slab::<C>::new(count);
    let mut st = vec![NodeState::default(); slab.table().len()];
    bench(label, iters, || {
        let nodes = safe::resolve_slab(&slab);
        safe::layout_indexed(&nodes, slab.table(), &mut st, ROOT_C);
        let mut out = 0u64;
        safe::paint_indexed(&nodes, slab.table(), &st, &mut out);
        out
    });
}

/// The index built once and reused, which is what a host would actually
/// do: a static tree does not move between frames.
fn safe_slab_cached<C: Tree + Default + 'static>(label: &str, iters: usize, count: usize) {
    let slab = Slab::<C>::new(count);
    let mut st = vec![NodeState::default(); slab.table().len()];
    let nodes = safe::resolve_slab(&slab);
    bench(label, iters, || {
        safe::layout_indexed(&nodes, slab.table(), &mut st, ROOT_C);
        let mut out = 0u64;
        safe::paint_indexed(&nodes, slab.table(), &st, &mut out);
        out
    });
}

/// Recursive descent — iced's traversal. One visit per node, children
/// reached through `&dyn` so code stays O(kinds).
fn recursive_root<T: RNode + Tree + Default>(label: &str, iters: usize) {
    let t = T::default();
    let mut st = vec![NodeState::default(); T::COUNT];
    bench(label, iters, || {
        t.layout(ROOT_C, &mut st, 0);
        t.place(lumen_static::Point::default(), &mut st, 0);
        let mut out = 0u64;
        t.paint(&st, 0, &mut out);
        out
    });
}

fn recursive_slab<C: RNode + Tree + Default>(label: &str, iters: usize, count: usize) {
    let slab = RSlab::<C>::new(count);
    let mut st = vec![NodeState::default(); 1 + count * C::COUNT];
    bench(label, iters, || {
        slab.layout(ROOT_C, &mut st);
        slab.place(&mut st);
        let mut out = 0u64;
        slab.paint(&st, &mut out);
        out
    });
}

fn unsafe_slab<C: Widget + Default>(label: &str, iters: usize, count: usize) {
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

fn safe_slab<C: Tree + Default + 'static>(label: &str, iters: usize, count: usize) {
    let slab = Slab::<C>::new(count);
    let mut st = vec![NodeState::default(); slab.table().len()];
    bench(label, iters, || {
        safe::layout_slab(&slab, &mut st, ROOT_C);
        let mut out = 0u64;
        safe::paint_slab(&slab, &st, &mut out);
        out
    });
}

/// Both arms must produce the same geometry, or the comparison is
/// meaningless. (Paint checksums differ only where the unsafe path folds
/// `desc.kind`, which the safe path also folds — so these match too.)
fn check_equivalence() {
    let u = Ten::default();
    let ubase = &u as *const Ten as *const u8;
    let mut ust = [NodeState::default(); Ten::NODE_COUNT];
    pass::layout(ubase, Ten::SUB_TABLE, Ten::KIND_LIST, &mut ust, ROOT_C);

    let s = SafeTen::default();
    let mut sst = [NodeState::default(); SafeTen::NODE_COUNT];
    safe::layout(&s, &mut sst, ROOT_C);

    for i in 0..Ten::NODE_COUNT {
        assert_eq!(ust[i].size, sst[i].size, "size mismatch at node {i}");
        assert_eq!(ust[i].pos, sst[i].pos, "pos mismatch at node {i}");
    }

    let mut a = Assembled::<Card>::new(45);
    let abase = a.base();
    pass::layout(abase, &a.table, &a.kinds, &mut a.states, ROOT_C);
    let slab = Slab::<SafeCard>::new(45);
    let mut sst = vec![NodeState::default(); slab.table().len()];
    safe::layout_slab(&slab, &mut sst, ROOT_C);
    for (i, (u, s)) in a.states.iter().zip(sst.iter()).enumerate() {
        assert_eq!(u.size, s.size, "slab size mismatch at {i}");
        assert_eq!(u.pos, s.pos, "slab pos mismatch at {i}");
    }
    // Recursive arm must agree too, or its numbers mean nothing.
    let r = SafeTen::default();
    let mut rst = [NodeState::default(); SafeTen::COUNT];
    r.layout(ROOT_C, &mut rst, 0);
    r.place(lumen_static::Point::default(), &mut rst, 0);
    for i in 0..Ten::NODE_COUNT {
        assert_eq!(ust[i].size, rst[i].size, "recursive size mismatch at {i}");
        assert_eq!(ust[i].pos, rst[i].pos, "recursive pos mismatch at {i}");
    }

    let rslab = RSlab::<SafeCard>::new(45);
    let mut rst = vec![NodeState::default(); 1 + 45 * SafeCard::COUNT];
    rslab.layout(ROOT_C, &mut rst);
    rslab.place(&mut rst);
    for (i, (u, r)) in a.states.iter().zip(rst.iter()).enumerate() {
        assert_eq!(u.size, r.size, "recursive slab size mismatch at {i}");
        assert_eq!(u.pos, r.pos, "recursive slab pos mismatch at {i}");
    }

    println!("equivalence: flat-unsafe, flat-safe and recursive all agree\n");
}

fn main() {
    println!("safety: unsafe base+offset vs safe &dyn accessor, identical tables\n");
    check_equivalence();

    unsafe_root::<Ten>("10/unsafe", 200_000);
    safe_root::<SafeTen>("10/safe", 200_000);
    safe_indexed::<SafeTen>("10/safe-indexed", 200_000);
    recursive_root::<SafeTen>("10/recursive", 200_000);
    println!();

    unsafe_slab::<Card>("1k/unsafe", 20_000, 45);
    safe_slab::<SafeCard>("1k/safe", 20_000, 45);
    safe_slab_indexed::<SafeCard>("1k/safe-indexed", 20_000, 45);
    safe_slab_cached::<SafeCard>("1k/safe-cached", 20_000, 45);
    recursive_slab::<SafeCard>("1k/recursive", 20_000, 45);
    println!();

    unsafe_slab::<Card>("100k/unsafe", 300, 4545);
    safe_slab::<SafeCard>("100k/safe", 300, 4545);
    safe_slab_indexed::<SafeCard>("100k/safe-indexed", 300, 4545);
    safe_slab_cached::<SafeCard>("100k/safe-cached", 300, 4545);
    recursive_slab::<SafeCard>("100k/recursive", 300, 4545);
}
