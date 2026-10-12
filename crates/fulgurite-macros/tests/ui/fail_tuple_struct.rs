use fulgurite_widgets::ops::Label;
use fulgurite::StaticTree;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Tup(Label);

fn main() {}
