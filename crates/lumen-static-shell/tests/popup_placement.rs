//! **A popup stays on the window** — M57/A18 (`docs/plan-widget-fixes.md`).
//!
//! `Popup::place` always went below its anchor and measured its content
//! against an infinite height, so a tooltip by the right edge ran off it,
//! a list near the bottom ran off the screen, and a long option list had
//! no way to be seen whole (`docs/widget-audit.md`, Tooltip, FilePicker).

use lumen_static::style::{Edges, Length, Patch, Rule, Selector};
use lumen_static::{App as DeriveApp, Event, StaticTree, Widget};
use lumen_static_shell::Host;
use lumen_static_widgets::ops::Button;
use lumen_static_widgets::{DatePicker, PickList, Tooltip};

const OPTS: &[&str] = &[
    "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve", "thirteen",
    "fourteen", "fifteen", "sixteen",
];

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("pick")]
    pick: PickList,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

fn part(owner_table: &[lumen_static::desc::NodeDesc], part: &str) -> usize {
    owner_table.iter().position(|d| d.wid == part).unwrap()
}

/// A picklist `top` px down a `w`×`h` window, opened.
fn opened(top: f32, w: u32, h: u32, options: &'static [&'static str]) -> Host<App> {
    let mut app = App::default();
    app.ui.pick = PickList::new(options);
    app.ui.pick.open();
    let mut host: Host<App> = Host::new(app, w, h);
    host.set_stylesheet(&[Rule::new(Selector::Kind("Ui"), Patch::new().padding(Edges { left: 20.0, right: 0.0, top, bottom: 0.0 }))]);
    host.pump();
    host.pump();
    host
}

#[test]
fn a_list_near_the_bottom_opens_above_its_field() {
    let host = opened(420.0, 400, 500, &OPTS[..5]);
    let t = <Ui as Widget>::SUB_TABLE;
    let (field, list) = (host.states[part(t, "chooser-trigger")], host.states[part(t, "chooser-list")]);
    println!("field y {}..{}, list y {}..{} in a 500 px window", field.pos.y, field.pos.y + field.size.h, list.pos.y, list.pos.y + list.size.h);
    assert!(list.size.h > 0.0, "setup: the list did not open");
    assert!(list.pos.y + list.size.h <= field.pos.y + 0.5, "the list did not flip above its field");
    // The other direction: with room below, it stays below.
    let host = opened(20.0, 400, 500, &OPTS[..5]);
    let (field, list) = (host.states[part(t, "chooser-trigger")], host.states[part(t, "chooser-list")]);
    assert!(list.pos.y >= field.pos.y + field.size.h - 0.5, "a list with room below went above");
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct PadUi {
    #[widget_id("date")]
    date: DatePicker,
}

#[derive(DeriveApp, Default)]
struct PadApp {
    #[tree]
    ui: PadUi,
}

/// **A pad opened above clears its trigger by the same gap as below**
/// (M57/D6). The picker hangs its pad a gap under the trigger and told
/// the popup to flip by the trigger's height alone, so a flipped pad
/// ended inside the trigger by exactly that gap — found at scale 2,
/// where the drive's window was short enough to flip it, as a stale row
/// of the trigger's text under the pad's edge.
#[test]
fn a_pad_near_the_bottom_clears_its_trigger() {
    let at = |top: f32| {
        let mut app = PadApp::default();
        app.ui.date.open();
        let mut host: Host<PadApp> = Host::new(app, 400, 500);
        host.set_stylesheet(&[Rule::new(Selector::Kind("PadUi"), Patch::new().padding(Edges { left: 20.0, right: 0.0, top, bottom: 0.0 }))]);
        host.pump();
        host.pump();
        let t = <PadUi as Widget>::SUB_TABLE;
        (host.states[part(t, "picker-trigger")], host.states[part(t, "picker-pad")])
    };
    let (field, pad) = at(420.0);
    let gap_above = field.pos.y - (pad.pos.y + pad.size.h);
    println!("flipped: field y {}..{}, pad y {}..{}; gap {gap_above} px (want 4)", field.pos.y, field.pos.y + field.size.h, pad.pos.y, pad.pos.y + pad.size.h);
    assert!(pad.size.h > 0.0, "setup: the pad did not open");
    assert!(pad.pos.y < field.pos.y, "setup: the pad did not flip above");
    let (field, pad) = at(20.0);
    let gap_below = pad.pos.y - (field.pos.y + field.size.h);
    println!("below: gap {gap_below} px");
    assert!((gap_below - 4.0).abs() < 0.01, "setup: the pad hangs {gap_below} px under its trigger");
    assert!((gap_above - gap_below).abs() < 0.01, "a flipped pad sits {gap_above} px from its trigger, {gap_below} px below it");
}

#[test]
fn a_list_taller_than_the_window_fits_and_scrolls() {
    let mut host = opened(10.0, 400, 300, OPTS);
    let t = <Ui as Widget>::SUB_TABLE;
    let list = host.states[part(t, "chooser-list")];
    println!("16 options: list {} px tall at y {} in a 300 px window", list.size.h, list.pos.y);
    assert!(list.pos.y >= 0.0 && list.pos.y + list.size.h <= 300.5, "the list ran off the window");
    let (x, y) = (list.pos.x + 20.0, list.pos.y + list.size.h / 2.0);
    host.post(Event::Scroll { x, y, dy: -60.0 });
    assert!(host.pump(), "a wheel over a list that does not fit was not taken");
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct TipUi {
    #[widget_id("tip")]
    tip: Tooltip<Button<TipApp>>,
}

#[derive(DeriveApp, Default)]
struct TipApp {
    #[tree]
    ui: TipUi,
}

#[test]
fn a_tooltip_by_the_right_edge_stays_inside() {
    let mut app = TipApp::default();
    app.ui.tip = Tooltip::new(Button::default(), "A long explanation of what this button does");
    app.ui.tip.content.label = "?";
    let mut host: Host<TipApp> = Host::new(app, 300, 200);
    host.set_stylesheet(&[Rule::new(
        Selector::Kind("TipUi"),
        Patch::new().padding(Edges { left: 270.0, right: 0.0, top: 20.0, bottom: 0.0 }).width(Length::FILL),
    )]);
    host.pump();
    let b = lumen_static::style::index_of_wid(<TipUi as Widget>::SUB_TABLE, "tip").unwrap() + 1;
    let s = host.states[b];
    host.post(Event::PointerMove { x: s.pos.x + 4.0, y: s.pos.y + 4.0 });
    host.pump();
    // The tip waits for the pointer to rest (M57/B35).
    host.tick(600.0);
    host.pump();
    host.pump();
    let tip = (0..host.states.len())
        .filter(|&k| k > b)
        .map(|k| host.states[k])
        .filter(|s| s.size.w > 40.0)
        .last()
        .expect("setup: no tip opened on hover");
    println!("tip x {}..{} in a 300 px window", tip.pos.x, tip.pos.x + tip.size.w);
    assert!(tip.pos.x + tip.size.w <= 300.5, "the tip ran off the right edge");
}
