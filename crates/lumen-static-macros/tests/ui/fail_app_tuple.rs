//! M55/T55.2: `App` needs named fields -- `#[tree]` names one.
use lumen_static::App;

#[derive(App)]
struct Counter(i64);

fn main() {}
