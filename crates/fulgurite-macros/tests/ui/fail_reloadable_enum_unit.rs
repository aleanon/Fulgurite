//! M55/T55.2: a `Reloadable` enum's variant holds one widget.
use fulgurite::Reloadable;

#[derive(Reloadable)]
enum Pane {
    Empty,
}

fn main() {}
