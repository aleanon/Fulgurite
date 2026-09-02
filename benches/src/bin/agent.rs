//! What the dev surface costs the frame it observes.
//!
//! `Agent::frame_done` measured 117x the frame at 10k nodes before
//! 2026-09-02: three collections allocated per call, and a per-node
//! `paint` that recursed so a container signed its whole subtree.
//! `docs/plan-review-fixes.md` F2.
//!
//! Run: `cargo run --release -p benches --bin agent`

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}
#[global_allocator]
static A: Counting = Counting;

use benches::{bench, Ten10};
use lumen_static::{pass, Constraints, NodeState, Widget};
use lumen_static_agent::{Agent, TreeMut, TreeRef};

const ROOT_C: Constraints =
    Constraints { min_w: 0.0, max_w: 1920.0, min_h: 0.0, max_h: 100_000.0 };

fn main() {
    black_box(());
    println!("agent: the cost of observing a frame\n");

    let mut w = Ten10::default();
    let sty = pass::default_styles(&w);
    let mut st = vec![NodeState::default(); Ten10::NODE_COUNT];
    pass::layout(&mut w, &sty, &mut st, ROOT_C);

    bench("frame: layout + paint", 20_000, || {
        pass::layout(&mut w, &sty, &mut st, ROOT_C);
        let mut out = lumen_static::DisplayList::new();
        pass::paint(&w, &st, &mut out);
        out.len() as u64
    });

    let mut agent = Agent::new();
    {
        let mut t = TreeMut::of_root(&mut w, &sty, &st);
        agent.frame_done(&mut t, 1, 1);
    }
    bench("frame_done (steady state)", 20_000, || {
        let mut t = TreeMut::of_root(&mut w, &sty, &st);
        agent.frame_done(&mut t, 1, 1);
        1
    });

    // Allocations per steady-state frame_done. The claim is zero.
    for _ in 0..5 {
        let mut t = TreeMut::of_root(&mut w, &sty, &st);
        agent.frame_done(&mut t, 1, 1);
    }
    let before = ALLOCS.load(Ordering::Relaxed);
    for _ in 0..100 {
        let mut t = TreeMut::of_root(&mut w, &sty, &st);
        agent.frame_done(&mut t, 1, 1);
    }
    let per_frame = (ALLOCS.load(Ordering::Relaxed) - before) as f64 / 100.0;
    println!("\nallocations per steady-state frame_done: {per_frame:.2}");

    // Commands recorded per node — the O(N.depth) symptom.
    let tree = TreeRef::of_root(&w, &sty, &st);
    let mut list = lumen_static::DisplayList::new();
    pass::paint(&w, &st, &mut list);
    println!(
        "\n{} nodes, one root paint records {} commands ({:.2}x N)",
        Ten10::NODE_COUNT,
        list.len(),
        list.len() as f32 / Ten10::NODE_COUNT as f32,
    );
    let _ = tree;
}
