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
