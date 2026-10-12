//! M55/T55.2: `Reloadable` needs named fields -- a saved field is keyed
//! by its name.
use fulgurite::Reloadable;

#[derive(Reloadable, Default)]
struct Pair(i64, i64);

fn main() {}
