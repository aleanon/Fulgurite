//! M55/T55.2 (R16-F4): an `#[app]` key given twice is refused. The
//! second used to win: `tick = t1, tick = t2` ran `t2`.
use fulgurite::{App, StaticTree};
use fulgurite_widgets::ops::Label;

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    a: Label,
}

fn t1(_: &mut Counter, _: f32) -> bool {
    false
}
fn t2(_: &mut Counter, _: f32) -> bool {
    true
}

#[derive(App, Default)]
#[app(tick = t1, tick = t2)]
struct Counter {
    #[tree]
    ui: Ui,
}

fn main() {}
