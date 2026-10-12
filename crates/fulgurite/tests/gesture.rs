//! T15.5: touch, and recognizers as types.

use fulgurite::gesture::{LongPress, Pinch, Touches, MAX_TOUCHES};
use fulgurite::{Event, InputQueue, Point, TouchPhase};

fn t(id: u32, x: f32, y: f32, phase: TouchPhase) -> Event {
    Event::Touch { id, x, y, phase }
}

/// **The budget, checked before the design and asserted after** — the
/// rule T14.1 set. A `u64` touch id would have made `Event` 24 bytes
/// and the default queue 1 560 rather than 1 048.
///
/// Since 2026-09-07 that is a cost to weigh rather than a wall: `CAP`
/// is a type parameter, so a host that needs a bigger vocabulary can
/// buy it back with a shorter queue. The `u32` id is still the right
/// call — the id is a per-session sequence — but it is now a choice
/// rather than a constraint.
#[test]
fn touch_did_not_grow_the_event_vocabulary() {
    assert_eq!(core::mem::size_of::<Event>(), 16);
    assert_eq!(core::mem::size_of::<InputQueue>(), 1048);
}

#[test]
fn touches_track_and_release() {
    let mut s = Touches::default();
    assert!(s.feed(&t(1, 0.0, 0.0, TouchPhase::Begin)));
    assert!(s.feed(&t(2, 10.0, 0.0, TouchPhase::Begin)));
    assert_eq!(s.len(), 2);
    assert!(s.feed(&t(1, 5.0, 0.0, TouchPhase::Move)));
    assert_eq!(s.at(0), Some(Point { x: 5.0, y: 0.0 }));
    assert!(s.feed(&t(1, 5.0, 0.0, TouchPhase::End)));
    assert_eq!(s.len(), 1);
    assert_eq!(s.at(0), Some(Point { x: 10.0, y: 0.0 }), "the survivor shifted down");
}

#[test]
fn a_third_finger_is_ignored_rather_than_evicting_one() {
    // Evicting would break a pinch in flight, which is worse than
    // ignoring a finger this recognizer does not track.
    let mut s = Touches::default();
    s.feed(&t(1, 0.0, 0.0, TouchPhase::Begin));
    s.feed(&t(2, 10.0, 0.0, TouchPhase::Begin));
    assert!(!s.feed(&t(3, 20.0, 0.0, TouchPhase::Begin)));
    assert_eq!(s.len(), MAX_TOUCHES);
    assert_eq!(s.at(0), Some(Point { x: 0.0, y: 0.0 }), "the first is still there");
}

#[test]
fn an_unknown_move_or_end_is_ignored() {
    let mut s = Touches::default();
    assert!(!s.feed(&t(9, 0.0, 0.0, TouchPhase::Move)));
    assert!(!s.feed(&t(9, 0.0, 0.0, TouchPhase::End)));
    assert!(s.is_empty());
}

#[test]
fn a_pinch_reports_scale_relative_to_where_it_started() {
    let mut p = Pinch::default();
    assert_eq!(p.feed(&t(1, 0.0, 0.0, TouchPhase::Begin)), None, "one finger is not a pinch");
    assert_eq!(p.feed(&t(2, 10.0, 0.0, TouchPhase::Begin)), Some(1.0), "starts at 1.0");

    let s = p.feed(&t(2, 20.0, 0.0, TouchPhase::Move)).expect("moved");
    assert!((s - 2.0).abs() < 0.01, "twice as far apart: {s}");
    assert_eq!(p.centre(), Some(Point { x: 10.0, y: 0.0 }), "centred between the fingers");

    let s = p.feed(&t(2, 5.0, 0.0, TouchPhase::Move)).expect("moved");
    assert!((s - 0.5).abs() < 0.01, "half as far: {s}");
}

#[test]
fn lifting_a_finger_ends_the_pinch() {
    let mut p = Pinch::default();
    p.feed(&t(1, 0.0, 0.0, TouchPhase::Begin));
    p.feed(&t(2, 10.0, 0.0, TouchPhase::Begin));
    assert_eq!(p.feed(&t(2, 10.0, 0.0, TouchPhase::End)), None);
    // And starting again is a fresh 1.0, not a continuation.
    assert_eq!(p.feed(&t(3, 40.0, 0.0, TouchPhase::Begin)), Some(1.0));
}

#[test]
fn a_long_press_fires_once() {
    let mut lp = LongPress::default();
    lp.feed(&t(1, 5.0, 5.0, TouchPhase::Begin));
    assert_eq!(lp.tick(200.0), None, "not yet");
    let at = lp.tick(400.0).expect("held long enough");
    assert_eq!(at, Point { x: 5.0, y: 5.0 });
    // Once, not per frame: a context menu should open once, not open
    // again on every tick the finger stays down.
    assert_eq!(lp.tick(400.0), None, "and only once");
}

#[test]
fn drifting_cancels_a_long_press() {
    let mut lp = LongPress::default();
    lp.feed(&t(1, 0.0, 0.0, TouchPhase::Begin));
    lp.feed(&t(1, 50.0, 0.0, TouchPhase::Move));
    assert_eq!(lp.tick(1000.0), None, "that was a drag, not a hold");
}

#[test]
fn a_small_drift_does_not_cancel_it() {
    let mut lp = LongPress::default();
    lp.feed(&t(1, 0.0, 0.0, TouchPhase::Begin));
    lp.feed(&t(1, 3.0, 3.0, TouchPhase::Move));
    assert!(lp.tick(1000.0).is_some(), "a finger is never perfectly still");
}

#[test]
fn lifting_cancels_a_long_press() {
    let mut lp = LongPress::default();
    lp.feed(&t(1, 0.0, 0.0, TouchPhase::Begin));
    lp.feed(&t(1, 0.0, 0.0, TouchPhase::End));
    assert_eq!(lp.tick(1000.0), None);
}

#[test]
fn cancel_is_not_end() {
    // The system took the touch away. A recognizer that treated it as a
    // release would commit an action the user did not finish, which is
    // why TouchPhase has four variants and not a bool.
    let mut s = Touches::default();
    s.feed(&t(1, 0.0, 0.0, TouchPhase::Begin));
    s.feed(&t(1, 0.0, 0.0, TouchPhase::Cancel));
    assert!(s.is_empty(), "the finger is gone either way");

    let mut lp = LongPress::default();
    lp.feed(&t(1, 0.0, 0.0, TouchPhase::Begin));
    lp.feed(&t(1, 0.0, 0.0, TouchPhase::Cancel));
    assert_eq!(lp.tick(1000.0), None, "and a cancelled hold does not fire");
}

#[test]
fn recognizers_cost_nothing_to_a_tree_that_holds_none() {
    // The gating policy's second mechanism again: these are types an
    // application owns, so there is no pass and nothing to opt out of.
    fn assert_copy<T: Copy>() {}
    assert_copy::<Touches>();
    assert_copy::<Pinch>();
    assert_copy::<LongPress>();
}

/// **A NaN `dt` must not stall a long press** — M47/T47.0, R7-F8.
///
/// `tick` clamps with `num::clamp`, which returns a NaN unchanged, so
/// `held` became NaN and `held >= hold` was false for ever: the press
/// never fired however long the finger stayed down.
///
/// After the NaN, 32 ticks of 16 ms (512 ms against the default 500 ms
/// hold) must fire it. The `-inf` arm (a clock that went backwards)
/// takes the same 32; the `1e9` arm (a stalled host) fires on the stall
/// itself and needs none.
#[test]
fn a_nan_dt_advances_nothing_and_the_press_still_fires() {
    const BOUND: usize = 32;
    for (label, bad, most) in
        [("NaN", f32::NAN, BOUND), ("-inf", f32::NEG_INFINITY, BOUND), ("1e9", 1e9, 0)]
    {
        let mut lp = LongPress::default();
        lp.feed(&t(1, 5.0, 5.0, TouchPhase::Begin));
        let mut fired = lp.tick(bad);
        let mut frames = 0;
        while fired.is_none() && frames < 10_000 {
            fired = lp.tick(16.0);
            frames += 1;
        }
        println!("dt {label}: fired={} after {frames} more 16 ms frames (bound {most})", fired.is_some());
        assert_eq!(
            fired,
            Some(Point { x: 5.0, y: 5.0 }),
            "dt {label}: unfired after {frames} frames (bound {most})"
        );
        assert!(frames <= most, "dt {label}: took {frames} frames to fire (bound {most})");
    }
}
