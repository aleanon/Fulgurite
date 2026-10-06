//! M55/T55.4 (R12-F5): the invalidation strategy of a window's host is a
//! type the application chooses, and no feature anywhere in the build
//! graph changes it. Rewritten by M89/T89.1 against [`WindowSlot`], which
//! replaced `Pane` (`docs/plan-windows.md`).
//!
//! **What it guards.** `Local::host` is a public field. Its strategy was
//! an alias, `Boundaries<256>` unless the `everything` feature was on, and
//! cargo unifies features across a build graph — so one crate turning
//! that diagnostic on changed the field's type for every other crate, and
//! `Boundaries<256>` being `Host`'s default too, the most natural
//! spelling, plain `Host<A>`, was exactly the one that stopped compiling
//! (E0308). **And a window holds no strategy at all**: a `WindowSlot`
//! keeps its session as a `Box<dyn Session>`, so a session of either
//! strategy goes in the same slot.
//!
//! A compile test: the functions below type-check or the binary does not
//! build. `just dev-shape` builds it in a graph with showcase's own
//! `everything` feature on, which is the build that used to break it.
use lumen_static::event::App;
use lumen_static::retain::Incremental;
use lumen_static::Everything;
use lumen_static_shell::session::{Local, Session};
use lumen_static_shell::Host;
use lumen_static_winit::{Policy, WindowSlot};

/// The spelling an author reaches for first.
pub fn host_of<A: App>(l: &mut Local<A, Policy>) -> &mut Host<A> {
    &mut l.host
}

/// The diagnostic, chosen in code: a session that says so.
pub fn everything_host_of<A: App>(l: &mut Local<A, Policy, Everything>) -> &mut Host<A, Incremental, Everything> {
    &mut l.host
}

/// Both strategies are the one session type a slot holds.
pub fn slot_sessions<A: App + 'static>(a: Local<A, Policy>, b: Local<A, Policy, Everything>) -> [Box<dyn Session>; 2] {
    [Box::new(a), Box::new(b)]
}

/// …and a slot answers with it, naming no strategy.
pub fn session_of(s: &mut WindowSlot) -> &mut dyn Session {
    s.session()
}

#[test]
fn a_session_holds_the_host_its_type_says_and_a_slot_holds_either() {
    // Nothing to run: the claim is that the functions compiled, in this
    // build graph, whatever features it carries.
    let _ = host_of::<NoApp>;
    let _ = everything_host_of::<NoApp>;
    let _ = slot_sessions::<NoApp>;
    let _ = session_of;
}

/// The smallest application: enough to name a `Local`.
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
