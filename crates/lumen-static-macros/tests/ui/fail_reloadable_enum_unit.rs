//! M55/T55.2: a `Reloadable` enum's variant holds one widget.
use lumen_static::Reloadable;

#[derive(Reloadable)]
enum Pane {
    Empty,
}

fn main() {}
