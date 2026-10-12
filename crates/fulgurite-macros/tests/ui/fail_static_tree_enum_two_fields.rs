//! M55/T55.2: an enum widget's variant holds exactly one widget.
use fulgurite::prelude::*;
use fulgurite_widgets::prelude::*;

#[derive(StaticTree)]
enum Pane {
    Two(Label, Label),
}

fn main() {}
