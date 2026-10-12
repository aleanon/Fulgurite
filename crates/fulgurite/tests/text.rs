//! M3: text storage — generic over the profile's choice, owned so the
//! measurement cache can be invalidated, and reload-clean for owning
//! storage.

use fulgurite_widgets::ops::{Fixed, Label};
use fulgurite::reload::{Reloadable, Value};
use fulgurite::text::Measured;
use fulgurite::{pass, Constraints, NodeState, StaticTree, Widget};
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
    let sty = vec![fulgurite::Style::default()];

    // First measurement is cold — the text has never been measured.
    assert!(!l.measure_is_warm(VIEW, &sty[0], 1.0, 0));
    pass::layout(&mut l, &sty, &mut st, VIEW);
    assert!(l.measure_is_warm(VIEW, &sty[0], 1.0, 0), "measuring warms the memo");

    // Re-laying out unchanged text under the same limits is a cache hit.
    pass::layout(&mut l, &sty, &mut st, VIEW);
    assert!(l.measure_is_warm(VIEW, &sty[0], 1.0, 0));

    // Mutating through the widget invalidates it — which is exactly what
    // a getter into sibling state could not do.
    l.set("goodbye".into());
    assert!(!l.measure_is_warm(VIEW, &sty[0], 1.0, 0), "a write marks the node dirty");

    // Different limits are a miss too: the same text measures
    // differently in a different box once shaping wraps it (M5).
    pass::layout(&mut l, &sty, &mut st, VIEW);
    let narrow = Constraints { max_w: 40.0, ..VIEW };
    assert!(!l.measure_is_warm(narrow, &sty[0], 1.0, 0));
}

/// A label's text is **code** (M49/T49.5, R5-F1): a hot swap keeps what
/// the new build wrote, not what the old one did. This test used to
/// restore "user typed this" into a `Label<String>`; a text a user types
/// is a `TextInput`'s, and one that changes at run time is `#[bind]`'s.
#[test]
fn a_label_saves_nothing_so_the_new_codes_text_shows() {
    let old: Label<String> = Label::new("v1's text".into());
    let saved = old.save();
    assert_eq!(saved, Value::Null, "a label saved its text");

    let mut fresh: Label<String> = Label::new("v2's text".into());
    fresh.restore(&saved);
    assert_eq!(fresh.as_str(), "v2's text");
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

/// A `Button` saves nothing and restores nothing — M49/T49.5.
///
/// F6.4 made `Button::restore` stop leaking its label; T49.5 removed the
/// restore altogether, because a button's label is code (R5-F1): after a
/// swap it reads what the new build says, and nothing is leaked because
/// nothing is restored.
#[test]
fn a_button_restores_nothing_so_nothing_leaks() {
    use fulgurite_widgets::ops::Button;

    struct App;
    let mut b: Button<App, String> = Button::default();
    b.label = "Store".into();
    assert_eq!(b.save(), Value::Null);
    b.restore(&Value::String("Save".into()));
    assert_eq!(b.label, "Store", "the new build's label stays");
}
