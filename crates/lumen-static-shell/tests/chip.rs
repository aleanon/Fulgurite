//! **A chip is a control: a click selects it, its × removes it** — M57/B2
//! (`docs/plan-widget-fixes.md`).
//!
//! `Chip`'s rustdoc advertised "a selected value, a removable token" while
//! the widget was `nohit` with no handlers (`docs/widget-audit.md`, Chip).
//! Through a `Host`, because the × is chosen across both passes: pass 1
//! records which part the gesture is on, and pass 2 asks for the handler.

use lumen_static::{App as DeriveApp, Event, EventCtx, StaticTree, Widget};
use lumen_static_shell::Host;
use lumen_static_widgets::Chip;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
struct Ui {
    #[widget_id("chip")]
    chip: Chip<&'static str, App>,
    #[widget_id("other")]
    other: Chip<&'static str, App>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
    clicks: u32,
    removes: u32,
}

fn click(a: &mut App, _: &EventCtx) {
    a.clicks += 1;
}
fn remove(a: &mut App, _: &EventCtx) {
    a.removes += 1;
}

fn host() -> Host<App> {
    let mut app = App::default();
    app.ui.chip = Chip::new("no_std");
    app.ui.chip.removable = true;
    app.ui.chip.on_click = Some(click);
    app.ui.chip.on_remove = Some(remove);
    app.ui.other = Chip::new("plain");
    let mut h = Host::new(app, 500, 200);
    h.pump();
    h
}

fn node(id: &str) -> usize {
    lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, id).unwrap()
}

fn press(h: &mut Host<App>, x: f32, y: f32, x2: f32) -> bool {
    h.post(Event::PointerDown { x, y, button: lumen_static::PointerButton::Left, clicks: 1 });
    h.pump();
    h.post(Event::PointerUp { x: x2, y, button: lumen_static::PointerButton::Left });
    h.pump()
}

#[test]
fn a_click_on_the_body_selects_and_on_the_x_removes() {
    let mut h = host();
    let s = h.states[node("chip")];
    let y = s.pos.y + s.size.h / 2.0;
    let body = s.pos.x + 14.0;
    let x = s.pos.x + s.size.w - 16.0;
    println!("chip at {:?} {:?}; body x {body}, × x {x}", s.pos, s.size);
    assert!(s.size.w > 40.0, "setup: the chip has no width");
    press(&mut h, body, y, body);
    assert_eq!((h.app.clicks, h.app.removes), (1, 0), "a click on the body did not run on_click alone");
    press(&mut h, x, y, x);
    assert_eq!((h.app.clicks, h.app.removes), (1, 1), "a click on the × did not run on_remove alone");
    // A press on the × that slides onto the body is a click, not a remove.
    press(&mut h, x, y, body);
    assert_eq!((h.app.clicks, h.app.removes), (2, 1), "a press that left the × still removed");
    // The keyboard activates the body.
    h.input.focus = Some(node("chip") as u32);
    h.post(Event::Key { key: lumen_static::event::Key::Enter, down: true });
    h.post(Event::Key { key: lumen_static::event::Key::Enter, down: false });
    h.pump();
    assert_eq!((h.app.clicks, h.app.removes), (3, 1), "Enter did not activate the chip");
}

/// A chip with no handler lets the click through, and is drawn without
/// a ×: it takes no room for an affordance it does not have.
#[test]
fn a_plain_chip_handles_nothing_and_has_no_x() {
    let mut h = host();
    let s = h.states[node("other")];
    let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    let handled = press(&mut h, x, y, x);
    assert!(!handled && h.app.clicks == 0, "a plain chip handled a click");
    let r = h.states[node("chip")];
    println!("removable {} px wide, plain {} px", r.size.w, s.size.w);
}

/// Selected is drawn: an outline in the text colour over a tint.
#[test]
fn a_selected_chip_is_drawn_selected() {
    let mut h = host();
    let strokes = |h: &Host<App>| h.list.cmds.iter().filter(|c| matches!(c, lumen_static::Cmd::StrokedRect { .. })).count();
    let before = strokes(&h);
    h.app.ui.chip.selected = true;
    h.invalidate_all();
    h.pump();
    println!("outlines: {before} unselected, {} selected", strokes(&h));
    assert_eq!((before, strokes(&h)), (0, 1), "selection is not drawn");
}
