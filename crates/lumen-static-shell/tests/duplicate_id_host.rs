//! M46/T46.0 item 4, R2-F7: an id that names two nodes acts on both.
//!
//! `Inner { #[widget_id("leaf")] Label }` is used twice, so `"leaf"` is on
//! two nodes of the table. Styling reads an id as a set — a rule on
//! `Selector::Id("leaf")` restyles both — and until T46.2 the addressing
//! half did not: `Host::invalidate` and `Host::restyle_subtree` resolved
//! the id through `index_of_wid`, acted on the first match, and returned
//! `true`. Under the default `Boundaries<256>` a one-of-two mark is a
//! stale second instance, not a no-op.
//!
//! Three checks, each printing what it measured beside what it needs:
//!
//! 1. `invalidate("leaf")` leaves **both** instances dirty.
//! 2. After writing the second instance's text behind the host's back
//!    and invalidating by id, the frame's display list equals a fresh
//!    host's for the same state — so the stale-frame half of the defect
//!    is checked, not only the mark.
//! 3. `restyle_subtree(sheet, "leaf")` gives the new font size at both,
//!    and does not panic in a debug build (the witness used to blame the
//!    sheet for what was a duplicated id).
//!
//! **Accepts good input:** a unique id marks exactly its own subtree and
//! returns `true`; a missing id returns `false` and marks nothing.

use lumen_static::invalidate::Boundaries;
use lumen_static::prelude::*;
use lumen_static::style::{count_wid, subtree_range, Patch, Rule, Selector};
use lumen_static::{Cmd, Incremental, Widget};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Inner {
    #[widget_id("leaf")]
    leaf: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    x: Inner,
    y: Inner,
    #[widget_id("solo")]
    solo: Label<String>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

type H = Host<App, Incremental, Boundaries<256>>;

fn app(x: &str, y: &str) -> App {
    let mut a = App::default();
    a.ui.x.leaf.set(x.into());
    a.ui.y.leaf.set(y.into());
    a.ui.solo.set("solo".into());
    a
}

fn settled(a: App) -> H {
    let mut h: H = Host::new(a, 300, 100);
    h.pump();
    h.pump();
    h
}

fn leaves() -> Vec<usize> {
    let t = <Ui as Widget>::SUB_TABLE;
    let hits: Vec<usize> = (0..t.len()).filter(|&i| t[i].wid == "leaf").collect();
    assert_eq!(hits.len(), 2, "setup: `leaf` must be on two nodes");
    assert_eq!(count_wid(t, "leaf"), 2);
    hits
}

/// The frame as text: each command, with a text command's string in
/// place of its buffer range (which moves whenever anything before it
/// changes length).
fn frame(h: &H) -> Vec<String> {
    h.list
        .cmds
        .iter()
        .map(|c| match c {
            Cmd::Text { rect, text, size, .. } => format!("Text {rect:?} {size} {:?}", h.list.str_of(*text)),
            other => format!("{other:?}"),
        })
        .collect()
}

#[test]
#[ignore = "red until T46.2: R2-F7, Host::invalidate marks the first of two"]
fn invalidate_marks_every_node_carrying_the_id() {
    let hits = leaves();
    let mut h = settled(app("a", "b"));
    assert!(h.invalidate("leaf"), "the id is in the tree");
    let c = h.changed();
    let dirty: Vec<(usize, bool)> = hits.iter().map(|&i| (i, !c.is_clean(i..i + 1))).collect();
    println!("invalidate(\"leaf\"): {dirty:?} dirty / bound: both");
    for (i, d) in dirty {
        assert!(d, "instance {i} clean=true after invalidate(\"leaf\")");
    }
}

#[test]
#[ignore = "red until T46.2: R2-F7, the second instance keeps its stale frame"]
fn invalidate_by_a_duplicated_id_repaints_both_instances() {
    let _ = leaves();
    let mut h = settled(app("a", "b"));
    // Behind the host's back, as an application writing a widget
    // directly does, then addressed by the id both instances carry.
    h.app.ui.y.leaf.set("a much longer second label".into());
    assert!(h.invalidate("leaf"));
    h.pump();
    let got = frame(&h);
    let want = frame(&settled(app("a", "a much longer second label")));
    let differing = got.iter().zip(want.iter()).filter(|(a, b)| a != b).count()
        + got.len().abs_diff(want.len());
    println!("frame after invalidate: {differing} commands differ from a fresh host's / bound 0");
    for (g, w) in got.iter().zip(want.iter()).filter(|(a, b)| a != b) {
        println!("  got  {g}\n  want {w}");
    }
    assert!(
        got.iter().any(|l| l.contains("a much longer second label")),
        "setup: the second label's new text must be in the frame"
    );
    assert_eq!(differing, 0, "the second instance kept a stale frame");
}

#[test]
#[ignore = "red until T46.2: R2-F7, restyle_subtree restyles the first of two (and panics in debug)"]
fn restyle_subtree_restyles_every_node_carrying_the_id() {
    let hits = leaves();
    let mut h = settled(app("a", "b"));
    let sheet = vec![Rule::new(Selector::Id("leaf"), Patch::new().font_size(40.0))];
    assert!(h.restyle_subtree(&sheet, "leaf"));
    let sizes: Vec<(usize, f32)> = hits.iter().map(|&i| (i, h.styles[i].font_size)).collect();
    let c = h.changed();
    let dirty: Vec<bool> = hits.iter().map(|&i| !c.is_clean(i..i + 1)).collect();
    println!("restyle_subtree: font_size {sizes:?} / bound: 40 at both; dirty {dirty:?}");
    for (i, s) in sizes {
        assert_eq!(s, 40.0, "instance {i} kept font_size {s}");
    }
    assert!(dirty.iter().all(|&d| d), "both restyled subtrees must be marked");
}

/// Accepts good input, unchanged by T46.2: a unique id marks its own
/// subtree and nothing else, and a missing id is `false`.
#[test]
fn a_unique_id_marks_one_subtree_and_a_missing_one_marks_none() {
    let t = <Ui as Widget>::SUB_TABLE;
    let hits = leaves();
    let solo = (0..t.len()).find(|&i| t[i].wid == "solo").unwrap();
    let mut h = settled(app("a", "b"));
    assert!(h.invalidate("solo"));
    let c = h.changed();
    let range = subtree_range(t, solo);
    println!("invalidate(\"solo\"): solo {range:?} clean={}", c.is_clean(range.clone()));
    assert!(!c.is_clean(range));
    for i in hits {
        assert!(c.is_clean(i..i + 1), "invalidate(\"solo\") marked leaf {i}");
    }

    let mut h = settled(app("a", "b"));
    assert!(!h.invalidate("nope"), "a missing id must be reported");
    assert!(!h.restyle_subtree(&[], "nope"), "a missing id must be reported");
}
