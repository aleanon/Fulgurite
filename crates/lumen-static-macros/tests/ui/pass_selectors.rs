// Both documented spellings of the selector attributes, asserted at
// runtime as well as compiled — the documented string form silently
// produced an empty class list until F1.5.
use lumen_static::ops::{Fixed, Label};
use lumen_static::{StaticTree, Widget};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("head")]
    #[widget_class("row", "striped")]
    head: Label,
    #[widget_class(row)]
    body: Fixed,
}

fn main() {
    let t = <Ui as Widget>::SUB_TABLE;
    assert_eq!(t[1].wid, "head");
    assert_eq!(t[1].classes, &["row", "striped"]);
    assert_eq!(t[2].classes, &["row"]);
}
