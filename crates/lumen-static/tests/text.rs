//! M3: text storage — generic over the profile's choice, owned so the
//! measurement cache can be invalidated, and reload-clean for owning
//! storage.

use lumen_static::ops::{Fixed, Label};
use lumen_static::reload::{Reloadable, Value};
use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};
use std::borrow::Cow;

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 200.0 };

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Bar {
    a: Label<String>,
    b: Label<Cow<'static, str>>,
    c: Label, // the default: &'static str, the no_std-friendly choice
    pad: Fixed,
}

#[test]
fn storage_is_the_profiles_choice() {
    let mut bar = Bar::default();
    bar.a.set("owned".to_owned());
    bar.b.set(Cow::Borrowed("borrowed literal"));
    bar.c.set("static");

    let mut st = [NodeState::default(); Bar::NODE_COUNT];
    let sty = pass::default_styles(&bar);
    pass::layout(&mut bar, &sty, &mut st, VIEW);

    // All three measure by the same rule, whatever holds the bytes.
    assert_eq!(st[1].size.w, 8.0 * "owned".len() as f32);
    assert_eq!(st[2].size.w, 8.0 * "borrowed literal".len() as f32);
    assert_eq!(st[3].size.w, 8.0 * "static".len() as f32);
}

#[test]
fn the_measure_cache_is_the_point() {
    let mut l: Label<String> = Label::new("hello".into());
    let mut st = [NodeState::default(); 1];
    let sty = vec![lumen_static::Style::default()];

    // First measurement is cold — the text has never been measured.
    assert!(!l.measure_is_warm(VIEW));
    pass::layout(&mut l, &sty, &mut st, VIEW);
    assert!(l.measure_is_warm(VIEW), "measuring warms the memo");

    // Re-laying out unchanged text under the same limits is a cache hit.
    pass::layout(&mut l, &sty, &mut st, VIEW);
    assert!(l.measure_is_warm(VIEW));

    // Mutating through the widget invalidates it — which is exactly what
    // a getter into sibling state could not do.
    l.set("goodbye".into());
    assert!(!l.measure_is_warm(VIEW), "a write marks the node dirty");

    // Different limits are a miss too: the same text measures
    // differently in a different box once shaping wraps it (M5).
    pass::layout(&mut l, &sty, &mut st, VIEW);
    let narrow = Constraints { max_w: 40.0, ..VIEW };
    assert!(!l.measure_is_warm(narrow));
}

#[test]
fn owning_storage_reloads_without_leaking() {
    let old: Label<String> = Label::new("user typed this".into());
    let saved = old.save();
    drop(old);

    let mut fresh: Label<String> = Reloadable::fresh();
    fresh.restore(&saved);
    assert_eq!(fresh.as_str(), "user typed this");
    // Restored through `TextStorage::from_string`, so `String` storage
    // owns it — no `Box::leak` on this path any more.
    assert_eq!(saved, Value::String("user typed this".into()));

    // …and restoring marks it dirty, so the memo cannot serve a stale
    // size for text that arrived behind the widget's back.
    assert!(!fresh.measure_is_warm(VIEW));
}
