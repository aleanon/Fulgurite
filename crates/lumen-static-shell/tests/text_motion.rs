//! **Words, clicks and line ends** — M45/T45.0 item 9, for R10-F14
//! (`docs/plan-text-correctness.md` §1.4).
//!
//! No widget read `EventCtx.clicks`, although the pass sets it and
//! `docs/plan-desktop.md` T14.3 carries it on the event for exactly this;
//! neither editor moved by word; `TextArea::on_event` had no Home or End
//! arm.
//!
//! Each field is **alone** in its tree, so the arrow rule (M44) cannot
//! mask a word motion. The measurer reports a cluster per character, half
//! an em apart, so a click's x names a byte.

use lumen_static::cx::{Cluster, GlyphSink, TextMeasure};
use lumen_static::prelude::*;
use lumen_static::{Event, Key, Mods, PointerButton, Size};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;
use lumen_static_widgets::TextArea;

struct Chars;

const PX: f32 = 16.0;

impl TextMeasure for Chars {
    fn measure(&mut self, t: &str, size: f32, max_w: f32) -> Size {
        self.shape(t, size, 400.0, max_w, &mut lumen_static::NoSink)
    }
    fn shape(&mut self, t: &str, size: f32, _w: f32, _max_w: f32, sink: &mut dyn GlyphSink) -> Size {
        let (mut x, mut line, mut w) = (0.0f32, 0u16, 0.0f32);
        for (b, c) in t.char_indices() {
            sink.cluster(Cluster { byte: b as u32, x, line });
            if c == '\n' {
                x = 0.0;
                line += 1;
            } else {
                x += size * 0.5;
                w = w.max(x);
            }
        }
        sink.cluster(Cluster { byte: t.len() as u32, x, line });
        Size { w, h: size * (line as f32 + 1.0) }
    }
    fn line_height(&mut self, size: f32) -> f32 {
        size
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct InputUi {
    #[widget_id("w")]
    w: TextInput,
}
#[derive(DeriveApp, Default)]
struct InputApp {
    #[tree]
    ui: InputUi,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct AreaUi {
    #[widget_id("w")]
    w: TextArea,
}
#[derive(DeriveApp, Default)]
struct AreaApp {
    #[tree]
    ui: AreaUi,
}

fn pump<A: App + 'static>(h: &mut Host<A>) {
    h.pump_with(&mut Chars);
}

fn key<A: App + 'static>(h: &mut Host<A>, mods: Mods, k: Key) {
    h.post(Event::Modifiers(mods));
    h.post(Event::Key { key: k, down: true });
    h.post(Event::Modifiers(Mods::NONE));
    pump(h);
}

/// A press completing `clicks` clicks at byte `at` of line `line`.
fn clicks<A: App + 'static>(h: &mut Host<A>, at: usize, line: usize, n: u8) {
    let s = h.states[1];
    let (x, y) = (s.pos.x + s.pad.0 + (at as f32 + 0.5) * PX * 0.5, s.pos.y + s.pad.1 + (line as f32 + 0.5) * PX);
    for k in 1..=n {
        h.post(Event::PointerDown { x, y, button: PointerButton::Left, clicks: k });
        h.post(Event::PointerUp { x, y, button: PointerButton::Left });
        pump(h);
    }
}

fn input(text: &str) -> Host<InputApp> {
    let mut a = InputApp::default();
    a.ui.w.set_str(text);
    let mut h: Host<InputApp> = Host::new(a, 600, 80);
    pump(&mut h);
    h.post(Event::Key { key: Key::Tab, down: true });
    pump(&mut h);
    assert_eq!(h.input.focus, Some(1), "setup: Tab did not focus the field");
    h
}

fn area(text: &str) -> Host<AreaApp> {
    let mut a = AreaApp::default();
    a.ui.w.set_str(text);
    let mut h: Host<AreaApp> = Host::new(a, 600, 200);
    pump(&mut h);
    h.post(Event::Key { key: Key::Tab, down: true });
    pump(&mut h);
    assert_eq!(h.input.focus, Some(1), "setup: Tab did not focus the area");
    h
}

type Sel = Option<core::ops::Range<usize>>;

fn check(name: &str, got: (usize, Sel), want: (usize, Sel), wrong: &mut usize) {
    let ok = got == want;
    println!("{name:<44} caret {:<3} sel {:<10} want caret {:<3} sel {:<10} {}", got.0, format!("{:?}", got.1), want.0, format!("{:?}", want.1), if ok { "ok" } else { "WRONG" });
    *wrong += !ok as usize;
}

const WORD: Mods = if cfg!(target_os = "macos") { Mods::ALT } else { Mods::CTRL };
const JUMP: Mods = if cfg!(target_os = "macos") { Mods::META } else { Mods::CTRL };

#[test]
#[ignore = "red until T45.6: R10-F14, no word motion, no multi-click, no Home/End in TextArea"]
fn words_clicks_and_line_ends() {
    let mut wrong = 0;
    // "hello brave world": words at 0..5, 6..11, 12..17.
    let t = "hello brave world";

    let mut h = input(t);
    key(&mut h, WORD, Key::Left);
    check("TextInput: word-Left from the end", (h.app.ui.w.caret(), h.app.ui.w.selection()), (12, None), &mut wrong);
    key(&mut h, WORD.with(Mods::SHIFT), Key::Left);
    check("TextInput: Shift+word-Left extends", (h.app.ui.w.caret(), h.app.ui.w.selection()), (6, Some(6..12)), &mut wrong);
    key(&mut h, Mods::NONE, Key::Home);
    key(&mut h, WORD, Key::Right);
    check("TextInput: word-Right from the start", (h.app.ui.w.caret(), h.app.ui.w.selection()), (5, None), &mut wrong);

    let mut h = input(t);
    clicks(&mut h, 8, 0, 2);
    check("TextInput: a double click selects the word", (h.app.ui.w.caret(), h.app.ui.w.selection()), (11, Some(6..11)), &mut wrong);
    let mut h = input(t);
    clicks(&mut h, 8, 0, 3);
    check("TextInput: a triple click selects everything", (h.app.ui.w.caret(), h.app.ui.w.selection()), (17, Some(0..17)), &mut wrong);

    // "one two\nthree four\nfive": line 1 is bytes 8..18.
    let t = "one two\nthree four\nfive";
    let mut h = area(t);
    clicks(&mut h, 2, 1, 1);
    let at = h.app.ui.w.caret();
    assert_eq!(at, 10, "setup: the click did not land on byte 10 of line 1");
    key(&mut h, Mods::NONE, Key::End);
    check("TextArea: End goes to the line's end", (h.app.ui.w.caret(), h.app.ui.w.selection()), (18, None), &mut wrong);
    key(&mut h, Mods::NONE, Key::Home);
    check("TextArea: Home goes to the line's start", (h.app.ui.w.caret(), h.app.ui.w.selection()), (8, None), &mut wrong);
    key(&mut h, JUMP, Key::End);
    check("TextArea: Ctrl+End goes to the end", (h.app.ui.w.caret(), h.app.ui.w.selection()), (23, None), &mut wrong);
    key(&mut h, JUMP, Key::Home);
    check("TextArea: Ctrl+Home goes to the start", (h.app.ui.w.caret(), h.app.ui.w.selection()), (0, None), &mut wrong);
    key(&mut h, WORD, Key::Right);
    check("TextArea: word-Right", (h.app.ui.w.caret(), h.app.ui.w.selection()), (3, None), &mut wrong);

    let mut h = area(t);
    clicks(&mut h, 7, 1, 2);
    check("TextArea: a double click selects the word", (h.app.ui.w.caret(), h.app.ui.w.selection()), (18, Some(14..18)), &mut wrong);
    let mut h = area(t);
    clicks(&mut h, 7, 1, 3);
    check("TextArea: a triple click selects the line", (h.app.ui.w.caret(), h.app.ui.w.selection()), (18, Some(8..18)), &mut wrong);

    println!("wrong {wrong} (bound 0)");
    assert_eq!(wrong, 0, "{wrong} word, click or line-end motions landed wrong");
}

/// **The accept direction**: a plain Left still moves one character.
#[test]
fn a_plain_left_still_moves_one_character() {
    let mut h = input("hello brave world");
    key(&mut h, Mods::NONE, Key::Left);
    println!("plain Left from 17: caret {}", h.app.ui.w.caret());
    assert_eq!(h.app.ui.w.caret(), 16);
}
