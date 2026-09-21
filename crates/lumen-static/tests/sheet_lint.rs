//! M25/T25.2: the sheet lint, in both directions.
//!
//! Every firing case has a quiet twin that differs in the one thing the
//! lint keys on, so a lint that fired on everything would fail as surely
//! as one that fired on nothing.

extern crate alloc;

use lumen_static_widgets::ops::{Fixed, Label};
use lumen_static::repeat::Repeat;
use lumen_static::style::{lint_sheet_deep, Fields, SheetFinding, SheetIssue};
use lumen_static::{Color, Length, Patch, Rule, Selector, StaticTree};
use lumen_static_widgets::{CheckBox};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct Ui {
    #[widget_id("title")]
    title: Label,
    #[widget_id("agree")]
    agree: CheckBox<()>,
    /// A control that paints a border — which a `CheckBox` does not,
    /// and the first version of `a_border_rule_on_a_node_that_draws_one_is_quiet`
    /// assumed it did. `Toggle::paint` draws a body and a mark and never
    /// calls `ops::border`, so the lint was right to report that rule
    /// and the test was wrong to expect silence.
    #[widget_id("field")]
    field: lumen_static_widgets::input::TextInput<alloc::string::String>,
    /// A kind that exists only in an arena's template: a rule for it
    /// matches, even though the main table has no `fixed`.
    rows: Repeat<Fixed>,
}

fn bind(_: &mut Fixed, _: usize) {}

fn lint(sheet: &[Rule]) -> Vec<SheetFinding> {
    let mut ui = Ui { rows: Repeat::new(3, 10.0, bind), ..Default::default() };
    let mut out = Vec::new();
    lint_sheet_deep(&mut ui, sheet, &mut |f| out.push(f));
    println!("{sheet:?}\n  -> {out:?}");
    out
}

const INK: Color = Color(20, 20, 20, 255);

#[test]
fn a_kind_spelled_like_a_struct_matches_nothing() {
    let found = lint(&[Rule::new(Selector::Kind("Label"), Patch::new().color(INK))]);
    assert_eq!(found, vec![SheetFinding { rule: 0, issue: SheetIssue::Unmatched }]);
}

#[test]
fn the_same_kind_spelled_as_the_tree_spells_it_is_quiet() {
    assert!(lint(&[Rule::new(Selector::Kind("label"), Patch::new().color(INK))]).is_empty());
}

#[test]
fn an_id_nothing_carries_matches_nothing() {
    let found = lint(&[
        Rule::new(Selector::Id("titel"), Patch::new().color(INK)),
        Rule::new(Selector::Id("title"), Patch::new().color(INK)),
    ]);
    assert_eq!(found, vec![SheetFinding { rule: 0, issue: SheetIssue::Unmatched }]);
}

#[test]
fn a_field_the_matched_kind_never_reads_is_reported_and_one_it_reads_is_not() {
    let found = lint(&[
        Rule::new(Selector::Kind("checkbox"), Patch::new().font_weight(700.0)),
        Rule::new(Selector::Kind("checkbox"), Patch::new().width(Length::Fixed(30.0))),
    ]);
    assert_eq!(
        found,
        vec![SheetFinding { rule: 0, issue: SheetIssue::Unread(Fields::FONT_WEIGHT) }],
        "a checkbox paints no text, so its font_weight is a no-op; its width is not"
    );
}

#[test]
fn only_the_unread_part_of_a_rule_is_reported() {
    let found = lint(&[Rule::new(
        Selector::Kind("checkbox"),
        Patch::new().font_weight(700.0).width(Length::Fixed(30.0)),
    )]);
    assert_eq!(
        found,
        vec![SheetFinding { rule: 0, issue: SheetIssue::Unread(Fields::FONT_WEIGHT) }]
    );
}

#[test]
fn a_kind_found_only_in_an_arena_template_matches() {
    assert!(
        lint(&[Rule::new(Selector::Kind("fixed"), Patch::new().color(INK))]).is_empty(),
        "a repeater's row kind is reachable by a rule, so the lint must see it"
    );
}

/// M25/T25.11: a role sets the field it resolves into, so it is judged as
/// that field. `Fixed` reads `color` and not `background`, so the same
/// kind gives both the firing case and its quiet twin.
#[test]
fn a_role_is_linted_as_the_field_it_sets() {
    use lumen_static::style::Role;
    let found = lint(&[
        Rule::new(Selector::Kind("fixed"), Patch::new().background_role(Role::Surface)),
        Rule::new(Selector::Kind("fixed"), Patch::new().color_role(Role::Text)),
    ]);
    assert_eq!(
        found,
        vec![SheetFinding { rule: 0, issue: SheetIssue::Unread(Fields::BACKGROUND) }],
        "a `Fixed` paints no background, so a background role on one is a no-op; its colour role is not"
    );
}

#[test]
fn any_always_matches() {
    assert!(lint(&[Rule::new(Selector::Any, Patch::new().width(Length::FILL))]).is_empty());
}

/// **A border rule is a rule like any other** (M27/T27.11).
///
/// `Style` gained `border_width` and `border_color` in T27.1 and
/// `Patch::fields` never learned about them, so a rule that set *only* a
/// border reported the empty set — and the lint's whole claim, "no rule
/// in this sheet is a no-op", was silently false for every border rule
/// in it.
///
/// Both directions, because a `Fields` bit that is always set is as
/// useless as one never set. **Which control goes in which direction
/// was measured, not assumed**: the first version of this pair had them
/// the other way round, on the reasoning that a text label is plain and
/// a checkbox has a box around it.
/// `lumen-static-widgets/tests/style_reads.rs` probes every kind's
/// output field by field and says otherwise — a `Label` paints
/// `ops::border` when a rule gives it one, and `Toggle::paint` draws a
/// body and a mark and never calls it.
#[test]
fn a_border_rule_on_a_node_that_draws_none_is_a_finding() {
    let found = lint(&[Rule::new(
        Selector::Id("agree"),
        Patch::new().border_width(2).border_color(INK),
    )]);
    assert_eq!(
        found,
        vec![SheetFinding { rule: 0, issue: SheetIssue::Unread(Fields::BORDER) }],
        "a border on a `CheckBox`, which paints none"
    );
}

#[test]
fn a_border_rule_on_a_node_that_draws_one_is_quiet() {
    assert!(
        lint(&[Rule::new(
            Selector::Id("field"),
            Patch::new().border_width(2).border_color(INK),
        )])
        .is_empty(),
        "a border on a control that paints one"
    );
}
