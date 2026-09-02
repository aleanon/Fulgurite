use lumen_static::ops::Label;
use lumen_static::StaticTree;

#[derive(StaticTree)]
enum Slot {
    Text(Label),
    Nothing,
}

fn main() {}
