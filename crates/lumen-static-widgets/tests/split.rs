//! M27/T27.8 — `SplitPane` and `Collapsible`.

use lumen_static::event::{Event, EventCtx, Handlers, Key, NodeSel};
use lumen_static::ops::{Button, Fixed};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, NodeState, Point};
use lumen_static_widgets::{Collapsible, PaneGrid, SplitPane};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 300.0 };

#[derive(Default)]
struct App {
    clicks: u32,
}

fn bump(a: &mut App, _: &EventCtx) {
    a.clicks += 1;
}

fn lay<W: Widget + Node>(w: &mut W) -> Vec<NodeState> {
    let styles = vec![<W as Widget>::type_defaults(); <W as Widget>::NODE_COUNT];
    let mut st = vec![NodeState::default(); <W as Widget>::NODE_COUNT];
    let mut m = lumen_static::cx::BlockMetrics;
    {
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(w, &mut cx, VIEW, &styles, &mut st, 0);
    }
    Node::place(w, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);
    st
}

fn drag<W: Widget + Node>(w: &mut W, local_x: f32) -> bool {
    let ev = Event::PointerDown {
        x: 0.0,
        y: 0.0,
        button: lumen_static::PointerButton::Left,
        clicks: 1,
    };
    let mut cx = EventCtx::for_event(0, ev);
    cx.local = Point { x: local_x, y: 10.0 };
    w.on_event(&ev, &cx)
}

// ---- SplitPane -------------------------------------------------------

#[test]
fn the_two_panes_share_the_width_at_the_split() {
    let mut s: SplitPane<Fixed, Fixed> = SplitPane::default();
    let st = lay(&mut s);
    let (a, b) = (st[1].size.w, st[2].size.w);
    assert!(
        (a - b).abs() < 1.0,
        "a 50/50 split gave {a} and {b} — they should be equal within the divider"
    );
    assert!(
        (a + b + 6.0 - st[0].size.w).abs() < 0.01,
        "the panes and the divider ({a} + {b} + 6) do not fill {}",
        st[0].size.w
    );
}

/// **The divider cannot be dragged flush to an edge.** A pane with no
/// area cannot be clicked to get it back, so the control would be
/// one-way.
#[test]
fn the_divider_stops_short_of_both_edges() {
    let mut s: SplitPane<Fixed, Fixed> = SplitPane::default();
    lay(&mut s);
    drag(&mut s, -500.0);
    assert!(s.split() > 0.0, "the divider reached the left edge: {}", s.split());
    drag(&mut s, 5000.0);
    assert!(s.split() < 1.0, "the divider reached the right edge: {}", s.split());
    // And the stop is the same distance from each end.
    let at_left = {
        let mut t: SplitPane<Fixed, Fixed> = SplitPane::default();
        lay(&mut t);
        drag(&mut t, -500.0);
        t.split()
    };
    assert!(
        (at_left - (1.0 - s.split())).abs() < 0.001,
        "the stops are asymmetric: {at_left} from the left, {} from the right",
        1.0 - s.split()
    );
}

/// The divider's grab area is wider than the bar it draws — a 6 px
/// target is under every accessibility floor there is.
#[test]
fn the_divider_is_easier_to_grab_than_it_looks() {
    let mut s: SplitPane<Fixed, Fixed> = SplitPane::default();
    let st = lay(&mut s);
    let w = st[0].size.w;
    let bar = (w - 6.0) * 0.5;
    // 5 px to the left of the bar: outside the 6 px it paints, inside
    // the grab margin.
    let hit = Node::hit(&s, &st, 0, bar - 5.0, 10.0).expect("nothing there at all");
    assert_eq!(
        hit.node, 0,
        "a press 5 px left of the divider landed on node {} — the grab margin is \
         what makes a 6 px bar usable",
        hit.node
    );
}

#[test]
fn a_split_pane_routes_to_both_panes() {
    let mut s: SplitPane<Button<App, &'static str>, Button<App, &'static str>> =
        SplitPane::default();
    s.first.on_click = bump;
    s.second.on_click = bump;
    assert!(
        Handlers::<App>::handler(&s, NodeSel { i: 1, inner: None }).is_some(),
        "it does not route to the first pane"
    );
    assert!(
        Handlers::<App>::handler(&s, NodeSel { i: 2, inner: None }).is_some(),
        "it does not route to the second pane — the half an off-by-one loses"
    );
    assert!(
        Handlers::<App>::handler(&s, NodeSel { i: 0, inner: None }).is_none(),
        "it answered a handler for the divider"
    );
}

// ---- Collapsible -----------------------------------------------------

#[test]
fn a_collapsed_section_is_only_its_header() {
    let mut c: Collapsible<Fixed> = Collapsible::new("Details", Fixed { w: 100.0, h: 120.0 });
    let st = lay(&mut c);
    assert!(!c.is_open());
    assert_eq!(st[0].size.h, 40.0, "a collapsed section is {} tall, not its header", st[0].size.h);
    assert_eq!(st[1].size.h, 0.0, "the body still holds geometry while collapsed");
}

#[test]
fn opening_it_makes_room_for_the_body() {
    let mut c: Collapsible<Fixed> = Collapsible::new("Details", Fixed { w: 100.0, h: 120.0 });
    c.set_open(true);
    let st = lay(&mut c);
    assert!(
        st[0].size.h > 150.0,
        "an open section with a 120 px body is {} tall",
        st[0].size.h
    );
    assert!(st[1].pos.y >= 40.0, "the body overlaps the header: y={}", st[1].pos.y);
}

/// **Only a press on the header toggles.** A press anywhere would
/// collapse the section the moment anyone clicked what is inside it.
#[test]
fn a_press_in_the_body_does_not_collapse_the_section() {
    let mut c: Collapsible<Fixed> = Collapsible::new("Details", Fixed { w: 100.0, h: 120.0 });
    c.set_open(true);
    lay(&mut c);
    let ev = Event::PointerDown {
        x: 0.0,
        y: 0.0,
        button: lumen_static::PointerButton::Left,
        clicks: 1,
    };
    let mut cx = EventCtx::for_event(0, ev);
    cx.local = Point { x: 10.0, y: 80.0 };
    assert!(!c.on_event(&ev, &cx), "a press in the body was taken by the header");
    assert!(c.is_open(), "a press in the body collapsed the section");
    // And on the header it does toggle.
    cx.local = Point { x: 10.0, y: 10.0 };
    assert!(c.on_event(&ev, &cx));
    assert!(!c.is_open(), "a press on the header did not collapse it");
}

#[test]
fn enter_toggles_it_too() {
    let mut c: Collapsible<Fixed> = Collapsible::new("Details", Fixed { w: 10.0, h: 10.0 });
    let ev = Event::Key { key: Key::Enter, down: true };
    assert!(c.on_event(&ev, &EventCtx::for_event(0, ev)));
    assert!(c.is_open(), "Enter did not open it");
}

#[test]
fn a_collapsed_body_cannot_be_hit() {
    let mut c: Collapsible<Button<App, &'static str>> = Collapsible::default();
    c.title = "Details";
    c.body.on_click = bump;
    let st = lay(&mut c);
    // Anywhere in the section: the body is zeroed, so nothing but the
    // header can answer.
    let hit = Node::hit(&c, &st, 0, 10.0, 10.0).expect("the header did not answer");
    assert_eq!(hit.node, 0, "a collapsed body answered a hit at node {}", hit.node);
}

// ---- PaneGrid --------------------------------------------------------

/// Four panes, three dividers — and the dividers are *its*, not two
/// nested splits'. A `SplitPane<SplitPane<A, C>, SplitPane<B, D>>` has
/// the same four panes with its horizontals in separate subtrees, where
/// nothing can report them together.
#[test]
fn a_pane_grid_is_four_panes_and_its_own_node() {
    type G = PaneGrid<Fixed, Fixed, Fixed, Fixed>;
    assert_eq!(
        <G as Widget>::NODE_COUNT,
        1 + 4 * <Fixed as Widget>::NODE_COUNT,
        "itself and its four panes"
    );
    let g = G::default();
    assert_eq!(g.vertical(), 0.5);
    assert_eq!(g.horizontal(0), 0.5);
    assert_eq!(g.horizontal(1), 0.5);
}

fn grid() -> PaneGrid<Fixed, Fixed, Fixed, Fixed> {
    PaneGrid::new(Fixed::default(), Fixed::default(), Fixed::default(), Fixed::default())
}

fn lay_grid(g: &mut PaneGrid<Fixed, Fixed, Fixed, Fixed>) -> Vec<NodeState> {
    let styles = lumen_static::pass::default_styles(g);
    let n = <PaneGrid<Fixed, Fixed, Fixed, Fixed> as Widget>::NODE_COUNT;
    let mut st = vec![NodeState::default(); n];
    let limits = Constraints { min_w: 400.0, max_w: 400.0, min_h: 300.0, max_h: 300.0 };
    let mut m = lumen_static::cx::BlockMetrics;
    {
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(g, &mut cx, limits, &styles, &mut st, 0);
    }
    Node::place(g, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);
    st
}

/// **Each column's horizontal divider is its own**, which is the whole
/// reason this is not two nested splits. Dragging the left one must not
/// move the right column's panes.
#[test]
fn a_column_s_divider_moves_only_that_column() {
    let mut g = grid();
    let st = lay_grid(&mut g);
    // Panes are spliced in declaration order and `Fixed` is one node,
    // so 1 is top-left, 2 top-right, 3 bottom-left, 4 bottom-right.
    let (tr_h, br_y) = (st[2].size.h, st[4].pos.y);
    assert!(tr_h > 0.0, "setup: the top-right pane has no height");
    assert!(g.set_divider(1, 0.25), "the left divider did not move");
    let st = lay_grid(&mut g);
    assert!(
        st[1].size.h < st[2].size.h,
        "the left column did not shrink: {} vs {}",
        st[1].size.h,
        st[2].size.h
    );
    assert_eq!(st[2].size.h, tr_h, "the right column's top pane changed height");
    assert_eq!(st[4].pos.y, br_y, "the right column's bottom pane moved");
}

#[test]
fn the_vertical_divider_moves_both_columns() {
    let mut g = grid();
    let st = lay_grid(&mut g);
    let left_w = st[1].size.w;
    assert!(g.set_divider(0, 0.25), "the vertical divider did not move");
    let st = lay_grid(&mut g);
    assert!(st[1].size.w < left_w, "the left column did not narrow");
    assert!(st[2].size.w > st[1].size.w, "the right column did not widen");
    assert_eq!(st[3].size.w, st[1].size.w, "a column's two panes disagree on width");
}

/// The same stop `SplitPane` keeps: a pane dragged to zero area cannot
/// be clicked to get it back.
#[test]
fn no_divider_can_be_dragged_flush_to_an_edge() {
    let mut g = grid();
    for which in 0..3u8 {
        g.set_divider(which, -5.0);
        let low = if which == 0 { g.vertical() } else { g.horizontal((which - 1) as usize) };
        assert!(low > 0.0, "divider {which} reached {low}");
        g.set_divider(which, 5.0);
        let high = if which == 0 { g.vertical() } else { g.horizontal((which - 1) as usize) };
        assert!(high < 1.0, "divider {which} reached {high}");
    }
}

/// **The divider a drag started on is kept.** The vertical grab margin
/// crosses both horizontals where they meet it, so a drag that
/// re-decided on every move would jump between dividers mid-gesture.
#[test]
fn a_drag_keeps_the_divider_it_started_on() {
    let mut g = grid();
    lay_grid(&mut g);
    // Press on the *left* horizontal, well away from the vertical.
    let press = Event::PointerDown {
        x: 0.0,
        y: 0.0,
        button: lumen_static::PointerButton::Left,
        clicks: 1,
    };
    let mut cx = EventCtx::for_event(0, press);
    cx.local = Point { x: 40.0, y: 147.0 };
    assert!(Node::on_event(&mut g, &press, &cx), "the press on a divider was declined");
    let before_v = g.vertical();
    // Now move across the vertical divider's grab margin.
    let mv = Event::PointerMove { x: 0.0, y: 0.0 };
    let mut cx = EventCtx::for_event(0, mv);
    cx.local = Point { x: 200.0, y: 60.0 };
    Node::on_event(&mut g, &mv, &cx);
    assert_eq!(g.vertical(), before_v, "the drag jumped to the vertical divider");
    assert!(
        (g.horizontal(0) - 0.2).abs() < 0.02,
        "the left divider did not follow to y=60/300: it is at {}",
        g.horizontal(0)
    );
}
