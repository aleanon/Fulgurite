//! **Vector text is outlines from the measurer** — M82/T82.7
//! (`docs/plan-paint-vocabulary.md`, "Vector text is outlines from the
//! measurer").
//!
//! `TextMeasure::outline` writes a run's glyph outlines in the run's
//! frame, each glyph at the pen the measurer advances by. These gates hold
//! `FontMetrics` (under `vector-text`) to fontdue's own layout of the same
//! run on the pinned face, and `ComplexText` (under `shaping` too) to its
//! shaper's pens:
//!
//! - **Outline**: `outline("AVWay", 32)` writes elements (the setup), its
//!   box is the union of fontdue's glyph boxes within 1 px, and the glyph
//!   ids it announces are `place_run`'s and ttf-parser's cmap's.
//!   Injection (T82.7's commit): pens ignored, every glyph at x = 0; the
//!   width collapses.
//! - **Pens**: each glyph's outline xmin is its pen, as `extent_by`
//!   advances it, plus ttf-parser's bbox xmin, within 1/64 px; a space
//!   advances the next pen and adds no element. Injection: pens taken from
//!   `Placed.x`; the bearing is applied twice and the gate names the glyph.
//!   The same gate against `ComplexText`; injection: its override deleted,
//!   so it answers `false`.
#![cfg(feature = "vector-text")]

use lumen_static::cx::{BlockMetrics, PathSink, TextMeasure};
use lumen_static::display::{path_bounds, PathEl, AFFINE_IDENTITY};
use lumen_static_shell::text::{FontMetrics, FontStack};

#[path = "fonts/pinned.rs"]
mod pinned;

const RUN: &str = "AVWay";
const PX: f32 = 32.0;

/// Collects the outline per glyph: (announced id, its elements).
#[derive(Default)]
struct PerGlyph {
    glyphs: Vec<(u16, Vec<PathEl>)>,
}

impl PathSink for PerGlyph {
    fn el(&mut self, e: PathEl) {
        self.glyphs.last_mut().expect("an element before any glyph was announced").1.push(e);
    }
    fn glyph(&mut self, index: u16) {
        self.glyphs.push((index, Vec::new()));
    }
}

impl PerGlyph {
    fn all(&self) -> Vec<PathEl> {
        self.glyphs.iter().flat_map(|g| g.1.iter().copied()).collect()
    }
}

fn metrics() -> FontMetrics {
    FontMetrics::new(FontStack::from_bytes(pinned::bytes(), None).expect("the pinned face parses"))
}

fn face() -> ttf_parser::Face<'static> {
    ttf_parser::Face::parse(pinned::bytes(), 0).expect("ttf-parser reads the pinned face")
}

#[test]
fn the_outline_covers_fontdues_glyph_boxes() {
    let mut m = metrics();
    let mut sink = PerGlyph::default();
    assert!(m.outline(RUN, PX, 400.0, &mut sink), "FontMetrics answered no outline under vector-text");
    let els = sink.all();
    println!("outline(\"{RUN}\", {PX}): {} elements over {} glyphs (bound > 0)", els.len(), sink.glyphs.len());
    assert!(!els.is_empty(), "setup: the outline wrote no element");

    let ob = path_bounds(&els, &AFFINE_IDENTITY).expect("finite points");
    let stack = FontStack::from_bytes(pinned::bytes(), None).unwrap();
    let (placed, size) = stack.place_run(RUN, PX, 400.0, f32::INFINITY);
    let mut fb = [f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY];
    for p in &placed {
        fb = [fb[0].min(p.x), fb[1].min(p.y), fb[2].max(p.x + p.w as f32), fb[3].max(p.y + p.h as f32)];
    }
    println!("outline box {ob:?}");
    println!("fontdue box {fb:?} (run measured {} x {})", size.w, size.h);
    for k in 0..4 {
        assert!(
            (ob[k] - fb[k]).abs() <= 1.0,
            "edge {k}: the outline's box {ob:?} is not fontdue's glyph boxes {fb:?} within 1 px"
        );
    }

    let f = face();
    let ids: Vec<(char, u16, u16, u16)> = RUN
        .chars()
        .zip(&placed)
        .zip(&sink.glyphs)
        .map(|((c, p), g)| (c, p.id.index(), f.glyph_index(c).map_or(0, |g| g.0), g.0))
        .collect();
    println!("(char, place_run id, cmap id, announced id): {ids:?}");
    assert_eq!(placed.len(), RUN.chars().count(), "setup: every glyph of {RUN} is inked");
    for (c, placed, cmap, announced) in ids {
        assert_eq!(placed, cmap, "{c:?}: place_run's id is not ttf-parser's cmap id");
        assert_eq!(announced, cmap, "{c:?}: the outline drew another glyph than the cmap's");
    }
}

/// The xmin of one glyph's outline.
fn xmin(els: &[PathEl]) -> f32 {
    path_bounds(els, &AFFINE_IDENTITY).map_or(f32::NAN, |b| b[0])
}

#[test]
fn each_glyph_sits_at_its_pen() {
    let f = face();
    let s = PX / f.units_per_em() as f32;
    let font = fontdue::Font::from_bytes(pinned::bytes(), fontdue::FontSettings::default()).unwrap();
    for text in [RUN, "A V"] {
        let mut sink = PerGlyph::default();
        assert!(metrics().outline(text, PX, 400.0, &mut sink));
        assert_eq!(sink.glyphs.len(), text.chars().count(), "{text:?}: one announcement per character");
        // The pen as `extent_by` advances it: from the line's start, by
        // ceil(advance).
        let mut pen = 0.0f32;
        for (c, (id, els)) in text.chars().zip(&sink.glyphs) {
            let gid = f.glyph_index(c).expect("the face has it");
            match f.glyph_bounding_box(gid) {
                Some(bb) => {
                    let want = pen + bb.x_min as f32 * s;
                    let got = xmin(els);
                    println!("{text:?} {c:?} (id {id}): pen {pen}, outline xmin {got:.4}, pen + bbox xmin {want:.4} (bound 1/64)");
                    assert!(
                        (got - want).abs() <= 1.0 / 64.0,
                        "{c:?} in {text:?}: outline xmin {got} is not pen {pen} + bbox xmin {} = {want}",
                        bb.x_min as f32 * s
                    );
                }
                None => {
                    println!("{text:?} {c:?} (id {id}): pen {pen}, no outline, {} elements (bound 0)", els.len());
                    assert!(els.is_empty(), "{c:?} has no bbox and wrote elements");
                }
            }
            pen += font.metrics_indexed(gid.0, PX).advance_width.ceil();
        }
    }
    // A space advances the next pen: V sits further right after "A ".
    let v_at = |text: &str| {
        let mut sink = PerGlyph::default();
        metrics().outline(text, PX, 400.0, &mut sink);
        xmin(&sink.glyphs.last().unwrap().1)
    };
    let (tight, spaced) = (v_at("AV"), v_at("A V"));
    println!("V's xmin: {tight} in \"AV\", {spaced} in \"A V\" (bound: further right)");
    assert!(spaced > tight + 1.0, "the space did not advance the pen");
}

/// At scale 2 the outline is measured at 64 px and divided back down: the
/// same run in logical px, within the pens' whole-pixel rounding.
#[test]
fn a_scaled_measurer_answers_in_logical_px() {
    let stack = FontStack::from_bytes(pinned::bytes(), None).unwrap();
    let (mut one, mut two) = (FontMetrics::new(stack.clone()), FontMetrics::with_scale(stack, 2.0));
    let (mut a, mut b) = (Vec::<PathEl>::new(), Vec::<PathEl>::new());
    assert!(one.outline(RUN, PX, 400.0, &mut a) && two.outline(RUN, PX, 400.0, &mut b));
    let (ba, bb) = (path_bounds(&a, &AFFINE_IDENTITY).unwrap(), path_bounds(&b, &AFFINE_IDENTITY).unwrap());
    println!("scale 1 box {ba:?}, scale 2 box {bb:?} (bound 1 px per edge)");
    assert_eq!(a.len(), b.len(), "the two scales drew different outlines");
    for k in 0..4 {
        assert!((ba[k] - bb[k]).abs() <= 1.0, "edge {k}: scale 2 is not in logical px");
    }
}

/// The provided default: a measurer with no outlines says so.
#[test]
fn a_stand_in_has_no_outlines() {
    let mut v = Vec::<PathEl>::new();
    assert!(!BlockMetrics.outline(RUN, PX, 400.0, &mut v));
    assert!(v.is_empty());
}

/// The same pen gate against `ComplexText`, the text crate's measurer:
/// its shaper's pens, its glyph's offset included.
#[cfg(feature = "shaping")]
mod complex {
    use super::*;
    use lumen_static_text::ComplexText;

    #[test]
    fn complex_text_outlines_sit_at_its_pens() {
        let mut ct = ComplexText::new(pinned::bytes(), PX).expect("parses");
        let mut sink = PerGlyph::default();
        let answered = ct.outline(RUN, PX, 400.0, &mut sink);
        println!("ComplexText::outline answered {answered}, {} elements", sink.all().len());
        assert!(answered, "ComplexText answered no outline");
        assert!(!sink.all().is_empty(), "setup: ComplexText wrote no element");
        let f = face();
        let s = PX / f.units_per_em() as f32;
        let pens = ct.shaped_glyphs(RUN, PX);
        assert_eq!(pens.len(), sink.glyphs.len());
        for ((gid, pen), (id, els)) in pens.iter().zip(&sink.glyphs) {
            assert_eq!(gid, id, "the outline announced another glyph than the shaper's");
            let bb = f.glyph_bounding_box(ttf_parser::GlyphId(*gid)).expect("ink");
            let (got, want) = (xmin(els), pen + bb.x_min as f32 * s);
            println!("ComplexText id {id}: pen {pen:.4}, outline xmin {got:.4}, pen + bbox xmin {want:.4} (bound 1/64)");
            assert!((got - want).abs() <= 1.0 / 64.0, "glyph {id}: outline xmin {got} is not {want}");
        }
    }
}
