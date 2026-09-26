//! **An arrow belongs to the widget that uses it, through a `Host`** —
//! M44/T44.1 (`docs/plan-input-semantics.md` §2.1), the `Host` legs of
//! `crates/lumen-static/tests/arrow_ownership.rs`.
//!
//! The same shape — a `TextInput` holding `abc` beside a `Button`, End,
//! one Left — for an unsplit and a split application, each under
//! `Everything` and `Boundaries<256>`. The split leg is the one that
//! matters: since M29/T29.6 a split application redraws only what an
//! event *consumed*, so a widget that took the arrow without the pass
//! recording it paints last frame's caret under `Boundaries` while
//! `Everything` paints the new one (§0 rule 1). Each leg prints
//! `(focus, caret)` beside the expected pair, and the frame hashes and
//! damage beside each other.

use lumen_static::event::EventCtx;
use lumen_static::invalidate::{Boundaries, Everything, Invalidation};
use lumen_static::prelude::*;
use lumen_static::retain::{Damage, Incremental};
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{Event, Key};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
struct URow {
    #[widget_id("field")]
    field: TextInput,
    #[widget_id("ok")]
    ok: Button<Unsplit>,
}

#[derive(DeriveApp, Default)]
struct Unsplit {
    #[tree]
    ui: URow,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
struct SRow {
    #[widget_id("field")]
    field: TextInput,
    #[widget_id("ok")]
    ok: Button<Store>,
}

#[derive(Default)]
struct Store {
    clicks: u32,
}

#[derive(DeriveApp, Default)]
struct Split {
    #[tree]
    ui: SRow,
    #[state]
    store: Store,
}

fn sheet() -> Vec<Rule> {
    vec![
        Rule::new(Selector::Id("field"), Patch::new().width(Length::Fixed(160.0)).height(Length::Fixed(24.0))),
        Rule::new(Selector::Id("ok"), Patch::new().width(Length::Fixed(60.0)).height(Length::Fixed(24.0))),
    ]
}

fn host<A: lumen_static::event::App, I: Invalidation>(app: A) -> Host<A, Incremental, I> {
    let mut h: Host<A, Incremental, I> = Host::new(app, 320, 80);
    h.set_stylesheet(&sheet());
    h.pump();
    h.pump();
    h
}

fn index<A: lumen_static::event::App>(wid: &str) -> u32 {
    lumen_static::style::index_of_wid(<A::Tree as Widget>::SUB_TABLE, wid).expect(wid) as u32
}

/// Focus the field by clicking it, End, then one Left; returns what the
/// Left produced.
fn drive<A: lumen_static::event::App, I: Invalidation>(
    h: &mut Host<A, Incremental, I>,
    caret: impl Fn(&A) -> usize,
) -> (Option<u32>, usize, u64, Damage) {
    let i = index::<A>("field") as usize;
    let s = h.states[i];
    let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    h.post(Event::down(x, y));
    h.post(Event::up(x, y));
    h.pump();
    assert_eq!(h.input.focus, Some(i as u32), "setup: the click did not focus the field");
    h.post(Event::Key { key: Key::End, down: true });
    h.pump();
    assert_eq!(caret(&h.app), 3, "setup: End did not reach the field");
    h.post(Event::Key { key: Key::Left, down: true });
    h.pump();
    (h.input.focus, caret(&h.app), h.list.hash_from(0), h.damage())
}

fn check<A: lumen_static::event::App>(
    what: &str,
    e: (Option<u32>, usize, u64, Damage),
    b: (Option<u32>, usize, u64, Damage),
) {
    let field = index::<A>("field");
    println!(
        "{what}: Everything (focus, caret) = {:?}  Boundaries = {:?}  expected {:?}; hashes {:#018x} / {:#018x}; damage {:?}",
        (e.0, e.1),
        (b.0, b.1),
        (Some(field), 2),
        e.2,
        b.2,
        b.3
    );
    assert_eq!((b.0, b.1), (Some(field), 2), "{what}: the Left left the field under Boundaries");
    assert_eq!((e.0, e.1), (Some(field), 2), "{what}: the Left left the field under Everything");
    assert!(!matches!(b.3, Damage::None), "{what}: a caret that moved damaged nothing");
    assert_eq!(e.2, b.2, "{what}: the narrowed frame differs from the whole-tree frame (a stale caret)");
}

#[test]
#[ignore = "red until T44.1: R3-F1 (the arrow-group rule takes the key first)"]
fn an_unsplit_application_keeps_the_arrow_in_the_field() {
    let mut e: Host<Unsplit, Incremental, Everything> = host(Unsplit::default());
    let mut b: Host<Unsplit, Incremental, Boundaries<256>> = host(Unsplit::default());
    for h in [&mut e.app, &mut b.app] {
        h.ui.field.set_str("abc");
        h.ui.ok.label = "ok";
    }
    let re = drive(&mut e, |a: &Unsplit| a.ui.field.caret());
    let rb = drive(&mut b, |a: &Unsplit| a.ui.field.caret());
    check::<Unsplit>("unsplit", re, rb);
}

#[test]
#[ignore = "red until T44.1: R3-F1 (the arrow-group rule takes the key first)"]
fn a_split_application_keeps_the_arrow_in_the_field_and_redraws_the_caret() {
    let make = || {
        let mut s = Split::default();
        s.ui.field.set_str("abc");
        s.ui.ok.label = "ok";
        s.ui.ok.on_click = |s: &mut Store, _: &EventCtx| s.clicks += 1;
        s
    };
    let mut e: Host<Split, Incremental, Everything> = host(make());
    let mut b: Host<Split, Incremental, Boundaries<256>> = host(make());
    let re = drive(&mut e, |a: &Split| a.ui.field.caret());
    let rb = drive(&mut b, |a: &Split| a.ui.field.caret());
    println!("split: the Left's marks under Boundaries: {:?}", b.last_marks());
    assert!(!b.last_marks().all, "the Left marked everything, so this leg cannot see a missing mark");
    check::<Split>("split", re, rb);
}
