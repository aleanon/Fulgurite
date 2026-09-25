//! **A chooser's list closes on a choice and on Escape, and stays
//! closed** — M57/A7 (`docs/plan-widget-fixes.md`).
//!
//! The audit drove a `PickList` and a `Combobox` in the live window: a
//! choice landed and the list stayed open, and so did Escape
//! (`docs/widget-audit.md` X5). `Chooser::layout` closed the list, then
//! opened it again because its field read as focused — a *level* checked
//! every layout. The unit test beside it called `on_event` directly, so
//! no layout pass ever ran and it could not see this.
//!
//! So this goes through a `Host`, frame by frame, and checks the list is
//! still closed two idle frames later — a list that reopens on the next
//! layout is the defect.

use lumen_static::event::Key;
use lumen_static::{App as DeriveApp, Event, StaticTree, Widget};
use lumen_static_shell::Host;
use lumen_static_widgets::{Combobox, PickList};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 24.0)]
struct Ui {
    #[widget_id("pick")]
    pick: PickList,
    #[widget_id("combo")]
    combo: Combobox,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

const FRUIT: &[&str] = &["Apple", "Banana", "Cherry", "Damson"];

fn host() -> Host<App> {
    let mut app = App::default();
    app.ui.pick = PickList::new(FRUIT);
    app.ui.combo = Combobox::new(FRUIT);
    let mut h = Host::new(app, 500, 500);
    h.pump();
    h
}

fn part(owner: &str, part: &str) -> usize {
    let table = <Ui as Widget>::SUB_TABLE;
    let o = lumen_static::style::index_of_wid(table, owner).unwrap();
    let span = lumen_static::style::subtree_range(table, o);
    span.clone().find(|&k| table[k].wid == part).unwrap_or_else(|| panic!("no {part} under #{owner}"))
}

fn click(h: &mut Host<App>, x: f32, y: f32) {
    h.post(Event::PointerDown { x, y, button: lumen_static::PointerButton::Left, clicks: 1 });
    h.pump();
    h.post(Event::PointerUp { x, y, button: lumen_static::PointerButton::Left });
    h.pump();
}

fn centre(h: &Host<App>, i: usize) -> (f32, f32) {
    let s = h.states[i];
    (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
}

fn open(h: &mut Host<App>, owner: &str) {
    let (x, y) = centre(h, part(owner, "chooser-trigger"));
    click(h, x, y);
}

fn is_open(h: &Host<App>, owner: &str) -> bool {
    match owner {
        "pick" => h.app.ui.pick.is_open(),
        _ => h.app.ui.combo.is_open(),
    }
}

fn idle(h: &mut Host<App>) {
    h.invalidate_all();
    h.pump();
    h.invalidate_all();
    h.pump();
}

#[test]
fn a_choice_closes_the_list_and_it_stays_closed() {
    for owner in ["pick", "combo"] {
        let mut h = host();
        open(&mut h, owner);
        assert!(is_open(&h, owner), "setup: a press on #{owner}'s field did not open it");
        // The third row of the open list.
        let list = h.states[part(owner, "chooser-list")];
        let (x, y) = (list.pos.x + 40.0, list.pos.y + 6.0 + 30.0 * 2.0 + 15.0);
        click(&mut h, x, y);
        let chosen = match owner {
            "pick" => h.app.ui.pick.selected(),
            _ => h.app.ui.combo.selected(),
        };
        assert_eq!(chosen, Some(2), "setup: the press on the third row did not choose it on #{owner}");
        let now = is_open(&h, owner);
        idle(&mut h);
        println!("#{owner}: after the choice open={now}, two frames later open={}", is_open(&h, owner));
        assert!(!now && !is_open(&h, owner), "#{owner}: the list is still open after a choice");
    }
}

#[test]
fn escape_closes_the_list_and_it_stays_closed() {
    for owner in ["pick", "combo"] {
        let mut h = host();
        open(&mut h, owner);
        assert!(is_open(&h, owner), "setup: #{owner} did not open");
        h.post(Event::Key { key: Key::Escape, down: true });
        let handled = h.pump();
        idle(&mut h);
        println!("#{owner}: Escape handled={handled}, open after={}", is_open(&h, owner));
        assert!(handled, "Escape reached nothing on #{owner}");
        assert!(!is_open(&h, owner), "#{owner}: the list is still open after Escape");
    }
}

/// The transition the old level stood in for: reaching the field by Tab
/// opens it, once.
#[test]
fn tab_to_a_picklist_opens_it() {
    let mut h = host();
    h.post(Event::Key { key: Key::Tab, down: true });
    h.pump();
    assert_eq!(h.input.focus, Some(part("pick", "chooser-trigger") as u32), "setup: Tab did not reach the field");
    assert!(h.app.ui.pick.is_open(), "Tab reached the picklist's field and it did not open");
}
