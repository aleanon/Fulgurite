//! Observing a frame must not change it.
//!
//! `audit.rs`'s module doc names, as a scar adopted from Lumen's own
//! review rather than re-earned, that "Lumen's `lint()` was re-shaping
//! every text node per call". The intrinsic probe then measured every
//! leaf twice per frame *through `layout`*, which writes `Label`'s
//! measurement memo — so the memo ended each frame keyed on the probe's
//! constraints and missed on the next one. Measured before the fix: 0
//! text measurements per idle frame with no agent, 400 of 400 with the
//! agent attached. `docs/plan-review-fixes.md` F2.1.

use lumen_static::cx::TextMeasure;
use lumen_static::ops::{Fixed, Label};
use lumen_static::widget::Node;
use lumen_static::{pass, Constraints, LayoutCx, NodeState, Size, StaticTree, Widget};
use lumen_static_agent::{Agent, TreeMut};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("a")]
    a: Label,
    #[widget_id("b")]
    b: Label,
    mark: Fixed,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 400.0 };

/// Counts every call that reaches the text stack.
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
    agent: Agent,
    metrics: Counting,
}

impl Fixture {
    fn new() -> Self {
        let mut ui = Ui::default();
        ui.a.set("first label");
        ui.b.set("second label");
        ui.mark = Fixed { w: 10.0, h: 10.0 };
        let styles = pass::default_styles(&ui);
        Fixture {
            ui,
            st: [NodeState::default(); Ui::NODE_COUNT],
            styles,
            agent: Agent::new(),
            metrics: Counting::default(),
        }
    }

    /// One frame's layout, reporting how many measurements it needed.
    fn layout(&mut self) -> usize {
        let before = self.metrics.calls.get();
        let mut cx = LayoutCx::new(&mut self.metrics);
        self.ui.layout(&mut cx, VIEW, &self.styles, &mut self.st, 0);
        self.metrics.calls.get() - before
    }

    fn observe(&mut self) {
        let mut tree = TreeMut::of_root(&mut self.ui, &self.styles, &self.st);
        self.agent.frame_done(&mut tree, lumen_static_agent::FrameTiming::passes(1, 1));
    }
}

#[test]
fn observing_a_frame_does_not_force_a_re_measure_on_the_next_one() {
    let mut f = Fixture::new();
    assert_eq!(f.layout(), 2, "first frame measures both labels");
    assert_eq!(f.layout(), 0, "an idle frame measures nothing — the memo holds");

    f.observe();
    assert_eq!(
        f.layout(),
        0,
        "and it still measures nothing after the agent has looked at it"
    );

    // Repeatedly, since the failure was per-frame and unbounded.
    for _ in 0..10 {
        f.observe();
        assert_eq!(f.layout(), 0, "every subsequent frame too");
    }
}

#[test]
fn the_memo_stays_warm_across_frame_done() {
    let mut f = Fixture::new();
    f.layout();
    assert!(f.ui.a.measure_is_warm(VIEW, &f.styles[1]), "warm before");
    f.observe();
    assert!(f.ui.a.measure_is_warm(VIEW, &f.styles[1]), "and warm after — the probe left no trace");
}

#[test]
fn a_real_change_still_re_measures_exactly_one_label() {
    let mut f = Fixture::new();
    f.layout();
    f.observe();
    f.ui.b.set("a different second label");
    assert_eq!(f.layout(), 1, "only the label that changed");
}
