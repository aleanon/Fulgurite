//! M55/T55.2 (R16-F4): an empty `#[widget_id]` is refused. It meant the
//! same as no id at all, so the author's selector matched nothing.
use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    #[widget_id("")]
    a: Label,
}

fn main() {}
