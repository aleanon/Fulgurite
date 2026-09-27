//! M55/T55.2: `StaticTree` on a union names the two shapes it derives.
use lumen_static::StaticTree;

#[derive(StaticTree)]
#[widget(column)]
union Bits {
    a: u32,
    b: f32,
}

fn main() {}
