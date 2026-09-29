//! M61/T61.0 gate 2 (`docs/plan-lss.md`): an `@sheet` the file does not have is refused, and the
//! message names the file, line and column.
use lumen_static::{include_lss, StaticTree};
use lumen_static_widgets::ops::Label;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("title")]
    title: Label,
}

const SHEET: &[lumen_static::Rule] = include_lss!(Ui, "fail_unknown_sheet.lss", huge);

fn main() {
    let _ = SHEET;
}
