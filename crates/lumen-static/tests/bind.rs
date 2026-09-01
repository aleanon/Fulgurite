//! T6.2: the binding runtime — output-keyed change detection, allocation
//! reuse, and the measurement memo it exists to protect.

use lumen_static::bind::{format_into, set_if_changed};
use lumen_static::ops::Label;
use lumen_static::text::TextBuffer;
use lumen_static::{BlockMetrics, Constraints, LayoutCx, Node, NodeState, Style};

const LIMITS: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 400.0 };

/// Lay a label out once so its measurement memo is warm.
fn warm(l: &mut Label<String>) {
    let mut m = BlockMetrics;
    let mut cx = LayoutCx::new(&mut m);
    let (style, mut st) = ([Style::default()], [NodeState::default()]);
    l.layout(&mut cx, LIMITS, &style, &mut st, 0);
}

#[test]
fn an_unchanged_value_does_not_invalidate_the_measurement() {
    let mut l: Label<String> = Label::new("count: 7".into());
    warm(&mut l);
    assert!(l.measure_is_warm(LIMITS));

    assert!(!format_into(&mut l, format_args!("count: {}", 7)), "no change reported");
    assert!(l.measure_is_warm(LIMITS), "and the memo survived — the point of the compare");

    assert!(format_into(&mut l, format_args!("count: {}", 8)));
    assert!(!l.measure_is_warm(LIMITS), "a real change re-measures");
}

#[test]
fn detection_cannot_be_defeated_by_mutating_the_source() {
    // The source is a plain field with no dirty bit and no setter to go
    // through; the binding still notices, because it compares output.
    let mut count = 1i64;
    let mut l: Label<String> = Label::new(String::new());
    assert!(format_into(&mut l, format_args!("{count}")));

    count = 2; // direct mutation, nothing observes it
    assert!(format_into(&mut l, format_args!("{count}")), "output compare caught it");
    assert_eq!(l.as_str(), "2");
}

#[test]
fn distinct_sources_that_format_alike_are_correctly_treated_as_equal() {
    // Rounding to one decimal: 1.44 and 1.44999 differ as values but not
    // as text, and re-shaping identical text would be wasted work.
    let mut l: Label<String> = Label::new(String::new());
    assert!(format_into(&mut l, format_args!("{:.1}", 1.44_f32)));
    warm(&mut l);
    assert!(!format_into(&mut l, format_args!("{:.1}", 1.44999_f32)));
    assert!(l.measure_is_warm(LIMITS));
}

#[test]
fn updating_a_string_label_reuses_its_allocation() {
    let mut s = String::with_capacity(64);
    s.push_str("hello");
    let before = s.capacity();
    s.set_from_str("a different string");
    assert_eq!(s, "a different string");
    assert_eq!(s.capacity(), before, "capacity kept: no allocation in the steady state");
}

#[test]
fn an_owned_cow_reuses_its_buffer_and_a_borrowed_one_becomes_owned() {
    let mut c: std::borrow::Cow<'static, str> = std::borrow::Cow::Borrowed("literal");
    c.set_from_str("computed");
    assert!(matches!(c, std::borrow::Cow::Owned(_)), "a literal has no buffer to reuse");
    let cap = match &c {
        std::borrow::Cow::Owned(s) => s.capacity(),
        _ => unreachable!(),
    };
    c.set_from_str("shorter");
    assert_eq!(c, "shorter");
    assert!(matches!(&c, std::borrow::Cow::Owned(s) if s.capacity() == cap), "buffer reused");
}

#[test]
fn set_if_changed_is_the_non_formatting_form() {
    let mut l: Label<String> = Label::new("same".into());
    warm(&mut l);
    assert!(!set_if_changed(&mut l, "same"));
    assert!(l.measure_is_warm(LIMITS));
    assert!(set_if_changed(&mut l, "other"));
    assert_eq!(l.as_str(), "other");
}

#[test]
fn a_nested_binding_does_not_deadlock_the_shared_buffer() {
    // A Display impl that itself runs a binding re-enters format_into.
    // Taking the scratch buffer rather than borrowing it makes that
    // merely slower instead of a panic.
    struct Nested;
    impl std::fmt::Display for Nested {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let mut inner: Label<String> = Label::new(String::new());
            format_into(&mut inner, format_args!("inner"));
            write!(f, "[{}]", inner.as_str())
        }
    }
    let mut l: Label<String> = Label::new(String::new());
    assert!(format_into(&mut l, format_args!("outer {}", Nested)));
    assert_eq!(l.as_str(), "outer [inner]");
}
