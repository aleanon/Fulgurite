//! **A right press is a context request, and nothing else** — M90/T90.2
//! (`docs/plan-desktop-integration.md`, "A secondary button never reaches
//! the press machinery").
//!
//! Through `pass::dispatch_overlaid_into` with the overlay list the last
//! `paint_overlaid` produced, as a `Host` dispatches, so the outside-press
//! dismissal and the overlay hit test are the real ones and the
//! `Consumed` set can be read directly:
//!
//! - a right press outside an open `Select` list closes it, as a left
//!   press does (`consumed` holds the popup), and its release activates
//!   nothing beneath;
//! - a node that consumes `Event::ContextRequest` receives it **once**, at
//!   the pressed point, and a left click on it delivers none (the accept
//!   direction).

use fulgurite::event::{Event, EventCtx, PointerButton};
use fulgurite::overlay::OverlaySink;
use fulgurite::style::{cascade_deep, Length, Patch, Rule, Selector, Style};
use fulgurite::widget::{Node, Widget};
use fulgurite::{pass, Constraints, NodeState, Point, Size, StaticTree};
use fulgurite_widgets::ops::Button;
use fulgurite_widgets::select::{Row, Select};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 400.0 };

/// A leaf that records each `ContextRequest` it is handed — the point
/// on the event and `ctx.pos` — and every press, and consumes the
/// requests only.
#[derive(Default)]
struct Probe {
    requests: Vec<(f32, f32, Point)>,
    presses: u32,
}

impl Widget for Probe {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [fulgurite::desc::NodeDesc] =
        &[fulgurite::desc::NodeDesc::new(0, fulgurite::desc::NO_PARENT, 0, 0, 0)];
    const KIND: &'static str = "probe";
    const SEEDS: &'static [fulgurite::style::Seed] =
        &[fulgurite::style::Seed::unlinted("probe", <Probe as Widget>::type_defaults)];

    fn type_defaults() -> Style {
        Style::default()
    }
}

impl Node for Probe {
    fn layout(&mut self, _cx: &mut fulgurite::LayoutCx, limits: Constraints, style: &[Style], st: &mut [NodeState], i: usize) -> Size {
        fulgurite::ops::record_painted(st, i, limits, Size { w: 100.0, h: 40.0 }, &style[i])
    }

    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        match *ev {
            Event::ContextRequest { x, y } => {
                self.requests.push((x, y, cx.pos));
                true
            }
            Event::PointerDown { .. } => {
                self.presses += 1;
                false
            }
            _ => false,
        }
    }

    fn paint(&self, _st: &[NodeState], _i: usize, _out: &mut dyn fulgurite::Painter) {}

    fulgurite::leaf_common!(Probe, 0);
}

impl<A> fulgurite::event::Handlers<A> for Probe {
    fn handler(&self, _sel: fulgurite::event::NodeSel) -> Option<fn(&mut A, &EventCtx)> {
        None
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("pick")]
    sel: Select<App>,
    #[widget_id("under")]
    btn: Button<App>,
    #[widget_id("probe")]
    probe: Probe,
}

#[derive(Default)]
struct App {
    ui: Ui,
    pressed: u32,
    chosen: u32,
}

impl fulgurite::event::App for App {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self {
        self
    }
    type Msg = fulgurite::event::Never;
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

fn bind(r: &mut Row<App>, i: usize) {
    r.label = format!("option {i}");
    r.on_click = |s: &mut App, _: &EventCtx| s.chosen += 1;
}

fn sheet() -> Vec<Rule> {
    vec![
        Rule::new(Selector::Id("pick"), Patch::new().width(Length::Fixed(100.0))),
        Rule::new(Selector::Id("under"), Patch::new().width(Length::Fixed(300.0)).height(Length::Fixed(200.0))),
        Rule::new(Selector::Kind("repeat"), Patch::new().width(Length::Fixed(100.0)).height(Length::Fixed(60.0))),
    ]
}

struct Rig {
    app: App,
    styles: Vec<Style>,
    st: Vec<NodeState>,
    input: fulgurite::event::InputState,
    ov: fulgurite::overlay::Overlays<8>,
}

impl Rig {
    fn new() -> Self {
        let mut app = App::default();
        app.ui.sel = Select::new(3, 20.0, bind);
        app.ui.btn.on_click = |s: &mut App, _: &EventCtx| s.pressed += 1;
        let mut styles = vec![Style::default(); Ui::NODE_COUNT];
        cascade_deep(&mut app.ui, &sheet(), &mut styles);
        let mut r = Rig {
            app,
            styles,
            st: vec![NodeState::default(); Ui::NODE_COUNT],
            input: Default::default(),
            ov: Default::default(),
        };
        r.frame();
        r
    }

    fn frame(&mut self) {
        let mut m = fulgurite::cx::BlockMetrics;
        let mut cx = fulgurite::LayoutCx::new(&mut m);
        Node::layout(&mut self.app.ui, &mut cx, VIEW, &self.styles, &mut self.st, 0);
        Node::place(&mut self.app.ui, Point { x: 0.0, y: 0.0 }, &self.styles, &mut self.st, 0);
        let mut list = fulgurite::display::DisplayList::new();
        pass::paint_overlaid(&self.app.ui, &self.st, &mut list, &mut self.ov);
    }

    /// Dispatch one event and lay out a frame; `(handled, consumed)`.
    fn send(&mut self, ev: Event) -> (bool, Vec<u32>) {
        let mut consumed = pass::Consumed::default();
        let h = pass::dispatch_overlaid_into(&mut self.app, &self.styles, &mut self.st, &mut self.input, &self.ov, &ev, &mut consumed);
        self.frame();
        (h, consumed.nodes().to_vec())
    }

    fn center(&self, wid: &str) -> (f32, f32) {
        let i = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).unwrap();
        let s = &self.st[i];
        (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
    }
}

fn right_down(x: f32, y: f32) -> Event {
    Event::PointerDown { x, y, button: PointerButton::Right, clicks: 1 }
}

fn right_up(x: f32, y: f32) -> Event {
    Event::PointerUp { x, y, button: PointerButton::Right }
}

/// **A right press outside an open list closes it, and its release
/// activates nothing beneath.** The release lands on a `Button` whose
/// handler counts, so a release that activated would show.
#[test]
fn a_right_press_outside_an_open_list_closes_it_and_activates_nothing() {
    let mut r = Rig::new();
    r.app.ui.sel.open();
    r.frame();
    assert!(r.app.ui.sel.is_open() && !r.ov.is_empty(), "setup: the menu is open and deferred");
    let popup = r.ov.entry(r.ov.len() - 1).node;
    // Low in the button, below the list (the list ends at y = 20 + 60).
    let (x, _) = r.center("under");
    let y = r.st[fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "under").unwrap()].pos.y + 180.0;
    let (down, consumed) = r.send(right_down(x, y));
    let open_after_press = r.app.ui.sel.is_open();
    let (up, _) = r.send(right_up(x, y));
    println!(
        "right press at ({x}, {y}) with the list open (popup node {popup}): handled {down}, consumed {consumed:?}, \
         open {open_after_press}; its release handled {up}, button presses {}, rows chosen {}",
        r.app.pressed, r.app.chosen
    );
    assert!(!open_after_press && down, "a right press outside the open list did not close it (handled {down})");
    assert!(consumed.contains(&popup), "the dismissal is not in `consumed`: {consumed:?} lacks the popup {popup}");
    assert!(!up && r.app.pressed == 0, "the right release activated what is beneath: handled {up}, presses {}", r.app.pressed);

    // Accept direction: a left click on the same point, nothing open,
    // presses the button — so the point is on it.
    r.send(Event::down(x, y));
    r.send(Event::up(x, y));
    println!("a left click at the same point, nothing open: button presses {}", r.app.pressed);
    assert_eq!(r.app.pressed, 1, "setup: a left click at ({x}, {y}) did not press the button");
}

/// **A node that consumes `ContextRequest` receives it once, at the
/// pressed point**; a left click on it delivers none.
#[test]
fn a_right_press_delivers_one_context_request_at_the_point() {
    let mut r = Rig::new();
    let (x, y) = r.center("probe");
    let (down, consumed) = r.send(right_down(x, y));
    let (up, _) = r.send(right_up(x, y));
    let probe = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "probe").unwrap() as u32;
    let got = r.app.ui.probe.requests.clone();
    println!(
        "right press+release at ({x}, {y}): press handled {down} (consumed {consumed:?}, probe node {probe}), \
         release handled {up}; requests {got:?}; presses seen {}",
        r.app.ui.probe.presses
    );
    assert!(down && consumed.contains(&probe), "the probe did not consume the request: handled {down}, consumed {consumed:?}");
    assert_eq!(got.len(), 1, "want one ContextRequest, got {got:?}");
    let (ex, ey, pos) = got[0];
    assert!((ex, ey) == (x, y) && pos == Point { x, y }, "the request is not at the pressed point ({x}, {y}): {got:?}");
    assert!(!up, "the right release was handled");
    assert_eq!(r.app.ui.probe.presses, 0, "a right press reached the probe as a press");

    // Accept direction: a left click reaches the probe as a press and
    // asks for no menu.
    r.send(Event::down(x, y));
    r.send(Event::up(x, y));
    println!("a left click: requests {}, presses {}", r.app.ui.probe.requests.len(), r.app.ui.probe.presses);
    assert_eq!((r.app.ui.probe.requests.len(), r.app.ui.probe.presses), (1, 1), "a left click asked for a context menu, or did not press");
}
