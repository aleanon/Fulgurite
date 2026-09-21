use lumen_static_widgets::ops::Label;
use lumen_static::{App, StaticTree};

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
