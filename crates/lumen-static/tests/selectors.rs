//! `#[widget_id]` and `#[widget_class]` reach `SUB_TABLE`, and the class
//! selector matches through the cascade.
//!
//! The documented spelling `#[widget_class("row", "striped")]` silently
//! produced an empty list until 2026-09-02 — `parse_nested_meta` yields
//! paths, a string literal is not one, and the error was discarded. So
//! `Selector::Class` was unreachable through the derive for anyone who
//! followed the docs, and nothing in the repo used the attribute at all.
//! These are its first uses (`docs/plan-review-fixes.md` F1.5).

use lumen_static::ops::{Fixed, Label};
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    /// The documented spelling: string literals.
    #[widget_id("head")]
    #[widget_class("row", "striped")]
    head: Label,
    /// The bare-ident spelling, also accepted.
    #[widget_id("body")]
    #[widget_class(row)]
    body: Fixed,
    /// No attributes: empty id, empty classes.
    tail: Fixed,
}

#[test]
fn ids_and_classes_reach_the_table() {
    let t = <Ui as Widget>::SUB_TABLE;
    assert_eq!(t[1].wid, "head");
    assert_eq!(t[1].classes, &["row", "striped"], "the documented string form");
    assert_eq!(t[2].wid, "body");
    assert_eq!(t[2].classes, &["row"], "the bare-ident form");
    assert_eq!(t[3].wid, "");
    assert_eq!(t[3].classes, &[] as &[&str]);
}

#[test]
fn a_class_rule_matches_every_member_and_nothing_else() {
    let mut ui = Ui::default();
    let mut styles = pass::default_styles(&ui);
    let sheet = [Rule::new(Selector::Class("row"), Patch::new().height(Length::Fixed(7.0)))];
    lumen_static::style::cascade(
        <Ui as Widget>::SUB_TABLE,
        <Ui as Widget>::SEEDS,
        &sheet,
        &mut styles,
    );
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    pass::layout(
        &mut ui,
        &styles,
        &mut st,
        Constraints { min_w: 0.0, max_w: 100.0, min_h: 0.0, max_h: 100.0 },
    );

    assert_eq!(st[1].size.h, 7.0, "`head` carries the class");
    assert_eq!(st[2].size.h, 7.0, "`body` carries it too");
    assert_ne!(st[3].size.h, 7.0, "`tail` does not");
}

#[test]
fn a_second_class_on_the_same_node_is_matched_independently() {
    let mut ui = Ui::default();
    let mut styles = pass::default_styles(&ui);
    let sheet = [Rule::new(Selector::Class("striped"), Patch::new().height(Length::Fixed(9.0)))];
    lumen_static::style::cascade(
        <Ui as Widget>::SUB_TABLE,
        <Ui as Widget>::SEEDS,
        &sheet,
        &mut styles,
    );
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    pass::layout(
        &mut ui,
        &styles,
        &mut st,
        Constraints { min_w: 0.0, max_w: 100.0, min_h: 0.0, max_h: 100.0 },
    );
    assert_eq!(st[1].size.h, 9.0, "only `head` has `striped`");
    assert_ne!(st[2].size.h, 9.0);
}
