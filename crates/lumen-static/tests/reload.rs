#![allow(clippy::field_reassign_with_default)] // a fresh build, then what the test stands in for
//! Step 4 correctness, in-process: keep/fresh/reset semantics, nested
//! widget subtrees, Vec arity from saved data, per-field fallback on
//! schema change (§12.2 generalized), and the no-code-pointers property.

use lumen_static_widgets::ops::{Fixed, Label, Spacer};
use lumen_static::reload::{take_log, Reloadable, Value};
use lumen_static::repeat::Repeat;
use lumen_static::{Reloadable as DeriveReloadable, StaticTree};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Default, Clone, Copy)]
enum Mode {
    #[default]
    Basic,
    Fancy,
}

#[derive(StaticTree, DeriveReloadable, Default)]
#[widget(row, spacing = 2.0)]
struct Header {
    title: Label,
    gap: Spacer,
    badge: Fixed,
}

#[derive(DeriveReloadable, Default)]
struct Session {
    header: Header, // nested widget subtree
    #[reload(keep)]
    counter: u32,
    #[reload(keep)]
    mode: Mode,
    #[reload(keep)]
    names: Vec<String>, // plain serde data, arity included
    #[reload(fresh)]
    theme: &'static str, // "code-adjacent": new build's value must win
    #[reload(reset)]
    cache_hits: u64,
}

fn user_session() -> Session {
    let mut s = Session::default();
    s.header.title.set("renamed by user");
    s.header.badge.w = 99.0;
    s.counter = 3;
    s.mode = Mode::Fancy;
    s.names = vec!["a".into(), "b".into()];
    s.theme = "old-theme";
    s.cache_hits = 41;
    s
}

#[test]
fn keep_fresh_reset_semantics() {
    let old = user_session();
    let bytes = serde_json::to_vec(&old.save()).unwrap(); // pointer-free by construction
    drop(old); // §7 sequence: old tree dropped before the "new code" takes over

    let saved: Value = serde_json::from_slice(&bytes).unwrap();
    let mut fresh = Session::default();
    fresh.theme = "new-theme"; // stand-in for the new dylib's Default
    fresh.restore(&saved);

    assert_eq!(fresh.counter, 3, "keep");
    assert_eq!(fresh.mode, Mode::Fancy, "keep enum");
    assert_eq!(fresh.names, vec!["a".to_owned(), "b".to_owned()], "keep vec");
    assert_eq!(fresh.header.title.as_str(), "renamed by user", "nested keep");
    assert_eq!(fresh.header.badge.w, 99.0, "nested keep leaf");
    assert_eq!(fresh.theme, "new-theme", "fresh: new code wins");
    assert_eq!(fresh.cache_hits, 0, "reset: recomputed from zero");
    assert!(take_log().is_empty(), "clean restore logs nothing");
}

#[test]
fn incompatible_fields_fall_back_per_field_and_log() {
    // Simulate a schema change: the saved state has a removed enum
    // variant, a type change, and lacks a newly added field entirely.
    let saved: Value = serde_json::json!({
        "counter": "not a number",          // type change
        "mode": "Baroque",                  // removed/unknown variant
        // "names" missing: field added in the new build
        "header": { "title": "kept title", "badge": { "w": 7.0, "h": 7.0 } },
    });
    let mut s = Session::default();
    s.counter = 11;
    s.restore(&saved);

    // Every incompatibility fell back alone; the rest restored.
    assert_eq!(s.counter, 11, "type-changed field keeps fresh value");
    assert_eq!(s.mode, Mode::Basic, "unknown variant keeps fresh value (§12.2)");
    assert!(s.names.is_empty(), "missing field keeps fresh value");
    assert_eq!(s.header.title.as_str(), "kept title", "unaffected fields still restore");

    let log = take_log();
    assert!(log.iter().any(|l| l.contains("Session.counter")), "log: {log:?}");
    assert!(log.iter().any(|l| l.contains("Session.mode")), "log: {log:?}");
    assert!(log.iter().any(|l| l.contains("Session.names")), "log: {log:?}");
}

#[test]
fn vec_of_widgets_arity_comes_from_saved_data() {
    let mut rows: Vec<Label> = Vec::new();
    assert!(rows.is_empty(), "an empty list is the zero-row list");
    let saved = Value::Array(
        (0..47).map(|i| Value::String(format!("row {i}"))).collect(),
    );
    rows.restore(&saved);
    assert_eq!(rows.len(), 47, "…and restore gives back all 47 (§7)");
    assert_eq!(rows[46].as_str(), "row 46");
}

#[test]
fn repeat_keeps_logical_state_and_drops_the_window() {
    let mut old = Repeat::<Header>::new(10_000, 24.0, lumen_static::repeat::noop_bind);
    old.scroll = 612.0;
    let saved = old.save();

    let mut fresh = Repeat::<Header>::default();
    fresh.restore(&saved);
    assert_eq!(fresh.count, 10_000, "keep: data size");
    assert_eq!(fresh.scroll, 612.0, "keep: the user's place in the list");
    assert_eq!(fresh.item_extent, 24.0, "keep");
    assert_eq!(fresh.window_len(), 0, "window is disposable (§12.4 decision)");
}

#[test]
fn saved_state_is_pointer_free_json() {
    // The whole saved form round-trips through text: nothing in it can be
    // a vtable or fn pointer. (A `#[reload(keep)]` fn pointer would not
    // compile — no `Serialize` impl — which is the stronger guarantee.)
    let s = user_session();
    let text = serde_json::to_string(&s.save()).unwrap();
    let back: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(back, s.save());
}
