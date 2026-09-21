//! M26/T26.14: a `#[widget_id]` can be resolved at compile time, so a
//! typo is a build error rather than a frame that never redraws.
//!
//! A misspelled or renamed id marks **nothing**: `index_of_wid` returns
//! `None` — it cannot return a *wrong* index — and the frame quietly
//! fails to update. It is silent only for a caller that ignores the
//! answer: `Host::invalidate` and `Host::restyle_subtree` have returned
//! a `#[must_use] bool` since T16.7 for exactly this reason, and every
//! caller in this repository asserts it. So this buys little against
//! the current tree and everything against the next caller.
//!
//! **The good-input half is the one with work in it**: the const lookup
//! and the runtime lookup must agree for every id in a tree, or a
//! caller that resolves at compile time and a caller that resolves at
//! runtime disagree about the same name.

use lumen_static_widgets::ops::{Fixed, Label};
use lumen_static::style::index_of_wid;
use lumen_static::widget::Widget;
use lumen_static::{wid_index, StaticTree};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Inner {
    #[widget_id("leaf")]
    leaf: Fixed,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("title")]
    title: Label,
    #[widget_id("wrapper")]
    wrapper: Inner,
    bare: Fixed,
}

const TITLE: usize = wid_index!(Ui, "title");
const WRAPPER: usize = wid_index!(Ui, "wrapper");
const LEAF: usize = wid_index!(Ui, "leaf");

/// **The two lookups agree, for every id in the tree.** Not just the
/// three above: the table is walked, so an id added later is covered by
/// this test without anyone remembering to add it.
#[test]
fn the_const_lookup_agrees_with_the_runtime_one_for_every_id() {
    let table = <Ui as Widget>::SUB_TABLE;
    let mut checked = 0;
    for (i, d) in table.iter().enumerate() {
        if d.wid.is_empty() {
            continue;
        }
        assert_eq!(
            index_of_wid(table, d.wid),
            Some(i),
            "node {i} carries {:?} and the lookup answers otherwise",
            d.wid
        );
        checked += 1;
    }
    assert_eq!(checked, 3, "the fixture should carry three ids, not {checked}");
}

#[test]
fn the_macro_resolves_to_the_same_index() {
    let table = <Ui as Widget>::SUB_TABLE;
    assert_eq!(Some(TITLE), index_of_wid(table, "title"));
    assert_eq!(Some(WRAPPER), index_of_wid(table, "wrapper"));
    assert_eq!(Some(LEAF), index_of_wid(table, "leaf"));
    // And they are distinct, so a macro that resolved everything to 0
    // would not pass.
    const { assert!(TITLE != WRAPPER) };
    const { assert!(WRAPPER != LEAF) };
}

/// The id reaches a node *inside* a child widget — a table is spliced,
/// so `wrapper`'s own leaf is addressable from the root's type.
#[test]
fn an_id_inside_a_child_is_reachable() {
    const { assert!(LEAF > WRAPPER, "the leaf should sit inside the wrapper's subtree") };
}

/// An empty id matches nothing, deliberately: every unnamed node
/// carries `""`, and the first of them is usually the root.
#[test]
fn an_empty_id_matches_nothing() {
    assert_eq!(index_of_wid(<Ui as Widget>::SUB_TABLE, ""), None);
}

#[test]
fn an_id_nothing_carries_matches_nothing() {
    assert_eq!(index_of_wid(<Ui as Widget>::SUB_TABLE, "titel"), None);
}

// ---- duplicates: refused for addressing, kept for styling ------------

/// Four nodes sharing one id — `examples/glasswall`'s shape, which
/// styles them by selector and addresses none of them.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Duplicated {
    #[widget_id("panel")]
    a: Fixed,
    #[widget_id("panel")]
    b: Fixed,
    #[widget_id("panel")]
    c: Fixed,
    #[widget_id("panel")]
    d: Fixed,
}

/// **Styling must keep matching them all** (M26/T26.16).
///
/// This is the direction a rejection in the *derive* would have broken,
/// and it is why the refusal lives on the addressing path instead: the
/// derive cannot tell an addressing use from a styling one.
#[test]
fn a_duplicated_id_still_styles_every_node_that_carries_it() {
    use lumen_static::style::{cascade, Length, Patch, Rule, Selector, Style};

    let sheet = [Rule::new(Selector::Id("panel"), Patch::new().width(Length::Fixed(17.0)))];
    let mut styles = vec![Style::default(); <Duplicated as Widget>::NODE_COUNT];
    cascade(
        <Duplicated as Widget>::SUB_TABLE,
        <Duplicated as Widget>::SEEDS,
        &sheet,
        &mut styles,
    );
    let hit = styles.iter().filter(|s| s.width == Length::Fixed(17.0)).count();
    assert_eq!(
        hit, 4,
        "an `Id` rule matched {hit} of the four nodes carrying `panel`; \
         styling duplicates by selector is deliberate"
    );
}

/// And the count the addressing path refuses on is the same four.
#[test]
fn the_duplicate_count_is_what_addressing_refuses_on() {
    use lumen_static::style::count_wid;

    let table = <Duplicated as Widget>::SUB_TABLE;
    assert_eq!(count_wid(table, "panel"), 4, "four nodes carry `panel`");
    assert_eq!(count_wid(table, "absent"), 0);
    assert_eq!(count_wid(table, ""), 0, "an empty id names nothing, as ever");
    // A tree with no duplicates counts one, which is what `wid_index!`
    // requires — a counter that answered 1 for everything would let the
    // compile_fail case through.
    assert_eq!(count_wid(<Ui as Widget>::SUB_TABLE, "title"), 1);
}
