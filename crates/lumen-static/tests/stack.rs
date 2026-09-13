//! M11/T11.6: the `stack` axis — children share the container's rect.
//!
//! The property everything else rests on is the first test: **two
//! siblings of a stack occupy the same box**. A `row` or a `column`
//! cannot produce that — they allocate space sequentially by measured
//! size — which is why this is a third container mode and not a style.
//!
//! The second property is z-order: a stack paints its children in field
//! order, so the last field is on top, and `hit` must therefore walk
//! them **backwards**. Every other container may walk forwards because
//! its children cannot overlap.

use lumen_static::ops::{Fixed, Label};
use lumen_static::{
    pass, Cmd, Constraints, DisplayList, Length, NodeState, Point, StaticTree, Style, Widget,
};

// ANCHOR: stack
#[derive(StaticTree, Default)]
#[widget(stack)]
struct Panel {
    #[widget_id("under")]
    under: Fixed,
    #[widget_id("over")]
    over: Fixed,
}
// ANCHOR_END: stack

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("stack")]
    stack: Panel,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 120.0 };

fn build(
    under: (f32, f32),
    over: (f32, f32),
) -> (Ui, Vec<Style>, [NodeState; Ui::NODE_COUNT]) {
    let mut ui = Ui::default();
    ui.stack.under.w = under.0;
    ui.stack.under.h = under.1;
    ui.stack.over.w = over.0;
    ui.stack.over.h = over.1;
    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    pass::layout(&mut ui, &sty, &mut st, VIEW);
    (ui, sty, st)
}

fn rect(st: &[NodeState], i: usize) -> [f32; 4] {
    lumen_static::ops::rect_of(&st[i])
}

/// The whole point of the mode.
#[test]
fn children_of_a_stack_share_one_rect() {
    let (_ui, _sty, st) = build((80.0, 40.0), (80.0, 40.0));
    // 0 = Ui, 1 = the stack, 2 = under, 3 = over.
    let (u, o) = (rect(&st, 2), rect(&st, 3));
    // Assert the setup happened before comparing: two zero-sized rects
    // are also equal, and that is the vacuous shape `AGENT.md` names.
    assert!(
        u[2] - u[0] > 1.0 && u[3] - u[1] > 1.0,
        "the children have no area: under = {u:?}"
    );
    assert_eq!(u, o, "a stack's children must occupy the same rect");
}

/// The container is the largest child on each axis, independently — not
/// the sum, and not the largest child's *box*.
///
/// `width` is set to `Shrink` first, because a container's default style
/// is `FILL` on width and `Shrink` on height — the same convention a row
/// and a column have, and shared deliberately. Left at the default this
/// would measure the viewport and say nothing about the stack.
#[test]
fn a_stack_sizes_to_the_largest_child_on_each_axis() {
    // Wider-but-shorter against narrower-but-taller, so a sum on either
    // axis, or taking one child's whole box, gives a different answer.
    let (mut ui, mut sty, mut st) = build((120.0, 20.0), (30.0, 90.0));
    sty[1].width = Length::Shrink;
    pass::layout(&mut ui, &sty, &mut st, VIEW);
    let s = st[1].size;
    assert_eq!((s.w, s.h), (120.0, 90.0), "stack size should be the per-axis max");
}

/// And the default is inherited from the container convention, not
/// invented here — pinned so a change to it is deliberate.
#[test]
fn a_stack_defaults_to_fill_width_like_every_other_container() {
    let (_ui, _sty, st) = build((120.0, 20.0), (30.0, 90.0));
    assert_eq!(st[1].size.w, 200.0, "default width is FILL, as for row and column");
    assert_eq!(st[1].size.h, 90.0, "default height is Shrink: the tallest child");
}

/// Paint order is field order, so the last field is on top.
#[test]
fn a_stack_paints_its_children_in_field_order() {
    let (ui, _sty, st) = build((80.0, 40.0), (40.0, 20.0));
    let mut list = DisplayList::new();
    pass::paint(&ui, &st, &mut list);
    let rects: Vec<[f32; 4]> = list
        .cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::Rect { rect, .. } | Cmd::RoundedRect { rect, .. } => Some(*rect),
            _ => None,
        })
        .collect();
    let under = rect(&st, 2);
    let over = rect(&st, 3);
    let iu = rects.iter().position(|r| *r == under);
    let io = rects.iter().position(|r| *r == over);
    assert!(iu.is_some() && io.is_some(), "both children must paint: {rects:?}");
    assert!(iu < io, "the later field must paint last, so it is on top");
}

/// …and `hit` must therefore answer with the topmost, which is the one
/// painted last.
#[test]
fn a_stack_hits_the_topmost_child_first() {
    // Both cover the same point. `over` is declared second, so it is
    // painted last and must win.
    let (ui, _sty, st) = build((80.0, 40.0), (80.0, 40.0));
    let h = pass::hit_test(&ui, &st, 10.0, 10.0).expect("the point is inside both children");
    assert_eq!(h.node, 3, "hit must return the topmost child (node 3), not node {}", h.node);
}

/// A `Fill` child stretches to the box the stack settled on, and a
/// `Shrink` one does not — the case the glass panel needs, where a
/// scroller fills and a bar sits on top of it.
#[test]
fn a_fill_child_stretches_and_a_shrink_child_does_not() {
    let (mut ui, mut sty, mut st) = build((60.0, 30.0), (40.0, 20.0));
    // `under` fills both axes; `over` keeps its own size.
    sty[2].width = Length::FILL;
    sty[2].height = Length::FILL;
    sty[1].height = Length::FILL;
    pass::layout(&mut ui, &sty, &mut st, VIEW);
    let u = rect(&st, 2);
    let o = rect(&st, 3);
    assert_eq!(
        (u[2] - u[0], u[3] - u[1]),
        (200.0, 120.0),
        "a Fill child should take the stack's whole box"
    );
    assert_eq!(
        (o[2] - o[0], o[3] - o[1]),
        (40.0, 20.0),
        "a Shrink child should keep its own size"
    );
    // And it is still a stack: they share an origin.
    assert_eq!((u[0], u[1]), (o[0], o[1]), "both children start at the same origin");
}

/// Alignment applies on **both** axes, because a stack has no cross axis.
#[test]
fn align_positions_a_child_on_both_axes() {
    let (mut ui, mut sty, mut st) = build((100.0, 60.0), (20.0, 10.0));
    sty[1].align = lumen_static::Align::Center;
    sty[1].width = Length::Shrink;
    pass::layout(&mut ui, &sty, &mut st, VIEW);
    let o = rect(&st, 3);
    // The stack is 100x60 (the larger child); the small one centres in
    // both — (100-20)/2 and (60-10)/2.
    assert_eq!((o[0], o[1]), (40.0, 25.0), "centred on both axes");
}

/// A stack inside a column still takes its place in the flow — it is
/// **not** out of flow, which is what distinguishes it from `Popup`.
#[test]
fn a_stack_is_in_flow() {
    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct Two {
        top: Panel,
        below: Fixed,
    }
    let mut ui = Two::default();
    ui.top.under.w = 50.0;
    ui.top.under.h = 30.0;
    ui.top.over.w = 20.0;
    ui.top.over.h = 10.0;
    ui.below.w = 50.0;
    ui.below.h = 25.0;
    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Two::NODE_COUNT];
    pass::layout(&mut ui, &sty, &mut st, VIEW);
    // The stack is 30 tall, so its sibling starts at y = 30.
    assert_eq!(st[1].size.h, 30.0, "the stack occupies its own height");
    let b = rect(&st, 4);
    assert_eq!(b[1], 30.0, "the next sibling follows the stack in flow, at y = {}", b[1]);
}

/// Label is used so the file's `use` is honest about what a stack can
/// hold; a text child measures like any other.
#[test]
fn a_stack_can_hold_a_measured_child() {
    #[derive(StaticTree, Default)]
    #[widget(stack)]
    struct Mixed {
        bg: Fixed,
        text: Label,
    }
    let mut ui = Mixed::default();
    ui.bg.w = 90.0;
    ui.bg.h = 40.0;
    ui.text.set("hi");
    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Mixed::NODE_COUNT];
    pass::layout(&mut ui, &sty, &mut st, VIEW);
    assert_eq!(st[0].pos, Point { x: 0.0, y: 0.0 });
    assert_eq!(st[1].pos, st[2].pos, "both children share the stack's origin");
    assert!(st[2].size.w > 0.0, "the label measured something");
}
