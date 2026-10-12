//! M55/T55.2 (R16-F4): `#[widget_id]` on the struct is refused. A
//! container's id is given by its parent's field; on the struct it was
//! accepted and nothing read it, so the root kept the empty id.
use fulgurite::prelude::*;
use fulgurite_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(column)]
#[widget_id("root")]
struct Ui {
    a: Label,
}

fn main() {}
