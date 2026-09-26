//! **A tooltip waits for the pointer to rest, shows for keyboard focus,
//! and closes on a press** — M57/B35 (`docs/plan-widget-fixes.md`).
//!
//! It opened on the first hover frame — so a pointer crossing a toolbar
//! strobed every tip — never for keyboard focus, and neither a press nor
//! Escape could close it: the next layout re-opened it, though its own
//! comment said a press closed it (`docs/widget-audit.md`, Tooltip).

use lumen_static::event::Key;
use lumen_static::{App as DeriveApp, Event, PointerButton, StaticTree, Widget};
use lumen_static_shell::Host;
use lumen_static_widgets::ops::Button;
use lumen_static_widgets::Tooltip;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("tip")]
    tip: Tooltip<Button<App>>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

fn host() -> Host<App> {
    let mut app = App::default();
    app.ui.tip = Tooltip::new(Button::default(), "Explains the button");
    app.ui.tip.content.label = "Go";
    let mut h: Host<App> = Host::new(app, 400, 300);
    h.set_stylesheet(&[lumen_static::style::Rule::new(
        lumen_static::style::Selector::Kind("Ui"),
        lumen_static::style::Patch::new().padding(lumen_static::style::Edges::all(40.0)),
    )]);
    h.pump();
    h
}

fn over_button(h: &mut Host<App>) -> (f32, f32) {
    let b = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "tip").unwrap() + 1;
    let s = h.states[b];
    let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    h.post(Event::PointerMove { x, y });
    h.pump();
    (x, y)
}

#[test]
fn it_waits_for_the_pointer_to_rest() {
    let mut h = host();
    over_button(&mut h);
    assert!(!h.app.ui.tip.tip.open, "the tip opened on the first hover frame");
    h.tick(200.0);
    h.pump();
    assert!(!h.app.ui.tip.tip.open, "the tip opened before the pointer had rested");
    h.tick(400.0);
    h.pump();
    h.pump();
    println!("after 600 ms at rest: open {}", h.app.ui.tip.tip.open);
    assert!(h.app.ui.tip.tip.open, "the tip did not open once the pointer rested");
}

#[test]
fn a_press_closes_it_and_it_stays_closed_while_hovered() {
    let mut h = host();
    let (x, y) = over_button(&mut h);
    h.tick(600.0);
    h.pump();
    h.pump();
    assert!(h.app.ui.tip.tip.open, "setup: the tip did not open");
    h.post(Event::PointerDown { x, y, button: PointerButton::Left, clicks: 1 });
    h.pump();
    h.post(Event::PointerUp { x, y, button: PointerButton::Left });
    h.pump();
    h.pump();
    println!("after a press, still hovered: open {}", h.app.ui.tip.tip.open);
    assert!(!h.app.ui.tip.tip.open, "a press did not close the tip");
}

#[test]
fn keyboard_focus_shows_it_and_escape_closes_it() {
    let mut h = host();
    h.post(Event::Key { key: Key::Tab, down: true });
    h.post(Event::Key { key: Key::Tab, down: false });
    h.pump();
    h.pump();
    assert!(h.app.ui.tip.tip.open, "keyboard focus on the control did not show its tip");
    h.post(Event::Key { key: Key::Escape, down: true });
    h.post(Event::Key { key: Key::Escape, down: false });
    h.pump();
    h.pump();
    h.pump();
    println!("after Escape: open {}", h.app.ui.tip.tip.open);
    assert!(!h.app.ui.tip.tip.open, "Escape did not close the tip");
}
