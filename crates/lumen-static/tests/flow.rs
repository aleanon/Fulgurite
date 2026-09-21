//! M27/T27.8 — `#[widget(wrap)]` and `#[widget(grid, columns = N)]`.
//!
//! A fourth and fifth container mode, and **one lowering, not two**: a
//! grid is a wrap whose line break is a *count* instead of a width, and
//! everything else — measuring loosely, accumulating a line, carrying
//! the tallest child down — is identical. Two lowerings would have been
//! two places for the cross-axis arithmetic to disagree.
//!
//! The property that matters and is easy to get wrong: **`place` must
//! produce the same lines `layout` did.** `place` has no `LayoutCx` and
//! re-runs the break test against the sizes already in `NodeState`; if
//! the two ever disagreed, children would be drawn outside the box their
//! container reported.

use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;
use lumen_static::widget::Node;
use lumen_static::{Constraints, NodeState, Point};

/// A fixed box, so every size in these tests is exact rather than
/// font-dependent.
fn cell(w: f32, h: f32) -> Fixed {
    Fixed { w, h }
}

#[derive(StaticTree, Default)]
#[widget(wrap, spacing = 10.0)]
struct Wrapped {
    #[widget_id("a")]
    a: Fixed,
    #[widget_id("b")]
    b: Fixed,
    #[widget_id("c")]
    c: Fixed,
    #[widget_id("d")]
    d: Fixed,
}

#[derive(StaticTree, Default)]
#[widget(grid, columns = 2, spacing = 10.0)]
struct Grid2 {
    #[widget_id("a")]
    a: Fixed,
    #[widget_id("b")]
    b: Fixed,
    #[widget_id("c")]
    c: Fixed,
    #[widget_id("d")]
    d: Fixed,
}

fn lay<W: Widget + Node + Default>(w: &mut W, max_w: f32) -> Vec<NodeState> {
    let styles = lumen_static::pass::default_styles(w);
    let mut st = vec![NodeState::default(); <W as Widget>::NODE_COUNT];
    let view = Constraints { min_w: 0.0, max_w, min_h: 0.0, max_h: 400.0 };
    let mut m = lumen_static::cx::BlockMetrics;
    {
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(w, &mut cx, view, &styles, &mut st, 0);
    }
    Node::place(w, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);
    st
}

/// Four 60 px cells with 10 px gaps in a 200 px box: 60,70,70 fits three
/// (60 + 10 + 60 + 10 + 60 = 200), so the fourth wraps.
#[test]
fn a_wrap_breaks_the_line_when_the_next_child_would_not_fit() {
    let mut w = Wrapped {
        a: cell(60.0, 20.0),
        b: cell(60.0, 20.0),
        c: cell(60.0, 20.0),
        d: cell(60.0, 20.0),
    };
    let st = lay(&mut w, 200.0);
    let y: Vec<f32> = (1..=4).map(|k| st[k].pos.y).collect();
    assert_eq!(
        (y[0], y[1], y[2]), (0.0, 0.0, 0.0),
        "the first three should share a line: {y:?}"
    );
    assert_eq!(y[3], 30.0, "the fourth should be on the next line (20 tall + 10 gap): {y:?}");
    let x: Vec<f32> = (1..=4).map(|k| st[k].pos.x).collect();
    assert_eq!(x, vec![0.0, 70.0, 140.0, 0.0], "x positions are {x:?}");
    // And the container reports a box that holds them.
    assert_eq!(st[0].size.h, 50.0, "two lines of 20 with a 10 gap is 50, got {}", st[0].size.h);
}

/// **The container's reported size contains its children.** This is the
/// property a `place` that disagreed with `layout` would break, and it
/// cannot be checked by looking at either one alone.
#[test]
fn every_child_lands_inside_the_box_the_container_reported() {
    let mut w = Wrapped {
        a: cell(90.0, 20.0),
        b: cell(90.0, 30.0),
        c: cell(90.0, 20.0),
        d: cell(90.0, 20.0),
    };
    let st = lay(&mut w, 200.0);
    let box_ = lumen_static::ops::rect_of(&st[0]);
    for (k, child) in st.iter().enumerate().take(5).skip(1) {
        let r = lumen_static::ops::rect_of(child);
        assert!(
            r[0] >= box_[0] - 0.01 && r[1] >= box_[1] - 0.01
                && r[2] <= box_[2] + 0.01 && r[3] <= box_[3] + 0.01,
            "child {k} at {r:?} escapes its container {box_:?}"
        );
    }
    // The tallest child on a line sets that line's height: 30, not 20.
    assert_eq!(st[3].pos.y, 40.0, "the second line starts at {} — 30 + 10", st[3].pos.y);
}

/// A grid breaks on the **count**, whatever the width allows.
#[test]
fn a_grid_breaks_every_n_children_however_wide_the_box() {
    let mut g = Grid2 {
        a: cell(20.0, 20.0),
        b: cell(20.0, 20.0),
        c: cell(20.0, 20.0),
        d: cell(20.0, 20.0),
    };
    // 2000 px wide: a wrap would put all four on one line.
    let st = lay(&mut g, 2000.0);
    let y: Vec<f32> = (1..=4).map(|k| st[k].pos.y).collect();
    assert_eq!(
        y, vec![0.0, 0.0, 30.0, 30.0],
        "a 2-column grid in a 2000 px box put its children at {y:?} — \
         a wrap would have read [0, 0, 0, 0]"
    );
}

/// And the same grid in a *narrow* box keeps its columns rather than
/// wrapping early — the other direction, which a width-based break
/// would fail.
#[test]
fn a_grid_keeps_its_columns_in_a_narrow_box() {
    let mut g = Grid2 {
        a: cell(60.0, 20.0),
        b: cell(60.0, 20.0),
        c: cell(60.0, 20.0),
        d: cell(60.0, 20.0),
    };
    let st = lay(&mut g, 80.0);
    let y: Vec<f32> = (1..=4).map(|k| st[k].pos.y).collect();
    assert_eq!(
        y, vec![0.0, 0.0, 30.0, 30.0],
        "the grid wrapped on width instead of on its column count: {y:?}"
    );
}

/// A hit reaches the child under the point, including one on a wrapped
/// line — which a walk that stopped at the first line would miss.
#[test]
fn a_hit_finds_a_child_on_the_second_line() {
    let mut w = Wrapped {
        a: cell(60.0, 20.0),
        b: cell(60.0, 20.0),
        c: cell(60.0, 20.0),
        d: cell(60.0, 20.0),
    };
    let st = lay(&mut w, 200.0);
    let hit = Node::hit(&w, &st, 0, 10.0, 35.0).expect("nothing under the second line");
    assert_eq!(hit.node, 4, "the point landed on node {}, not the fourth child", hit.node);
}
