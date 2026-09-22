//! **Does an edited stylesheet reach the frame on a hot reload?**
//!
//! No. This test demonstrates that, and it is written to be inverted
//! when the fix lands.
//!
//! # Why this was looked for
//!
//! `docs/events-decision.md` rule 3 says nothing that outlives a frame
//! may hold a code reference, and `fresh()` exists so handler `fn`
//! pointers come from the build that is loaded *now*. A stylesheet is
//! the same kind of thing and was never treated as such:
//!
//! - `Selector::Id`, `Selector::Class` and `Selector::Kind` each hold a
//!   `&'static str`, which points into the `.rodata` of whichever
//!   library built the `Rule`;
//! - `Host::keep_sheet` copies the `Rule`s — `extend_from_slice` — but
//!   a copy of a fat pointer is the same pointer, so the strings still
//!   belong to the old build;
//! - nothing in the reload sequence re-asks. `hot_new`, `hot_save` and
//!   `hot_restore` deal only with the application struct, and
//!   `fixtures/hot-host` never constructs a `Host` at all, so
//!   `just hot-demo` has never had a stylesheet in it to notice.
//!
//! The user-visible symptom is the one in the test name: edit your
//! stylesheet, reload, and the frame does not change. Safe — the loader
//! never `dlclose`s — and wrong in exactly the way that decision record
//! calls "last week's logic".
//!
//! # What this test does and does not prove
//!
//! It runs **in-process**, so it proves the half that does not need two
//! dylibs: *a host that has been given a stylesheet keeps applying it
//! after its application is replaced by a freshly built one, and there
//! is no API by which the new build's sheet could reach it.* That is the
//! defect a user hits.
//!
//! It does **not** prove the dangling-`&'static str` half, which needs
//! two real libraries and a pointer-range check. That is a second test
//! and is not written yet; this file's claim is deliberately the
//! smaller one.
//!
//! # Inverting this test
//!
//! When `Application::build`/`Application::style` land, `Host` will
//! re-ask for the sheet as part of the swap, and
//! `V2` is what the panel must then be. The two `V1` expectations below
//! become `V2`, `the_host_is_never_asked_for_a_sheet_again` is deleted,
//! and the test name loses its "and that is the defect" suffix.
//! Nothing else changes:
//! the setup is already the shape the fix has to satisfy.

use lumen_static::display::Color;
use lumen_static::prelude::*;
use lumen_static::reload::Reloadable as _;
use lumen_static::style::{Patch, Rule, Selector};
use lumen_static::{App as DeriveApp, Reloadable as DeriveReloadable, StaticTree};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

/// A panel whose background comes from the sheet and from nowhere else,
/// so "which sheet is in effect" is a question about one `u8`.
#[derive(StaticTree, Default, DeriveReloadable)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("panel")]
    panel: Fixed,
}

#[derive(DeriveApp, DeriveReloadable, Default)]
struct Session {
    #[tree]
    ui: Ui,
    /// Carried across the swap, so the test also shows the reload
    /// sequence *worked* — see the "setup silently did not happen"
    /// shape in `AGENT.md`. Without this a sheet that never updated and
    /// a reload that never happened look identical.
    #[reload(keep)]
    n: u32,
}

/// The sheet v1 ships.
const V1: Color = Color(0xc8, 0x1e, 0x1e, 0xff);
/// The sheet the author edits to, and rebuilds. The frame must show
/// this after the swap; today it never does.
const V2: Color = Color(0x1e, 0xc8, 0x3c, 0xff);

fn sheet(bg: Color) -> Vec<Rule> {
    vec![Rule::new(
        Selector::Id("panel"),
        Patch::new().background(bg).width(Length::Fixed(80.0)).height(Length::Fixed(40.0)),
    )]
}

/// The panel's **resolved** background — what the cascade actually put
/// in the state array, not what the sheet says. Reading the sheet back
/// would test that a `Vec` still holds what was pushed into it.
fn panel_bg<A: lumen_static::event::App>(h: &Host<A>) -> Color {
    let i = lumen_static::style::index_of_wid(A::Tree::SUB_TABLE, "panel")
        .expect("#panel must be on the table");
    h.states[i].bg
}

#[test]
fn an_edited_stylesheet_does_not_reach_the_frame_on_reload_and_that_is_the_defect() {
    let mut host: Host<Session> = Host::new(Session::default(), 200, 100);
    host.set_stylesheet(&sheet(V1));
    host.app.n = 7;
    host.pump();

    assert_eq!(
        panel_bg(&host),
        V1,
        "setup: v1's sheet must be in effect before the swap, or the assertion below \
         is comparing two frames that never had a sheet"
    );

    // ---- the swap, exactly as the loader performs it -----------------
    //
    // `hot_save` on the old app, `hot_new` (which is `fresh()`) from the
    // new library, `hot_restore` to graft the keep-fields back. In
    // process there is one build, so this models the *sequence*; what it
    // exercises is that the sequence touches the application and never
    // the host's sheet.
    let saved = host.app.save();
    host.app = Session::fresh();
    host.app.restore(&saved);
    host.pump();

    assert_eq!(
        host.app.n, 7,
        "the reload sequence did not happen: a kept field did not survive, so a \
         stale sheet below would prove nothing"
    );

    // ---- the defect ---------------------------------------------------
    assert_eq!(
        panel_bg(&host),
        V1,
        "THE DEFECT: after a reload the frame still uses the sheet the *previous* \
         build supplied. Invert this to V2 when `Application::style` lands."
    );
    assert_ne!(
        panel_bg(&host),
        V2,
        "if this fires, the sheet now updates on reload and this whole test should \
         be inverted rather than deleted"
    );
}

/// **The other direction, and the reason the test above is not
/// vacuous.**
///
/// `AGENT.md`: a gate that refuses bad input must also accept good
/// input, checked in the same session. The test above asserts that a
/// value *did not* change, and the way that assertion lies is if the
/// value could never change — a cascade that never ran, a `bg` nothing
/// writes, a panel the sheet does not match.
///
/// So: perform the identical swap, then hand the host the new sheet by
/// hand. The panel turns `V2`. Every part of the machinery works; the
/// only missing piece is that nothing *asks*, which is precisely the
/// defect and precisely what `Application::style` would supply.
#[test]
fn the_same_swap_does_change_the_frame_when_someone_applies_the_new_sheet() {
    let mut host: Host<Session> = Host::new(Session::default(), 200, 100);
    host.set_stylesheet(&sheet(V1));
    host.pump();
    assert_eq!(panel_bg(&host), V1, "setup: v1's sheet is in effect");

    let saved = host.app.save();
    host.app = Session::fresh();
    host.app.restore(&saved);
    // The one line the reload sequence does not contain.
    host.set_stylesheet(&sheet(V2));
    host.pump();

    assert_eq!(
        panel_bg(&host),
        V2,
        "the cascade, the selector and the state array all work across a swap — so \
         the stale frame above is caused by nothing re-asking, not by a tree that \
         cannot be restyled"
    );
}

#[test]
fn the_host_is_never_asked_for_a_sheet_again() {
    let mut host: Host<Session> = Host::new(Session::default(), 200, 100);
    host.set_stylesheet(&sheet(V1));
    host.pump();
    let before = host.stylesheet().len();

    let saved = host.app.save();
    host.app = Session::fresh();
    host.app.restore(&saved);
    host.pump();

    // The mechanism behind the test above: the sheet is a `Vec<Rule>`
    // the host owns, the swap never touches it, and no trait method
    // exists that could hand it a new one. `set_stylesheet` is the only
    // way in and only an application's own `main` calls it — which the
    // reload path does not re-run.
    assert_eq!(
        host.stylesheet().len(),
        before,
        "the sheet changed across a swap; that would mean a path exists and this \
         test is describing the wrong mechanism"
    );
    assert_eq!(
        host.stylesheet()[0].patch.background,
        Some(V1),
        "the host still holds the rule the pre-swap build handed it"
    );
}
