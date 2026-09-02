//! A repeater's rows must not re-measure when nothing about them
//! changed.
//!
//! `Repeat::layout` calls `bind(row, index)` for every materialized row
//! on every layout — that is how a virtualized window gets its content.
//! The obvious bind body is `row.label.set_str(..)`, and until
//! 2026-09-02 `set_str` dirtied unconditionally, so every visible row
//! re-measured on every frame including idle ones: 62 text measurements
//! per frame for a 31-row window, in the one widget whose whole purpose
//! is to make large lists cheap. Nothing in `fn(&mut T, usize)` steered
//! an author away from it (`docs/plan-review-fixes.md` F4.6).

use lumen_static::cx::TextMeasure;
use lumen_static::ops::Label;
use lumen_static::repeat::Repeat;
use lumen_static::widget::Node;
use lumen_static::{pass, Constraints, LayoutCx, NodeState, Size, StaticTree, Widget};

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Row {
    name: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    list: Repeat<Row>,
}

/// The natural bind: write this row's text every time.
fn bind_row(row: &mut Row, i: usize) {
    row.name.set_str(&format!("row {i}"));
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 600.0 };

#[derive(Default)]
struct Counting {
    calls: std::cell::Cell<usize>,
}

impl TextMeasure for Counting {
    fn measure(&mut self, text: &str, size: f32, max_w: f32) -> Size {
        self.calls.set(self.calls.get() + 1);
        lumen_static::BlockMetrics.measure(text, size, max_w)
    }
}

struct Fixture {
    ui: Ui,
    st: [NodeState; Ui::NODE_COUNT],
    styles: Vec<lumen_static::Style>,
    m: Counting,
}

impl Fixture {
    fn new() -> Self {
        let mut ui = Ui { list: Repeat::new(10_000, 20.0, bind_row) };
        ui.list.scroll = 0.0;
        let styles = pass::default_styles(&ui);
        Fixture { ui, st: [NodeState::default(); Ui::NODE_COUNT], styles, m: Counting::default() }
    }

    /// One layout, returning how many measurements it needed.
    fn layout(&mut self) -> usize {
        let before = self.m.calls.get();
        let mut cx = LayoutCx::new(&mut self.m);
        let ui = &mut self.ui;
        ui.layout(&mut cx, VIEW, &self.styles, &mut self.st, 0);
        self.m.calls.get() - before
    }
}

#[test]
fn an_idle_repeater_frame_re_measures_nothing() {
    let mut f = Fixture::new();
    let first = f.layout();
    assert!(first > 0, "the first frame measures the window ({first} rows)");
    assert_eq!(f.ui.list.window_len(), 31, "600px viewport / 20px rows, plus the partial");

    for _ in 0..10 {
        assert_eq!(f.layout(), 0, "an idle frame measures nothing");
    }
}

#[test]
fn scrolling_by_one_row_re_measures_only_what_changed() {
    let mut f = Fixture::new();
    f.layout();

    // Scroll exactly one row. The window is slot-indexed, so every
    // slot's *row index* shifts — but only one row's text is new to the
    // window, and `set_str` compares, so only the slots whose text
    // actually differs re-measure.
    f.ui.list.scroll = 20.0;
    let n = f.layout();
    assert_eq!(
        n, 1,
        "a one-row scroll re-measures exactly the row that entered, of {}",
        f.ui.list.window_len()
    );

    // Sub-item scroll: no row enters or leaves at all.
    f.ui.list.scroll = 27.0;
    assert_eq!(f.layout(), 0, "a scroll within one item changes no row's content");
}
