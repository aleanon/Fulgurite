//! M26/T26.1: an enum's **active** variant must survive a frame in which
//! something else was marked.
//!
//! The enum derive hides every variant's slots before laying the active
//! one out — `hide_subtree(st, i + 1, limits, NODE_COUNT - 1)` — and
//! `hide_subtree` stamps `constraints = limits` as well as zeroing the
//! size. A derived container's `layout` then begins with
//! `can_skip() && is_clean(..) && st[i].constraints == limits` →
//! `return st[i].size`. So on a frame where the enum's own subtree is
//! clean but the enum is still reached, the active variant returns the
//! zero it was just handed, and the stage disappears.
//!
//! **Why the variant must be a container.** A leaf has no such guard and
//! recomputes whatever it is given, so an enum of leaves cannot exhibit
//! this at all — a test written that way would pass with the defect in
//! place. `pointer_invalidation.rs` makes the same point for the
//! sibling-container case.
//!
//! **Why `Boundaries`.** Under `Everything` nothing is ever clean, so the
//! guard never fires and every assertion here passes with the defect
//! present — which is exactly the shape M25/T25.0 had.

use fulgurite::invalidate::Boundaries;
use fulgurite_widgets::ops::Label;
use fulgurite::style::{Length, Patch, Rule, Selector};
use fulgurite::{App as DeriveApp, StaticTree, Widget};
use fulgurite_shell::Host;

/// A derived container, so it carries the relayout boundary the defect
/// needs; its size comes from the sheet rather than from content, so a
/// zero size means the skip and not an empty string.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Pane {
    text: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Spare {
    text: Label,
}

#[derive(StaticTree)]
#[allow(dead_code)] // `Spare` is never built: being the hidden one is the point.
enum Stage {
    Shown(Pane),
    Hidden(Spare),
}

impl Default for Stage {
    fn default() -> Self {
        Stage::Shown(Pane::default())
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("stage")]
    stage: Stage,
    #[widget_id("sibling")]
    sibling: Label,
}

#[derive(DeriveApp, Default)]
struct Screen {
    #[tree]
    ui: Ui,
}

const PANE_W: f32 = 40.0;
const PANE_H: f32 = 20.0;

fn sheet() -> Vec<Rule> {
    vec![Rule::new(
        Selector::Kind("Pane"),
        Patch::new().width(Length::Fixed(PANE_W)).height(Length::Fixed(PANE_H)),
    )]
}

/// The index of the active variant's container: the enum's own node, plus
/// one. `STARTS[0]` is 1 for every enum the derive emits.
fn pane_index() -> usize {
    fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "stage")
        .expect("the stage id exists")
        + 1
}

/// The sibling label's index, **by its id**.
///
/// Not by arithmetic off the enum: the enum reserves slots for every
/// variant, so "the node after the pane" is the pane's own child and the
/// node after *that* is the hidden variant. The first version of this
/// test read one of those, found a zero that never changes, and fired its
/// own setup assertion — which is the assertion doing its job.
fn sibling_index() -> usize {
    fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "sibling")
        .expect("the sibling id exists")
}

fn settled<I: fulgurite::invalidate::Invalidation>() -> Host<Screen, fulgurite::Incremental, I>
{
    let mut h: Host<Screen, fulgurite::Incremental, I> = Host::new(Screen::default(), 200, 100);
    h.app.ui.sibling.set("side");
    h.set_stylesheet(&sheet());
    h.pump();
    h.pump();
    h
}

/// The defect: mark a *sibling*, and the stage collapses.
#[test]
fn a_marked_sibling_does_not_collapse_the_active_variant() {
    let mut h: Host<Screen, fulgurite::Incremental, Boundaries<16>> = settled();
    let pane = pane_index();

    // Setup, asserted rather than assumed: the stage has the size the
    // sheet gives it, and the frame has settled so a subtree can be clean.
    assert_eq!(
        (h.states[pane].size.w, h.states[pane].size.h),
        (PANE_W, PANE_H),
        "setup: the active variant is laid out at the sheet's size"
    );
    assert!(h.changed().can_skip(), "setup: a still frame leaves something clean to skip");

    // Something else changes, and says so. The stage is not marked.
    h.app.ui.sibling.set("a considerably longer label");
    assert!(h.invalidate("sibling"), "setup: the sibling id exists");
    let sibling = sibling_index();
    let before = h.states[sibling].size.w;
    h.pump();

    // The frame did work — otherwise both sides agree about a frame in
    // which nothing happened, which is the vacuous shape `AGENT.md` names.
    assert!(
        h.states[sibling].size.w != before,
        "setup: the marked sibling did not relayout ({before} -> {}), so this frame proves nothing",
        h.states[sibling].size.w
    );

    println!("stage after a sibling's frame: {:?}", h.states[pane].size);
    assert_eq!(
        (h.states[pane].size.w, h.states[pane].size.h),
        (PANE_W, PANE_H),
        "the active variant collapsed: the enum hid it and the clean-skip returned the zero"
    );
}

/// The same session under `Everything`, where the guard never fires —
/// green with the defect present, and the reason this file is written
/// against `Boundaries`.
#[test]
fn the_same_session_under_everything_is_unaffected() {
    let mut h: Host<Screen, fulgurite::Incremental, fulgurite::invalidate::Everything> =
        settled();
    let pane = pane_index();
    h.app.ui.sibling.set("a considerably longer label");
    let _ = h.invalidate("sibling");
    h.pump();
    assert_eq!((h.states[pane].size.w, h.states[pane].size.h), (PANE_W, PANE_H));
}
