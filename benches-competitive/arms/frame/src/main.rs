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
//! This arm reports four, because lumen-static has a stage the others do not:
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
use lumen_static::ops::Label;
use lumen_static::prelude::*;
use lumen_static::{
    pass, Constraints, DisplayList, Incremental, LayoutCx, Recording, Retention,
};
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
    let styles = pass::default_styles(&ui);
    let mut st = vec![lumen_static::NodeState::default(); Ui::NODE_COUNT];
    // Natural height, as Qt's harness is careful to give its rows: squashing
    // 3000 rows into 800 px is a different and much cheaper layout problem.
    let view = Constraints { min_w: 0.0, max_w: W as f32, min_h: 0.0, max_h: f32::INFINITY };

    let mut list = DisplayList::new();
    let mut canvas = Canvas::with_font(W, H, font);
    let mut retain = Incremental::default();

    let mut layout = |ui: &mut Ui, st: &mut Vec<lumen_static::NodeState>, m: &mut FontMetrics| {
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

    let mut best = [f64::INFINITY; 4];
    for k in 0..iters {
        let text = format!("counter: {:05}", k % 10000);

        // 1. layout only
        ui.rows.items[0].set(text.clone());
        let t = Instant::now();
        layout(&mut ui, &mut st, &mut metrics);
        best[0] = best[0].min(t.elapsed().as_secs_f64() * 1e6);

        // 2. layout + paint
        ui.rows.items[0].set(format!("{text}."));
        let t = Instant::now();
        layout(&mut ui, &mut st, &mut metrics);
        list.clear();
        pass::paint(&ui, &st, &mut list);
        best[1] = best[1].min(t.elapsed().as_secs_f64() * 1e6);

        // 3. + rasterise the whole viewport
        ui.rows.items[0].set(format!("{text}.."));
        let t = Instant::now();
        layout(&mut ui, &mut st, &mut metrics);
        list.clear();
        pass::paint(&ui, &st, &mut list);
        canvas.clear(lumen_static_shell::CLEAR);
        canvas.render_within(&list, [0.0, 0.0, W as f32, H as f32]);
        best[2] = best[2].min(t.elapsed().as_secs_f64() * 1e6);

        // 4. + rasterise only the damage
        ui.rows.items[0].set(format!("{text}..."));
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

    assert!(!list.cmds.is_empty(), "the frame drew nothing, so it measured nothing");
    if std::env::var_os("DIAG").is_some() {
        let d = retain.finish();
        eprintln!("diag: {} cmds, damage {:?}", list.cmds.len(), d);
        eprintln!("diag: tree height {}", st[0].size.h);
    }
    for (what, us) in
        ["layout", "+paint", "+render (full)", "+render (damage)"].iter().zip(best)
    {
        println!("lumen-static/{what:<22} {us:9.1} us   ({n} rows)");
    }
}
