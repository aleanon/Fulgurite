//! T15.8: that the complex stack shapes what the default one cannot,
//! and that it stays out of everyone else's way.
//!
//! The font is a system face, so these skip rather than fail where it
//! is absent — the rule `baked_face.rs` and the live gate already use.

use lumen_static::cx::{Cluster, GlyphSink, TextMeasure};
use lumen_static_text::ComplexText;

#[derive(Default)]
struct Clusters(Vec<Cluster>);

impl GlyphSink for Clusters {
    fn cluster(&mut self, c: Cluster) {
        self.0.push(c);
    }
}

/// A face with wide coverage. `None` skips.
fn face() -> Option<ComplexText> {
    for p in [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    ] {
        if let Ok(bytes) = std::fs::read(p) {
            // Leaked on purpose: a `rustybuzz::Face` borrows its tables
            // for as long as it lives.
            let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
            if let Some(f) = ComplexText::new(bytes, 16.0) {
                return Some(f);
            }
        }
    }
    println!("shaping: no system face found, skipped");
    None
}

#[test]
fn latin_still_measures_sensibly() {
    let Some(mut t) = face() else { return };
    let s = t.measure("hello", 16.0, f32::INFINITY);
    assert!(s.w > 10.0 && s.w < 200.0, "a plausible width: {}", s.w);
    assert!(s.h > 0.0);
}

#[test]
fn an_empty_string_still_reports_a_caret_position() {
    let Some(mut t) = face() else { return };
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
    let Some(mut t) = face() else { return };
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
    let Some(mut t) = face() else { return };
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
    let Some(mut t) = face() else { return };
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
    let bytes: Vec<u32> = c.0.iter().map(|c| c.byte).collect();
    assert!(
        bytes.windows(2).any(|w| w[1] < w[0]),
        "a visual-order layout revisits earlier bytes: {bytes:?}"
    );
}

#[test]
fn line_height_comes_from_the_face_not_a_guess() {
    let Some(mut t) = face() else { return };
    let h = t.line_height(16.0);
    assert!(h > 16.0 && h < 40.0, "a real face's metrics: {h}");
}
