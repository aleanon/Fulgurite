//! `W1006` — "the frame is blank" — must not fire on a frame that is
//! full of content.
//!
//! `audit.rs`'s own module doc calls this class of false positive "the
//! first thing an agent ever sees" and the reason the intrinsic probe
//! exists. But `collect` walks the table and treats a node as a leaf
//! only when `child_count == 0 && !is_container()`, and every off-table
//! container carries `CONTAINER` — so nothing inside a `Repeat`,
//! `Either`, `List` or `Dyn` is ever examined, and `anything_visible`
//! stays false unless a `stats()` impl happens to vouch for it. `List`
//! escapes by accident because it implements `stats`; `Either` and `Dyn`
//! do not (`docs/plan-review-fixes.md` F4.4).

use lumen_static::hatch::{Dyn, Either};
use lumen_static::ops::{Fixed, Label};
use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};
use lumen_static_agent::{Agent, TreeMut, TreeRef};
use serde_json::json;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Pane {
    a: Label,
    b: Fixed,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct WithEither {
    tabs: Either<Pane, Pane>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct WithDyn {
    island: Dyn,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 400.0 };

fn lint_codes<T: Widget + lumen_static::widget::Node>(ui: &mut T) -> Vec<String> {
    lint_codes_in(ui, VIEW)
}

fn lint_codes_in<T: Widget + lumen_static::widget::Node>(
    ui: &mut T,
    view: Constraints,
) -> Vec<String> {
    let styles = pass::default_styles(ui);
    let mut st = vec![NodeState::default(); T::NODE_COUNT];
    pass::layout(ui, &styles, &mut st, view);

    let mut agent = Agent::new();
    {
        let mut tree = TreeMut::of_root(ui, &styles, &st);
        agent.frame_done(&mut tree, lumen_static_agent::FrameTiming::passes(1, 1));
    }
    let tree = TreeRef::of_root(ui, &styles, &st);
    let r = agent.dispatch(
        &tree,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.lint", "params": {} }),
    );
    r["result"]["findings"]
        .as_array()
        .map(|a| a.iter().filter_map(|f| f["code"].as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

#[test]
fn an_either_holding_visible_content_is_not_reported_blank() {
    let mut ui = WithEither { tabs: Either::new(Pane::default(), Pane::default()) };
    ui.tabs.a.a.set("hello world");
    ui.tabs.a.b = Fixed { w: 40.0, h: 20.0 };
    let codes = lint_codes(&mut ui);
    assert!(
        !codes.iter().any(|c| c == "W1006"),
        "the Either's branch is laid out and painted; got {codes:?}"
    );
}

#[test]
fn a_dyn_island_holding_visible_content_is_not_reported_blank() {
    let mut pane = Pane::default();
    pane.a.set("hello world");
    pane.b = Fixed { w: 40.0, h: 20.0 };
    let mut ui = WithDyn { island: Dyn::new(pane) };
    let codes = lint_codes(&mut ui);
    assert!(!codes.iter().any(|c| c == "W1006"), "got {codes:?}");
}

/// A pane with no text: genuinely zero-sized, unlike `Pane`, whose
/// `Label` measures a line height even when empty.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct EmptyPane {
    a: Fixed,
    b: Fixed,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct WithEmptyEither {
    tabs: Either<EmptyPane, EmptyPane>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct PlainEmpty {
    a: Fixed,
    b: Fixed,
}

/// The lint must still fire on a frame that really is blank — otherwise
/// the suppression above has simply disabled it.
#[test]
fn a_genuinely_blank_frame_still_reports_itself() {
    // A zero-height viewport, as `lint.rs`'s own blank test uses: the
    // lint is about a frame with nothing in it, and that is how the
    // suite has always spelled one.
    let mut ui = PlainEmpty::default();
    let codes = lint_codes_in(&mut ui, Constraints { max_h: 0.0, ..VIEW });
    assert!(
        codes.iter().any(|c| c == "W1006"),
        "nothing has area or content; got {codes:?}"
    );
}

/// And the suppression is gated on *area*: an arena container the walk
/// cannot see into suppresses the claim only while it occupies space. A
/// zero-area one hides nothing, so the frame is blank and says so.
#[test]
fn an_arena_container_with_no_area_does_not_suppress_the_lint() {
    let mut ui = WithEmptyEither { tabs: Either::new(EmptyPane::default(), EmptyPane::default()) };
    let codes = lint_codes_in(&mut ui, Constraints { max_h: 0.0, ..VIEW });
    assert!(
        codes.iter().any(|c| c == "W1006"),
        "a zero-area Either conceals nothing; got {codes:?}"
    );
}

/// `ui.probe` and `ui.getTree` must agree on a node's id.
///
/// They computed it from two hand-written copies of the same bit layout
/// — four copies in all, across three containers and this crate — under
/// a comment citing a Repeat method deleted in M0.
/// Nothing checked they agreed, and when they disagree an agent that
/// probes a point and then looks the node up in the tree gets two
/// different answers (F4.4).
#[test]
fn probe_and_the_snapshot_agree_on_a_repeater_row_id() {
    use lumen_static::ops::Spacer;
    use lumen_static::repeat::{noop_bind, Repeat};

    #[derive(StaticTree, Default)]
    #[widget(row, spacing = 0.0)]
    struct RowT {
        ink: Fixed,
        pad: Spacer,
    }

    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct App {
        list: Repeat<RowT>,
    }

    let mut ui = App { list: Repeat::new(100, 20.0, noop_bind) };
    let styles = pass::default_styles(&ui);
    let mut st = [NodeState::default(); App::NODE_COUNT];
    pass::layout(&mut ui, &styles, &mut st, VIEW);
    lumen_static::widget::Node::place(&mut ui, lumen_static::Point::default(), &styles, &mut st, 0);

    let agent = Agent::new();
    let tree = TreeRef::of_root(&ui, &styles, &st);

    // Probe a point inside the third row.
    let y = 2.5 * 20.0;
    let probe = agent.dispatch(
        &tree,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.probe",
                 "params": { "x": 5.0, "y": y } }),
    );
    let probed_id = probe["result"]["id"].as_u64().expect("probe found a node");
    let row = probe["result"]["row"].as_u64().expect("and it is inside a row");

    let inner = probe["result"]["inner"].as_u64().expect("and names a node within it") as usize;

    // The repeater is table node 1, so the snapshot ids its rows against
    // node 2 (AccessKit reserves 0). Rebuild the id the way the snapshot
    // does and require the same answer.
    let expected = lumen_static::observe::instance_id(2, row, inner);
    assert_eq!(probed_id, expected, "probe and snapshot must agree on the id");
    assert_eq!(row, 2, "the point was inside the third row");
}
