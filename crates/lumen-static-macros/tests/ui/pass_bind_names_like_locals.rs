//! M55/T55.1 (R16-F3): a binding's user tokens resolve in the user's
//! scope, not the derive's.
//!
//! `update_marked` declared `let mut changed = false;` at call-site span
//! and then spliced the user's `#[bind]` tokens beside it, so a user item
//! named `changed` was shadowed by the derive's `bool`: a `with = changed`
//! projection became a fn-pointer mismatch at the user's token, and an
//! extra format argument calling `changed()` became "expected function,
//! found `bool`". Each application below uses one.
use lumen_static::{App, StaticTree};
use lumen_static_widgets::ops::Label;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct Ui {
    pub a: Label<String>,
}

mod with_form {
    use super::*;

    /// A projection whose name is the derive's old local.
    pub fn changed(dst: &mut Label<String>, src: &u32) {
        lumen_static::bind::set_if_changed(dst, &format!("{src} 42"));
    }

    #[derive(App, Default)]
    pub struct Projected {
        #[tree]
        pub ui: Ui,
        #[bind(ui.a, with = changed)]
        pub n: u32,
    }
}

mod extra_form {
    use super::*;

    /// A value whose name is the derive's old local.
    pub fn changed() -> u32 {
        42
    }

    #[derive(App, Default)]
    pub struct Formatted {
        #[tree]
        pub ui: Ui,
        #[bind(ui.a, "{} {}", changed())]
        pub n: u32,
    }
}

/// The other locals the expansion declares, named by a user.
mod the_rest {
    use super::*;

    pub fn __ln_all() -> u32 {
        42
    }

    #[derive(App, Default)]
    pub struct Rest {
        #[tree]
        pub ui: Ui,
        #[bind(ui.a, "{} {}", __ln_all())]
        pub n: u32,
    }
}

fn main() {
    use lumen_static::event::App as _;
    let mut p = with_form::Projected { n: 3, ..Default::default() };
    p.update();
    assert_eq!(p.ui.a.as_str(), "3 42");
    let mut f = extra_form::Formatted { n: 3, ..Default::default() };
    f.update();
    assert_eq!(f.ui.a.as_str(), "3 42");
    let mut r = the_rest::Rest { n: 3, ..Default::default() };
    r.update();
    assert_eq!(r.ui.a.as_str(), "3 42");
}
