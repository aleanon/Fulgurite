//! M55/T55.2 (R16-F4): a second `#[tree]` field is refused. The sentence
//! had no `.stderr` until M55: the review disabled this check and the
//! zero-column one together and this suite stayed green.
use fulgurite::{App, StaticTree};
use fulgurite_widgets::ops::Label;

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    a: Label,
}

#[derive(App, Default)]
struct Counter {
    #[tree]
    ui: Ui,
    #[tree]
    other: Ui,
}

fn main() {}
