//! M26/T26.8: every text widget memoizes, and a still frame shapes
//! nothing.
//!
//! `Badge` and `Chip` had **no memo at all** — they shaped on every
//! layout, which `--bin bindings` measured at ~410 ns against a 4.97 ns
//! hit. `Button` open-coded the one it needed. `Label` had the helper.
//! Three conventions, two of them absent, in five widgets.
//!
//! The gate counts: a still frame must ask the text stack nothing, for
//! every one of them, and it names the widget that asked.

use core::cell::Cell;

use lumen_static::cx::{BlockMetrics, TextMeasure};
use lumen_static_widgets::ops::{Button, Label};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, NodeState, Point, Size};
use lumen_static_widgets::{Badge, Chip};

#[derive(Default)]
struct Counting {
    inner: BlockMetrics,
    calls: Cell<u32>,
}

impl TextMeasure for Counting {
    fn measure(&mut self, text: &str, size: f32, max_w: f32) -> Size {
        self.calls.set(self.calls.get() + 1);
        self.inner.measure(text, size, max_w)
    }
    fn measure_weighted(&mut self, text: &str, size: f32, weight: f32, max_w: f32) -> Size {
        self.calls.set(self.calls.get() + 1);
        self.inner.measure_weighted(text, size, weight, max_w)
    }
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 300.0, min_h: 0.0, max_h: 200.0 };

/// Lay `w` out `n` times and answer how many calls the last `n - 1`
/// made — the first is the cold one every memo is allowed.
fn still_frames<W: Widget + Node>(w: &mut W, n: u32) -> u32 {
    let styles = vec![<W as Node>::defaults(w); <W as Widget>::NODE_COUNT];
    let mut st = vec![NodeState::default(); <W as Widget>::NODE_COUNT];
    let mut m = Counting::default();
    {
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(w, &mut cx, VIEW, &styles, &mut st, 0);
    }
    let cold = m.calls.get();
    assert!(cold > 0, "setup: the first layout measured nothing at all");
    for _ in 1..n {
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(w, &mut cx, VIEW, &styles, &mut st, 0);
    }
    Node::place(w, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);
    m.calls.get() - cold
}

#[test]
fn a_still_frame_measures_nothing_in_any_text_widget() {
    let mut label: Label<String> = Label::new("hello".into());
    assert_eq!(still_frames(&mut label, 6), 0, "Label measured on a still frame");

    let mut button: Button<(), String> = Button::default();
    button.label = "hello".into();
    assert_eq!(still_frames(&mut button, 6), 0, "Button measured on a still frame");

    let mut badge: Badge<String> = Badge::new("9".into());
    assert_eq!(still_frames(&mut badge, 6), 0, "Badge measured on a still frame");

    let mut chip: Chip<String> = Chip::new("tag".into());
    assert_eq!(still_frames(&mut chip, 6), 0, "Chip measured on a still frame");
}

/// And the memo is keyed, not merely present: changed text is measured
/// again. A memo that never misses has stopped answering.
#[test]
fn changed_text_is_measured_again() {
    let mut badge: Badge<String> = Badge::new("9".into());
    let styles = vec![<Badge<String> as Node>::defaults(&badge)];
    let mut st = vec![NodeState::default(); 1];
    let mut m = Counting::default();
    let lay = |b: &mut Badge<String>, m: &mut Counting, st: &mut Vec<NodeState>| {
        let mut cx = lumen_static::LayoutCx::new(m);
        Node::layout(b, &mut cx, VIEW, &styles, st, 0)
    };
    let first = lay(&mut badge, &mut m, &mut st);
    let after_cold = m.calls.get();
    badge.set("99+".into());
    let second = lay(&mut badge, &mut m, &mut st);
    assert!(m.calls.get() > after_cold, "new text was not measured");
    assert!(
        second.w > first.w,
        "the badge did not grow with its text: {} -> {}",
        first.w,
        second.w
    );
}

/// A restyle is a miss too — the key carries the size and the weight,
/// which is what M26/T26.3 put in it.
#[test]
fn a_restyle_is_a_miss() {
    let mut chip: Chip<String> = Chip::new("tag".into());
    let mut styles = vec![<Chip<String> as Node>::defaults(&chip)];
    let mut st = vec![NodeState::default(); 1];
    let mut m = Counting::default();
    {
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(&mut chip, &mut cx, VIEW, &styles, &mut st, 0);
    }
    let cold = m.calls.get();
    styles[0].font_size *= 2.0;
    {
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(&mut chip, &mut cx, VIEW, &styles, &mut st, 0);
    }
    assert!(
        m.calls.get() > cold,
        "doubling the font size was served from the memo: the key does not \
         carry the size"
    );
}

/// Ignored by the counter above but asserted here: the helper's own
/// `Style` type. A `Style` with no text in it is still a key.
#[test]
fn an_empty_string_is_still_memoized() {
    let mut badge: Badge<String> = Badge::new(String::new());
    assert_eq!(still_frames(&mut badge, 4), 0, "an empty badge re-measured");
}
