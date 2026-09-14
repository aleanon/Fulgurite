//! M25/T25.2: the sheet lint, in both directions.
//!
//! Every firing case has a quiet twin that differs in the one thing the
//! lint keys on, so a lint that fired on everything would fail as surely
//! as one that fired on nothing.

use lumen_static::ops::{Fixed, Label};
use lumen_static::repeat::Repeat;
use lumen_static::style::{lint_sheet_deep, Fields, SheetFinding, SheetIssue};
use lumen_static::{CheckBox, Color, Length, Patch, Rule, Selector, StaticTree};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct Ui {
    #[widget_id("title")]
    title: Label,
    #[widget_id("agree")]
    agree: CheckBox<()>,
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
