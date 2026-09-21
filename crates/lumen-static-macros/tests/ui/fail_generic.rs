use lumen_static_widgets::ops::Label;
use lumen_static::StaticTree;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Generic<T> {
    a: Label,
    t: std::marker::PhantomData<T>,
}

fn main() {}
