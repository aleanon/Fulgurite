//! M31/T31.2: a reloadable application must not also derive `Default`.
//!
//! `Reloadable::fresh()` routes to `Application::build()` for a struct
//! with `#[tree]`, so a `Default` beside it is a **second construction
//! path** — `main` takes one, the hot swap takes the other, and a
//! handler assigned in only one of them comes back as its placeholder.
//!
//! Rust has no negative bounds, so the derive emits a blanket impl that
//! overlaps only when `Self: Default`. The error is E0119 and the
//! trait's name is the message.
//!
//! The passing direction is `pass_reloadable_app_without_default.rs`.

use lumen_static::{App, Reloadable, StaticTree};
use lumen_static_widgets::ops::Label;

#[derive(StaticTree, Default, Reloadable)]
#[widget(column, spacing = 0.0)]
struct Ui {
    a: Label,
}

#[derive(App, Reloadable, Default)]
struct Session {
    #[tree]
    ui: Ui,
    #[reload(keep)]
    n: u32,
}

impl lumen_static::event::Application for Session {
    fn build() -> Self {
        Session { ui: Ui::default(), n: 0 }
    }
}

fn main() {}
