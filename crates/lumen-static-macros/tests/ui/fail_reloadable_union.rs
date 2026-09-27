//! M55/T55.2: `Reloadable` on a union names what it derives.
use lumen_static::Reloadable;

#[derive(Reloadable)]
union Bits {
    a: u32,
    b: f32,
}

fn main() {}
