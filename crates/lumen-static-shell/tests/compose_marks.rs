//! **A composition step is marked** — M41/T41.0's gate, closed by
//! T41.1, for R1-F2 and R10-F9 (`docs/plan-boundaries-repair.md` §1.1).
//!
//! The window routes `Ime::Preedit`, `Ime::Commit` and `Ime::Disabled`
//! to [`Host::compose`], and the redraw that follows pumps an empty
//! queue: nothing a handler did, so nothing the pump's own mark sites
//! see. Until T41.1 `compose` marked nothing either, so under the
//! `Boundaries` default a field's `layout` — which computes the scroll
//! offset, the content size and the clusters the caret is placed from —
//! was skipped, and the commit was painted against last frame's layout.
//! `Everything` lays every node out anyway and hid it, as it hid M25's
//! hover and M34's tick.
//!
//! In `tick_marks.rs`'s shape: the same session under both strategies,
//! compared frame by frame. **Each case asserts that it can fail**:
//! `compose` answered `true`, focus is on the field meant, and the
//! `Everything` arm's text really scrolled (its x is left of the
//! field's padding). The short commit is the control: it scrolls
//! nothing and still changes the content-sized field's box, so it shows
//! the mark is not specific to a scroll.
//!
//! The routing test is R10-F9's: before this file, nothing called
//! `Host::compose` at all — `ime.rs` calls the widgets' inherent
//! methods — so both `Node::compose` impls could be no-ops with every
//! test green (the review's injection `125aa35`).

use lumen_static::event::Composition::{Cancel, Commit, Preedit};
use lumen_static::invalidate::{Everything, Invalidation};
use lumen_static::prelude::*;
use lumen_static::retain::Incremental;
use lumen_static::{Cmd, DisplayList, Event, Key};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
struct Plain {
    #[widget_id("field")]
    field: TextInput,
    #[widget_id("after")]
    after: Label<String>,
}

#[derive(DeriveApp, Default)]
struct PlainApp {
    #[tree]
    ui: Plain,
}

/// The composite: a `SearchField`'s editable part is a node *inside*
/// the widget, so focus — and the mark — land on a part, not on the
/// widget's own root.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
struct Search {
    #[widget_id("search")]
    search: SearchField,
    #[widget_id("after")]
    after: Label<String>,
}

#[derive(DeriveApp, Default)]
struct SearchApp {
    #[tree]
    ui: Search,
}

/// Narrow enough that a 24-character commit is wider than the field
/// can grow, so the field both grows to its limit and scrolls.
const W: u32 = 140;

/// The x of the text run whose content is exactly `s`.
fn text_x(list: &DisplayList, s: &str) -> Option<f32> {
    list.cmds.iter().find_map(|c| match c {
        Cmd::Text { rect, text, .. } if list.str_of(*text) == s => Some(rect[0]),
        _ => None,
    })
}

/// A case: which app, where its editable node is, and how to read it.
trait Case: App + Default + 'static {
    /// The table index of the node that edits text.
    fn editable() -> usize;
    fn text(&self) -> &str;
    fn prepare(&mut self) {}
}

impl Case for PlainApp {
    fn editable() -> usize {
        lumen_static::style::index_of_wid(<Plain as Widget>::SUB_TABLE, "field").unwrap()
    }
    fn text(&self) -> &str {
        self.ui.field.as_str()
    }
}

impl Case for SearchApp {
    fn editable() -> usize {
        let t = <Search as Widget>::SUB_TABLE;
        let root = lumen_static::style::index_of_wid(t, "search").unwrap();
        // The field is the SearchField's second part.
        root + 2
    }
    fn text(&self) -> &str {
        self.ui.search.as_str()
    }
}

/// Built, laid out twice, and the field focused **by a click**, with
/// focus asserted — a compose with nothing focused answers `false` and
/// would make every comparison below vacuous.
fn focused<A: Case, I: Invalidation>() -> Host<A, Incremental, I> {
    let mut a = A::default();
    a.prepare();
    let mut h: Host<A, Incremental, I> = Host::new(a, W, 60);
    h.pump();
    h.pump();
    let i = A::editable();
    let s = h.states[i];
    assert!(s.size.w > 0.0, "setup: the editable node was not laid out: {s:?}");
    h.click(s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    h.pump();
    assert_eq!(h.input.focus, Some(i as u32), "setup: the click did not focus the field");
    h.pump();
    h
}

/// One frame of one arm: the list hash, the field's text x, its width,
/// and the node's left padding edge.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Frame {
    hash: u64,
    x: Option<f32>,
    w: f32,
}

fn frame<A: Case, I: Invalidation>(h: &Host<A, Incremental, I>) -> Frame {
    let i = A::editable();
    Frame { hash: h.list.hash_from(0), x: text_x(&h.list, h.app.text()), w: h.states[i].size.w }
}

/// The left edge of the field's text area: where unscrolled text starts.
fn pad_edge<A: Case, I: Invalidation>(h: &Host<A, Incremental, I>) -> f32 {
    let s = h.states[A::editable()];
    s.pos.x + s.pad.0
}

/// Run `steps` against both strategies, pumping `frames` times after
/// each, and compare every frame. Returns how many frames differed, and
/// prints each one beside its bound.
fn compare<A: Case>(name: &str, steps: &[lumen_static::event::Composition<'_>], must_scroll: bool) {
    let mut e: Host<A, Incremental, Everything> = focused();
    let mut b: Host<A> = focused();
    let mut scrolled = false;
    let mut differing = 0;
    for (k, c) in steps.iter().enumerate() {
        let (ce, cb) = (e.compose(*c), b.compose(*c));
        assert!(ce && cb, "{name} step {k} ({c:?}): compose answered {ce}/{cb}, so nothing changed");
        assert_eq!(e.app.text(), b.app.text(), "{name} step {k}: the buffers differ");
        for f in 1..=3 {
            e.pump();
            b.pump();
            let (fe, fb) = (frame(&e), frame(&b));
            let edge = pad_edge(&e);
            if fe.x.is_some_and(|x| x < edge) {
                scrolled = true;
            }
            let same = fe == fb;
            println!(
                "{name} step {k} {c:?} frame {f}: Everything x={:?} w={} {:#x} | Boundaries<256> x={:?} w={} {:#x} | pad edge {edge} | bound: equal -> {same}",
                fe.x, fe.w, fe.hash, fb.x, fb.w, fb.hash
            );
            if !same {
                differing += 1;
            }
        }
    }
    if must_scroll {
        assert!(scrolled, "{name}: Everything never scrolled the text, so this case cannot fail");
    }
    assert_eq!(differing, 0, "{name}: {differing} frames differ between Everything and Boundaries<256>");
}

const LONG: &str = "abcdefghijklmnopqrstuvwx";

#[test]
fn a_commit_that_scrolls_the_field_draws_the_same_frame_under_both_strategies() {
    compare::<PlainApp>("plain", &[Commit(LONG), Preedit("yz"), Cancel], true);
}

#[test]
fn a_commit_into_a_composite_draws_the_same_frame_under_both_strategies() {
    compare::<SearchApp>("search", &[Commit(LONG), Preedit("yz"), Cancel], true);
}

/// **The control**: a commit short enough that nothing scrolls. It
/// proves the comparison is not simply refusing every compose.
#[test]
fn a_short_commit_draws_the_same_frame_under_both_strategies() {
    compare::<PlainApp>("short", &[Commit("ab")], false);
}

/// **R10-F9: composition routed through the host**, the way the window
/// routes it — focus by Tab, then a preedit and a commit through
/// [`Host::compose`], never the widget's inherent method.
#[test]
fn composition_reaches_the_focused_field_through_the_host() {
    let mut h: Host<PlainApp> = Host::new(PlainApp::default(), W, 60);
    h.pump();
    assert!(!h.compose(Commit("x")), "nothing focused: compose must answer false");
    assert_eq!(h.app.ui.field.as_str(), "", "and change nothing");
    // **The accept direction** (T41.1): a compose that changed nothing
    // marks nothing, so the frame after it is still.
    h.pump();
    let m = h.last_marks();
    println!("after a compose with nothing focused: {m:?} (bound: nothing marked)");
    assert!(!m.all && m.ranges == 0, "a compose that changed nothing marked {m:?}");
    h.post(Event::Key { key: Key::Tab, down: true });
    h.pump();
    assert_eq!(h.input.focus, Some(PlainApp::editable() as u32), "setup: Tab did not focus the field");
    assert!(h.compose(Preedit("ni")), "a preedit into a focused field is a change");
    assert_eq!(h.app.ui.field.as_str(), "ni");
    assert_eq!(h.app.ui.field.preedit(), Some(0..2));
    assert!(h.compose(Commit("你")), "a commit is a change");
    assert_eq!(h.app.ui.field.as_str(), "你");
    assert_eq!(h.app.ui.field.preedit(), None);
    // And one that changed something marks precisely: the field's own
    // subtree, not the whole tree.
    h.pump();
    let m = h.last_marks();
    println!("after a commit: {m:?} (bound: one range, not all)");
    assert!(!m.all && m.ranges == 1, "a commit marked {m:?}, not the field alone");
    assert!(!h.compose(Cancel), "nothing composing: a cancel changes nothing");
    println!("routed: {:?}", h.app.ui.field.as_str());
}

/// A number field, alone: `NumberInput` owns a `TextInput` and forwards
/// every `Node` method to it at the same index — every one but `compose`.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
struct Number {
    #[widget_id("number")]
    number: NumberInput,
}

#[derive(DeriveApp, Default)]
struct NumberApp {
    #[tree]
    ui: Number,
}

/// **R10-F15: a number field takes composed digits and refuses composed
/// letters** — M45/T45.0 item 12. `NumberInput` did not override
/// `compose`, so the provided `false` refused every commit, digits
/// included, and an IME user could not enter a number at all.
#[test]
fn a_number_field_takes_composed_digits_and_refuses_letters() {
    let mut h: Host<NumberApp> = Host::new(NumberApp::default(), W, 60);
    h.pump();
    h.post(Event::Key { key: Key::Tab, down: true });
    h.pump();
    assert_eq!(h.input.focus, Some(1), "setup: Tab did not focus the number field");
    let before = h.app.ui.number.as_str().to_owned();
    let digits = h.compose(Commit("12"));
    h.pump();
    let after = h.app.ui.number.as_str().to_owned();
    let letters = h.compose(Commit("ab"));
    h.pump();
    let last = h.app.ui.number.as_str().to_owned();
    println!("commit \"12\": accepted {digits}, {before:?} -> {after:?} (want accepted, containing \"12\")");
    println!("commit \"ab\": accepted {letters}, {after:?} -> {last:?} (want refused, unchanged)");
    assert!(digits && after.contains("12"), "a composed number was refused");
    assert!(!letters && last == after, "composed letters reached a number field");
}
