//! A `List<Label<String>>` of rows, laid out and painted — the frame arm's
//! shape, the container `List::observe` walks.
use lumen_static::hatch::List;
use lumen_static::prelude::*;
use lumen_static_shell::Host;
use lumen_static_widgets::ops::Label;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    rows: List<Label<String>>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

fn main() {
    let mut app = App::default();
    app.ui.rows.items = (0..200).map(|i| Label::new(format!("row {i}"))).collect();
    let mut h: Host<App> = Host::new(app, 400, 800);
    for _ in 0..3 {
        h.pump();
    }
    println!("{}", h.list.len());
}
