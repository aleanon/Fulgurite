//! The other direction of `fail_reloadable_app_derives_default.rs` —
//! M31/T31.2.
//!
//! `AGENT.md`: a gate that refuses bad input must also accept good
//! input, checked in the same session. A refusal built out of a
//! coherence conflict is especially worth checking both ways, because
//! the failure mode of getting it wrong is a blanket impl that overlaps
//! *everything* — a gate that refuses every application, including the
//! correct ones.
//!
//! Identical to the failing case but for the missing `Default`.

use fulgurite::{App, Reloadable, StaticTree};
use fulgurite_widgets::ops::Label;

#[derive(StaticTree, Default, Reloadable)]
#[widget(column, spacing = 0.0)]
struct Ui {
    a: Label,
}

#[derive(App, Reloadable)]
struct Session {
    #[tree]
    ui: Ui,
    #[reload(keep)]
    n: u32,
}

impl Session {
    fn build() -> Self {
        Session { ui: Ui::default(), n: 0 }
    }
}

fn main() {
    // The widget `Ui` still derives `Default` and must keep working:
    // the refusal is scoped to applications, and `Repeat` materialises
    // rows with `T::default()` every frame.
    let _ = Ui::default();
    let _ = Session::build();
}
