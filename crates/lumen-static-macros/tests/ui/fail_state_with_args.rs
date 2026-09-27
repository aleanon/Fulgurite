//! M55/T55.2 (R16-F4): `#[state]` takes no arguments. It was detected by
//! its path alone (T33.0), so `#[state(junk)]` compiled.
use lumen_static::{App, StaticTree};
use lumen_static_widgets::ops::Label;

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    a: Label,
}

#[derive(Default)]
struct Tally {
    n: i64,
}

#[derive(App, Default)]
struct Counter {
    #[tree]
    ui: Ui,
    #[state(junk)]
    state: Tally,
}

fn main() {}
