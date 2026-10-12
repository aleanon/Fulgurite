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
//! **The stack is built with `FontStack::from_static`** since M81/T81.3:
//! a `from_bytes` stack keeps no bytes and so cannot reach the shaper
//! whatever its mode, and the injection below ("take the `Complex` branch
//! whenever the feature is compiled") would have stayed green against it.
//! The last test runs the corpus on a stack in `Shaping::Complex`: K81a's
//! response routes every run with nothing to shape through fontdue, so it
//! folds to the same constant.
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
//! **No `Basic` glyph is phased** — M81/T81.8. Each placement also goes
//! through the round site both backends use, `text::pen_of`, and the
//! phase it chooses is counted: 0 over the corpus, on a `Basic` stack and
//! on a `Complex` one (whose runs here have nothing to shape), so every
//! `Basic` glyph keeps fontdue's 1× bitmap and no golden moves. Injection
//! (T81.8's commit): `pen_of` phases a phase-0 placement too; the count
//! prints the whole corpus.
//!
//! **Its injection is `cap_shift` flooring instead of rounding**, which
//! moves the checksum at every scale. Flooring a glyph's own `y` would
//! not: every `Basic` glyph sits on whole pixels (the fractional count
//! is printed, 0 at T81.0), so there is nothing to floor
//! (`docs/plan-shaped-paint.md`, "What building found").
#![cfg(feature = "truetype")]

use fulgurite::text::GlyphKey;
use fulgurite_shell::text::{pen_of, FontStack, GlyphId, ShapeScratch};

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

/// One scale's fold, the number of glyphs it placed, how many of its
/// narrowed runs came back taller than the free one (wrapped), how many
/// placements sit at a fractional x or y, and how many `pen_of` phases
/// (from a whole run origin, as a frame's is).
fn fold_scale(font: &FontStack, scratch: &mut ShapeScratch, scale: f32) -> (u64, usize, usize, usize, usize) {
    let mut h = Fnv::new();
    let mut glyphs = 0;
    let mut wrapped = 0;
    let mut fractional = 0;
    let mut phased = 0;
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
                        phased += usize::from(pen_of(10.0 + p.x, p).id.key().phase != 0);
                    }
                    glyphs += scratch.placed().len();
                }
            }
        }
    }
    (h.0, glyphs, wrapped, fractional, phased)
}

#[test]
fn every_basic_placement_folds_to_the_recorded_checksum() {
    let bytes = pinned::bytes();
    let font = FontStack::from_static(bytes, Some(bytes)).expect("the pinned face parses");
    let mut scratch = ShapeScratch::new();
    let mut total = Fnv::new();
    let mut glyphs = 0;
    let mut wrapped = 0;
    let mut by_scale = [0u64; 3];
    let mut phased = 0;
    for (i, &k) in SCALES.iter().enumerate() {
        let (sum, n, wr, fr, ph) = fold_scale(&font, &mut scratch, k);
        println!(
            "scale {k}: checksum {sum:#018x}, {n} glyphs placed, {wr} narrowed runs wrapped, \
             {fr} at a fractional x or y, {ph} phased by pen_of (recorded {:#018x})",
            BASIC_BY_SCALE[i]
        );
        phased += ph;
        wrapped += wr;
        by_scale[i] = sum;
        total.u64(sum);
        glyphs += n;
    }
    println!(
        "all scales: checksum {:#018x}, {glyphs} glyphs placed (recorded {BASIC_CHECKSUM:#018x}); \
         {phased} phased (bound 0)",
        total.0
    );
    assert!(glyphs > 0, "setup: the corpus placed no glyph");
    assert_eq!(phased, 0, "pen_of gave {phased} Basic glyphs a phase: they would leave fontdue's 1x bitmap");
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

/// **D1 (c): the core's `GlyphKey` is 8 B, and so is the shell's `GlyphId`
/// that wraps it** — M81/T81.1. `GlyphCache` and the GPU atlas key on the
/// id, so a wider id is a wider cache entry.
#[test]
fn glyph_key_and_glyph_id_stay_eight_bytes() {
    let (key, id) = (core::mem::size_of::<GlyphKey>(), core::mem::size_of::<GlyphId>());
    println!("size_of::<GlyphKey>() = {key} (bound 8), size_of::<GlyphId>() = {id} (bound 8)");
    assert_eq!(key, 8, "GlyphKey is {key} B, not 8");
    assert_eq!(id, 8, "GlyphId is {id} B, not 8");
}

/// **Every id the corpus places reads back as a `GlyphKey` field for
/// field** — M81/T81.1. The expected fields come from outside the id: the
/// size passed in, the face the weight picks on a stack whose bold face
/// is present (0 for 400, 1 for 700), `GlyphId::index` for the index, and
/// phase 0 (no phase before M81b). And the conversion is one to one over
/// the corpus: as many distinct keys as distinct ids, each key naming one
/// id. Injection: the conversion drops `face`; the gate names the field
/// and the first id.
#[test]
fn every_corpus_glyph_id_round_trips_through_its_glyph_key() {
    let bytes = pinned::bytes();
    let font = FontStack::from_static(bytes, Some(bytes)).expect("the pinned face parses");
    let mut scratch = ShapeScratch::new();
    let mut by_key = std::collections::HashMap::<GlyphKey, GlyphId>::new();
    let mut ids = std::collections::HashSet::<GlyphId>::new();
    let (mut checked, mut faces) = (0usize, [0usize; 2]);
    for &scale in &SCALES {
        for &t in &STRINGS {
            for &px in &SIZES {
                for &w in &WEIGHTS {
                    let px = px * scale;
                    font.place_run_into(&mut scratch, t, px, w, 0, f32::INFINITY);
                    let face = u8::from(w >= 700.0);
                    for p in scratch.placed() {
                        let k = GlyphKey::from(p.id);
                        let want = GlyphKey { px_bits: px.to_bits(), index: p.id.index(), face, phase: 0 };
                        for (field, got, exp) in [
                            ("px_bits", k.px_bits, want.px_bits),
                            ("index", k.index as u32, want.index as u32),
                            ("face", k.face as u32, want.face as u32),
                            ("phase", k.phase as u32, want.phase as u32),
                        ] {
                            assert_eq!(got, exp, "GlyphKey.{field} is {got}, expected {exp}, for the first id {:?} ({t:?} at {px} px, weight {w})", p.id);
                        }
                        if let Some(prev) = by_key.insert(k, p.id) {
                            assert_eq!(prev, p.id, "one GlyphKey {k:?} names two ids, {prev:?} and {:?}", p.id);
                        }
                        ids.insert(p.id);
                        faces[face as usize] += 1;
                        checked += 1;
                    }
                }
            }
        }
    }
    println!(
        "{checked} ids checked field for field (regular {}, bold {}); {} distinct ids, {} distinct keys",
        faces[0], faces[1], ids.len(), by_key.len()
    );
    assert!(checked > 0 && faces[1] > 0, "setup: the corpus placed no glyph, or none from the bold face");
    assert_eq!(ids.len(), by_key.len(), "distinct ids and distinct keys differ in number");
}

/// **K81a's response: a `Complex` stack places a run with nothing to shape
/// exactly as `Basic` does** — M81/T81.3 (`docs/plan-shaped-paint.md`,
/// `docs/results-text.md` M81/T81.3). The whole corpus on a stack in
/// `Shaping::Complex` folds to `BASIC_CHECKSUM`, and reaches the shaper 0
/// times. Then a mixed paragraph laid out as its runs: the Latin runs'
/// placements are identical to a `Basic` stack's, and the Hebrew run still
/// goes through the shaper (counter printed) and is placed right to left,
/// unlike `Basic`'s. Injection (T81.3's commit): `needs_shaping` always
/// answers yes; the Latin rows go back to the shaper and the checksum moves.
#[cfg(feature = "shaping")]
#[test]
fn a_complex_stack_places_runs_with_nothing_to_shape_as_basic_does() {
    use fulgurite_shell::text::{text_counters, Shaping};
    let bytes = pinned::bytes();
    let complex = FontStack::from_static(bytes, Some(bytes)).expect("parses").with_shaping(Shaping::Complex);
    let basic = FontStack::from_static(bytes, Some(bytes)).expect("parses");
    let mut scratch = ShapeScratch::new();
    let before = text_counters().complex;
    let mut total = Fnv::new();
    let mut glyphs = 0;
    for (i, &k) in SCALES.iter().enumerate() {
        let (sum, n, _, _, ph) = fold_scale(&complex, &mut scratch, k);
        println!(
            "Complex stack, scale {k}: checksum {sum:#018x} (recorded {:#018x}), {n} glyphs, {ph} phased (bound 0)",
            BASIC_BY_SCALE[i]
        );
        assert_eq!(ph, 0, "a run with nothing to shape was phased on a Complex stack at scale {k}");
        assert_eq!(sum, BASIC_BY_SCALE[i], "a Complex stack placed a run with nothing to shape off Basic's placement at scale {k}");
        total.u64(sum);
        glyphs += n;
    }
    let shaped = text_counters().complex - before;
    println!("Complex stack, all scales: checksum {:#018x} (recorded {BASIC_CHECKSUM:#018x}), {glyphs} glyphs; {shaped} runs through the shaper (bound 0)", total.0);
    assert!(glyphs > 0, "setup: the corpus placed no glyph");
    assert_eq!(shaped, 0, "plain Latin went through the shaper on a Complex stack");
    assert_eq!(total.0, BASIC_CHECKSUM, "the Complex stack's checksum moved");

    // A mixed paragraph, as the runs a label row paints.
    let runs = ["Taj Mahal, ", "\u{5e9}\u{5dc}\u{5d5}\u{5dd} \u{5e2}\u{5d5}\u{5dc}\u{5dd}", " quay j"];
    let place = |font: &FontStack, scratch: &mut ShapeScratch, t: &str| -> Vec<(u16, u32, u32)> {
        font.place_run_into(scratch, t, 16.0, 400.0, 0, f32::INFINITY);
        scratch.placed().iter().map(|p| (p.id.index(), p.x.to_bits(), p.y.to_bits())).collect()
    };
    let mut b = ShapeScratch::new();
    for t in runs {
        let before = text_counters().complex;
        let c = place(&complex, &mut scratch, t);
        let through = text_counters().complex - before;
        let p = place(&basic, &mut b, t);
        let hebrew = t.chars().any(|ch| ('\u{590}'..'\u{600}').contains(&ch));
        println!("{t:?}: {through} runs through the shaper; identical to Basic: {}", c == p);
        if hebrew {
            assert_eq!(through, 1, "setup: the Hebrew run did not go through the shaper");
            assert_ne!(c, p, "the Hebrew run was placed as Basic places it");
        } else {
            assert_eq!(through, 0, "the Latin run {t:?} went through the shaper");
            assert_eq!(c, p, "the Latin run {t:?} was placed off Basic's placement");
        }
    }
}
