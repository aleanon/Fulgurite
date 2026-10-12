//! M55/T55.2: a source path followed by anything but `,` and a format
//! string is refused at the source.
use fulgurite::{App, StaticTree};
use fulgurite_widgets::ops::Label;

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    a: Label<String>,
}

#[derive(Default)]
struct Tally {
    count: i64,
}

#[derive(App, Default)]
struct Counter {
    #[tree]
    ui: Ui,
    #[bind(ui.a, count "count: {}")]
    state: Tally,
}

fn main() {}
