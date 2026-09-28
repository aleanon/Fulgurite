//! `floor` and `ceil` without `std`.
//!
//! Two operations at four call sites (`cx.rs`, `repeat.rs`) are the
//! whole of this core's dependence on std's float math — `.abs()`,
//! `.min()` and `.max()` are already in `core`. Four calls do not
//! justify a `libm` dependency, so M13 supplies them here
//! (`docs/plan-embedded.md` T13.2).
//!
//! **These are bit-exact with `f32::floor`/`f32::ceil` over all 2³²
//! inputs**, which `exhaustive_agreement_with_std` proves rather than
//! samples. That gate is why the implementations below are written
//! domain-complete instead of as the obvious `x as i32 as f32`: the
//! obvious form disagrees with std for every finite input with
//! |x| ≥ 2³¹, for ±inf, for NaN, and for −0.0 — and those inputs are
//! reachable, because `Repeat::scroll` is a public field and
//! `scroll / item_extent` can be ±inf or NaN through the public API.

/// True when `x` can have a fractional part *and* fit an `i32` —
/// i.e. |x| < 2²³.
///
/// Bit math rather than `x.abs() < 8_388_608.0` for two reasons: it is
/// `const`, and NaN and ±inf answer `false` by falling out of the same
/// comparison rather than needing a separate arm, since an all-ones
/// exponent field compares larger than 2²³'s. Every value it rejects is
/// already integral, so both functions below return it unchanged.
const fn has_fraction(x: f32) -> bool {
    (x.to_bits() & 0x7fff_ffff) < 0x4b00_0000
}

/// Zero with `x`'s sign. IEEE 754 says `floor(-0.3)` is `-0.0`, not
/// `+0.0`, and the exhaustive gate compares bit patterns — so this is
/// the difference between passing and failing it for the ~2³⁰ inputs
/// that round to a zero.
const fn zero_like(x: f32) -> f32 {
    f32::from_bits(x.to_bits() & 0x8000_0000)
}

/// The largest integer not greater than `x`. Bit-exact with `f32::floor`.
pub const fn floor(x: f32) -> f32 {
    if !has_fraction(x) {
        return x;
    }
    let t = x as i32 as f32;
    let r = if t > x { t - 1.0 } else { t };
    if r == 0.0 {
        zero_like(x)
    } else {
        r
    }
}

/// `x` modulo 1, always in `[0, 1)` — the fractional part of a phase.
///
/// `f32::rem_euclid` is **std-only**, like every other float method
/// here, and two call sites do not justify a `libm` dependency — the
/// same reasoning [`floor`] and [`sqrt`] are argued from.
///
/// Built on [`floor`] rather than on a truncation, which is what makes
/// it right for negatives: `-0.25` gives `0.75`, where `x - x.trunc()`
/// would give `-0.25` and put a phase behind its own start. An
/// animation that ran backwards for the first frame after a wrap is the
/// symptom that distinction prevents.
///
/// Non-finite input gives `0.0`: a phase has nowhere to be otherwise,
/// and returning `NaN` would propagate into a position.
pub fn fract_euclid(x: f32) -> f32 {
    if !x.is_finite() {
        return 0.0;
    }
    let f = x - floor(x);
    // `floor` is exact, so `f` is already in `[0, 1)` — except that a
    // large `x` can round `x - floor(x)` to exactly 1.0.
    if f >= 1.0 {
        0.0
    } else {
        f
    }
}

/// `x` rounded to the nearest integer, halves up (toward +∞) — M57/A15.
///
/// For snapping geometry to the pixel grid, where "halves up" is what
/// keeps a 1 px edge pair at 428.5–429.5 exactly one pixel (429–430)
/// rather than splitting its halves in opposite directions. Built on
/// [`floor`] for the reason every function here is: `f32::round` is not
/// in `core`. Non-finite input is returned unchanged.
pub fn round(x: f32) -> f32 {
    if !x.is_finite() {
        return x;
    }
    floor(x + 0.5)
}

/// The smallest integer not less than `x`. Bit-exact with `f32::ceil`.
pub const fn ceil(x: f32) -> f32 {
    if !has_fraction(x) {
        return x;
    }
    let t = x as i32 as f32;
    let r = if t < x { t + 1.0 } else { t };
    if r == 0.0 {
        zero_like(x)
    } else {
        r
    }
}

/// `x` limited to `[lo, hi]`, without `f32::clamp`'s panic path.
///
/// **This exists for a measured reason.** `f32::clamp` panics when
/// `lo > hi` or either bound is NaN, and its message *formats a float* —
/// which links `core::num::flt2dec` into the binary. On
/// `thumbv7em-none-eabihf` that is **14 KB of a 43 KB firmware, a third
/// of `.text`**, for a branch no frame ever takes
/// (`docs/results-embedded.md`).
///
/// Behaviour matches `f32::clamp` everywhere `f32::clamp` returns:
/// NaN in gives NaN out, and the bounds are inclusive. **So a caller
/// that stores the result guards against NaN itself** -- `SplitPane`'s
/// `set_split` did not, and stored one (M55/T55.7, R16-F11). Where
/// `f32::clamp` *panics* — `lo > hi`, or a NaN bound — this is total
/// instead, which is the better failure for a frame loop: a GUI that
/// stops drawing is worse than one that draws a degenerate rect.
#[inline]
pub fn clamp(x: f32, lo: f32, hi: f32) -> f32 {
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

/// Square root, without `std`.
///
/// `f32::sqrt` is std-only — it lowers to an intrinsic that needs
/// `libm` on a bare target — and T15.5's pinch recognizer needs a
/// distance. Same reasoning as [`floor`] and [`ceil`]: two call sites do
/// not justify a `libm` dependency.
///
/// **Not bit-exact with `f32::sqrt`, and this one says so.** `floor` and
/// `ceil` are exact because they are selections; a square root is an
/// approximation, and claiming otherwise would be the kind of drift
/// `AGENT.md` exists to stop. The implementation is the classic
/// bit-halving seed plus three Newton steps, and
/// `sqrt_is_accurate_enough_to_measure_with` pins the error it actually
/// achieves rather than the one it aspires to.
///
/// Domain: negatives and NaN give NaN, `+inf` gives `+inf`, and both
/// zeroes give themselves — matching `f32::sqrt` on every one.
pub fn sqrt(x: f32) -> f32 {
    if x.is_nan() || x < 0.0 {
        return f32::NAN;
    }
    if x == 0.0 || !x.is_finite() {
        // Both zeroes keep their sign, as IEEE says, and +inf is its
        // own root.
        return x;
    }
    // Halve the exponent by halving the bit pattern about the bias —
    // an initial guess within a factor of about 1.03.
    let mut y = f32::from_bits(0x1fbd_1df5 + (x.to_bits() >> 1));
    // Three Newton steps. Two leaves visible error at the top of the
    // range; four buys nothing a `f32` can represent.
    y = 0.5 * (y + x / y);
    y = 0.5 * (y + x / y);
    y = 0.5 * (y + x / y);
    y
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The values a hand-written table would be built from: the ones
    /// where the implementation branches, and the ones a cast gets
    /// wrong. Cheap enough for the every-commit gate; the exhaustive
    /// test below is the real proof.
    const INTERESTING: &[f32] = &[
        0.0,
        -0.0,
        0.5,
        -0.5,
        1.0,
        -1.0,
        1.5,
        -1.5,
        f32::MIN_POSITIVE,
        -f32::MIN_POSITIVE,
        8_388_607.5,
        -8_388_607.5,
        8_388_608.0, // 2^23: the boundary has_fraction tests
        -8_388_608.0,
        2_147_483_520.0, // largest f32 below 2^31, where the cast breaks
        -2_147_483_648.0,
        4e9, // beyond i32, which `as i32` saturates
        -4e9,
        f32::MAX,
        f32::MIN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
    ];

    #[test]
    fn agrees_with_std_at_the_interesting_values() {
        for &x in INTERESTING {
            assert_eq!(floor(x).to_bits(), x.floor().to_bits(), "floor({x})");
            assert_eq!(ceil(x).to_bits(), x.ceil().to_bits(), "ceil({x})");
        }
    }

    #[test]
    fn signed_zero_survives() {
        // The divergence a range-restricted cast leaves behind, and the
        // reason `zero_like` exists: both of these are `+0.0` without it.
        assert!(floor(-0.0).is_sign_negative());
        assert!(ceil(-0.5).is_sign_negative());
        assert!(floor(0.5).is_sign_positive());
    }

    #[test]
    fn clamp_matches_std_where_std_answers() {
        for &(x, lo, hi) in &[
            (0.5f32, 0.0f32, 1.0f32),
            (-1.0, 0.0, 1.0),
            (2.0, 0.0, 1.0),
            (0.0, 0.0, 1.0),
            (1.0, 0.0, 1.0),
            (f32::INFINITY, 0.0, 1.0),
            (f32::NEG_INFINITY, 0.0, 1.0),
            (-3.0, -2.0, 5.0),
        ] {
            assert_eq!(clamp(x, lo, hi), x.clamp(lo, hi), "clamp({x}, {lo}, {hi})");
        }
        // NaN in, NaN out -- the one non-panicking NaN case std has.
        assert!(clamp(f32::NAN, 0.0, 1.0).is_nan());
    }

    #[test]
    fn clamp_is_total_where_std_panics() {
        // `f32::clamp(_, 1.0, 0.0)` panics; this returns a bound. A
        // frame loop that draws a degenerate rect beats one that stops.
        assert_eq!(clamp(0.5, 1.0, 0.0), 1.0);
        assert_eq!(clamp(0.5, f32::NAN, 1.0), 0.5);
    }

    /// The error bound `sqrt` actually achieves, pinned rather than
    /// asserted to be zero — it is an approximation and the doc says so.
    /// `fract_euclid` is **Euclidean**, which is the whole reason it is
    /// not `x - x.trunc()`.
    ///
    /// The negative cases are the test: a truncating version gives
    /// `-0.25` for `-0.25` and puts a phase a quarter-turn behind its
    /// own start, so an animation runs backwards for the first frame
    /// after it wraps. Every assertion below except the first two would
    /// pass against a truncating implementation.
    #[test]
    fn fract_euclid_is_euclidean_not_truncating() {
        // The ordinary range, where both spellings agree.
        assert_eq!(fract_euclid(0.25), 0.25);
        assert_eq!(fract_euclid(2.5), 0.5);

        // The negatives, where they do not.
        assert_eq!(fract_euclid(-0.25), 0.75, "a truncating version gives -0.25");
        assert_eq!(fract_euclid(-1.5), 0.5, "and -0.5 here");
        assert_eq!(fract_euclid(-3.0), 0.0, "an exact negative integer is a zero phase");

        // The range is closed at 0 and open at 1, at both ends and for
        // a magnitude big enough that `x - floor(x)` can round up.
        for x in [0.0, -0.0, 1.0, -1.0, 1e7, -1e7, 8_388_608.0, -8_388_608.0] {
            let f = fract_euclid(x);
            assert!((0.0..1.0).contains(&f), "fract_euclid({x}) = {f} is outside [0, 1)");
        }

        // Non-finite has nowhere to be; NaN must not reach a position.
        for x in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(fract_euclid(x), 0.0, "fract_euclid({x}) must not propagate");
        }
    }

    #[test]
    fn sqrt_is_accurate_enough_to_measure_with() {
        let mut worst = 0.0f32;
        // A wide sample: subnormals, the ordinary range, and the top.
        let mut x = f32::MIN_POSITIVE;
        while x < 1e30 {
            let got = sqrt(x);
            let want = x.sqrt();
            let rel = ((got - want) / want).abs();
            worst = worst.max(rel);
            x *= 1.0009;
        }
        assert!(worst < 1e-6, "relative error {worst:e} over the range");

        for (x, want) in [(0.0f32, 0.0f32), (1.0, 1.0), (4.0, 2.0), (2.25, 1.5)] {
            assert!((sqrt(x) - want).abs() < 1e-6, "sqrt({x})");
        }
    }

    #[test]
    fn sqrt_matches_std_on_the_special_values() {
        assert!(sqrt(f32::NAN).is_nan());
        assert!(sqrt(-1.0).is_nan(), "as f32::sqrt does");
        assert_eq!(sqrt(f32::INFINITY), f32::INFINITY);
        assert_eq!(sqrt(0.0).to_bits(), 0.0f32.to_bits());
        assert_eq!(sqrt(-0.0).to_bits(), (-0.0f32).to_bits(), "signed zero survives");
    }

    #[test]
    fn usable_in_const_context() {
        const F: f32 = floor(-1.5);
        const C: f32 = ceil(-1.5);
        assert_eq!((F, C), (-2.0, -1.0));
    }

    /// Every `f32` there is. `#[ignore]` because it is ~4.3 billion
    /// iterations: seconds in release, minutes in the gate's debug
    /// build. `just floats` runs it, and `just check` runs that.
    #[test]
    #[ignore = "exhaustive: run via `just floats` (release), not the debug gate"]
    fn exhaustive_agreement_with_std() {
        for bits in 0..=u32::MAX {
            let x = f32::from_bits(bits);
            assert_eq!(floor(x).to_bits(), x.floor().to_bits(), "floor({x}) bits {bits:#x}");
            assert_eq!(ceil(x).to_bits(), x.ceil().to_bits(), "ceil({x}) bits {bits:#x}");
        }
    }
}
