//! M27/T27.8: `columns` belongs to `grid` and nothing else.
//!
//! Accepting it on a row and ignoring it is the shape the `stack`/
//! `spacing` refusal beside it exists to prevent — someone spends an
//! afternoon on a layout that was never going to change.
use fulgurite::prelude::*;
use fulgurite_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(row, columns = 3)]
struct Ui {
    a: Label,
}

fn main() {}
