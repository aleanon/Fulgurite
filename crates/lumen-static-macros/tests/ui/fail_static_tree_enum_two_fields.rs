//! M55/T55.2: an enum widget's variant holds exactly one widget.
use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;

#[derive(StaticTree)]
enum Pane {
    Two(Label, Label),
}

fn main() {}
