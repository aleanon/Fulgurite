// A widget may be named after a type the derive uses internally. The
// emitted code used to `use ::fulgurite::{Size, Point, ..}`, which
// shadowed the user's own type at the point the impl was written (F6.2).
use fulgurite_widgets::ops::Label;
use fulgurite::StaticTree;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Size {
    a: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Point {
    a: Label,
}

/// Collided with the `Handlers<S>` generic parameter.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct S {
    a: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Holder {
    s: Size,
    p: Point,
    q: S,
}

fn main() {}
