//! **A control's words are part of it** — M57/B11, B12
//! (`docs/plan-widget-fixes.md`).
//!
//! The showcase paired each toggle with a `Label`, and clicking "I agree"
//! did nothing; and a toggle that was *on* showed no hover and no press,
//! because its body is `color` and only `background` was shaded
//! (`docs/widget-audit.md`, CheckBox, Switch). Through a `Host`, because
//! both are about which node the pass hovers and presses.

use lumen_static::{App as DeriveApp, Event, EventCtx, StaticTree, Widget};
use lumen_static_shell::Host;
use lumen_static_widgets::{CheckBox, Labelled};

#[derive(StaticTree, Default)]
#[widget(row, spacing = 30.0)]
struct Ui {
    #[widget_id("agree")]
    agree: Labelled<CheckBox<App>>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
    flips: u32,
}

fn flip(a: &mut App, _: &EventCtx) {
    a.flips += 1;
}

fn host() -> Host<App> {
    let mut app = App::default();
    let t = CheckBox::<App> { on_toggle: flip, ..Default::default() };
    app.ui.agree = Labelled::new(t, "I agree to the terms");
    let mut h = Host::new(app, 500, 100);
    h.pump();
    h
}

fn at(id: &str) -> usize {
    lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, id).unwrap()
}

#[test]
fn a_click_on_the_words_is_a_click_on_the_control() {
    let mut h = host();
    let (w, c) = (h.states[at("agree")], h.states[at("agree") + 1]);
    println!("labelled {:?}+{:?}, control {:?}+{:?}", w.pos, w.size, c.pos, c.size);
    assert!(w.size.w > c.size.w + 60.0, "setup: the words take no room");
    let (x, y) = (w.pos.x + w.size.w - 10.0, w.pos.y + w.size.h / 2.0);
    h.post(Event::PointerDown { x, y, button: lumen_static::PointerButton::Left, clicks: 1 });
    h.pump();
    h.post(Event::PointerUp { x, y, button: lumen_static::PointerButton::Left });
    h.pump();
    assert_eq!(h.app.flips, 1, "a click on the words did not reach the toggle");
    // Past the words is not the control.
    let x = w.pos.x + w.size.w + 10.0;
    h.post(Event::PointerDown { x, y, button: lumen_static::PointerButton::Left, clicks: 1 });
    h.pump();
    h.post(Event::PointerUp { x, y, button: lumen_static::PointerButton::Left });
    h.pump();
    assert_eq!(h.app.flips, 1, "a click past the words flipped it");
}

#[test]
fn hovering_the_words_shades_the_control_even_when_on() {
    let mut h = host();
    h.app.ui.agree.control.on = true;
    h.invalidate_all();
    h.pump();
    let i = at("agree") + 1;
    let rest = h.states[i].fg;
    let w = h.states[at("agree")];
    h.post(Event::PointerMove { x: w.pos.x + w.size.w - 10.0, y: w.pos.y + w.size.h / 2.0 });
    h.pump();
    let hover = h.states[i].fg;
    println!("on body at rest {rest:?}, hovered via the words {hover:?}; control hovered {}", h.states[i].hovered);
    assert!(h.states[i].hovered, "hovering the words did not hover the control");
    assert_ne!(rest, hover, "an on toggle shows no hover");
}
