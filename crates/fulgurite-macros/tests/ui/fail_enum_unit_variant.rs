use fulgurite_widgets::ops::Label;
use fulgurite::StaticTree;

#[derive(StaticTree)]
enum Slot {
    Text(Label),
    Nothing,
}

fn main() {}
