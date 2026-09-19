//! M26/T26.7: the text widgets stop measuring what shaping returned.
//!
//! `TextMeasure::shape` **returns a `Size`** — it always has. `input.rs`
//! threw it away and called `measure_weighted` on the same run on the
//! next line, unconditionally, every layout, including the frames its
//! own shape cache hit. `TextArea` did the same.
//!
//! The gate is a **counting measurer**: it wraps the block stand-in and
//! records every call. A frame in which nothing about the text changed
//! must ask it nothing.

use core::cell::Cell;

use lumen_static::cx::{BlockMetrics, TextMeasure};
use lumen_static::input::TextInput;
use lumen_static::textarea::TextArea;
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, NodeState, Point, Size, Style};

#[derive(Default)]
struct Counting {
    inner: BlockMetrics,
    measures: Cell<u32>,
    shapes: Cell<u32>,
    lines: Cell<u32>,
}

impl TextMeasure for Counting {
    fn measure(&mut self, text: &str, size: f32, max_w: f32) -> Size {
        self.measures.set(self.measures.get() + 1);
        self.inner.measure(text, size, max_w)
    }

    fn measure_weighted(&mut self, text: &str, size: f32, weight: f32, max_w: f32) -> Size {
        self.measures.set(self.measures.get() + 1);
        self.inner.measure_weighted(text, size, weight, max_w)
    }

    fn shape(
        &mut self,
        text: &str,
        size: f32,
        weight: f32,
        max_w: f32,
        sink: &mut dyn lumen_static::cx::GlyphSink,
    ) -> Size {
        self.shapes.set(self.shapes.get() + 1);
        // Counted separately, and *not* forwarded to `measure`: a shape
        // that went through this type's own `measure_weighted` would
        // make every shape look like a measure and the counts below
        // would be the same number twice.
        self.inner.shape(text, size, weight, max_w, sink)
    }

    fn line_height(&mut self, size: f32) -> f32 {
        self.lines.set(self.lines.get() + 1);
        self.inner.line_height(size)
    }
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 200.0 };

fn lay<W: Widget + Node>(w: &mut W, m: &mut Counting, styles: &[Style], st: &mut [NodeState]) {
    let mut cx = lumen_static::LayoutCx::new(m);
    Node::layout(w, &mut cx, VIEW, styles, st, 0);
    Node::place(w, Point { x: 0.0, y: 0.0 }, styles, st, 0);
}

#[test]
fn an_unchanged_field_asks_the_text_stack_nothing() {
    let mut f = TextInput::<String>::new("hello there");
    let styles = lumen_static::pass::default_styles(&f);
    let mut st = vec![NodeState::default(); <TextInput<String> as Widget>::NODE_COUNT];
    let mut m = Counting::default();

    lay(&mut f, &mut m, &styles, &mut st);
    let (first_shapes, first_measures) = (m.shapes.get(), m.measures.get());
    assert!(first_shapes > 0, "setup: the first layout shaped nothing");

    for _ in 0..5 {
        lay(&mut f, &mut m, &styles, &mut st);
    }
    assert_eq!(
        m.shapes.get(),
        first_shapes,
        "five still frames re-shaped: the shape cache is not hitting at all"
    );
    assert_eq!(
        m.measures.get(),
        first_measures,
        "five still frames measured {} more times — `shape` already \
         returned that size",
        m.measures.get() - first_measures
    );
}

/// And the width it reuses is the one shaping gave it: a cached number
/// that is *wrong* is worse than a measured one.
#[test]
fn the_cached_width_is_the_measured_width() {
    let mut f = TextInput::<String>::new("hello there");
    let styles = lumen_static::pass::default_styles(&f);
    let mut st = vec![NodeState::default(); <TextInput<String> as Widget>::NODE_COUNT];
    let mut m = Counting::default();
    lay(&mut f, &mut m, &styles, &mut st);
    let cached = st[0].size.w;

    let mut plain = BlockMetrics;
    let want = plain.measure_weighted("hello there", styles[0].font_size, styles[0].font_weight, f32::INFINITY).w
        + styles[0].padding.horizontal();
    assert!(
        (cached - want).abs() < 0.5,
        "the field laid out {cached} px wide; measuring its text says {want}"
    );
}

/// The cache is keyed, so *changed* text is re-shaped and re-measured.
/// A memo that never misses is a memo that has stopped answering.
#[test]
fn changed_text_is_shaped_again() {
    let mut f = TextInput::<String>::new("hello");
    let styles = lumen_static::pass::default_styles(&f);
    let mut st = vec![NodeState::default(); <TextInput<String> as Widget>::NODE_COUNT];
    let mut m = Counting::default();
    lay(&mut f, &mut m, &styles, &mut st);
    let before = m.shapes.get();
    let narrow = st[0].size.w;

    f.set_str("hello there, considerably longer");
    lay(&mut f, &mut m, &styles, &mut st);
    assert!(m.shapes.get() > before, "new text was not shaped");
    assert!(
        st[0].size.w > narrow,
        "the box did not grow with the text: {narrow} -> {}",
        st[0].size.w
    );
}

#[test]
fn an_unchanged_area_asks_the_text_stack_nothing() {
    let mut a = TextArea::<String>::new("two words\nand a second line");
    let styles = lumen_static::pass::default_styles(&a);
    let mut st = vec![NodeState::default(); <TextArea<String> as Widget>::NODE_COUNT];
    let mut m = Counting::default();

    lay(&mut a, &mut m, &styles, &mut st);
    let (shapes, measures, lines) = (m.shapes.get(), m.measures.get(), m.lines.get());
    assert!(shapes > 0, "setup: the first layout shaped nothing");

    for _ in 0..5 {
        lay(&mut a, &mut m, &styles, &mut st);
    }
    assert_eq!(m.shapes.get(), shapes, "five still frames re-shaped the area");
    assert_eq!(
        m.measures.get(),
        measures,
        "five still frames measured {} more times",
        m.measures.get() - measures
    );
    assert_eq!(
        m.lines.get(),
        lines,
        "five still frames asked for the line height {} more times",
        m.lines.get() - lines
    );
}
