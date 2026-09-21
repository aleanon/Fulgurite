//! M8c/T8c.4: re-cascading a subtree agrees with re-cascading the table.
//!
//! The property that makes it sound is that every `Selector` is
//! node-local — no combinator reads an ancestor — so a node's resolved
//! style is a function of that node and the sheet. These tests pin that
//! property rather than the implementation: if a descendant combinator
//! is ever added, `a_scoped_cascade_equals_a_full_one` is what fails.

use lumen_static_widgets::ops::{Fixed, Label, Spacer};
use lumen_static::style::{
    cascade_deep, cascade_subtree, index_of_wid, subtree_range, Length, Patch, Rule, Selector,
};
use lumen_static::{pass, StaticTree, Style, Widget};

#[derive(StaticTree, Default)]
#[widget(row, spacing = 2.0)]
struct Steps {
    #[widget_id("dec")]
    dec: Fixed,
    #[widget_id("inc")]
    inc: Fixed,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct Card {
    #[widget_id("value")]
    value: Label,
    #[widget_id("pill")]
    pill: Label,
    steps: Steps,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Page {
    #[widget_id("head")]
    head: Label,
    #[widget_id("card")]
    card: Card,
    #[widget_id("foot")]
    foot: Spacer,
}

fn base() -> Vec<Rule> {
    vec![
        Rule::new(Selector::Any, Patch::new().font_size(12.0)),
        Rule::new(Selector::Id("head"), Patch::new().font_size(20.0)),
        Rule::new(Selector::Id("value"), Patch::new().font_size(40.0)),
    ]
}

/// The base sheet plus a rule that matches only inside the card — what
/// a class toggle looks like.
fn toggled() -> Vec<Rule> {
    let mut s = base();
    s.push(Rule::new(Selector::Id("value"), Patch::new().font_size(76.0)));
    s.push(Rule::new(Selector::Id("pill"), Patch::new().width(Length::Fixed(88.0))));
    s
}

fn styled(sheet: &[Rule]) -> Vec<Style> {
    let mut app = Page::default();
    let mut out = pass::default_styles(&app);
    cascade_deep(&mut app, sheet, &mut out);
    out
}

#[test]
fn a_subtree_is_a_contiguous_range_of_the_table() {
    let t = Page::SUB_TABLE;
    // The card is the second field: root, head, then the card's own
    // subtree, then the foot.
    let card = index_of_wid(t, "card").expect("card is in the table");
    let r = subtree_range(t, card);
    assert_eq!(r.len(), Card::NODE_COUNT, "the range is exactly the card's nodes");
    assert!(r.contains(&index_of_wid(t, "value").unwrap()));
    assert!(r.contains(&index_of_wid(t, "pill").unwrap()));
    assert!(r.contains(&index_of_wid(t, "dec").unwrap()), "and its grandchildren");
    assert!(!r.contains(&index_of_wid(t, "head").unwrap()));
    assert!(!r.contains(&index_of_wid(t, "foot").unwrap()));

    // A last child has no next sibling: its range must still end at the
    // right place, walking up to an ancestor that has one.
    let inc = index_of_wid(t, "inc").expect("inc");
    assert_eq!(subtree_range(t, inc).len(), 1, "a leaf is its own subtree");
    // The root spans everything.
    assert_eq!(subtree_range(t, 0), 0..Page::NODE_COUNT);
}

#[test]
fn a_scoped_cascade_equals_a_full_one() {
    // The oracle: swap in a sheet whose new rules match only inside the
    // card, scoped; compare against cascading the whole table with it.
    let mut app = Page::default();
    let mut scoped = pass::default_styles(&app);
    cascade_deep(&mut app, &base(), &mut scoped);

    let card = index_of_wid(Page::SUB_TABLE, "card").unwrap();
    cascade_subtree(&mut app, &toggled(), &mut scoped, card);

    assert_eq!(scoped, styled(&toggled()), "scoped and full cascades disagree");
}

#[test]
fn scoping_to_a_leaf_restyles_only_it() {
    let mut app = Page::default();
    let mut out = pass::default_styles(&app);
    cascade_deep(&mut app, &base(), &mut out);
    let before = out.clone();

    // A sheet that changes only `#value` — the contract for scoping to
    // `value` alone. (`toggled()` also changes `#pill`, and scoping it
    // to a leaf is what the debug oracle below refuses.)
    let mut only_value = base();
    only_value.push(Rule::new(Selector::Id("value"), Patch::new().font_size(76.0)));

    let value = index_of_wid(Page::SUB_TABLE, "value").unwrap();
    cascade_subtree(&mut app, &only_value, &mut out, value);

    assert_eq!(out[value].font_size, 76.0, "the scoped node took the new rule");
    for (i, s) in out.iter().enumerate() {
        if i != value {
            assert_eq!(*s, before[i], "node {i} changed outside the scope");
        }
    }
}

#[test]
fn an_unknown_id_is_not_found() {
    assert_eq!(index_of_wid(Page::SUB_TABLE, "nope"), None);
    assert_eq!(index_of_wid(Page::SUB_TABLE, ""), None, "the empty wid is not an id");
}

#[test]
#[should_panic(expected = "disagreeing with a full cascade")]
#[cfg(debug_assertions)]
fn scoping_a_sheet_that_changes_the_outside_is_caught_in_debug() {
    // The caller's contract, enforced: this sheet also changes `head`,
    // which is outside the card, so the scoped result would silently
    // differ from a full cascade.
    let mut app = Page::default();
    let mut out = pass::default_styles(&app);
    cascade_deep(&mut app, &base(), &mut out);

    let mut wrong = toggled();
    wrong.push(Rule::new(Selector::Id("head"), Patch::new().font_size(99.0)));
    let card = index_of_wid(Page::SUB_TABLE, "card").unwrap();
    cascade_subtree(&mut app, &wrong, &mut out, card);
}
