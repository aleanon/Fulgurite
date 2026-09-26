//! **A control on a veil's panel activates** — M57/C6.
//!
//! `Veil::on_event` swallowed every event that reached it while open, and
//! an event on the panel's button *reaches* it: the button declines the
//! press in pass 1, the press bubbles, and the veil took it — which ended
//! pass 1 before activation ran. So a modal's Confirm never activated,
//! by pointer or by Enter. The drive said it did, because the same press
//! also asked to dismiss, and the showcase counted both on one number.

use lumen_static::event::Key;
use lumen_static::{App as DeriveApp, Event, EventCtx, PointerButton, StaticTree, Widget};
use lumen_static_shell::Host;
use lumen_static_widgets::ops::Button;
use lumen_static_widgets::Modal;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Dialog {
    #[widget_id("confirm")]
    confirm: Button<App>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("modal")]
    modal: Modal<Dialog>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
    confirmed: u32,
}

fn host() -> Host<App> {
    let mut app = App::default();
    app.ui.modal = Modal::new(Dialog::default());
    app.ui.modal.panel.confirm.label = "Confirm";
    app.ui.modal.panel.confirm.on_click = |s: &mut App, _: &EventCtx| s.confirmed += 1;
    app.ui.modal.open();
    let mut h = Host::new(app, 400, 300);
    h.pump();
    h.pump();
    h
}

#[test]
fn the_panels_button_activates_by_pointer_and_by_enter() {
    let mut h = host();
    let i = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "confirm").unwrap();
    let s = h.states[i];
    let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    h.post(Event::PointerDown { x, y, button: PointerButton::Left, clicks: 1 });
    h.post(Event::PointerUp { x, y, button: PointerButton::Left });
    h.pump();
    println!("after a click: confirmed {}, dismissal asked {}", h.app.confirmed, h.app.ui.modal.take_dismissed());
    assert_eq!(h.app.confirmed, 1, "a click on the panel's button did not activate it");

    h.input.focus = Some(i as u32);
    h.post(Event::Key { key: Key::Enter, down: true });
    h.post(Event::Key { key: Key::Enter, down: false });
    h.pump();
    assert_eq!(h.app.confirmed, 2, "Enter on the focused panel button did not activate it");
    assert!(!h.app.ui.modal.take_dismissed(), "a press on the panel asked to dismiss");
}
