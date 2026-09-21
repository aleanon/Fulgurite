//! M27/T27.3 — `W1011`: two nodes with the same author id.
//!
//! An author id is the agent-facing stable address *and* an `#id`
//! selector for the cascade. Two nodes sharing one makes **both**
//! unaddressable: `ui.resolve` refuses the ambiguity and every
//! `input.*` that takes a selector refuses with it.
//!
//! It is easy to write by accident, because an id lives on a *type's*
//! field: any composite instantiated twice duplicates every id inside
//! it. `examples/showcase` had eight, across three demos, from three
//! `Pair*` types used two and three times each. Nothing caught it — the
//! sheet rules styled both copies, which looks right — until the first
//! run of `just widget-drive` could not address either checkbox.

use lumen_static_widgets::ops::Label;
use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};
use lumen_static_agent::{Agent, TreeMut, TreeRef};
use serde_json::{json, Value};

/// A composite with an id inside it — the shape that makes this easy to
/// write by accident.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Pair {
    #[widget_id("cell")]
    cell: Label,
}

/// Used twice: both `cell`s answer to `#cell`.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Twice {
    a: Pair,
    b: Pair,
}

/// The same shape with distinct ids — the accepting direction. A gate
/// that refuses bad input must also accept good input.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct PairB {
    #[widget_id("cell2")]
    cell: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Once {
    a: Pair,
    b: PairB,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 200.0 };

fn findings<W: Widget + lumen_static::widget::Node + Default>() -> Vec<Value> {
    let mut app = W::default();
    let styles = pass::default_styles(&app);
    let mut states = vec![NodeState::default(); W::NODE_COUNT];
    pass::layout(&mut app, &styles, &mut states, VIEW);
    let mut agent = Agent::new();
    {
        let mut tree = TreeMut::of_root(&mut app, &styles, &states);
        agent.frame_done(&mut tree, lumen_static_agent::FrameTiming::passes(1000, 500));
    }
    let tree = TreeRef::of_root(&app, &styles, &states);
    let resp = agent.dispatch(
        &tree,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.lint", "params": {} }),
    );
    resp["result"]["findings"].as_array().cloned().unwrap_or_default()
}

fn dupes(f: &[Value]) -> Vec<&Value> {
    f.iter().filter(|x| x["code"] == "W1011").collect()
}

#[test]
fn a_duplicated_author_id_is_reported() {
    let f = findings::<Twice>();
    let d = dupes(&f);
    assert_eq!(d.len(), 1, "expected one W1011, got {}: {f:?}", d.len());
    let msg = d[0]["message"].as_str().unwrap();
    assert!(msg.contains("cell"), "the finding does not name the id: {msg}");
    // It names *both* nodes, or a reader cannot find the second one.
    assert!(
        d[0]["detail"]["first"].as_u64().is_some(),
        "the finding does not say which node holds the first copy: {:?}",
        d[0]
    );
}

/// And the other direction, in the same session.
#[test]
fn distinct_ids_are_not_reported() {
    let f = findings::<Once>();
    assert!(dupes(&f).is_empty(), "W1011 fired on a tree with no duplicate id: {f:?}");
}

/// The consequence the lint is about, asserted rather than described:
/// a duplicated id cannot be resolved, so neither copy is addressable.
#[test]
fn neither_copy_can_be_addressed() {
    let mut app = Twice::default();
    let styles = pass::default_styles(&app);
    let mut states = vec![NodeState::default(); Twice::NODE_COUNT];
    pass::layout(&mut app, &styles, &mut states, VIEW);
    let tree = TreeRef::of_root(&app, &styles, &states);
    let agent = Agent::new();
    let resp = agent.dispatch(
        &tree,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.resolve",
                 "params": { "selector": "#cell" } }),
    );
    let err = resp.get("error").unwrap_or_else(|| panic!("#cell resolved: {resp}"));
    assert!(
        err["message"].as_str().unwrap().contains("matched 2"),
        "expected an ambiguity, got {err:?}"
    );
}

/// **Scoped since M27/T27.0's third landing.** Two copies under
/// *different* owners are both reachable — `#a > #cell` and
/// `#b > #cell` — so reporting them would be the lint crying wolf at
/// exactly the pattern the descendant step exists to support.
///
/// Without this the lint fires on every composite used twice, which is
/// every `Chooser`, `Tooltip` and `SearchField` in any real tree.
#[test]
fn copies_under_different_owners_are_not_reported() {
    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct Owned {
        #[widget_id("a")]
        a: Pair,
        #[widget_id("b")]
        b: Pair,
    }

    let f = findings::<Owned>();
    assert!(
        dupes(&f).is_empty(),
        "W1011 fired on two copies that a descendant step can separate: {f:?}"
    );
}

/// And the other direction, so the scoping is not just the check turned
/// off: two copies under the *same* owner are still unaddressable,
/// because no step can tell them apart.
#[test]
fn two_copies_under_one_owner_are_still_reported() {
    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct Both {
        a: Pair,
        b: Pair,
    }

    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct Owner {
        #[widget_id("only")]
        only: Both,
    }

    let f = findings::<Owner>();
    assert_eq!(
        dupes(&f).len(), 1,
        "two `#cell`s share the owner `#only` and neither is addressable: {f:?}"
    );
}
