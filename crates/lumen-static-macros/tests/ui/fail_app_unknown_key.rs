//! M55/T55.2 (R16-F4): an unknown `#[app]` key names the two it accepts.
use lumen_static::{App, StaticTree};
use lumen_static_widgets::ops::Label;

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    a: Label,
}

fn hook(_: &mut Counter) -> bool {
    false
}

#[derive(App, Default)]
#[app(before_update = hook)]
struct Counter {
    #[tree]
    ui: Ui,
}

fn main() {}
