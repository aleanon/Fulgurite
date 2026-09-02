//! A stylesheet rule must be able to reach inside a template.
//!
//! `cascade` walks one table, and every construct that keeps children
//! off the table was therefore unreachable: a rule could not set the
//! padding of a repeater's row, and `#[widget_id]` inside a template was
//! dead for styling as well as for agent selectors. Only the repeater's
//! case was documented as a known limit — `Either`, `List` and `Dyn`
//! inherited it silently, which is the difference between a decision and
//! an accident (`docs/plan-review-fixes.md` F4.5).

use lumen_static::hatch::{Either, List, Side};
use lumen_static::ops::{Fixed, Label};
use lumen_static::repeat::{noop_bind, Repeat};
use lumen_static::style::{cascade_deep, Length, Patch, Rule, Selector};
use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Row {
    #[widget_id("icon")]
    #[widget_class("chip")]
    icon: Fixed,
    name: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct WithRepeat {
    list: Repeat<Row>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct WithList {
    list: List<Row>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct WithEither {
    tabs: Either<Row, Row>,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 200.0 };

#[test]
fn an_id_rule_reaches_a_repeater_template_row() {
    let mut ui = WithRepeat { list: Repeat::new(50, 24.0, noop_bind) };
    let mut styles = pass::default_styles(&ui);
    cascade_deep(
        &mut ui,
        &[Rule::new(Selector::Id("icon"), Patch::new().width(Length::Fixed(33.0)))],
        &mut styles,
    );
    let mut st = [NodeState::default(); WithRepeat::NODE_COUNT];
    pass::layout(&mut ui, &styles, &mut st, VIEW);

    // Every materialized row's icon carries the rule.
    for k in 0..ui.list.window_len() {
        assert_eq!(
            ui.list.instance_state(k, 1).size.w,
            33.0,
            "row {k}'s icon should carry the #icon rule"
        );
    }
}

#[test]
fn a_class_rule_reaches_a_template_too() {
    let mut ui = WithRepeat { list: Repeat::new(10, 24.0, noop_bind) };
    let mut styles = pass::default_styles(&ui);
    cascade_deep(
        &mut ui,
        &[Rule::new(Selector::Class("chip"), Patch::new().width(Length::Fixed(17.0)))],
        &mut styles,
    );
    let mut st = [NodeState::default(); WithRepeat::NODE_COUNT];
    pass::layout(&mut ui, &styles, &mut st, VIEW);
    assert_eq!(ui.list.instance_state(0, 1).size.w, 17.0);
}

#[test]
fn a_rule_reaches_list_items() {
    let mut ui = WithList { list: List::new((0..4).map(|_| Row::default()).collect()) };
    let mut styles = pass::default_styles(&ui);
    cascade_deep(
        &mut ui,
        &[Rule::new(Selector::Id("icon"), Patch::new().width(Length::Fixed(21.0)))],
        &mut styles,
    );
    let mut st = [NodeState::default(); WithList::NODE_COUNT];
    pass::layout(&mut ui, &styles, &mut st, VIEW);
    for k in 0..4 {
        assert_eq!(ui.list.item_state(k, 1).size.w, 21.0, "item {k}");
    }
}

#[test]
fn a_rule_reaches_both_either_branches() {
    // Both, not just the showing one: both keep their state and either
    // may be shown next, so styling only the active branch would make a
    // toggle change more than which branch is visible.
    let mut ui = WithEither { tabs: Either::new(Row::default(), Row::default()) };
    let mut styles = pass::default_styles(&ui);
    cascade_deep(
        &mut ui,
        &[Rule::new(Selector::Id("icon"), Patch::new().width(Length::Fixed(9.0)))],
        &mut styles,
    );
    let mut st = [NodeState::default(); WithEither::NODE_COUNT];

    pass::layout(&mut ui, &styles, &mut st, VIEW);
    assert_eq!(ui.tabs.active_state(1).size.w, 9.0, "branch A");

    ui.tabs.show(Side::B);
    pass::layout(&mut ui, &styles, &mut st, VIEW);
    assert_eq!(ui.tabs.active_state(1).size.w, 9.0, "branch B, after a toggle");
}

#[test]
fn a_rule_that_matches_nothing_changes_nothing() {
    let mut ui = WithRepeat { list: Repeat::new(10, 24.0, noop_bind) };
    let mut styles = pass::default_styles(&ui);
    let before = styles.clone();
    cascade_deep(
        &mut ui,
        &[Rule::new(Selector::Id("nonesuch"), Patch::new().width(Length::Fixed(99.0)))],
        &mut styles,
    );
    assert_eq!(styles, before, "the main table is untouched");
    let mut st = [NodeState::default(); WithRepeat::NODE_COUNT];
    pass::layout(&mut ui, &styles, &mut st, VIEW);
    assert_ne!(ui.list.instance_state(0, 1).size.w, 99.0);
}
