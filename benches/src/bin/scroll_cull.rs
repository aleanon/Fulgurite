//! **What culling a scrolled derived column is worth** — M85b
//! (`docs/plan-layout-input.md`, T85.6 and K85b), for
//! `docs/results-layout-input.md`.
//!
//! A derived 220-row column in a 400 px `ScrollView` at the top of an
//! 800 × 800 window, scrolled to the middle, painted into a `DisplayList`
//! twice per run: once with `set_bounds(Some(surface))`, as `Host::frame`
//! does, and once with `None`, which culls nothing anywhere. Each arm is
//! timed for `RUNS` runs, interleaved, and prints its median and its
//! spread (the run medians' minimum and maximum) beside its command count.
//!
//! **The command counts are the setup check** (the plan's B13). An arm
//! that culls nothing shows it: at T85.6 a derived container does not
//! cull, so the two counts are equal and the times differ only by noise.
//! The rows in the window and in the viewport are printed beside them, so
//! the count a culling arm should reach is visible in the same output.
//! K85b judges the culled median against the uncull arm's spread, once
//! there is a culled arm to judge.
//!
//! **220 rows, not the plan's 1,000.** A `ScrollView`'s content is
//! spliced into its own table, which holds `MAX_SCROLL_NODES` (256)
//! entries, so a derived column of a thousand leaves does not compile
//! inside one (`docs/plan-layout-input.md`, "What building found",
//! T85.6). The column is twenty-two derived columns of ten `Fixed` rows,
//! every level a derived container and every row a leaf on the table:
//! 243 nodes: six and a half windows of rows, of which the window holds
//! 34 and the viewport 17 (printed).
//!
//! Run: `just bench-lock cargo run --release -p benches --bin scroll_cull`

use std::hint::black_box;
use std::time::Instant;

use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{pass, Constraints, DisplayList, NodeState, StaticTree, Widget};
use lumen_static_widgets::ops::Fixed;
use lumen_static_widgets::ScrollView;

/// Ten 24 px rows.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct R10 {
    a: Fixed,
    b: Fixed,
    c: Fixed,
    d: Fixed,
    e: Fixed,
    f: Fixed,
    g: Fixed,
    h: Fixed,
    i: Fixed,
    j: Fixed,
}

/// Twenty-two blocks of ten: 220 rows, 243 nodes -- the largest round
/// column a `ScrollView` holds (`MAX_SCROLL_NODES`, 256, counts the
/// content's nodes; see the module note).
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Rows {
    b0: R10,
    b1: R10,
    b2: R10,
    b3: R10,
    b4: R10,
    b5: R10,
    b6: R10,
    b7: R10,
    b8: R10,
    b9: R10,
    b10: R10,
    b11: R10,
    b12: R10,
    b13: R10,
    b14: R10,
    b15: R10,
    b16: R10,
    b17: R10,
    b18: R10,
    b19: R10,
    b20: R10,
    b21: R10,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("scroll")]
    scroll: ScrollView<Rows>,
}

const WINDOW: f32 = 800.0;
const VIEWPORT_H: f32 = 400.0;
const ROW_H: f32 = 24.0;
const ROWS: usize = 220;
/// Scrolled to the middle of the column's 5,280 px.
const OFFSET: f32 = 2_640.0;
/// Runs a side, interleaved.
const RUNS: usize = 8;
/// Paints per run; the run's figure is their median.
const PAINTS: usize = 4000;

fn run(ui: &Ui, st: &[NodeState], list: &mut DisplayList, bounds: Option<[f32; 4]>) -> (f64, usize) {
    let mut ns: Vec<f64> = (0..PAINTS)
        .map(|_| {
            list.clear();
            list.set_bounds(bounds);
            let t = Instant::now();
            pass::paint(black_box(ui), st, list);
            let e = t.elapsed().as_nanos() as f64;
            black_box(list.cmds.len());
            e
        })
        .collect();
    ns.sort_by(|a, b| a.total_cmp(b));
    (ns[PAINTS / 2] / 1e3, list.cmds.len())
}

fn summary(v: &mut [f64]) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.total_cmp(b));
    let m = (v[v.len() / 2 - 1] + v[v.len() / 2]) / 2.0;
    (m, v[0], v[v.len() - 1])
}

fn main() {
    let mut ui = Ui::default();
    let mut styles = pass::default_styles(&ui);
    let sheet = [Rule::new(
        Selector::Id("scroll"),
        Patch::new().width(Length::Fixed(400.0)).height(Length::Fixed(VIEWPORT_H)),
    )];
    lumen_static::style::cascade(<Ui as Widget>::SUB_TABLE, <Ui as Widget>::SEEDS, &sheet, &mut styles);
    let mut st = vec![NodeState::default(); <Ui as Widget>::NODE_COUNT];
    let view = Constraints { min_w: 0.0, max_w: WINDOW, min_h: 0.0, max_h: WINDOW };
    pass::layout(&mut ui, &styles, &mut st, view);
    ui.scroll.scroll = OFFSET;
    pass::layout(&mut ui, &styles, &mut st, view);

    // Setup: the scroller is the viewport the plan names, scrolled, and
    // every row is a laid-out leaf.
    let vp = st[1];
    let leaves: Vec<&NodeState> =
        <Ui as Widget>::SUB_TABLE.iter().zip(st.iter()).filter(|(d, _)| d.child_count == 0).map(|(_, s)| s).collect();
    assert_eq!(leaves.len(), ROWS, "setup: {} leaves, not {ROWS} rows", leaves.len());
    assert_eq!(vp.size.h, VIEWPORT_H, "setup: the viewport is {} px, not {VIEWPORT_H}", vp.size.h);
    assert_eq!(ui.scroll.scroll, OFFSET, "setup: the scroller sits at {}, not {OFFSET}", ui.scroll.scroll);
    let meets = |s: &NodeState, top: f32, bottom: f32| s.pos.y < bottom && s.pos.y + s.size.h > top;
    let in_window = leaves.iter().filter(|s| meets(s, 0.0, WINDOW)).count();
    let in_viewport = leaves.iter().filter(|s| meets(s, vp.pos.y, vp.pos.y + vp.size.h)).count();
    assert!(leaves.iter().all(|s| s.size.h == ROW_H), "setup: a row is not {ROW_H} px tall");
    assert!(
        in_window > in_viewport && in_viewport > 0,
        "setup: rows in the window {in_window}, in the viewport {in_viewport} -- the window must hold rows the viewport does not"
    );

    let surface = Some([0.0, 0.0, WINDOW, WINDOW]);
    let mut list = DisplayList::new();
    let (mut some, mut none) = (Vec::new(), Vec::new());
    let (mut n_some, mut n_none) = (0, 0);
    for _ in 0..RUNS {
        let (t, n) = run(&ui, &st, &mut list, surface);
        some.push(t);
        n_some = n;
        let (t, n) = run(&ui, &st, &mut list, None);
        none.push(t);
        n_none = n;
    }
    assert!(n_none > ROWS, "setup: the uncull arm painted {n_none} commands for {ROWS} rows");

    println!(
        "scroll_cull: a derived {ROWS}-row column ({ROW_H} px rows) in a {VIEWPORT_H} px ScrollView, {WINDOW}x{WINDOW} window, offset {OFFSET}"
    );
    println!("rows: {ROWS} on the table, {in_window} in the window, {in_viewport} in the viewport");
    println!("{RUNS} runs a side, interleaved, each the median of {PAINTS} paints\n");
    println!("{:>24} {:>10} {:>12} {:>22}", "arm", "commands", "median µs", "spread µs");
    let (m, lo, hi) = summary(&mut some);
    println!("{:>24} {n_some:>10} {m:>12.2} {:>22}", "bounds Some(surface)", format!("{lo:.2}–{hi:.2}"));
    let (m, lo, hi) = summary(&mut none);
    println!("{:>24} {n_none:>10} {m:>12.2} {:>22}", "bounds None", format!("{lo:.2}–{hi:.2}"));
    println!();
    if n_some == n_none {
        println!("culls nothing: Some(surface) painted {n_some} commands, None {n_none} (equal)");
    } else {
        println!("culled: Some(surface) painted {n_some} commands against None's {n_none}");
    }
}
