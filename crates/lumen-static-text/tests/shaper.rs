//! **`Shaper`, `FaceSource`, and clusters in source order** — M81/T81.2
//! (`docs/plan-shaped-paint.md`).
//!
//! The gates of the task, on the pinned face (M43/T43.11), each printing
//! the quantity it bounds:
//!
//! - a Hebrew run's clusters arrive in source order with their leading
//!   edges falling and `rtl` set, and a Latin run's rise with `rtl`
//!   clear;
//! - the containment corpus as one paragraph, wrapped at 80..400 px:
//!   no line wider than its box, and every break at a UAX #14
//!   opportunity or inside a word wider than the box;
//! - a `FaceSource`'s answer is the face a glyph is keyed with: Hebrew
//!   from the fallback face the source names, a 700 run from the bold;
//! - `Cluster` is still 12 bytes with `rtl` in it;
//! - since M81/T81.5, a direction change in a mixed line is reported as
//!   two insertion points with one byte, the ending run's trailing edge
//!   first.

use lumen_static::cx::Cluster;
use lumen_static_text::{FaceSource, ShapedRun, Shaper};

#[path = "../../lumen-static-shell/tests/fonts/pinned.rs"]
mod pinned;

const HEBREW: &str = "\u{5e9}\u{5dc}\u{5d5}\u{5dd}";
const PX: f32 = 16.0;

/// One face for every character, as `ComplexText` uses.
struct One(&'static [u8]);

impl FaceSource for One {
    fn face_for(&self, ch: char, _weight: f32) -> (u8, char) {
        (0, ch)
    }
    fn bytes(&self, face: u8) -> Option<&'static [u8]> {
        (face == 0).then_some(self.0)
    }
}

fn shape(text: &str, max_w: f32) -> ShapedRun {
    let src = One(pinned::bytes());
    let mut run = ShapedRun::new();
    Shaper::new().shape(&src, text, PX, 400.0, max_w, &mut run);
    run
}

/// **Clusters in source order, leading edges falling right to left.**
/// The trailing insertion point (byte = len) is left out of the x
/// check: it is the paragraph direction's end, not a cluster's edge.
///
/// Injection (T81.2's commit): lay runs out in logical order -- skip the
/// visual reordering of each line -- and the Hebrew x prints rising.
#[test]
fn hebrew_clusters_arrive_in_source_order_with_falling_leading_edges() {
    let run = shape(HEBREW, f32::INFINITY);
    let letters: Vec<&Cluster> = run.clusters.iter().filter(|c| (c.byte as usize) < HEBREW.len()).collect();
    let xs: Vec<(u32, f32, bool)> = letters.iter().map(|c| (c.byte, c.x, c.rtl)).collect();
    println!("Hebrew clusters (byte, leading x, rtl), as reported: {xs:?} (bound: byte rises, x strictly falls, rtl set)");
    assert_eq!(letters.len(), HEBREW.chars().count(), "setup: one cluster per Hebrew letter");
    assert_eq!(run.clusters.len(), HEBREW.chars().count() + 1, "setup: and the trailing insertion point");
    assert!(xs.windows(2).all(|w| w[1].0 > w[0].0), "clusters are not in source order: {xs:?}");
    assert!(xs.iter().all(|c| c.2), "a Hebrew cluster without rtl: {xs:?}");
    assert!(xs.windows(2).all(|w| w[1].1 < w[0].1), "Hebrew leading edges do not fall in source order: {xs:?}");
    // The first letter's leading (right) edge is the run's right end.
    let w = run.width();
    println!("run width {w}, first letter's leading edge {}", xs[0].1);
    assert!((xs[0].1 - w).abs() < 0.01, "the first Hebrew letter does not start at the run's right end");

    // The accept direction, in the same test: Latin rises, rtl clear.
    let latin = shape("Latin", f32::INFINITY);
    let lx: Vec<(u32, f32, bool)> = latin.clusters.iter().map(|c| (c.byte, c.x, c.rtl)).collect();
    println!("Latin clusters (byte, leading x, rtl): {lx:?} (bound: x strictly rises, rtl clear)");
    assert_eq!(lx.len(), "Latin".len() + 1, "setup: five letters and the trailing point");
    assert!(lx.windows(2).all(|w| w[1].1 > w[0].1), "Latin leading edges do not rise: {lx:?}");
    assert!(lx.iter().all(|c| !c.2), "a Latin cluster with rtl set: {lx:?}");
}

/// A mixed line: source order throughout, the Hebrew word's clusters
/// falling inside it and the Latin ones rising, the base direction
/// left to right (the trailing point at the right end).
///
/// Since M81/T81.5 a direction change is **two insertion points**: the
/// byte where the Hebrew word starts (4) and where it ends (12) each
/// appear twice, the ending run's trailing edge first. Both trailing
/// edges are the one edge between the space and the word — which no
/// leading edge is — so they are equal and lie between the space's
/// leading edge and the word's. Injection (T81.5's commit): `place_line`
/// pushes no closing point; the pair count prints 0.
#[test]
fn a_mixed_line_keeps_source_order_and_each_runs_direction() {
    let text = "abc \u{5e9}\u{5dc}\u{5d5}\u{5dd} def";
    let run = shape(text, f32::INFINITY);
    let cs: Vec<(u32, f32, bool)> = run.clusters.iter().map(|c| (c.byte, c.x, c.rtl)).collect();
    println!("mixed clusters (byte, x, rtl): {cs:?}");
    assert!(cs.windows(2).all(|w| w[1].0 >= w[0].0), "not in source order: {cs:?}");
    // The pairs: a byte reported twice, the first carrying the other
    // direction.
    let pairs: Vec<(u32, f32, f32)> =
        cs.windows(2).filter(|w| w[0].0 == w[1].0).map(|w| (w[0].0, w[0].1, w[1].1)).collect();
    println!("direction changes (byte, trailing edge, leading edge): {pairs:?} (bound: bytes [4, 12], trailing edges equal)");
    assert_eq!(pairs.iter().map(|p| p.0).collect::<Vec<_>>(), vec![4, 12], "not one pair per direction change: {pairs:?}");
    assert!(cs.windows(2).filter(|w| w[0].0 == w[1].0).all(|w| w[0].2 != w[1].2), "a pair keeps one direction: {cs:?}");
    let (edge, space, word) = (pairs[0].1, cs[3].1, pairs[0].2);
    assert!((pairs[1].1 - edge).abs() < 0.01, "the two sides of one edge disagree: {pairs:?}");
    assert!(space < edge && edge < word, "the edge {edge} is not between the space's {space} and the word's {word}");
    let letter = |k: usize| cs.get(k + 1).is_none_or(|n| n.0 != cs[k].0);
    let heb: Vec<f32> = (0..cs.len()).filter(|&k| cs[k].2 && letter(k)).map(|k| cs[k].1).collect();
    assert_eq!(heb.len(), 4, "setup: the four Hebrew letters are right to left");
    assert!(heb.windows(2).all(|w| w[1] < w[0]), "Hebrew x does not fall: {heb:?}");
    let last = cs.last().unwrap();
    assert!(!last.2 && (last.1 - run.width()).abs() < 0.01, "the trailing point is not at a left-to-right line's end: {last:?}");
}

/// The containment corpus's twelve strings
/// (`crates/lumen-static-shell/tests/text_containment.rs` `STRINGS`), as
/// one paragraph. Copied, since that file is a shell test with a `Host`
/// in it; the wrap gate needs words of several widths, not those exact
/// strings.
const PARAGRAPH: &str = "Taj Tj fj Settings jejunj Hij POSITIVE Wolf gypsy quay j Taj Mahal W";

/// What one wrapped shape measured: the worst overshoot past `max_w`,
/// breaks at an opportunity, breaks inside a too-wide word, lines.
struct Wrapped {
    worst: f32,
    at_opportunity: usize,
    in_word: usize,
    lines: usize,
}

/// Shape `text` at `max_w` and check every line. A line's width is the
/// larger of the shaper's own figure and the extent its glyphs reach by
/// the face's advances (whitespace glyphs left out), so a shaper that
/// misreported a width could not pass by it.
fn wrapped(text: &str, max_w: f32) -> Wrapped {
    let bytes = pinned::bytes();
    let face = rustybuzz::Face::from_slice(bytes, 0).expect("the pinned face parses");
    let scale = PX / face.units_per_em() as f32;
    let space = face.glyph_index(' ').map_or(0, |g| g.0);
    let adv = |id: u16| face.glyph_hor_advance(rustybuzz::ttf_parser::GlyphId(id)).unwrap_or(0) as f32 * scale;
    let opportunities: Vec<usize> = unicode_linebreak::linebreaks(text).map(|(b, _)| b).collect();
    let run = shape(text, max_w);
    let mut out = Wrapped { worst: f32::NEG_INFINITY, at_opportunity: 0, in_word: 0, lines: run.line_count() };
    for (i, line) in run.lines.iter().enumerate() {
        let extent = run
            .glyphs
            .iter()
            .filter(|g| g.line as usize == i && g.key.index != space)
            .map(|g| g.x + adv(g.key.index))
            .fold(0.0f32, f32::max);
        out.worst = out.worst.max(line.width.max(extent) - max_w);
        if i == 0 {
            continue;
        }
        let b = line.start as usize;
        if opportunities.contains(&b) {
            out.at_opportunity += 1;
        } else {
            // Inside a word: the word between the opportunities around
            // the break must itself be wider than the box.
            let s = opportunities.iter().copied().filter(|&o| o < b).max().unwrap_or(0);
            let e = opportunities.iter().copied().find(|&o| o > b).unwrap_or(text.len());
            let word = text[s..e].trim_end();
            let ww = shape(word, f32::INFINITY).width();
            println!("max_w {max_w}: break at byte {b} inside {word:?}, {ww:.2} px wide");
            assert!(
                ww > max_w,
                "a break at byte {b} is neither an opportunity nor inside a word wider than {max_w} px ({word:?}, {ww:.2} px)"
            );
            out.in_word += 1;
        }
    }
    out
}

/// **Wrap**: every line within `max_w`, every break where UAX #14 allows
/// one or inside a word wider than the box.
///
/// The sweep is the plan's, 80..400 px step 8; at 16 px no corpus word
/// is wider than 80, so a 40 px case follows, which breaks inside
/// `Settings` and `POSITIVE` and so exercises the in-word check.
///
/// Injection (T81.2's commit): the breaker ignores `max_w`; the 80 px
/// case prints one line and its overshoot.
#[test]
fn the_corpus_paragraph_wraps_within_max_w_at_opportunities() {
    let (mut worst, mut worst_at) = (f32::NEG_INFINITY, 0.0f32);
    let (mut at_opportunity, mut in_word) = (0usize, 0usize);
    let mut lines_at_80 = 0usize;
    let mut w = 80.0f32;
    while w <= 400.0 {
        let r = wrapped(PARAGRAPH, w);
        if w == 80.0 {
            lines_at_80 = r.lines;
            println!("max_w 80: {} lines, overshoot {:.3} px", r.lines, r.worst);
        }
        if r.worst > worst {
            (worst, worst_at) = (r.worst, w);
        }
        at_opportunity += r.at_opportunity;
        in_word += r.in_word;
        w += 8.0;
    }
    println!("lines at 80 px: {lines_at_80} (setup: > 1)");
    println!("80..400: worst overshoot {worst:.3} px at max_w {worst_at} (bound 0.5)");
    println!("80..400: breaks at a UAX #14 opportunity {at_opportunity}, inside a word wider than max_w {in_word}");
    assert!(lines_at_80 > 1, "setup: the 80 px case did not wrap");
    assert!(worst <= 0.5, "a line overshoots its box by {worst:.3} px at max_w {worst_at}");

    let narrow = wrapped(PARAGRAPH, 40.0);
    println!(
        "40 px: {} lines, worst overshoot {:.3} px (bound 0.5), at an opportunity {}, inside a word {} (setup: > 0)",
        narrow.lines, narrow.worst, narrow.at_opportunity, narrow.in_word
    );
    assert!(narrow.in_word > 0, "setup: no corpus word was wider than 40 px");
    assert!(narrow.worst <= 0.5, "a line overshoots 40 px by {:.3}", narrow.worst);
}

/// Three faces under the source's rule: face 0 for Latin at 400, face 1
/// at 700, face 2 for Hebrew -- face 0 "lacks" it by the rule, which is
/// what a `FaceSource` is for. One committed face backs all three ids,
/// so the glyph indices agree and only the face id can tell them apart.
struct Three(&'static [u8]);

impl FaceSource for Three {
    fn face_for(&self, ch: char, weight: f32) -> (u8, char) {
        if ('\u{590}'..='\u{5ff}').contains(&ch) {
            (2, ch)
        } else if weight >= 600.0 {
            (1, ch)
        } else {
            (0, ch)
        }
    }
    fn bytes(&self, face: u8) -> Option<&'static [u8]> {
        (face <= 2).then_some(self.0)
    }
}

/// **Weight and fallback come from the source.**
///
/// Injection (T81.2's commit): the shaper ignores the face the source
/// answers and uses face 0; the gate names the first Hebrew character.
#[test]
fn hebrew_takes_the_sources_fallback_face_and_a_bold_run_its_bold_face() {
    let src = Three(pinned::bytes());
    let mut shaper = Shaper::new();
    let mut run = ShapedRun::new();

    let text = "ab \u{5e9}\u{5dc}\u{5d5}\u{5dd}";
    shaper.shape(&src, text, PX, 400.0, f32::INFINITY, &mut run);
    // Glyphs are in visual order, so each character's glyph is found by
    // its cmap index, which is distinct for every character here.
    let faces: Vec<(char, u8)> = {
        let face = rustybuzz::Face::from_slice(pinned::bytes(), 0).unwrap();
        text.chars()
            .filter(|c| !c.is_whitespace())
            .map(|ch| {
                let id = face.glyph_index(ch).map_or(0, |g| g.0);
                let key = run.glyphs.iter().find(|g| g.key.index == id).map(|g| g.key.face).unwrap_or(u8::MAX);
                (ch, key)
            })
            .collect()
    };
    println!("400 weight, faces per char: {faces:?} (bound: Hebrew 2, Latin 0)");
    assert_eq!(run.glyphs.len(), text.chars().count(), "setup: one glyph per character");
    for &(ch, f) in &faces {
        if ('\u{590}'..='\u{5ff}').contains(&ch) {
            assert_eq!(f, 2, "Hebrew {ch:?} (U+{:04X}) was keyed face {f}, not the source's fallback face 2", ch as u32);
        } else {
            assert_eq!(f, 0, "Latin {ch:?} was keyed face {f}, not 0");
        }
    }
    assert!(run.glyphs.iter().all(|g| g.key.index != 0), "setup: a glyph fell to .notdef");

    shaper.shape(&src, "ab", PX, 700.0, f32::INFINITY, &mut run);
    let bold: Vec<u8> = run.glyphs.iter().map(|g| g.key.face).collect();
    println!("700 weight, faces per glyph of \"ab\": {bold:?} (bound: all 1)");
    assert!(!bold.is_empty() && bold.iter().all(|&f| f == 1), "a 700 run did not take the bold face: {bold:?}");
}

/// **`Cluster` stays 12 bytes**: `rtl` takes padding after `line`.
#[test]
fn a_cluster_is_still_twelve_bytes() {
    let n = core::mem::size_of::<Cluster>();
    println!("size_of::<Cluster>() = {n} (bound: 12)");
    assert_eq!(n, 12);
}

/// A Latin run builds no bidi levels, and a substitute keeps its source
/// byte: the cluster offsets are the caller's even when the shaped
/// character is a different length.
#[test]
fn a_substitute_keeps_its_source_byte() {
    struct Sub(&'static [u8]);
    impl FaceSource for Sub {
        fn face_for(&self, ch: char, _w: f32) -> (u8, char) {
            (0, if ch == '\u{2212}' { '-' } else { ch })
        }
        fn bytes(&self, face: u8) -> Option<&'static [u8]> {
            (face == 0).then_some(self.0)
        }
    }
    let text = "a\u{2212}b";
    let mut run = ShapedRun::new();
    Shaper::new().shape(&Sub(pinned::bytes()), text, PX, 400.0, f32::INFINITY, &mut run);
    let bytes: Vec<u32> = run.clusters.iter().map(|c| c.byte).collect();
    println!("cluster bytes {bytes:?} (bound: [0, 1, 4, 5])");
    assert_eq!(bytes, vec![0, 1, 4, 5]);
    let face = rustybuzz::Face::from_slice(pinned::bytes(), 0).unwrap();
    let hyphen = face.glyph_index('-').map_or(0, |g| g.0);
    assert!(run.glyphs.iter().any(|g| g.key.index == hyphen), "the substitute was not shaped");
}

/// Mandatory breaks end lines, and a text ending in one puts the
/// trailing point on a new empty line.
#[test]
fn a_newline_ends_a_line_and_draws_nothing() {
    let run = shape("ab\ncd\n", f32::INFINITY);
    let lines: Vec<(u32, u32, f32)> = run.lines.iter().map(|l| (l.start, l.end, l.width)).collect();
    println!("lines {lines:?}, glyphs {}", run.glyphs.len());
    assert_eq!(run.line_count(), 3, "two lines and the empty one after the final newline");
    assert_eq!(run.glyphs.len(), 4, "a newline is drawn as nothing");
    let last = run.clusters.last().unwrap();
    assert_eq!((last.byte, last.line, last.x), (6, 2, 0.0));
}
