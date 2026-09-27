//! M55/T55.4 (R12-F5): the invalidation strategy of a window's host is a
//! type the application chooses, and no feature anywhere in the build
//! graph changes it.
//!
//! **What it guards.** `Pane::host` and `Local::host` are public fields
//! of type `Host<A, Ret, Inval>`, and `Inval` was `Boundaries<256>`
//! unless the `everything` feature was on. Cargo unifies features across
//! a build graph, so one crate turning that diagnostic on changed these
//! fields' types for every other crate -- and `Boundaries<256>` is also
//! `Host`'s default, so the most natural spelling, plain `Host<A>`, is
//! exactly the one that stopped compiling (E0308).
//!
//! It is a compile test: the function below either type-checks or the
//! test binary does not build. It must build in the default graph *and*
//! in one where the diagnostic is on.
use lumen_static::event::App;
use lumen_static_shell::Host;
use lumen_static_winit::Pane;

/// The spelling an author reaches for first.
pub fn host_of<A: App>(p: &mut Pane<A>) -> &mut Host<A> {
    &mut p.host
}

#[test]
fn a_pane_holds_the_host_its_type_says() {
    // Nothing to run: the claim is that `host_of` compiled.
    let _ = host_of::<NoApp>;
}

/// The smallest application: enough to name a `Pane`.
#[derive(lumen_static::StaticTree, Default)]
#[widget(column)]
struct Ui {
    a: lumen_static_widgets::ops::Spacer,
}

#[derive(lumen_static::App, Default)]
struct NoApp {
    #[tree]
    ui: Ui,
}
