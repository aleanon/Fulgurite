//! M33 §2.2: a second `#[state]` field is refused, not silently ignored.
use lumen_static::event::EventCtx;
use lumen_static::{App, StaticTree};
use lumen_static_widgets::ops::{Button, Label};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    title: Label,
    inc: Button<Tally>,
}

#[derive(Default)]
struct Tally {
    count: i64,
}

#[derive(App)]
struct Counter {
    #[tree]
    ui: Ui,
    #[state]
    state: Tally,
    #[state]
    other: Tally,
}

fn main() {}
