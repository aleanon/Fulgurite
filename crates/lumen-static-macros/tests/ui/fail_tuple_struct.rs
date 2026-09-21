use lumen_static_widgets::ops::Label;
use lumen_static::StaticTree;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Tup(Label);

fn main() {}
