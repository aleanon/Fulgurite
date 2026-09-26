//! **The primary touch drives the pointer** — M44/T44.4
//! (`docs/plan-input-semantics.md` §2.4), closing R15-F4.
//!
//! `Event::Touch` is translated one to one from the platform, and no
//! stock widget handles it, so on a touchscreen a tap activated nothing:
//! a `Button` did not click, a `CheckBox` did not toggle and a `Slider`
//! did not move. From the review's r15-touch-only fixture (probe
//! `85ad947`). Each control is tapped at its centre, by touch and then —
//! the accept direction — by pointer, and the state change each tap made
//! is printed beside the count.

use lumen_static::event::{EventCtx, TouchPhase};
use lumen_static::prelude::*;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::Event;
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;
use lumen_static_widgets::slider::Slider;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct Ui {
    #[widget_id("button")]
    button: Button<App>,
    #[widget_id("check")]
    check: CheckBox<App>,
    #[widget_id("slider")]
    slider: Slider,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
    clicks: u32,
    toggles: u32,
}

fn host() -> Host<App> {
    let mut app = App::default();
    app.ui.button.label = "tap";
    app.ui.button.on_click = |a: &mut App, _: &EventCtx| a.clicks += 1;
    app.ui.check.on_toggle = |a: &mut App, _: &EventCtx| a.toggles += 1;
    app.ui.slider = Slider::new(0.1);
    let mut h: Host<App> = Host::new(app, 320, 200);
    h.set_stylesheet(&[
        Rule::new(Selector::Id("button"), Patch::new().width(Length::Fixed(80.0)).height(Length::Fixed(32.0))),
        Rule::new(Selector::Id("check"), Patch::new().width(Length::Fixed(24.0)).height(Length::Fixed(24.0))),
        Rule::new(Selector::Id("slider"), Patch::new().width(Length::Fixed(200.0)).height(Length::Fixed(24.0))),
    ]);
    h.pump();
    h.pump();
    h
}

/// A point on `wid`: its centre, or for the slider three quarters along,
/// so a tap that lands moves the value away from its 0.1 start.
fn point(h: &Host<App>, wid: &str) -> (f32, f32) {
    let i = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).expect(wid);
    let s = h.states[i];
    assert!(s.size.w > 0.0 && s.size.h > 0.0, "setup: #{wid} has no area");
    let fx = if wid == "slider" { 0.75 } else { 0.5 };
    (s.pos.x + s.size.w * fx, s.pos.y + s.size.h / 2.0)
}

fn state(h: &Host<App>) -> (u32, u32, f32) {
    (h.app.clicks, h.app.toggles, h.app.ui.slider.value())
}

/// Tap each of the three with `tap`, and count the taps that changed
/// what they should have.
fn tap_all(tap: impl Fn(&mut Host<App>, f32, f32)) -> (usize, Vec<String>) {
    let mut h = host();
    let mut landed = 0;
    let mut log = Vec::new();
    for wid in ["button", "check", "slider"] {
        let (x, y) = point(&h, wid);
        // The probe must hit the control, or a "0" means the tap missed.
        let hit = h.probe(x, y).expect("setup: the tap point hits nothing");
        let want = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).unwrap() as u32;
        assert_eq!(hit.node, want, "setup: the tap point on #{wid} hits node {} instead", hit.node);
        let before = state(&h);
        tap(&mut h, x, y);
        let after = state(&h);
        let moved = match wid {
            "button" => after.0 == before.0 + 1,
            "check" => after.1 == before.1 + 1,
            _ => after.2 != before.2,
        };
        landed += moved as usize;
        log.push(format!("#{wid}: {before:?} -> {after:?}"));
    }
    (landed, log)
}

fn by_pointer(h: &mut Host<App>, x: f32, y: f32) {
    h.post(Event::down(x, y));
    h.post(Event::up(x, y));
    h.pump();
}

fn by_touch(h: &mut Host<App>, x: f32, y: f32) {
    h.post(Event::Touch { id: 7, x, y, phase: TouchPhase::Begin });
    h.post(Event::Touch { id: 7, x, y, phase: TouchPhase::End });
    h.pump();
}

/// The accept direction: the same three taps by pointer all land, so a
/// 0 from touch is about touch and not about the fixture.
#[test]
fn a_pointer_tap_lands_on_all_three() {
    let (n, log) = tap_all(by_pointer);
    println!("pointer actions handled: {n} / 3  {log:?}");
    assert_eq!(n, 3, "the fixture's pointer taps do not land");
}

#[test]
#[ignore = "red until T44.4: R15-F4 (no widget handles Event::Touch, and nothing promotes it to the pointer)"]
fn a_touch_tap_lands_on_all_three() {
    let (p, _) = tap_all(by_pointer);
    let (n, log) = tap_all(by_touch);
    println!("touch actions handled: {n} / 3 (pointer: {p} / 3)  {log:?}");
    assert_eq!(n, 3, "a tap on a touchscreen does not activate the control under it");
}
