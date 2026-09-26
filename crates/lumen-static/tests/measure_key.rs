//! M26/T26.3: a label's measurement memo must miss when the font size or
//! weight changes, not only when the text or the limits do.
//!
//! `Measured` keyed on the constraints and a dirty bit, while
//! `Label::layout` read the size and weight *inside* the closure it hands
//! the memo — so neither ever entered the key. A restyle that changed only
//! a label's font size, under unchanged limits, was served last frame's
//! box. Nothing in the tree tripped it: `examples/counter-window` keeps the
//! same size in every arm of its sign-dependent sheet.
//!
//! **Why a measurer of its own.** `BlockMetrics` ignores weight, so a test
//! run against it cannot tell a key that holds the weight from one that
//! drops it — the weight half of this defect would pass with the bug in
//! place. `Weighted` makes width depend on weight as well as size, and
//! counts its calls so the good-input half (an unchanged style still hits
//! the memo) is asserted, not assumed.

use std::cell::Cell;

use lumen_static_widgets::ops::Label;
use lumen_static::{Constraints, LayoutCx, Node, NodeState, Size, Style, TextMeasure};

const LIMITS: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 400.0 };

/// Width = chars × size × weight / 800, so a change in either moves it.
#[derive(Default)]
struct Weighted {
    calls: Cell<usize>,
}

impl TextMeasure for Weighted {
    fn measure(&mut self, text: &str, size: f32, max_w: f32) -> Size {
        self.measure_weighted(text, size, 400.0, max_w)
    }
    fn measure_weighted(&mut self, text: &str, size: f32, weight: f32, _max_w: f32) -> Size {
        self.calls.set(self.calls.get() + 1);
        Size { w: text.len() as f32 * size * weight / 800.0, h: size }
    }
}

/// Lay `l` out once under `style` and return its width.
fn width(l: &mut Label<String>, m: &mut Weighted, style: Style) -> f32 {
    let mut cx = LayoutCx::new(m);
    let (sty, mut st) = ([style], [NodeState::default()]);
    l.layout(&mut cx, LIMITS, &sty, &mut st, 0).w
}

#[test]
fn a_font_size_change_under_the_same_limits_re_measures() {
    let mut m = Weighted::default();
    let mut l: Label<String> = Label::new("hello".into());
    let small = Style::default();
    let large = Style { font_size: small.font_size * 2.0, ..small };

    let w_small = width(&mut l, &mut m, small);
    let w_large = width(&mut l, &mut m, large);
    println!("16 px: {w_small}; 32 px, same text and limits: {w_large}");
    assert_ne!(w_small, 0.0, "setup: the label measured something");
    assert_eq!(
        w_large,
        w_small * 2.0,
        "the memo served the old font size's box: the key does not hold the size"
    );
}

#[test]
fn a_font_weight_change_under_the_same_limits_re_measures() {
    let mut m = Weighted::default();
    let mut l: Label<String> = Label::new("hello".into());
    let regular = Style::default();
    let bold = Style { font_weight: 800.0, ..regular };
    assert_ne!(regular.font_weight, bold.font_weight, "setup: the weights differ");

    let w_regular = width(&mut l, &mut m, regular);
    let w_bold = width(&mut l, &mut m, bold);
    println!("weight {}: {w_regular}; weight 800: {w_bold}", regular.font_weight);
    assert_eq!(
        w_bold,
        w_regular * 2.0,
        "the memo served the old weight's box: the key does not hold the weight"
    );
}

/// `Button` was already right — its private key held the size and weight —
/// and M26/T26.3 moved it onto the shared `MeasureKey`. This guards that
/// move: a button must still re-measure on a font change, and still hit
/// the memo when nothing changed.
#[test]
fn a_button_re_measures_on_a_font_change_and_hits_when_nothing_changed() {
    let mut m = Weighted::default();
    let mut b = lumen_static_widgets::ops::Button::<(), String>::default();
    b.label = "go".into();
    let base = b.defaults();
    let large = Style { font_size: base.font_size * 2.0, ..base };
    let lay = |b: &mut lumen_static_widgets::ops::Button<(), String>, m: &mut Weighted, s: Style| {
        let mut cx = LayoutCx::new(m);
        let (sty, mut st) = ([s], [NodeState::default()]);
        b.layout(&mut cx, LIMITS, &sty, &mut st, 0).w
    };

    let w1 = lay(&mut b, &mut m, base);
    let calls = m.calls.get();
    let w1_again = lay(&mut b, &mut m, base);
    let w2 = lay(&mut b, &mut m, large);
    println!("button: {w1} -> {w1_again} (same style) -> {w2} (double size)");
    assert_eq!(m.calls.get(), calls + 1, "the unchanged layout re-measured, or the changed one did not");
    assert_eq!(w1, w1_again);
    assert_ne!(w2, w1, "a button served its old box after a font change");
}

/// The other direction: widening the key must not have broken the memo.
/// An unchanged style under unchanged limits still measures once.
#[test]
fn an_unchanged_style_still_hits_the_memo() {
    let mut m = Weighted::default();
    let mut l: Label<String> = Label::new("hello".into());
    let style = Style { font_size: 20.0, font_weight: 600.0, ..Style::default() };
    for _ in 0..5 {
        width(&mut l, &mut m, style);
    }
    println!("measure calls over five identical layouts: {}", m.calls.get());
    assert_eq!(m.calls.get(), 1, "an unchanged style re-measured: the memo no longer hits");
}

/// **A NaN limit still hits the memo** — M45/T45.0 item 5, R2-F10
/// (`docs/plan-text-correctness.md` §1.2).
///
/// `MeasureKey` stored its `Constraints` by value and compared them with
/// `f32`'s `PartialEq`, under which a NaN is unequal to itself — so a
/// label laid out under a NaN limit never hit, while the rustdoc's "bit
/// patterns, not values" covered only the other fields. Nothing in the
/// tree produces a NaN limit today: this is a memo miss, not a stale box.
#[test]
fn five_layouts_under_a_nan_limit_measure_once() {
    let mut m = Weighted::default();
    let mut l: Label<String> = Label::new("hello".into());
    let nan = Constraints { max_w: f32::NAN, ..LIMITS };
    for _ in 0..5 {
        let mut cx = LayoutCx::new(&mut m);
        let (sty, mut st) = ([Style::default()], [NodeState::default()]);
        l.layout(&mut cx, nan, &sty, &mut st, 0);
    }
    println!("five layouts at NaN max_w: {} measurer calls (bound 1)", m.calls.get());
    assert_eq!(m.calls.get(), 1, "the memo missed on every NaN layout");
}
