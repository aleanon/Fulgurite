//! M55/T55.4 (R12-F5): the invalidation strategy of a window's host is a
//! type the application chooses, and no feature anywhere in the build
//! graph changes it.
//!
//! **What it guards.** `Pane::host` and `Local::host` are public fields.
//! Their strategy was an alias, `Boundaries<256>` unless the `everything`
//! feature was on, and cargo unifies features across a build graph — so
//! one crate turning that diagnostic on changed these fields' types for
//! every other crate, and `Boundaries<256>` being `Host`'s default too,
//! the most natural spelling, plain `Host<A>`, was exactly the one that
//! stopped compiling (E0308).
//!
//! A compile test: the functions below type-check or the binary does not
//! build. `just dev-shape` builds it in a graph with showcase's own
//! `everything` feature on, which is the build that used to break it.
use lumen_static::event::App;
use lumen_static::retain::Incremental;
use lumen_static::Everything;
use lumen_static_shell::Host;
use lumen_static_winit::Pane;

/// The spelling an author reaches for first.
pub fn host_of<A: App>(p: &mut Pane<A>) -> &mut Host<A> {
    &mut p.host
}

/// The diagnostic, chosen in code: a pane that says so.
pub fn everything_host_of<A: App>(p: &mut Pane<A, Everything>) -> &mut Host<A, Incremental, Everything> {
    &mut p.host
}

#[test]
fn a_pane_holds_the_host_its_type_says() {
    // Nothing to run: the claim is that both functions compiled, in this
    // build graph, whatever features it carries.
    let _ = host_of::<NoApp>;
    let _ = everything_host_of::<NoApp>;
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
