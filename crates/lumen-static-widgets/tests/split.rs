//! M27/T27.8 — `SplitPane` and `Collapsible`.

use lumen_static::event::{Event, EventCtx, Handlers, Key, NodeSel};
use lumen_static::ops::{Button, Fixed};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, NodeState, Point};
use lumen_static_widgets::{Collapsible, SplitPane};

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
