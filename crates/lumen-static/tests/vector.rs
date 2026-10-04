//! T15.3: lines and gradients, both allocator-free.
//!
//! The split this task makes is by *cost*: a line needs no buffer, a
//! gradient's stops are inline and bounded, and only arbitrary paths
//! need somewhere to put vertices. So the first two are in every
//! profile and the third is gated — and these tests are about the first
//! two staying that way.

use lumen_static::display::{Stops, MAX_STOPS};
use lumen_static::{Cmd, Color, DisplayList, Painter, Point};

const RED: Color = Color(255, 0, 0, 255);
const BLUE: Color = Color(0, 0, 255, 255);

#[test]
fn a_line_is_a_command() {
    let mut l = DisplayList::new();
    l.line(Point { x: 0.0, y: 0.0 }, Point { x: 10.0, y: 5.0 }, 2.0, RED);
    assert!(matches!(l.cmds[0], Cmd::Line { width: 2.0, .. }));
}

#[test]
fn a_degenerate_line_is_dropped() {
    // The same rule every other command follows: nothing with no extent
    // reaches the list, so a renderer never has to test for it.
    let mut l = DisplayList::new();
    l.line(Point { x: 1.0, y: 1.0 }, Point { x: 1.0, y: 1.0 }, 2.0, RED);
    l.line(Point { x: 0.0, y: 0.0 }, Point { x: 5.0, y: 0.0 }, 0.0, RED);
    assert!(l.cmds.is_empty(), "zero-length and zero-width both drop");
}

#[test]
fn two_stops_interpolate_and_the_ends_are_exact() {
    let s = Stops::two(RED, BLUE);
    assert_eq!(s.sample(0.0), RED, "the first stop is exact");
    assert_eq!(s.sample(1.0), BLUE, "and so is the last");
    let mid = s.sample(0.5);
    assert!(mid.0 > 100 && mid.0 < 160 && mid.2 > 100 && mid.2 < 160, "halfway: {mid:?}");
}

#[test]
fn sampling_outside_the_range_clamps_rather_than_wrapping() {
    let s = Stops::two(RED, BLUE);
    assert_eq!(s.sample(-1.0), RED);
    assert_eq!(s.sample(2.0), BLUE);
    // NaN must not produce a colour out of nowhere. `num::clamp` maps
    // it to itself and the first-stop test catches it.
    assert_eq!(s.sample(f32::NAN), BLUE, "total, not a panic");
}

#[test]
fn stops_past_len_are_ignored() {
    let mut s = Stops::two(RED, BLUE);
    s.color[2] = Color(0, 255, 0, 255);
    s.at[2] = 0.5;
    // len is still 2, so the green never appears.
    let mid = s.sample(0.5);
    assert!(mid.1 < 200, "a stop past len is not sampled: {mid:?}");
}

#[test]
fn an_empty_gradient_is_transparent_rather_than_black() {
    let s = Stops { at: [0.0; MAX_STOPS], color: [RED; MAX_STOPS], len: 0 };
    assert_eq!(s.sample(0.5), Color(0, 0, 0, 0), "nothing to draw, so nothing is drawn");
}

#[test]
fn a_painter_without_gradients_falls_back_to_a_flat_fill() {
    // The provided body is a flat fill of the first stop, not a no-op:
    // a missing background is a worse failure than an un-graded one.
    struct Flat(Vec<Color>);
    impl Painter for Flat {
        fn rect(&mut self, _: [f32; 4], c: Color) {
            self.0.push(c);
        }
        fn text_spaced(&mut self, _: [f32; 4], _: &str, _: f32, _: f32, _: u8, _: Color) {}
        fn rounded_rect(&mut self, _: [f32; 4], _: Color, _: f32) {}
        fn shadow(&mut self, _: [f32; 4], _: f32, _: f32) {}
        fn stroked_rect(&mut self, _: [f32; 4], _: Color, _: f32, _: f32) {}
        fn push_clip(&mut self, _: [f32; 4]) {}
        fn pop_clip(&mut self) {}
    }
    let mut f = Flat(Vec::new());
    f.gradient([0.0, 0.0, 10.0, 10.0], Stops::two(RED, BLUE), 0.0);
    assert_eq!(f.0, vec![RED], "the first stop, as a plain rect");
}

#[test]
fn the_vocabulary_stays_allocator_free() {
    // Stops are inline, so a gradient needs no buffer and `Cmd` stays
    // `Copy` — which is what keeps both of these in the `no_std`
    // profile alongside images.
    fn assert_copy<T: Copy>() {}
    assert_copy::<Stops>();
    assert_copy::<Cmd>();
    assert_eq!(core::mem::size_of::<Stops>(), MAX_STOPS * 4 + MAX_STOPS * 4 + 4);
}
