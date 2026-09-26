//! **Every text widget takes a paste as one edit** — M45/T45.7
//! (`docs/plan-text-correctness.md` §2.2 C).
//!
//! The paste hook is a provided method on `Node`, and a wrapper that does
//! not forward it is the `NumberInput` shape R10-F13 and R10-F15 found
//! twice. So every text widget, composites included, is pasted into here,
//! alone in its tree, under both invalidations: the value it keeps, the
//! count, and the same frame under `Everything` and `Boundaries<256>`.
//!
//! And the fallback: a widget with no paste of its own gets the string as
//! typing, each line ending as `Key::Enter`.
//!
//! **Each widget sits in a row before a label, and the measurer reports
//! clusters.** The first version put each alone under the `BlockMetrics`
//! stand-in, and an injection that stopped marking the paste left it
//! green: a `TextArea` is as wide as its limits, the stand-in reports no
//! clusters, and paint reads the text straight from the buffer — so a
//! skipped layout drew the same pixels. Here a content-sized field moves
//! the label after it, and every caret is placed from shaped clusters.

use core::cell::RefCell;

use lumen_static::desc::{NodeDesc, NodeState, NO_PARENT};
use lumen_static::display::Painter;
use lumen_static::event::{EventCtx, Handlers, NodeSel};
use lumen_static::invalidate::{Everything, Invalidation};
use lumen_static::ops::{record_painted, resolve_self};
use lumen_static::prelude::*;
use lumen_static::retain::Incremental;
use lumen_static::style::{Seed, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{leaf_common, Constraints, Event, Key, LayoutCx, Size};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;
use lumen_static_widgets::{Combobox, NumberInput, RichTextEditor, SearchField, TextArea};

/// A cluster per `char`, half an em apart, a line per `'\n'`.
struct Chars;

impl lumen_static::TextMeasure for Chars {
    fn measure(&mut self, t: &str, size: f32, max_w: f32) -> Size {
        self.shape(t, size, 400.0, max_w, &mut lumen_static::NoSink)
    }
    fn shape(&mut self, t: &str, size: f32, _w: f32, _max_w: f32, sink: &mut dyn lumen_static::cx::GlyphSink) -> Size {
        use lumen_static::cx::Cluster;
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
        #[widget(row, spacing = 4.0)]
        pub struct $ui {
            #[widget_id("w")]
            pub w: $ty,
            #[widget_id("after")]
            pub after: Label<&'static str>,
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
lone!(NumberUi, NumberApp, NumberInput);
lone!(KeysUi, KeysApp, Keys);

trait Case: App + Default + 'static {
    const NAME: &'static str;
    const EDITABLE: usize;
    fn value(&self) -> String;
}

impl Case for InputApp {
    const NAME: &'static str = "TextInput";
    const EDITABLE: usize = 1;
    fn value(&self) -> String {
        self.ui.w.as_str().into()
    }
}
impl Case for AreaApp {
    const NAME: &'static str = "TextArea";
    const EDITABLE: usize = 1;
    fn value(&self) -> String {
        self.ui.w.as_str().into()
    }
}
impl Case for SearchApp {
    const NAME: &'static str = "SearchField";
    const EDITABLE: usize = 3;
    fn value(&self) -> String {
        self.ui.w.as_str().into()
    }
}
impl Case for ComboApp {
    const NAME: &'static str = "Combobox";
    const EDITABLE: usize = 2;
    fn value(&self) -> String {
        self.ui.w.trigger.as_str().into()
    }
}
impl Case for EditorApp {
    const NAME: &'static str = "RichTextEditor";
    const EDITABLE: usize = 2;
    fn value(&self) -> String {
        self.ui.w.input.as_str().into()
    }
}
impl Case for NumberApp {
    const NAME: &'static str = "NumberInput";
    const EDITABLE: usize = 1;
    fn value(&self) -> String {
        self.ui.w.as_str().into()
    }
}
impl Case for KeysApp {
    const NAME: &'static str = "a widget with no paste";
    const EDITABLE: usize = 1;
    fn value(&self) -> String {
        self.ui.w.got.borrow().clone()
    }
}

/// A focusable leaf that edits only through `on_event`: it records each
/// `Text` it takes, and an Enter as `'\n'`, and has no paste of its own.
#[derive(Default)]
pub struct Keys {
    got: RefCell<String>,
}

impl Widget for Keys {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::FOCUSABLE)];
    const KIND: &'static str = "paste-keys";
    const SEEDS: &'static [Seed] = &[Seed::unlinted(Self::KIND, Self::type_defaults)];
}

impl Node for Keys {
    fn layout(&mut self, _cx: &mut LayoutCx, limits: Constraints, style: &[Style], st: &mut [NodeState], i: usize) -> Size {
        let size = resolve_self(limits, &style[i], Size { w: 40.0, h: 20.0 });
        record_painted(st, i, limits, size, &style[i])
    }
    fn paint(&self, _st: &[NodeState], _i: usize, _out: &mut dyn Painter) {}
    fn on_event(&mut self, ev: &Event, _cx: &EventCtx) -> bool {
        match ev {
            Event::Text { ch } if !ch.is_control() => self.got.borrow_mut().push(*ch),
            Event::Key { key: Key::Enter, down: true } => self.got.borrow_mut().push('\n'),
            _ => return false,
        }
        true
    }
    leaf_common!(Keys, NodeDesc::FOCUSABLE);
}

impl<S> Handlers<S> for Keys {
    fn handler(&self, _sel: NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        None
    }
}

fn focused<A: Case, I: Invalidation>() -> Host<A, Incremental, I> {
    let mut h: Host<A, Incremental, I> = Host::new(A::default(), 500, 400);
    h.pump_with(&mut Chars);
    h.pump_with(&mut Chars);
    // Focused by a click, so the node's own state says so and its caret
    // is painted: a caret placed from stale clusters is what a skipped
    // layout of a fixed-size editor shows.
    let s = h.states[A::EDITABLE];
    h.click(s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    h.pump_with(&mut Chars);
    assert_eq!(h.input.focus, Some(A::EDITABLE as u32), "setup: the click did not focus {}", A::NAME);
    h
}

/// Paste `text` under both strategies: (value, count, frames equal).
fn paste<A: Case>(text: &str) -> (String, usize, bool) {
    let mut e: Host<A, Incremental, Everything> = focused();
    let mut b: Host<A> = focused();
    let (ne, nb) = (e.paste_with(text, &mut Chars), b.paste_with(text, &mut Chars));
    for _ in 0..2 {
        e.pump_with(&mut Chars);
        b.pump_with(&mut Chars);
    }
    assert_eq!(ne, nb, "{}: the count differs between strategies", A::NAME);
    assert_eq!(e.app.value(), b.app.value(), "{}: the value differs between strategies", A::NAME);
    (b.app.value(), nb, e.list.hash_from(0) == b.list.hash_from(0))
}

/// `kept` is the count the paste must answer: the characters of `want`
/// it added, which is all of `want` for a widget that starts empty.
fn check<A: Case>(text: &str, want: &str, kept: usize, bad: &mut usize) {
    let (v, n, same) = paste::<A>(text);
    let ok = v == want && n == kept && same;
    println!(
        "{:<24} pasted {text:?} -> {v:?} (want {want:?}), count {n} (want {kept}), frames equal {same} {}",
        A::NAME,
        if ok { "ok" } else { "WRONG" }
    );
    *bad += !ok as usize;
}

#[test]
fn every_text_widget_takes_a_paste_as_one_edit() {
    let mut bad = 0;
    check::<InputApp>("ab\ncd", "abcd", 4, &mut bad);
    check::<AreaApp>("ab\r\ncd\ref", "ab\ncd\nef", 8, &mut bad);
    check::<SearchApp>("find\tme", "findme", 6, &mut bad);
    check::<ComboApp>("app\nle", "apple", 5, &mut bad);
    check::<EditorApp>("# Title\r\n**bold**", "# Title\n**bold**", 16, &mut bad);
    // A number field keeps what its number rule allows, after its own
    // "0.00": a paste appends at the caret, as typing does.
    check::<NumberApp>("1a2.5", "0.0012.5", 4, &mut bad);
    check::<KeysApp>("ab\ncd\r\nef", "ab\ncd\nef", 8, &mut bad);
    println!("wrong {bad} (bound 0)");
    assert_eq!(bad, 0, "{bad} widgets did not take a paste as one edit");
}

/// Nothing focused keeps nothing.
#[test]
fn a_paste_with_nothing_focused_keeps_nothing() {
    let mut h: Host<InputApp> = Host::new(InputApp::default(), 500, 400);
    h.pump();
    let n = h.paste("hello");
    println!("nothing focused: kept {n}, field {:?}", h.app.ui.w.as_str());
    assert_eq!((n, h.app.ui.w.as_str()), (0, ""));
}
