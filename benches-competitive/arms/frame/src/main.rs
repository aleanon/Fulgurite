//! Frame cost: an N-row list where **one row's text changes** each frame.
//!
//! The same workload, iteration count and reporting shape as
//! `/mnt/Utvikling-linux/Lumen/benches-competitive/harnesses/qt/frame.cpp` and
//! `.../gtk/frame.c`: N rows in a flat column at 400 px wide, row 0's text
//! replaced each iteration, **min of 200 iterations after 20 warm-ups**, and
//! the stages reported separately rather than as one number.
//!
//! # Why the stages are reported apart
//!
//! Quoting one number would flatter somebody. Qt's harness says this too, and
//! for the same reason: it reports `layout` and `+paint` separately because
//! quoting only the paint number flatters the Rust arms (which stop at a
//! display list) and quoting only layout flatters Qt.
//!
//! This arm reports **seven** rows (eight under `observe`), because
//! lumen-static has stages the others do not. Four of them are the
//! one-row-change ladder below; the other three are an idle floor and the
//! all-rows pair, which is the workload Lumen's criterion benches use
//! against masonry and iced. The doc said "four" until T16.3 counted them.
//!
//! - `layout`          — the layout pass. Comparable to Qt's `layout` and
//!                       GTK's `measure`.
//! - `+paint`          — plus building the display list. This is where the
//!                       Rust arms in the Lumen comparison stop.
//! - `+render (full)`  — plus rasterising the whole 400x800 viewport. This is
//!                       the apples-to-apples with Qt's `+paint`.
//! - `+render (damage)`— plus rasterising **only what changed**. This is what
//!                       lumen-static actually does per frame (M8b/T13.8), and
//!                       it is reported *beside* the full number rather than
//!                       instead of it, because a comparison that quoted only
//!                       this one would be measuring an optimisation the other
//!                       arms were not allowed to use.
//!
//! No arm here virtualizes. `List` is the non-virtualized container — every
//! element is materialized — which is what `GtkBox`, `QVBoxLayout` and iced's
//! `column` do in the arms this is being compared against. `Repeat` would
//! virtualize and would not be comparable.

use std::time::Instant;

use lumen_static::hatch::List;
use lumen_static_widgets::ops::Label;
use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;
use lumen_static::{pass, Constraints, DisplayList, Incremental, LayoutCx, Recording, Retention};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::{FontMetrics, FontStack};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    rows: List<Label<String>>,
}

const W: u32 = 400;
const H: u32 = 800;

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(3000);
    let iters: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(200);

    let font = FontStack::system().expect("a system font: every arm shapes text for real");
    let mut metrics = FontMetrics::new(font.clone());

    let mut ui = Ui::default();
    ui.rows.items = (0..n).map(|i| Label::new(format!("row {i}"))).collect();

    // **The column is pinned to the viewport width, and this is load-bearing
    // for the damage number** (T16.2).
    //
    // `List::paint` clips its items to the list node's rect, and that node's
    // width is the *widest row*. A row whose text got one pixel wider
    // therefore rewrote the clip, and `Diffing::step` unions old ink with
    // new -- so a one-row change damaged the entire column. At 100 rows this
    // arm alternated between 1,562 and 57,600 damaged pixels depending on
    // which digits row 0 happened to contain, and because the arm reports a
    // *minimum*, it preferentially selected the frames where the width did
    // not change. The published `+render (damage)` figure described the
    // lucky frames.
    //
    // A pinned width is also the more representative case: a list in a
    // window is the window's width, not its content's.
    let mut styles = pass::default_styles(&ui);
    lumen_static::style::cascade_deep(
        &mut ui,
        &[
            // The **list node**, not its parent: `List::paint` clips to
            // `rect_of(&st[i])` on its own state, so pinning the column
            // above it leaves the content-dependent width in place. Pinning
            // `Ui` first was exactly that mistake, and `DIAG=1` -- newly
            // able to tell the truth -- reported a full-height damage strip
            // until this line named the right node.
            Rule::new(Selector::Kind("list"), Patch::new().width(Length::Fixed(W as f32))),
            Rule::new(Selector::Kind("Ui"), Patch::new().width(Length::Fixed(W as f32))),
        ],
        &mut styles,
    );
    let mut st = vec![lumen_static::NodeState::default(); Ui::NODE_COUNT];
    // Natural height, as Qt's harness is careful to give its rows: squashing
    // 3000 rows into 800 px is a different and much cheaper layout problem.
    let view = Constraints { min_w: 0.0, max_w: W as f32, min_h: 0.0, max_h: f32::INFINITY };

    let mut list = DisplayList::new();
    let mut canvas = Canvas::with_font(W, H, font);
    let mut retain = Incremental::default();

    let layout = |ui: &mut Ui, st: &mut Vec<lumen_static::NodeState>, m: &mut FontMetrics| {
        let mut cx = LayoutCx::new(m);
        pass::layout_with(ui, &mut cx, &styles, st, view);
    };

    // Warm-up: caches, the allocator's high-water mark, and the branch
    // predictor. 20, as the Qt and GTK harnesses use.
    for k in 0..20 {
        ui.rows.items[0].set(format!("warm {k}"));
        layout(&mut ui, &mut st, &mut metrics);
        list.clear();
        {
            let mut rec = Recording::new(&mut list, &mut retain);
            pass::paint(&ui, &st, &mut rec);
        }
        retain.finish();
    }

    // The idle floor: the same loop with **no text change**, so the
    // comparison is symmetric. Qt's harness reports one for exactly this
    // reason — a framework that spends 251 us on a frame where nothing
    // happened is telling you something a "cost of the change" number
    // alone would hide, and so is one that spends nothing.
    let mut floor = f64::INFINITY;
    for _ in 0..iters {
        let t = Instant::now();
        layout(&mut ui, &mut st, &mut metrics);
        list.clear();
        {
            let mut rec = Recording::new(&mut list, &mut retain);
            pass::paint(&ui, &st, &mut rec);
        }
        let d = retain.finish();
        if let Some(area) = d.clip_to([0.0, 0.0, W as f32, H as f32]) {
            canvas.clear_within(area, lumen_static_shell::CLEAR);
            canvas.render_within(&list, area);
        }
        floor = floor.min(t.elapsed().as_secs_f64() * 1e6);
    }
    println!("lumen-static/{:<22} {floor:9.1} us   ({n} rows)", "idle (floor)");

    // The **all-rows-change** workload, which is what Lumen's criterion
    // benches (`vs_toolkits`, `vs_iced`) measure against masonry and iced.
    // Every row's text changes, so no cache can serve any of it and no
    // damage rect can be smaller than the frame — the measurement that
    // stops a shaping cache flattering one side.
    let mut best_all = [f64::INFINITY; 2];
    for k in 0..iters {
        for (i, row) in ui.rows.items.iter_mut().enumerate() {
            row.set(format!("row {i} · {k}"));
        }
        let t = Instant::now();
        layout(&mut ui, &mut st, &mut metrics);
        best_all[0] = best_all[0].min(t.elapsed().as_secs_f64() * 1e6);

        for (i, row) in ui.rows.items.iter_mut().enumerate() {
            row.set(format!("row {i} · {k}."));
        }
        let t = Instant::now();
        layout(&mut ui, &mut st, &mut metrics);
        list.clear();
        pass::paint(&ui, &st, &mut list);
        best_all[1] = best_all[1].min(t.elapsed().as_secs_f64() * 1e6);
    }
    println!("lumen-static/{:<22} {:9.1} us   ({n} rows)", "ALL rows: layout", best_all[0]);
    println!("lumen-static/{:<22} {:9.1} us   ({n} rows)", "ALL rows: +paint", best_all[1]);

    let mut best = [f64::INFINITY; 4];
    for k in 0..iters {
        // Every sub-step's string is the **same length and the same glyph
        // widths** -- five zero-padded digits and a step marker. The four
        // steps used to append `.`, `..` and `...`, so each measured a
        // differently-wide row and moved the column's intrinsic width
        // between sub-steps. Digits are tabular in both candidate faces, so
        // this run's width is constant frame to frame (T16.2).
        let text = |step: u8| format!("counter: {:05} #{step}", k % 10000);

        // 1. layout only
        ui.rows.items[0].set(text(0));
        let t = Instant::now();
        layout(&mut ui, &mut st, &mut metrics);
        best[0] = best[0].min(t.elapsed().as_secs_f64() * 1e6);

        // 2. layout + paint
        ui.rows.items[0].set(text(1));
        let t = Instant::now();
        layout(&mut ui, &mut st, &mut metrics);
        list.clear();
        pass::paint(&ui, &st, &mut list);
        best[1] = best[1].min(t.elapsed().as_secs_f64() * 1e6);

        // 3. + rasterise the whole viewport
        ui.rows.items[0].set(text(2));
        let t = Instant::now();
        layout(&mut ui, &mut st, &mut metrics);
        list.clear();
        pass::paint(&ui, &st, &mut list);
        canvas.clear(lumen_static_shell::CLEAR);
        canvas.render_within(&list, [0.0, 0.0, W as f32, H as f32]);
        best[2] = best[2].min(t.elapsed().as_secs_f64() * 1e6);

        // 4. + rasterise only the damage
        ui.rows.items[0].set(text(3));
        let t = Instant::now();
        layout(&mut ui, &mut st, &mut metrics);
        list.clear();
        {
            // Retention observes the stream as paint goes past (T13.8), so
            // this is the cost a real frame pays, not an extra pass.
            let mut rec = Recording::new(&mut list, &mut retain);
            pass::paint(&ui, &st, &mut rec);
        }
        let d = retain.finish();
        if let Some(area) = d.clip_to([0.0, 0.0, W as f32, H as f32]) {
            canvas.clear_within(area, lumen_static_shell::CLEAR);
            canvas.render_within(&list, area);
        }
        best[3] = best[3].min(t.elapsed().as_secs_f64() * 1e6);
    }

    // The dev surface, when it is compiled in. `observe` runs the agent's
    // journal diff *and* the ambient audit over the whole tree on every
    // painted frame — the right default for `cargo run` and the wrong thing
    // to put in a row next to Qt, which runs no such pass. Measured, not
    // assumed, and reported as its own row (T14/BENCH note).
    #[cfg(feature = "observe")]
    {
        let mut agent = lumen_static_agent::Agent::new();
        let mut best_obs = f64::INFINITY;
        for k in 0..iters {
            ui.rows.items[0].set(format!("obs {k}"));
            let t = Instant::now();
            layout(&mut ui, &mut st, &mut metrics);
            list.clear();
            pass::paint(&ui, &st, &mut list);
            {
                let mut tree = lumen_static_agent::TreeMut::of_root(&mut ui, &styles, &st);
                agent.frame_done(&mut tree, lumen_static_agent::FrameTiming::passes(0, 0));
            }
            best_obs = best_obs.min(t.elapsed().as_secs_f64() * 1e6);
        }
        println!("lumen-static/{:<22} {best_obs:9.1} us   ({n} rows)", "+paint +observe");
    }

    assert!(!list.cmds.is_empty(), "the frame drew nothing, so it measured nothing");
    if std::env::var_os("DIAG").is_some() {
        // **Paint a real frame first.** This used to call `retain.finish()`
        // straight after the loops, with no intervening `observe` -- so
        // `Diffing::finish` saw zero observed commands against a non-empty
        // previous frame, took that for a structural change and returned
        // `Damage::All` unconditionally. The diagnostic printed `All`
        // whatever the frame actually damaged, which is to say it had never
        // once been true (T16.2).
        //
        // Two frames, because the first re-establishes the baseline the
        // loops above left and only the second reports a one-row change.
        let mut area = None;
        for step in 0..2u8 {
            ui.rows.items[0].set(format!("diag: {:05} #{step}", 4242));
            layout(&mut ui, &mut st, &mut metrics);
            list.clear();
            let mut rec = Recording::new(&mut list, &mut retain);
            pass::paint(&ui, &st, &mut rec);
            drop(rec);
            area = retain.finish().clip_to([0.0, 0.0, W as f32, H as f32]);
        }
        let px = area.map_or(0, |a| ((a[2] - a[0]) * (a[3] - a[1])).max(0.0) as u64);
        eprintln!("diag: {} cmds, damage {:?}", list.cmds.len(), area);
        eprintln!("diag: damaged {px} px of {}", W as u64 * H as u64);
        eprintln!("diag: tree height {}", st[0].size.h);

        // **Is the damage the same every frame?** The reason this check
        // exists: the arm reports a *minimum* over 200 iterations, so a
        // damage figure that varies frame to frame is reported at its
        // smallest and the published number describes the luckiest frame
        // rather than the typical one. Before T16.2 this swung between
        // 1,562 and 57,600 px at N=100. A spread here is a broken
        // measurement, not a curiosity.
        // A row's height, derived from the column rather than read out of
        // `st`: `List`'s items live in its arena, not in the node table, so
        // `st` holds only `Ui` and the list itself. The first version of
        // this line used `st[1].size.h` -- the list's *total* height, 2200
        // px at N=100 -- which made the bound below 4400 px and the
        // assertion vacuous. It passed, which is how it was noticed.
        let row_h = (st[0].size.h / n as f32).max(1.0);
        let (mut lo, mut hi, mut tallest) = (u64::MAX, 0u64, 0.0f32);
        for k in 0..200u32 {
            ui.rows.items[0].set(format!("counter: {:05} #{}", k % 10000, k % 4));
            layout(&mut ui, &mut st, &mut metrics);
            list.clear();
            let mut rec = Recording::new(&mut list, &mut retain);
            pass::paint(&ui, &st, &mut rec);
            drop(rec);
            let a = retain.finish().clip_to([0.0, 0.0, W as f32, H as f32]);
            let p = a.map_or(0, |a| ((a[2] - a[0]) * (a[3] - a[1])).max(0.0) as u64);
            tallest = tallest.max(a.map_or(0.0, |a| a[3] - a[1]));
            lo = lo.min(p);
            hi = hi.max(p);
        }
        eprintln!(
            "diag: damage over 200 frames: {lo}..{hi} px, tallest {tallest:.0} px              ({:.1} rows of {row_h:.0} px)",
            tallest / row_h
        );

        // **The property is the height, not the area.** The area still moves
        // a little because row 0's own width moves: the face's digits are
        // not perfectly tabular, so `counter: 00042` and `counter: 00199`
        // differ by about a pixel. That is *correct* damage -- the row
        // really did change width -- and asserting it away would be
        // asserting that text has fixed width.
        //
        // What must not happen is the damage escaping the changed row and
        // taking the column with it, which is what a content-dependent
        // container clip did: 1,562 px on a lucky frame and 57,600 on an
        // unlucky one, 36 rows tall. Two rows is the bound; the first
        // version of this check asserted exact equality, failed on the
        // one-pixel wobble, and was testing the wrong thing.
        assert!(
            tallest <= row_h * 2.0,
            "damage is {tallest:.0} px tall, {:.1} rows: it has escaped the row that changed",
            tallest / row_h
        );
    }
    for (what, us) in
        ["layout", "+paint", "+render (full)", "+render (damage)"].iter().zip(best)
    {
        println!("lumen-static/{what:<22} {us:9.1} us   ({n} rows)");
    }

    // **Culled paint, reported apart and never mixed in** (T16.12).
    //
    // Every row above materializes and paints all N rows, because that
    // is what `GtkBox`, `QVBoxLayout` and iced's `column` do and it is
    // what makes the comparison in `docs/results-competitive.md` fair.
    // A `DisplayList` told where the frame is going culls the rows that
    // cannot be seen — which is what GTK and Qt do in *their* paint
    // paths, and what this framework did not.
    //
    // It is a separate row and not a replacement, because quoting it
    // against an arm that does not cull would be comparing forty rows
    // of work with ten thousand.
    let mut culled = f64::INFINITY;
    let mut culled_cmds = 0usize;
    for k in 0..iters {
        ui.rows.items[0].set(format!("counter: {:05} #c", k % 10000));
        let t = Instant::now();
        layout(&mut ui, &mut st, &mut metrics);
        list.clear();
        list.set_bounds(Some([0.0, 0.0, W as f32, H as f32]));
        pass::paint(&ui, &st, &mut list);
        culled = culled.min(t.elapsed().as_secs_f64() * 1e6);
        culled_cmds = list.cmds.len();
    }
    list.set_bounds(None);
    println!(
        "lumen-static/{:<22} {culled:9.1} us   ({n} rows, {culled_cmds} of {} cmds)",
        "+paint (culled)",
        n + 2
    );
}
