//! **A keyboard can choose from a Select** — M57/B21.
//!
//! A `Select`'s rows are the application's (virtual, bound, never
//! focused). Down and Up did nothing, Tab was trapped in the overlay and
//! focus never reached a row, and Enter on the trigger ran the trigger's
//! handler: a keyboard user could open the menu and not choose from it
//! (`docs/widget-audit.md`, Select). Down and Up now walk a highlight,
//! and Enter reaches the highlighted row's own handler, with `ctx.row`.

use fulgurite::event::Key;
use fulgurite::{App as DeriveApp, Event, EventCtx, StaticTree, Widget};
use fulgurite_shell::Host;
use fulgurite_widgets::select::Row;
use fulgurite_widgets::Select;

const OPTS: [&str; 3] = ["Apple", "Banana", "Cherry"];

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("sel")]
    sel: Select<App>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
    pick: Option<u64>,
}

fn bind(r: &mut Row<App>, i: usize) {
    r.label.clear();
    r.label.push_str(OPTS[i]);
    r.on_click = |s: &mut App, cx: &EventCtx| s.pick = cx.row;
}

fn key(h: &mut Host<App>, k: Key) {
    h.post(Event::Key { key: k, down: true });
    h.post(Event::Key { key: k, down: false });
    h.pump();
}

#[test]
fn down_down_enter_chooses_the_second_row() {
    let mut app = App::default();
    app.ui.sel = Select::new(3, 30.0, bind);
    app.ui.sel.menu.open = true;
    let mut h: Host<App> = Host::new(app, 400, 400);
    h.pump();
    let trigger = <Ui as Widget>::SUB_TABLE.iter().position(|d| d.wid == "select-trigger").unwrap_or(1) as u32;
    h.input.focus = Some(trigger);
    h.pump();
    key(&mut h, Key::Down);
    key(&mut h, Key::Down);
    key(&mut h, Key::Enter);
    println!("Down, Down, Enter: picked {:?}", h.app.pick);
    assert_eq!(h.app.pick, Some(1), "the keyboard did not choose the second row");
}
