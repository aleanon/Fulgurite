//! **The editor formats from the keyboard, and End goes to the end** —
//! M57/B8 (`docs/plan-widget-fixes.md`).
//!
//! `RichTextEditor::bold` and `code` existed and nothing called them, and
//! Home/End were unhandled (`docs/widget-audit.md`, RichTextEditor).
//! Through a `Host`, because Ctrl+B reaches the editor only by the text
//! area declining it and the pass walking up — and because modifiers are
//! state the pass tracks from `Event::Modifiers`, which a direct
//! `on_event` call would hand in and so could not test.

use fulgurite::event::{Key, Mods};
use fulgurite::{App as DeriveApp, Event, StaticTree, Widget};
use fulgurite_shell::Host;
use fulgurite_widgets::RichTextEditor;

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    #[widget_id("ed")]
    ed: RichTextEditor,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

fn host(src: &str) -> Host<App> {
    let mut app = App::default();
    app.ui.ed = RichTextEditor::new(src);
    let mut h = Host::new(app, 400, 300);
    h.pump();
    let table = <Ui as Widget>::SUB_TABLE;
    let ed = fulgurite::style::index_of_wid(table, "ed").unwrap();
    let span = fulgurite::style::subtree_range(table, ed);
    let input = span.clone().find(|&k| table[k].wid == "editor-input").unwrap();
    h.input.focus = Some(input as u32);
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
fn ctrl_b_and_ctrl_e_format_the_selection() {
    let mut h = host("word");
    key(&mut h, Key::Char('a'), Mods::CTRL);
    assert_eq!(h.app.ui.ed.input.selected_str(), Some("word"), "setup: Ctrl+A selected nothing");
    let handled = key(&mut h, Key::Char('b'), Mods::CTRL);
    println!("after Ctrl+B: {:?} (handled {handled})", h.app.ui.ed.input.as_str());
    assert_eq!(h.app.ui.ed.input.as_str(), "**word**", "Ctrl+B did not bold the selection");
    let mut h = host("x");
    key(&mut h, Key::End, Mods::NONE);
    key(&mut h, Key::Char('e'), Mods::CTRL);
    println!("after Ctrl+E at the end: {:?}", h.app.ui.ed.input.as_str());
    assert_eq!(h.app.ui.ed.input.as_str(), "x``", "Ctrl+E did not insert a code pair");
    // A plain `b` is typing, not a command.
    let mut h = host("");
    assert!(!key(&mut h, Key::Char('b'), Mods::NONE) || h.app.ui.ed.input.as_str().is_empty());
    assert_eq!(h.app.ui.ed.input.as_str(), "", "an unmodified key ran a command");
}

/// Byte moves only: a headless host's `BlockMetrics` shapes into no
/// clusters at all (`TextMeasure::shape`'s default), so Up and Down have
/// no geometry to use here and would test nothing.
#[test]
fn home_and_end_move_within_the_line() {
    let mut h = host("first line\nsecond");
    let caret = |h: &Host<App>| h.app.ui.ed.input.caret();
    // A new area's caret is at the end of its text.
    assert_eq!(caret(&h), 17, "setup: the caret does not start at the end");
    key(&mut h, Key::Home, Mods::NONE);
    let home2 = caret(&h);
    key(&mut h, Key::End, Mods::NONE);
    let end2 = caret(&h);
    key(&mut h, Key::Home, Mods::NONE);
    key(&mut h, Key::Left, Mods::NONE);
    assert_eq!(caret(&h), 10, "setup: Left from the second line did not reach the first");
    key(&mut h, Key::Home, Mods::NONE);
    let home1 = caret(&h);
    key(&mut h, Key::End, Mods::NONE);
    let end1 = caret(&h);
    println!("second line: Home {home2}, End {end2}; first line: Home {home1}, End {end1}");
    assert_eq!((home2, end2, home1, end1), (11, 17, 0, 10), "Home/End did not reach the line's ends");
}
