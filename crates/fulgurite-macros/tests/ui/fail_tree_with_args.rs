//! M55/T55.2 (R16-F4): `#[tree]` takes no arguments. It was detected by
//! its path alone, so `#[tree(junk = 5)]` compiled.
use fulgurite::{App, StaticTree};
use fulgurite_widgets::ops::Label;

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    a: Label,
}

#[derive(App, Default)]
struct Counter {
    #[tree(junk = 5)]
    ui: Ui,
}

fn main() {}
