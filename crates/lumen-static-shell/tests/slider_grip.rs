//! **A slider keeps its grip, reaches its ends, and pages** — M57/B14
//! (`docs/plan-widget-fixes.md`).
//!
//! A press on the knob snapped its centre to the pointer (a jump of up to
//! half a knob); the pointer mapped over the whole width while the knob
//! was clamped half a knob in, so the value under the knob was not the
//! value drawn; and only Left and Right moved it
//! (`docs/widget-audit.md`, Slider).

use lumen_static::event::Key;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{App as DeriveApp, Event, StaticTree, Widget};
use lumen_static_shell::Host;
use lumen_static_widgets::Slider;

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    #[widget_id("s")]
    s: Slider,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

fn host(v: f32) -> Host<App> {
    let mut app = App::default();
    app.ui.s = Slider::new(v);
    let mut h = Host::new(app, 300, 100);
    h.set_stylesheet(&[Rule::new(Selector::Id("s"), Patch::new().width(Length::Fixed(200.0)))]);
    h.pump();
    h
}

fn box_of(h: &Host<App>) -> (f32, f32, f32, f32) {
    let i = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "s").unwrap();
    let s = h.states[i];
    (s.pos.x, s.pos.y, s.size.w, s.size.h)
}

fn press(h: &mut Host<App>, x: f32) {
    let (_, y, _, hh) = box_of(h);
    h.post(Event::PointerDown { x, y: y + hh / 2.0, button: lumen_static::PointerButton::Left, clicks: 1 });
    h.pump();
}

fn release(h: &mut Host<App>, x: f32) {
    let (_, y, _, hh) = box_of(h);
    h.post(Event::PointerUp { x, y: y + hh / 2.0, button: lumen_static::PointerButton::Left });
    h.pump();
}

#[test]
fn a_press_on_the_knob_does_not_move_it() {
    let mut h = host(0.5);
    let (x0, _, w, hh) = box_of(&h);
    let d = hh * 0.75;
    let centre = x0 + d / 2.0 + (w - d) * 0.5;
    // Six px right of the knob's centre, still on the knob.
    press(&mut h, centre + 6.0);
    let v = h.app.ui.s.value();
    println!("knob centre {centre}, press at +6: value {v}");
    assert_eq!(v, 0.5, "a press on the knob moved it");
    // A drag keeps the grip: 20 px right moves the value 20 px worth.
    h.post(Event::PointerMove { x: centre + 26.0, y: 0.0 });
    h.pump();
    let moved = h.app.ui.s.value();
    let want = 0.5 + 20.0 / (w - d);
    println!("dragged 20 px: {moved}, expected {want}");
    assert!((moved - want).abs() < 1e-3, "the drag lost the grip");
    release(&mut h, centre + 26.0);
}

#[test]
fn both_ends_are_reachable_under_the_pointer() {
    let mut h = host(0.5);
    let (x0, _, w, hh) = box_of(&h);
    let d = hh * 0.75;
    // Half a knob in from either end is exactly the end, and a press on
    // the track (not the knob) jumps there.
    press(&mut h, x0 + d / 2.0);
    release(&mut h, x0 + d / 2.0);
    let lo = h.app.ui.s.value();
    press(&mut h, x0 + w - d / 2.0);
    release(&mut h, x0 + w - d / 2.0);
    let hi = h.app.ui.s.value();
    println!("ends: {lo} {hi}");
    assert_eq!((lo, hi), (0.0, 1.0), "an end is not where the knob's centre can go");
}

#[test]
fn up_down_and_page_keys_move_it() {
    let mut h = host(0.5);
    let i = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "s").unwrap();
    h.input.focus = Some(i as u32);
    let key = |h: &mut Host<App>, k: Key| {
        h.post(Event::Key { key: k, down: true });
        h.post(Event::Key { key: k, down: false });
        h.pump();
        h.app.ui.s.value()
    };
    let up = key(&mut h, Key::Up);
    let down = key(&mut h, Key::Down);
    let page = key(&mut h, Key::PageUp);
    let back = key(&mut h, Key::PageDown);
    println!("Up {up}, Down {down}, PageUp {page}, PageDown {back}");
    assert!((up - 0.55).abs() < 1e-5 && (down - 0.5).abs() < 1e-5, "Up/Down did not step");
    assert!((page - 1.0).abs() < 1e-5 && (back - 0.5).abs() < 1e-5, "a page is not ten steps");
}
