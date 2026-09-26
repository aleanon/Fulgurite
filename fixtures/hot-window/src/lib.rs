//! A counter that reloads itself inside its window — M37/T37.4
//! (`docs/plan-window-reload.md`).
//!
//! The drive edits the lines marked `// EDIT ...` in a copy of this
//! file: the bound label's format (`EDIT format`), and a line in the
//! click handler that becomes a type error or a panic (`EDIT handler`).

use lumen_static::event::EventCtx;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{App as DeriveApp, Reloadable as DeriveReloadable, StaticTree};
use lumen_static_shell::program::{application, Program};
use lumen_static_widgets::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(StaticTree, Default, DeriveReloadable)]
#[widget(column, spacing = 8.0)]
pub struct Ui {
    #[widget_id("inc")]
    pub inc: Button<Tally>,
    #[widget_id("value")]
    pub value: Label<String>,
}

/// What a handler may touch (M33), and what a reload keeps.
#[derive(Default, Serialize, Deserialize)]
pub struct Tally {
    pub count: i64,
}

#[derive(DeriveApp, DeriveReloadable)]
pub struct Counter {
    #[tree]
    pub ui: Ui,
    #[state]
    #[reload(keep)]
    #[bind(ui.value, count, "count {}")] // EDIT format
    pub tally: Tally,
}

fn on_inc(t: &mut Tally, _: &EventCtx) {
    let step = 1; // EDIT handler
    t.count += step;
}

/// The boot: the one place a `Counter` is built — by `main`, by the
/// library a reload loads, and by `Host::reload` (M49/T49.4).
fn boot() -> Counter {
    let mut c = Counter { ui: Ui::default(), tally: Tally::default() };
    c.ui.inc.label = "+";
    c.ui.inc.on_click = on_inc;
    c
}

// ANCHOR: program
/// The program, described once: the binary runs it, the library exports
/// it, and a reload boots the new build's copy of it (§2.2).
pub fn program() -> Program<Counter> {
    application(boot)
        .title("hot-window")
        .size(320, 160)
        .style(|_: &Counter| vec![Rule::new(Selector::Id("inc"), Patch::new().width(Length::FILL))])
}

// Only a hot build is a library a window loads (M49/T49.9).
#[cfg(feature = "hot")]
lumen_static_reload::export_program!(program);
// ANCHOR_END: program
