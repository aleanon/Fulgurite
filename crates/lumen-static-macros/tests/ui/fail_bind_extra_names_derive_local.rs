//! M55/T55.1 (R16-F3): a bare `changed` in a binding names nothing in
//! the user's scope, and is refused as such.
//!
//! Until T55.1 it named the derive's own `bool` local, compiled, and
//! printed "3 false" -- the review's repro. The refusal is rustc's, at
//! the user's token, which is the point: the user's tokens resolve in
//! the user's scope.
use lumen_static::{App, StaticTree};
use lumen_static_widgets::ops::Label;

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    a: Label<String>,
}

#[derive(App, Default)]
struct Counter {
    #[tree]
    ui: Ui,
    #[bind(ui.a, "{} {}", changed)]
    n: u32,
}

fn main() {}
