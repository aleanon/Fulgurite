//! M27/T27.8: a grid needs a column count.
//!
//! Without one it is a wrap, and silently becoming a different layout is
//! worse than refusing — the author asked for columns.
use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(grid, spacing = 4.0)]
struct Ui {
    a: Label,
}

fn main() {}
