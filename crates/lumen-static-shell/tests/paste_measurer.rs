//! **A paste is laid out with the caller's measurer** — M41/T41.0, for
//! R2-F1 and R10-F4 (`docs/plan-boundaries-repair.md` §1.2, §2.4).
//! Promotes the review's scratch `r2_paste.rs`.
//!
//! `Host::paste` pumps with the `BlockMetrics` stand-in for every
//! 64-character chunk and once at the end, although the window that
//! calls it owns the real measurer. A `TextInput`'s shaped memo is keyed
//! on the text's length, size, weight and limit — not on the measurer —
//! so the next real frame is served the stand-in's box and no clusters.
//! Under `Boundaries` it is worse: nothing marks the field after the
//! paste's own pumps, so the real frame does not lay it out at all.
//!
//! Since M41/T41.5 a caller with a real stack pastes through
//! `Host::paste_with`, and `Session::paste` does.
//!
//! Each case pastes into one host and types the same text into another,
//! then pumps both with a proportional measurer, under both strategies.
//! Typing is the control: it is the path `paste` claims to be.

use lumen_static::cx::{Cluster, GlyphSink, TextMeasure};
use lumen_static::invalidate::{Everything, Invalidation};
use lumen_static::prelude::*;
use lumen_static::retain::Incremental;
use lumen_static::{Cmd, Event, Size};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

/// Proportional, with clusters: an `m` is wider than the stand-in's
/// half-em and an `i` narrower, so a box measured by the wrong stack is
/// a different box.
struct Prop;

fn advance(c: char, size: f32) -> f32 {
    size * match c {
        'i' => 0.25,
        'm' => 0.9,
        _ => 0.5,
    }
}

impl TextMeasure for Prop {
    fn measure(&mut self, t: &str, size: f32, _max_w: f32) -> Size {
        Size { w: t.chars().map(|c| advance(c, size)).sum(), h: size }
    }

    fn shape(&mut self, t: &str, size: f32, weight: f32, max_w: f32, sink: &mut dyn GlyphSink) -> Size {
        let mut x = 0.0;
        for (b, c) in t.char_indices() {
            sink.cluster(Cluster { byte: b as u32, x, line: 0 });
            x += advance(c, size);
        }
        sink.cluster(Cluster { byte: t.len() as u32, x, line: 0 });
        self.measure_weighted(t, size, weight, max_w)
    }
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
struct Ui {
    #[widget_id("field")]
    field: TextInput,
    #[widget_id("after")]
    after: Label<String>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

const FIELD: usize = 1;

fn host<I: Invalidation>() -> Host<App, Incremental, I> {
    let mut a = App::default();
    a.ui.after.set("after".into());
    let mut h: Host<App, Incremental, I> = Host::new(a, 400, 60);
    h.pump_with(&mut Prop);
    h.post(Event::Key { key: lumen_static::Key::Tab, down: true });
    h.pump_with(&mut Prop);
    assert_eq!(h.input.focus, Some(FIELD as u32), "setup: Tab did not focus the field");
    h
}

/// What a person sees of the field: its width, the x its text is drawn
/// at (the scroll), and where a click at `click_x` puts the caret.
#[derive(Debug, PartialEq)]
struct Seen {
    w: f32,
    text_x: Option<f32>,
    caret_after_click: usize,
}

fn seen<I: Invalidation>(h: &mut Host<App, Incremental, I>, click_x: f32) -> Seen {
    let s = h.states[FIELD];
    let text = h.app.ui.field.as_str().to_owned();
    let text_x = h.list.cmds.iter().find_map(|c| match c {
        Cmd::Text { rect, text: t, .. } if h.list.str_of(*t) == text => Some(rect[0]),
        _ => None,
    });
    let w = s.size.w;
    h.click(s.pos.x + click_x, s.pos.y + s.size.h / 2.0);
    h.pump_with(&mut Prop);
    Seen { w, text_x, caret_after_click: h.app.ui.field.caret() }
}

/// Paste into one host and type into another, then two real frames.
fn case<I: Invalidation>(strategy: &str, text: &str, click_x: f32) -> (Seen, Seen) {
    let mut pasted: Host<App, Incremental, I> = host();
    // As a window's session does: with the measurer its frames use.
    let n = pasted.paste_with(text, &mut Prop);
    assert_eq!(n, text.chars().count(), "setup: the paste was not delivered whole");
    pasted.pump_with(&mut Prop);
    pasted.pump_with(&mut Prop);

    let mut typed: Host<App, Incremental, I> = host();
    for ch in text.chars() {
        typed.post(Event::Text { ch });
        typed.pump_with(&mut Prop);
    }
    typed.pump_with(&mut Prop);

    assert_eq!(pasted.app.ui.field.as_str(), text, "setup: the paste did not land");
    assert_eq!(typed.app.ui.field.as_str(), text, "setup: the typing did not land");
    let (p, t) = (seen(&mut pasted, click_x), seen(&mut typed, click_x));
    println!(
        "{strategy} {}-char: paste w={} x={:?} caret@{click_x}={} | typed w={} x={:?} caret@{click_x}={} | bound: equal -> {}",
        text.chars().count(),
        p.w,
        p.text_x,
        p.caret_after_click,
        t.w,
        t.text_x,
        t.caret_after_click,
        p == t
    );
    (p, t)
}

fn both(text: &str, click_x: f32) {
    let (pe, te) = case::<Everything>("Everything", text, click_x);
    let (pb, tb) = case::<lumen_static::Boundaries<256>>("Boundaries<256>", text, click_x);
    assert_eq!(te, tb, "the typed control differs between strategies, so the comparison is not about paste");
    assert_eq!(pe, te, "Everything: a paste laid out differently from typing the same text");
    assert_eq!(pb, tb, "Boundaries<256>: a paste laid out differently from typing the same text");
}

#[test]
fn a_short_paste_measures_like_typing() {
    both("mmmmmmmm", 30.0);
}

/// Crosses the 64-character chunk pump, so the stand-in pumps twice.
#[test]
fn a_paste_longer_than_the_queue_measures_like_typing() {
    both(&"mi".repeat(50), 200.0);
}

/// R10-F4: a click after a paste lands on the byte typing would give.
#[test]
fn a_click_after_a_paste_lands_where_it_would_after_typing() {
    both("the quick brown fox", 84.5);
}

/// **The window's path**: `Session::paste` on a `Local` session, which
/// owns the system face. The `Host` cases above pass the measurer by
/// hand; this is the call a window makes, and the one T41.5 changed.
#[cfg(feature = "truetype")]
#[test]
fn a_session_paste_measures_with_the_sessions_face() {
    use lumen_static_shell::session::{Local, Crashing, Session};
    let session = || {
        let mut s: Local<App, Crashing> = Local::from_host(Host::new(App::default(), 400, 60), "", lumen_static::Color(255, 255, 255, 255), Crashing);
        s.frame(0.0, 400, 60);
        s.post(Event::Key { key: lumen_static::Key::Tab, down: true });
        s.frame(0.0, 400, 60);
        assert_eq!(s.host.input.focus, Some(FIELD as u32), "setup: Tab did not focus the field");
        s
    };
    let text = "mmmmmmmm";
    let mut pasted = session();
    pasted.paste(text);
    pasted.frame(0.0, 400, 60);
    let mut typed = session();
    for ch in text.chars() {
        typed.post(Event::Text { ch });
        typed.frame(0.0, 400, 60);
    }
    typed.frame(0.0, 400, 60);
    assert_eq!(pasted.host.app.ui.field.as_str(), text, "setup: the paste did not land");
    let (pw, tw) = (pasted.host.states[FIELD].size.w, typed.host.states[FIELD].size.w);
    // The stand-in's box: eight half-em glyphs at the default 16 px, plus
    // 8 px of padding a side — what a paste measured with `BlockMetrics`
    // is (80, the Host cases' red figure).
    let stand_in = 8.0 * 16.0 * 0.5 + 16.0;
    println!("session 8-char: paste w={pw} | typed w={tw} | stand-in w={stand_in} | bound: equal");
    assert_ne!(tw, stand_in, "setup: the session has no face of its own, so this case cannot fail");
    assert_eq!(pw, tw, "a session's paste laid out differently from typing the same text");
}
