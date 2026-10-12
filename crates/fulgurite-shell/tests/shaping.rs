//! T7.2b: the shaping seam. `TextMeasure::shape` reports every cluster
//! boundary — the caret positions a text field has — and agrees with
//! `measure_weighted` about the run's size.
#![cfg(feature = "truetype")]

use fulgurite::{Cluster, GlyphSink, TextMeasure};
use fulgurite_shell::text::{FontMetrics, FontStack};

#[path = "fonts/pinned.rs"]
mod pinned;

/// The committed face, never the system's (M43/T43.11): these tests used
/// to return early when no system face existed -- a pass that shaped
/// nothing -- and shaped whichever face the machine had when one did.
fn metrics() -> FontMetrics {
    FontMetrics::new(FontStack::from_bytes(pinned::bytes(), None).expect("the pinned face parses"))
}

/// A sink with a hard ceiling, to exercise `is_full`.
struct Capped {
    out: Vec<Cluster>,
    cap: usize,
}

impl GlyphSink for Capped {
    fn cluster(&mut self, c: Cluster) {
        self.out.push(c);
    }
    fn is_full(&self) -> bool {
        self.out.len() >= self.cap
    }
}

#[test]
fn a_run_reports_one_cluster_per_char_plus_the_trailing_caret() {
    let mut m = metrics();
    let mut out: Vec<Cluster> = Vec::new();
    let size = m.shape("Hi there", 16.0, 400.0, f32::INFINITY, &mut out);

    assert_eq!(out.len(), "Hi there".chars().count() + 1, "n chars → n+1 caret positions");
    assert_eq!(out[0].byte, 0);
    assert_eq!(out.last().unwrap().byte, "Hi there".len() as u32, "trailing caret is at the end");
    assert!(out.windows(2).all(|w| w[0].byte < w[1].byte), "byte offsets ascend: {out:?}");
    assert!(out.windows(2).all(|w| w[0].x <= w[1].x), "x ascends on one line");
    assert!(out.iter().all(|c| c.line == 0), "no wrapping was asked for");

    // The trailing caret sits at the run's advance width, which is what
    // `measure_weighted` reports — the two must not disagree, or a caret
    // at the end of a field lands outside the field's own box.
    let measured = m.measure_weighted("Hi there", 16.0, 400.0, f32::INFINITY);
    assert_eq!(size, measured, "shape and measure agree on size");
    assert!(
        (out.last().unwrap().x - measured.w).abs() <= 1.0,
        "trailing caret {} vs width {}",
        out.last().unwrap().x,
        measured.w
    );
}

#[test]
fn every_byte_offset_lands_on_a_char_boundary_even_after_substitution() {
    let mut m = metrics();
    // U+2212 MINUS SIGN is three bytes and substitutes to one-byte '-'
    // in a face that lacks it. Offsets must still index the caller's
    // string, or a caret lands mid-character and slicing panics.
    let text = "\u{2212}10 \u{2014} ok";
    let mut out: Vec<Cluster> = Vec::new();
    m.shape(text, 16.0, 400.0, f32::INFINITY, &mut out);

    for c in &out {
        assert!(
            text.is_char_boundary(c.byte as usize),
            "byte {} is not a char boundary of {text:?}",
            c.byte
        );
        // The offset must be usable: this is the operation a text field
        // performs on every keystroke.
        let _ = &text[..c.byte as usize];
    }
    assert_eq!(out.last().unwrap().byte, text.len() as u32);
}

#[test]
fn wrapping_reports_the_line_each_cluster_landed_on() {
    let mut m = metrics();
    let mut out: Vec<Cluster> = Vec::new();
    let size = m.shape("aaaa bbbb cccc dddd", 16.0, 400.0, 60.0, &mut out);
    let lines = out.iter().map(|c| c.line).max().unwrap();
    assert!(lines >= 1, "60 px must wrap this run; got one line");
    assert!(size.h > m.line_height(16.0), "a wrapped run is taller than one line");
    // A later line never reports a smaller line number: clusters come in
    // source order, and so do lines.
    assert!(out.windows(2).all(|w| w[0].line <= w[1].line), "lines ascend: {out:?}");
}

#[test]
fn a_full_sink_stops_receiving_and_the_size_is_still_right() {
    let mut m = metrics();
    let mut capped = Capped { out: Vec::new(), cap: 3 };
    let size = m.shape("Hi there", 16.0, 400.0, f32::INFINITY, &mut capped);
    assert_eq!(capped.out.len(), 3, "the sink refused to grow past its capacity");
    assert_eq!(size, m.measure_weighted("Hi there", 16.0, 400.0, f32::INFINITY), "size unaffected");
}

#[test]
fn an_empty_run_still_has_one_caret_position() {
    let mut m = metrics();
    let mut out: Vec<Cluster> = Vec::new();
    let size = m.shape("", 16.0, 400.0, f32::INFINITY, &mut out);
    assert_eq!(out, vec![Cluster { byte: 0, x: 0.0, line: 0, rtl: false }], "an empty field has a caret");
    assert_eq!(size.w, 0.0);
    assert!(size.h > 0.0, "an empty line is still a line tall");
}

#[test]
fn the_stand_in_measures_without_clusters() {
    // `BlockMetrics` has no shaper: the default `shape` measures and
    // reports nothing, which a caller distinguishes by getting no
    // clusters rather than by getting wrong ones.
    let mut b = fulgurite::BlockMetrics;
    let mut out: Vec<Cluster> = Vec::new();
    let size = b.shape("Hi", 16.0, 400.0, f32::INFINITY, &mut out);
    assert!(out.is_empty(), "the stand-in claims no cluster positions");
    assert_eq!(size, b.measure("Hi", 16.0, f32::INFINITY));
}

/// M26/T26.17 Phase D #13: a character **no text face has** must be
/// substituted, and the substitution must be observable.
///
/// The test above this one names substitution in its title and does not
/// exercise it. Its comment is conditional and honest — "in a face that
/// lacks it" — but U+2212 and U+2014 are in every system face this
/// repository has run against, so `has_all(wanted)` answers true, the
/// fall-through never runs, and the assertions inspect an *unsubstituted*
/// string. It is a real test of byte offsets and a vacuous one of
/// substitution.
///
/// U+E000 is the first Private Use Area code point. No text face assigns
/// it, by definition, so the fall-through is forced. `prepare` maps any
/// uncovered character it has no specific replacement for to `'?'`, so a
/// PUA run must measure exactly as wide as a run of question marks —
/// and, with substitution skipped, as wide as a run of `.notdef` boxes
/// instead.
#[test]
fn a_character_no_face_has_is_substituted() {
    let mut m = metrics();
    let pua = m.measure_weighted("\u{E000}\u{E000}\u{E000}", 16.0, 400.0, f32::INFINITY);
    let subs = m.measure_weighted("???", 16.0, 400.0, f32::INFINITY);
    let plain = m.measure_weighted("abc", 16.0, 400.0, f32::INFINITY);
    println!("PUA run {:?}; '???' {:?}; 'abc' {:?}", pua.w, subs.w, plain.w);
    assert_ne!(subs.w, 0.0, "setup: the face measured something for '???'");
    assert_ne!(
        subs.w, plain.w,
        "setup: '???' and 'abc' must differ, or this comparison cannot fail"
    );
    assert_eq!(
        pua.w, subs.w,
        "three unassigned code points did not measure as three '?': the substitution \
         in FontStack::prepare did not run"
    );
}
