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
//! Run: `cargo run --release -p benches --bin grid`

use std::hint::black_box;
use std::time::Instant;

use lumen_static::ops::Label;
use lumen_static::repeat::Repeat;
use lumen_static::{App as DeriveApp, StaticTree, Widget};
use lumen_static::display::Color;
use lumen_static_shell::software::Canvas;
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

fn bind_body(row: &mut BodyRow, i: usize) {
    row.a.set(format!("r{i}"));
    row.b.set(format!("{:.2}", i as f32 * 1.37));
    row.c.set(if i.is_multiple_of(3) { "pending".into() } else { "ok".into() });
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
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 2.0)]
pub struct Ui {
    top: TopBar,
    sheet: Sheet,
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
const PAGE: Color = Color(255, 255, 255, 255);

fn app() -> Grid {
    let mut g = Grid::default();
    g.ui.top.status.set("Sheet1".into());
    g.ui.body = Repeat::new(ROWS, 18.0, bind_body);
    // Varied cell text: shaping is part of a real frame, and identical
    // strings would let the memo hide it.
    let mut n = 0usize;
    for b in [
        &mut g.ui.sheet.b0, &mut g.ui.sheet.b1, &mut g.ui.sheet.b2, &mut g.ui.sheet.b3,
        &mut g.ui.sheet.b4, &mut g.ui.sheet.b5, &mut g.ui.sheet.b6, &mut g.ui.sheet.b7,
        &mut g.ui.sheet.b8, &mut g.ui.sheet.b9, &mut g.ui.sheet.b10, &mut g.ui.sheet.b11,
        &mut g.ui.sheet.b12, &mut g.ui.sheet.b13, &mut g.ui.sheet.b14, &mut g.ui.sheet.b15,
        &mut g.ui.sheet.b16, &mut g.ui.sheet.b17, &mut g.ui.sheet.b18, &mut g.ui.sheet.b19,
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
    g
}

type Bounded = Host<Grid, lumen_static::Incremental, lumen_static::Boundaries<16>>;

fn host() -> Bounded {
    let mut h: Bounded = Host::new(app(), W, H);
    h.pump();
    h.pump();
    h
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
fn arm_a(h: &mut Bounded, v: i64) {
    h.app.cell = v;
    h.pump();
}

fn arm_b(h: &mut Bounded, v: i64) {
    h.app.cell = v;
    h.app.ui.top.formula.set(format!("= {v}"));
    assert!(h.invalidate("formula"), "setup: #formula is on the table");
    h.pump();
}

fn main() {
    black_box(());
    let table = <Ui as Widget>::NODE_COUNT;

    // ---- T29.0: the fixture asserts its own shape ---------------------
    //
    // An arm that cannot exhibit the defect is a finding about the arm,
    // so these run before any number is taken from it.
    assert!(table >= 2000, "the frozen grid is {table} nodes, too few to show a whole-tree walk");
    let formula_at = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "formula")
        .expect("#formula must be a TABLE node — in an arena it would never go through #[bind]");
    let mut h = host();
    assert_eq!(h.app.ui.top.formula.as_str(), "= 0", "setup: the binding wrote the formula bar");

    // A single edit changes exactly one bound value.
    let status_before = h.app.ui.top.status.as_str().to_owned();
    h.app.cell = 41;
    h.pump();
    assert_eq!(h.app.ui.top.formula.as_str(), "= 41", "the edit reached the bound label");
    assert_eq!(h.app.ui.top.status.as_str(), status_before, "the edit touched a second label");

    println!(
        "grid: {table} table nodes (frozen grid), {ROWS} virtualized rows, \
         #formula at index {formula_at}\n"
    );

    // ---- the byte-identity gate --------------------------------------
    //
    // The two arms must produce the *same frame*. Without this the probe
    // could report a saving that is really a frame which did less.
    // **A value whose formatted width differs from the one on screen.**
    // The first version compared at 7 against an initial "= 0" — the same
    // character count, so a stale box is the same size and the frames
    // agree however wrong the mark was. The injection that marks the
    // wrong subtree passed against it, which is how that was found.
    let mut ha = host();
    let mut hb = host();
    arm_a(&mut ha, 1_234_567);
    arm_b(&mut hb, 1_234_567);
    let (fa, fb) = (ha.list.hash_from(0), hb.list.hash_from(0));
    println!("frame hash: mark_all {fa:#x}, narrowed {fb:#x}");
    assert_eq!(fa, fb, "the two arms do not draw the same frame; the probe would be meaningless");
    assert_eq!(ha.list.len(), hb.list.len(), "command counts differ");

    // ---- T29.1: the probe --------------------------------------------
    const RUNS: usize = 8;
    const ITERS: usize = 40;
    println!(
        "\n{:<10} {:>12} {:>12} {:>12} {:>12} {:>12}",
        "arm", "layout µs", "paint µs", "raster µs", "frame µs", "damage px"
    );

    for arm in ["mark_all", "narrowed"] {
        for run in 0..RUNS {
            let mut h = host();
            let mut canvas = Canvas::new(W, H);
            let (mut lay, mut pnt, mut ras) = (Vec::new(), Vec::new(), Vec::new());
            let mut px: Vec<f64> = Vec::new();
            for i in 0..ITERS {
                let v = (run * ITERS + i) as i64 + 1;
                match arm {
                    "mark_all" => arm_a(&mut h, v),
                    _ => arm_b(&mut h, v),
                }
                let (l, p) = h.timings();
                lay.push(l as f64 / 1000.0);
                pnt.push(p as f64 / 1000.0);
                // **Damage-bounded, as a host rasterises**: clear and
                // render only what retention says changed. `Host::render`
                // draws the whole list, which no shell does and which
                // would inflate the frame this probe is judged against.
                let dmg = h.take_damage_for_age(1).clip_to([0.0, 0.0, W as f32, H as f32]);
                let t = Instant::now();
                match dmg {
                    Some(area) => {
                        canvas.clear_within(area, PAGE);
                        canvas.render_within(&h.list, area);
                        px.push(((area[2] - area[0]) * (area[3] - area[1])) as f64);
                    }
                    None => px.push(0.0),
                }
                ras.push(t.elapsed().as_nanos() as f64 / 1000.0);
            }
            let (l, p, r) = (med(&mut lay), med(&mut pnt), med(&mut ras));
            println!(
                "{arm:<10} {l:>12.2} {p:>12.2} {r:>12.2} {:>12.2} {:>12.0}",
                l + p + r,
                med(&mut px)
            );
        }
        println!();
    }

    println!(
        "   layout is the only stage a narrower mark can help: paint walks the whole\n   \
         table either way (T16.8 refused a paint skip), and the raster column is\n   \
         damage-bounded, so both arms rasterise the same few pixels."
    );
}
