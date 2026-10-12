//! M61/T61.0 gate 2 (`docs/plan-lss.md`): a file that does not exist is refused, and the
//! message names the file, line and column.
use fulgurite::{include_lss, StaticTree};
use fulgurite_widgets::ops::Label;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("title")]
    title: Label,
}

const SHEET: &[fulgurite::Rule] = include_lss!(Ui, "no_such.lss");

fn main() {
    let _ = SHEET;
}
