//! M55/T55.2 (R16-F4): a second `#[widget_id]` on one field is refused.
//! `parse_wid` returned the first and dropped the second.
use fulgurite::prelude::*;
use fulgurite_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    #[widget_id("save")]
    #[widget_id("store")]
    a: Label,
}

fn main() {}
