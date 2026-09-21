//! The scoped containment invariant, pinned on the one construct that
//! exercises it (A1 of `docs/text-and-scrolling-decisions.md`).
//!
//! A repeater's window overruns its viewport in both directions, and
//! that is *required* by virtualization rather than incidental to it:
//! `place` puts the first row at `first * extent - scroll`, negative for
//! any scroll off an item boundary, and `visible_range` materializes one
//! row past the bottom because a viewport is rarely an exact multiple of
//! the item extent.
//!
//! So the invariant is not "children never escape their parents" — that
//! has been false since Step 3. It is: **content may extend beyond a
//! clipping ancestor and is clipped there; it may not extend beyond a
//! non-clipping one.** These tests pin the escape geometry (unchanged)
//! and the clip that makes it invisible and unhittable (new).
//!
//! It went unnoticed for so long because the guard in
//! `lumen-static-agent/tests/lint.rs` walks `SUB_TABLE`, and a
//! repeater's rows live in an off-table arena — the one mechanism that
//! breaks the invariant is invisible to the test written to catch it.

use lumen_static_widgets::ops::{Fixed, Label};
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
fn a_fractionally_scrolled_repeat_overruns_its_viewport_and_is_clipped() {
    // Scrolled off an item boundary: `place` puts row 0 at
    // `0 * 24 - 12 = -12`, above the viewport top (repeat.rs:175), and
    // `visible_range` materializes one extra row to cover the partial
    // one at the bottom (repeat.rs:87). The geometry is unchanged by the
    // clip — only what reaches the screen is.
    let (above, below, clips) = escapes(12.0);
    assert_eq!(above, 12.0, "row 0 sits above the repeater by half an item");
    assert_eq!(below, 32.0, "the +1 materialized row hangs below it");
    assert_eq!(clips, 1, "and the repeater clips its own window");
}

#[test]
fn even_on_an_item_boundary_the_extra_row_overruns() {
    // Virtualization needs the +1 row whenever the viewport is not an
    // exact multiple of the item extent, so this is not a fractional-
    // scroll edge case — it is the normal state of a scrolling list.
    let (above, below, clips) = escapes(0.0);
    assert_eq!(above, 0.0);
    // ceil(100/24) + 1 = 6 rows materialized = 144px of content in a
    // 100px viewport: the overhang is larger here than in the
    // fractional case above, and present on every frame.
    assert_eq!(below, 44.0, "100px viewport / 24px items never divides evenly");
    assert_eq!(clips, 1);
}

#[test]
fn hit_and_paint_agree_on_where_the_list_ends() {
    // `Repeat::hit` has always gated on the repeater's own rect before
    // looking at any row, so a click 6px above the list is rejected.
    // Before the clip, paint had already drawn a row there — visible but
    // not clickable, the class of bug P4 fixed for probe-vs-click. The
    // clip rect is now exactly the rect `hit` tests against, so the two
    // cannot diverge.
    let mut ui = Ui { list: Repeat::new(1000, 24.0, noop_bind) };
    ui.list.scroll = 12.0;
    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    pass::layout(&mut ui, &sty, &mut st, VIEW);

    assert!(pass::hit_test(&ui, &st, 10.0, -6.0).is_none(), "no click above the list");
    assert!(pass::hit_test(&ui, &st, 10.0, 50.0).is_some(), "clicks inside still land");

    // The clip rect and the rect `hit` gates on are the same rect.
    let mut out = lumen_static::DisplayList::new();
    pass::paint(&ui, &st, &mut out);
    let clip = out
        .cmds
        .iter()
        .find_map(|c| match c {
            Cmd::PushClip { rect } => Some(*rect),
            _ => None,
        })
        .expect("the repeater clips");
    assert_eq!(clip, lumen_static::ops::rect_of(&st[1]));
}

/// The agent and AccessKit must see the same list paint and hit do.
///
/// A repeater's window overruns its viewport by construction, and F1.6
/// clipped those rows from paint while `hit` had always rejected them —
/// but `observe` announced them anyway, with bounds where nothing draws
/// and no click can land. That is the probe-versus-click divergence one
/// surface over: an agent reading `ui.getTree` would see rows it could
/// not interact with (F4.7).
#[test]
#[cfg(feature = "observe")]
fn observe_announces_only_rows_that_are_actually_on_screen() {
    use lumen_static::widget::Node;

    let mut ui = Ui { list: Repeat::new(1000, 24.0, noop_bind) };
    ui.list.scroll = 12.0;
    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    pass::layout(&mut ui, &sty, &mut st, VIEW);

    let materialized = ui.list.window_len();
    assert!(materialized >= 6, "the window overruns: {materialized} rows for a 100px viewport");

    let mut root = accesskit::Node::new(accesskit::Role::GenericContainer);
    let mut extra: Vec<(accesskit::NodeId, accesskit::Node)> = Vec::new();
    ui.list.observe(&st, 1, 1, &mut root, &mut extra);

    let announced = root.children().len();
    assert!(
        announced < materialized,
        "announced {announced} of {materialized} materialized rows — the overrun must be excluded"
    );

    // Every announced row overlaps the list's own rect.
    let list_rect = lumen_static::ops::rect_of(&st[1]);
    for k in 0..materialized {
        let r = lumen_static::ops::rect_of(&ui.list.row_state(k));
        let on_screen = lumen_static::hit::rects_overlap(list_rect, r);
        if !on_screen {
            assert!(
                r[3] <= list_rect[1] || r[1] >= list_rect[3],
                "an excluded row must be wholly outside the viewport, got {r:?}"
            );
        }
    }
}
