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

use fulgurite::event::{EventCtx, TouchPhase};
use fulgurite::prelude::*;
use fulgurite::style::{Length, Patch, Rule, Selector};
use fulgurite::Event;
use fulgurite_shell::Host;
use fulgurite_widgets::prelude::*;
use fulgurite_widgets::slider::Slider;

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
    let i = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).expect(wid);
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
        let want = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).unwrap() as u32;
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
fn a_touch_tap_lands_on_all_three() {
    let (p, _) = tap_all(by_pointer);
    let (n, log) = tap_all(by_touch);
    println!("touch actions handled: {n} / 3 (pointer: {p} / 3)  {log:?}");
    assert_eq!(n, 3, "a tap on a touchscreen does not activate the control under it");
}

fn touch(h: &mut Host<App>, id: u32, x: f32, y: f32, phase: TouchPhase) {
    h.post(Event::Touch { id, x, y, phase });
}

/// **A cancelled touch is not a tap** (M44/T44.4): Begin then Cancel on
/// the button presses it, then releases the press and the capture, and
/// clicks nothing.
#[test]
fn a_cancelled_touch_clicks_nothing_and_releases_the_press() {
    let mut h = host();
    let (x, y) = point(&h, "button");
    touch(&mut h, 3, x, y, TouchPhase::Begin);
    h.pump();
    let button = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "button").unwrap() as u32;
    assert_eq!(h.input.pressed, Some(button), "setup: the touch did not press the button");
    touch(&mut h, 3, x, y, TouchPhase::Cancel);
    h.pump();
    println!(
        "Begin then Cancel on the button: clicks {} (expected 0); pressed {:?} capture {:?} (expected None None)",
        h.app.clicks, h.input.pressed, h.input.capture
    );
    assert_eq!(h.app.clicks, 0, "a cancelled touch clicked the button");
    assert_eq!((h.input.pressed, h.input.capture), (None, None), "the cancel left the press or the capture");

    // And a slider dragged by a finger that is then cancelled lets go.
    let (sx, sy) = point(&h, "slider");
    touch(&mut h, 4, sx, sy, TouchPhase::Begin);
    h.pump();
    assert!(h.input.capture.is_some(), "setup: the slider did not capture the touch");
    touch(&mut h, 4, sx, sy, TouchPhase::Cancel);
    h.pump();
    let v = h.app.ui.slider.value();
    h.post(Event::PointerMove { x: sx - 150.0, y: sy });
    h.pump();
    println!("slider after a cancelled drag and a free move: {v} -> {} (capture {:?})", h.app.ui.slider.value(), h.input.capture);
    assert_eq!(h.input.capture, None, "the cancel did not release the slider's capture");
    assert_eq!(h.app.ui.slider.value(), v, "a free move dragged the slider after its touch was cancelled");
}

/// **Only the primary touch is a pointer**: a second finger down and up
/// on the button while the first is held elsewhere clicks nothing.
///
/// The first finger rests on the **background**, not on a control. The
/// first version held it on the slider, whose capture then took the
/// second finger's release whatever the promotion did — so promoting
/// every finger (the injection) left the test green.
#[test]
fn a_second_finger_does_not_click() {
    let mut h = host();
    let (bx, by) = point(&h, "button");
    let (ex, ey) = (310.0, 190.0);
    assert!(h.probe(ex, ey).is_none_or(|x| x.node == 0), "setup: the first finger's point is on a control");
    touch(&mut h, 1, ex, ey, TouchPhase::Begin);
    h.pump();
    assert_eq!(h.input.capture, None, "setup: the first finger captured something");
    touch(&mut h, 2, bx, by, TouchPhase::Begin);
    touch(&mut h, 2, bx, by, TouchPhase::End);
    h.pump();
    println!("second finger tapped the button while the first rested on the background: clicks {} (expected 0)", h.app.clicks);
    assert_eq!(h.app.clicks, 0, "a second finger clicked the button");
    touch(&mut h, 1, ex, ey, TouchPhase::End);
    h.pump();
    // With the first finger gone, the next Begin is primary again.
    touch(&mut h, 5, bx, by, TouchPhase::Begin);
    touch(&mut h, 5, bx, by, TouchPhase::End);
    h.pump();
    assert_eq!(h.app.clicks, 1, "after the primary lifted, the next finger did not become primary");
}

/// **X11's extra `CursorMoved`**: winit synthesises a cursor move for the
/// first touch there. It must be one more move and never a second press.
#[test]
fn a_platform_cursor_move_for_the_touch_is_not_a_second_press() {
    let mut h = host();
    let (x, y) = point(&h, "button");
    touch(&mut h, 9, x, y, TouchPhase::Begin);
    h.post(Event::PointerMove { x, y });
    touch(&mut h, 9, x, y, TouchPhase::End);
    h.post(Event::PointerMove { x, y });
    h.pump();
    println!("touch tap with the platform's cursor moves interleaved: clicks {} (expected 1)", h.app.clicks);
    assert_eq!(h.app.clicks, 1, "the platform's cursor move turned one tap into {} clicks", h.app.clicks);
}
