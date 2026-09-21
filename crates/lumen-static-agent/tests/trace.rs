//! M12/T12.4: a failure trace that answers the question instead of
//! inviting a re-run.

use lumen_static_widgets::ops::{Button, Label};
use lumen_static::style::{Patch, Rule, Selector};
use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};
use lumen_static_agent::trace::Trace;
use lumen_static_agent::{Agent, TreeRef};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("head")]
    head: Label,
    #[widget_id("save")]
    save: Button<()>,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 200.0 };

fn fixture(hide_save: bool) -> (Ui, Vec<lumen_static::Style>, [NodeState; Ui::NODE_COUNT]) {
    let mut ui = Ui::default();
    ui.head.set("head");
    ui.save.label = "save";
    let mut styles = pass::default_styles(&ui);
    if hide_save {
        lumen_static::style::cascade_deep(
            &mut ui,
            &[Rule::new(Selector::Id("save"), Patch::new().visible(false))],
            &mut styles,
        );
    }
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    pass::layout(&mut ui, &styles, &mut st, VIEW);
    (ui, styles, st)
}

#[test]
fn a_trace_answers_why_the_click_did_not_land() {
    // The commonest UI test failure there is. The trace carries the
    // answer, so reading it replaces re-running.
    let (ui, styles, st) = fixture(true);
    let tree = TreeRef::of_root(&ui, &styles, &st);
    let agent = Agent::new();

    let mut t = Trace::new("clicking_save_saves", "expected 1 save, got 0");
    t.collect(&agent, &tree, Some("#save"));

    let text = t.summary();
    assert!(text.contains("clicking_save_saves"), "{text}");
    assert!(text.contains("does NOT reach"), "the summary says why: {text}");
    assert!(text.contains("visible"), "and names the cause: {text}");
}

#[test]
fn the_jsonl_carries_every_method_including_the_ones_that_errored() {
    // A method that failed is itself a fact about the frame; swallowing
    // it leaves a gap exactly where the reader is looking.
    let (ui, styles, st) = fixture(false);
    let tree = TreeRef::of_root(&ui, &styles, &st);
    let agent = Agent::new();

    let mut t = Trace::new("t", "boom");
    t.collect(&agent, &tree, Some("#nope"));
    let jsonl = t.to_jsonl();
    let lines: Vec<&str> = jsonl.lines().collect();
    assert_eq!(lines.len(), t.len() + 1, "a header line plus one per method");
    let joined = lines.join("\n");
    assert!(joined.contains("ui.explain"), "{joined}");
    assert!(joined.contains("\"failure\":\"boom\""), "{joined}");
    // `#nope` resolves to nothing, and the error is recorded rather than
    // dropped.
    assert!(joined.contains("nearest") || joined.contains("code"), "the error is kept: {joined}");
}

#[test]
fn a_trace_without_a_selector_still_collects_the_frame() {
    let (ui, styles, st) = fixture(false);
    let tree = TreeRef::of_root(&ui, &styles, &st);
    let agent = Agent::new();
    let mut t = Trace::new("t", "no selector involved");
    t.collect(&agent, &tree, None);
    let j = t.to_jsonl();
    assert!(!j.contains("ui.explain"), "nothing to explain");
    assert!(j.contains("ui.lint") && j.contains("app.diagnostics"), "{j}");
}

#[test]
fn a_healthy_frame_produces_a_short_summary() {
    // The summary is the part a reader sees first, and on a frame with
    // nothing wrong it should not bury the failure message.
    let (ui, styles, st) = fixture(false);
    let tree = TreeRef::of_root(&ui, &styles, &st);
    let agent = Agent::new();
    let mut t = Trace::new("t", "assertion failed: n == 1");
    t.collect(&agent, &tree, Some("#save"));
    let s = t.summary();
    assert!(s.contains("assertion failed"), "{s}");
    assert!(s.contains("reaches this node"), "it says the control was fine: {s}");
    assert!(s.lines().count() <= 4, "short: {s}");
}
