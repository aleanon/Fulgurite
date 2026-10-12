//! M55/T55.2: the accepting direction of the repeat refusals -- every
//! key given once still compiles, in one attribute or split across two.
//! One token away from `fail_axis_twice.rs`, `fail_widget_key_twice.rs`,
//! `fail_widget_id_twice.rs`, `fail_app_key_twice.rs` and
//! `fail_reload_twice.rs`.
use fulgurite::{App, Reloadable, StaticTree};
use fulgurite_widgets::ops::Label;

#[derive(StaticTree, Default)]
#[widget(grid, columns = 3)]
#[widget(spacing = 2.0, fuse)]
struct Cells {
    #[widget_id("a")]
    #[widget_class("cell")]
    a: Label,
    b: Label,
}

#[derive(StaticTree, Default)]
#[widget(column)]
#[widget_class("root")]
struct Ui {
    #[widget_id("cells")]
    cells: Cells,
}

#[derive(StaticTree)]
enum Pane {
    A(Label),
    B(Cells),
}

impl Default for Pane {
    fn default() -> Self {
        Pane::A(Label::default())
    }
}

#[derive(Default)]
struct Tally {
    n: i64,
}

fn t(_: &mut Counter, _: f32) -> bool {
    false
}
fn after(_: &mut Counter) -> bool {
    false
}

#[derive(App, Reloadable)]
#[app(tick = t)]
#[app(after_update = after)]
struct Counter {
    #[tree]
    #[reload(fresh)]
    ui: Ui,
    #[state]
    #[reload(fresh)]
    state: Tally,
    #[reload(keep)]
    kept: i64,
}

fn main() {
    let _ = Pane::default();
    let _ = Counter { ui: Ui::default(), state: Tally::default(), kept: 0 }.state.n;
}
