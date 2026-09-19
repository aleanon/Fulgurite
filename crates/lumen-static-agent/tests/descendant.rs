//! M27/T27.0's third landing — `#outer > #inner`.
//!
//! **A part id lives on a *type's* table.** A composite instantiated
//! twice carries every id inside it twice, and `W1011` reports both as
//! unaddressable: `ui.resolve` refuses the ambiguity and so does every
//! `input.*` that takes a selector.
//!
//! That fired four times during M27 — `PairCheck`, `VeilBody`, the
//! tooltip's parts, and the three `Chooser` kinds — and the workaround
//! each time was to split the type. That does not scale, and it does
//! nothing for an application that legitimately wants two dropdowns.
//!
//! `#outer > #inner` says "the `#inner` inside `#outer`", so a part id
//! only has to be unique within its owner. The owner is addressed the
//! way it always was — by the `#[widget_id]` the *application* gave its
//! own field, which is unique because the application wrote it.
//!
//! `>` and not a space: a space already means "and", and changing that
//! would silently reinterpret every selector already written.

use lumen_static::ops::Label;
use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};
use lumen_static_agent::{Agent, TreeRef};
use serde_json::{json, Value};

/// A composite with a part id inside it — the shape that makes this
/// necessary.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Part {
    #[widget_id("part-label")]
    label: Label,
}

/// A level between the owner and the part, so **depth matters**.
///
/// The first version of this fixture had the part as a *direct* child,
/// and an injection that stopped the ancestor walk after one level would
/// have left every test green — the fixture could not tell "any
/// ancestor" from "the parent". A step has to reach a grandchild, and
/// only a tree with one can say so.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Mid {
    #[widget_id("mid")]
    mid: Part,
}

/// Used twice. Both `part-label`s answer to the same name.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("first")]
    first: Mid,
    #[widget_id("second")]
    second: Mid,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 200.0 };

fn resolve(sel: &str) -> Value {
    let mut app = Ui::default();
    app.first.mid.label.set("one");
    app.second.mid.label.set("two");
    let styles = pass::default_styles(&app);
    let mut st = vec![NodeState::default(); Ui::NODE_COUNT];
    pass::layout(&mut app, &styles, &mut st, VIEW);
    let tree = TreeRef::of_root(&app, &styles, &st);
    let agent = Agent::new();
    agent.dispatch(
        &tree,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.resolve",
                 "params": { "selector": sel } }),
    )
}

/// The problem, asserted — so the fix below is not a solution looking
/// for one.
#[test]
fn a_duplicated_part_id_is_ambiguous_on_its_own() {
    let r = resolve("#part-label");
    let err = r.get("error").unwrap_or_else(|| panic!("#part-label resolved: {r}"));
    assert!(
        err["message"].as_str().unwrap().contains("matched 2"),
        "expected an ambiguity, got {err:?}"
    );
}

/// And the fix.
#[test]
fn a_descendant_step_picks_the_one_inside_its_owner() {
    for (sel, want) in [("#first > #part-label", 3usize), ("#second > #part-label", 6)] {
        let r = resolve(sel);
        assert!(r.get("error").is_none(), "{sel} did not resolve: {r}");
        assert_eq!(
            r["result"]["index"], want,
            "{sel} resolved to node {}, not {want}",
            r["result"]["index"]
        );
    }
}

/// **The two answers differ**, or the test above would pass on an
/// implementation that ignored the step and returned the first match
/// twice.
#[test]
fn the_two_owners_give_two_different_nodes() {
    let a = resolve("#first > #part-label")["result"]["index"].clone();
    let b = resolve("#second > #part-label")["result"]["index"].clone();
    assert_ne!(a, b, "both owners resolved to node {a}");
}

/// A step is a **strict** descendant: the owner is not inside itself.
#[test]
fn an_owner_is_not_its_own_descendant() {
    let r = resolve("#first > #first");
    assert!(r.get("error").is_some(), "`#first > #first` resolved to {r}");
}

/// A part that is not in that owner is not found there — the accepting
/// gate's other direction.
#[test]
fn a_step_does_not_reach_outside_its_owner() {
    let r = resolve("#first > #second");
    assert!(
        r.get("error").is_some(),
        "`#first > #second` resolved, but `#second` is a sibling: {r}"
    );
}

/// **A step reaches a grandchild, not only a child.** This is the
/// clause a walk that stopped at the parent would fail, and the reason
/// the fixture has a level in the middle.
#[test]
fn a_step_reaches_any_depth_not_only_a_direct_child() {
    let r = resolve("#first > #part-label");
    assert!(
        r.get("error").is_none(),
        "`#part-label` is a *grandchild* of `#first`, and the step did not reach it: {r}"
    );
    // And the middle level is reachable from the same owner, so the
    // test above is not satisfied by a step that skipped exactly one.
    let m = resolve("#first > #mid");
    assert!(m.get("error").is_none(), "the middle level is unreachable: {m}");
    assert_ne!(
        m["result"]["index"], r["result"]["index"],
        "`#mid` and `#part-label` resolved to the same node"
    );
}

/// Steps chain, so a part inside a part is reachable.
#[test]
fn steps_chain() {
    let r = resolve("#first > #part-label");
    assert!(r.get("error").is_none(), "{r}");
    // And with whitespace around the step, because a selector is typed
    // by hand as often as it is generated.
    let chained = resolve("#first > #mid > #part-label");
    assert!(chained.get("error").is_none(), "a two-step selector failed: {chained}");
    assert_eq!(
        chained["result"]["index"], r["result"]["index"],
        "`#first > #mid > #part-label` and `#first > #part-label` disagree"
    );
    let spaced = resolve("#first>#part-label");
    assert_eq!(
        spaced["result"]["index"], r["result"]["index"],
        "`#first>#part-label` and `#first > #part-label` disagree"
    );
}
