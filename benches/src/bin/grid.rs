//! M29/T29.0 and T29.1: a spreadsheet-shaped tree, and the probe that
//! decides whether narrowing a binding's mark is worth building.
//!
//! **Why a new fixture.** The blast radius of `#[bind]` — one binding
//! writing one label makes `Host::pump_with` call `mark_all()` — was
//! measured on 901 synthetic nodes and reported as 9.7× on *layout*
//! (`docs/results-frame-and-types.md`, finding 14). Nothing in the tree
//! can say whether that matters in a frame: the tally is about twenty
//! nodes, and `just stages` drives 10 000 rows that are **virtualized**,
//! so they are not on the table and `#[bind]` never touches them.
//!
//! So this fixture has both halves, which is what
//! `docs/plan-damage-precision.md` §2.1 requires:
//!
//! - **thousands of table nodes** — the frozen grid, which is what a
//!   `mark_all` walks;
//! - **a virtualized body** through `Repeat`, because that is how a
//!   spreadsheet is really built;
//! - **one `#[bind]`-driven label on the table** — the formula bar. If
//!   the whole grid were virtualized a cell edit would never go through
//!   `#[bind]` at all and this would measure nothing;
//! - **varied text**, because shaping dominates a real frame and a
//!   fixture of one repeated string would flatter a layout-only win.
//!
//! **Four things a seven-lens review of M29 found wrong with the first
//! version of this file**, each of which had made a published number
//! mean something other than what it said:
//!
//! 1. **The body was virtualized by declaration only.** Nothing bounded
//!    the sheet, so a column child got `remaining_main.max(0.0)` = 0, the
//!    `Repeat` was laid out at 1400×0 and materialized **one** row of its
//!    five thousand. The stylesheet below gives both panes a height, and
//!    `window_len()` is now asserted rather than assumed.
//! 2. **Nothing shaped.** `Host::pump` measures with `BlockMetrics` — a
//!    fixed advance per byte — so the "varied text" was varied strings
//!    through a stand-in that cannot see them. It drives a real face now.
//! 3. **The "layout" column was layout + place**, because that is what
//!    `Host::timings` has always returned. `place` takes no `Changed`, so
//!    it is the one pass a narrower mark can never move; folded into
//!    layout it read as residue still to be removed.
//! 4. **Both arms wrote the bound value directly**, which is the one way
//!    a real application never changes it. `pump_with` calls `mark_all`
//!    for a *handled event* eighty lines before it asks `App::update`
//!    anything, so on the frame an actual click produces, the narrowing
//!    this milestone exists to build is already unreachable. `arm_c`
//!    measures that frame and the probe reports it beside the others.
//!
//! **And the fifth, from M38 (R6-F2): the `frame` column was not the
//! frame.** It is layout + place + paint + raster, and `pump_with` also
//! dispatches, drains, runs the bindings, cascades the arenas and closes
//! retention. Since M53/T53.0 every arm pumps through [`pump`], which
//! times the whole call, and each run prints the pump and its gap from
//! the stage sum, bounded at [`GAP_BOUND`] of the pump
//! (`docs/plan-frame-cost.md`). The binary exits non-zero when a run's
//! gap is over it.
//!
//! Run: `just grid`, or `cargo run --release -p benches --bin grid`

use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use lumen_static_widgets::ops::{Button, Label};
use lumen_static::repeat::Repeat;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{App as DeriveApp, EventCtx, StaticTree, Widget};
use lumen_static::display::Color;
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::{FontMetrics, FontStack};
use lumen_static::cx::TextMeasure;
use lumen_static_shell::Host;

// ---- the frozen grid: table nodes ------------------------------------

/// Ten cells. 11 nodes.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 1.0)]
pub struct GridRow {
    #[widget_id("cell")]
    c0: Label<String>,
    c1: Label<String>,
    c2: Label<String>,
    c3: Label<String>,
    c4: Label<String>,
    c5: Label<String>,
    c6: Label<String>,
    c7: Label<String>,
    c8: Label<String>,
    c9: Label<String>,
}

/// Ten rows. 111 nodes.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 1.0)]
pub struct GridBlock {
    r0: GridRow,
    r1: GridRow,
    r2: GridRow,
    r3: GridRow,
    r4: GridRow,
    r5: GridRow,
    r6: GridRow,
    r7: GridRow,
    r8: GridRow,
    r9: GridRow,
}

/// Twenty blocks: 2 221 nodes of frozen grid.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 2.0)]
pub struct Sheet {
    b0: GridBlock,
    b1: GridBlock,
    b2: GridBlock,
    b3: GridBlock,
    b4: GridBlock,
    b5: GridBlock,
    b6: GridBlock,
    b7: GridBlock,
    b8: GridBlock,
    b9: GridBlock,
    b10: GridBlock,
    b11: GridBlock,
    b12: GridBlock,
    b13: GridBlock,
    b14: GridBlock,
    b15: GridBlock,
    b16: GridBlock,
    b17: GridBlock,
    b18: GridBlock,
    b19: GridBlock,
}

/// One scrolled row, off the table in an arena.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 1.0)]
pub struct BodyRow {
    a: Label<String>,
    b: Label<String>,
    c: Label<String>,
}

/// **Through `format_into`, not `set(format!(..))`** — M29/T29.5.
///
/// A repeater binds every materialized row on every frame, so a bind is
/// steady-state code. The obvious spelling heap-allocates once per row
/// per frame, which measured **10.8 µs** on this fixture's 18-row window
/// — eight times the framework's own cost for re-binding it. That is
/// what the first version of this file was measuring and attributing to
/// `Repeat::layout` (`docs/results-damage-precision.md` T29.5).
fn bind_body(row: &mut BodyRow, i: usize) {
    use lumen_static::bind::format_into;
    format_into(&mut row.a, format_args!("r{i}"));
    format_into(&mut row.b, format_args!("{:.2}", i as f32 * 1.37));
    format_into(&mut row.c, format_args!("{}", if i.is_multiple_of(3) { "pending" } else { "ok" }));
}

/// The toolbar. **Its own container, and that is load-bearing:** a leaf
/// hanging directly off the root can never go stale, because any mark
/// anywhere dirties the root and a container lays out *all* its children.
/// The first version of this fixture put the formula bar there, and the
/// injection that marks the wrong subtree could not fail against it.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
pub struct TopBar {
    /// **The bound label, and it is on the table** — the property the
    /// fixture asserts before it measures anything.
    #[widget_id("formula")]
    formula: Label<String>,
    #[widget_id("status")]
    status: Label<String>,
    /// **The only honest way to change a bound value.** Every `#[bind]`
    /// in this workspace is written by a click handler, and a handled
    /// event marks the whole tree before `App::update` is consulted —
    /// so an arm that writes `cell` from outside measures a frame shape
    /// no interactive application produces. `arm_c` goes through here.
    #[widget_id("bump")]
    bump: Button<Grid>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 2.0)]
pub struct Ui {
    top: TopBar,
    /// **Bounded by the stylesheet**, so the body below it gets a
    /// viewport. Unbounded, this pane consumed the whole window and the
    /// repeater materialized one zero-height row.
    #[widget_id("sheet")]
    sheet: Sheet,
    #[widget_id("body")]
    body: Repeat<BodyRow>,
}

#[derive(DeriveApp, Default)]
pub struct Grid {
    #[tree]
    ui: Ui,
    /// The formula bar's source: one value, one bound label.
    #[bind(ui.top.formula, "= {}")]
    cell: i64,
}

const W: u32 = 1400;
const H: u32 = 900;
const ROWS: usize = 5000;
const ROW_H: f32 = 18.0;
/// The frozen pane's height. Bounded so the body gets the rest — see
/// `Ui::sheet`.
const SHEET_H: f32 = 560.0;
/// The scrolling body's viewport. Enough rows that `Repeat` is doing its
/// job rather than reporting a window of one.
const BODY_H: f32 = 300.0;
const PAGE: Color = Color(255, 255, 255, 255);

/// `cell += 1`, reached by a click. The handler writes **application
/// state**, not the tree; `#[bind]` turns that into the label. That is
/// the ordinary shape, and it is the shape on which `handled` has
/// already called `mark_all` by the time the binding runs.
fn on_bump(g: &mut Grid, _: &EventCtx) {
    g.cell += 1;
}

fn sheet_rules() -> Vec<Rule> {
    vec![
        Rule::new(Selector::Id("sheet"), Patch::new().height(Length::Fixed(SHEET_H))),
        Rule::new(Selector::Id("body"), Patch::new().height(Length::Fixed(BODY_H))),
    ]
}

fn app() -> Grid {
    let mut g = Grid::default();
    g.ui.top.status.set("Sheet1".into());
    g.ui.top.bump.label = "+";
    g.ui.top.bump.on_click = on_bump;
    g.ui.body = Repeat::new(ROWS, ROW_H, bind_body);
    fill_sheet(&mut g.ui.sheet);
    g
}

/// The frozen pane's cell text, shared by the unsplit grid and its split
/// twin so the two draw the same sheet.
fn fill_sheet(sheet: &mut Sheet) {
    // Varied cell text: shaping is part of a real frame, and identical
    // strings would let the memo hide it.
    let mut n = 0usize;
    for b in [
        &mut sheet.b0, &mut sheet.b1, &mut sheet.b2, &mut sheet.b3,
        &mut sheet.b4, &mut sheet.b5, &mut sheet.b6, &mut sheet.b7,
        &mut sheet.b8, &mut sheet.b9, &mut sheet.b10, &mut sheet.b11,
        &mut sheet.b12, &mut sheet.b13, &mut sheet.b14, &mut sheet.b15,
        &mut sheet.b16, &mut sheet.b17, &mut sheet.b18, &mut sheet.b19,
    ] {
        for r in [
            &mut b.r0, &mut b.r1, &mut b.r2, &mut b.r3, &mut b.r4,
            &mut b.r5, &mut b.r6, &mut b.r7, &mut b.r8, &mut b.r9,
        ] {
            for c in [
                &mut r.c0, &mut r.c1, &mut r.c2, &mut r.c3, &mut r.c4,
                &mut r.c5, &mut r.c6, &mut r.c7, &mut r.c8, &mut r.c9,
            ] {
                n += 1;
                c.set(match n % 5 {
                    0 => format!("{:.2}", n as f32 * 0.37),
                    1 => format!("Q{}", n % 4 + 1),
                    2 => format!("{}", n * 7),
                    3 => "—".into(),
                    _ => format!("row {n}"),
                });
            }
        }
    }
}

// ---- M33/T33.4: the same grid, split ---------------------------------

/// **The grid's split twin** (`docs/plan-state-split.md` T33.4).
///
/// The tree types are duplicated only where a handler's state type
/// appears — `TopBar` carries `Button<Cell>` — and the handler-free sheet
/// and body are shared. The unsplit grid above stays: M29/T29.1's
/// recorded figures were measured on it, and its twin is what T29.6
/// measures a narrowed handled frame on.
mod split {
    use super::*;

    #[derive(StaticTree, Default)]
    #[widget(row, spacing = 4.0)]
    pub struct TopBar {
        #[widget_id("formula")]
        pub formula: Label<String>,
        #[widget_id("status")]
        pub status: Label<String>,
        #[widget_id("bump")]
        pub bump: Button<Cell>,
    }

    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 2.0)]
    pub struct Ui {
        pub top: TopBar,
        #[widget_id("sheet")]
        pub sheet: Sheet,
        #[widget_id("body")]
        pub body: Repeat<BodyRow>,
    }

    /// What `on_bump` may touch. The tree is not in here.
    #[derive(Default)]
    pub struct Cell {
        pub value: i64,
    }

    #[derive(DeriveApp, Default)]
    pub struct Grid {
        #[tree]
        pub ui: Ui,
        #[state]
        #[bind(ui.top.formula, value, "= {}")]
        pub cell: Cell,
    }

    pub fn app() -> Grid {
        let mut g = Grid::default();
        g.ui.top.status.set("Sheet1".into());
        g.ui.top.bump.label = "+";
        g.ui.top.bump.on_click = |c: &mut Cell, _: &EventCtx| c.value += 1;
        g.ui.body = Repeat::new(ROWS, ROW_H, bind_body);
        fill_sheet(&mut g.ui.sheet);
        g
    }
}

// The twin is split, checked where it costs nothing.
const _: () = assert!(<split::Grid as lumen_static::event::App>::STATE_IS_SPLIT);

type BoundedSplit = Host<split::Grid, lumen_static::Incremental, lumen_static::Boundaries<16>>;

fn host_split(m: &mut dyn TextMeasure) -> BoundedSplit {
    let mut h: BoundedSplit = Host::new(split::app(), W, H);
    h.set_stylesheet(&sheet_rules());
    h.pump_with(m);
    h.pump_with(m);
    h
}

type Bounded = Host<Grid, lumen_static::Incremental, lumen_static::Boundaries<16>>;

/// The measurer the probe runs under.
///
/// **A real face, not `BlockMetrics`.** `Host::pump` measures with a
/// fixed advance per byte, which cannot see the difference between
/// "Q3" and "row 1417" beyond their lengths — so a fixture that varies
/// its text and then measures it with a stand-in has varied nothing.
/// `benches --bin bindings` names the same trap for the memo arms.
fn metrics() -> Box<dyn TextMeasure> {
    match FontStack::system() {
        Some(stack) => Box::new(FontMetrics::new(stack)),
        // Stated rather than silently substituted: a run with no system
        // font is measuring something else and its numbers do not
        // belong beside the published ones.
        None => {
            println!("!! no system font: falling back to BlockMetrics — NOT the published arm");
            Box::new(lumen_static::cx::BlockMetrics)
        }
    }
}

fn host(m: &mut dyn TextMeasure) -> Bounded {
    let mut h: Bounded = Host::new(app(), W, H);
    h.set_stylesheet(&sheet_rules());
    h.pump_with(m);
    h.pump_with(m);
    h
}

/// The whole `pump_with`, timed from outside — M53/T53.0 (R6-F2).
///
/// Every arm pumps through this, so the probe can print what a frame
/// actually costs beside the stage sum it used to publish as "the
/// frame". `Host::timings3` covers layout, place and paint; the pump
/// also dispatches, drains, runs the bindings, cascades the arenas and
/// closes retention, and none of that was in the sum
/// (`docs/plan-frame-cost.md` §1.1).
static PUMP_NS: AtomicU64 = AtomicU64::new(0);

fn pump<A: lumen_static::event::App>(
    h: &mut Host<A, lumen_static::Incremental, lumen_static::Boundaries<16>>,
    m: &mut dyn TextMeasure,
) {
    let t = Instant::now();
    h.pump_with(m);
    PUMP_NS.store(t.elapsed().as_nanos() as u64, Ordering::Relaxed);
}

/// The gap bound: the pump minus the stages published as its cost may be
/// at most this share of the pump. Five per cent, from
/// `docs/plan-frame-cost.md` T53.0; a stage nothing times shows up here.
const GAP_BOUND: f64 = 0.05;

/// One run's per-frame samples, and the line it prints — M53/T53.1.
///
/// Every column comes from `Host::stage_timings`, the host timing
/// itself, so a stage `pump_with` gains later is gap here rather than a
/// sum that silently goes stale (`docs/plan-frame-cost.md` §2.1). The
/// `stage3` column is what the table published as "the frame" until
/// T53.1 (layout + place + paint + raster), kept beside the new one so a
/// reader can see which figure an older citation used.
#[derive(Default)]
struct Samples {
    input: Vec<f64>,
    cascade: Vec<f64>,
    layout: Vec<f64>,
    place: Vec<f64>,
    paint: Vec<f64>,
    finish: Vec<f64>,
    raster: Vec<f64>,
    pump: Vec<f64>,
    gap: Vec<f64>,
    outer: Vec<f64>,
    px: Vec<f64>,
}

/// The medians of one run, in µs.
struct Run {
    input: f64,
    cascade: f64,
    layout: f64,
    place: f64,
    paint: f64,
    finish: f64,
    raster: f64,
    pump: f64,
    gap: f64,
    outer: f64,
    px: f64,
}

impl Samples {
    /// Read the frame `h` just pumped, then rasterise its damage.
    ///
    /// **Damage-bounded, as a host rasterises**: clear and render only
    /// what retention says changed. `Host::render` draws the whole list,
    /// which no shell does and which would inflate the frame this probe
    /// is judged against.
    fn record<A: lumen_static::event::App>(
        &mut self,
        h: &mut Host<A, lumen_static::Incremental, lumen_static::Boundaries<16>>,
        canvas: &mut Canvas,
    ) {
        let us = |ns: u64| ns as f64 / 1000.0;
        let st = h.stage_timings();
        self.input.push(us(st.input));
        self.cascade.push(us(st.cascade));
        self.layout.push(us(st.layout));
        self.place.push(us(st.place));
        self.paint.push(us(st.paint));
        self.finish.push(us(st.finish));
        self.pump.push(us(st.pump));
        self.gap.push(st.gap() as f64 / 1000.0);
        self.outer.push(us(PUMP_NS.load(Ordering::Relaxed)));
        let dmg = h.take_damage_for_age(1).clip_to([0.0, 0.0, W as f32, H as f32]);
        let t = Instant::now();
        match dmg {
            Some(area) => {
                canvas.clear_within(area, PAGE);
                canvas.render_within(&h.list, area);
                self.px.push(((area[2] - area[0]) * (area[3] - area[1])) as f64);
            }
            None => self.px.push(0.0),
        }
        self.raster.push(t.elapsed().as_nanos() as f64 / 1000.0);
    }

    fn run(mut self) -> Run {
        Run {
            input: med(&mut self.input),
            cascade: med(&mut self.cascade),
            layout: med(&mut self.layout),
            place: med(&mut self.place),
            paint: med(&mut self.paint),
            finish: med(&mut self.finish),
            raster: med(&mut self.raster),
            pump: med(&mut self.pump),
            gap: med(&mut self.gap),
            outer: med(&mut self.outer),
            px: med(&mut self.px),
        }
    }
}

impl Run {
    /// Layout + place + paint + raster: the old "frame" column.
    fn stage3(&self) -> f64 {
        self.layout + self.place + self.paint + self.raster
    }

    /// The whole pump plus the raster: the frame, since T53.1.
    fn frame(&self) -> f64 {
        self.pump + self.raster
    }

    fn print(&self, arm: &str) {
        println!(
            "{arm:<13} {:>7.2} {:>7.2} {:>7.2} {:>7.2} {:>7.2} {:>7.2} {:>7.2} {:>8.2} {:>8.2} {:>8.2} {:>7.2} {:>8.2} {:>9.0}",
            self.input,
            self.cascade,
            self.layout,
            self.place,
            self.paint,
            self.finish,
            self.raster,
            self.stage3(),
            self.pump,
            self.frame(),
            self.gap,
            self.outer,
            self.px,
        );
    }
}

/// One run of `iters` frames of `act`, as the probe loop in `main` does
/// it. Generic so the split twin (M29/T29.6) is measured by the same code
/// rather than a copy that could drift.
fn measure<A: lumen_static::event::App>(
    h: &mut Host<A, lumen_static::Incremental, lumen_static::Boundaries<16>>,
    iters: usize,
    m: &mut dyn TextMeasure,
    mut act: impl FnMut(&mut Host<A, lumen_static::Incremental, lumen_static::Boundaries<16>>, &mut dyn TextMeasure),
) -> Run {
    let mut canvas = Canvas::new(W, H);
    let mut s = Samples::default();
    for _ in 0..iters {
        act(h, m);
        s.record(h, &mut canvas);
    }
    s.run()
}

/// The median of `v`, which is sorted in place.
fn med(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// One frame of each arm, reported as (layout µs, paint µs, raster µs).
///
/// **Arm A** is what `#[bind]` forces today: the bound value changes, so
/// `App::update` writes the label, returns `true`, and the host marks
/// everything.
///
/// **Arm B** is the scaffold M29/T29.1 measures against: the same frame
/// content, reached by writing the label directly and marking only its
/// subtree. `App::update` then formats the same string the label already
/// holds, compares, writes nothing and reports `false` — so `mark_all`
/// never runs.
///
/// **Arm C is the frame an application actually produces**, and it is
/// the one the first version of this probe did not have. A click is
/// dispatched, a handler consumes it, and `pump_with` answers that with
/// `mark_all()` at `lumen-static-shell/src/lib.rs:694` — *before* it
/// asks `App::update` anything. So whatever the binding reports, the
/// tree is already fully marked, and the saving B measures is
/// unreachable. A and B differ in what they mark; C differs in **who
/// caused the change**, which is the variable that decides whether any
/// of this milestone is reachable at all.
fn arm_a(h: &mut Bounded, v: i64, m: &mut dyn TextMeasure) {
    h.app.cell = v;
    pump(h, m);
}

/// **The control, since M29/T29.3.** Arm A used to *be* the `mark_all`
/// frame, because `#[bind]` marked everything; now the binding marks its
/// own node, so arm A narrows and this arm forces what it used to do —
/// or the comparison would collapse into itself (the plan's T29.3
/// warning).
fn arm_forced(h: &mut Bounded, v: i64, m: &mut dyn TextMeasure) {
    h.app.cell = v;
    h.invalidate_all();
    pump(h, m);
}

fn arm_b(h: &mut Bounded, v: i64, m: &mut dyn TextMeasure) {
    h.app.cell = v;
    h.app.ui.top.formula.set(format!("= {v}"));
    assert!(h.invalidate("formula"), "setup: #formula is on the table");
    pump(h, m);
}

fn arm_c(h: &mut Bounded, at: (f32, f32), m: &mut dyn TextMeasure) {
    h.click(at.0, at.1);
    pump(h, m);
}

fn main() {
    black_box(());
    let table = <Ui as Widget>::NODE_COUNT;
    let m = &mut *metrics();

    // ---- T29.0: the fixture asserts its own shape ---------------------
    //
    // An arm that cannot exhibit the defect is a finding about the arm,
    // so these run before any number is taken from it. **Every bound
    // prints its measured quantity beside it**, because a bound that
    // cannot be exceeded reads exactly like one that holds.
    assert!(table >= 2000, "the frozen grid is {table} nodes, too few to show a whole-tree walk");
    let formula_at = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "formula")
        .expect("#formula must be a TABLE node — in an arena it would never go through #[bind]");
    let mut h = host(m);
    assert_eq!(h.app.ui.top.formula.as_str(), "= 0", "setup: the binding wrote the formula bar");

    // A single edit changes exactly one bound value.
    let status_before = h.app.ui.top.status.as_str().to_owned();
    h.app.cell = 41;
    h.pump_with(m);
    assert_eq!(h.app.ui.top.formula.as_str(), "= 41", "the edit reached the bound label");
    assert_eq!(h.app.ui.top.status.as_str(), status_before, "the edit touched a second label");

    // **The body must actually virtualize.** §2.1 requires a virtualized
    // body; the first version of this fixture satisfied that by
    // *declaration* — `Repeat::new(5000, ..)` — while the sheet ate the
    // whole column and left it a 1400x0 box, so it materialized one row.
    // `ROWS` is a constructor argument; these two are behaviour.
    let (win, vp) = (h.app.ui.body.window_len(), h.app.ui.body.viewport());
    assert!(win > 1, "the body materialized {win} row(s) — it is not virtualizing anything");
    assert!(vp > 0.0, "the body's viewport is {vp} px — nothing below the fold exists");

    // **What the frame actually contains.** 2,226 nodes exist; the
    // number that matters for a layout walk is how many have a box, and
    // for a paint cull how many are off the surface. Printed, not
    // asserted into a shape — a later reader needs to scale the headline.
    let laid_out = h.states.iter().filter(|s| s.size.w > 0.0 && s.size.h > 0.0).count();
    let off_surface = h.states.iter().filter(|s| s.pos.y >= H as f32 || s.pos.x >= W as f32).count();

    println!(
        "grid: {table} table nodes, {laid_out} with a non-degenerate box, \
         {off_surface} off-surface\n      \
         body: {win} of {ROWS} rows materialized, viewport {vp:.0} px, \
         #formula at table index {formula_at}\n"
    );

    // ---- M50/T50.2: the glyph cache holds a real screen ---------------
    //
    // The timed runs below rasterise on `Canvas::new`, block glyphs, so
    // their raster column is comparable with every table already
    // published. This draws the same grid on a truetype canvas instead —
    // three full frames, then twenty edits over their damage — and prints
    // the canvas's glyph cache. `uncached` is the risk
    // `docs/plan-raster-allocation.md` §5 names: a cap too small for a real
    // screen would keep the allocation gate green while a window
    // allocated. It must read 0, and so must the misses after warm-up.
    //
    // **`--shaping basic|complex`** (M81/T81.3, for K81b in T81.8): the
    // truetype canvas's stack in that mode; `complex` needs the benches
    // `shaping` feature. The runs the shaper laid out are printed as the
    // setup. Since K81a's response (`docs/plan-shaped-paint.md`, "What
    // building found") a run with nothing to shape takes fontdue's path
    // under `Complex`, so this grid's Latin and digits print 0 there.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let shaping = match args.iter().position(|a| a == "--shaping").map(|i| args.get(i + 1).map(String::as_str)) {
        None | Some(Some("basic")) => lumen_static_shell::text::Shaping::Basic,
        #[cfg(feature = "shaping")]
        Some(Some("complex")) => lumen_static_shell::text::Shaping::Complex,
        Some(other) => panic!("--shaping takes basic or complex (complex needs the `shaping` feature), not {other:?}"),
    };
    if let Some(face) = FontStack::system() {
        let face = face.with_shaping(shaping);
        let shaped0 = lumen_static_shell::text::text_counters();
        let mut hw = host(m);
        let mut tc = Canvas::with_font(W, H, face);
        for _ in 0..3 {
            lumen_static::Renderer::render(&mut tc, &hw.list, W, H);
        }
        let _ = hw.take_damage_for_age(1);
        let warm = tc.glyph_cache_stats();
        for k in 0..20 {
            arm_a(&mut hw, 1000 + k, m);
            for r in hw.take_damage_for_age(1).regions([0.0, 0.0, W as f32, H as f32]) {
                tc.clear_within(r, PAGE);
                tc.render_within(&hw.list, r);
            }
        }
        let after = tc.glyph_cache_stats();
        let shaped1 = lumen_static_shell::text::text_counters();
        println!(
            "glyph cache [{shaping:?}]: warm {warm:?}; after 20 edits {after:?} (bound: uncached 0, resets 0, \
             misses after warm-up only for digits not yet drawn); {} runs painted, {} through the shaper\n",
            shaped1.shaped - shaped0.shaped,
            shaped1.complex - shaped0.complex
        );
        assert_eq!(after.uncached, 0, "the glyph cache drew {} glyphs uncached on the grid", after.uncached);
        assert_eq!(after.resets, 0, "the glyph cache reset {} times on one screen", after.resets);
    }

    // ---- the byte-identity gate --------------------------------------
    //
    // The two arms must produce the *same frame*. Without this the probe
    // could report a saving that is really a frame which did less.
    // **A value whose formatted width differs from the one on screen.**
    // The first version compared at 7 against an initial "= 0" — the same
    // character count, so a stale box is the same size and the frames
    // agree however wrong the mark was. The injection that marks the
    // wrong subtree passed against it, which is how that was found.
    let mut ha = host(m);
    let mut hb = host(m);
    arm_a(&mut ha, 1_234_567, m);
    arm_b(&mut hb, 1_234_567, m);
    let (fa, fb) = (ha.list.hash_from(0), hb.list.hash_from(0));
    println!("frame hash: mark_all {fa:#x}, narrowed {fb:#x}");
    assert_eq!(fa, fb, "the two arms do not draw the same frame; the probe would be meaningless");
    assert_eq!(ha.list.len(), hb.list.len(), "command counts differ");

    // ---- arm C's setup, asserted before it is measured ----------------
    //
    // AGENT.md's first vacuous shape is a click that never lands: the
    // frame is identical on both sides because nothing happened. So find
    // the button by hit test rather than by arithmetic on a layout, and
    // assert that the handler ran and that the host saw the event as
    // handled.
    let mut hc = host(m);
    let bump_at = {
        let s = hc.states[lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "bump")
            .expect("#bump must be on the table")];
        (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
    };
    let hit = hc.probe(bump_at.0, bump_at.1).expect("nothing is under #bump's centre");
    let before = hc.app.cell;
    arm_c(&mut hc, bump_at, m);
    assert_eq!(hc.app.cell, before + 1, "the click did not reach on_bump — arm C measures nothing");
    assert_eq!(
        hc.app.ui.top.formula.as_str(),
        format!("= {}", before + 1),
        "the handler ran but the binding did not reach the label"
    );
    println!(
        "arm C: #bump at ({:.0},{:.0}), hit node {}, cell {before} -> {}",
        bump_at.0, bump_at.1, hit.node, hc.app.cell
    );

    // ---- M33/T33.4: the split twin draws the same frame ---------------
    //
    // A fresh host of each, one click on #bump each: the split one's
    // handler writes `cell.value` and the binding renders it, so the
    // frames must agree byte for byte. Printed side by side.
    {
        let mut u = host(m);
        let mut t = host_split(m);
        arm_c(&mut u, bump_at, m);
        t.click(bump_at.0, bump_at.1);
        t.pump_with(m);
        assert_eq!(t.app.cell.value, 1, "the click did not reach the split twin's handler");
        let (hu, ht) = (u.list.hash_from(0), t.list.hash_from(0));
        println!("split twin: unsplit {hu:#x}, split {ht:#x}, formula {:?}", t.app.ui.top.formula.as_str());
        assert_eq!(hu, ht, "the split twin does not draw the unsplit grid's frame");
    }

    // ---- T29.1: the probe --------------------------------------------
    const RUNS: usize = 8;
    const ITERS: usize = 40;
    println!(
        "\n{:<13} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7} {:>8} {:>8} {:>8} {:>7} {:>8} {:>9}",
        "arm", "input", "cascade", "layout", "place", "paint", "finish", "raster", "stage3", "pump", "frame",
        "gap", "outer", "damage px"
    );
    // Every run whose pump the stage sum does not account for, by more
    // than `GAP_BOUND` of the pump (M53/T53.0).
    let mut over: Vec<String> = Vec::new();

    for arm in ["mark_all", "bound", "narrowed", "handled"] {
        for run in 0..RUNS {
            let mut h = host(m);
            let r = measure(&mut h, ITERS, m, |h, m| {
                // `v` from a counter the closure owns: each frame must
                // write a value the label does not already hold.
                let v = (run * ITERS) as i64 + h.frame_number() as i64 + 1;
                match arm {
                    "mark_all" => arm_forced(h, v, m),
                    "bound" => arm_a(h, v, m),
                    "narrowed" => arm_b(h, v, m),
                    _ => arm_c(h, bump_at, m),
                }
            });
            r.print(arm);
            gap_check(&mut over, arm, run, &r);
        }
        println!();
    }

    // **M29/T29.6: the same click, on the split twin.** Its handler can
    // reach only `cell`, so the host marks the node the event consumed
    // and the binding marks the formula bar — and the frame should land
    // on the `narrowed` row, not the `mark_all` one.
    for run in 0..RUNS {
        let mut h = host_split(m);
        let r = measure(&mut h, ITERS, m, |h, m| {
            h.click(bump_at.0, bump_at.1);
            pump(h, m);
        });
        r.print("handled/split");
        gap_check(&mut over, "handled/split", run, &r);
    }
    println!();

    println!(
        "   `layout` is the only stage a narrower mark can help. `place` takes no\n   \
         `Changed` and walks every node, so it is a floor neither arm can move;\n   \
         `paint` walks the whole table either way (T16.8 refused a paint skip);\n   \
         `raster` is damage-bounded, so every arm rasterises the same few pixels.\n\n   \
         `handled` is the frame a real click produces. It should land on the\n   \
         `mark_all` row however the binding is narrowed, because a consumed event\n   \
         calls mark_all() before App::update is asked anything — which is what\n   \
         decides how much of this milestone is reachable at all.\n\n   \
         damage px is the value's digit count, not run-to-run variation: a wider\n   \
         `#formula` shifts `#status` beside it, so a frame that adds a digit\n   \
         damages the toolbar row. The first edit after a fresh host damages the\n   \
         whole surface; the per-run median excludes it."
    );

    // ---- M53/T53.0/T53.1: the stage sum against the pump --------------
    println!(
        "\n   Every column but `raster`, `outer` and `damage px` is the host's own\n   \
         `stage_timings` (M53/T53.1). `pump` is all of `pump_with`; `frame` is\n   \
         pump + raster, the figure to cite; `stage3` is layout + place + paint +\n   \
         raster, which this table published as the frame until T53.1. `gap` is\n   \
         pump minus the six stages, bounded at {:.0}% of the pump per run; `outer`\n   \
         is the same pump timed from outside the call, as a check on the host's.",
        GAP_BOUND * 100.0
    );
    if !over.is_empty() {
        println!("\n!! {} of the runs above: the stage sum does not account for the pump", over.len());
        for o in &over {
            println!("   {o}");
        }
        std::process::exit(1);
    }
}

/// One run's stage sum against its pump, and a failure if the gap is
/// over [`GAP_BOUND`] of the pump — the quantity beside its bound.
fn gap_check(over: &mut Vec<String>, arm: &str, run: usize, r: &Run) {
    let bound = GAP_BOUND * r.pump;
    let line = format!(
        "{arm} run {run}: stage-sum {:.2} vs pump {:.2}, gap {:.2} µs (bound {bound:.2})",
        r.pump - r.gap,
        r.pump,
        r.gap
    );
    if r.gap.abs() > bound {
        over.push(line);
    }
}
