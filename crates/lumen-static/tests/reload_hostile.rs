//! Saved state is the one input the core does not write itself.
//!
//! Every other reload test round-trips `save()` into `restore()`, so the
//! oracle is the encoder and no value outside its range is ever decoded.
//! `item_extent` came back unchecked: a subnormal cleared the `<= 0.0`
//! guard and overflowed `visible_range`'s `as usize + 1` **inside the
//! core**, and `1e-6` with a large `count` asked for hundreds of
//! millions of instances from forty bytes of JSON. The three
//! `log_fallback` arms in `Repeat::restore` had never executed.
//! `docs/plan-review-fixes.md` F1.4.
//!
//! Hand-written JSON throughout — that is the point.

#![cfg(feature = "reload")]

use lumen_static::ops::{Fixed, Label};
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

/// Restore the given saved value, lay out a full frame, and report the
/// window size that survived.
fn restore_and_lay_out(count: serde_json::Value, extent: serde_json::Value) -> usize {
    let mut ui = Ui { list: Repeat::new(0, 24.0, noop_bind) };
    ui.list.restore(&json!({ "count": count, "scroll": 0.0, "item_extent": extent }));
    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    pass::layout(&mut ui, &sty, &mut st, VIEW);
    ui.list.window_len()
}

#[test]
fn a_subnormal_extent_does_not_overflow_the_window_calculation() {
    // 1e-40 is > 0.0, so it cleared the old guard; the division then
    // saturated the cast and `+ 1` overflowed. Now it is rejected at
    // restore, which under §7's per-field fallback means the field keeps
    // the *fresh* value — so the frame lays out normally rather than
    // degrading, and the rejection is visible in the log.
    let _ = take_log();
    let w = restore_and_lay_out(json!(1000), json!(1e-40));
    assert_eq!(w, 26, "the fresh 24px extent was kept, so the frame is ordinary");
    assert!(take_log().iter().any(|l| l.contains("item_extent")), "and it is logged");
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
fn a_tiny_extent_cannot_ask_for_a_hundred_million_rows() {
    let _ = take_log();
    let w = restore_and_lay_out(json!(u32::MAX), json!(1e-6));
    assert!(w <= Repeat::<Row>::MAX_WINDOW, "window {w} is capped");
    assert!(take_log().iter().any(|l| l.contains("item_extent")));
}

#[test]
fn every_out_of_domain_extent_is_rejected_and_logged() {
    for bad in [json!(0.0), json!(-1.0), json!(1e-40), json!(1e-7)] {
        let _ = take_log();
        let w = restore_and_lay_out(json!(1000), bad.clone());
        assert!(w <= Repeat::<Row>::MAX_WINDOW, "{bad}: window {w}");
        assert!(
            take_log().iter().any(|l| l.contains("item_extent")),
            "{bad}: rejection must be visible in the log"
        );
    }
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
fn a_huge_count_with_a_sane_extent_still_works_and_stays_bounded() {
    let w = restore_and_lay_out(json!(u32::MAX), json!(24.0));
    assert!(w > 0, "a big list still renders");
    assert!(w <= Repeat::<Row>::MAX_WINDOW);
}

#[test]
fn ordinary_saved_state_is_unaffected() {
    let _ = take_log();
    let w = restore_and_lay_out(json!(1000), json!(24.0));
    assert_eq!(w, 26, "600px viewport / 24px rows, plus the partial one");
    assert!(
        !take_log().iter().any(|l| l.contains("item_extent")),
        "a good value logs nothing"
    );
}
