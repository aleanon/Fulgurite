//! The smallest window this framework can open — the T14.0 gate.
//!
//! Everything below `main` is the application. The window, the event
//! loop, the raster, damage, buffer age, the scale factor and the font
//! are `fulgurite-winit`. Before M14 this file would have been ~480
//! lines, because there was nowhere else for them to live.
//!
//! It is also the size measurement for T15.12: built plain it renders
//! on the software canvas, built `--features gpu` it renders on wgpu,
//! and **not one line here changes** (`docs/results-gpu.md`). Which
//! backend a run actually got is printed at startup, and
//! `FULGURITE_BACKEND=software` forces the fallback.

// ANCHOR: app
use fulgurite::prelude::*;
use fulgurite_widgets::prelude::*;
use fulgurite_winit::{application, Program, Run};

#[derive(DeriveApp)]
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

/// **The program, described once** — M32. How to build it and what its
/// window looks like, in one value that the window and the headless
/// `--shot` below both start from.
fn program() -> Program<Counter> {
    application(|| {
        let mut app = Counter { ui: Ui::default(), count: 0 };
        app.ui.button.label = "click me";
        app.ui.button.on_click = |c: &mut Counter, _: &EventCtx| c.count += 1;
        app
    })
    .title("minimal")
    .size(320, 160)
}

fn main() {
    program().run();
}
// ANCHOR_END: app
