//! M84/T84.8: `msg`, `apply` and `jobs` come together -- a message type
//! nothing interprets, an `apply` with nothing to take, and jobs for no
//! message are each refused by name.
use lumen_static::{App, StaticTree};
use lumen_static_widgets::ops::Label;

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    a: Label,
}

fn take(_: &mut Counter, _: u32) -> bool {
    false
}

#[derive(App, Default)]
#[app(apply = take)]
struct Counter {
    #[tree]
    ui: Ui,
}

fn main() {}
