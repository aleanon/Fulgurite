//! M3: text storage — generic over the profile's choice, owned so the
//! measurement cache can be invalidated, and reload-clean for owning
//! storage.

use lumen_static::ops::{Fixed, Label};
use lumen_static::reload::{Reloadable, Value};
use lumen_static::text::Measured;
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
    assert!(!l.measure_is_warm(VIEW, &sty[0]));
    pass::layout(&mut l, &sty, &mut st, VIEW);
    assert!(l.measure_is_warm(VIEW, &sty[0]), "measuring warms the memo");

    // Re-laying out unchanged text under the same limits is a cache hit.
    pass::layout(&mut l, &sty, &mut st, VIEW);
    assert!(l.measure_is_warm(VIEW, &sty[0]));

    // Mutating through the widget invalidates it — which is exactly what
    // a getter into sibling state could not do.
    l.set("goodbye".into());
    assert!(!l.measure_is_warm(VIEW, &sty[0]), "a write marks the node dirty");

    // Different limits are a miss too: the same text measures
    // differently in a different box once shaping wraps it (M5).
    pass::layout(&mut l, &sty, &mut st, VIEW);
    let narrow = Constraints { max_w: 40.0, ..VIEW };
    assert!(!l.measure_is_warm(narrow, &sty[0]));
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
    assert!(!fresh.measure_is_warm(VIEW, &lumen_static::Style::default()));
}

/// A tree holding a `Label` is `Sync`.
///
/// `Measured` used interior mutability — `Cell<Option<..>>` with a
/// `&self` getter — inherited from the flat-pass design, where ops were
/// handed a shared base pointer. `layout` takes `&mut self` and
/// `get_or` has one caller inside it, so the `Cell` bought nothing and
/// cost `Sync` on every tree containing text. `repeat.rs` records
/// removing a `RefCell` for exactly this reason
/// (`docs/plan-review-fixes.md` F6.3).
#[test]
fn a_tree_with_text_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Label<String>>();
    assert_send_sync::<Label<&'static str>>();
    assert_send_sync::<Measured>();
}

/// `Button` restores without leaking when its storage can own.
///
/// `Button::restore` used `Box::leak` unconditionally, with no comment,
/// while `Label`'s two equivalent impls document theirs prominently. It
/// leaked its label on every reload, and there was no non-leaking path
/// because `Button` had no storage parameter at all
/// (`docs/plan-review-fixes.md` F6.4).
#[test]
fn a_button_with_owned_storage_restores_without_leaking() {
    use lumen_static::ops::Button;

    struct App;
    let mut b: Button<App, String> = Button::default();
    b.restore(&Value::String("Save".into()));
    assert_eq!(b.label, "Save");

    // Restoring repeatedly reuses the owned buffer rather than leaking a
    // new `&'static str` each time.
    for i in 0..100 {
        b.restore(&Value::String(format!("v{i}")));
    }
    assert_eq!(b.label, "v99");

    // The default storage still exists and still leaks — deliberately,
    // as a dev-only path, and now as a choice the type records.
    let mut d: Button<App> = Button::default();
    d.restore(&Value::String("Save".into()));
    assert_eq!(d.label, "Save");
}
