//! **A search field clears on Escape and on its ×, submits on Enter, and
//! keeps a long query's end in view** — M57/B26
//! (`docs/plan-widget-fixes.md`).
//!
//! `SearchField::clear` existed and nothing called it; there was no ×
//! and no submit (`docs/widget-audit.md`, SearchField).

use lumen_static::display::Cmd;
use lumen_static::event::Key;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{App as DeriveApp, Event, StaticTree, Widget};
use lumen_static_shell::Host;
use lumen_static_widgets::SearchField;

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    #[widget_id("q")]
    q: SearchField,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

fn host(text: &str) -> Host<App> {
    let mut app = App::default();
    app.ui.q = SearchField::new(text);
    let mut h = Host::new(app, 400, 100);
    h.set_stylesheet(&[Rule::new(Selector::Id("q"), Patch::new().width(Length::Fixed(220.0)))]);
    h.pump();
    let field = at("q") + 2;
    h.input.focus = Some(field as u32);
    h.pump();
    h
}

fn at(id: &str) -> usize {
    lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, id).unwrap()
}

fn key(h: &mut Host<App>, k: Key) -> bool {
    h.post(Event::Key { key: k, down: true });
    h.post(Event::Key { key: k, down: false });
    h.pump()
}

fn click(h: &mut Host<App>, x: f32, y: f32) {
    h.post(Event::PointerDown { x, y, button: lumen_static::PointerButton::Left, clicks: 1 });
    h.pump();
    h.post(Event::PointerUp { x, y, button: lumen_static::PointerButton::Left });
    h.pump();
}

#[test]
fn escape_clears_the_query() {
    let mut h = host("rust");
    let handled = key(&mut h, Key::Escape);
    println!("Escape handled={handled}, query {:?}", h.app.ui.q.as_str());
    assert_eq!(h.app.ui.q.as_str(), "", "Escape did not clear the query");
}

#[test]
fn the_x_shows_with_a_query_and_clears_it() {
    let mut h = host("rust");
    let s = h.states[at("q")];
    let lines = |h: &Host<App>| h.list.cmds.iter().filter(|c| matches!(c, Cmd::Line { .. })).count();
    let with = lines(&h);
    // Past the text, on the ×: the right padding plus half its column.
    let (x, y) = (s.pos.x + s.size.w - 12.0 - 10.0, s.pos.y + s.size.h / 2.0);
    click(&mut h, x, y);
    let without = lines(&h);
    println!("lines with a query {with}, after the × {without}; query {:?}", h.app.ui.q.as_str());
    assert_eq!(h.app.ui.q.as_str(), "", "a press on the × did not clear");
    assert_eq!(with - without, 2, "the × is not two strokes that go with the query");
    // A press on the text does not clear.
    let mut h = host("rust");
    click(&mut h, s.pos.x + 50.0, y);
    assert_eq!(h.app.ui.q.as_str(), "rust", "a press on the text cleared the query");
}

#[test]
fn enter_submits() {
    let mut h = host("rust");
    key(&mut h, Key::Enter);
    assert!(h.app.ui.q.take_submitted(), "Enter did not submit");
    assert!(!h.app.ui.q.take_submitted(), "a submission was taken twice");
}

#[test]
fn a_long_query_keeps_its_end_in_view() {
    let mut h = host("a query far longer than the field it is typed into");
    key(&mut h, Key::Home);
    key(&mut h, Key::End);
    let f = h.states[at("q") + 2];
    let caret = h
        .list
        .cmds
        .iter()
        .find_map(|c| match c {
            Cmd::Rect { rect, .. } if (rect[2] - rect[0] - 1.0).abs() < 1e-3 => Some(rect[0]),
            _ => None,
        })
        .expect("setup: no caret");
    // **The pill's box, not the field's.** The first version bounded the
    // caret by the field, which was laid out 328 px wide inside a 220 px
    // pill: a caret at 360 passed, 140 px outside the control.
    let p = h.states[at("q")];
    let (lo, hi) = (p.pos.x, p.pos.x + p.size.w);
    assert!(f.pos.x + f.size.w <= hi + 0.5, "the field runs past the pill: {} > {hi}", f.pos.x + f.size.w);
    let text_x = h
        .list
        .cmds
        .iter()
        .find_map(|c| match c {
            Cmd::Text { rect, .. } => Some(rect[0]),
            _ => None,
        })
        .unwrap();
    let pill = h.states[at("q")];
    println!("pill {:?}; pill [{lo}, {hi}], field ends {}, caret at {caret}, text starts at {text_x}", pill.size, f.pos.x + f.size.w);
    // The setup: the query must not fit, or "in view" is true of any caret.
    assert!(text_x < f.pos.x, "setup: the query fits in the field and never scrolled");
    assert!(caret >= lo && caret <= hi, "the caret at the end of a long query is outside the field");
}
