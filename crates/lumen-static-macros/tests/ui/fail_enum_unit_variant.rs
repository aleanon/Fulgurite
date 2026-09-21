use lumen_static_widgets::ops::Label;
use lumen_static::StaticTree;

#[derive(StaticTree)]
enum Slot {
    Text(Label),
    Nothing,
}

fn main() {}
