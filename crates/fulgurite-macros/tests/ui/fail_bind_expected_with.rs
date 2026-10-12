//! M55/T55.2 (R16-F4): `key = value` in a `#[bind]` must be `with`.
use fulgurite::{App, StaticTree};
use fulgurite_widgets::ops::Label;

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    a: Label<String>,
}

fn show(dst: &mut Label<String>, src: &i64) {
    fulgurite::bind::set_if_changed(dst, &src.to_string());
}

#[derive(App, Default)]
struct Counter {
    #[tree]
    ui: Ui,
    #[bind(ui.a, using = show)]
    n: i64,
}

fn main() {}
