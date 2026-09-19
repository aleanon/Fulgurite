//! M26/T26.10: a `Dyn` island's kinds and defaults come from its
//! **seeds**, not from the live instance inside it.
//!
//! This was the last place the cascade read a live node — the defect
//! M24 was entirely about, surviving in the one spot T24.1 did not
//! reach. The stated reason was that type erasure puts the content's
//! kinds out of reach of a const array; it does not. `Dyn::new` is
//! generic over `W`, so `W::SEEDS` is in hand at exactly the moment
//! `W::SUB_TABLE` is, and the erasure happens afterwards.
//!
//! Two consequences, and the second is the one with teeth:
//!
//! * a rule naming a kind inside an island matches it, on the seed;
//! * the sheet lint stops reporting **every** field as read for an
//!   island, which is a hole that made `SheetIssue::Unread` unable to
//!   fire for anything inside one.

use lumen_static::hatch::Dyn;
use lumen_static::ops::Label;
use lumen_static::style::{lint_sheet_deep, Fields, SheetFinding, SheetIssue};
use lumen_static::{Color, Length, Patch, Rule, Selector, StaticTree};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Island {
    #[widget_id("box")]
    boxed: Dyn,
}

const INK: Color = Color(20, 20, 20, 255);

fn lint(sheet: &[Rule]) -> Vec<SheetFinding> {
    let mut ui = Island { boxed: Dyn::new(Label::new("hello")) };
    let mut out = Vec::new();
    lint_sheet_deep(&mut ui, sheet, &mut |f| out.push(f));
    println!("{sheet:?}\n  -> {out:?}");
    out
}

/// A rule naming the boxed content's kind matches it.
#[test]
fn a_rule_naming_the_islands_kind_matches() {
    assert!(
        lint(&[Rule::new(Selector::Kind("label"), Patch::new().color(INK))]).is_empty(),
        "a `label` rule should match the `Label` inside the island"
    );
}

/// **And the lint's `Fields::ALL` hole is closed.** A rule setting a
/// field the boxed kind does not read is now a finding; it could not be
/// one while every island node claimed to read everything.
#[test]
fn a_field_the_boxed_kind_never_reads_is_reported() {
    let found = lint(&[Rule::new(Selector::Kind("label"), Patch::new().spacing(7.0))]);
    assert_eq!(
        found,
        vec![SheetFinding { rule: 0, issue: SheetIssue::Unread(Fields::SPACING) }],
        "a `label` reads no `spacing`; with `Fields::ALL` for islands this \
         could not be reported"
    );
}

/// The quiet twin: a field it *does* read stays quiet, so the fix is a
/// mask and not a lint that fires on everything.
#[test]
fn a_field_the_boxed_kind_reads_is_quiet() {
    assert!(
        lint(&[Rule::new(Selector::Kind("label"), Patch::new().width(Length::Fixed(3.0)))])
            .is_empty(),
        "a `label` does read `width`"
    );
}
