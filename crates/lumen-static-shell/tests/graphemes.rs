//! **Editing steps by grapheme** — M45/T45.0 item 8, for R10-F7
//! (`docs/plan-text-correctness.md` §1.4).
//!
//! `prev_boundary` and `next_boundary` used `char_indices` in both
//! editors, so Backspace after a combining accent, a ZWJ family, a flag
//! or a skin-tone emoji left half a character — in all five text widgets,
//! since the three composites edit through the two editors.
//!
//! In the shell's tests, not the widgets crate's as the plan named: every
//! case runs through a `Host`, and the widgets crate has no host to run
//! through. Each widget is **alone** in its tree: with a focusable sibling
//! an arrow key moves focus instead (M44's), which would mask `Right`.
//! Focus is by a click at the text's start, asserted; the text is typed.

use lumen_static::cx::{Cluster, GlyphSink, TextMeasure};
use lumen_static::prelude::*;
use lumen_static::{Event, Key, Size};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;
use lumen_static_widgets::{Combobox, RichTextEditor, SearchField, TextArea};

/// A cluster per `char`, half an em apart, and a line per `'\n'` — what
/// fontdue reports, without a face.
pub struct Chars;

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

macro_rules! lone {
    ($ui:ident, $app:ident, $ty:ty) => {
        #[derive(StaticTree, Default)]
        #[widget(column, spacing = 0.0)]
        pub struct $ui {
            #[widget_id("w")]
            pub w: $ty,
        }
        #[derive(DeriveApp, Default)]
        pub struct $app {
            #[tree]
            pub ui: $ui,
        }
    };
}

lone!(InputUi, InputApp, TextInput);
lone!(AreaUi, AreaApp, TextArea);
lone!(SearchUi, SearchApp, SearchField);
lone!(ComboUi, ComboApp, Combobox);
lone!(EditorUi, EditorApp, RichTextEditor);

/// One widget under test: its name, the index of the node that edits,
/// and how to read its value.
pub trait Edit: App + Default + 'static {
    const NAME: &'static str;
    const EDITABLE: usize;
    /// Whether Home puts the caret at byte 0. `TextArea` has no Home
    /// (R10-F14, T45.6), so it and the editor built on it are sent there
    /// by a click; a `Combobox` cannot be, because its open list is an
    /// overlay and a press outside it only dismisses it.
    const HOME_KEY: bool = true;
    fn value(&self) -> &str;
}

impl Edit for InputApp {
    const NAME: &'static str = "TextInput";
    const EDITABLE: usize = 1;
    fn value(&self) -> &str {
        self.ui.w.as_str()
    }
}
impl Edit for AreaApp {
    const NAME: &'static str = "TextArea";
    const EDITABLE: usize = 1;
    const HOME_KEY: bool = false;
    fn value(&self) -> &str {
        self.ui.w.as_str()
    }
}
impl Edit for SearchApp {
    const NAME: &'static str = "SearchField";
    // The field is the SearchField's second part.
    const EDITABLE: usize = 3;
    fn value(&self) -> &str {
        self.ui.w.as_str()
    }
}
impl Edit for ComboApp {
    const NAME: &'static str = "Combobox";
    const EDITABLE: usize = 2;
    fn value(&self) -> &str {
        self.ui.w.trigger.as_str()
    }
}
impl Edit for EditorApp {
    const NAME: &'static str = "RichTextEditor";
    const EDITABLE: usize = 2;
    const HOME_KEY: bool = false;
    fn value(&self) -> &str {
        self.ui.w.input.as_str()
    }
}

/// A host with the editable node focused by a click at its left edge.
pub fn focused<A: Edit>() -> Host<A> {
    let mut h: Host<A> = Host::new(A::default(), 500, 400);
    h.pump_with(&mut Chars);
    h.pump_with(&mut Chars);
    let s = h.states[A::EDITABLE];
    assert!(s.size.w > 0.0, "setup: {} was not laid out", A::NAME);
    h.click(s.pos.x + s.pad.0 + 1.0, s.pos.y + s.pad.1 + 2.0);
    h.pump_with(&mut Chars);
    assert_eq!(h.input.focus, Some(A::EDITABLE as u32), "setup: the click did not focus {}", A::NAME);
    h
}

pub fn type_str<A: Edit>(h: &mut Host<A>, s: &str) {
    for ch in s.chars() {
        h.post(Event::Text { ch });
        h.pump_with(&mut Chars);
    }
    assert_eq!(h.app.value(), s, "setup: typing into {} did not land", A::NAME);
}

pub fn key<A: Edit>(h: &mut Host<A>, k: Key) {
    h.post(Event::Key { key: k, down: true });
    h.pump_with(&mut Chars);
}

/// Put the caret at byte 0: Home, or a click at the text's start.
/// Asserted, since everything after it measures from there.
pub fn home<A: Edit>(h: &mut Host<A>) -> usize {
    if A::HOME_KEY {
        key(h, Key::Home);
        return 0;
    }
    let s = h.states[A::EDITABLE];
    h.click(s.pos.x + s.pad.0 + 0.5, s.pos.y + s.pad.1 + 2.0);
    h.pump_with(&mut Chars);
    0
}

const CLUSTERS: [(&str, &str); 4] = [
    ("combining accent", "e\u{301}"),
    ("ZWJ family", "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}"),
    ("flag", "\u{1f1f3}\u{1f1f4}"),
    ("skin tone", "\u{1f44d}\u{1f3fd}"),
];

/// Backspace after `x` + a cluster, and Right-then-Delete over two
/// clusters and an `x`. Returns (cases, right).
fn run<A: Edit>() -> (usize, usize) {
    let mut right = 0;
    let mut cases = 0;
    for (name, g) in CLUSTERS {
        let mut h = focused::<A>();
        type_str(&mut h, &format!("x{g}"));
        key(&mut h, Key::Backspace);
        let left = h.app.value().chars().count();
        let ok = h.app.value() == "x";
        println!("{} Backspace after x+{name}: {:?}, {left} codepoints left (want 1) {}", A::NAME, h.app.value(), if ok { "ok" } else { "WRONG" });
        right += ok as usize;
        cases += 1;

        let mut h = focused::<A>();
        type_str(&mut h, &format!("{g}{g}x"));
        home(&mut h);
        key(&mut h, Key::Right);
        key(&mut h, Key::Delete);
        let want = format!("{g}x");
        let ok = h.app.value() == want;
        println!(
            "{} Right, Delete over {name}{name}x: {} codepoints left (want {}) {}",
            A::NAME,
            h.app.value().chars().count(),
            want.chars().count(),
            if ok { "ok" } else { "WRONG" }
        );
        right += ok as usize;
        cases += 1;
    }
    (cases, right)
}

#[test]
fn backspace_and_delete_remove_whole_graphemes_in_every_text_widget() {
    let mut total = (0, 0);
    for (c, r) in [run::<InputApp>(), run::<AreaApp>(), run::<SearchApp>(), run::<ComboApp>(), run::<EditorApp>()] {
        total.0 += c;
        total.1 += r;
    }
    println!("whole graphemes {} / {} (bound {})", total.1, total.0, total.0);
    assert_eq!(total.1, total.0, "{} of {} edits left part of a grapheme", total.0 - total.1, total.0);
}

/// **The accept direction**: ASCII and a precomposed `é` delete one
/// character, as they always did.
#[test]
fn a_single_codepoint_still_deletes_as_one() {
    for s in ["xa", "x\u{e9}"] {
        let mut h = focused::<InputApp>();
        type_str(&mut h, s);
        key(&mut h, Key::Backspace);
        println!("Backspace after {s:?}: {:?}", h.app.value());
        assert_eq!(h.app.value(), "x");
    }
}
