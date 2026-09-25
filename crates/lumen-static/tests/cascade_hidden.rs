//! M24/T24.1 — a hidden subtree cascades from **its own type**.
//!
//! The cascade's two non-structural inputs used to come off the live
//! tree, and a hidden subtree has no live node to ask: `Option::<W>::node`
//! answers with the `Option` when `None`, and the enum switch answers
//! with the enum for an inactive variant's indices. Both are deliberate —
//! the agent walks those indices — and both meant a hidden `CheckBox`
//! resolved the *container's* kind and defaults.
//!
//! What that looked like on screen is why this file exists:
//! `Fixed(24.0)` became `Shrink`, so every hidden control was a box with
//! no size of its own — "most widgets just look like a label", reported
//! against `examples/showcase` on 2026-09-12
//! (`docs/plan-select.md` §1).

use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;
use lumen_static::style::{cascade_deep, Length, Patch, Rule, Selector, Style};
use lumen_static::widget::{Node, Widget};

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Text {
    #[widget_id("t")]
    l: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Boxes {
    #[widget_id("b")]
    t: CheckBox<()>,
}

#[derive(StaticTree)]
#[allow(dead_code)] // `Second` is never built: being hidden is the point.
enum Pick {
    First(Text),
    Second(Boxes),
}
impl Default for Pick {
    fn default() -> Self {
        Pick::First(Text::default())
    }
}

/// **`cascade_deep`, not `cascade`.** This is what `Host::set_stylesheet`
/// calls, so it is what has to be gated — a test that reached past it to
/// the primitive would pass while the shipped path went on reading the
/// live tree.
fn resolved<W: Widget>(tree: &mut W, sheet: &[Rule]) -> Vec<Style> {
    let mut out = vec![Style::default(); W::SUB_TABLE.len()];
    cascade_deep(tree, sheet, &mut out);
    out
}

fn index_of<W: Widget>(wid: &str) -> usize {
    W::SUB_TABLE.iter().position(|d| d.wid == wid).expect("no such widget id")
}

/// The defect, as an assertion: a `Kind` rule must reach the variant
/// that is **not** showing.
#[test]
fn a_kind_rule_reaches_an_inactive_variant() {
    let mut tree = Pick::default(); // First(Text); Second(Boxes) is hidden
    let bi = index_of::<Pick>("b");

    // The premise: the live tree genuinely cannot answer for this node.
    // Without this the test could pass on a tree where nothing is hidden
    // and prove nothing at all.
    assert_eq!(
        Node::kind_name(Node::node(&tree, bi)),
        <Pick as Widget>::KIND,
        "premise failed: node {bi} is reachable through the live tree, so this \
         test is not exercising a hidden subtree"
    );

    let sheet =
        [Rule::new(Selector::Kind("checkbox"), Patch::new().width(Length::Fixed(99.0)))];
    let out = resolved(&mut tree, &sheet);
    assert_eq!(
        out[bi].width,
        Length::Fixed(99.0),
        "a Kind(\"checkbox\") rule did not reach the hidden CheckBox: it resolved {:?}",
        out[bi].width
    );
}

/// And with **no** rule at all it must still get its own defaults, which
/// is the half that produced the reported symptom: nobody writes a
/// stylesheet rule for a checkbox's width, they rely on the widget.
#[test]
fn an_inactive_variant_keeps_its_own_defaults() {
    let mut tree = Pick::default();
    let bi = index_of::<Pick>("b");
    let out = resolved(&mut tree, &[]);
    // Its own seed resolved — defaults plus its palette roles (M57/A8).
    let own = <CheckBox<()> as Widget>::SEEDS[0].style(&lumen_static::style::Palette::LIGHT);
    assert_eq!(
        out[bi].width, own.width,
        "hidden CheckBox resolved width {:?}; its own default is {:?}",
        out[bi].width, own.width
    );
    assert_eq!(out[bi].background, own.background, "and its resting colour is gone too");
    assert!(
        !matches!(out[bi].width, Length::Shrink),
        "a Fixed-width control resolved Shrink — the container's defaults leaked in, \
         which on screen is a control with no size of its own"
    );
}

/// The other direction, which is not optional: the fix must not hand
/// *every* node the leaf's defaults. The showing variant, the enum's own
/// node and the container between them each keep theirs.
#[test]
fn the_showing_variant_is_unaffected() {
    let mut tree = Pick::default();
    let out = resolved(&mut tree, &[]);
    let ti = index_of::<Pick>("t");
    assert_eq!(out[ti].width, <Label<String> as Widget>::type_defaults().width);
    assert_eq!(out[0].width, <Pick as Widget>::type_defaults().width, "the enum's own node");
    // The hidden variant's *container* node, between the enum and the
    // checkbox, must be `Boxes` — not the enum and not the checkbox.
    let bi = index_of::<Pick>("b");
    assert_eq!(out[bi - 1].spacing, <Boxes as Widget>::type_defaults().spacing);
    assert_ne!(out[bi - 1].width, <CheckBox<()> as Widget>::type_defaults().width);
}

/// An `Option` adds no node, so an empty one must resolve exactly as a
/// full one. This is what `Demos::fill_all` was working around in
/// `examples/showcase` — build the tree full, cascade, then empty it.
#[test]
fn an_empty_option_resolves_as_its_content() {
    let mut full: Option<Boxes> = Some(Boxes::default());
    let mut empty: Option<Boxes> = None;
    let sheet = [Rule::new(Selector::Kind("checkbox"), Patch::new().radius(3.0))];
    let a = resolved(&mut full, &sheet);
    let b = resolved(&mut empty, &sheet);
    assert_eq!(a, b, "Some and None cascaded differently");
    let bi = index_of::<Option<Boxes>>("b");
    assert_eq!(b[bi].radius, 3.0, "the rule did not reach an empty Option's content");
    assert_eq!(b[bi].width, <CheckBox<()> as Widget>::type_defaults().width);
}
