//! M49/T49.0 (R5-F10): M32's front door, hot-reloadable.
//!
//! A `#[tree]` application booted by a free function passed to
//! `application`, with **no** `Application` impl, derives `Reloadable`
//! and exports its program. Before T49.4 the derive's `fresh()` named
//! `Application::build`, so this failed with E0277: a window reload boots
//! the library's `program()` and never calls `build`, but every hot
//! application still had to write one — the second construction path
//! M31 removed, back in a new shape.

use fulgurite::event::EventCtx;
use fulgurite::{App, Reloadable, StaticTree};
use fulgurite_shell::program::{application, Program};
use fulgurite_widgets::ops::{Button, Label};

#[derive(StaticTree, Default, Reloadable)]
#[widget(column, spacing = 0.0)]
pub struct Ui {
    pub inc: Button<Tally>,
    pub value: Label<String>,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct Tally {
    pub count: i64,
}

#[derive(App, Reloadable)]
pub struct Counter {
    #[tree]
    pub ui: Ui,
    #[state]
    #[reload(keep)]
    #[bind(ui.value, count, "count {}")]
    pub tally: Tally,
}

fn on_inc(t: &mut Tally, _: &EventCtx) {
    t.count += 1;
}

fn boot() -> Counter {
    let mut c = Counter { ui: Ui::default(), tally: Tally::default() };
    c.ui.inc.on_click = on_inc;
    c
}

pub fn program() -> Program<Counter> {
    application(boot)
}

fulgurite_reload::export_program!(program);

fn main() {
    let _ = program();
}
