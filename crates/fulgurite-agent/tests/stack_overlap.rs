//! W1004 — sibling overlap — M11/T11.6.
//!
//! The code was reserved for years on the grounds that overlap was
//! "structurally impossible while containers place children sequentially
//! by measured size". `#[widget(stack)]` is the layout mode that makes
//! it possible, so the code ships with it.
//!
//! **Both directions, in one file, because the hard half is the quiet
//! one.** A stack's children overlap *on purpose*; a code that fired on
//! every stack would be noise, and noise is how a diagnostic stops being
//! read. So there is a firing case and a quiet case, and the quiet case
//! is the one this feature could plausibly have got wrong.
//!
//! The firing case pokes the node states directly rather than building a
//! layout that overlaps. That is deliberate: a *sequential* container
//! still cannot produce overlap — `lint.rs`'s
//! `overflow_is_impossible_here_...` pins exactly that and still passes
//! — so the only way to hand the detector its input is to hand it the
//! input. The lint reads `NodeState`; this gives it overlapping ones.

use fulgurite_widgets::ops::Fixed;
use fulgurite::{pass, Constraints, NodeState, StaticTree, Widget};
use fulgurite_agent::{Agent, TreeMut, TreeRef};
use serde_json::{json, Value};

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Seq {
    a: Fixed,
    b: Fixed,
}

#[derive(StaticTree, Default)]
#[widget(stack)]
struct Layered {
    under: Fixed,
    over: Fixed,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct SeqApp {
    row: Seq,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct StackApp {
    panel: Layered,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 300.0 };

/// Run one frame through **one** agent and read its findings.
///
/// The first version of this helper built a *fresh* `Agent` to dispatch
/// `ui.lint`, so it always answered with an empty list — and the two
/// "quiet" tests below passed without the audit having run at all. That
/// is `AGENT.md`'s first vacuous shape, caught here only because the
/// firing case sits in the same file and failed. A quiet assertion needs
/// a firing one beside it or it is checking nothing.
fn codes<W: fulgurite::Node + Widget>(
    app: &mut W,
    styles: &[fulgurite::Style],
    states: &[NodeState],
) -> Vec<String> {
    let mut agent = Agent::new();
    {
        let mut tree = TreeMut::of_root(app, styles, states);
        agent.frame_done(&mut tree, fulgurite_agent::FrameTiming::passes(10, 10));
    }
    let tree = TreeRef::of_root(app, styles, states);
    let resp = agent.dispatch(
        &tree,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.lint", "params": {} }),
    );
    assert!(resp.get("error").is_none(), "unexpected error: {resp}");
    let found = resp["result"]["findings"].as_array().unwrap().clone();
    found.iter().map(|f: &Value| f["code"].as_str().unwrap().to_owned()).collect()
}

/// A stack's children overlap completely, and that must be silent.
#[test]
fn a_stack_does_not_report_overlap() {
    let mut app = StackApp::default();
    app.panel.under.w = 120.0;
    app.panel.under.h = 60.0;
    app.panel.over.w = 90.0;
    app.panel.over.h = 40.0;
    let styles = pass::default_styles(&app);
    let mut states = [NodeState::default(); StackApp::NODE_COUNT];
    pass::layout(&mut app, &styles, &mut states, VIEW);

    // Assert the setup happened: if these did not actually overlap the
    // quiet result would prove nothing at all.
    let (u, o) = (rect(&states[2]), rect(&states[3]));
    assert!(
        overlap_area(u, o) > 100.0,
        "the stack's children do not overlap, so this proves nothing: {u:?} {o:?}"
    );

    let quiet = codes(&mut app, &styles, &states);
    assert!(!quiet.iter().any(|c| c == "W1004"), "a stack must be exempt: {quiet:?}");

    // **The control, in this test rather than only in the next one.**
    // "No W1004" is also what a lint that never ran says, and that is
    // exactly how the first version of this file passed. So the same
    // geometry goes through a *sequential* container, and must report:
    // if this half stops firing, the exemption above is proving nothing
    // and the reader finds out here rather than three refactors later.
    let mut ctrl = SeqApp::default();
    ctrl.row.a.w = 120.0;
    ctrl.row.a.h = 60.0;
    ctrl.row.b.w = 90.0;
    ctrl.row.b.h = 40.0;
    let cs = pass::default_styles(&ctrl);
    let mut cst = [NodeState::default(); SeqApp::NODE_COUNT];
    pass::layout(&mut ctrl, &cs, &mut cst, VIEW);
    cst[3].pos = states[3].pos;
    cst[3].size = states[3].size;
    let loud = codes(&mut ctrl, &cs, &cst);
    assert!(
        loud.iter().any(|c| c == "W1004"),
        "the control did not fire, so the exemption above proves nothing: {loud:?}"
    );
}

/// A sequential container whose children collide must report.
#[test]
fn a_row_with_colliding_children_reports_overlap() {
    let mut app = SeqApp::default();
    app.row.a.w = 100.0;
    app.row.a.h = 50.0;
    app.row.b.w = 100.0;
    app.row.b.h = 50.0;
    let styles = pass::default_styles(&app);
    let mut states = [NodeState::default(); SeqApp::NODE_COUNT];
    pass::layout(&mut app, &styles, &mut states, VIEW);

    // Sequential placement put `b` at x = 100. Drag it back over `a`,
    // which is the state a layout mode with overlap could produce and
    // this one cannot.
    let before = overlap_area(rect(&states[2]), rect(&states[3]));
    assert_eq!(before, 0.0, "a row must not overlap on its own: that is W1003's invariant");
    states[3].pos.x = 40.0;
    assert!(overlap_area(rect(&states[2]), rect(&states[3])) > 100.0, "the poke did not land");

    let codes = codes(&mut app, &styles, &states);
    assert!(codes.iter().any(|c| c == "W1004"), "expected W1004: {codes:?}");
}

/// Sharing an edge is what sequential placement *does*; it is not an
/// overlap and must stay quiet.
#[test]
fn touching_edges_are_not_an_overlap() {
    let mut app = SeqApp::default();
    app.row.a.w = 100.0;
    app.row.a.h = 50.0;
    app.row.b.w = 100.0;
    app.row.b.h = 50.0;
    let styles = pass::default_styles(&app);
    let mut states = [NodeState::default(); SeqApp::NODE_COUNT];
    pass::layout(&mut app, &styles, &mut states, VIEW);
    assert_eq!(states[3].pos.x, 100.0, "the two should share an edge at x = 100");

    let codes = codes(&mut app, &styles, &states);
    assert!(!codes.iter().any(|c| c == "W1004"), "a shared edge is not an overlap: {codes:?}");
}

fn rect(s: &NodeState) -> [f32; 4] {
    [s.pos.x, s.pos.y, s.pos.x + s.size.w, s.pos.y + s.size.h]
}

fn overlap_area(a: [f32; 4], b: [f32; 4]) -> f32 {
    let w = (a[2].min(b[2]) - a[0].max(b[0])).max(0.0);
    let h = (a[3].min(b[3]) - a[1].max(b[1])).max(0.0);
    w * h
}
