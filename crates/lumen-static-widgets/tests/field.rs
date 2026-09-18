//! M27/T27.5 — `NumberInput`, `RangeSlider`, `SearchField`.
//!
//! `NumberInput`'s eleven actions are `docs/plan-widget-parity.md` §4's
//! worked inventory, written **before** the widget existed. This file is
//! that table as assertions, which is the point of writing it first: an
//! inventory produced afterwards describes what was built.

use lumen_static::event::{Event, EventCtx, Key};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, NodeState, Point};
use lumen_static_widgets::{NumberInput, RangeSlider, SearchField};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 100.0 };

fn key(n: &mut dyn Node, k: Key) -> bool {
    let ev = Event::Key { key: k, down: true };
    n.on_event(&ev, &EventCtx::for_event(0, ev))
}

fn typed(n: &mut dyn Node, s: &str) {
    for ch in s.chars() {
        let ev = Event::Text { ch };
        n.on_event(&ev, &EventCtx::for_event(0, ev));
    }
}

// ---- NumberInput: the inventory -------------------------------------

/// The seed §4 names: min 0, max 10, step 0.5, opening at 3.0 —
/// mid-range on purpose, so a clamp test can reach a boundary and a step
/// test can be told from a jump to an end.
fn seeded() -> NumberInput {
    let mut n = NumberInput::new(3.0, 0.0, 10.0, 0.5);
    n.decimals = 1;
    n.set_value(3.0);
    n
}

/// (2) Typing digits reaches the text.
#[test]
fn typing_digits_reaches_the_field() {
    let mut n = seeded();
    n.set_value(0.0);
    // Start from a known text rather than appending to "0.0".
    let before = n.as_str().to_owned();
    typed(&mut n, "7");
    assert_eq!(n.as_str(), format!("{before}7"), "a digit did not reach the text");
}

/// (3) A letter is refused, and is **not** silently lost and then undone
/// on blur: the text is unchanged the instant it is typed.
#[test]
fn a_letter_is_refused_outright() {
    let mut n = seeded();
    let before = n.as_str().to_owned();
    typed(&mut n, "abc");
    assert_eq!(n.as_str(), before, "letters reached a number field: {:?}", n.as_str());
    // And the widget consumed them, so a parent does not treat them as
    // unhandled text and act on them itself.
    let ev = Event::Text { ch: 'z' };
    assert!(n.on_event(&ev, &EventCtx::for_event(0, ev)), "a refused letter was passed on");
}

/// (4) Enter commits what was typed, and (10) select-all replaces
/// rather than appends.
///
/// The two belong together, because **typing appends at the caret** —
/// which is what any field does and what makes "type 5" give `0.05`
/// rather than `5`. The first version of this test asserted the wrong
/// behaviour and read `0.05`; the widget was right.
#[test]
fn select_all_then_typing_replaces_and_enter_commits() {
    let mut n = seeded();
    n.set_value(0.0);
    assert!(n.select_all(), "select_all reported nothing to select");
    typed(&mut n, "5");
    assert_eq!(n.as_str(), "5", "typing over a selection did not replace it");
    key(&mut n, Key::Enter);
    assert_eq!(n.value(), 5.0, "Enter did not commit; text is {:?}", n.as_str());
}

/// And without the selection, typing appends — asserted, so the
/// behaviour above is a choice rather than an accident.
#[test]
fn typing_without_a_selection_appends_at_the_caret() {
    let mut n = seeded();
    n.set_value(0.0);
    let before = n.as_str().to_owned();
    typed(&mut n, "5");
    assert_eq!(n.as_str(), format!("{before}5"), "typing did not append at the caret");
}

/// (5) Committing normalises: "007" becomes "7.0", not "007".
#[test]
fn committing_normalises_the_text() {
    let mut n = seeded();
    n.set_value(0.0);
    n.select_all();
    typed(&mut n, "007");
    key(&mut n, Key::Enter);
    assert_eq!(n.value(), 7.0);
    assert_eq!(n.as_str(), "7.0", "the text was not rewritten from the value");
}

/// (6) A bare "-" reverts to the last committed value rather than
/// clearing. Blanking loses the one number the user cannot retype from
/// the screen.
#[test]
fn an_unparsable_partial_reverts() {
    let mut n = seeded();
    assert_eq!(n.value(), 3.0);
    for _ in 0..8 {
        key(&mut n, Key::Backspace);
    }
    typed(&mut n, "-");
    key(&mut n, Key::Enter);
    assert_eq!(n.value(), 3.0, "an unparsable field lost the committed value");
    assert_eq!(n.as_str(), "3.0", "the text was not put back");
}

/// (7) An arrow moves by exactly `step`.
#[test]
fn an_arrow_moves_by_one_step() {
    let mut n = seeded();
    key(&mut n, Key::Up);
    assert_eq!(n.value(), 3.5, "Up moved to {} from 3.0 at step 0.5", n.value());
    key(&mut n, Key::Down);
    key(&mut n, Key::Down);
    assert_eq!(n.value(), 2.5);
}

/// (8) It clamps at a bound rather than wrapping.
#[test]
fn it_clamps_at_the_minimum_and_does_not_wrap() {
    let mut n = seeded();
    for _ in 0..20 {
        key(&mut n, Key::Down);
    }
    assert_eq!(n.value(), 0.0, "value is {} — 20 steps of 0.5 from 3.0 must stop at min", n.value());
    for _ in 0..40 {
        key(&mut n, Key::Up);
    }
    assert_eq!(n.value(), 10.0, "value is {} — it must stop at max, not wrap", n.value());
}

/// (9) PageUp is ten steps, not one and not an end.
#[test]
fn page_up_is_ten_steps() {
    let mut n = seeded();
    n.set_value(0.0);
    key(&mut n, Key::PageUp);
    assert_eq!(
        n.value(), 5.0,
        "PageUp gave {} — one step is 0.5, ten steps is 5.0, and the max is 10.0",
        n.value()
    );
}

/// (1) and the blur half of (5): focus is state, not an event, so the
/// commit happens in `layout`.
#[test]
fn losing_focus_commits() {
    let mut n = seeded();
    let styles = vec![<NumberInput as Widget>::type_defaults()];
    let mut st = vec![NodeState::default()];
    let mut m = lumen_static::cx::BlockMetrics;

    let mut frame = |n: &mut NumberInput, st: &mut Vec<NodeState>| {
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(n, &mut cx, VIEW, &styles, st, 0);
    };

    st[0].focused = true;
    frame(&mut n, &mut st);
    for _ in 0..8 {
        key(&mut n, Key::Backspace);
    }
    typed(&mut n, "9");
    assert_eq!(n.value(), 3.0, "setup: typing must not commit on its own");
    st[0].focused = false;
    frame(&mut n, &mut st);
    assert_eq!(n.value(), 9.0, "losing focus did not commit; text is {:?}", n.as_str());
}

/// A value out of range is clamped when set, not stored and clamped
/// later — a field that reports 99 while showing 10 is two answers.
#[test]
fn set_value_clamps_and_the_text_follows() {
    let mut n = seeded();
    n.set_value(99.0);
    assert_eq!(n.value(), 10.0);
    assert_eq!(n.as_str(), "10.0", "the text disagrees with the value");
}

// ---- RangeSlider -----------------------------------------------------

#[test]
fn the_handles_cannot_cross() {
    let mut r = RangeSlider::new(0.3, 0.7);
    r.set_low(0.9);
    assert_eq!(r.low(), 0.7, "the low handle passed the high one: {} > {}", r.low(), r.high());
    r.set_high(0.1);
    assert_eq!(r.high(), r.low(), "the high handle passed the low one");
}

#[test]
fn a_press_takes_the_nearer_handle() {
    let mut r = RangeSlider::new(0.2, 0.8);
    // Lay out so the track width is known: `on_event` sees only `local`.
    let styles = vec![<RangeSlider as Widget>::type_defaults()];
    let mut st = vec![NodeState::default()];
    let mut m = lumen_static::cx::BlockMetrics;
    let mut cx = lumen_static::LayoutCx::new(&mut m);
    Node::layout(&mut r, &mut cx, VIEW, &styles, &mut st, 0);
    let w = st[0].size.w;
    assert!(w > 0.0, "setup: the track has no width");

    // A press at 25% is nearest the low handle.
    let ev = Event::PointerDown {
        x: 0.0,
        y: 0.0,
        button: lumen_static::PointerButton::Left,
        clicks: 1,
    };
    let mut cx0 = EventCtx::for_event(0, ev);
    cx0.local = Point { x: w * 0.25, y: 5.0 };
    r.on_event(&ev, &cx0);
    assert!(
        (r.low() - 0.25).abs() < 0.02 && r.high() == 0.8,
        "a press at 25% moved low={} high={}, expected the low handle only",
        r.low(), r.high()
    );
}

/// A range collapsed to a point must still be openable — which is why
/// an equidistant press takes the **high** handle.
#[test]
fn a_collapsed_range_can_be_reopened() {
    let mut r = RangeSlider::new(0.5, 0.5);
    let styles = vec![<RangeSlider as Widget>::type_defaults()];
    let mut st = vec![NodeState::default()];
    let mut m = lumen_static::cx::BlockMetrics;
    let mut cx = lumen_static::LayoutCx::new(&mut m);
    Node::layout(&mut r, &mut cx, VIEW, &styles, &mut st, 0);
    let w = st[0].size.w;
    let ev = Event::PointerDown {
        x: 0.0,
        y: 0.0,
        button: lumen_static::PointerButton::Left,
        clicks: 1,
    };
    let mut cx0 = EventCtx::for_event(0, ev);
    cx0.local = Point { x: w * 0.5, y: 5.0 };
    r.on_event(&ev, &cx0);
    // Now drag right; the high handle must follow.
    let mv = Event::PointerMove { x: 0.0, y: 0.0 };
    let mut cx1 = EventCtx::for_event(0, mv);
    cx1.local = Point { x: w * 0.9, y: 5.0 };
    r.on_event(&mv, &cx1);
    assert!(
        r.high() > 0.8,
        "a collapsed range would not open: low={} high={} — an equidistant press \
         that took the *low* handle pins it, because set_low clamps to high",
        r.low(), r.high()
    );
}

/// Tab reaches the other handle, or a keyboard can only ever move one.
#[test]
fn tab_switches_which_handle_the_keyboard_moves() {
    let mut r = RangeSlider::new(0.2, 0.8);
    key(&mut r, Key::Right);
    assert!(r.low() > 0.2 && r.high() == 0.8, "the first arrow did not move the low handle");
    key(&mut r, Key::Tab);
    let before = r.high();
    key(&mut r, Key::Right);
    assert!(r.high() > before, "Tab did not reach the high handle");
}

// ---- SearchField -----------------------------------------------------

#[test]
fn a_click_anywhere_in_a_search_field_reaches_the_text() {
    let mut s = SearchField::new("query");
    let styles = vec![<SearchField as Widget>::type_defaults(); 3];
    let mut st = vec![NodeState::default(); <SearchField as Widget>::NODE_COUNT];
    let mut m = lumen_static::cx::BlockMetrics;
    {
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(&mut s, &mut cx, VIEW, &styles, &mut st, 0);
    }
    Node::place(&mut s, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);
    // The far right of the box is padding, not text — and must still
    // focus the field rather than the container.
    let r = lumen_static::ops::rect_of(&st[0]);
    let hit = Node::hit(&s, &st, 0, r[2] - 2.0, (r[1] + r[3]) / 2.0)
        .expect("the box did not answer a hit at all");
    assert_eq!(
        hit.node, 2,
        "a click in the padding landed on node {} — the container, which focuses nothing",
        hit.node
    );
}

#[test]
fn the_icon_is_left_of_the_text_and_both_are_inside() {
    let mut s = SearchField::new("query");
    let styles = vec![<SearchField as Widget>::type_defaults(); 3];
    let mut st = vec![NodeState::default(); <SearchField as Widget>::NODE_COUNT];
    let mut m = lumen_static::cx::BlockMetrics;
    {
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(&mut s, &mut cx, VIEW, &styles, &mut st, 0);
    }
    Node::place(&mut s, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);
    let (box_, icon, field) = (
        lumen_static::ops::rect_of(&st[0]),
        lumen_static::ops::rect_of(&st[1]),
        lumen_static::ops::rect_of(&st[2]),
    );
    assert!(icon[2] <= field[0], "the icon overlaps the text: {icon:?} vs {field:?}");
    assert!(
        icon[0] >= box_[0] && field[2] <= box_[2] + 0.01,
        "something escapes the box: icon {icon:?}, field {field:?}, box {box_:?}"
    );
}

#[test]
fn clearing_reports_whether_it_did_anything() {
    let mut s = SearchField::new("query");
    assert!(s.clear(), "clearing a non-empty field reported no change");
    assert_eq!(s.as_str(), "");
    assert!(!s.clear(), "clearing an empty field reported a change — a caller would re-search");
}
