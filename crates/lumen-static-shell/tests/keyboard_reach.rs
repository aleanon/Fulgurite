//! **The keyboard reaches containers, and focus follows an overlay** —
//! M57/A12 (`docs/plan-widget-fixes.md`).
//!
//! `SplitPane`, `PaneGrid`, `Collapsible`, `Tree` and a picker's pad
//! handled keys in code that never ran: none of them was focusable, so
//! Tab walked past them (`docs/widget-audit.md` X8). And a modal that
//! opened left focus behind it, where a keyboard user could not reach
//! the dialog at all.

use lumen_static::event::Key;
use lumen_static::{App as DeriveApp, Event, StaticTree, Widget};
use lumen_static_shell::Host;
use lumen_static_widgets::ops::{Button, Label};
use lumen_static_widgets::{Collapsible, Modal, SplitPane};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Dialog {
    #[widget_id("confirm")]
    confirm: Button<App>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 12.0)]
struct Ui {
    #[widget_id("before")]
    before: Button<App>,
    #[widget_id("split")]
    split: SplitPane<Label, Label>,
    #[widget_id("acc")]
    acc: Collapsible<Label>,
    #[widget_id("modal")]
    modal: Modal<Dialog>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

fn index(wid: &str) -> u32 {
    lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).unwrap() as u32
}

fn host() -> Host<App> {
    let mut app = App::default();
    app.ui.before.label = "Before";
    app.ui.split = SplitPane::new(Label::new("left"), Label::new("right"));
    app.ui.acc = Collapsible::new("Section", Label::new("body"));
    app.ui.modal = Modal::new(Dialog::default());
    app.ui.modal.panel.confirm.label = "Confirm";
    let mut h = Host::new(app, 500, 400);
    h.set_stylesheet(&[lumen_static::style::Rule::new(
        lumen_static::style::Selector::Id("split"),
        lumen_static::style::Patch::new().width(lumen_static::style::Length::Fixed(300.0)).height(lumen_static::style::Length::Fixed(60.0)),
    )]);
    h.pump();
    h
}

fn key(h: &mut Host<App>, k: Key) -> bool {
    h.post(Event::Key { key: k, down: true });
    let handled = h.pump();
    h.post(Event::Key { key: k, down: false });
    h.pump();
    handled
}

fn tab_to(h: &mut Host<App>, wid: &str) {
    for _ in 0..8 {
        if h.input.focus == Some(index(wid)) {
            return;
        }
        key(h, Key::Tab);
    }
    panic!("Tab never reached #{wid}; focus is {:?}", h.input.focus);
}

#[test]
fn tab_reaches_a_split_pane_and_arrows_move_its_divider() {
    let mut h = host();
    tab_to(&mut h, "split");
    let before = h.app.ui.split.split();
    assert!(key(&mut h, Key::Right), "Right on the focused split pane was not handled");
    println!("split {before} -> {}", h.app.ui.split.split());
    assert!(h.app.ui.split.split() > before, "Right did not move the divider");
}

#[test]
fn tab_reaches_a_section_and_enter_toggles_it() {
    let mut h = host();
    tab_to(&mut h, "acc");
    let was = h.app.ui.acc.is_open();
    key(&mut h, Key::Enter);
    assert_ne!(h.app.ui.acc.is_open(), was, "Enter on the focused section did not toggle it");
}

#[test]
fn focus_moves_into_an_opened_modal_and_back_when_it_closes() {
    let mut h = host();
    tab_to(&mut h, "before");
    h.app.ui.modal.open();
    h.invalidate_all();
    h.pump(); // paints the veil: the overlay exists from here
    assert!(h.more_deferred(), "an overlay opened and the host asked for no frame to move focus");
    h.pump();
    println!("focus after open: {:?} (confirm is {})", h.input.focus, index("confirm"));
    assert_eq!(h.input.focus, Some(index("confirm")), "focus did not move into the dialog");

    h.app.ui.modal.close();
    h.invalidate_all();
    h.pump();
    h.pump();
    assert_eq!(h.input.focus, Some(index("before")), "focus did not come back from the closed dialog");
}
