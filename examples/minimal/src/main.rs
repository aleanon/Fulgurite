//! The smallest window this framework can open — the T14.0 gate.
//!
//! Everything below `main` is the application. The window, the event
//! loop, the raster, damage, buffer age, the scale factor and the font
//! are `lumen-static-winit`. Before M14 this file would have been ~480
//! lines, because there was nowhere else for them to live.
//!
//! It is also the size measurement for T15.12: built plain it renders
//! on the software canvas, built `--features gpu` it renders on wgpu,
//! and **not one line here changes** (`docs/results-gpu.md`). Which
//! backend a run actually got is printed at startup, and
//! `LUMEN_BACKEND=software` forces the fallback.

use lumen_static::prelude::*;
use lumen_static_winit::{run, Config};

#[derive(DeriveApp, Default)]
struct Counter {
    #[tree]
    ui: Ui,
    #[bind(ui.value, "clicked {} times")]
    count: i64,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 12.0)]
struct Ui {
    value: Label<String>,
    button: Button<Counter>,
}

fn main() {
    let mut app = Counter::default();
    app.ui.button.label = "click me";
    app.ui.button.on_click = |c: &mut Counter, _: &EventCtx| c.count += 1;
    run(app, Config { title: "minimal", width: 320, height: 160, ..Config::default() });
}
