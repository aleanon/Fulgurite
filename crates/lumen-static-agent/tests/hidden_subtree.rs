//! M24/T24.3 — the agent must not paint or probe inside a hidden subtree.
//!
//! **This was a release-build panic**, and nothing in the suite saw it:
//! `just check` was green, 134 test binaries were green, and the window
//! died the moment a dropdown opened over a stage whose demo was an enum
//! switch.
//!
//! `Node::node(i)` answers with the *container* for an index inside a
//! branch `layout` zeroed — deliberately, so the agent can walk those
//! indices and report them. The journal and the auditor both took that
//! stand-in and used it **at `i`**: painting or probing it lays its own
//! children out from `i` rather than from the container's index, so the
//! deeper the hidden node the further past the end of `states` it
//! writes.
//!
//! Every earlier tree escaped it by luck — no hidden subtree, or one
//! near enough the start of the table that the overrun stayed in bounds
//! and quietly hashed the wrong node.

use lumen_static::ops::{Fixed, Label, Spacer};
use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};
use lumen_static_agent::{Agent, FrameTiming, TreeMut, TreeRef};
use serde_json::json;

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 300.0 };

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Deep {
    #[widget_id("d1")]
    a: Label,
    #[widget_id("d2")]
    b: Label,
    #[widget_id("d3")]
    c: Fixed,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Shallow {
    #[widget_id("s1")]
    a: Label,
}

/// The variant switch, with the **big** variant second so its indices
/// run to the very end of the table. That placement is the test: a
/// stand-in painted at a *low* hidden index overruns into nodes that
/// exist, and nothing notices.
#[derive(StaticTree)]
#[allow(dead_code)] // `Big` is never built: staying hidden is the point.
enum Stage {
    Small(Shallow),
    Big(Deep),
}

impl Default for Stage {
    fn default() -> Self {
        Stage::Small(Shallow::default())
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("head")]
    head: Label,
    pad: Spacer,
    #[widget_id("stage")]
    stage: Stage,
}

fn run_a_frame() {
    let mut app = Ui::default();
    app.head.set("title");
    if let Stage::Small(s) = &mut app.stage {
        s.a.set("showing");
    }
    let styles = pass::default_styles(&app);
    let mut states = [NodeState::default(); Ui::NODE_COUNT];
    pass::layout(&mut app, &styles, &mut states, VIEW);

    // The premise: the inactive variant really is hidden, and its
    // indices really do run to the end of the table. Without both this
    // test cannot exercise the overrun and would pass on the broken
    // code.
    let last = Ui::NODE_COUNT - 1;
    let mut live = Vec::new();
    lumen_static_agent::live_nodes(&app, <Ui as Widget>::SUB_TABLE, &mut live);
    assert!(!live[last], "setup: the last node is not inside a hidden subtree");
    assert!(live[1], "setup: a visible node was reported hidden");
    assert!(live[0], "setup: the root was reported hidden");
    println!("{} of {} nodes live", live.iter().filter(|x| **x).count(), live.len());

    let mut agent = Agent::new();
    // The journal (`ui.lastChange`) and the auditor (`ui.lint`) both run
    // here — the ambient per-frame work M23/P3 made unconditional.
    let mut tree = TreeMut::of_root(&mut app, &styles, &states);
    agent.frame_done(&mut tree, FrameTiming::passes(1000, 500));

    let tree = TreeRef::of_root(&app, &styles, &states);
    let resp = agent.dispatch(
        &tree,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.lint", "params": {} }),
    );
    assert!(resp.get("error").is_none(), "ui.lint errored: {resp}");
    println!(
        "{} nodes, last hidden at {last}; lint returned {} findings",
        Ui::NODE_COUNT,
        resp["result"]["findings"].as_array().map_or(0, |a| a.len())
    );
}

/// The whole test is that this returns.
#[test]
fn a_frame_with_a_hidden_subtree_at_the_end_does_not_panic() {
    run_a_frame();
}

/// And the report still reaches the hidden nodes — the guard must not
/// have been bought by making the agent blind to them, which is the
/// obvious wrong fix.
#[test]
fn the_hidden_nodes_are_still_in_the_tree_report() {
    let mut app = Ui::default();
    let styles = pass::default_styles(&app);
    let mut states = [NodeState::default(); Ui::NODE_COUNT];
    pass::layout(&mut app, &styles, &mut states, VIEW);
    let agent = Agent::new();
    let tree = TreeRef::of_root(&app, &styles, &states);
    let resp = agent.dispatch(
        &tree,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.getTree", "params": { "depth": 8 } }),
    );
    let nodes = resp["result"]["nodes"].as_array().expect("a tree");
    let wids: Vec<&str> = nodes.iter().filter_map(|n| n["wid"].as_str()).collect();
    println!("{} nodes reported: {wids:?}", nodes.len());
    for want in ["d1", "d2", "d3"] {
        assert!(wids.contains(&want), "{want} — inside the hidden variant — is not reported");
    }
}

/// M26/T26.2: and they report **their own** kind, not their container's.
///
/// `Node::node(i)` answers for a hidden index with the container standing
/// in for it, so a kind read off the instance is the container's — the
/// defect M24/T24.1 fixed in the cascade, in the reporting surface next
/// door. Kinds come from `Widget::SEEDS`, which is per index and cannot
/// vary per instance.
#[test]
fn a_hidden_node_reports_its_own_kind() {
    let mut app = Ui::default();
    let styles = pass::default_styles(&app);
    let mut states = [NodeState::default(); Ui::NODE_COUNT];
    // **Laid out first**, as the test above does. `ui.getTree` walks the
    // frame, and a tree that was never laid out reports no nodes at all:
    // the first version of this test omitted this and read `<missing>`
    // for every id, catching its own setup rather than the defect.
    pass::layout(&mut app, &styles, &mut states, VIEW);
    let agent = Agent::new();
    let tree = TreeRef::of_root(&app, &styles, &states);
    let resp = agent.dispatch(
        &tree,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.getTree", "params": { "depth": 8 } }),
    );
    // **`ui.resolve`, not `ui.getTree`.** The tree dump reports `role`,
    // `wid` and geometry and no kind at all, so a kind assertion against
    // it reads `null` for every node and catches nothing — which is what
    // the first version of this test did.
    let _ = resp;
    let kind_of = |wid: &str| -> String {
        let r = agent.dispatch(
            &tree,
            &json!({ "jsonrpc": "2.0", "id": 2, "method": "ui.resolve",
                     "params": { "selector": format!("#{wid}") } }),
        );
        println!("resolve #{wid}: {r}");
        r["result"]["kind"].as_str().unwrap_or("<missing>").to_owned()
    };

    // Setup: #d3 really is inside the inactive variant.
    let mut live = Vec::new();
    lumen_static_agent::live_nodes(&app, <Ui as Widget>::SUB_TABLE, &mut live);
    let hidden = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "d3")
        .expect("#d3 exists in the table");
    assert!(!live[hidden], "setup: #d3 is not inside a hidden subtree");

    assert_eq!(kind_of("d1"), "label", "a hidden label reported its container's kind");
    assert_eq!(kind_of("d3"), "fixed", "a hidden Fixed reported its container's kind");

    // A gate that refuses bad input must still accept good input: the
    // visible side is unchanged.
    assert_eq!(kind_of("head"), "label", "a visible label's kind changed");
}
