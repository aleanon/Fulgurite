//! **The paths that stay conservative, asserted** — M29/T29.4
//! (`docs/plan-damage-precision.md`).
//!
//! T29.3 narrowed a binding's mark. These are the paths it deliberately
//! does *not* narrow, each asserted with the quantity a frame holds —
//! `Host::last_marks` — so a later reader cannot mistake the partial fix
//! for a general one. Both directions: a narrowed path reads exactly one
//! range, a conservative one reads `all`.
//!
//! One path the plan listed as conservative **is not any more**: a path
//! through a hand-written container. T29.2's address walk crosses one,
//! so it narrows, and is asserted here as narrowing — with the frame
//! checked against `Everything`, since narrowing is only a win if right.

use fulgurite::event::{App, EventCtx};
use fulgurite::invalidate::{Boundaries, Everything, Invalidation, Marks};
use fulgurite::prelude::*;
use fulgurite::retain::Incremental;
use fulgurite::spawn::{Descriptor, Immediate, JobFn};
use fulgurite_shell::Host;
use fulgurite_widgets::prelude::*;
use fulgurite_widgets::Glass;

const ALL: Marks = Marks { all: true, ranges: 0 };

#[derive(StaticTree, Default)]
#[widget(column, spacing = 2.0)]
struct Ui {
    top: Top,
    #[widget_id("glass")]
    glass: Glass<Label<String>>,
    #[widget_id("go")]
    go: Button<Knobs>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 2.0)]
struct Top {
    #[widget_id("formula")]
    formula: Label<String>,
    #[widget_id("pill")]
    pill: Label<String>,
}

#[derive(Default)]
struct Knobs {
    n: i64,
    clicks: u32,
}

fn set_pill(dst: &mut Label<String>, src: &Knobs) {
    fulgurite::bind::set_if_changed(dst, if src.n < 0 { "neg" } else { "pos" });
}

/// Every conservative path in one application, each switchable.
#[derive(DeriveApp, Default)]
#[app(after_update = hook)]
struct Paths {
    #[tree]
    ui: Ui,
    #[state]
    #[bind(ui.top.formula, n, "= {}")]
    #[bind(ui.glass.content, n, "{} in glass")]
    k: Knobs,
    /// When set, the `after_update` hook reports a change.
    hook_fires: bool,
}

fn hook(p: &mut Paths) -> bool {
    core::mem::take(&mut p.hook_fires)
}

fn build() -> Paths {
    let mut p = Paths::default();
    p.ui.go.on_click = |k: &mut Knobs, _: &EventCtx| k.clicks += 1;
    p
}

fn host<I: Invalidation>() -> Host<Paths, Incremental, I> {
    let mut h: Host<Paths, Incremental, I> = Host::new(build(), 300, 300);
    h.pump();
    h.pump();
    h
}

#[test]
fn a_bound_write_narrows_even_through_a_hand_written_container() {
    let mut e: Host<Paths, Incremental, Everything> = host();
    let mut b: Host<Paths, Incremental, Boundaries<16>> = host();
    e.app.k.n = 1_234_567;
    b.app.k.n = 1_234_567;
    e.pump();
    b.pump();
    println!("two bindings wrote, one through Glass<W>: {:?}", b.last_marks());
    // Two bindings wrote — one on the table, one inside `Glass<W>` —
    // and each is a range.
    assert_eq!(b.last_marks(), Marks { all: false, ranges: 2 }, "a bound write did not narrow");
    assert_eq!(e.list.hash_from(0), b.list.hash_from(0), "the narrowed frame is wrong");
    assert!(b.last_arenas_owed(), "path 6: a bound write must still owe the arenas a cascade");
}

/// **Its own application**, because a `with =` binding reports a write on
/// *every* update — the hook returns nothing, so the derive cannot know
/// it wrote nothing — and so an application that has one marks
/// everything on every pump. The first version of this file put it in
/// `Paths`, and every other path read `all` because of it.
#[derive(DeriveApp, Default)]
struct WithApp {
    #[tree]
    ui: Top,
    #[bind(ui.pill, with = set_pill)]
    k: Knobs,
}

#[test]
fn a_with_hook_marks_everything() {
    let mut b: Host<WithApp, Incremental, Boundaries<16>> = Host::new(WithApp::default(), 200, 50);
    b.pump();
    b.app.k.n = -5;
    b.pump();
    println!("with =: {:?}", b.last_marks());
    assert_eq!(b.last_marks(), ALL, "path 1: a `with =` hook still marks everything");
}

#[test]
fn an_after_update_that_changed_something_marks_everything() {
    let mut b: Host<Paths, Incremental, Boundaries<16>> = host();
    b.app.hook_fires = true;
    b.pump();
    assert_eq!(b.last_marks(), ALL, "path 2: `after_update` still marks everything");
    b.pump();
    assert_eq!(b.last_marks(), Marks::default(), "and a hook that changed nothing marks nothing");
}

/// **Path 4, rescoped by M29/T29.6**: a handled event marks everything
/// only for an application whose handlers can reach the tree. A split one
/// (`#[state]`) narrows to the nodes the event consumed plus what its
/// bindings wrote.
#[test]
fn a_handled_event_narrows_only_for_a_split_application() {
    let mut b: Host<Paths, Incremental, Boundaries<16>> = host();
    let i = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "go").unwrap();
    let s = b.states[i];
    b.click(s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    b.pump();
    assert_eq!(b.app.k.clicks, 1, "setup: the click did not land");
    println!("split, handled: {:?}", b.last_marks());
    assert!(!b.last_marks().all, "a split application's click still marked everything");

    let mut u: Host<Unsplit, Incremental, Boundaries<16>> = Host::new(unsplit(), 200, 60);
    u.pump();
    let i = fulgurite::style::index_of_wid(<UUi as Widget>::SUB_TABLE, "go").unwrap();
    let s = u.states[i];
    u.click(s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    u.pump();
    assert_eq!(u.app.clicks, 1, "setup: the unsplit click did not land");
    assert_eq!(u.last_marks(), ALL, "path 4: an unsplit application's handled event marks everything");
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct UUi {
    #[widget_id("go")]
    go: Button<Unsplit>,
}

/// No `#[state]`: its handler receives the whole application, tree
/// included.
#[derive(DeriveApp, Default)]
struct Unsplit {
    #[tree]
    ui: UUi,
    clicks: u32,
}

fn unsplit() -> Unsplit {
    let mut u = Unsplit::default();
    u.ui.go.on_click = |a: &mut Unsplit, _: &EventCtx| a.clicks += 1;
    u
}

// ---- path 5: an applied message --------------------------------------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct MUi {
    a: Fixed,
}

#[derive(Default)]
struct Msgs {
    ui: MUi,
    got: u64,
}

fn echo(d: Descriptor, tx: &fulgurite::inbox::Sender<u64>) {
    let _ = tx.send(d.payload);
}

impl App for Msgs {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self {
        self
    }
    type Tree = MUi;
    type Msg = u64;
    fn tree(&self) -> &MUi {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut MUi {
        &mut self.ui
    }
    fn apply(this: &mut Self, m: u64) -> bool {
        this.got = m;
        true
    }
    fn job_for(_: Descriptor) -> Option<JobFn<u64>> {
        Some(echo)
    }
}

#[test]
fn an_applied_message_marks_everything() {
    let mut h: Host<Msgs, Incremental, Boundaries<16>> = Host::new(Msgs::default(), 100, 50);
    h.set_spawner(Box::new(Immediate));
    h.pump();
    assert!(h.spawn(fulgurite::spawn::Job {
        desc: Descriptor::new(1, 9),
        job: echo,
        on_fail: None
    }));
    h.pump();
    assert_eq!(h.app.got, 9, "setup: the message did not land");
    assert_eq!(h.last_marks(), ALL, "path 5: an applied message still marks everything");
}
