//! M61/T61.0 gate 3 (`docs/plan-lss.md`): an `#id` the tree does not have fails to compile, by name
//! and at the rule's line and column (§2.3).
use fulgurite::{include_lss, StaticTree};
use fulgurite_widgets::ops::Label;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("title")]
    title: Label,
}

const SHEET: &[fulgurite::Rule] = include_lss!(Ui, "tree_fail_id.lss");

fn main() {
    let _ = SHEET;
}
