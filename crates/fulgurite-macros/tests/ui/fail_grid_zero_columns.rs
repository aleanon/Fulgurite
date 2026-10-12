//! M55/T55.2 (R16-F4): a grid of zero columns is refused at the count.
//! Untested until M55 (see `fail_tree_twice.rs`).
use fulgurite::prelude::*;
use fulgurite_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(grid, columns = 0)]
struct Ui {
    a: Label,
}

fn main() {}
