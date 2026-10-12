//! **A menu runs a command** — M57/B23 (`docs/plan-widget-fixes.md`).
//!
//! `Menu` was a `Chooser` kind that stored its pick as a selection: choosing
//! "Copy" made it the menu's value and ran nothing, its role was ComboBox,
//! and it had no keyboard (`docs/widget-audit.md`, Menu). A command is now
//! taken once with `take_chosen`, the menu keeps no value, and focus moves
//! into an open menu so the keyboard can choose.

use fulgurite::event::Key;
use fulgurite::{App as DeriveApp, Event, PointerButton, StaticTree, Widget};
use fulgurite_shell::Host;
use fulgurite_widgets::Menu;

const COMMANDS: &[&str] = &["Cut", "Copy", "Paste"];

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("menu")]
    menu: Menu,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

fn opened() -> Host<App> {
    let mut app = App::default();
    app.ui.menu = Menu::new(COMMANDS);
    app.ui.menu.open();
    let mut h: Host<App> = Host::new(app, 400, 400);
    h.pump();
    h.pump();
    h
}

fn list(h: &Host<App>) -> fulgurite::NodeState {
    let t = <Ui as Widget>::SUB_TABLE;
    h.states[t.iter().position(|d| d.wid == "chooser-list").unwrap()]
}

#[test]
fn a_click_runs_a_command_once_and_keeps_no_value() {
    let mut h = opened();
    let l = list(&h);
    let (x, y) = (l.pos.x + 30.0, l.pos.y + 6.0 + 30.0 + 15.0);
    h.post(Event::PointerDown { x, y, button: PointerButton::Left, clicks: 1 });
    h.post(Event::PointerUp { x, y, button: PointerButton::Left });
    h.pump();
    h.pump();
    let ran = h.app.ui.menu.take_chosen();
    println!("clicked the second row: ran {ran:?}, selected {:?}, open {}", h.app.ui.menu.selected(), h.app.ui.menu.is_open());
    assert_eq!(ran, Some(1), "the click on Copy ran nothing");
    assert_eq!(h.app.ui.menu.take_chosen(), None, "the same command ran twice");
    assert_eq!(h.app.ui.menu.selected(), None, "a menu kept its command as a value");
    assert!(!h.app.ui.menu.is_open(), "the menu stayed open after running a command");
}

#[test]
fn the_keyboard_chooses_a_command_in_an_open_menu() {
    let mut h = opened();
    let t = <Ui as Widget>::SUB_TABLE;
    let li = t.iter().position(|d| d.wid == "chooser-list").unwrap() as u32;
    assert_eq!(h.input.focus, Some(li), "focus did not move into the open menu");
    for k in [Key::Down, Key::Down, Key::Enter] {
        h.post(Event::Key { key: k, down: true });
        h.post(Event::Key { key: k, down: false });
        h.pump();
    }
    h.pump();
    let ran = h.app.ui.menu.take_chosen();
    println!("Down, Down, Enter: ran {ran:?}");
    assert_eq!(ran, Some(1), "Down, Down, Enter did not run the second command");
}
