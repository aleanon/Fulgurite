//! **A `Repeat` whose data changed** — M41/T41.0, for R1-F4
//! (`docs/plan-boundaries-repair.md` §1.2, §2.3).
//!
//! `Repeat` binds its rows inside `layout`, and its rustdoc promised
//! that "a data change needs no invalidation call". Under the
//! `Boundaries` default a clean subtree's `layout` is skipped before the
//! repeater ever runs, so that promise holds only for a frame something
//! else marked. A bind is a plain `fn` and cannot capture, so data from
//! outside the application — a global — is the only way a row can show
//! changing data, and that is what these cases change.
//!
//! Three cases, each against `Everything` in one process, each printing
//! what `Boundaries` marked, the row text rects, and both list hashes:
//!
//! 1. **A global changed between pumps**, with nothing queued: once
//!    reported through [`Host::invalidate`] (equal), and once with
//!    nothing reported (stale — the contract, pinned rather than
//!    implied: an unreported write goes unseen under `Boundaries`).
//! 2. **The f4b case**: a split application whose click, in a sibling
//!    container, changes the data. M29/T29.6 narrowed a split
//!    application's consumed click to the consumed subtree, so the list
//!    is never marked. The click lands and the `Everything` arm's rows
//!    change, so the case can fail.
//! 3. **Rows that are containers**, after a *reported* change. A row is
//!    laid out with the frame's `Changed`, whose ranges are main-table
//!    indices, while the row's own indices are its arena slot's — so a
//!    derived row asks whether an unrelated main-table range is clean,
//!    and keeps its old geometry even though the list was invalidated.

use std::cell::Cell;

use lumen_static::event::EventCtx;
use lumen_static::invalidate::{Everything, Invalidation};
use lumen_static::prelude::*;
use lumen_static::repeat::Repeat;
use lumen_static::retain::Incremental;
use lumen_static::{Cmd, DisplayList};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

thread_local! {
    /// What every bind reads: the row index plus this.
    static DATA: Cell<usize> = const { Cell::new(0) };
}

fn bind_label(row: &mut Label<String>, i: usize) {
    lumen_static::bind::format_into(row, format_args!("row {}", i + DATA.with(Cell::get)));
}

/// A row that is a derived container, for case 3.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 2.0)]
struct Boxed {
    name: Label<String>,
}

fn bind_boxed(row: &mut Boxed, i: usize) {
    // Wider text for a larger offset, so a stale box is a different box.
    let d = DATA.with(Cell::get);
    let tail = if d == 0 { "" } else { " and a much longer tail" };
    lumen_static::bind::format_into(&mut row.name, format_args!("row {}{tail}", i + d));
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Side {
    #[widget_id("go")]
    go: Button<Store>,
}

/// The list inside a container of its own: a derived container checks
/// its own range at the top of `layout`, so this is the node a narrowed
/// frame skips. A `Repeat` directly under the root would be laid out
/// whenever the root was, and the case could not fail.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Body {
    #[widget_id("list")]
    list: Repeat<Label<String>>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct Ui {
    side: Side,
    body: Body,
}

#[derive(Default)]
struct Store {
    n: u32,
}

/// Split: a handler gets the `Store`, never the tree.
#[derive(DeriveApp, Default)]
struct Split {
    #[tree]
    ui: Ui,
    #[state]
    store: Store,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct BoxedUi {
    #[widget_id("head")]
    head: Label<String>,
    #[widget_id("list")]
    list: Repeat<Boxed>,
}

#[derive(DeriveApp, Default)]
struct BoxedApp {
    #[tree]
    ui: BoxedUi,
}

/// Every `row …` text run: its rect and its string.
fn rows(list: &DisplayList) -> Vec<([f32; 4], String)> {
    list.cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::Text { rect, text, .. } if list.str_of(*text).starts_with("row ") => {
                Some((*rect, list.str_of(*text).to_owned()))
            }
            _ => None,
        })
        .collect()
}

fn split<I: Invalidation>() -> Host<Split, Incremental, I> {
    let mut a = Split::default();
    a.ui.side.go.label = "go";
    a.ui.side.go.on_click = |s: &mut Store, _: &EventCtx| {
        s.n += 1;
        // Set, not bumped: both arms click, and both must see one change.
        DATA.with(|d| d.set(100));
    };
    a.ui.body.list = Repeat::new(50, 20.0, bind_label);
    let mut h: Host<Split, Incremental, I> = Host::new(a, 240, 160);
    h.pump();
    h.pump();
    h
}

fn boxed<I: Invalidation>() -> Host<BoxedApp, Incremental, I> {
    let mut a = BoxedApp::default();
    a.ui.head.set("head".into());
    a.ui.list = Repeat::new(50, 20.0, bind_boxed);
    let mut h: Host<BoxedApp, Incremental, I> = Host::new(a, 360, 160);
    h.pump();
    h.pump();
    h
}

/// Compare one frame of the two arms and print it. Returns whether the
/// lists are equal.
fn same_frame<A: App, I: Invalidation>(
    what: &str,
    e: &Host<A, Incremental, Everything>,
    b: &Host<A, Incremental, I>,
) -> bool {
    let (re, rb) = (rows(&e.list), rows(&b.list));
    let (he, hb) = (e.list.hash_from(0), b.list.hash_from(0));
    println!(
        "{what}: marks {:?} | Everything {he:#x} first row {:?} | Boundaries {hb:#x} first row {:?} | bound: equal -> {}",
        b.last_marks(),
        re.first(),
        rb.first(),
        he == hb
    );
    he == hb
}

/// Case 1: a global changed with nothing queued. Reported, the frame
/// matches; unreported, it is stale, which is the contract.
#[test]
fn a_reported_data_change_is_drawn_and_an_unreported_one_is_not() {
    DATA.with(|d| d.set(0));
    let mut e: Host<Split, Incremental, Everything> = split();
    let mut b: Host<Split> = split();
    let before = rows(&e.list);
    assert!(same_frame("settled", &e, &b), "setup: the two arms start apart");

    // Unreported.
    DATA.with(|d| d.set(100));
    assert!(!e.pump() && !b.pump(), "setup: nothing was queued, so nothing is handled");
    let after = rows(&e.list);
    assert_ne!(before.first(), after.first(), "Everything's rows did not change, so this case cannot fail");
    assert!(!same_frame("unreported", &e, &b), "an unreported data change was drawn under Boundaries — the contract changed");

    // Reported.
    assert!(b.invalidate("list"), "the list id must exist");
    b.pump();
    e.pump();
    assert!(same_frame("reported", &e, &b), "a reported data change was not drawn");
}

/// Case 2 (f4b): a click in a sibling container changes the data.
#[test]
#[ignore = "red until T41.3: R1-F4, a split application's click does not mark a Repeat whose data it changed"]
fn a_click_that_changes_a_repeaters_data_draws_the_same_frame() {
    DATA.with(|d| d.set(0));
    let mut e: Host<Split, Incremental, Everything> = split();
    let mut b: Host<Split> = split();
    let before = rows(&e.list);
    let go = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "go").unwrap();
    let s = b.states[go];
    let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    e.click(x, y);
    b.click(x, y);
    let (he, hb) = (e.pump(), b.pump());
    assert!(he && hb, "setup: the click was not handled ({he}/{hb})");
    assert_eq!((e.app.store.n, b.app.store.n), (1, 1), "setup: the handler did not run");
    assert_ne!(before.first(), rows(&e.list).first(), "Everything's rows did not change, so this case cannot fail");
    let m = b.last_marks();
    println!("click marked {m:?} (a narrowed click: not all, one range)");
    assert!(!m.all, "setup: the click fell back to marking everything, so it cannot show the defect");
    for f in 0..3 {
        assert!(same_frame(&format!("after click, frame {f}"), &e, &b), "frame {f}: the list is stale under Boundaries");
        e.pump();
        b.pump();
    }
}

/// Case 3: rows that are containers keep their geometry after a
/// reported change.
#[test]
#[ignore = "red until T41.3: R1-F4, a Repeat's container rows are skipped against main-table ranges"]
fn container_rows_are_laid_out_again_after_a_reported_change() {
    DATA.with(|d| d.set(0));
    let mut e: Host<BoxedApp, Incremental, Everything> = boxed();
    let mut b: Host<BoxedApp> = boxed();
    let before = rows(&e.list);
    DATA.with(|d| d.set(7));
    assert!(b.invalidate("list"), "the list id must exist");
    e.pump();
    b.pump();
    let after = rows(&e.list);
    assert_ne!(before.first(), after.first(), "Everything's rows did not change, so this case cannot fail");
    let (wb, wa) = (before[0].0[2] - before[0].0[0], after[0].0[2] - after[0].0[0]);
    println!("Everything's first row text: {wb} px wide before, {wa} px after");
    assert!(same_frame("reported, container rows", &e, &b), "container rows kept last frame's geometry after Host::invalidate");
}
