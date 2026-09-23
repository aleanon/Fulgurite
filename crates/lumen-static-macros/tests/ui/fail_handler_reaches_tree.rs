//! M33/T33.0: with a `#[state]` field, a handler receives the state and
//! the tree is not reachable from it. `pass_state_split.rs` is this file
//! with the handler's body changed to write the state.
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
}

fn on_inc(t: &mut Tally, _: &EventCtx) {
    t.ui.title.set("reached the tree");
}

fn main() {
    let _ = on_inc;
}
