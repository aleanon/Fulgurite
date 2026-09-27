//! M55/T55.2: a `Reloadable` enum's variant holds exactly one widget.
use lumen_static::Reloadable;
use lumen_static_widgets::ops::Label;

#[derive(Reloadable)]
enum Pane {
    Two(Label, Label),
}

fn main() {}
