use fulgurite_widgets::ops::Label;
use fulgurite::{App, StaticTree};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    a: Label<String>,
}

#[derive(App, Default)]
struct BadBind {
    #[tree]
    ui: Ui,
    #[bind(ui.a)]
    n: i64,
}

fn main() {}
