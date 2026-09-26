//! **`frame_done` allocates nothing, and a test says so** — M48/T48.0 and
//! T48.8 (R14-F4, `docs/plan-agent-truth.md` §1.2).
//!
//! `docs/results-agent-cost.md` claimed zero allocations in steady state,
//! and `benches --bin agent` printed 4.00 per call: `live_nodes` built an
//! identity `Vec` on every call, and the journal and the auditor each made
//! a fresh `live` vector. The bench prints; nothing asserted. This binary
//! is its own, because `#[global_allocator]` is per binary.
//!
//! The tree is the shape `benches --bin agent` builds — ~900 ordinary
//! nodes — laid out twice: once in a tall viewport (few findings) and
//! once in a short one, where most of the tree is below the fold and the
//! audit carries hundreds of findings every frame.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};
use lumen_static_agent::{Agent, FrameTiming, TreeMut};
use lumen_static_widgets::{Fixed, Label, Spacer};

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
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(p, l, new) }
    }
}
#[global_allocator]
static A: Counting = Counting;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 2.0)]
struct Ten {
    l1: Label,
    f1: Fixed,
    s1: Spacer,
    l2: Label,
    f2: Fixed,
    l3: Label,
    f3: Fixed,
    s2: Spacer,
    l4: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 1.0)]
struct Block {
    a: Ten,
    b: Ten,
    c: Ten,
    d: Ten,
    e: Ten,
    f: Ten,
    g: Ten,
    h: Ten,
    i: Ten,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Tree {
    a: Block,
    b: Block,
    c: Block,
    d: Block,
    e: Block,
    f: Block,
    g: Block,
    h: Block,
    i: Block,
    j: Block,
}

fn per_frame(max_h: f32) -> (f64, usize) {
    let mut w = Tree::default();
    let sty = pass::default_styles(&w);
    let mut st = vec![NodeState::default(); Tree::NODE_COUNT];
    pass::layout(&mut w, &sty, &mut st, Constraints { min_w: 0.0, max_w: 1920.0, min_h: 0.0, max_h });
    let mut agent = Agent::new();
    for _ in 0..5 {
        agent.frame_done(&mut TreeMut::of_root(&mut w, &sty, &st), FrameTiming::passes(1, 1));
    }
    let before = ALLOCS.load(Ordering::Relaxed);
    for _ in 0..100 {
        agent.frame_done(&mut TreeMut::of_root(&mut w, &sty, &st), FrameTiming::passes(1, 1));
    }
    ((ALLOCS.load(Ordering::Relaxed) - before) as f64 / 100.0, agent.finding_count())
}

#[test]
#[ignore = "red until T48.8: R14-F4, frame_done allocates four times a call"]
fn frame_done_allocates_nothing_in_steady_state() {
    let (tall, tall_findings) = per_frame(100_000.0);
    let (short, short_findings) = per_frame(900.0);
    println!("{} nodes", Tree::NODE_COUNT);
    println!("allocs/frame_done {tall:.2} / bound 0 ({tall_findings} findings)");
    println!("allocs/frame_done {short:.2} / bound 0 ({short_findings} findings)");
    assert!(short_findings > 100, "setup: the short viewport carries only {short_findings} findings");
    assert_eq!(tall, 0.0, "frame_done allocated {tall:.2} times a call with few findings");
    assert_eq!(short, 0.0, "frame_done allocated {short:.2} times a call with {short_findings} findings");
}
