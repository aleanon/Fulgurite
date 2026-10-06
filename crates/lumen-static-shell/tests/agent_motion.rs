//! **`input.key` holds modifiers and `input.click` counts clicks** —
//! M45/T45.6. The drive harness moves a caret by word and selects by
//! double and triple click with them; without them an agent could reach
//! neither, because a key or a click it sent carried no modifier and a
//! click count of 1.

use lumen_static::prelude::*;
use lumen_static::{Color, Event, Key};
use lumen_static_shell::session::{FrameSource, Local, Crashing, Screen, Session};
use lumen_static_shell::Host;
use lumen_static_widgets::TextArea;
use serde_json::Value;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("area")]
    area: TextArea,
}

#[derive(DeriveApp, Default)]
struct Fx {
    #[tree]
    ui: Ui,
}

struct Blind;

impl Screen for Blind {
    fn present(&mut self, _src: &mut dyn FrameSource, _page: Color) -> lumen_static_shell::session::FrameTimes {
        Default::default()
    }
    fn frame_rgba(&mut self) -> Option<(u32, u32, &[u8])> {
        None
    }
    fn fetch_rgba(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        None
    }
}

const WORD: &str = if cfg!(target_os = "macos") { "alt" } else { "ctrl" };

fn session() -> Local<Fx, Crashing> {
    let mut a = Fx::default();
    a.ui.area.set_str("one two");
    let mut s = Local::from_host(Host::new(a, 300, 200), "motion", Color(255, 255, 255, 255), Crashing);
    s.frame(0.0, 300, 200);
    s.post(Event::Key { key: Key::Tab, down: true });
    s.frame(0.0, 300, 200);
    assert_eq!(s.host.input.focus, Some(1), "setup: Tab did not focus the area");
    s
}

fn call(s: &mut Local<Fx, Crashing>, method: &str, params: &str) -> Value {
    let req = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"{method}","params":{params}}}"#);
    serde_json::from_str(&s.agent(&req, &mut Blind)).expect("JSON")
}

#[test]
fn a_key_with_mods_moves_by_word_and_a_counted_click_selects() {
    let mut s = session();
    let r = call(&mut s, "input.key", &format!(r#"{{"key":"Left","mods":["{WORD}"]}}"#));
    println!("word-Left from 7: {r}; caret {} (want 4)", s.host.app.ui.area.caret());
    assert_eq!(s.host.app.ui.area.caret(), 4, "input.key's mods did not reach the area");
    // The mods are released after the key: a plain Left steps one.
    call(&mut s, "input.key", r#"{"key":"Left"}"#);
    println!("then a plain Left: caret {} (want 3)", s.host.app.ui.area.caret());
    assert_eq!(s.host.app.ui.area.caret(), 3, "the mods outlived their key");

    let r = call(&mut s, "input.click", r##"{"selector":"#area","clicks":3}"##);
    println!("triple click: {r}; selection {:?} (want Some(0..7))", s.host.app.ui.area.selection());
    assert_eq!(s.host.app.ui.area.selection(), Some(0..7), "input.click's clicks did not reach the area");
}

/// **The refusing direction**: a count past three and an unknown
/// modifier are errors, not a click and a plain key.
#[test]
fn a_bad_count_or_modifier_is_refused() {
    let mut s = session();
    let r = call(&mut s, "input.click", r##"{"selector":"#area","clicks":4}"##);
    println!("clicks 4: {r}");
    assert!(r.get("error").is_some(), "{r}");
    let r = call(&mut s, "input.key", r#"{"key":"Left","mods":["hyper"]}"#);
    println!("mods [hyper]: {r}; caret {}", s.host.app.ui.area.caret());
    assert!(r.get("error").is_some(), "{r}");
    assert_eq!(s.host.app.ui.area.caret(), 7, "a refused key still moved the caret");
}
