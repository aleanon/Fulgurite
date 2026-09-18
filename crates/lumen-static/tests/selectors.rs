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

/// **A struct-level `#[widget_class]` reaches the container's own node.**
///
/// M27/T27.7. It was accepted by `attributes(...)` and silently ignored:
/// nothing read it, the class never reached the table, and a rule
/// written against it matched nothing. That is F1.5's shape one level
/// up — the documented-looking spelling producing `&[]` — and it was
/// found by the showcase's sheet lint reporting the rule `Unmatched`,
/// not by anyone reading the derive.
#[test]
fn a_struct_level_class_reaches_the_container() {
    use lumen_static::widget::Widget;

    #[derive(lumen_static::StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    #[widget_class("panel", "raised")]
    struct Card {
        #[widget_id("inner")]
        inner: lumen_static::ops::Label,
    }

    let t = <Card as Widget>::SUB_TABLE;
    assert_eq!(
        t[0].classes,
        &["panel", "raised"],
        "the container's own classes are {:?}; a struct-level \
         #[widget_class] reached nothing",
        t[0].classes
    );
    // And it did not leak onto the child, which has none of its own.
    assert!(
        t[1].classes.is_empty(),
        "the container's classes leaked onto its child: {:?}",
        t[1].classes
    );
}

/// The accepting direction, in the same session: a container with no
/// `#[widget_class]` has no classes, so the test above cannot be
/// satisfied by a derive that puts classes on everything.
#[test]
fn a_container_without_the_attribute_has_no_classes() {
    use lumen_static::widget::Widget;

    #[derive(lumen_static::StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct Plain {
        inner: lumen_static::ops::Label,
    }

    assert!(
        <Plain as Widget>::SUB_TABLE[0].classes.is_empty(),
        "a container with no attribute has classes {:?}",
        <Plain as Widget>::SUB_TABLE[0].classes
    );
}
