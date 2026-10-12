//! **Which node a binding writes** — M29/T29.2
//! (`docs/plan-damage-precision.md` §2.3).
//!
//! `bind::node_of` answers by address. The oracle here is **not** an
//! address: it is the table's own author ids, which the derive writes and
//! nothing about addresses can move. So an injection into the address
//! walk cannot move the oracle with it — the trap §2.3 names for an
//! oracle that recomputes the same sum.

use fulgurite::bind::node_of;
use fulgurite::prelude::*;
use fulgurite::widget::Widget;
use fulgurite_widgets::prelude::*;
use fulgurite_widgets::select::Select;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Top {
    #[widget_id("formula")]
    formula: Label<String>,
    #[widget_id("status")]
    status: Label<String>,
}

/// **A single-field struct**: its only child sits at its own address,
/// and may have the same size — a parent and child that share an address
/// and are *not* the same node, which a naive "same address, skip" would
/// get wrong.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Solo {
    #[widget_id("solo")]
    only: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct PaneA {
    #[widget_id("a")]
    a: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct PaneB {
    #[widget_id("b")]
    b: Label<String>,
}

/// **An enum whose inactive variant's indices come after the target.**
/// Those indices resolve to the enum itself, which *does* contain the
/// target's address — so without the degenerate-index skip the deepest
/// "containing" node would be one of `B`'s, not `a`.
#[derive(StaticTree)]
#[allow(dead_code)]
enum Pane {
    A(PaneA),
    B(PaneB),
}
impl Default for Pane {
    fn default() -> Self {
        Pane::A(PaneA::default())
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    top: Top,
    solo: Solo,
    pane: Pane,
    #[widget_id("pick")]
    pick: Select<()>,
}

fn id(wid: &str) -> usize {
    <Ui as Widget>::SUB_TABLE.iter().position(|d| d.wid == wid).expect("no such id")
}

fn at<T>(r: &T) -> *const u8 {
    r as *const T as *const u8
}

#[test]
fn a_binding_names_the_node_its_target_lives_in() {
    let ui = Ui::default();
    let Pane::A(pa) = &ui.pane else { unreachable!() };
    let cases: [(&str, *const u8, usize); 5] = [
        ("ui.top.formula", at(&ui.top.formula), id("formula")),
        ("ui.top.status", at(&ui.top.status), id("status")),
        ("ui.solo.only (single-field parent)", at(&ui.solo.only), id("solo")),
        ("the active variant's label", at(&pa.a), id("a")),
        // A field *inside* a widget: the popup's `open`. Its owner is the
        // popup node, the node the select stamps `select-popup` on.
        ("ui.pick.menu.open (a field in a widget)", at(&ui.pick.menu.open), id("select-popup")),
    ];
    for (path, ptr, want) in cases {
        let got = node_of(&ui, ptr);
        println!("{path:<42} node_of {got:?}  id says {want}");
        assert_eq!(got, Some(want), "{path}: the address walk named the wrong node");
    }
}

#[test]
fn an_address_outside_the_tree_names_nothing() {
    let ui = Ui::default();
    let elsewhere = 7u64;
    assert_eq!(node_of(&ui, at(&elsewhere)), None, "a foreign address must not name a node");
}

/// **`BindSite` caches every answer, index 0 and "nowhere" included** —
/// M52/T52.3. It stores the index plus two so that a new site is all
/// zero (`.bss`, not `.data`); the offset must round-trip for the root's
/// index 0, for a large index, and for `None`, and `find` must run once.
#[test]
fn a_bind_site_caches_zero_nowhere_and_a_large_index_once() {
    use fulgurite::bind::BindSite;
    for want in [Some(0usize), Some(1), Some(2), Some(usize::MAX - 2), None] {
        let site = BindSite::new();
        let mut calls = 0;
        let first = site.get(|| {
            calls += 1;
            want
        });
        let second = site.get(|| {
            calls += 1;
            Some(12345)
        });
        println!("want {want:?}: first {first:?}, cached {second:?}, find ran {calls}x");
        assert_eq!(first, want, "the first answer is find's");
        assert_eq!(second, want, "the cached answer is the first one, not find's second");
        assert_eq!(calls, 1, "find ran once");
    }
}

/// A tree with an `Option` — `None` and `Some` — and an enum parked on
/// its *second* variant, for the equivalence sweep below.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Wide {
    ui: Ui,
    #[widget_id("maybe")]
    maybe: Option<Solo>,
    #[widget_id("sure")]
    sure: Option<Solo>,
    other: Pane,
}

/// **The static walk answers exactly what the vtable walk answered** —
/// M52/T52.5 (`docs/plan-embedded-footprint.md` §2.3, F-NEW-2).
///
/// `node_of` now asks each type's `Widget::bind_walk`: derived trees walk
/// their fields on concrete types, leaves answer from their own address,
/// and hand-written containers (`Select`'s popup, `Option`) fall back to
/// the vtable walk over their children. `node_of_by_vtable` is the old
/// walk, kept as this oracle. Every byte from 16 before the tree to 16
/// past it is asked of both — so an enum's inactive variant and a `None`,
/// which resolve to their parent, are covered along with every field —
/// and the first disagreement is printed with its offset.
#[test]
fn the_static_walk_agrees_with_the_vtable_walk_at_every_byte() {
    use fulgurite::bind::node_of_by_vtable;
    let w = Wide { sure: Some(Solo::default()), other: Pane::B(PaneB::default()), ..Default::default() };
    let base = at(&w) as usize;
    let size = core::mem::size_of::<Wide>();
    let (mut asked, mut named, mut first) = (0usize, 0usize, None);
    for off in -16isize..(size as isize + 16) {
        let p = (base as isize + off) as usize as *const u8;
        let (got, want) = (node_of(&w, p), node_of_by_vtable(&w, p));
        asked += 1;
        named += got.is_some() as usize;
        if got != want && first.is_none() {
            first = Some((off, got, want));
        }
    }
    let distinct: std::collections::BTreeSet<_> =
        (0..size).filter_map(|o| node_of(&w, (base + o) as *const u8)).collect();
    println!(
        "asked {asked} addresses over a {size} B tree of {} nodes: {named} named a node, {} distinct nodes; first disagreement {first:?}",
        <Wide as Widget>::NODE_COUNT,
        distinct.len()
    );
    assert_eq!(first, None, "(offset, static walk, vtable walk): the two walks disagree");
    // The sweep must reach the fallback and the enum: a node inside the
    // `Some` and the second variant's label are both named.
    let Pane::B(pb) = &w.other else { unreachable!() };
    let sure = w.sure.as_ref().unwrap();
    assert!(node_of(&w, at(&sure.only)).is_some_and(|i| i > 0), "the Some's label is named");
    assert!(node_of(&w, at(&pb.b)).is_some(), "the active second variant's label is named");
    assert!(distinct.len() > 10, "the sweep named only {} distinct nodes", distinct.len());
}
