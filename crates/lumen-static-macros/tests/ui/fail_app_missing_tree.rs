use lumen_static::ops::Label;
use lumen_static::{App, StaticTree};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    a: Label,
}

#[derive(App, Default)]
struct NoTree {
    ui: Ui,
    n: i32,
}

fn main() {}
