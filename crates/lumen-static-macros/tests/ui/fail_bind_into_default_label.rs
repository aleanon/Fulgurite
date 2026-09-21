use lumen_static_widgets::ops::Label;
use lumen_static::{App, StaticTree};

// `Label`'s default storage is `&'static str`, which can only accept a
// new value by leaking — so a binding into one must not compile. This
// pins the `TextBuffer` split (T6.2/F1.x).
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    a: Label,
}

#[derive(App, Default)]
struct Leaky {
    #[tree]
    ui: Ui,
    #[bind(ui.a, "{}")]
    n: i64,
}

fn main() {}
