//! M24/T24.2 — the dropdown as a widget.
//!
//! Four claims, each of which the showcase's hand-wired picker had to
//! get right by hand and one of which it got wrong:
//!
//! 1. the option count is data — the tree's shape does not move with it;
//! 2. a press on the trigger opens it;
//! 3. a press on the trigger *while open* closes it and does **not**
//!    reopen it — the classic dropdown bug;
//! 4. a press on a row reaches the application with the right index.

use lumen_static::display::{Cmd, DisplayList};
use lumen_static::prelude::*;
use lumen_static::select::{Row, Select};
use lumen_static::style::{cascade_deep, Length, Patch, Rule, Selector, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{pass, Constraints, Event, EventCtx, NodeState};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 300.0, min_h: 0.0, max_h: 400.0 };
const ROW_H: f32 = 20.0;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("pick")]
    sel: Select<App>,
}

#[derive(Default)]
struct App {
    ui: Ui,
    picked: Vec<u64>,
}

impl lumen_static::event::App for App {
    // M28/T28.2: this fixture defers no work. `Never` is
    // uninhabited, so nothing can be sent and `apply` cannot
    // be called.
    type Msg = lumen_static::event::Never;
    fn apply(&mut self, m: Self::Msg) -> bool {
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
    r.on_click = |s: &mut App, cx: &EventCtx| {
        s.picked.push(cx.row.expect("a row index arrives from hit testing"));
        s.ui.sel.close();
    };
}

fn sheet() -> Vec<Rule> {
    vec![
        Rule::new(Selector::Id("pick"), Patch::new().width(Length::Fixed(160.0))),
        Rule::new(Selector::Kind("repeat"), Patch::new().width(Length::Fixed(160.0)).height(Length::Fixed(200.0))),
        Rule::new(Selector::Kind("button"), Patch::new().height(Length::Fixed(ROW_H))),
    ]
}

struct Fx {
    app: App,
    styles: Vec<Style>,
    st: Vec<NodeState>,
    input: lumen_static::event::InputState,
    ov: lumen_static::overlay::Overlays<8>,
}

impl Fx {
    fn new(count: usize) -> Fx {
        let mut app = App::default();
        app.ui.sel = Select::new(count, ROW_H, bind);
        let mut styles = vec![Style::default(); Ui::NODE_COUNT];
        cascade_deep(&mut app.ui, &sheet(), &mut styles);
        let mut f = Fx {
            app,
            styles,
            st: vec![NodeState::default(); Ui::NODE_COUNT],
            input: Default::default(),
            ov: Default::default(),
        };
        f.frame();
        f
    }

    fn frame(&mut self) -> DisplayList {
        let mut m = lumen_static::cx::BlockMetrics;
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(&mut self.app.ui, &mut cx, VIEW, &self.styles, &mut self.st, 0);
        Node::place(
            &mut self.app.ui,
            lumen_static::Point { x: 0.0, y: 0.0 },
            &self.styles,
            &mut self.st,
            0,
        );
        let mut list = DisplayList::new();
        pass::paint_overlaid(&self.app.ui, &self.st, &mut list, &mut self.ov);
        list
    }

    /// A full press and release at `(x, y)` — activation happens on the
    /// release, so a test that only pressed would never fire a handler.
    fn send(&mut self, ev: Event) -> bool {
        let handled = pass::dispatch_overlaid(
            &mut self.app,
            &self.styles,
            &mut self.st,
            &mut self.input,
            &self.ov,
            &ev,
        );
        self.frame();
        handled
    }

    /// A full press and release — activation happens on the release, so
    /// a test that only pressed would never fire a handler.
    fn click(&mut self, x: f32, y: f32) {
        self.send(Event::down(x, y));
        self.send(Event::up(x, y));
    }

    fn row_texts(&mut self) -> Vec<String> {
        let list = self.frame();
        list.cmds
            .iter()
            .filter_map(|c| match c {
                Cmd::Text { text, .. } => Some(list.str_of(*text).to_string()),
                _ => None,
            })
            .collect()
    }
}

/// The count is data. Three menus of wildly different lengths are the
/// same four nodes — which is the whole reason this is a `Repeat` and
/// not eighteen fields.
#[test]
fn the_option_count_does_not_change_the_tree() {
    assert_eq!(<Select<App> as Widget>::NODE_COUNT, 4);
    assert_eq!(<Ui as Widget>::NODE_COUNT, 1 + 4);
    for n in [0usize, 3, 18, 5000] {
        let f = Fx::new(n);
        assert_eq!(f.app.ui.sel.count(), n);
        assert_eq!(<Ui as Widget>::SUB_TABLE.len(), 5, "{n} options moved the table");
    }
}

/// Closed, the menu paints nothing and takes no space — the property
/// `Popup` exists for, and the one a form depends on.
#[test]
fn a_closed_menu_is_absent() {
    let mut f = Fx::new(18);
    assert!(!f.app.ui.sel.is_open());
    let texts = f.row_texts();
    assert!(
        !texts.iter().any(|t| t.starts_with("option ")),
        "a closed menu painted rows: {texts:?}"
    );
    // And it did not stretch the control: the select is the trigger.
    let h = f.st[1].size.h;
    assert!(h <= ROW_H + 0.01, "a closed Select is {h} px tall, not a trigger's {ROW_H}");
}

#[test]
fn a_press_on_the_trigger_opens_it() {
    let mut f = Fx::new(18);
    f.click(20.0, ROW_H / 2.0);
    assert!(f.app.ui.sel.is_open(), "the trigger did not open the menu");
    let texts = f.row_texts();
    let rows = texts.iter().filter(|t| t.starts_with("option ")).count();
    assert!(rows >= 5, "only {rows} rows painted: {texts:?}");
}

/// **The classic dropdown bug.** A press outside every overlay is
/// dismissed and consumed by the pass, so the trigger must never see it
/// — and `Select::on_event` writes `open = true` rather than `!open` so
/// that even if it did, it could not reopen what was just closed.
#[test]
fn pressing_the_trigger_again_closes_it_and_leaves_it_closed() {
    let mut f = Fx::new(18);
    f.click(20.0, ROW_H / 2.0);
    assert!(f.app.ui.sel.is_open(), "setup: the menu never opened");
    f.click(20.0, ROW_H / 2.0);
    assert!(
        !f.app.ui.sel.is_open(),
        "the trigger closed the menu and reopened it in the same click"
    );
}

/// One handler for every row; `ctx.row` says which.
#[test]
fn a_press_on_a_row_reaches_the_application_with_its_index() {
    let mut f = Fx::new(18);
    f.click(20.0, ROW_H / 2.0);
    assert!(f.app.ui.sel.is_open(), "setup: the menu never opened");

    // Row 3 sits three rows below the trigger.
    let y = ROW_H + ROW_H * 3.0 + ROW_H / 2.0;
    f.click(20.0, y);
    assert_eq!(f.app.picked, vec![3], "clicking row 3 at y={y} picked {:?}", f.app.picked);
    assert!(!f.app.ui.sel.is_open(), "the handler's close() did not take");
}

/// Escape closes it — `pass::dispatch_overlaid`'s, reached through
/// `Select::dismiss` forwarding to the popup.
#[test]
fn escape_closes_it() {
    let mut f = Fx::new(18);
    f.click(20.0, ROW_H / 2.0);
    assert!(f.app.ui.sel.is_open());
    f.send(Event::Key { key: lumen_static::Key::Escape, down: true });
    assert!(!f.app.ui.sel.is_open(), "Escape did not dismiss the menu");
}

