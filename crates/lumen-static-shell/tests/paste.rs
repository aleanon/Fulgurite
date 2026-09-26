//! **A paste is one edit that keeps what the widget keeps** — M45/T45.0
//! item 7, for R8-F4, R8-F5, R10-F5 and R10-F6
//! (`docs/plan-text-correctness.md` §1.3, §2.2).
//!
//! `Host::paste` posted one `Event::Text` per character, keeping `'\n'` in
//! its filter; a `TextArea` accepts text only when it is not a control
//! character and takes a newline only as `Key::Enter`, so every pasted
//! newline was dropped while `paste` counted it as delivered. It pumped
//! once per 64 characters, each pump linear in the text, so a large paste
//! was quadratic. And the test named for newlines pasted no newline into
//! a single-line field.
//!
//! Every case runs through `Host` under both invalidations (§0 rule 2),
//! focuses by **typing first** — a paste with nothing focused keeps
//! nothing and would make every comparison vacuous — and compares with
//! the typed control: the path a paste claims to be.

use lumen_static::invalidate::{Everything, Invalidation};
use lumen_static::prelude::*;
use lumen_static::retain::Incremental;
use lumen_static::{Event, Key};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct Ui {
    #[widget_id("area")]
    area: TextArea,
    #[widget_id("field")]
    field: TextInput,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

const AREA: usize = 1;
const FIELD: usize = 2;

/// A host with `target` focused by Tab, and the focus proved by typing a
/// character into it and deleting it again.
fn focused<I: Invalidation>(target: usize) -> Host<App, Incremental, I> {
    let mut h: Host<App, Incremental, I> = Host::new(App::default(), 400, 300);
    h.pump();
    for _ in 0..target {
        h.post(Event::Key { key: Key::Tab, down: true });
        h.pump();
    }
    assert_eq!(h.input.focus, Some(target as u32), "setup: Tab did not reach node {target}");
    h.post(Event::Text { ch: 'z' });
    h.pump();
    assert_eq!(value(&h, target), "z", "setup: typing did not land in node {target}");
    h.post(Event::Key { key: Key::Backspace, down: true });
    h.pump();
    assert_eq!(value(&h, target), "", "setup: the probe character stayed");
    h
}

fn value<I: Invalidation>(h: &Host<App, Incremental, I>, at: usize) -> String {
    match at {
        AREA => h.app.ui.area.as_str().to_owned(),
        _ => h.app.ui.field.as_str().to_owned(),
    }
}

/// Type `s` as a person would: Enter for a line break, one event each.
fn typed<I: Invalidation>(target: usize, s: &str) -> Host<App, Incremental, I> {
    let mut h = focused::<I>(target);
    for ch in s.chars() {
        match ch {
            '\n' => h.post(Event::Key { key: Key::Enter, down: true }),
            c => h.post(Event::Text { ch: c }),
        }
        h.pump();
    }
    h.pump();
    h
}

const MIXED: &str = "ab\ncd\r\nef";

fn case<I: Invalidation>(strategy: &str, target: usize, want: &str, control: &str) -> usize {
    let mut p = focused::<I>(target);
    let n = p.paste(MIXED);
    p.pump();
    let t = typed::<I>(target, control);
    let (pv, tv) = (value(&p, target), value(&t, target));
    let same = p.list.hash_from(0) == t.list.hash_from(0);
    println!(
        "{strategy} node {target}: pasted {MIXED:?} -> {pv:?} (want {want:?}), count {n} (want {}), typed {control:?} -> {tv:?}, frames equal {same}",
        want.chars().count()
    );
    let mut bad = 0;
    bad += (pv != want) as usize;
    bad += (n != want.chars().count()) as usize;
    bad += (tv != want) as usize;
    bad += !same as usize;
    bad
}

#[test]
fn a_paste_keeps_newlines_in_a_text_area_and_strips_them_in_a_field() {
    let mut bad = 0;
    bad += case::<Everything>("Everything", AREA, "ab\ncd\nef", "ab\ncd\nef");
    bad += case::<lumen_static::Boundaries<256>>("Boundaries<256>", AREA, "ab\ncd\nef", "ab\ncd\nef");
    bad += case::<Everything>("Everything", FIELD, "abcdef", "abcdef");
    bad += case::<lumen_static::Boundaries<256>>("Boundaries<256>", FIELD, "abcdef", "abcdef");
    println!("mismatches {bad} (bound 0)");
    assert_eq!(bad, 0, "{bad} paste results differ from what the widget keeps or from typing");
}

fn pumps<I: Invalidation>(strategy: &str, len: usize) -> (u64, bool) {
    let mut h = focused::<I>(AREA);
    let line = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcde\n";
    let text: String = line.repeat(len / line.len());
    let f0 = h.frame_number();
    let n = h.paste(&text);
    let pumped = h.frame_number() - f0;
    println!("{strategy}: a {} KB paste pumped {pumped} times (bound 1), kept {n} of {}", text.len() / 1024, text.chars().count());
    let whole = h.app.ui.area.as_str() == text;
    println!("{strategy}: landed whole {whole} ({} of {} bytes)", h.app.ui.area.as_str().len(), text.len());
    (pumped, whole)
}

#[test]
fn a_large_paste_pumps_once() {
    let (e, ew) = pumps::<Everything>("Everything", 256 * 1024);
    let (b, bw) = pumps::<lumen_static::Boundaries<256>>("Boundaries<256>", 256 * 1024);
    assert!(e == 1 && b == 1, "a paste pumped {e}/{b} times; one edit is one pump");
    assert!(ew && bw, "the paste did not land whole");
}

/// **The accept direction**: a ten-character paste pumps once and equals
/// typing.
#[test]
fn a_short_paste_pumps_once_and_equals_typing() {
    let mut p = focused::<Everything>(FIELD);
    let f0 = p.frame_number();
    assert_eq!(p.paste("0123456789"), 10);
    let pumped = p.frame_number() - f0;
    let t = typed::<Everything>(FIELD, "0123456789");
    println!("ten characters: pumped {pumped} (bound 1), pasted {:?} typed {:?}", value(&p, FIELD), value(&t, FIELD));
    assert_eq!(pumped, 1);
    assert_eq!(value(&p, FIELD), value(&t, FIELD));
}
