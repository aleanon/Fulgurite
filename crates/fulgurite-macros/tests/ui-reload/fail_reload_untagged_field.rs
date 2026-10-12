//! M49/T49.0 (R5-F7): an untagged field whose type is not `Reloadable`.
//!
//! The derive treats an untagged field as nested state, so it needs the
//! trait. A plain number wants `#[reload(keep)]`; a widget that has no
//! impl wants one, or `#[reload(fresh)]`. The error must land on the
//! **field**, and say which attribute fixes it — before T49.8 it landed
//! on the derive and named neither.

use fulgurite::{App, Reloadable, StaticTree};
use fulgurite_widgets::ops::Label;

/// A widget of the application's own with no `Reloadable` impl.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Panel {
    a: Label,
}

#[derive(StaticTree, Default, Reloadable)]
#[widget(column, spacing = 0.0)]
struct Ui {
    panel: Panel,
}

#[derive(App, Reloadable)]
struct Counter {
    #[tree]
    ui: Ui,
    count: u32,
}

fn main() {}
