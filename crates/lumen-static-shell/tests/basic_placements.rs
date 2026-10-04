//! **Every `Basic` placement, folded into one number** — M81/T81.0
//! (`docs/plan-shaped-paint.md`).
//!
//! M81 puts a second shaping path beside fontdue's inside
//! `FontStack::place_run_into`, behind a shell feature (M81's planned shaping) and
//! a stack's mode. Its promise to every application that does not ask for
//! it is that nothing moves: a `Basic` stack places every glyph exactly
//! where it did before the feature existed, with the feature compiled in
//! or not. This is the cross-build check of that promise, in the shape
//! `tests/banded.rs` uses: every placement of the corpus folded into one
//! checksum, asserted equal to a constant recorded at T81.0, so a build
//! with that feature (T81.3) and one without compare against the same number
//! rather than against each other inside one process.
//!
//! The corpus is `tests/text_containment.rs`' twelve strings at its ten
//! sizes and two weights, at scales 1, 1.25 and 2 (the run laid out at
//! `size × scale` physical px, as a scaled frame does), on the pinned
//! face as both the regular and the bold face so the weight picks a face
//! id. Each run is placed with no width limit at the face's own spacing
//! (`line_height` 0) and again inside 60 % of its own width, so wrapped
//! lines are folded too.
//!
//! Folded per glyph: the glyph index, `x` and `y` as `f32` bits, and the
//! bitmap's `w` and `h`; per run, the glyph count and the returned box.
//! The face id and size are not read from the id (it exposes only
//! `GlyphId::index` at T81.0, and its field order is T81.1's to change);
//! the size is the one passed in and folded per run.
//!
//! **Its injection is `cap_shift` flooring instead of rounding**, which
//! moves the checksum at every scale. Flooring a glyph's own `y` would
//! not: every `Basic` glyph sits on whole pixels (the fractional count
//! is printed, 0 at T81.0), so there is nothing to floor
//! (`docs/plan-shaped-paint.md`, "What building found").
#![cfg(feature = "truetype")]

use lumen_static_shell::text::{FontStack, ShapeScratch};

#[path = "fonts/pinned.rs"]
mod pinned;

/// `tests/text_containment.rs`' `STRINGS`, `SIZES` and `WEIGHTS`, restated
/// because a test file cannot import another's constants without running
/// its tests twice. The checksum changes if these do, so a drift is loud.
const STRINGS: [&str; 12] =
    ["Taj", "Tj", "fj", "Settings", "jejunj", "Hij", "POSITIVE", "Wolf", "gypsy", "quay j", "Taj Mahal", "W"];
const SIZES: [f32; 10] = [10.0, 11.0, 12.0, 13.0, 14.0, 16.0, 18.0, 20.0, 24.0, 32.0];
const WEIGHTS: [f32; 2] = [400.0, 700.0];
const SCALES: [f32; 3] = [1.0, 1.25, 2.0];

/// The fold at T81.0's parent (d4859b4), per scale and over all three.
/// A mismatch names the scale first, then the total.
const BASIC_BY_SCALE: [u64; 3] = [0x5ed8_be56_16d2_7ffd, 0xd2ae_c39c_e740_42e1, 0x59fa_2d5d_c546_9959];
const BASIC_CHECKSUM: u64 = 0x8664_44d1_064c_cda8;

struct Fnv(u64);
impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    fn u64(&mut self, v: u64) {
        for b in v.to_le_bytes() {
            self.0 = (self.0 ^ b as u64).wrapping_mul(0x100_0000_01b3);
        }
    }
}

/// One scale's fold, the number of glyphs it placed, and how many of
/// its narrowed runs came back taller than the free one (wrapped).
fn fold_scale(font: &FontStack, scratch: &mut ShapeScratch, scale: f32) -> (u64, usize, usize, usize) {
    let mut h = Fnv::new();
    let mut glyphs = 0;
    let mut wrapped = 0;
    let mut fractional = 0;
    for &t in &STRINGS {
        for &px in &SIZES {
            for &w in &WEIGHTS {
                let px = px * scale;
                let free = font.place_run_into(scratch, t, px, w, 0, f32::INFINITY);
                for max_w in [f32::INFINITY, (free.w * 0.6).floor()] {
                    let size = font.place_run_into(scratch, t, px, w, 0, max_w);
                    wrapped += usize::from(size.h > free.h);
                    h.u64(px.to_bits() as u64);
                    h.u64(scratch.placed().len() as u64);
                    h.u64(size.w.to_bits() as u64);
                    h.u64(size.h.to_bits() as u64);
                    for p in scratch.placed() {
                        h.u64(p.id.index() as u64);
                        h.u64(p.x.to_bits() as u64);
                        h.u64(p.y.to_bits() as u64);
                        h.u64(p.w as u64);
                        h.u64(p.h as u64);
                        fractional += usize::from(p.x.fract() != 0.0 || p.y.fract() != 0.0);
                    }
                    glyphs += scratch.placed().len();
                }
            }
        }
    }
    (h.0, glyphs, wrapped, fractional)
}

#[test]
fn every_basic_placement_folds_to_the_recorded_checksum() {
    let bytes = pinned::bytes();
    let font = FontStack::from_bytes(bytes, Some(bytes)).expect("the pinned face parses");
    let mut scratch = ShapeScratch::new();
    let mut total = Fnv::new();
    let mut glyphs = 0;
    let mut wrapped = 0;
    let mut by_scale = [0u64; 3];
    for (i, &k) in SCALES.iter().enumerate() {
        let (sum, n, wr, fr) = fold_scale(&font, &mut scratch, k);
        println!(
            "scale {k}: checksum {sum:#018x}, {n} glyphs placed, {wr} narrowed runs wrapped, \
             {fr} at a fractional x or y (recorded {:#018x})",
            BASIC_BY_SCALE[i]
        );
        wrapped += wr;
        by_scale[i] = sum;
        total.u64(sum);
        glyphs += n;
    }
    println!("all scales: checksum {:#018x}, {glyphs} glyphs placed (recorded {BASIC_CHECKSUM:#018x})", total.0);
    assert!(glyphs > 0, "setup: the corpus placed no glyph");
    assert!(wrapped > 0, "setup: no narrowed run wrapped, so no wrapped line is folded");
    for (i, &k) in SCALES.iter().enumerate() {
        assert_eq!(
            by_scale[i], BASIC_BY_SCALE[i],
            "a Basic placement moved at scale {k}: checksum {:#018x}, recorded {:#018x}",
            by_scale[i], BASIC_BY_SCALE[i]
        );
    }
    assert_eq!(total.0, BASIC_CHECKSUM, "the Basic placements' checksum moved");
}
