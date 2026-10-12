use fulgurite_widgets::ops::Label;
use fulgurite::StaticTree;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct BadClass {
    #[widget_class(1)]
    a: Label,
}

fn main() {}
