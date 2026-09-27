//! M27/T27.7 — `Modal`, `Sheet`, `Drawer`.
//!
//! Five claims, and the first two are what "modal" means:
//!
//! 1. **A shut veil is absent**, not merely unpainted — a panel that
//!    only skipped `paint` would still hit-test at full window size and
//!    swallow every click.
//! 2. **An open one claims everything it covers**, so a press on the
//!    scrim cannot reach the content behind it.
//! 3. Its panel still works — a wrapper that does not route
//!    `Handlers<S>` leaves every control inside it dead, which is what
//!    `Dyn` shipped as.
//! 4. The scrim paints **before** the panel and both in the overlay
//!    phase, or the dim lands under what it is meant to dim.
//! 5. Dismissal is a *request*: the widget never closes itself, because
//!    an application that must confirm before discarding cannot undo a
//!    close that already happened.

// on_event only: these test panel geometry and the dismissal request.
// Delivery -- Shift+Tab in the trap, the scrim press -- is crates/lumen-
// static-shell/tests/veil_b41.rs, and scripts/drive/widgets/veil.py live.
// (M57/D2)

use lumen_static::display::{Cmd, DisplayList};
use lumen_static::event::{Event, EventCtx, Handlers, Key, NodeSel};
use lumen_static_widgets::ops::Button;
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, NodeState, Point};
use lumen_static_widgets::{Drawer, Modal, Sheet};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 300.0 };

#[derive(Default)]
struct App {
    clicks: u32,
}

fn bump(a: &mut App, _: &EventCtx) {
    a.clicks += 1;
}

type Panel = Button<App, &'static str>;

fn built<const K: u8>(open: bool) -> lumen_static_widgets::Veil<Panel, K> {
    let mut b = Panel::default();
    b.label = "OK";
    b.on_click = bump;
    let mut v = lumen_static_widgets::Veil::<Panel, K>::new(b);
    if open {
        v.open();
    }
    v
}

fn lay<const K: u8>(v: &mut lumen_static_widgets::Veil<Panel, K>) -> Vec<NodeState> {
    let styles = vec![
        <lumen_static_widgets::Veil<Panel, K> as Widget>::type_defaults();
        <lumen_static_widgets::Veil<Panel, K> as Widget>::NODE_COUNT
    ];
    let mut st =
        vec![NodeState::default(); <lumen_static_widgets::Veil<Panel, K> as Widget>::NODE_COUNT];
    let mut m = lumen_static::cx::BlockMetrics;
    {
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(v, &mut cx, VIEW, &styles, &mut st, 0);
    }
    Node::place(v, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);
    st
}

/// (1) Shut is absent.
#[test]
fn a_shut_veil_is_absent_not_merely_unpainted() {
    let mut v = built::<{ lumen_static_widgets::MODAL }>(false);
    let st = lay(&mut v);
    assert_eq!(st[0].size.w, 0.0, "a shut veil still occupies {}px", st[0].size.w);
    assert!(
        Node::hit(&v, &st, 0, 10.0, 10.0).is_none(),
        "a shut veil still answers a hit — it would swallow every click in the window"
    );
    let mut list = DisplayList::new();
    Node::paint(&v, &st, 0, &mut list);
    assert!(list.cmds.is_empty(), "a shut veil painted {} commands", list.cmds.len());
}

/// (2) An open one claims everything it covers.
#[test]
fn an_open_veil_claims_the_whole_window() {
    let mut v = built::<{ lumen_static_widgets::MODAL }>(true);
    let st = lay(&mut v);
    assert_eq!(st[0].size.w, VIEW.max_w, "an open veil is not the window's width");
    // A corner is scrim, never panel — it must still be claimed.
    let hit = Node::hit(&v, &st, 0, 2.0, 2.0).expect("the scrim did not claim a corner press");
    assert_eq!(hit.node, 0, "a press on the scrim reached node {} instead of the veil", hit.node);
}

/// (3) What is *on* the veil still works.
#[test]
fn the_panel_on_a_veil_still_takes_its_clicks() {
    let mut v = built::<{ lumen_static_widgets::MODAL }>(true);
    let st = lay(&mut v);
    let p = lumen_static::ops::rect_of(&st[1]);
    assert!(p[2] > p[0], "setup: the panel has no width");
    let hit = Node::hit(&v, &st, 0, (p[0] + p[2]) / 2.0, (p[1] + p[3]) / 2.0)
        .expect("nothing under the panel's centre");
    assert_eq!(hit.node, 1, "the scrim took a press on the panel (node {})", hit.node);
    assert!(
        Handlers::<App>::handler(&v, NodeSel { i: 1, inner: None }).is_some(),
        "the veil does not route to the control on it — the shape `Dyn` shipped as"
    );
    assert!(
        Handlers::<App>::handler(&v, NodeSel { i: 0, inner: None }).is_none(),
        "the veil answered a handler for its own node"
    );
}

/// (4) Scrim first, panel second, and both deferred together.
#[test]
fn the_scrim_paints_before_the_panel() {
    let mut v = built::<{ lumen_static_widgets::MODAL }>(true);
    let st = lay(&mut v);
    let mut list = DisplayList::new();
    Node::paint(&v, &st, 0, &mut list);
    let first = list.cmds.first().expect("an open veil painted nothing");
    let full = lumen_static::ops::rect_of(&st[0]);
    match first {
        Cmd::Rect { rect, color } => {
            assert_eq!(*rect, full, "the first command is not the full-window scrim");
            assert!(color.3 > 0, "the scrim is transparent — it dims nothing");
        }
        other => panic!("the first command is {other:?}, not the scrim"),
    }
    assert!(list.cmds.len() > 1, "the panel was not painted after the scrim");
}

/// (5) Dismissal is a request, taken once.
#[test]
fn a_scrim_press_requests_dismissal_but_does_not_close() {
    let mut v = built::<{ lumen_static_widgets::MODAL }>(true);
    lay(&mut v);
    let ev = Event::PointerDown {
        x: 0.0,
        y: 0.0,
        button: lumen_static::PointerButton::Left,
        clicks: 1,
    };
    assert!(v.on_event(&ev, &EventCtx::for_event(0, ev)), "the scrim did not consume the press");
    assert!(v.open, "the widget closed itself — an app that must confirm cannot undo that");
    assert!(v.take_dismissed(), "no dismissal was requested");
    assert!(!v.take_dismissed(), "the same request was served twice");
}

#[test]
fn escape_requests_dismissal_too() {
    let mut v = built::<{ lumen_static_widgets::MODAL }>(true);
    lay(&mut v);
    let ev = Event::Key { key: Key::Escape, down: true };
    assert!(v.on_event(&ev, &EventCtx::for_event(0, ev)));
    assert!(v.take_dismissed(), "Escape did not request dismissal");
}

/// A shut veil takes nothing at all, or it would eat the application's
/// keyboard while invisible.
#[test]
fn a_shut_veil_consumes_nothing() {
    let mut v = built::<{ lumen_static_widgets::MODAL }>(false);
    let ev = Event::Key { key: Key::Escape, down: true };
    assert!(!v.on_event(&ev, &EventCtx::for_event(0, ev)), "a shut veil ate a keystroke");
}

/// The three kinds put the panel in three places — which is the whole
/// difference between them, and the thing a shared implementation can
/// get wrong.
#[test]
fn the_three_kinds_place_the_panel_differently() {
    let mut m = built::<{ lumen_static_widgets::MODAL }>(true);
    let mut s = built::<{ lumen_static_widgets::SHEET }>(true);
    let mut d = built::<{ lumen_static_widgets::DRAWER }>(true);
    let (mst, sst, dst) = (lay(&mut m), lay(&mut s), lay(&mut d));
    let (mp, sp, dp) = (
        lumen_static::ops::rect_of(&mst[1]),
        lumen_static::ops::rect_of(&sst[1]),
        lumen_static::ops::rect_of(&dst[1]),
    );
    // A sheet is at the bottom and spans the width.
    assert!(
        (sp[3] - VIEW.max_h).abs() < 0.01 && (sp[2] - sp[0] - VIEW.max_w).abs() < 0.01,
        "a sheet is {sp:?} in a {}x{} window — it should span the bottom",
        VIEW.max_w, VIEW.max_h
    );
    // A drawer is at the left and spans the height.
    assert!(
        dp[0] < 0.01 && (dp[3] - dp[1] - VIEW.max_h).abs() < 0.01,
        "a drawer is {dp:?} — it should run down the left"
    );
    // A modal is centred, so it touches no edge.
    assert!(
        mp[0] > 0.01 && mp[2] < VIEW.max_w - 0.01,
        "a modal is {mp:?} — it should be centred, not against an edge"
    );
    assert_eq!(<Modal<Panel> as Widget>::KIND, "modal");
    assert_eq!(<Sheet<Panel> as Widget>::KIND, "sheet");
    assert_eq!(<Drawer<Panel> as Widget>::KIND, "drawer");
}
