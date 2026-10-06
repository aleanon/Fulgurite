//! **What a secondary button does today** — M90/T90.0's pins
//! (`docs/plan-desktop-integration.md`, "A secondary button never reaches
//! the press machinery").
//!
//! Every gate here builds its host with `Program::host`, the function the
//! window's session boots through, and drives it with `post` and `pump`,
//! under `Everything` and `Boundaries<256>`. Each one **pins today's
//! behaviour green** and names the task that flips it
//! (`docs/plan-iced-roadmap.md` §3.1): a right release activates (pass
//! 2's `activates` matches `PointerUp { .. }` for any button), a right
//! press acts in pass 1 on every widget that matches `PointerDown { .. }`,
//! and a right release mid-drag ends a left drag's capture. T90.2 flips
//! all three. A primary touch held for 600 ms is still a tap — its `End`
//! is a release, so a `Button` under it activates — and a press-acting
//! control has acted at `Begin`; T90.3 flips the first and keeps the
//! second (second review P16).
//!
//! Each pin asserts its setup landed (the left button at the same point
//! changes the same effect) and prints the quantity it pins.

use lumen_static::event::{EventCtx, PointerButton, TouchPhase};
use lumen_static::invalidate::{Boundaries, Everything, Invalidation};
use lumen_static::prelude::*;
use lumen_static::retain::Incremental;
use lumen_static::Event;
use lumen_static_shell::program::{application, Program};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

const W: u32 = 360;
const H: u32 = 420;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct Ui {
    #[widget_id("button")]
    button: Button<Count>,
    #[widget_id("check")]
    check: CheckBox<Count>,
    #[widget_id("slider")]
    slider: Slider,
    #[widget_id("section")]
    section: Collapsible<Label>,
    #[widget_id("tabs")]
    tabs: Tabs,
    #[widget_id("field")]
    field: TextInput,
}

/// How many times each release-acting control's handler ran.
#[derive(Default)]
struct Count {
    clicks: u32,
    toggles: u32,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
    #[state]
    count: Count,
}

fn boot() -> App {
    let mut a = App::default();
    a.ui.button.label = "press";
    a.ui.button.on_click = |c: &mut Count, _: &EventCtx| c.clicks += 1;
    a.ui.check.on_toggle = |c: &mut Count, _: &EventCtx| c.toggles += 1;
    let mut body = Label::default();
    body.set("Body");
    a.ui.section = Collapsible::new("Section", body);
    a.ui.tabs = Tabs::new(&["One", "Two", "Three"]);
    a.ui.field = TextInput::new("hello world");
    // The caret at the start, so a press -- which puts it where it
    // lands, and a font-less host shapes no clusters, so that is the end
    // -- moves it: 0 -> 11.
    a.ui.field.move_home();
    a
}

fn program() -> Program<App> {
    application(boot).size(W, H)
}

type H<I> = Host<App, Incremental, I>;

fn index(wid: &str) -> usize {
    lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).unwrap_or_else(|| panic!("no #{wid}"))
}

/// A point inside `#wid`, at fractions `(fx, fy)` of its box.
fn at<I: Invalidation>(h: &H<I>, wid: &str, fx: f32, fy: f32) -> (f32, f32) {
    let s = &h.states[index(wid)];
    (s.pos.x + s.size.w * fx, s.pos.y + s.size.h * fy)
}

/// Where each widget is pressed: inside its box, and where a press acts.
/// The tab is the second (not the selected first), the field's caret
/// starts at 0 and a press moves it to the end (no font, no clusters),
/// the slider starts at 0 and is pressed at three quarters, and the
/// section is pressed in its header band.
fn target(name: &str) -> (&'static str, f32, f32) {
    match name {
        "Button" => ("button", 0.5, 0.5),
        "CheckBox" => ("check", 0.5, 0.5),
        "Slider" => ("slider", 0.75, 0.5),
        "Collapsible" => ("section", 0.3, 0.1),
        "Tabs" => ("tabs", 0.5, 0.5),
        "TextInput" => ("field", 0.5, 0.5),
        _ => unreachable!(),
    }
}

/// The widget's effect, as one number: the handler count, the toggle
/// count, the value, open, the selected tab, the caret byte.
fn effect<I: Invalidation>(h: &H<I>, name: &str) -> f32 {
    match name {
        "Button" => h.app.count.clicks as f32,
        "CheckBox" => h.app.count.toggles as f32,
        "Slider" => h.app.ui.slider.value(),
        "Collapsible" => h.app.ui.section.is_open() as u8 as f32,
        "Tabs" => h.app.ui.tabs.selected() as f32,
        "TextInput" => h.app.ui.field.caret() as f32,
        _ => unreachable!(),
    }
}

/// A press and a release with `button` at the widget's target point, in
/// one pump. Answers `(handled, effect before, effect after)`.
fn click<I: Invalidation>(name: &str, button: PointerButton) -> (bool, f32, f32) {
    let mut h: H<I> = program().host();
    h.pump();
    h.pump();
    let (wid, fx, fy) = target(name);
    let (x, y) = at(&h, wid, fx, fy);
    let before = effect(&h, name);
    h.post(Event::PointerMove { x, y });
    h.pump();
    h.post(Event::PointerDown { x, y, button, clicks: 1 });
    h.post(Event::PointerUp { x, y, button });
    let handled = h.pump();
    (handled, before, effect(&h, name))
}

const WIDGETS: [&str; 6] = ["Button", "CheckBox", "Slider", "Collapsible", "Tabs", "TextInput"];

fn right_press_and_release_acts<I: Invalidation>(strategy: &str) {
    let mut wrong = Vec::new();
    for name in WIDGETS {
        // Setup landed: the left button at the same point changes the
        // effect, so the point is on the control and the effect is one a
        // press there moves.
        let (lh, lb, la) = click::<I>(name, PointerButton::Left);
        assert!(lh && la != lb, "setup [{strategy}]: a left click on {name} did not act (handled {lh}, {lb} -> {la})");
        let (rh, rb, ra) = click::<I>(name, PointerButton::Right);
        println!(
            "[{strategy}] {name:<11}: right press+release handled {rh}, effect {rb} -> {ra} \
             (left: handled {lh}, {lb} -> {la})"
        );
        if !(rh && ra == la) {
            wrong.push(name);
        }
    }
    assert!(
        wrong.is_empty(),
        "GAP pinned [{strategy}]: a right press and release acts like a left one today -- \
         a right release activates and a right press acts in pass 1; T90.2 flips this. \
         These no longer act as a left click: {wrong:?}"
    );
}

/// **A right press and release acts like a left one** on all six
/// widgets — the release-acting two (a right release activates) and the
/// press-acting four (a right press acts in pass 1). T90.2 flips it.
#[test]
fn a_right_press_and_release_acts_like_a_left_one_today() {
    right_press_and_release_acts::<Everything>("Everything");
    right_press_and_release_acts::<Boundaries<256>>("Boundaries");
}

fn right_release_mid_drag<I: Invalidation>(strategy: &str) {
    let mut h: H<I> = program().host();
    h.pump();
    h.pump();
    let (x0, y) = at(&h, "slider", 0.25, 0.5);
    let (x1, _) = at(&h, "slider", 0.5, 0.5);
    let (x2, _) = at(&h, "slider", 0.9, 0.5);
    h.post(Event::PointerMove { x: x0, y });
    h.post(Event::down(x0, y));
    h.pump();
    let pressed = h.app.ui.slider.value();
    h.post(Event::PointerMove { x: x1, y });
    h.pump();
    let dragged = h.app.ui.slider.value();
    // Setup landed: the left drag captured and the value followed it.
    assert!(h.input.capture.is_some() && dragged > pressed, "setup [{strategy}]: the left drag did not capture ({pressed} -> {dragged})");
    h.post(Event::PointerUp { x: x1, y, button: PointerButton::Right });
    h.pump();
    let held = h.input.capture.is_some();
    h.post(Event::PointerMove { x: x2, y });
    h.pump();
    let after = h.app.ui.slider.value();
    h.post(Event::up(x2, y));
    h.pump();
    println!(
        "[{strategy}] slider drag: press {pressed:.3}, dragged {dragged:.3}; a right release, capture held {held}; \
         the next move to 0.9 reads {after:.3}"
    );
    assert!(
        !held && after == dragged,
        "GAP pinned [{strategy}]: a right release ends a left drag's capture today, so the value \
         no longer follows -- T90.2 flips this; capture held {held}, value {dragged:.3} -> {after:.3}"
    );
}

/// **A right release during a left slider drag ends the drag** — the
/// capture arm matches `PointerUp { .. }` for any button. T90.2 flips it.
#[test]
fn a_right_release_ends_a_left_drag_today() {
    right_release_mid_drag::<Everything>("Everything");
    right_release_mid_drag::<Boundaries<256>>("Boundaries");
}

/// A primary touch at the widget's target point, held for 600 ms by
/// `Host::tick`, then lifted. Answers `(effect before, after the hold,
/// after the lift)`.
fn hold<I: Invalidation>(name: &str) -> (f32, f32, f32) {
    let mut h: H<I> = program().host();
    h.pump();
    h.pump();
    let (wid, fx, fy) = target(name);
    let (x, y) = at(&h, wid, fx, fy);
    let before = effect(&h, name);
    h.post(Event::Touch { id: 1, x, y, phase: TouchPhase::Begin });
    h.pump();
    h.tick(600.0);
    h.pump();
    let held = effect(&h, name);
    h.post(Event::Touch { id: 1, x, y, phase: TouchPhase::End });
    h.pump();
    (before, held, effect(&h, name))
}

fn long_hold<I: Invalidation>(strategy: &str) {
    let mut fired = Vec::new();
    let (b, held, lifted) = hold::<I>("Button");
    println!("[{strategy}] Button held 600 ms: handler count {b} -> {held} (held) -> {lifted} (lifted; pinned 1)");
    assert_eq!(held, 0.0, "setup [{strategy}]: the button activated before its touch lifted");
    if lifted != 1.0 {
        fired.push(format!(
            "GAP pinned [{strategy}]: a 600 ms hold's End is a release today, so the button under it \
             activates -- T90.3 flips this to a cancel; handler count {lifted}"
        ));
    }
    for name in ["Collapsible", "Tabs"] {
        let (b, held, lifted) = hold::<I>(name);
        println!("[{strategy}] {name} held 600 ms: {b} -> {held} (held) -> {lifted} (lifted; pinned 1, acted at Begin)");
        if !(b == 0.0 && held == 1.0 && lifted == 1.0) {
            fired.push(format!(
                "pinned [{strategy}]: a press-acting {name} acts at the touch's Begin, before any hold \
                 resolves (P16; T90.3 keeps this): {b} -> {held} -> {lifted}"
            ));
        }
    }
    assert!(fired.is_empty(), "{}", fired.join("\n"));
}

/// **A long hold is a tap today**, and a press-acting control has acted
/// at `Begin`. T90.3 cancels the release and keeps the press (P16).
#[test]
fn a_long_hold_is_a_tap_today() {
    long_hold::<Everything>("Everything");
    long_hold::<Boundaries<256>>("Boundaries");
}

/// **The sizes M90 must not move**, at T90.0 — the baseline rows of
/// `docs/results-desktop-integration.md` (`docs/plan-iced-roadmap.md`
/// §2.1, K1). M90 adds `Event::ContextRequest`, `Event::Menu`,
/// `Key::Menu` and `Action::ContextMenu` and no `Style` or `NodeState`
/// field, so each later task re-reads these and finds them unchanged.
#[test]
fn the_sizes_m90_must_not_move() {
    use core::mem::size_of;
    let (ev, action, key) = (size_of::<Event>(), size_of::<lumen_static::Action>(), size_of::<lumen_static::Key>());
    let (style, state) = (size_of::<lumen_static::Style>(), size_of::<lumen_static::NodeState>());
    println!(
        "Event {ev} B (bound 16), Action {action} B (8), Key {key} B (8), Style {style} B (84), NodeState {state} B (88), \
         per node {} B",
        style + state
    );
    assert_eq!(ev, 16, "Event grew past K1's 16 B");
    assert_eq!(action, 8, "Action moved from its size at T90.0");
    assert_eq!(key, 8, "Key moved from its size at T90.0");
    assert_eq!(style, 84, "Style moved from its size at T90.0");
    assert_eq!(state, 88, "NodeState moved from its size at T90.0");
}
