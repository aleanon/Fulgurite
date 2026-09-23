//! **An edited stylesheet reaches the frame on a hot reload** —
//! M31/T31.0.
//!
//! This file was written the other way round, on 2026-09-22: it
//! *demonstrated* that it did not, with two injections that fired by
//! name, and its header said exactly how to invert it once the fix
//! landed. This is that inversion. The setup is unchanged, which was
//! the point of writing it first.
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
//! dylibs: *`Host::reload` rebuilds the application from the code loaded
//! now and re-asks it for a stylesheet.* That is the defect a user
//! hits, fixed.
//!
//! It does **not** prove the dangling-`&'static str` half, which needs
//! two real libraries and a pointer-range check. That is a second test
//! and is not written yet; this file's claim is deliberately the
//! smaller one.
//!
//! # The shape of the fix
//!
//! `Application::build` supplies the application from the currently
//! loaded code, and the program's style source — kept by the host since
//! M32, when `Application::style` was retired — supplies its sheet;
//! `Host::adopt` re-asks it. The `V2` sheet below is what an author
//! edited to and rebuilt, and the assertion is that the frame shows it.

use lumen_static::display::Color;
use lumen_static::prelude::*;
use lumen_static::event::Application;
use lumen_static::reload::Reloadable as _;
use lumen_static::style::{Patch, Rule, Selector};
use lumen_static::{App as DeriveApp, Reloadable as DeriveReloadable, StaticTree};
use lumen_static_shell::program::application;
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

#[derive(DeriveApp, DeriveReloadable)]
struct Session {
    #[tree]
    ui: Ui,
    /// Carried across the swap, so the test also shows the reload
    /// sequence *worked* — see the "setup silently did not happen"
    /// shape in `AGENT.md`. Without this a sheet that never updated and
    /// a reload that never happened look identical.
    #[reload(keep)]
    n: u32,
    /// What the program's style source reads (`launched_by_v1`).
    /// `#[reload(fresh)]` because a sheet is code-shaped data and must
    /// come from the new build — which is this whole file's subject.
    #[reload(fresh)]
    sheet: Vec<Rule>,
}

/// **The build step, and the only one.** `Reloadable::fresh` routes
/// here because this struct has a `#[tree]` field, so the swap below
/// runs exactly this code — and `Session` deliberately does not derive
/// `Default`, so there is no second path for it to disagree with.
impl Application for Session {
    fn build() -> Self {
        // The edited code: this build wants V2.
        Session { ui: Ui::default(), n: 0, sheet: sheet(V2) }
    }

}

/// **A host launched by the previous build**, through the one path a
/// host is built by (M32): a program whose boot is v1's and whose style
/// source reads the application's own sheet. That source is kept by the
/// host, and `adopt` re-asks it of whatever application it is handed —
/// which is the mechanism under test.
fn launched_by_v1() -> Host<Session> {
    application(|| Session { ui: Ui::default(), n: 0, sheet: sheet(V1) })
        .size(200, 100)
        .style(|s: &Session| s.sheet.clone())
        .host()
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

/// The swap, as a loader performs it: `save` the old application,
/// `fresh` one from the code loaded now — which routes to
/// `Application::build` because `Session` has a `#[tree]` field — and
/// graft the kept fields back.
fn rebuilt(old: &Session) -> Session {
    let saved = old.save();
    let mut next = Session::fresh();
    next.restore(&saved);
    next
}

#[test]
fn an_edited_stylesheet_reaches_the_frame_on_reload() {
    // Launched by the *previous* build, which installed V1.
    let mut host: Host<Session> = launched_by_v1();
    host.app.n = 7;
    host.pump();

    assert_eq!(
        panel_bg(&host),
        V1,
        "setup: the previous build's sheet must be in effect, or the assertion \
         below is comparing two frames that never had a sheet"
    );

    // The swap. `build()` is the edited code, and it asks for V2.
    let next = rebuilt(&host.app);
    host.adopt(next);
    host.pump();

    assert_eq!(
        host.app.n, 7,
        "the reload sequence did not happen: a kept field did not survive, so the \
         sheet assertion below would prove nothing"
    );
    assert_eq!(
        panel_bg(&host),
        V2,
        "M31/T31.0: the host must re-ask the new build for its stylesheet. Before \
         the host re-asked a sheet source this was V1 — the previous build's rules, \
         kept because nothing asked."
    );
}

/// **The control, and the reason the test above is not vacuous.**
///
/// The same rebuild, adopted the way the host used to do it — assign
/// the application and say nothing about the sheet. The frame keeps V1.
/// That is the defect this milestone removed, kept as a test so the
/// assertion above is known to be watching the sheet source and not something
/// incidental to `pump`.
#[test]
fn a_swap_that_does_not_re_ask_keeps_the_previous_builds_sheet() {
    let mut host: Host<Session> = launched_by_v1();
    host.pump();
    assert_eq!(panel_bg(&host), V1, "setup: V1 is in effect");

    // `adopt` minus its one operative line.
    host.app = rebuilt(&host.app);
    host.pump();

    assert_eq!(
        panel_bg(&host),
        V1,
        "replacing the application alone cannot change the sheet — if this is V2, \
         something other than `adopt` is applying it and the gate above is loose"
    );
}

#[test]
fn the_host_is_asked_for_a_sheet_on_every_swap() {
    let mut host: Host<Session> = launched_by_v1();
    host.pump();
    assert_eq!(
        host.stylesheet()[0].patch.background,
        Some(V1),
        "setup: the host holds the previous build's rule"
    );

    host.adopt(rebuilt(&host.app));

    // The mechanism behind the first test: the *stored* sheet changed,
    // not merely the resolved style — so a later `set_palette`
    // re-cascade resolves against the new build's rules too.
    assert_eq!(
        host.stylesheet()[0].patch.background,
        Some(V2),
        "the host still holds the pre-swap rule; `adopt` replaced the application \
         without replacing the sheet it is styled by"
    );
}
