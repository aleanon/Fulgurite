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
/// NaN in gives NaN out, and the bounds are inclusive. Where
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
