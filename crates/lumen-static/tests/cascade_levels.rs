//! M26/T26.6: the cascade skips the specificity passes a sheet has none
//! of — and the ordering that makes those passes necessary is unchanged.
//!
//! The loop runs four passes per node so a higher-specificity rule
//! always lands last. Three of the four are usually empty: every sheet
//! in this workspace uses `Id` and `Kind` and neither `Class` nor
//! `Any`, so half the passes scanned every rule per node to match
//! nothing.
//!
//! **What is gated is the ordering, not the skip.** A differential test
//! against a naive copy of the loop would pass under an injection that
//! broke both, since both would be the same code. So this asserts the
//! contract the passes exist for: the most specific rule that matches a
//! node is the one whose value survives, whichever levels the sheet
//! happens to contain.

use lumen_static::ops::Label;
use lumen_static::style::{cascade, Length, Patch, Rule, Selector, Style};
use lumen_static::{StaticTree, Widget};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("named")]
    #[widget_class("boxed")]
    named: Label,
    #[widget_class("boxed")]
    classed: Label,
    plain: Label,
}

/// One rule per level, all setting the same field, so the winner is
/// decided by specificity alone.
fn four_levels() -> [Rule; 4] {
    [
        Rule::new(Selector::Any, Patch::new().font_size(10.0)),
        Rule::new(Selector::Kind("label"), Patch::new().font_size(20.0)),
        Rule::new(Selector::Class("boxed"), Patch::new().font_size(30.0)),
        Rule::new(Selector::Id("named"), Patch::new().font_size(40.0)),
    ]
}

fn sizes(sheet: &[Rule]) -> Vec<f32> {
    let mut styles = vec![Style::default(); Ui::NODE_COUNT];
    cascade(<Ui as Widget>::SUB_TABLE, <Ui as Widget>::SEEDS, sheet, &mut styles);
    // 1 is `named`, 2 `classed`, 3 `plain`.
    vec![styles[1].font_size, styles[2].font_size, styles[3].font_size]
}

#[test]
fn the_most_specific_rule_wins_at_every_level() {
    let got = sizes(&four_levels());
    assert_eq!(
        got,
        vec![40.0, 30.0, 20.0],
        "id > class > kind: the node with an id should take 40, the one with \
         only a class 30, and the bare label 20"
    );
}

/// The skip's own claim: removing a level the sheet does not have
/// changes nothing, and removing one it *does* changes exactly that
/// node.
#[test]
fn a_sheet_missing_a_level_still_resolves_the_rest() {
    let all = four_levels();
    // No `Class` rule: the classed node falls back to its kind's 20.
    let without_class = [all[0], all[1], all[3]];
    assert_eq!(
        sizes(&without_class),
        vec![40.0, 20.0, 20.0],
        "with no class rule the classed node should take the kind's value"
    );
    // No `Id` rule: the named node falls back to its class's 30.
    let without_id = [all[0], all[1], all[2]];
    assert_eq!(
        sizes(&without_id),
        vec![30.0, 30.0, 20.0],
        "with no id rule the named node should take the class's value"
    );
    // Only `Any`: everything takes it.
    assert_eq!(sizes(&all[..1]), vec![10.0, 10.0, 10.0], "an `Any` rule reaches every node");
}

/// **The order within a level is source order**, which the skip must not
/// disturb: two rules of the same specificity, the later one wins.
#[test]
fn within_one_level_the_later_rule_wins() {
    let sheet = [
        Rule::new(Selector::Kind("label"), Patch::new().width(Length::Fixed(11.0))),
        Rule::new(Selector::Kind("label"), Patch::new().width(Length::Fixed(22.0))),
    ];
    let mut styles = vec![Style::default(); Ui::NODE_COUNT];
    cascade(<Ui as Widget>::SUB_TABLE, <Ui as Widget>::SEEDS, &sheet, &mut styles);
    assert_eq!(
        styles[1].width,
        Length::Fixed(22.0),
        "the second of two rules at the same level should land last"
    );
}

/// An empty sheet leaves every node on its own defaults — the case
/// where *every* level is absent, and the one a mask computed wrong
/// would most easily break.
#[test]
fn an_empty_sheet_changes_nothing() {
    let mut styles = vec![Style::default(); Ui::NODE_COUNT];
    cascade(<Ui as Widget>::SUB_TABLE, <Ui as Widget>::SEEDS, &[], &mut styles);
    let defaults = lumen_static::pass::default_styles(&Ui::default());
    for (i, (got, want)) in styles.iter().zip(defaults.iter()).enumerate() {
        assert_eq!(got.font_size, want.font_size, "node {i} moved off its defaults");
    }
}
