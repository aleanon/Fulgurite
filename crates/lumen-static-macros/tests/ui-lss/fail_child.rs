//! M61/T61.0 gate 2 (`docs/plan-lss.md`): `fail_child.lss` is refused, and the
//! message names the file, line and column.
use lumen_static::{include_lss, StaticTree};
use lumen_static_widgets::ops::Label;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("title")]
    title: Label,
}

const SHEET: &[lumen_static::Rule] = include_lss!(Ui, "fail_child.lss");

fn main() {
    let _ = SHEET;
}
