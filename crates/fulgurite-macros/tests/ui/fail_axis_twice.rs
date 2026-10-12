//! M55/T55.2 (R16-F4): an axis given twice is refused, naming the first.
//! It used to overwrite: `#[widget(row, column)]` laid out as a column,
//! and so did the same two keywords in two separate attributes.
use fulgurite::prelude::*;
use fulgurite_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(row, column)]
struct OneAttribute {
    a: Label,
}

#[derive(StaticTree, Default)]
#[widget(row)]
#[widget(column, spacing = 2.0)]
struct TwoAttributes {
    a: Label,
}

fn main() {}
