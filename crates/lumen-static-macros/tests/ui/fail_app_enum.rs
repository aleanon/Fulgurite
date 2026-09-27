//! M55/T55.2: `App` derives a struct.
use lumen_static::App;

#[derive(App)]
enum Counter {
    A,
}

fn main() {}
