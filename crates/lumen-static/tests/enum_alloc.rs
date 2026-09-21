//! The enum derive allocated two `Vec`s per `layout` call, in
//! framework-generated code, against the zero-allocation-in-steady-state
//! constraint. Its variants now live in the main table, so there is
//! nothing to allocate (F4.3).

use std::alloc::{GlobalAlloc, Layout, System};
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

use lumen_static_widgets::ops::{Fixed, Label};
use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};

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

    let before = ALLOCS.load(Ordering::Relaxed);
    for _ in 0..100 {
        pass::layout(&mut ui, &sty, &mut st, VIEW);
    }
    let per_frame = (ALLOCS.load(Ordering::Relaxed) - before) as f64 / 100.0;
    assert_eq!(per_frame, 0.0, "three enum nodes allocated {per_frame} times per layout");
}
