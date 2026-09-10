//! M11/T11.6: `spacing` has no meaning on a stack, and is rejected at
//! the token that asked for it rather than accepted and ignored.
use lumen_static::ops::Fixed;
use lumen_static::StaticTree;

#[derive(StaticTree, Default)]
#[widget(stack, spacing = 4.0)]
struct Spaced {
    a: Fixed,
    b: Fixed,
}

fn main() {}
