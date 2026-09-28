//! M61/T61.0 gate 3 (`docs/plan-lss.md`): an `#id` the tree does not have fails to compile, by name
//! and at the rule's line and column (§2.3).
use lumen_static::{include_lss, StaticTree};
use lumen_static_widgets::ops::Label;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("title")]
    title: Label,
}

const SHEET: &[lumen_static::Rule] = include_lss!(Ui, "fail_tree_id.lss");

fn main() {
    let _ = SHEET;
}
