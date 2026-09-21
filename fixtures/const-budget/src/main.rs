//! A large static tree, as a compile-time budget.
//!
//! `table_is_well_formed` is `const`-asserted at every derive expansion,
//! so its cost is paid by the compiler, not at runtime — and it was O(n²):
//! 20 s at 3.2k nodes, and at 10k a hard `long_running_const_eval`
//! failure, meaning a tree that large **could not be compiled at all**.
//! That is the only ceiling in this project on what a program is allowed
//! to express rather than how fast it runs
//! (`docs/results-const-eval.md`, `docs/plan-review-fixes.md` F5).
//!
//! This crate exists so a regression shows up as a build that takes
//! minutes instead of a property nobody re-checks. `just const-budget`
//! times it against a threshold.

use lumen_static_widgets::ops::{Fixed, Label, Spacer};
use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Leafy {
    a: Label,
    b: Fixed,
    c: Spacer,
    d: Fixed,
    e: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Six {
    a: Leafy,
    b: Leafy,
    c: Leafy,
    d: Leafy,
    e: Leafy,
    f: Leafy,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Big {
    a: Six,
    b: Six,
    c: Six,
    d: Six,
    e: Six,
    f: Six,
    g: Six,
    h: Six,
    i: Six,
    j: Six,
}

/// Ten `Big`s: ~3,800 nodes, comfortably past the size that used to cost
/// 20 s and within sight of the one that would not build.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Huge {
    a: Big,
    b: Big,
    c: Big,
    d: Big,
    e: Big,
    f: Big,
    g: Big,
    h: Big,
    i: Big,
    j: Big,
}

fn main() {
    let mut w = Huge::default();
    let sty = pass::default_styles(&w);
    let mut st = vec![NodeState::default(); Huge::NODE_COUNT];
    let c = Constraints { min_w: 0.0, max_w: 1920.0, min_h: 0.0, max_h: 1080.0 };
    pass::layout(&mut w, &sty, &mut st, c);

    // The table was verified at compile time; say so at runtime too, so
    // the binary is not merely a build artifact.
    assert!(lumen_static::widget::table_is_well_formed(<Huge as Widget>::SUB_TABLE));
    println!("const-budget: {} nodes, depth {}", Huge::NODE_COUNT, <Huge as Widget>::DEPTH);
}
