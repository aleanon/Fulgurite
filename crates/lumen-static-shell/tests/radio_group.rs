//! **Arrows walk a radio group and move its selection** — M57/B13
//! (`docs/plan-widget-fixes.md`).
//!
//! The arrow group was the focused node's parent, so a radio beside its
//! words — in a `Labelled`, or in the row the showcase used before it —
//! was its parent's only focusable, and Down did nothing; and where the
//! arrows did move focus between radios, the choice stayed behind
//! (`docs/widget-audit.md`, Radio). Through a `Host`, because both are
//! the pass's focus rules.

use lumen_static::event::{Key, Mods};
use lumen_static::{App as DeriveApp, Event, EventCtx, StaticTree, Widget};
use lumen_static_shell::Host;
use lumen_static_widgets::{CheckBox, Labelled, Radio};

#[derive(StaticTree, Default)]
#[widget(row, spacing = 20.0)]
struct Radios {
    #[widget_id("r0")]
    a: Labelled<Radio<App>>,
    #[widget_id("r1")]
    b: Labelled<Radio<App>>,
    #[widget_id("r2")]
    c: Labelled<Radio<App>>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 20.0)]
struct Boxes {
    #[widget_id("c0")]
    a: Labelled<CheckBox<App>>,
    #[widget_id("c1")]
    b: Labelled<CheckBox<App>>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 20.0)]
struct Ui {
    radios: Radios,
    boxes: Boxes,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
    choice: usize,
    ticks: u32,
}

fn c0(a: &mut App, _: &EventCtx) {
    a.choice = 0;
}
fn c1(a: &mut App, _: &EventCtx) {
    a.choice = 1;
}
fn c2(a: &mut App, _: &EventCtx) {
    a.choice = 2;
}
fn tick(a: &mut App, _: &EventCtx) {
    a.ticks += 1;
}

fn radio(f: fn(&mut App, &EventCtx)) -> Labelled<Radio<App>> {
    Labelled::new(Radio::<App> { on_toggle: f, ..Default::default() }, "option")
}

fn host() -> Host<App> {
    let mut app = App::default();
    app.ui.radios = Radios { a: radio(c0), b: radio(c1), c: radio(c2) };
    app.ui.boxes.a = Labelled::new(CheckBox::<App> { on_toggle: tick, ..Default::default() }, "one");
    app.ui.boxes.b = Labelled::new(CheckBox::<App> { on_toggle: tick, ..Default::default() }, "two");
    let mut h = Host::new(app, 600, 200);
    h.pump();
    h
}

/// The control inside the `Labelled` named `id`.
fn control(id: &str) -> u32 {
    let table = <Ui as Widget>::SUB_TABLE;
    let w = lumen_static::style::index_of_wid(table, id).unwrap();
    (w + 1) as u32
}

fn down(h: &mut Host<App>, k: Key) {
    h.post(Event::Key { key: k, down: true });
    h.post(Event::Key { key: k, down: false });
    h.pump();
}

#[test]
fn down_moves_a_radio_groups_focus_and_its_selection() {
    let mut h = host();
    h.input.focus = Some(control("r0"));
    h.pump();
    down(&mut h, Key::Down);
    println!("after Down: focus {:?} (r1 is {}), choice {}", h.input.focus, control("r1"), h.app.choice);
    assert_eq!(h.input.focus, Some(control("r1")), "Down did not move focus to the next radio");
    assert_eq!(h.app.choice, 1, "focus moved and the selection stayed behind");
    down(&mut h, Key::Up);
    down(&mut h, Key::Up);
    println!("after Up, Up: focus {:?}, choice {}", h.input.focus, h.app.choice);
    assert_eq!((h.input.focus, h.app.choice), (Some(control("r2")), 2), "Up did not wrap to the last radio");
    let _ = Mods::NONE;
}

/// Checkboxes are a group for focus, but an arrow never ticks one.
#[test]
fn an_arrow_moves_between_checkboxes_without_toggling() {
    let mut h = host();
    h.input.focus = Some(control("c0"));
    h.pump();
    down(&mut h, Key::Right);
    println!("focus {:?} (c1 is {}), ticks {}", h.input.focus, control("c1"), h.app.ticks);
    assert_eq!(h.input.focus, Some(control("c1")), "Right did not reach the next checkbox");
    assert_eq!(h.app.ticks, 0, "an arrow toggled a checkbox");
}
