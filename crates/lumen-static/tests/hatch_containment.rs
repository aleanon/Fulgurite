//! The scoped containment invariant on the three off-table containers
//! that share `Repeat`'s shape: children in an arena the owner places,
//! `hit` gated on the owner's rect, and — before this — `paint` emitting
//! no clip. `docs/text-and-scrolling-decisions.md`, A1 fallout.
//!
//! Each is driven to the same state: a `Fixed` height smaller than its
//! content, so `resolve_self` shrinks the node below the item sum. Then
//! `hit` and `paint` are asked where the container ends.

use lumen_static::assemble::Assembled;
use lumen_static::hatch::{Dyn, List};
use lumen_static::ops::Fixed;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{pass, Cmd, Constraints, NodeState, StaticTree, Widget};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 400.0 };

/// A 20px-tall block.
fn block() -> Fixed {
    Fixed { w: 40.0, h: 20.0 }
}

/// Style node 1 (the container under test) to 50px tall, then lay out.
fn lay<T: Widget>(ui: &mut T, states: &mut [NodeState], id: &'static str) {
    let mut styles = pass::default_styles(ui);
    let sheet = [Rule::new(Selector::Id(id), Patch::new().height(Length::Fixed(50.0)))];
    lumen_static::style::cascade(
        T::SUB_TABLE,
        T::SEEDS,
        &sheet,
        &mut styles,
    );
    pass::layout(ui, &styles, states, VIEW);
}

fn clip_of<T: Widget>(ui: &T, st: &[NodeState]) -> Option<[f32; 4]> {
    let mut out = lumen_static::DisplayList::new();
    pass::paint(ui, st, &mut out);
    out.cmds.iter().find_map(|c| match c {
        Cmd::PushClip { rect } => Some(*rect),
        _ => None,
    })
}

// ---- List -------------------------------------------------------------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct WithList {
    #[widget_id("list")]
    list: List<Fixed>,
}

#[test]
fn a_list_taller_than_its_box_clips_where_hit_stops() {
    let mut ui = WithList { list: List::new((0..10).map(|_| block()).collect()) };
    let mut st = [NodeState::default(); WithList::NODE_COUNT];
    lay(&mut ui, &mut st, "list");

    assert_eq!(st[1].size.h, 50.0, "resolve_self honoured the Fixed height");
    // Ten 20px items were laid out inside it: 200px of content in 50px.
    assert!(pass::hit_test(&ui, &st, 5.0, 45.0).is_some(), "inside the box");
    assert!(pass::hit_test(&ui, &st, 5.0, 150.0).is_none(), "item 7 exists but is unreachable");
    assert_eq!(clip_of(&ui, &st), Some(lumen_static::ops::rect_of(&st[1])), "paint clips to the same rect");
}

// ---- Dyn --------------------------------------------------------------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Tall {
    a: Fixed,
    b: Fixed,
    c: Fixed,
    d: Fixed,
    e: Fixed,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct WithDyn {
    #[widget_id("dyn")]
    island: Dyn<()>,
}

#[test]
fn a_dyn_island_taller_than_its_box_clips_where_hit_stops() {
    let tall = Tall { a: block(), b: block(), c: block(), d: block(), e: block() };
    let mut ui = WithDyn { island: Dyn::new(tall) };
    let mut st = [NodeState::default(); WithDyn::NODE_COUNT];
    lay(&mut ui, &mut st, "dyn");

    assert_eq!(st[1].size.h, 50.0);
    assert!(pass::hit_test(&ui, &st, 5.0, 45.0).is_some());
    assert!(pass::hit_test(&ui, &st, 5.0, 90.0).is_none(), "block e exists but is unreachable");
    assert_eq!(clip_of(&ui, &st), Some(lumen_static::ops::rect_of(&st[1])));
}

// ---- Assembled --------------------------------------------------------

#[test]
fn an_assembled_column_taller_than_its_limits_clips_where_hit_stops() {
    // Ten 20px cards in a 50px-tall viewport.
    let mut a = Assembled::<Fixed>::new(10);
    for c in a.instances.iter_mut() {
        *c = block();
    }
    a.layout(Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 50.0 });

    assert!(a.hit(5.0, 45.0).is_some());
    assert!(a.hit(5.0, 150.0).is_none());
    let mut out = lumen_static::DisplayList::new();
    a.paint(&mut out);
    let clip = out.cmds.iter().find_map(|c| match c {
        Cmd::PushClip { rect } => Some(*rect),
        _ => None,
    });
    assert!(clip.is_some(), "assembled paint clips its column");
}
