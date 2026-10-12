//! **A full queue must not strand a gesture** — M44/T44.5
//! (`docs/plan-input-semantics.md` §2.5), closing R3-F2.
//!
//! `Host::post` pushes into a 64-slot `InputQueue` and discards the
//! refusal, and the window posts every `CursorMoved` unmerged. So more
//! than 64 events between two pumps loses the tail — and when the tail
//! holds the `PointerUp`, capture is never released: a later move with no
//! button held still drags the slider.
//!
//! Two shapes, because T44.5 has two mechanisms and each needs a case
//! the other cannot rescue: a run of bare moves (which **merge** into one
//! slot), and moves interleaved with a modifier change (which cannot
//! merge, so the release must **evict** a move to find room). The first
//! version had only the bare run, and removing the eviction left it
//! green. `Host::dropped_inputs` is printed beside `CAP` in each.

use fulgurite::prelude::*;
use fulgurite::style::{Length, Patch, Rule, Selector};
use fulgurite::{Event, InputQueue};
use fulgurite_shell::Host;
use fulgurite_widgets::slider::Slider;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("slider")]
    slider: Slider,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

const CAP: usize = InputQueue::<64>::CAP;

fn host() -> (Host<App>, f32, f32, f32) {
    let mut app = App::default();
    app.ui.slider = Slider::new(0.1);
    let mut h: Host<App> = Host::new(app, 320, 80);
    h.set_stylesheet(&[Rule::new(
        Selector::Id("slider"),
        Patch::new().width(Length::Fixed(200.0)).height(Length::Fixed(24.0)),
    )]);
    h.pump();
    h.pump();
    let i = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "slider").unwrap();
    let s = h.states[i];
    assert!(s.size.w > 0.0, "setup: the slider has no area");
    (h, s.pos.x, s.size.w, s.pos.y + s.size.h / 2.0)
}

/// Press at 10%, post `moves` moves (each followed by a modifier change
/// when `interleave`) and a release without pumping, pump once; then an
/// unpressed move to 90%. Returns what the gesture left, and the host's
/// dropped count.
fn gesture(moves: usize, interleave: bool) -> (Option<u32>, Option<u32>, f32, f32, u32) {
    let (mut h, x0, w, y) = host();
    h.post(Event::down(x0 + w * 0.1, y));
    h.pump();
    let si = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "slider").unwrap() as u32;
    assert_eq!(h.input.capture, Some(si), "setup: the press did not capture the slider");
    for k in 0..moves {
        let f = 0.1 + 0.3 * (k as f32 / moves.max(1) as f32);
        h.post(Event::PointerMove { x: x0 + w * f, y });
        if interleave {
            h.post(Event::Modifiers(fulgurite::Mods::NONE));
        }
    }
    h.post(Event::up(x0 + w * 0.4, y));
    h.pump();
    let (capture, pressed) = (h.input.capture, h.input.pressed);
    let released_at = h.app.ui.slider.value();
    // No button held: this move must not drag anything.
    h.post(Event::PointerMove { x: x0 + w * 0.9, y });
    h.pump();
    (capture, pressed, released_at, h.app.ui.slider.value(), h.dropped_inputs())
}

#[test]
fn a_release_behind_two_hundred_moves_still_releases() {
    let (capture, pressed, at_release, after, dropped) = gesture(200, false);
    println!(
        "200 moves + 1 up into CAP {CAP}: dropped {dropped}; capture {capture:?} pressed {pressed:?}; \
         value at release {at_release}, after an unpressed move to 0.9: {after}"
    );
    assert_eq!(capture, None, "the release was dropped and capture is stranded");
    assert_eq!(pressed, None, "the release was dropped and the press is stranded");
    assert_eq!(after, at_release, "an unpressed move dragged the slider");
    assert_eq!(dropped, 0, "a run of moves lost events instead of merging");
}

/// Moves that cannot merge fill the queue, and the release must still
/// get in — by evicting the newest move.
#[test]
fn a_release_behind_a_full_queue_of_unmergeable_moves_still_releases() {
    let (capture, pressed, at_release, after, dropped) = gesture(100, true);
    println!(
        "100 x (move, modifiers) + 1 up into CAP {CAP}: dropped {dropped} (nonzero: counted, not silent); \
         capture {capture:?} pressed {pressed:?}; value at release {at_release}, after {after}"
    );
    assert_eq!(capture, None, "a full queue refused the release and capture is stranded");
    assert_eq!(pressed, None, "a full queue refused the release and the press is stranded");
    assert_eq!(after, at_release, "an unpressed move dragged the slider");
    assert!(dropped > 0, "setup: the queue never filled, so this exercised no eviction");
}

/// The accept direction: a gesture that fits the queue releases, and the
/// move after it drags nothing.
#[test]
fn a_release_behind_sixty_three_moves_releases() {
    let (capture, pressed, at_release, after, dropped) = gesture(63, false);
    println!(
        "63 moves + 1 up into CAP {CAP}: dropped {dropped}; capture {capture:?} pressed {pressed:?}; value at release {at_release}, after {after}"
    );
    assert_eq!((capture, pressed), (None, None), "a gesture that fits did not release");
    assert_eq!(after, at_release, "an unpressed move dragged the slider");
}

/// And a single move is delivered at its exact position: merging never
/// moves a move.
#[test]
fn a_single_move_lands_where_it_was_posted() {
    let (mut h, x0, w, y) = host();
    // Pressed on the track, **off the knob**: a press on the knob keeps
    // its offset from the knob's centre through the drag (M57/B14), which
    // would move every later position by that offset. At 10% the press
    // landed on the knob of a slider at 0.
    h.post(Event::down(x0 + w * 0.3, y));
    h.pump();
    h.post(Event::PointerMove { x: x0 + w * 0.6, y });
    h.pump();
    // **Against the slider's own mapping**, not `x / w`: since M57/B14 the
    // knob's centre travels inside the track, so a pointer 60% along is
    // not the value 0.6. A press made directly there — on the track, so
    // with no grab offset — is what the delivered move must equal.
    let (mut d, _, _, _) = host();
    d.post(Event::down(x0 + w * 0.6, y));
    d.pump();
    let direct = d.app.ui.slider.value();
    println!("one move to 60% under capture: value {}; a press there: {direct}", h.app.ui.slider.value());
    assert!((h.app.ui.slider.value() - direct).abs() < 1e-4, "the move landed at {}, a press there gives {direct}", h.app.ui.slider.value());
}
