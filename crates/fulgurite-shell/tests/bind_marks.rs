//! **A binding that wrote marks the node it wrote** — M29/T29.3
//! (`docs/plan-damage-precision.md`).
//!
//! The invalidation oracle's property — byte-identical frames under
//! `Everything` and `Boundaries` — over host-driven writes to a bound
//! label, **plus** the assertion that makes it not vacuous: the
//! `Boundaries` frame actually narrowed. A mark that fell back to
//! everything draws the same frame, so equality alone cannot tell the
//! mechanism ran.
//!
//! The bound label sits one container down (`top`), not on the root: a
//! root that is dirty lays out all its children, so a label hung off it
//! could never be stale however wrong the mark was — the trap the grid
//! fixture fell into first (`docs/results-damage-precision.md` T29.0).

use fulgurite::invalidate::{Boundaries, Everything, Invalidation, Marks};
use fulgurite::prelude::*;
use fulgurite::retain::Incremental;
use fulgurite_shell::Host;
use fulgurite_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
struct Top {
    #[widget_id("formula")]
    formula: Label<String>,
    #[widget_id("status")]
    status: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 2.0)]
struct Filler {
    a: Label<String>,
    b: Label<String>,
    c: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 2.0)]
struct Ui {
    top: Top,
    filler: Filler,
}

#[derive(DeriveApp, Default)]
struct Sheet {
    #[tree]
    ui: Ui,
    #[bind(ui.top.formula, "= {}")]
    cell: i64,
}

fn app() -> Sheet {
    let mut s = Sheet::default();
    s.ui.top.status.set("Sheet1".into());
    s.ui.filler.a.set("alpha".into());
    s.ui.filler.b.set("beta".into());
    s.ui.filler.c.set("gamma".into());
    s
}

fn host<I: Invalidation>() -> Host<Sheet, Incremental, I> {
    let mut h: Host<Sheet, Incremental, I> = Host::new(app(), 400, 200);
    h.pump();
    h.pump();
    h
}

#[test]
fn a_host_driven_write_narrows_and_draws_the_same_frame() {
    let mut every: Host<Sheet, Incremental, Everything> = host();
    let mut bound: Host<Sheet, Incremental, Boundaries<16>> = host();
    // Values whose formatted widths differ, so a stale box is visible.
    for v in [1_234_567, 9, 42_000, -3] {
        every.app.cell = v;
        bound.app.cell = v;
        every.pump();
        bound.pump();
        let (he, hb) = (every.list.hash_from(0), bound.list.hash_from(0));
        let marks = bound.last_marks();
        println!("cell {v:>8}: Everything {he:#x}, Boundaries {hb:#x}, marks {marks:?}");
        assert_eq!(
            marks,
            Marks { all: false, ranges: 1 },
            "setup: the write did not narrow — a frame that marked everything proves nothing below"
        );
        assert_eq!(he, hb, "cell {v}: the narrowed frame differs from the whole-tree frame");
    }
}

#[test]
fn a_frame_that_wrote_nothing_marks_nothing() {
    let mut bound: Host<Sheet, Incremental, Boundaries<16>> = host();
    bound.pump();
    assert_eq!(bound.last_marks(), Marks { all: false, ranges: 0 }, "a still frame marked something");
}
