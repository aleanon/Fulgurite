//! Characterization: virtualization alone does **not** keep children
//! inside their parent.
//!
//! `hit.rs`'s module comment claims children never escape their parents,
//! and `lumen-static-agent/tests/lint.rs` guards it — but that guard
//! walks `SUB_TABLE`, and a `Repeat`'s rows live in an off-table arena,
//! so the one mechanism that already breaks the invariant is invisible
//! to the test that exists to catch it.
//!
//! This test records the actual behaviour so the gap is visible, and is
//! expected to **fail when question A1 of
//! `docs/text-and-scrolling-questions.md` is answered**. That is its
//! job: whoever settles A1 must come here and re-pin it to the scoped
//! invariant rather than discovering this later.

use lumen_static::ops::{Fixed, Label};
use lumen_static::repeat::{noop_bind, Repeat};
use lumen_static::{pass, Cmd, Constraints, NodeState, StaticTree, Widget};

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

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 100.0 };

/// Lay out a 24px-per-row list scrolled to `scroll`, and report how far
/// the materialized window escapes the repeater's own rect.
fn escapes(scroll: f32) -> (f32, f32, usize) {
    let mut ui = Ui { list: Repeat::new(1000, 24.0, noop_bind) };
    ui.list.scroll = scroll;
    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    pass::layout(&mut ui, &sty, &mut st, VIEW);

    let me = st[1];
    let rows = ui.list.window_len();
    let first = ui.list.row_state(0).pos.y;
    let last = ui.list.row_state(rows - 1);

    let mut out = lumen_static::DisplayList::new();
    pass::paint(&ui, &st, &mut out);
    let clips = out.cmds.iter().filter(|c| matches!(c, Cmd::PushClip { .. })).count();

    (me.pos.y - first, (last.pos.y + last.size.h) - (me.pos.y + me.size.h), clips)
}

#[test]
fn a_fractionally_scrolled_repeat_paints_outside_its_parent() {
    // Scrolled off an item boundary: `place` puts row 0 at
    // `0 * 24 - 12 = -12`, above the viewport top (repeat.rs:175), and
    // `visible_range` deliberately materializes one extra row to cover
    // the partial one at the bottom (repeat.rs:87).
    let (above, below, clips) = escapes(12.0);
    assert_eq!(above, 12.0, "row 0 sits above the repeater by half an item");
    assert_eq!(below, 32.0, "the +1 materialized row hangs below it");
    assert_eq!(clips, 0, "and nothing clips it — it paints over its neighbours");
}

#[test]
fn even_on_an_item_boundary_the_extra_row_escapes() {
    // Virtualization needs the +1 row whenever the viewport is not an
    // exact multiple of the item extent, so this is not a fractional-
    // scroll edge case — it is the normal state of a scrolling list.
    let (above, below, clips) = escapes(0.0);
    assert_eq!(above, 0.0);
    // ceil(100/24) + 1 = 6 rows materialized = 144px of content in a
    // 100px viewport: the overhang is larger here than in the
    // fractional case above, and it is present on every frame.
    assert_eq!(below, 44.0, "100px viewport / 24px items never divides evenly");
    assert_eq!(clips, 0);
}

#[test]
fn hit_testing_already_behaves_as_if_clipped() {
    // The asymmetry that makes this a live defect rather than a cosmetic
    // one: `Repeat::hit` gates on the repeater's own rect before looking
    // at any row, so a click 6px above the list is correctly rejected —
    // while paint has already drawn a row there. Hit and paint disagree,
    // which is the class of bug P4 fixed for probe-vs-click.
    let mut ui = Ui { list: Repeat::new(1000, 24.0, noop_bind) };
    ui.list.scroll = 12.0;
    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    pass::layout(&mut ui, &sty, &mut st, VIEW);

    assert!(pass::hit_test(&ui, &st, 10.0, -6.0).is_none(), "no click above the list");
    assert!(pass::hit_test(&ui, &st, 10.0, 50.0).is_some(), "clicks inside still land");
}
