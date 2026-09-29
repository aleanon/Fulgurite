//! M61/T61.0 gate 3 (`docs/plan-lss.md`): a `.class` no node carries fails to compile, by name
//! and at the rule's line and column (§2.3).
use lumen_static::{include_lss, StaticTree};
use lumen_static_widgets::ops::Label;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("title")]
    title: Label,
}

const SHEET: &[lumen_static::Rule] = include_lss!(Ui, "tree_fail_class.lss");

fn main() {
    let _ = SHEET;
}
