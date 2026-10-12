//! The size + idle-RSS arm: a window with a label and a button, shown, then
//! idle.
//!
//! Byte-for-byte the same content as the Qt, GTK, iced, Xilem, Slint and Lumen
//! apps in `/mnt/Utvikling-linux/Lumen/benches-competitive/harnesses`, because
//! a size comparison between differently-shaped applications measures the
//! shapes.

use fulgurite::prelude::*;
use fulgurite_widgets::prelude::*;
use fulgurite_winit::{run, Config};

#[derive(DeriveApp, Default)]
struct Counter {
    #[tree]
    ui: Ui,
    #[bind(ui.value, "{}")]
    count: i64,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct Ui {
    value: Label<String>,
    button: Button<Counter>,
}

fn main() {
    let mut app = Counter::default();
    app.ui.button.label = "Increment";
    app.ui.button.on_click = |c: &mut Counter, _: &EventCtx| c.count += 1;
    run(app, Config { title: "fulgurite-app", width: 400, height: 300, ..Config::default() });
}
