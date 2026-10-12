//! **A release activates only what its press began in** — M57/A9
//! (`docs/plan-widget-fixes.md`).
//!
//! Pass 2 ran the handler of whatever a `PointerUp` landed on, so
//! pressing one button and letting go over another pressed the second,
//! and a press on the background that slid onto a button was a click
//! (`docs/widget-audit.md` X7). Every toolkit cancels both.

use fulgurite::{App as DeriveApp, Event, EventCtx, PointerButton, StaticTree};
use fulgurite_shell::Host;
use fulgurite_widgets::ops::Button;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 40.0)]
struct Ui {
    #[widget_id("a")]
    a: Button<App>,
    #[widget_id("b")]
    b: Button<App>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
    a: u32,
    b: u32,
}

fn host() -> Host<App> {
    let mut app = App::default();
    app.ui.a.label = "Alpha";
    app.ui.a.on_click = |s: &mut App, _: &EventCtx| s.a += 1;
    app.ui.b.label = "Beta";
    app.ui.b.on_click = |s: &mut App, _: &EventCtx| s.b += 1;
    let mut h = Host::new(app, 400, 200);
    h.pump();
    h
}

fn centre(h: &Host<App>, wid: &str) -> (f32, f32) {
    let i = fulgurite::style::index_of_wid(<Ui as fulgurite::Widget>::SUB_TABLE, wid).unwrap();
    let s = h.states[i];
    (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
}

/// Press at `from`, move through `via`, release at `to`; whether the
/// release was handled.
fn gesture(h: &mut Host<App>, from: (f32, f32), via: &[(f32, f32)], to: (f32, f32)) -> bool {
    h.post(Event::PointerDown { x: from.0, y: from.1, button: PointerButton::Left, clicks: 1 });
    h.pump();
    for &(x, y) in via {
        h.post(Event::PointerMove { x, y });
        h.pump();
    }
    h.post(Event::PointerUp { x: to.0, y: to.1, button: PointerButton::Left });
    h.pump()
}

#[test]
fn pressing_one_button_and_releasing_on_another_presses_neither() {
    let mut h = host();
    let (a, b) = (centre(&h, "a"), centre(&h, "b"));
    let handled = gesture(&mut h, a, &[b], b);
    println!("press A, release B: handled={handled}, a={} b={}", h.app.a, h.app.b);
    assert_eq!((h.app.a, h.app.b), (0, 0), "a press on A released on B activated something");
    assert!(!handled, "the release was reported handled");
}

#[test]
fn a_press_on_the_background_released_on_a_button_is_not_a_click() {
    let mut h = host();
    let b = centre(&h, "b");
    gesture(&mut h, (390.0, 190.0), &[b], b);
    assert_eq!(h.app.b, 0, "a press that began on the background clicked B");
}

/// The other direction: an ordinary click, and a drag that leaves and
/// comes back, both still activate.
#[test]
fn a_click_and_a_drag_back_onto_the_button_still_activate() {
    let mut h = host();
    let (a, b) = (centre(&h, "a"), centre(&h, "b"));
    gesture(&mut h, a, &[], a);
    assert_eq!(h.app.a, 1, "a plain click on A did not activate it");
    gesture(&mut h, a, &[b, a], a);
    assert_eq!(h.app.a, 2, "a drag off A and back did not activate it");
}
