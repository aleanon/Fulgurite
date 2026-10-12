//! T15.8: that the complex stack shapes what the default one cannot,
//! and that it stays out of everyone else's way.
//!
//! The font is the committed test face (M43/T43.11), so these run on
//! every machine; until then they skipped, passing, without a system face.

use fulgurite::cx::{Cluster, GlyphSink, TextMeasure};
use fulgurite_text::ComplexText;

#[derive(Default)]
struct Clusters(Vec<Cluster>);

impl GlyphSink for Clusters {
    fn cluster(&mut self, c: Cluster) {
        self.0.push(c);
    }
}

#[path = "../../fulgurite-shell/tests/fonts/pinned.rs"]
mod pinned;

/// The committed face (M43/T43.11). These used to try two system paths
/// and return early -- passing -- when neither existed.
fn face() -> ComplexText {
    ComplexText::new(pinned::bytes(), 16.0).expect("the pinned face parses")
}

#[test]
fn latin_still_measures_sensibly() {
    let mut t = face();
    let s = t.measure("hello", 16.0, f32::INFINITY);
    assert!(s.w > 10.0 && s.w < 200.0, "a plausible width: {}", s.w);
    assert!(s.h > 0.0);
}

#[test]
fn an_empty_string_still_reports_a_caret_position() {
    let mut t = face();
    let mut c = Clusters::default();
    t.shape("", 16.0, 400.0, f32::INFINITY, &mut c);
    assert_eq!(c.0.len(), 1);
    assert_eq!(c.0[0].byte, 0);
}

/// The bug this guards: a caret computed from a shaper's *own* indices
/// lands mid-character. `TextMeasure::shape`'s contract is that byte
/// offsets are the caller's, and a bidi split is where that is easiest
/// to get wrong.
#[test]
fn cluster_offsets_index_the_callers_string() {
    let mut t = face();
    let text = "abc";
    let mut c = Clusters::default();
    t.shape(text, 16.0, 400.0, f32::INFINITY, &mut c);
    for cl in &c.0 {
        assert!(
            text.is_char_boundary(cl.byte as usize),
            "offset {} is not a char boundary of {text:?}",
            cl.byte
        );
    }
    assert_eq!(c.0.last().unwrap().byte as usize, text.len(), "and a trailing caret");
}

/// **The thing the default stack cannot do.**
#[test]
fn a_right_to_left_run_is_laid_out_and_stays_on_boundaries() {
    let mut t = face();
    let text = "\u{5e9}\u{5dc}\u{5d5}\u{5dd}";
    let mut c = Clusters::default();
    let size = t.shape(text, 16.0, 400.0, f32::INFINITY, &mut c);
    assert!(size.w > 0.0, "it measured something");
    assert!(c.0.len() > 1, "and produced clusters");
    for cl in &c.0 {
        assert!(text.is_char_boundary(cl.byte as usize), "offsets stay on boundaries");
    }
}

#[test]
fn a_mixed_line_produces_runs_in_both_directions() {
    let mut t = face();
    // A Hebrew word inside an English sentence: two runs reading in
    // opposite directions, which `BakedFace` and `FontMetrics` cannot
    // express at all.
    let text = "a \u{5e9}\u{5dc}\u{5d5}\u{5dd} b";
    let mut c = Clusters::default();
    let size = t.shape(text, 16.0, 400.0, f32::INFINITY, &mut c);
    assert!(size.w > 0.0);
    for cl in &c.0 {
        assert!(text.is_char_boundary(cl.byte as usize));
    }
    // Since M81/T81.2 clusters arrive in source order with leading-edge
    // x (`Cluster`'s contract), so two directions show as x falling
    // somewhere while the byte rises, and as both `rtl` values. This
    // used to assert that the *bytes* went backwards -- the visual order
    // `ComplexText` reported, which broke `GlyphSink`'s "source order".
    let bytes: Vec<u32> = c.0.iter().map(|c| c.byte).collect();
    let xs: Vec<f32> = c.0.iter().map(|c| c.x).collect();
    println!("bytes {bytes:?}, x {xs:?}");
    // A byte may repeat once at a direction change (M81/T81.5: the
    // ending run's trailing edge, then the next run's leading edge).
    assert!(bytes.windows(2).all(|w| w[1] >= w[0]), "clusters not in source order: {bytes:?}");
    assert!(bytes.windows(3).all(|w| !(w[0] == w[1] && w[1] == w[2])), "a byte reported three times: {bytes:?}");
    assert!(xs.windows(2).any(|w| w[1] < w[0]), "x never falls, so nothing ran right to left: {xs:?}");
    assert!(c.0.iter().any(|c| c.rtl) && c.0.iter().any(|c| !c.rtl), "both directions are reported");
}

#[test]
fn line_height_comes_from_the_face_not_a_guess() {
    let mut t = face();
    let h = t.line_height(16.0);
    assert!(h > 16.0 && h < 40.0, "a real face's metrics: {h}");
}

/// **The right-to-left run, asserted as right-to-left** — M45/T45.0
/// item 3, R10-F11. The test above asserts a positive width, more than
/// one cluster and offsets on boundaries, which a shaper laying the run
/// out left to right satisfies too: the review's injection that did so
/// (`let rtl = false && …`) left it green. Here the cluster at byte 0 is
/// the rightmost, and x falls as the byte rises. The trailing insertion
/// point (byte = len) is left out: it is reported at the run's advance,
/// which is the logical end, not a glyph.
#[test]
fn a_right_to_left_run_places_its_first_character_rightmost() {
    let mut t = face();
    let text = "\u{5e9}\u{5dc}\u{5d5}\u{5dd}";
    let mut c = Clusters::default();
    t.shape(text, 16.0, 400.0, f32::INFINITY, &mut c);
    let glyphs: Vec<&Cluster> = c.0.iter().filter(|cl| (cl.byte as usize) < text.len()).collect();
    let mut by_byte = glyphs.clone();
    by_byte.sort_by_key(|cl| cl.byte);
    let xs: Vec<(u32, f32)> = by_byte.iter().map(|cl| (cl.byte, cl.x)).collect();
    println!("clusters (byte, x) in logical order: {xs:?} (bound: x strictly falls)");
    assert_eq!(by_byte.len(), text.chars().count(), "setup: one cluster per character");
    assert!(xs.windows(2).all(|w| w[1].1 < w[0].1), "x does not fall as the byte rises: {xs:?}");
}

/// **The accept direction**: a left-to-right run's x rises.
#[test]
fn a_left_to_right_run_places_its_first_character_leftmost() {
    let mut t = face();
    let text = "Latin";
    let mut c = Clusters::default();
    t.shape(text, 16.0, 400.0, f32::INFINITY, &mut c);
    let xs: Vec<f32> = c.0.iter().map(|cl| cl.x).collect();
    println!("Latin x: {xs:?}");
    assert!(xs.windows(2).all(|w| w[1] > w[0]), "x does not rise: {xs:?}");
}

/// **Arabic joins** — R10-F11's other half. The shaped glyph ids differ
/// from the isolated letters' cmap ids, and the joined word is narrower
/// than its letters' isolated advances summed. A shaper with joining off
/// (the init, medi and fina features disabled) produces the cmap ids and
/// their sum.
#[test]
fn an_arabic_word_is_shaped_into_joined_forms() {
    let t = face();
    let face = rustybuzz::Face::from_slice(pinned::bytes(), 0).expect("the pinned face parses");
    let word = "\u{645}\u{631}\u{62d}\u{628}\u{627}";
    let px = 16.0;
    let upem = face.units_per_em() as f32;
    let isolated: Vec<u16> = word.chars().rev().map(|c| face.glyph_index(c).map_or(0, |g| g.0)).collect();
    let summed: f32 = isolated
        .iter()
        .map(|&g| face.glyph_hor_advance(rustybuzz::ttf_parser::GlyphId(g)).unwrap_or(0) as f32 * px / upem)
        .sum();
    let shaped = t.shaped_glyphs(word, px);
    let ids: Vec<u16> = shaped.iter().map(|g| g.0).collect();
    let mut m = face_measure();
    let w = m.measure(word, px, f32::INFINITY).w;
    println!("isolated (visual order): {isolated:?}, summed advance {summed:.2}");
    println!("shaped:                  {ids:?}, width {w} (bound: < {summed:.2}, ids differ)");
    assert!(isolated.iter().all(|&g| g != 0), "setup: the pinned face lacks an Arabic letter");
    assert_ne!(ids, isolated, "the word was shaped into its isolated forms: nothing joined");
    assert!(w < summed.ceil(), "the joined word is not narrower than its isolated letters");
}

fn face_measure() -> ComplexText {
    face()
}
