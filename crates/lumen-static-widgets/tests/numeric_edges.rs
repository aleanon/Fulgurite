//! M55/T55.7 (R16-F11): the range widgets keep their last good value.
//!
//! `clamp_finite` returned the range's *minimum* for a non-finite value
//! while its own rustdoc said "the last good value is the honest
//! answer", so a NaN step reset a `NumberInput` to its minimum and a
//! `RangeSlider`'s handle to 0. With `min > max` a number field pinned to
//! `min`, above the stated maximum. And `SplitPane::set_split` clamped
//! through `num::clamp`, which passes a NaN through, so `split()` could
//! report NaN.
//!
//! Each row prints its value beside the one it must keep. The last two
//! tests are the accepting direction: a finite step still moves, and a
//! step of 0 is still a no-op.

use lumen_static::event::{Event, EventCtx, Key};
use lumen_static::widget::Node;
use lumen_static_widgets::ops::Fixed;
use lumen_static_widgets::{NumberInput, RangeSlider, SplitPane};

fn key(n: &mut dyn Node, k: Key) -> bool {
    let ev = Event::Key { key: k, down: true };
    n.on_event(&ev, &EventCtx::for_event(0, ev))
}

#[test]
#[ignore = "red until T55.7: R16-F11 (a NaN step resets a NumberInput to its minimum)"]
fn a_nan_step_keeps_the_number() {
    let mut n = NumberInput::new(5.0, 0.0, 10.0, 1.0);
    n.step = f32::NAN;
    key(&mut n, Key::Up);
    println!("after Up: {} (last good 5)", n.value());
    assert_eq!(n.value(), 5.0, "a NaN step moved the value");
}

#[test]
#[ignore = "red until T55.7: R16-F11 (a NaN step resets a RangeSlider handle to 0)"]
fn a_nan_step_keeps_the_range() {
    let mut r = RangeSlider::new(0.3, 0.7);
    r.step = f32::NAN;
    key(&mut r, Key::Right);
    key(&mut r, Key::Left);
    println!("after Right, Left: ({}, {}) (last good (0.3, 0.7))", r.low(), r.high());
    assert_eq!((r.low(), r.high()), (0.3, 0.7), "a NaN step moved a handle");
}

#[test]
#[ignore = "red until T55.7: R16-F11 (min > max pins the value to min)"]
fn unordered_bounds_are_ordered_once() {
    // `min 10, max 0`: the range is [0, 10], and 5 is inside it. The old
    // clamp pinned it to 10 -- inside [0, 10] as well, which is why the
    // plan's "a value inside [0, 10]" could not fail; it is the value
    // that must survive, and the bounds that must be put in order.
    let n = NumberInput::new(5.0, 10.0, 0.0, 1.0);
    println!("min 10, max 0, value 5: value {} in [{}, {}]", n.value(), n.min, n.max);
    assert_eq!((n.min, n.max), (0.0, 10.0), "the bounds were not ordered");
    assert_eq!(n.value(), 5.0, "a value inside the ordered range moved");
}

#[test]
#[ignore = "red until T55.7: R16-F11 (SplitPane stores a NaN fraction)"]
fn a_nan_split_is_refused() {
    let mut p: SplitPane<Fixed, Fixed> = SplitPane::default();
    let before = p.split();
    let moved = p.set_split(f32::NAN);
    println!("set_split(NaN): returned {moved}, split {} (was {before})", p.split());
    assert!(!moved, "set_split(NaN) reported a move");
    assert_eq!(p.split(), before, "set_split(NaN) changed the split");
}

#[test]
fn a_finite_step_still_moves_and_zero_does_not() {
    let mut n = NumberInput::new(5.0, 0.0, 10.0, 1.0);
    key(&mut n, Key::Up);
    println!("step 1, Up: {} (expected 6)", n.value());
    assert_eq!(n.value(), 6.0);
    n.step = 0.0;
    key(&mut n, Key::Up);
    println!("step 0, Up: {} (expected 6)", n.value());
    assert_eq!(n.value(), 6.0);
    let mut p: SplitPane<Fixed, Fixed> = SplitPane::default();
    assert!(p.set_split(0.25), "a finite split must move");
    assert_eq!(p.split(), 0.25);
}
