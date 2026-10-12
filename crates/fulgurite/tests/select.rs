//! M24/T24.2 — the dropdown as a widget.
//!
//! Four claims, each of which the showcase's hand-wired picker had to
//! get right by hand and one of which it got wrong:
//!
//! 1. the option count is data — the tree's shape does not move with it;
//! 2. a press on the trigger opens it — through the application's
//!    state since M33/T33.3, when `Select` became controlled;
//! 3. a press on the trigger *while open* closes it and does **not**
//!    reopen it — the classic dropdown bug;
//! 4. a press on a row reaches the application with the right index;
//! 5. a keyboard does the same (M44/T44.10): opened with Enter, the menu
//!    takes focus, Down moves its active row, Enter chooses it, and focus
//!    comes back to the trigger — and Tab while it is open closes it
//!    (M44/T44.9).

use fulgurite::display::{Cmd, DisplayList};
use fulgurite::prelude::*;
use fulgurite_widgets::select::{Row, Select};
use fulgurite::style::{cascade_deep, Length, Patch, Rule, Selector, Style};
use fulgurite::widget::{Node, Widget};
use fulgurite::{pass, Constraints, Event, EventCtx, NodeState};

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
    /// **The application owns `open`** (M33/T33.3); `update` binds it
    /// into the popup, exactly as `#[bind(ui.sel.menu.open, open)]`
    /// would.
    open: bool,
    dismissed: u32,
}

impl fulgurite::event::App for App {
    // M33: this app keeps handlers on the whole of itself.
    type State = Self;
    fn state_mut(&mut self) -> &mut Self::State {
        self
    }
    // M28/T28.2: this fixture defers no work. `Never` is
    // uninhabited, so nothing can be sent and `apply` cannot
    // be called.
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
    fn update(&mut self) -> bool {
        fulgurite::bind::BindValue::bind_value(&mut self.ui.sel.menu.open, &self.open)
    }
}

fn bind(r: &mut Row<App>, i: usize) {
    r.label = format!("option {i}");
    r.on_click = |s: &mut App, cx: &EventCtx| {
        s.picked.push(cx.row.expect("a row index arrives from hit testing"));
        s.open = false;
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
    input: fulgurite::event::InputState,
    ov: fulgurite::overlay::Overlays<8>,
}

impl Fx {
    fn new(count: usize) -> Fx {
        let mut app = App::default();
        app.ui.sel = Select::new(count, ROW_H, bind);
        app.ui.sel.trigger.on_click = |s: &mut App, _: &EventCtx| s.open = true;
        app.ui.sel.on_dismiss = |s: &mut App, _: &EventCtx| {
            s.open = false;
            s.dismissed += 1;
        };
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
        let mut m = fulgurite::cx::BlockMetrics;
        let mut cx = fulgurite::LayoutCx::new(&mut m);
        Node::layout(&mut self.app.ui, &mut cx, VIEW, &self.styles, &mut self.st, 0);
        Node::place(
            &mut self.app.ui,
            fulgurite::Point { x: 0.0, y: 0.0 },
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
        // The binding step, between input and layout — what `Host::pump`
        // runs as `App::update`.
        fulgurite::event::App::update(&mut self.app);
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
    f.send(Event::Key { key: fulgurite::Key::Escape, down: true });
    assert!(!f.app.ui.sel.is_open(), "Escape did not dismiss the menu");
}


/// **A dismissal reaches the application** — M33/T33.3.
///
/// The pass closes the popup on a press outside; the application's `open`
/// must follow, or the next binding reopens it. The assertion is on the
/// *next frame*, after `update` has bound the state back in.
#[test]
fn a_press_outside_tells_the_application_and_the_menu_stays_closed() {
    let mut f = Fx::new(5);
    f.click(20.0, ROW_H / 2.0);
    assert!(f.app.open && f.app.ui.sel.is_open(), "setup: the menu never opened");

    f.click(280.0, 390.0);
    f.click(280.0, 390.0); // a second frame, after the binding ran
    println!("open: {}, dismissed: {}", f.app.ui.sel.is_open(), f.app.dismissed);
    assert_eq!(f.app.dismissed, 1, "the pass did not tell the Select's owner it was dismissed");
    assert!(
        !f.app.ui.sel.is_open(),
        "the menu reopened: the application still said open, and the binding wrote it back"
    );
}

/// **A keyboard user can choose a row** — M44/T44.0 item 13, R3-F6
/// (`docs/plan-input-semantics.md` §1.6).
///
/// Tab to the trigger, Enter opens the menu, Down twice moves the active
/// row onto row 1 (the first Down lands on row 0 when nothing is chosen),
/// and Enter chooses it. Each key's `handled` is printed, because a Down
/// that nobody took is the failure this names.
#[test]
fn tab_enter_down_down_enter_chooses_row_one() {
    use fulgurite::Key;
    let mut f = Fx::new(5);
    // A whole press, down then up, as a keyboard sends one: since M57/A10
    // a second down with no up between is an auto-repeat, which does not
    // activate — so an Enter sent as downs alone chose nothing.
    let key = |f: &mut Fx, k: Key| {
        let handled = f.send(Event::Key { key: k, down: true });
        f.send(Event::Key { key: k, down: false });
        handled
    };
    key(&mut f, Key::Tab);
    let trigger = f.input.focus;
    assert!(trigger.is_some(), "setup: Tab focused nothing");
    let opened = key(&mut f, Key::Enter);
    assert!(opened && f.app.ui.sel.is_open(), "setup: Enter on the trigger did not open the menu");
    let handled = [key(&mut f, Key::Down), key(&mut f, Key::Down), key(&mut f, Key::Enter)];
    println!(
        "Down, Down, Enter handled {handled:?}; picked {:?} (expected [1]); open {} (expected false)",
        f.app.picked,
        f.app.ui.sel.is_open()
    );
    assert_eq!(f.app.picked, vec![1], "the keyboard did not choose row 1");
    assert!(!f.app.ui.sel.is_open(), "choosing a row did not close the menu");
    // And focus comes back to the trigger, which is where the keyboard
    // user was (T44.9's return; the menu took focus when it opened).
    f.send(Event::Modifiers(fulgurite::Mods::NONE));
    println!("after choosing: focus {:?} (trigger {trigger:?})", f.input.focus);
    assert_eq!(f.input.focus, trigger, "focus did not come back to the trigger");
}

/// Tab while the menu is open closes it and moves on, rather than
/// setting focus to `None` (R3-F6 with R3-F7).
///
/// **Not "keeps focus inside the menu"**, which is what T44.0 item 13
/// first said: that contradicts §2.3's decision that Tab dismisses a
/// non-modal, non-passive overlay (a menu) through the path Escape takes.
/// The plan's "What building found" records the correction. The
/// dismissal must reach the application (`on_dismiss`), or the next
/// binding reopens the controlled `Select`.
#[test]
fn tab_while_the_menu_is_open_closes_it_and_keeps_focus() {
    use fulgurite::Key;
    let mut f = Fx::new(5);
    f.send(Event::Key { key: Key::Tab, down: true });
    let trigger = f.input.focus;
    f.send(Event::Key { key: Key::Enter, down: true });
    assert!(f.app.ui.sel.is_open(), "setup: Enter on the trigger did not open the menu");
    let handled = f.send(Event::Key { key: Key::Tab, down: true });
    f.send(Event::Modifiers(fulgurite::Mods::NONE)); // a second frame, after the binding ran
    println!(
        "Tab with the menu open: handled {handled}, focus {:?} (trigger {trigger:?}; expected Some), open {} (expected false), dismissed {} (expected 1)",
        f.input.focus,
        f.app.ui.sel.is_open(),
        f.app.dismissed
    );
    assert!(f.input.focus.is_some(), "Tab with the menu open set focus to None");
    assert_eq!(f.app.dismissed, 1, "the Tab's dismissal did not reach the application");
    assert!(!f.app.ui.sel.is_open(), "the menu is still open after Tab");
}
