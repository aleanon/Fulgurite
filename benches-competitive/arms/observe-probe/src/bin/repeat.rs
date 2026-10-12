//! A `Repeat<Label<String>>` window of rows — the container
//! `Repeat::observe` walks.
use fulgurite::prelude::*;
use fulgurite::repeat::Repeat;
use fulgurite_shell::Host;
use fulgurite_widgets::ops::Label;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    rows: Repeat<Label<String>>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

fn bind(l: &mut Label<String>, i: usize) {
    use core::fmt::Write;
    let mut s = String::new();
    let _ = write!(s, "row {i}");
    l.set(s);
}

fn main() {
    let mut app = App::default();
    app.ui.rows = Repeat::new(10_000, 18.0, bind);
    let mut h: Host<App> = Host::new(app, 400, 800);
    for _ in 0..3 {
        h.pump();
    }
    println!("{}", h.list.len());
}
