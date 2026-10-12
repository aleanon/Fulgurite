//! **Pull-to-refresh, as a gesture** — M57/B40 (`docs/plan-widget-fixes.md`).
//!
//! The audit called it broken as a gesture (`docs/widget-audit.md`,
//! PullToRefresh): a wheel pull never sprang back, because release came
//! only on a `PointerUp` a wheel never sends; dragging with the pointer did
//! nothing, because there was no arm for it; and any downward wheel pulled,
//! scrolled content or not. Each claim, driven through a `Host`.

use fulgurite::{App as DeriveApp, Event, EventCtx, PointerButton, StaticTree, Widget};
use fulgurite_shell::Host;
use fulgurite_widgets::ops::{Button, Label};
use fulgurite_widgets::{PullToRefresh, ScrollView};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("pull")]
    pull: PullToRefresh<Label>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

fn host() -> Host<App> {
    let mut app = App::default();
    app.ui.pull = PullToRefresh::new(Label::new("pull me down"));
    let mut h: Host<App> = Host::new(app, 300, 300);
    h.set_stylesheet(&[fulgurite::style::Rule::new(
        fulgurite::style::Selector::Id("pull"),
        fulgurite::style::Patch::new()
            .width(fulgurite::style::Length::Fixed(240.0))
            .height(fulgurite::style::Length::Fixed(160.0)),
    )]);
    h.pump();
    h
}

fn centre(h: &Host<App>) -> (f32, f32) {
    let i = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "pull").unwrap();
    let s = h.states[i];
    (s.pos.x + 40.0, s.pos.y + 20.0)
}

#[test]
fn a_wheel_pull_springs_back_and_fires_when_the_wheel_stops() {
    let mut h = host();
    let (x, y) = centre(&h);
    for _ in 0..4 {
        h.post(Event::Scroll { x, y, dy: 60.0 });
        h.pump();
    }
    assert!(h.app.ui.pull.is_armed(), "setup: four wheel notches did not arm it (pull {})", h.app.ui.pull.pull());
    let deadline = h.next_deadline_ms();
    assert!(deadline.is_some(), "an armed wheel pull left the window nothing to wake for");
    h.tick(200.0);
    h.pump();
    println!("pulled {} after 200 ms idle; triggered {}", h.app.ui.pull.pull(), h.app.ui.pull.take_triggered());
    assert_eq!(h.app.ui.pull.pull(), 0.0, "the wheel pull did not spring back when the wheel went quiet");
}

#[test]
fn a_pointer_drag_pulls_and_its_release_fires() {
    let mut h = host();
    let (x, y) = centre(&h);
    h.post(Event::PointerDown { x, y, button: PointerButton::Left, clicks: 1 });
    h.pump();
    h.post(Event::PointerMove { x, y: y + 200.0 });
    h.pump();
    let pulled = h.app.ui.pull.pull();
    println!("dragged 200 px: pull {pulled}");
    assert!(h.app.ui.pull.is_armed(), "a 200 px drag did not pull it to armed (pull {pulled})");
    h.post(Event::PointerUp { x, y: y + 200.0, button: PointerButton::Left });
    h.pump();
    assert!(h.app.ui.pull.take_triggered(), "releasing an armed drag did not fire");
    assert_eq!(h.app.ui.pull.pull(), 0.0);
}

// ---- only at the top ---------------------------------------------------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Rows {
    a: Label,
    b: Label,
    c: Label,
    d: Label,
    e: Label,
    f: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct ListUi {
    #[widget_id("pull")]
    pull: PullToRefresh<ScrollView<Rows>>,
}

#[derive(DeriveApp, Default)]
struct ListApp {
    #[tree]
    ui: ListUi,
}

/// A drag on a list scrolled away from its top does not pull.
///
/// Not a wheel: scroll chaining already hands a wheel to the list first,
/// which takes it while it can scroll toward its top — so the top check
/// cannot fire for a wheel, and the first draft of this test (a wheel)
/// stayed green with the check deleted. A *drag* is taken by the pull
/// itself, and is where the check decides.
#[test]
fn a_scrolled_list_does_not_pull() {
    let mut app = ListApp::default();
    let mut rows = Rows::default();
    for l in [&mut rows.a, &mut rows.b, &mut rows.c, &mut rows.d, &mut rows.e, &mut rows.f] {
        l.set("a row of text");
    }
    app.ui.pull = PullToRefresh::new(ScrollView::new(rows));
    let mut h: Host<ListApp> = Host::new(app, 300, 300);
    h.set_stylesheet(&[fulgurite::style::Rule::new(
        fulgurite::style::Selector::Id("pull"),
        fulgurite::style::Patch::new()
            .width(fulgurite::style::Length::Fixed(200.0))
            .height(fulgurite::style::Length::Fixed(50.0)),
    )]);
    h.pump();
    h.app.ui.pull.content.scroll = 30.0;
    h.invalidate_all();
    h.pump();
    assert!(h.app.ui.pull.content.scroll > 0.0, "setup: the list is not scrolled (it fits its box)");
    let i = fulgurite::style::index_of_wid(<ListUi as Widget>::SUB_TABLE, "pull").unwrap();
    let s = h.states[i];
    let (x, y) = (s.pos.x + 20.0, s.pos.y + 10.0);
    h.post(Event::PointerDown { x, y, button: PointerButton::Left, clicks: 1 });
    h.pump();
    h.post(Event::PointerMove { x, y: y + 200.0 });
    h.pump();
    println!("list scrolled {}, dragged 200 px: pull {}", h.app.ui.pull.content.scroll, h.app.ui.pull.pull());
    assert_eq!(h.app.ui.pull.pull(), 0.0, "a drag on scrolled content pulled");
    h.post(Event::PointerUp { x, y: y + 200.0, button: PointerButton::Left });
    h.pump();
}

/// And a control inside keeps its click: the drag must not capture a
/// button's press.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct BtnUi {
    #[widget_id("pull")]
    pull: PullToRefresh<Button<BtnApp>>,
}

#[derive(DeriveApp, Default)]
struct BtnApp {
    #[tree]
    ui: BtnUi,
    n: u32,
}

#[test]
fn a_button_inside_keeps_its_click() {
    let mut app = BtnApp::default();
    app.ui.pull = PullToRefresh::new(Button::default());
    app.ui.pull.content.label = "Go";
    app.ui.pull.content.on_click = |s: &mut BtnApp, _: &EventCtx| s.n += 1;
    let mut h: Host<BtnApp> = Host::new(app, 300, 300);
    h.pump();
    let b = fulgurite::style::index_of_wid(<BtnUi as Widget>::SUB_TABLE, "pull").unwrap() + 1;
    let s = h.states[b];
    let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    h.post(Event::PointerDown { x, y, button: PointerButton::Left, clicks: 1 });
    h.post(Event::PointerUp { x, y, button: PointerButton::Left });
    h.pump();
    assert_eq!(h.app.n, 1, "the pull took the button's press");
}
