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
    s.at[2] = 128;
    // len is still 2, so the green never appears.
    let mid = s.sample(0.5);
    assert!(mid.1 < 200, "a stop past len is not sampled: {mid:?}");
}

#[test]
fn an_empty_gradient_is_transparent_rather_than_black() {
    let s = Stops { at: [0; MAX_STOPS], color: [RED; MAX_STOPS], len: 0 };
    assert_eq!(s.sample(0.5), Color(0, 0, 0, 0), "nothing to draw, so nothing is drawn");
}

#[test]
fn a_painter_without_gradients_falls_back_to_a_flat_fill() {
    // The provided body is a flat fill of the first stop, not a no-op:
    // a missing background is a worse failure than an un-graded one.
    // Since M82/T82.2 it is rounded by the gradient's corner, so a
    // rounded card's fallback keeps its shape.
    struct Flat(Vec<(Color, f32)>);
    impl Painter for Flat {
        // M82/T82.3: a leaf with no fill table.
        fn fills(&self) -> &[lumen_static::style::Fill] {
            &[]
        }
        fn rect(&mut self, _: [f32; 4], c: Color) {
            self.0.push((c, -1.0));
        }
        fn text_spaced(&mut self, _: [f32; 4], _: &str, _: f32, _: f32, _: u8, _: Color) {}
        fn rounded_rect(&mut self, _: [f32; 4], c: Color, radius: f32) {
            self.0.push((c, radius));
        }
        fn shadow(&mut self, _: [f32; 4], _: f32, _: f32) {}
        fn stroked_rect(&mut self, _: [f32; 4], _: Color, _: f32, _: f32) {}
        fn push_clip(&mut self, _: [f32; 4]) {}
        fn pop_clip(&mut self) {}
        fn path(&mut self, _: &[lumen_static::display::PathEl], _: lumen_static::display::Affine, _: lumen_static::display::PathPaint) {}
    }
    let mut f = Flat(Vec::new());
    let radial = lumen_static::display::GradShape::Radial { center: [0.5, 0.5], radius: 1.0 };
    f.gradient([0.0, 0.0, 10.0, 10.0], Stops::two(RED, BLUE), radial, 6.0);
    assert_eq!(f.0, vec![(RED, 6.0)], "the first stop, as a rect rounded by the gradient's corner");
}

#[test]
fn the_vocabulary_stays_allocator_free() {
    // Stops are inline, so a gradient needs no buffer and `Cmd` stays
    // `Copy` — which is what keeps both of these in the `no_std`
    // profile alongside images.
    fn assert_copy<T: Copy>() {}
    assert_copy::<Stops>();
    assert_copy::<Cmd>();
    // M82/T82.1: an offset is one byte in 1/255 steps, so a stop is 5 B
    // and `Stops` is 4 x 1 + 4 x 4 + 1 = 21 B with align 1 (it was 36 with
    // `f32` offsets). That is the room M82/T82.2's wider `Gradient` uses.
    let size = core::mem::size_of::<Stops>();
    println!("Stops is {size} B (want 21: {MAX_STOPS} offsets x 1 B, {MAX_STOPS} colours x 4 B, len 1 B)");
    assert_eq!(size, 21, "Stops is not 21 B; the Gradient payload's arithmetic rests on it");
}

/// The `f32` sampler `Stops::sample` replaced (M82/T82.1), kept as the
/// oracle: the same arithmetic over offsets given as fractions.
fn sample_f32(at: [f32; MAX_STOPS], color: [Color; MAX_STOPS], len: u8, t: f32) -> Color {
    let n = (len as usize).min(MAX_STOPS);
    if n == 0 {
        return Color(0, 0, 0, 0);
    }
    let t = t.clamp(0.0, 1.0);
    if t <= at[0] {
        return color[0];
    }
    for k in 1..n {
        if t <= at[k] {
            let span = at[k] - at[k - 1];
            let f = if span > 0.0 { (t - at[k - 1]) / span } else { 0.0 };
            let (a, b) = (color[k - 1], color[k]);
            let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * f) as u8;
            return Color(l(a.0, b.0), l(a.1, b.1), l(a.2, b.2), l(a.3, b.3));
        }
    }
    color[n - 1]
}

/// Max channel delta of `Stops::sample` against the `f32` oracle over
/// 1,001 values of `t`, and how many of them differ at all.
fn delta_against_f32(bytes: [u8; MAX_STOPS], fractions: [f32; MAX_STOPS], len: u8) -> (u8, usize) {
    let color = [Color(0, 0, 0, 255), Color(255, 255, 255, 255), Color(0, 64, 255, 255), Color(0, 0, 0, 255)];
    let s = Stops { at: bytes, color, len };
    let (mut max, mut differ) = (0u8, 0usize);
    for i in 0..=1000 {
        let t = i as f32 / 1000.0;
        let (a, b) = (s.sample(t), sample_f32(fractions, color, len, t));
        let d = [a.0.abs_diff(b.0), a.1.abs_diff(b.1), a.2.abs_diff(b.2), a.3.abs_diff(b.3)].into_iter().max().unwrap();
        max = max.max(d);
        differ += (d > 0) as usize;
    }
    (max, differ)
}

/// M82/T82.1: **1/255 offsets sample as the `f32` ones did.** An offset
/// that is a multiple of 1/255 is the same `f32` after the division, so
/// the colour is identical at every `t`; 0.5 is not (it becomes 128, 0.502)
/// and moves by at most one channel step.
#[test]
fn byte_offsets_sample_as_the_f32_ones_did() {
    // Representable: 0, 51/255 (= 0.2), 1.
    let (max, differ) = delta_against_f32([0, 51, 255, 0], [0.0, 51.0 / 255.0, 1.0, 0.0], 3);
    println!("offsets 0, 51/255, 1: max channel delta {max} over 1,001 t ({differ} differ; bound 0)");
    assert_eq!(max, 0, "an offset exact in 1/255 sampled differently from its f32");
    let (max2, _) = delta_against_f32([0, 255, 0, 0], [0.0, 1.0, 0.0, 0.0], 2);
    println!("Stops::two's 0, 1: max channel delta {max2} (bound 0)");
    assert_eq!(max2, 0, "a two-stop gradient sampled differently");
    // Not representable: 0.5 rounds to 128.
    let (max, differ) = delta_against_f32([0, 128, 255, 0], [0.0, 0.5, 1.0, 0.0], 3);
    println!("offset 0.5 as 128: max channel delta {max} over 1,001 t, {differ} of them differ (bound 1)");
    assert!(max <= 1, "a half-step offset error moved a channel by {max}");
}
