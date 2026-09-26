//! Saved state is the one input the core does not write itself.
//!
//! Every other reload test round-trips `save()` into `restore()`, so the
//! oracle is the encoder and no value outside its range is ever decoded.
//! `item_extent` came back unchecked: a subnormal cleared the `<= 0.0`
//! guard and overflowed `visible_range`'s `as usize + 1` **inside the
//! core**, and `1e-6` with a large `count` asked for hundreds of
//! millions of instances from forty bytes of JSON.
//! `docs/plan-review-fixes.md` F1.4.
//!
//! **Since M49/T49.5 a saved `count` or `item_extent` is not read at
//! all**: both are the build's (a row height is code, and the count comes
//! from the next update's binding), so `Repeat` saves and restores only
//! its scroll. The hostile values below must therefore change nothing,
//! which is a stronger answer than F1.4's range check; `visible_range`'s
//! own clamp is still tested, because a caller can set the field.
//!
//! Hand-written JSON throughout — that is the point.

#![cfg(feature = "reload")]

use lumen_static_widgets::ops::{Fixed, Label};
use lumen_static::reload::{take_log, Reloadable};
use lumen_static::repeat::{noop_bind, Repeat};
use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};
use serde_json::json;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Row {
    icon: Fixed,
    name: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    list: Repeat<Row>,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 600.0 };

/// A list the build made with 1000 rows of 24 px; restore hostile saved
/// state onto it, lay out a full frame, and report the window size.
fn restore_and_lay_out(count: serde_json::Value, extent: serde_json::Value) -> usize {
    let mut ui = Ui { list: Repeat::new(1000, 24.0, noop_bind) };
    ui.list.restore(&json!({ "count": count, "scroll": 0.0, "item_extent": extent }));
    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    pass::layout(&mut ui, &sty, &mut st, VIEW);
    ui.list.window_len()
}

#[test]
fn a_saved_extent_or_count_is_not_read_at_all() {
    for (count, extent) in [
        (json!(1000), json!(1e-40)),
        (json!(u32::MAX), json!(1e-6)),
        (json!(1000), json!(0.0)),
        (json!(1000), json!(-1.0)),
        (json!(1000), json!(1e-7)),
        (json!(u32::MAX), json!(24.0)),
    ] {
        let w = restore_and_lay_out(count.clone(), extent.clone());
        assert_eq!(w, 26, "count {count}, extent {extent}: the build's 1000 x 24 px frame, window {w}");
    }
}

#[test]
fn visible_range_itself_is_bounded_even_if_a_bad_extent_reaches_it() {
    // restore() is one door; a caller can set the field directly. The
    // clamp in `visible_range` is the other half, so the two are tested
    // separately.
    let mut ui = Ui { list: Repeat::new(1000, 24.0, noop_bind) };
    ui.list.item_extent = 1e-40;
    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    pass::layout(&mut ui, &sty, &mut st, VIEW);
    assert_eq!(ui.list.window_len(), 0, "below MIN_ITEM_EXTENT means no rows");

    ui.list.item_extent = 1e-3;
    pass::layout(&mut ui, &sty, &mut st, VIEW);
    assert!(ui.list.window_len() <= Repeat::<Row>::MAX_WINDOW, "and a small one is capped");
}

#[test]
fn a_non_finite_scroll_is_rejected() {
    // NaN and infinity are not representable in JSON, so they arrive as
    // an overflowing float literal instead.
    let _ = take_log();
    let mut r = Repeat::<Row>::new(10, 24.0, noop_bind);
    r.restore(&json!({ "count": 10, "scroll": 1e300, "item_extent": 24.0 }));
    assert!(r.scroll.is_finite(), "scroll stayed finite, got {}", r.scroll);
    assert!(take_log().iter().any(|l| l.contains("scroll")));
}

#[test]
fn ordinary_saved_state_is_unaffected() {
    let _ = take_log();
    let w = restore_and_lay_out(json!(1000), json!(24.0));
    assert_eq!(w, 26, "600px viewport / 24px rows, plus the partial one");
    assert!(take_log().is_empty(), "a good value logs nothing");
}
