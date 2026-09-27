//! M55/T55.2 (R16-F4): a `#[widget]` key given twice is refused. The
//! second `spacing` used to win silently.
use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 2.0, spacing = 8.0)]
struct Ui {
    a: Label,
}

fn main() {}
