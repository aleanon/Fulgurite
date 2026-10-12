//! **A pane grid: every divider by keyboard, the right cursor over each,
//! and a pixel minimum** — M57/B48 (`docs/plan-widget-fixes.md`,
//! `docs/widget-audit.md` PaneGrid). Only the vertical divider had keys,
//! one cursor covered all three, and the stop was a fraction.

use fulgurite::event::{Key, Mods};
use fulgurite::style::Cursor;
use fulgurite::{App as DeriveApp, Event, StaticTree, Widget};
use fulgurite_shell::Host;
use fulgurite_widgets::ops::Spacer;
use fulgurite_widgets::PaneGrid;

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    #[widget_id("g")]
    g: PaneGrid<Spacer, Spacer, Spacer, Spacer>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

fn host() -> Host<App> {
    let mut h: Host<App> = Host::new(App::default(), 1000, 800);
    h.pump();
    let i = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "g").unwrap();
    h.input.focus = Some(i as u32);
    h.pump();
    h
}

fn key(h: &mut Host<App>, k: Key, mods: Mods) -> bool {
    h.post(Event::Modifiers(mods));
    h.post(Event::Key { key: k, down: true });
    h.post(Event::Key { key: k, down: false });
    h.post(Event::Modifiers(Mods::NONE));
    h.pump()
}

#[test]
fn tab_walks_the_dividers_and_each_moves_on_its_own_axis() {
    let mut h = host();
    key(&mut h, Key::Right, Mods::NONE);
    let v = h.app.ui.g.vertical();
    key(&mut h, Key::Tab, Mods::NONE);
    key(&mut h, Key::Down, Mods::NONE);
    let left = h.app.ui.g.horizontal(0);
    key(&mut h, Key::Tab, Mods::NONE);
    key(&mut h, Key::Up, Mods::NONE);
    let right = h.app.ui.g.horizontal(1);
    println!("vertical {v}, left column {left}, right column {right}");
    assert!((v - 0.55).abs() < 1e-4, "Right did not move the vertical divider");
    assert!((left - 0.55).abs() < 1e-4, "Down on the second stop did not move the left column's");
    assert!((right - 0.45).abs() < 1e-4, "Up on the third stop did not move the right column's");
    // At the last divider, Tab lets focus go; Shift+Tab walks back first.
    let i = h.input.focus;
    let back = key(&mut h, Key::Tab, Mods::SHIFT);
    assert!(back && h.input.focus == i, "Shift+Tab left the grid instead of walking back");
}

#[test]
fn each_divider_shows_its_own_cursor() {
    let mut h = host();
    let i = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "g").unwrap();
    let s = h.states[i];
    let vx = s.pos.x + (s.size.w - 6.0) * 0.5 + 3.0;
    let hy = s.pos.y + (s.size.h - 6.0) * 0.5 + 3.0;
    h.post(Event::PointerMove { x: vx, y: s.pos.y + 100.0 });
    h.pump();
    let over_v = h.cursor();
    h.post(Event::PointerMove { x: s.pos.x + 100.0, y: hy });
    h.pump();
    let over_h = h.cursor();
    println!("over the vertical divider {over_v:?}, over a horizontal one {over_h:?}");
    assert_eq!((over_v, over_h), (Cursor::ColResize, Cursor::RowResize));
}

#[test]
fn a_pane_stops_at_min_px() {
    let mut h = host();
    for _ in 0..40 {
        key(&mut h, Key::Left, Mods::NONE);
    }
    let i = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "g").unwrap();
    let tl = h.states[i + 1].size.w;
    println!("top-left pane after 40 Lefts: {tl} px (min_px 48)");
    assert!((tl - 48.0).abs() < 0.5, "the pane is not held at min_px");
}
