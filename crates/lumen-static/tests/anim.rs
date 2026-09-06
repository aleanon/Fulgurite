//! T15.4: animation without a clock.
//!
//! The design's claim is that the core needs no timer — the host says
//! how much time passed and everything else is arithmetic. These tests
//! are that claim, plus the two behaviours an interrupted animation has
//! to get right.

use lumen_static::{Animated, Color, Easing};

#[test]
fn a_new_value_is_already_settled() {
    // A widget built at rest must not animate on its first frame, or
    // every window fades in by accident.
    let a = Animated::new(5.0f32);
    assert!(a.settled());
    assert_eq!(a.get(), 5.0);
}

#[test]
fn it_moves_over_the_duration_and_stops() {
    let mut a = Animated::new(0.0f32);
    a.duration = 100.0;
    a.easing = Easing::Linear;
    a.go(10.0);
    assert!(!a.settled(), "going somewhere");

    assert!(a.tick(50.0), "the value changed");
    assert!((a.get() - 5.0).abs() < 0.01, "halfway: {}", a.get());

    a.tick(50.0);
    assert!(a.settled());
    assert_eq!(a.get(), 10.0, "and lands exactly on the target");
    assert!(!a.tick(50.0), "a settled value reports no change");
}

/// **The behaviour that makes an interruption look right.**
#[test]
fn retargeting_starts_from_where_it_is_now() {
    let mut a = Animated::new(0.0f32);
    a.duration = 100.0;
    a.easing = Easing::Linear;
    a.go(10.0);
    a.tick(50.0);
    let midpoint = a.get();

    // Told to come back while halfway out. It must leave from the
    // middle, not snap to 10 first.
    a.go(0.0);
    assert!((a.get() - midpoint).abs() < 0.01, "no jump: {} vs {midpoint}", a.get());
    a.tick(100.0);
    assert_eq!(a.get(), 0.0);
}

#[test]
fn retargeting_to_the_current_target_does_nothing() {
    // A binding may set this every frame, so it must be idempotent or
    // the animation restarts forever and never arrives.
    let mut a = Animated::new(0.0f32);
    a.duration = 100.0;
    a.go(10.0);
    a.tick(50.0);
    let before = a.get();
    a.go(10.0);
    a.go(10.0);
    assert_eq!(a.get(), before, "restating the target is not a new animation");
}

#[test]
fn set_jumps_without_animating() {
    let mut a = Animated::new(0.0f32);
    a.set(7.0);
    assert!(a.settled());
    assert_eq!(a.get(), 7.0);
}

#[test]
fn a_stalled_host_does_not_overshoot() {
    // A host that stalls hands back a huge dt. The curve is defined on
    // 0..=1 and the value must land on the target, not past it.
    let mut a = Animated::new(0.0f32);
    a.duration = 100.0;
    a.go(10.0);
    a.tick(1_000_000.0);
    assert_eq!(a.get(), 10.0);
    assert!(a.settled());
}

#[test]
fn a_clock_that_goes_backwards_does_not_move_it() {
    let mut a = Animated::new(0.0f32);
    a.duration = 100.0;
    a.easing = Easing::Linear;
    a.go(10.0);
    a.tick(-50.0);
    assert_eq!(a.get(), 0.0, "negative time is not progress");
}

#[test]
fn a_degenerate_duration_settles_rather_than_dividing_by_zero() {
    for d in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let mut a = Animated::new(0.0f32);
        a.duration = d;
        a.go(10.0);
        assert!(a.settled(), "duration {d} must settle, not run forever");
        assert_eq!(a.get(), 10.0, "and read as the target");
    }
}

#[test]
fn easings_are_monotone_and_hit_both_ends() {
    for e in [Easing::Linear, Easing::EaseIn, Easing::EaseOut, Easing::EaseInOut] {
        assert!((e.apply(0.0)).abs() < 1e-6, "{e:?} starts at 0");
        assert!((e.apply(1.0) - 1.0).abs() < 1e-6, "{e:?} ends at 1");
        let mut last = -1.0;
        for k in 0..=20 {
            let v = e.apply(k as f32 / 20.0);
            assert!(v >= last - 1e-6, "{e:?} went backwards at {k}");
            last = v;
        }
        // Out of range clamps rather than exploding.
        assert_eq!(e.apply(-5.0), e.apply(0.0));
        assert_eq!(e.apply(5.0), e.apply(1.0));
    }
}

#[test]
fn colours_animate_too() {
    let mut a = Animated::new(Color(0, 0, 0, 255));
    a.duration = 100.0;
    a.easing = Easing::Linear;
    a.go(Color(100, 200, 50, 255));
    a.tick(50.0);
    let c = a.get();
    assert!(c.0 > 40 && c.0 < 60, "halfway on red: {c:?}");
    assert_eq!(c.3, 255, "alpha held");
}

#[test]
fn an_animated_value_costs_no_registry() {
    // The gating policy's second mechanism: a type, not a feature. A
    // tree with no `Animated<T>` field has no animation state, so there
    // is nothing to opt out of.
    assert_eq!(
        core::mem::size_of::<Animated<f32>>(),
        4 * 4 + core::mem::size_of::<Easing>().max(4)
    );
    fn assert_copy<T: Copy>() {}
    assert_copy::<Animated<f32>>();
}
