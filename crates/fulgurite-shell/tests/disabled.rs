//! **A disabled control takes no input, and says so** — M57/A16
//! (`docs/plan-widget-fixes.md`).
//!
//! No widget had a disabled state at all (`docs/widget-audit.md` X12).
//! `Style::disabled` refuses hits, focus and activation — for the node and
//! for anything inside it — and fades what it paints.

use fulgurite::event::Key;
use fulgurite::style::{Patch, Rule, Selector};
use fulgurite::{App as DeriveApp, Event, EventCtx, StaticTree};
use fulgurite_shell::Host;
use fulgurite_widgets::ops::Button;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 20.0)]
struct Ui {
    #[widget_id("off")]
    off: Button<App>,
    #[widget_id("on")]
    on: Button<App>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
    off: u32,
    on: u32,
}

fn host(disabled: bool) -> Host<App> {
    let mut app = App::default();
    app.ui.off.label = "Off";
    app.ui.off.on_click = |s: &mut App, _: &EventCtx| s.off += 1;
    app.ui.on.label = "On";
    app.ui.on.on_click = |s: &mut App, _: &EventCtx| s.on += 1;
    let mut h = Host::new(app, 300, 100);
    h.set_stylesheet(&[Rule::new(Selector::Id("off"), Patch::new().disabled(disabled))]);
    h.pump();
    h
}

fn centre(h: &Host<App>, wid: &str) -> (f32, f32) {
    let i = fulgurite::style::index_of_wid(<Ui as fulgurite::Widget>::SUB_TABLE, wid).unwrap();
    let s = h.states[i];
    (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
}

fn click(h: &mut Host<App>, (x, y): (f32, f32)) -> bool {
    h.post(Event::PointerDown { x, y, button: fulgurite::PointerButton::Left, clicks: 1 });
    let a = h.pump();
    h.post(Event::PointerUp { x, y, button: fulgurite::PointerButton::Left });
    let b = h.pump();
    a || b
}

#[test]
fn a_disabled_button_is_not_pressed_not_focused_and_not_tabbed_to() {
    let mut h = host(true);
    let c = centre(&h, "off");
    let handled = click(&mut h, c);
    println!("click on the disabled button: handled={handled}, presses={}, focus={:?}", h.app.off, h.input.focus);
    assert!(!handled && h.app.off == 0, "the disabled button was pressed");
    assert_eq!(h.input.focus, None, "the press focused the disabled button");
    h.post(Event::Key { key: Key::Tab, down: true });
    h.pump();
    let on = fulgurite::style::index_of_wid(<Ui as fulgurite::Widget>::SUB_TABLE, "on").unwrap() as u32;
    assert_eq!(h.input.focus, Some(on), "Tab stopped on the disabled button");
}

/// The other direction: the same button, enabled, works — so the refusal
/// above is the flag and not a setup that could never land.
#[test]
fn the_same_button_enabled_is_pressed() {
    let mut h = host(false);
    let c = centre(&h, "off");
    assert!(click(&mut h, c), "the enabled button was not pressed");
    assert_eq!(h.app.off, 1);
}
