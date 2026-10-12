//! M33 §2.2: `#[state]` on the tree would hand the tree to every handler
//! and make `STATE_IS_SPLIT` a lie.
use fulgurite::event::EventCtx;
use fulgurite::{App, StaticTree};
use fulgurite_widgets::ops::{Button, Label};

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
    #[state]
    ui: Ui,
    count: i64,
}

fn main() {}
