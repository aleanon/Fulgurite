//! The enum derive allocated two `Vec`s per `layout` call, in
//! framework-generated code, against the zero-allocation-in-steady-state
//! constraint. Its variants now live in the main table, so there is
//! nothing to allocate (F4.3).

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    /// **Per-thread, not a global counter.**
    ///
    /// `cargo test` runs a binary's tests on parallel threads sharing
    /// one process and one allocator, so a global count includes the
    /// harness's own allocations and anything another test is doing.
    /// This file had a `static AtomicUsize` and failed once under load
    /// while passing eight times out of eight in isolation — the same
    /// defect `crates/fulgurite-shell/tests/alloc_gate.rs` documents
    /// having already fixed, where it reported "0.16 allocations per
    /// frame from code that allocates none".
    ///
    /// A gate that flakes is a gate that gets ignored, which is worse
    /// than not having one.
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn allocs() -> usize {
    ALLOCS.with(Cell::get)
}

struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        // `try_with`: during thread teardown the TLS is gone, and a
        // panic in the allocator is not recoverable.
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}
#[global_allocator]
static A: Counting = Counting;

use fulgurite_widgets::ops::{Fixed, Label};
use fulgurite::{pass, Constraints, NodeState, StaticTree, Widget};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Panel {
    a: Label,
    b: Fixed,
}

#[derive(StaticTree)]
enum Slot {
    Text(Label),
    Nested(Panel),
}

impl Default for Slot {
    fn default() -> Self {
        Slot::Text(Label::default())
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    one: Slot,
    two: Slot,
    three: Slot,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 400.0 };

#[test]
fn laying_out_enum_nodes_allocates_nothing() {
    let mut ui = Ui { two: Slot::Nested(Panel::default()), ..Default::default() };
    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Ui::NODE_COUNT];

    // Warm up: the first pass may grow whatever the leaves own.
    for _ in 0..5 {
        pass::layout(&mut ui, &sty, &mut st, VIEW);
    }

    let before = allocs();
    for _ in 0..100 {
        pass::layout(&mut ui, &sty, &mut st, VIEW);
    }
    let per_frame = (allocs() - before) as f64 / 100.0;
    assert_eq!(per_frame, 0.0, "three enum nodes allocated {per_frame} times per layout");
}
