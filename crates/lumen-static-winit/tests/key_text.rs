//! **Key text arrives whole** — M45/T45.0 item 10, for R10-F8
//! (`docs/plan-text-correctness.md` §1.4).
//!
//! winit's `KeyEvent::text` can carry more than one character: a dead key
//! followed by a letter it cannot combine with (`^` then `a` on Windows's
//! US-International layout delivers `"^a"`), a decomposed accent, a flag,
//! an Indic conjunct from a layout that commits one per key. The
//! translation took the first non-control character and dropped the rest.
//!
//! Driven through `keys::deliver_key` — the function the window's event
//! handler calls for every `KeyboardInput` — into a real `Local` session,
//! under the window's default invalidation (`Boundaries`), so a string
//! that arrived but was never laid out is a failure here too.

use lumen_static::prelude::*;
use lumen_static::{Color, Event, Key, Mods};
use lumen_static_shell::session::{Local, Recover, Session};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;
use lumen_static_winit::keys::deliver_key;
use lumen_static_winit::translate::keyboard;
use winit::keyboard::{Key as WKey, NamedKey};

#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
struct Ui {
    #[widget_id("field")]
    field: TextInput,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

const FIELD: usize = 1;

fn session() -> Local<App> {
    let mut s = Local::from_host(Host::new(App::default(), 600, 60), "", Color(255, 255, 255, 255), Recover::Off);
    s.frame(0.0, 600, 60);
    assert!(deliver_key(&mut s, &WKey::Named(NamedKey::Tab), None, true, Mods::NONE), "setup: Tab reached nothing");
    s.frame(0.0, 600, 60);
    assert_eq!(s.host.input.focus, Some(FIELD as u32), "setup: Tab did not focus the field");
    s
}

/// One key press whose text is `text`, then a frame. Returns the value
/// and the field's width before and after.
fn press(text: &str) -> (String, f32, f32) {
    let mut s = session();
    let before = s.host.states[FIELD].size.w;
    let label: WKey = WKey::Character(text.into());
    deliver_key(&mut s, &label, Some(text), true, Mods::NONE);
    deliver_key(&mut s, &label, None, false, Mods::NONE);
    s.frame(0.0, 600, 60);
    s.frame(0.0, 600, 60);
    (s.host.app.ui.field.as_str().to_owned(), before, s.host.states[FIELD].size.w)
}

#[test]
#[ignore = "red until T45.8: R10-F8, key text past its first character is lost"]
fn multi_character_key_text_reaches_the_field_whole() {
    let cases = ["^a", "e\u{301}", "\u{1f1f3}\u{1f1f4}", "\u{915}\u{94d}\u{937}"];
    let mut whole = 0;
    for t in cases {
        let (v, w0, w1) = press(t);
        let ok = v == t && w1 > w0;
        println!("{t:?}: field holds {v:?} ({} of {} chars), width {w0} -> {w1} | bound: whole and wider -> {ok}", v.chars().count(), t.chars().count());
        whole += ok as usize;
    }
    println!("whole {whole} / {} (bound {})", cases.len(), cases.len());
    assert_eq!(whole, cases.len(), "{} of {} multi-character key texts arrived whole", whole, cases.len());
}

/// **The accept direction**: a single character still goes through
/// `Event::Text`, so recorded sessions do not change.
#[test]
fn a_single_character_is_still_an_event_text() {
    let t = keyboard(&WKey::Character("a".into()), Some("a"), true, Mods::NONE);
    let evs: Vec<Event> = t.iter().collect();
    println!("trace for \"a\": {evs:?}");
    assert!(evs.contains(&Event::Text { ch: 'a' }), "a single 'a' did not produce Event::Text");
    assert!(evs.contains(&Event::Key { key: Key::Char('a'), down: true }));
    let (v, w0, w1) = press("a");
    println!("\"a\": field holds {v:?}, width {w0} -> {w1}");
    assert_eq!(v, "a");
}
