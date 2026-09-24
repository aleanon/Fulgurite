//! A window library: a counter whose **bound label's format** is the
//! edit between v1 and v2, and whose **count** is `#[reload(keep)]` —
//! M37/T37.1 (`docs/plan-window-reload.md`).
//!
//! A button fills the top of a fixed 240×120 window, so a loader with no
//! tree in hand can click it at a known point; the label under it is the
//! only other text on the frame, with the kept `name` below it.

use lumen_static::event::{Application, EventCtx};
use lumen_static::style::{Edges, Length, Patch, Rule, Selector};
use lumen_static::{App as DeriveApp, Reloadable as DeriveReloadable, StaticTree};
use lumen_static_shell::program::{application, Program};
use lumen_static_widgets::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
pub struct Ui {
    #[widget_id("inc")]
    pub inc: Button<Tally>,
    #[widget_id("value")]
    pub value: Label<String>,
    #[widget_id("name")]
    pub name: Label<String>,
}

/// The state a handler may touch (M33), and what a swap keeps.
#[derive(Default, Serialize, Deserialize)]
pub struct Tally {
    pub count: i64,
}

#[derive(DeriveApp, DeriveReloadable)]
pub struct Counter {
    #[tree]
    #[reload(fresh)]
    pub ui: Ui,
    #[state]
    #[reload(keep)]
    #[cfg_attr(not(feature = "v2"), bind(ui.value, count, "count {}"))]
    #[cfg_attr(feature = "v2", bind(ui.value, count, "clicks: {}"))]
    pub tally: Tally,
    /// The edit v3 makes: this kept field's **type** changes, so v1's or
    /// v2's saved value no longer fits (T37.3). It must reset to v3's
    /// boot value while `tally` survives.
    #[reload(keep)]
    #[bind(ui.name, "name {}")]
    pub name: Name,
}

#[cfg(not(feature = "v3"))]
pub type Name = String;
#[cfg(feature = "v3")]
pub type Name = u32;

#[cfg(not(feature = "v3"))]
fn boot_name() -> Name {
    String::from("first")
}
#[cfg(feature = "v3")]
fn boot_name() -> Name {
    7
}

fn on_inc(t: &mut Tally, _: &EventCtx) {
    t.count += 1;
}

impl Application for Counter {
    fn build() -> Self {
        let mut c = Counter { ui: Ui::default(), tally: Tally::default(), name: boot_name() };
        c.ui.inc.label = "+";
        c.ui.inc.on_click = on_inc;
        c
    }
}

/// The program, described once — what a static `main` would run, and
/// what the library exports.
pub fn program() -> Program<Counter> {
    application(Counter::build).title("reload-app").size(240, 120).style(|_: &Counter| {
        vec![
            Rule::new(Selector::Kind("Ui"), Patch::new().padding(Edges::all(0.0))),
            Rule::new(Selector::Id("inc"), Patch::new().width(Length::FILL).height(Length::Fixed(60.0))),
        ]
    })
}

lumen_static_reload::export_program!(program);
