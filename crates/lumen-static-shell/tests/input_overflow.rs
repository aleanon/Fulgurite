//! **A full queue must not strand a gesture** — M44/T44.5
//! (`docs/plan-input-semantics.md` §2.5), closing R3-F2.
//!
//! `Host::post` pushes into a 64-slot `InputQueue` and discards the
//! refusal, and the window posts every `CursorMoved` unmerged. So more
//! than 64 events between two pumps loses the tail — and when the tail
//! holds the `PointerUp`, capture is never released: a later move with no
//! button held still drags the slider.
//!
//! `Host` does not expose its queue's length or dropped count yet (T44.5
//! adds a dropped-inputs accessor), so the counts printed here are the ones posted
//! and the capacity; what was dropped is `posted - CAP` until then.

use lumen_static::prelude::*;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{Event, InputQueue};
use lumen_static_shell::Host;
use lumen_static_widgets::slider::Slider;

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
    let i = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "slider").unwrap();
    let s = h.states[i];
    assert!(s.size.w > 0.0, "setup: the slider has no area");
    (h, s.pos.x, s.size.w, s.pos.y + s.size.h / 2.0)
}

/// Press at 10%, post `moves` moves and a release without pumping, pump
/// once; then an unpressed move to 90%. Returns what the gesture left.
fn gesture(moves: usize) -> (Option<u32>, Option<u32>, f32, f32) {
    let (mut h, x0, w, y) = host();
    h.post(Event::down(x0 + w * 0.1, y));
    h.pump();
    let si = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "slider").unwrap() as u32;
    assert_eq!(h.input.capture, Some(si), "setup: the press did not capture the slider");
    for k in 0..moves {
        let f = 0.1 + 0.3 * (k as f32 / moves.max(1) as f32);
        h.post(Event::PointerMove { x: x0 + w * f, y });
    }
    h.post(Event::up(x0 + w * 0.4, y));
    h.pump();
    let (capture, pressed) = (h.input.capture, h.input.pressed);
    let released_at = h.app.ui.slider.value();
    // No button held: this move must not drag anything.
    h.post(Event::PointerMove { x: x0 + w * 0.9, y });
    h.pump();
    (capture, pressed, released_at, h.app.ui.slider.value())
}

#[test]
#[ignore = "red until T44.5: R3-F2 (the queue refuses the release, so capture is stranded)"]
fn a_release_behind_two_hundred_moves_still_releases() {
    let posted: usize = 1 + 200 + 1;
    let (capture, pressed, at_release, after) = gesture(200);
    println!(
        "posted {posted} events (1 down pumped alone, then 200 moves + 1 up) into CAP {CAP}: \
         would drop {} of them; after the pump capture {capture:?} pressed {pressed:?}; \
         value at release {at_release}, after an unpressed move to 0.9: {after}",
        (posted - 1).saturating_sub(CAP)
    );
    assert_eq!(capture, None, "the release was dropped and capture is stranded");
    assert_eq!(pressed, None, "the release was dropped and the press is stranded");
    assert_eq!(after, at_release, "an unpressed move dragged the slider");
}

/// The accept direction: a gesture that fits the queue releases, and the
/// move after it drags nothing.
#[test]
fn a_release_behind_sixty_three_moves_releases() {
    let (capture, pressed, at_release, after) = gesture(63);
    println!(
        "63 moves + 1 up into CAP {CAP}: capture {capture:?} pressed {pressed:?}; value at release {at_release}, after {after}"
    );
    assert_eq!((capture, pressed), (None, None), "a gesture that fits did not release");
    assert_eq!(after, at_release, "an unpressed move dragged the slider");
}
