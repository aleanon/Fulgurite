//! M33/T33.0: a split application compiles, and its handler writes the
//! state. The other direction of `fail_handler_reaches_tree.rs`, which
//! is this file with one line changed.
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
    t.count += 1;
}

/// The other direction: no `#[state]`, so the handler gets the whole
/// application and the constant must say so.
#[derive(App)]
struct Unsplit {
    #[tree]
    ui: Ui2,
    count: i64,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui2 {
    title: Label,
}

fn main() {
    let _ = on_inc;
    const _: () = assert!(<Counter as lumen_static::event::App>::STATE_IS_SPLIT);
    const _: () = assert!(!<Unsplit as lumen_static::event::App>::STATE_IS_SPLIT);
}
