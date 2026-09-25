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
//!    was never marked. Since M41/T41.3 the handler reports the change
//!    the way `Repeat::bind`'s rustdoc asks — it bumps a version in its
//!    state, which a value `#[bind]` copies into `Repeat::generation` —
//!    and the binding marks exactly the list. The click lands and the
//!    `Everything` arm's rows change, so the case can fail; a click that
//!    changes no data still narrows to one range.
//! 3. **Rows that are containers**, after a *reported* change. A row was
//!    laid out with the frame's `Changed`, whose ranges are main-table
//!    indices, while the row's own indices are its arena slot's — so a
//!    derived row asked whether an unrelated main-table range was clean,
//!    and kept its old geometry even though the list was invalidated.
//!    Since T41.3 a marked repeater lays its rows out under
//!    `Changed::ALL`.
//! 4. **A generation bumped without a mark**, on a frame that reaches the
//!    repeater because its sibling was invalidated: the repeater itself
//!    is clean, and it is the moved `generation` that lays its container
//!    rows out again.
//! 5. and 6. **`List` and `Dyn`** have the same shape — content laid out
//!    in an arena whose indices start at zero — and the same fix,
//!    `Changed::arena_view`.

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
    /// The data's version: bumped by a handler that changes `DATA`.
    version: u64,
}

/// Split: a handler gets the `Store`, never the tree. The list's
/// `generation` is bound from the store — how a split application
/// reports a data change (M41/T41.3).
#[derive(DeriveApp, Default)]
struct Split {
    #[tree]
    ui: Ui,
    #[state]
    #[bind(ui.body.list.generation, version)]
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
        // The version is set the same way, per store — `DATA` is shared
        // by both arms' hosts, so it cannot say which arm has seen it —
        // and a second click therefore changes no data and reports none.
        DATA.with(|d| d.set(100));
        s.version = 1;
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
    // Two: the hover's mark on the button (T25.0 — the pointer arrived
    // with this click) and the binding's on the list.
    println!("click marked {m:?} (bound: not all, two ranges -- the hovered button and the list)");
    assert!(!m.all, "the click fell back to marking everything: the fix is not narrow");
    assert_eq!(m.ranges, 2, "the data change did not mark the list, or marked more than it");
    for f in 0..3 {
        assert!(same_frame(&format!("after click, frame {f}"), &e, &b), "frame {f}: the list is stale under Boundaries");
        e.pump();
        b.pump();
    }

    // Accept direction: a click that changes no data marks nothing. The
    // pointer is already on the button, and a split application's
    // handler writes only its state, which moved no binding.
    e.click(x, y);
    b.click(x, y);
    assert!(e.pump() && b.pump(), "setup: the second click was not handled");
    assert_eq!(b.app.store.n, 2, "setup: the second click's handler did not run");
    let m = b.last_marks();
    println!("second click marked {m:?} (bound: not all, no range)");
    assert!(!m.all && m.ranges == 0, "a click with no data change marked something: {m:?}");
    assert!(same_frame("after second click", &e, &b), "the second click's frame differs");
}

/// Case 3: rows that are containers keep their geometry after a
/// reported change.
#[test]
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
    println!("Everything's first row text: {wb} px wide before, {wa} px after (bound: wider)");
    // The geometry, not only the text: a row keeps its text-changed-but-
    // box-stale shape under *both* strategies if the repeater ignores its
    // own mark everywhere, and the frames then agree about a stale box
    // (M41/T41.3's injection C, which left this case green without it).
    assert!(wa > wb, "Everything's row did not widen, so this case cannot fail on geometry");
    assert!(same_frame("reported, container rows", &e, &b), "container rows kept last frame's geometry after Host::invalidate");
}

/// Case 4: the repeater is reached (its sibling was marked) but not
/// marked itself; the moved `generation` is what re-lays its rows.
#[test]
fn a_moved_generation_lays_container_rows_out_again() {
    DATA.with(|d| d.set(0));
    let mut e: Host<BoxedApp, Incremental, Everything> = boxed();
    let mut b: Host<BoxedApp> = boxed();
    let before = rows(&e.list);
    DATA.with(|d| d.set(7));
    e.app.ui.list.generation += 1;
    b.app.ui.list.generation += 1;
    // The sibling, not the list: the root is laid out, so the repeater
    // is reached with its own range clean.
    assert!(b.invalidate("head"), "the head id must exist");
    e.pump();
    b.pump();
    let after = rows(&e.list);
    assert_ne!(before.first(), after.first(), "Everything's rows did not change, so this case cannot fail");
    let (wb, wa) = (before[0].0[2] - before[0].0[0], after[0].0[2] - after[0].0[0]);
    println!("Everything's first row text: {wb} px wide before, {wa} px after (bound: wider)");
    assert!(wa > wb, "Everything's row did not widen, so this case cannot fail on geometry");
    let m = b.last_marks();
    assert!(!m.all && m.ranges == 1, "setup: expected only the head marked, got {m:?}");
    assert!(same_frame("generation moved, list unmarked", &e, &b), "container rows ignored a moved generation");
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct ListUi {
    #[widget_id("head")]
    head: Label<String>,
    #[widget_id("items")]
    items: lumen_static::hatch::List<Boxed>,
}

#[derive(DeriveApp, Default)]
struct ListApp {
    #[tree]
    ui: ListUi,
}

fn listed<I: Invalidation>() -> Host<ListApp, Incremental, I> {
    let mut a = ListApp::default();
    a.ui.head.set("head".into());
    a.ui.items = lumen_static::hatch::List::new((0..4).map(|_| Boxed::default()).collect());
    for (k, it) in a.ui.items.items.iter_mut().enumerate() {
        it.name.set(format!("row {k}"));
    }
    let mut h: Host<ListApp, Incremental, I> = Host::new(a, 360, 160);
    h.pump();
    h.pump();
    h
}

/// Case 5: `List` has `Repeat`'s index-space shape — its items are laid
/// out in an arena — so a reported write to an item must re-lay it.
#[test]
fn a_lists_container_items_are_laid_out_again_after_a_reported_change() {
    let mut e: Host<ListApp, Incremental, Everything> = listed();
    let mut b: Host<ListApp> = listed();
    let before = rows(&e.list);
    for h in [&mut e.app, &mut b.app] {
        h.ui.items.items[0].name.set("row 0 and a much longer tail".into());
    }
    assert!(b.invalidate("items"), "the items id must exist");
    e.pump();
    b.pump();
    let after = rows(&e.list);
    let (wb, wa) = (before[0].0[2] - before[0].0[0], after[0].0[2] - after[0].0[0]);
    println!("Everything's first item text: {wb} px wide before, {wa} px after (bound: wider)");
    assert!(wa > wb, "Everything's item did not widen, so this case cannot fail on geometry");
    assert!(same_frame("reported, list items", &e, &b), "a List's container items kept last frame's geometry after Host::invalidate");
}

thread_local! {
    /// The width of every `Grow` leaf.
    static GROW: Cell<f32> = const { Cell::new(10.0) };
}

/// A leaf whose width is whatever `GROW` says: the smallest thing whose
/// box changes with no field anyone could write through a `Dyn`.
#[derive(Default)]
struct Grow;

impl lumen_static::Widget for Grow {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [lumen_static::NodeDesc] =
        &[lumen_static::NodeDesc::new(0, lumen_static::NO_PARENT, 0, 0, 0)];
    const KIND: &'static str = "grow-probe";
    const SEEDS: &'static [lumen_static::style::Seed] =
        &[lumen_static::style::Seed::unlinted(Self::KIND, Self::type_defaults)];
}

impl lumen_static::widget::Node for Grow {
    fn layout(
        &mut self,
        _cx: &mut lumen_static::LayoutCx,
        limits: lumen_static::Constraints,
        style: &[lumen_static::Style],
        st: &mut [lumen_static::NodeState],
        i: usize,
    ) -> lumen_static::Size {
        let size = lumen_static::ops::resolve_self(limits, &style[i], lumen_static::Size { w: GROW.with(Cell::get), h: 10.0 });
        lumen_static::ops::record_painted(st, i, limits, size, &style[i])
    }

    fn paint(&self, st: &[lumen_static::NodeState], i: usize, out: &mut dyn lumen_static::display::Painter) {
        let r = lumen_static::ops::rect_of(&st[i]);
        out.rect(r, lumen_static::Color(0, 0, 0, 255));
    }

    lumen_static::leaf_common!(Grow, 0, nohit);
}

impl<S> lumen_static::event::Handlers<S> for Grow {
    fn handler(&self, _sel: lumen_static::event::NodeSel) -> Option<fn(&mut S, &lumen_static::EventCtx)> {
        None
    }
}

/// A derived container inside the island, so something there has a
/// boundary to skip at.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 2.0)]
struct Island {
    grow: Grow,
    tail: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct DynUi {
    #[widget_id("head")]
    head: Label<String>,
    /// Two more nodes ahead of the island, so its main-table range
    /// (4..5) does not overlap the island's own arena indices (0..3). At
    /// index 2 it did, and the case was green without the fix: the
    /// island's root asked about 0..3, the mark was 2..3, and the
    /// coincidence re-laid it.
    pad: Label,
    pad2: Label,
    #[widget_id("island")]
    island: lumen_static::hatch::Dyn<DynApp>,
}

#[derive(DeriveApp, Default)]
struct DynApp {
    #[tree]
    ui: DynUi,
}

fn islanded<I: Invalidation>() -> Host<DynApp, Incremental, I> {
    let mut a = DynApp::default();
    a.ui.head.set("head".into());
    a.ui.island = lumen_static::hatch::Dyn::new(Island { grow: Grow, tail: Label::new("tail") });
    let mut h: Host<DynApp, Incremental, I> = Host::new(a, 360, 160);
    h.pump();
    h.pump();
    h
}

/// Where the island's `tail` label is painted.
fn tail_x(list: &DisplayList) -> Option<f32> {
    list.cmds.iter().find_map(|c| match c {
        Cmd::Text { rect, text, .. } if list.str_of(*text) == "tail" => Some(rect[0]),
        _ => None,
    })
}

/// Case 6: a `Dyn` island has the same index-space shape.
#[test]
fn a_dyn_islands_containers_are_laid_out_again_after_a_reported_change() {
    GROW.with(|g| g.set(10.0));
    let mut e: Host<DynApp, Incremental, Everything> = islanded();
    let mut b: Host<DynApp> = islanded();
    let island = lumen_static::style::index_of_wid(<DynUi as Widget>::SUB_TABLE, "island").unwrap();
    assert!(island >= 3, "setup: the island at {island} overlaps its own arena's 0..3");
    let before = tail_x(&e.list);
    GROW.with(|g| g.set(90.0));
    assert!(b.invalidate("island"), "the island id must exist");
    e.pump();
    b.pump();
    let (xe, xb) = (tail_x(&e.list), tail_x(&b.list));
    println!("tail x: Everything {before:?} -> {xe:?} | Boundaries {xb:?} (bound: equal, and Everything moved)");
    assert_ne!(before, xe, "Everything's tail did not move, so this case cannot fail");
    assert_eq!(xe, xb, "a Dyn island's container kept last frame's geometry after Host::invalidate");
}
