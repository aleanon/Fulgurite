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
//!
//! And `VectorText` (M82/T82.8), the widget that paints them:
//!
//! - **Rotated**: at 90° its ink box's w/h is the inverse of 0°'s, and the
//!   ink stays inside the node's box (setup: elements above 0, or the
//!   fallback would pass vacuously). Injections: paint ignores the angle;
//!   the box is sized from the unrotated run.
//! - **Memo**: re-layout with nothing changed asks for no outline, a text
//!   change asks once, a generation bump with the text unchanged asks
//!   once. Injection: the key omits the generation; that case asks 0
//!   times and serves a stale outline.
//! - **Fallback**: with `BlockMetrics`, exactly one `Cmd::Text` and no
//!   `Path`. Injection: the fallback removed; nothing is painted.
#![cfg(feature = "vector-text")]

use fulgurite::cx::{BlockMetrics, PathSink, TextMeasure};
use fulgurite::display::{path_bounds, PathEl, AFFINE_IDENTITY};
use fulgurite_shell::text::{FontMetrics, FontStack};

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
    use fulgurite_text::ComplexText;

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

// ---- VectorText (M82/T82.8) -------------------------------------------

mod widget {
    use super::*;
    use core::cell::Cell;
    use fulgurite::display::{Cmd, DisplayList};
    use fulgurite::widget::{Node, Widget};
    use fulgurite::{Constraints, LayoutCx, NodeState, Point, Size, Style};
    use fulgurite_widgets::VectorText;

    const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 800.0 };

    fn style() -> Style {
        Style { font_size: PX, ..<VectorText as Widget>::type_defaults() }
    }

    /// Lay `w` out and paint it: (its node box, the list).
    fn frame(w: &mut VectorText, m: &mut dyn TextMeasure) -> ([f32; 4], DisplayList) {
        let styles = [style()];
        let mut st = [NodeState::default()];
        {
            let mut cx = LayoutCx::new(m);
            Node::layout(w, &mut cx, VIEW, &styles, &mut st, 0);
        }
        Node::place(w, Point { x: 10.0, y: 20.0 }, &styles, &mut st, 0);
        let mut list = DisplayList::new();
        Node::paint(w, &st, 0, &mut list);
        let s = st[0];
        ([s.pos.x, s.pos.y, s.pos.x + s.size.w, s.pos.y + s.size.h], list)
    }

    fn ink(list: &DisplayList) -> Option<([f32; 4], usize)> {
        list.cmds.iter().find_map(|c| match c {
            Cmd::Path { rect, els, .. } => Some((*rect, els.1 as usize)),
            _ => None,
        })
    }

    #[test]
    fn a_turned_run_turns_its_ink_and_keeps_it_in_its_box() {
        let mut m = metrics();
        let mut ratio = [0.0f32; 2];
        for (k, angle) in [0.0f32, 90.0].into_iter().enumerate() {
            let mut w = VectorText::new("Vertical", angle);
            let (node, list) = frame(&mut w, &mut m);
            let (ink, n) = ink(&list).expect("setup: no Path painted -- the fallback drew text");
            assert!(n > 0, "setup: the path has no elements");
            let (iw, ih) = (ink[2] - ink[0], ink[3] - ink[1]);
            ratio[k] = iw / ih;
            println!("{angle}°: {n} elements, ink {ink:?} ({iw:.2} x {ih:.2}, w/h {:.4}), node {node:?}", ratio[k]);
            for e in 0..2 {
                assert!(ink[e] >= node[e] - 0.01, "{angle}°: the ink {ink:?} leaves the node's box {node:?} (edge {e})");
                assert!(ink[e + 2] <= node[e + 2] + 0.01, "{angle}°: the ink {ink:?} leaves the node's box {node:?} (edge {})", e + 2);
            }
        }
        println!("w/h at 90° {:.4} against 1 / (w/h at 0°) {:.4} (bound 1%)", ratio[1], 1.0 / ratio[0]);
        assert!(ratio[0] > 2.0, "setup: the run is not wider than tall at 0°");
        assert!(
            (ratio[1] * ratio[0] - 1.0).abs() < 0.01,
            "the ink at 90° ({:.4}) is not 0°'s turned ({:.4})",
            ratio[1],
            1.0 / ratio[0]
        );
    }

    /// Counts outline calls, and reports a generation the test sets.
    struct Counting {
        inner: FontMetrics,
        calls: Cell<u32>,
        generation: u32,
    }

    impl TextMeasure for Counting {
        fn measure(&mut self, t: &str, size: f32, max_w: f32) -> Size {
            self.inner.measure(t, size, max_w)
        }
        fn measure_weighted(&mut self, t: &str, size: f32, weight: f32, max_w: f32) -> Size {
            self.inner.measure_weighted(t, size, weight, max_w)
        }
        fn generation(&self) -> u32 {
            self.generation
        }
        // A wrapper forwards it (the trait's rustdoc).
        fn outline(&mut self, t: &str, size: f32, weight: f32, sink: &mut dyn PathSink) -> bool {
            self.calls.set(self.calls.get() + 1);
            self.inner.outline(t, size, weight, sink)
        }
    }

    #[test]
    fn the_outline_is_memoised_on_text_and_generation() {
        let mut m = Counting { inner: metrics(), calls: Cell::new(0), generation: 7 };
        let mut w = VectorText::<String>::new("memo".into(), 30.0);
        let lay = |w: &mut VectorText<String>, m: &mut Counting| {
            let styles = [style()];
            let mut st = [NodeState::default()];
            let mut cx = LayoutCx::new(m);
            Node::layout(w, &mut cx, VIEW, &styles, &mut st, 0);
        };
        lay(&mut w, &mut m);
        assert_eq!(m.calls.get(), 1, "setup: the first layout outlined once");
        assert!(w.is_vector(), "setup: the measurer had outlines");
        let asked = |m: &mut Counting, w: &mut VectorText<String>| {
            let before = m.calls.get();
            lay(w, m);
            m.calls.get() - before
        };
        let still = asked(&mut m, &mut w);
        w.text = "memo!".into();
        let changed = asked(&mut m, &mut w);
        m.generation = 8;
        let bumped = asked(&mut m, &mut w);
        println!("outline calls: still {still} (bound 0), text changed {changed} (bound 1), generation bumped {bumped} (bound 1)");
        assert_eq!(still, 0, "a still re-layout asked for the outline again");
        assert_eq!(changed, 1, "a text change was served the old outline");
        assert_eq!(bumped, 1, "a new generation was served the old outline (M45/T45.2's shape)");
    }

    #[test]
    fn without_outlines_it_paints_axis_aligned_text() {
        let mut w = VectorText::new("Fallback", 90.0);
        let (node, list) = frame(&mut w, &mut BlockMetrics);
        let texts = list.cmds.iter().filter(|c| matches!(c, Cmd::Text { .. })).count();
        let paths = list.cmds.iter().filter(|c| matches!(c, Cmd::Path { .. })).count();
        println!("BlockMetrics: {texts} Text (bound 1), {paths} Path (bound 0), node {node:?}");
        assert!(!w.is_vector());
        assert_eq!((texts, paths), (1, 0), "the fallback is one axis-aligned run and no path");
        assert!(node[2] - node[0] > node[3] - node[1], "the fallback's box is the run's own, not turned");
    }
}
