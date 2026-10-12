//! M55/T55.2 (R16-F4): an unknown `#[widget]` key names every key the
//! parser accepts.
use fulgurite::prelude::*;
use fulgurite_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(row, gap = 4.0)]
struct Ui {
    a: Label,
}

fn main() {}
