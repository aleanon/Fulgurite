//! **The embedded face wraps at the space** — M45/T45.12 (R10-F10,
//! `docs/plan-text-correctness.md`).
//!
//! `BakedFace::shape` said it "wraps at the last space that fits". On
//! overflow it rewound only the widest-line counter and reset x: the
//! glyphs of the word already placed stayed on the old line, so the
//! break landed **mid-word** and the reported width included the half
//! word that had moved. `"aaa bbbb"` at 60 px broke at byte 6 and
//! measured 55 px wide against a 60 px box, with its longest line 41.
//!
//! For each phrase and width: no line starts mid-word (unless one word
//! is wider than the box, which must still break at a glyph), each
//! line's content fits, and the measured width is at most the limit.

use embedded::face::FACE;
use lumen_static::cx::{Cluster, GlyphSink, TextMeasure};

/// The core's `Vec<Cluster>` sink is behind its `alloc` feature, which
/// this fixture's device profile leaves off.
struct Sink(Vec<Cluster>);
impl GlyphSink for Sink {
    fn cluster(&mut self, c: Cluster) {
        self.0.push(c);
    }
}

const SIZE: f32 = 16.0;

fn shape(text: &str, max_w: f32) -> (lumen_static::Size, Vec<Cluster>) {
    let mut face = FACE;
    let mut sink = Sink(Vec::new());
    let s = face.shape(text, SIZE, 400.0, max_w, &mut sink);
    (s, sink.0)
}

/// The width of the widest word in `text`, so a phrase whose words all
/// fit can be held to "no mid-word break" and one that cannot is held
/// only to "breaks at a glyph".
fn widest_word(text: &str) -> f32 {
    let mut face = FACE;
    text.split(' ').map(|w| face.measure(w, SIZE, 0.0).w).fold(0.0, f32::max)
}

#[test]
#[ignore = "red until T45.12: BakedFace wraps mid-word (R10-F10)"]
fn aaa_bbbb_at_60_is_two_lines_and_41_wide() {
    let (s, cl) = shape("aaa bbbb", 60.0);
    let lines = cl.iter().map(|c| c.line).max().unwrap_or(0) + 1;
    let start_of_line_1 = cl.iter().find(|c| c.line == 1).map(|c| c.byte);
    println!("\"aaa bbbb\" at 60: {lines} lines, w {} (must be 41), line 1 starts at byte {start_of_line_1:?} (must be 4)", s.w);
    assert_eq!(lines, 2);
    assert_eq!(start_of_line_1, Some(4), "line 1 starts mid-word");
    assert_eq!(s.w, 41.0);
}

#[test]
#[ignore = "red until T45.12: BakedFace wraps mid-word (R10-F10)"]
fn no_line_starts_mid_word_and_every_line_fits() {
    let corpus = [
        "aaa bbbb",
        "the quick brown fox jumps over the lazy dog",
        "Hello, world. A label that wraps.",
        "one two three four five six seven eight nine ten",
        "x yy zzz wwww vvvvv",
    ];
    let mut failures = Vec::new();
    for text in corpus {
        for max_w in [40.0f32, 60.0, 80.0, 120.0, 200.0] {
            let (s, cl) = shape(text, max_w);
            let fits_words = widest_word(text) <= max_w;
            let bytes = text.as_bytes();
            let mut prev_line = 0;
            for c in &cl {
                if c.line != prev_line {
                    let b = c.byte as usize;
                    let mid = b > 0 && b < bytes.len() && bytes[b - 1] != b' ' && bytes[b] != b' ';
                    if mid && fits_words {
                        failures.push(format!("{text:?} at {max_w}: line {} starts mid-word at byte {b}", c.line));
                    }
                    prev_line = c.line;
                }
            }
            if fits_words && s.w > max_w {
                failures.push(format!("{text:?} at {max_w}: width {} over the limit", s.w));
            }
        }
    }
    println!("{} failures: {failures:#?}", failures.len());
    assert!(failures.is_empty(), "{} lines broke wrong", failures.len());
}

/// The accept direction: a word with no space in it wider than the box
/// still breaks at a glyph, without panicking, and every glyph lands.
#[test]
fn an_unbreakable_word_breaks_at_a_glyph() {
    let text = "abcdefghijklmnop";
    let (s, cl) = shape(text, 40.0);
    let lines = cl.iter().map(|c| c.line).max().unwrap_or(0) + 1;
    println!("{text:?} at 40: {lines} lines, w {}, {} clusters", s.w, cl.len());
    assert!(lines >= 4, "a 16-letter word at 40 px must take several lines");
    assert_eq!(cl.len(), text.len() + 1, "every byte and the end have a cluster");
    assert!(s.w <= 40.0, "a hard-broken line is at most the limit, got {}", s.w);
}
