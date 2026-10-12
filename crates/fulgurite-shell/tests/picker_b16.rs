//! **A picker closes when its trigger says so, a clock takes the minute,
//! and a long file list scrolls** — M57/B16–B19
//! (`docs/plan-widget-fixes.md`).
//!
//! A press on the pad's own padding or weekday strip closed the popup
//! (the pad declined it and it bubbled to `Picker::on_event`, which
//! flipped `open` for any release); picking the hour closed the clock; a
//! long file list was a popup taller than the window; Space did nothing
//! (`docs/widget-audit.md`, the pickers).

use fulgurite::event::Key;
use fulgurite::{App as DeriveApp, Event, StaticTree, Widget};
use fulgurite_shell::Host;
use fulgurite_widgets::{DatePicker, FilePicker, TimePicker};

#[derive(StaticTree, Default)]
#[widget(row, spacing = 200.0)]
struct Ui {
    #[widget_id("date")]
    date: DatePicker,
    #[widget_id("time")]
    time: TimePicker,
    #[widget_id("file")]
    file: FilePicker,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

const NAMES: [&str; 20] = [
    "f00", "f01", "f02", "f03", "f04", "f05", "f06", "f07", "f08", "f09", "f10", "f11", "f12", "f13",
    "f14", "f15", "f16", "f17", "f18", "f19",
];

fn host() -> Host<App> {
    let mut app = App::default();
    app.ui.file = FilePicker::new().with_files(&NAMES);
    let mut h = Host::new(app, 1200, 900);
    h.pump();
    h
}

fn part(owner: &str, part: &str) -> usize {
    let table = <Ui as Widget>::SUB_TABLE;
    let o = fulgurite::style::index_of_wid(table, owner).unwrap();
    fulgurite::style::subtree_range(table, o).find(|&k| table[k].wid == part).unwrap()
}

fn click(h: &mut Host<App>, x: f32, y: f32) -> bool {
    h.post(Event::PointerDown { x, y, button: fulgurite::PointerButton::Left, clicks: 1 });
    let a = h.pump();
    h.post(Event::PointerUp { x, y, button: fulgurite::PointerButton::Left });
    a | h.pump()
}

fn open(h: &mut Host<App>, owner: &str) {
    let t = h.states[part(owner, "picker-trigger")];
    click(h, t.pos.x + 20.0, t.pos.y + t.size.h / 2.0);
}

#[test]
fn a_press_inside_the_pad_off_every_cell_keeps_it_open() {
    let mut h = host();
    open(&mut h, "date");
    assert!(h.app.ui.date.is_open(), "setup: the trigger did not open the date picker");
    let pad = h.states[part("date", "picker-pad")];
    // The weekday strip: below the 28 px header, above the first row.
    let (x, y) = (pad.pos.x + 40.0, pad.pos.y + 8.0 + 28.0 + 6.0);
    let handled = click(&mut h, x, y);
    println!("pad at {:?}+{:?}; weekday-strip press handled={handled}, open={}", pad.pos, pad.size, h.app.ui.date.is_open());
    assert!(h.app.ui.date.is_open(), "a press on the weekday strip closed the popup");
    // And the corner padding.
    click(&mut h, pad.pos.x + 3.0, pad.pos.y + pad.size.h - 3.0);
    assert!(h.app.ui.date.is_open(), "a press in the pad's padding closed the popup");
    // The trigger still closes it.
    open(&mut h, "date");
    assert!(!h.app.ui.date.is_open(), "the trigger no longer closes it");
}

#[test]
fn space_opens_a_focused_picker() {
    let mut h = host();
    h.input.focus = Some(part("date", "picker-trigger") as u32);
    h.post(Event::Key { key: Key::Space, down: true });
    h.post(Event::Key { key: Key::Space, down: false });
    h.pump();
    assert!(h.app.ui.date.is_open(), "Space did not open the picker");
}

#[test]
fn the_hour_keeps_the_clock_open_and_the_minute_closes_it() {
    let mut h = host();
    open(&mut h, "time");
    let pad = h.states[part("time", "picker-pad")];
    let cell = |c: usize, r: usize| (pad.pos.x + 8.0 + 34.0 * c as f32 + 17.0, pad.pos.y + 8.0 + 26.0 * r as f32 + 13.0);
    // Hour 14: row 2, column 2.
    let (x, y) = cell(2, 2);
    click(&mut h, x, y);
    println!("after the hour: {} open={}", h.app.ui.time.text(), h.app.ui.time.is_open());
    assert_eq!(h.app.ui.time.text(), "14:00", "setup: the press did not pick hour 14");
    assert!(h.app.ui.time.is_open(), "picking the hour closed the clock");
    // :15 is minute cell 3: row 4, column 3.
    let (x, y) = cell(3, 4);
    click(&mut h, x, y);
    println!("after the minute: {} open={}", h.app.ui.time.text(), h.app.ui.time.is_open());
    assert_eq!(h.app.ui.time.text(), "14:15");
    assert!(!h.app.ui.time.is_open(), "the minute did not close the clock");
}

#[test]
fn a_long_file_list_is_a_window_that_scrolls() {
    let mut h = host();
    open(&mut h, "file");
    let pad = h.states[part("file", "picker-pad")];
    println!("file pad {:?}+{:?}", pad.pos, pad.size);
    assert!(pad.size.h < 26.0 * 9.0 + 16.0, "the pad is {} px tall for 20 names", pad.size.h);
    let (x, y) = (pad.pos.x + 40.0, pad.pos.y + 8.0 + 13.0);
    h.post(Event::Scroll { x, y, dy: -26.0 * 3.0 });
    let scrolled = h.pump();
    // The first row on show is now the fourth name.
    click(&mut h, x, y);
    println!("wheel handled={scrolled}; chose {}", h.app.ui.file.text());
    assert!(scrolled, "the wheel did not reach the list");
    assert_eq!(h.app.ui.file.text(), "f03", "the first visible row after three rows of wheel is not f03");
}
