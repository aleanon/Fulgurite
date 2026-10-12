//! A `List<Label<String>>` of rows, laid out and painted — the frame arm's
//! shape, the container `List::observe` walks.
use fulgurite::hatch::List;
use fulgurite::prelude::*;
use fulgurite_shell::Host;
use fulgurite_widgets::ops::Label;

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
