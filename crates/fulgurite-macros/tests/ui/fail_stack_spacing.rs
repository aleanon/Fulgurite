//! M11/T11.6: `spacing` has no meaning on a stack, and is rejected at
//! the token that asked for it rather than accepted and ignored.
use fulgurite_widgets::ops::Fixed;
use fulgurite::StaticTree;

#[derive(StaticTree, Default)]
#[widget(stack, spacing = 4.0)]
struct Spaced {
    a: Fixed,
    b: Fixed,
}

fn main() {}
