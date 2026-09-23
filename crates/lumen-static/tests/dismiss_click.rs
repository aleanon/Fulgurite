//! **The click that dismisses an overlay is consumed whole** — found
//! while making `Select` controlled (M33/T33.3).
//!
//! `pass::dispatch_overlaid` consumes the *press* outside every open
//! overlay, and its comment says why: "the click that closes a menu
//! should not also press the button underneath it". But activation
//! happens on the *release*, and nothing required a release to end a
//! press that began on the same node. So the down was eaten, and the
//! up still activated whatever it landed on.

use lumen_static::prelude::*;
use lumen_static::style::{cascade_deep, Length, Patch, Rule, Selector, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{pass, Constraints, Event, EventCtx, NodeState};
use lumen_static::overlay::OverlaySink;
use lumen_static_widgets::ops::Button;
use lumen_static_widgets::select::{Row, Select};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 400.0 };

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Ui {
    #[widget_id("pick")]
    sel: Select<App>,
    #[widget_id("under")]
    btn: Button<App>,
}

#[derive(Default)]
struct App {
    ui: Ui,
    pressed: u32,
}

impl lumen_static::event::App for App {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self {
        self
    }
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

fn bind(r: &mut Row<App>, i: usize) {
    r.label = format!("option {i}");
}

fn sheet() -> Vec<Rule> {
    vec![
        Rule::new(Selector::Id("pick"), Patch::new().width(Length::Fixed(100.0))),
        Rule::new(Selector::Id("under"), Patch::new().width(Length::Fixed(100.0)).height(Length::Fixed(20.0))),
        Rule::new(Selector::Kind("repeat"), Patch::new().width(Length::Fixed(100.0)).height(Length::Fixed(80.0))),
    ]
}

#[test]
fn a_click_that_dismisses_a_menu_does_not_also_press_what_is_under_it() {
    let mut app = App::default();
    app.ui.sel = Select::new(3, 20.0, bind);
    app.ui.btn.on_click = |s: &mut App, _: &EventCtx| s.pressed += 1;
    let mut styles = vec![Style::default(); Ui::NODE_COUNT];
    cascade_deep(&mut app.ui, &sheet(), &mut styles);
    let mut st = vec![NodeState::default(); Ui::NODE_COUNT];
    let mut input = lumen_static::event::InputState::default();
    let mut ov: lumen_static::overlay::Overlays<8> = Default::default();
    let frame = |app: &mut App, st: &mut Vec<NodeState>, ov: &mut lumen_static::overlay::Overlays<8>| {
        let mut m = lumen_static::cx::BlockMetrics;
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(&mut app.ui, &mut cx, VIEW, &styles, st, 0);
        Node::place(&mut app.ui, lumen_static::Point { x: 0.0, y: 0.0 }, &styles, st, 0);
        let mut list = lumen_static::display::DisplayList::new();
        pass::paint_overlaid(&app.ui, st, &mut list, ov);
    };

    app.ui.sel.open();
    frame(&mut app, &mut st, &mut ov);
    assert!(app.ui.sel.is_open() && !ov.is_empty(), "setup: the menu is open and deferred");

    // The button, beside the select — outside the popup.
    let bi = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "under").unwrap();
    let (x, y) = (st[bi].pos.x + 10.0, st[bi].pos.y + 10.0);
    for ev in [Event::down(x, y), Event::up(x, y)] {
        pass::dispatch_overlaid(&mut app, &styles, &mut st, &mut input, &ov, &ev);
        frame(&mut app, &mut st, &mut ov);
    }
    println!("button at ({x}, {y}); menu open: {}; presses: {}", app.ui.sel.is_open(), app.pressed);
    assert!(!app.ui.sel.is_open(), "setup: the outside press did not dismiss the menu");
    assert_eq!(
        app.pressed, 0,
        "the click that dismissed the menu also pressed the button under it — the down was \
         consumed and the up activated anyway"
    );

    // The other direction: with nothing open, the same click presses it.
    for ev in [Event::down(x, y), Event::up(x, y)] {
        pass::dispatch_overlaid(&mut app, &styles, &mut st, &mut input, &ov, &ev);
        frame(&mut app, &mut st, &mut ov);
    }
    assert_eq!(app.pressed, 1, "an ordinary click on the button must still press it");
}
