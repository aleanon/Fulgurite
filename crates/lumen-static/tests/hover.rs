//! M27/T27.11 finding 3: a pointer move reaches the node it is over.
//!
//! **The gap this closes.** A `PointerMove` was delivered to a widget
//! only under pointer capture, and capture is granted only to a node
//! that consumed a `PointerDown` — so any widget that paints *rows* and
//! wanted to light the one under the pointer could do it only while the
//! button was held. `Tree` and `OptionList` both shipped with that code
//! and neither could ever run it.
//!
//! **And why it is a second event.** Routing the move itself was tried
//! first: `Slider` reads a move as a drag, correctly, so a hover would
//! have dragged it — `slider.rs::a_move_with_nothing_captured_is_not_routed`
//! went red and said so. `Event::Hover` is the same gesture asked as a
//! different question, and a widget that does not match it is
//! unaffected.
//!
//! This dispatches through `pass::dispatch`, deliberately: the widget
//! half is tested in `lumen-static-widgets`, and a test that called
//! `on_event` with a `Hover` in hand would pass with the synthesis in
//! `dispatch` deleted. It was written that way first and that injection
//! left it green.

use lumen_static::event::{Event, EventCtx};
use lumen_static::widget::Node;
use lumen_static::{pass, Constraints, NodeState, Point, Size, StaticTree, Style, Widget};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 100.0, min_h: 0.0, max_h: 60.0 };

/// A leaf that records the hovers and the moves it is given.
#[derive(Default)]
struct Rows {
    hovers: Vec<f32>,
    moves: Vec<f32>,
}

impl Widget for Rows {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [lumen_static::desc::NodeDesc] =
        &[lumen_static::desc::NodeDesc::new(0, lumen_static::desc::NO_PARENT, 0, 0, 0)];
    const KIND: &'static str = "rows";
    const SEEDS: &'static [lumen_static::style::Seed] =
        &[lumen_static::style::Seed::unlinted("rows", <Rows as Widget>::type_defaults)];

    fn type_defaults() -> Style {
        Style { ..Style::default() }
    }
}

impl Node for Rows {
    fn layout(
        &mut self,
        _cx: &mut lumen_static::LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let s = Size { w: limits.max_w, h: limits.max_h };
        lumen_static::ops::record_painted(st, i, limits, s, &style[i])
    }

    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        match ev {
            Event::Hover { .. } => {
                self.hovers.push(cx.local.y);
                true
            }
            Event::PointerMove { .. } => {
                self.moves.push(cx.local.y);
                true
            }
            _ => false,
        }
    }

    fn paint(&self, _st: &[NodeState], _i: usize, _out: &mut dyn lumen_static::Painter) {}

    lumen_static::leaf_common!(Rows, 0);
}

impl<A> lumen_static::event::Handlers<A> for Rows {
    fn handler(&self, _sel: lumen_static::event::NodeSel) -> Option<fn(&mut A, &EventCtx)> {
        None
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("rows")]
    rows: Rows,
}

#[derive(Default)]
struct App {
    ui: Ui,
}

impl lumen_static::event::App for App {
    // M33: this app keeps handlers on the whole of itself.
    type State = Self;
    fn state_mut(&mut self) -> &mut Self::State {
        self
    }
    // M28/T28.2: this fixture defers no work. `Never` is
    // uninhabited, so nothing can be sent and `apply` cannot
    // be called.
    type Msg = lumen_static::event::Never;
    fn apply(_: &mut Self, m: Self::Msg) -> bool {
        match m {}
    }
    type Tree = Ui;
    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
}

fn fixture() -> (App, Vec<Style>, Vec<NodeState>, lumen_static::event::InputState) {
    let mut app = App::default();
    let styles = pass::default_styles(&app.ui);
    let mut st = vec![NodeState::default(); Ui::NODE_COUNT];
    pass::layout(&mut app.ui, &styles, &mut st, VIEW);
    (app, styles, st, Default::default())
}

#[test]
fn a_move_arrives_as_a_hover_on_the_node_under_it() {
    let (mut app, styles, mut st, mut input) = fixture();
    assert!(st[1].size.h > 0.0, "setup: the leaf has no box to hover");
    let ev = Event::PointerMove { x: 10.0, y: 30.0 };
    let took = pass::dispatch(&mut app, &styles, &mut st, &mut input, &ev);
    assert!(took, "the move was not routed to the node under the pointer");
    assert_eq!(
        app.ui.rows.hovers,
        vec![30.0],
        "the node received {:?} hovers and {:?} moves",
        app.ui.rows.hovers.len(),
        app.ui.rows.moves.len()
    );
    assert!(
        app.ui.rows.moves.is_empty(),
        "the raw move was delivered too; a widget that reads a move as a drag \
         would have followed a pointer nobody pressed"
    );
}

#[test]
fn a_hover_outside_every_node_reaches_nothing() {
    let (mut app, styles, mut st, mut input) = fixture();
    let ev = Event::PointerMove { x: 400.0, y: 400.0 };
    let took = pass::dispatch(&mut app, &styles, &mut st, &mut input, &ev);
    assert!(!took, "a move outside the tree was reported handled");
    assert!(app.ui.rows.hovers.is_empty(), "a hover reached a node the pointer was not over");
}

#[test]
fn the_local_point_is_the_node_s_own() {
    let (mut app, styles, mut st, mut input) = fixture();
    st[1].pos = Point { x: 0.0, y: 12.0 };
    let ev = Event::PointerMove { x: 10.0, y: 30.0 };
    pass::dispatch(&mut app, &styles, &mut st, &mut input, &ev);
    assert_eq!(
        app.ui.rows.hovers,
        vec![18.0],
        "the hover's local y is not measured from the node's own origin, so a \
         widget cannot turn it into a row"
    );
}

// ---- R3-F12: an event writes the nodes it changed ----------------------

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Twelve {
    f0: lumen_static_widgets::ops::Fixed,
    f1: lumen_static_widgets::ops::Fixed,
    f2: lumen_static_widgets::ops::Fixed,
    f3: lumen_static_widgets::ops::Fixed,
    f4: lumen_static_widgets::ops::Fixed,
    f5: lumen_static_widgets::ops::Fixed,
    f6: lumen_static_widgets::ops::Fixed,
    f7: lumen_static_widgets::ops::Fixed,
    f8: lumen_static_widgets::ops::Fixed,
    f9: lumen_static_widgets::ops::Fixed,
    f10: lumen_static_widgets::ops::Fixed,
    f11: lumen_static_widgets::ops::Fixed,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Block {
    c0: Twelve,
    c1: Twelve,
    c2: Twelve,
    c3: Twelve,
    c4: Twelve,
    c5: Twelve,
    c6: Twelve,
    c7: Twelve,
    c8: Twelve,
    c9: Twelve,
    c10: Twelve,
    c11: Twelve,
}

/// About 1,700 nodes: the size of table R3-F12 was measured on.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Big {
    b0: Block,
    b1: Block,
    b2: Block,
    b3: Block,
    b4: Block,
    b5: Block,
    b6: Block,
    b7: Block,
    b8: Block,
    b9: Block,
    b10: Block,
}

#[derive(lumen_static::App, Default)]
struct BigApp {
    #[tree]
    ui: Big,
}

/// **At most four `NodeState` writes per pointer move** — M44/T44.0
/// item 7, gating T44.6 (`docs/plan-input-semantics.md`).
///
/// A move changes at most two hover bits (the node left and the node
/// entered) and, at most, two focus bits; everything else it wrote was a
/// sweep of the whole table, twice. The count comes from
/// `InputState::state_writes`, because a sweep that rewrites a bit with
/// the value it already had is invisible to any before/after comparison.
#[test]
#[ignore = "red until T44.6: R3-F12 (every event sweeps the state array)"]
fn a_pointer_move_writes_at_most_four_states() {
    let mut app = BigApp::default();
    let styles = pass::default_styles(&app.ui);
    let mut st = vec![NodeState::default(); Big::NODE_COUNT];
    let big = Constraints { min_w: 0.0, max_w: 2000.0, min_h: 0.0, max_h: 4000.0 };
    for (k, s) in [&mut app.ui.b0.c0.f0, &mut app.ui.b0.c0.f1].into_iter().enumerate() {
        s.w = 10.0 + k as f32;
        s.h = 10.0;
    }
    pass::layout(&mut app.ui, &styles, &mut st, big);
    let mut input = lumen_static::event::InputState::default();
    // Warm: the pointer is already over the first leaf.
    pass::dispatch(&mut app, &styles, &mut st, &mut input, &Event::PointerMove { x: 2.0, y: 2.0 });
    assert!(input.hovered.is_some(), "setup: the warm-up move hit nothing");
    let before = input.state_writes.0;
    pass::dispatch(&mut app, &styles, &mut st, &mut input, &Event::PointerMove { x: 13.0, y: 2.0 });
    let writes = input.state_writes.0 - before;
    println!("one PointerMove on a {}-node table: {writes} state writes (bound 4); hovered {:?}", Big::NODE_COUNT, input.hovered);
    assert!(input.hovered.is_some(), "setup: the move hit nothing");
    assert!(writes <= 4, "a pointer move wrote {writes} states on a {}-node table", Big::NODE_COUNT);
}
