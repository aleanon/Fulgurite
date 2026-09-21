//! P2 tests: the state-diff answer to "what changed" — geometry vs
//! content classification, coalescing across frames, ring truncation,
//! structural (hot-reload) resets, and damage rects.

use lumen_static_widgets::ops::{Fixed, Label, Spacer};
use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};
use lumen_static_agent::{Agent, TreeMut, TreeRef};
use serde_json::{json, Value};

#[derive(StaticTree, Default)]
#[widget(row, spacing = 2.0)]
struct Bar {
    #[widget_id("title")]
    title: Label,
    pad: Spacer,
    #[widget_id("badge")]
    badge: Fixed,
}

/// A structurally different tree, for the reset test.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Other {
    a: Label,
    b: Label,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 600.0 };

struct Fixture {
    app: Bar,
    states: [NodeState; Bar::NODE_COUNT],
    styles: Vec<lumen_static::Style>,
    agent: Agent,
}

impl Fixture {
    fn new() -> Self {
        let mut app = Bar::default();
        app.title.set("hello");
        let styles = pass::default_styles(&app);
        let mut f = Fixture {
            app,
            states: [NodeState::default(); Bar::NODE_COUNT],
            styles,
            agent: Agent::new(),
        };
        f.frame(); // seeds the baseline
        f
    }

    /// Lay out and record one frame.
    fn frame(&mut self) {
        pass::layout(&mut self.app, &self.styles, &mut self.states, VIEW);
        let mut tree = TreeMut::of_root(&mut self.app, &self.styles, &self.states);
        self.agent.frame_done(&mut tree, lumen_static_agent::FrameTiming::passes(1000, 500));
    }

    fn result(&self, method: &str, params: Value) -> Value {
        let tree = TreeRef::of_root(&self.app, &self.styles, &self.states);
        let resp = self.agent.dispatch(
            &tree,
            &json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }),
        );
        assert!(resp.get("error").is_none(), "unexpected error: {resp}");
        resp["result"].clone()
    }

    fn changed(&self, params: Value) -> Vec<Value> {
        self.result("ui.lastChange", params)["changed"].as_array().unwrap().clone()
    }
}

fn find<'a>(changed: &'a [Value], wid: &str) -> Option<&'a Value> {
    changed.iter().find(|c| c["wid"] == wid)
}

#[test]
fn seed_frame_reports_nothing_and_a_still_frame_stays_quiet() {
    let mut f = Fixture::new();
    assert!(f.changed(json!({})).is_empty(), "seeding is not a change");
    f.frame();
    assert!(f.changed(json!({})).is_empty(), "an unchanged frame reports nothing");
}

#[test]
fn content_change_without_geometry_is_classified_as_content() {
    let mut f = Fixture::new();
    // Same length, so the label's measured size is identical — only the
    // paint checksum can see this. A geometry-only diff would miss it.
    f.app.title.set("world");
    f.frame();

    let changed = f.changed(json!({}));
    let title = find(&changed, "title").expect("title reported");
    assert_eq!(title["change"], "content");
    assert_eq!(title["old"]["size"], title["new"]["size"]);
    assert_eq!(title["kind"], "label");
    assert!(title["src"].as_str().unwrap().contains("tests/changes.rs"));
}

#[test]
fn geometry_change_reports_old_and_new() {
    let mut f = Fixture::new();
    let before = f.states[3].size.w; // badge is the third field
    f.app.badge.w = before + 40.0;
    f.frame();

    let changed = f.changed(json!({}));
    let badge = find(&changed, "badge").expect("badge reported");
    assert_eq!(badge["change"], "geometry");
    assert_eq!(badge["old"]["size"][0].as_f64().unwrap() as f32, before);
    assert_eq!(badge["new"]["size"][0].as_f64().unwrap() as f32, before + 40.0);
}

#[test]
fn coalesces_across_frames_keeping_oldest_old_and_newest_new() {
    let mut f = Fixture::new();
    let start = f.states[3].size.w;
    let at_seed = f.agent.frame;

    f.app.badge.w = start + 10.0;
    f.frame();
    f.app.badge.w = start + 25.0;
    f.frame();

    // Since the seed: one entry spanning both steps.
    let changed = f.changed(json!({ "since": at_seed }));
    let badge = find(&changed, "badge").expect("badge reported");
    assert_eq!(badge["old"]["size"][0].as_f64().unwrap() as f32, start);
    assert_eq!(badge["new"]["size"][0].as_f64().unwrap() as f32, start + 25.0);
    assert_eq!(changed.iter().filter(|c| c["wid"] == "badge").count(), 1, "coalesced");

    // Since the previous frame only: the last step alone.
    let changed = f.changed(json!({ "since": f.agent.frame - 1 }));
    let badge = find(&changed, "badge").unwrap();
    assert_eq!(badge["old"]["size"][0].as_f64().unwrap() as f32, start + 10.0);
}

#[test]
fn truncation_is_reported_not_hidden() {
    let mut f = Fixture::new();
    // The ring holds 64 frames; run well past it.
    for i in 0..80 {
        f.app.badge.w = 20.0 + (i % 7) as f32;
        f.frame();
    }
    let r = f.result("ui.lastChange", json!({ "since": 0 }));
    assert_eq!(r["truncated"], true, "history no longer reaches frame 0");
    assert!(r["oldest_frame"].as_u64().unwrap() > 1);

    let r = f.result("ui.lastChange", json!({ "since": f.agent.frame - 2 }));
    assert_eq!(r["truncated"], false, "recent history is complete");
}

#[test]
fn structural_change_resets_the_baseline() {
    let mut f = Fixture::new();
    f.frame();

    // Feed the same agent a structurally different tree — what a hot
    // reload or a tree swap looks like from the journal's side.
    let mut other = Other::default();
    let mut ost = [NodeState::default(); Other::NODE_COUNT];
    let osty = pass::default_styles(&other);
    pass::layout(&mut other, &osty, &mut ost, VIEW);
    {
        let mut otree = TreeMut::of_root(&mut other, &osty, &ost);
        f.agent.frame_done(&mut otree, lumen_static_agent::FrameTiming::passes(1, 1));
    }

    let otree = TreeRef::of_root(&other, &osty, &ost);
    let resp = f.agent.dispatch(
        &otree,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.lastChange", "params": {} }),
    );
    let r = &resp["result"];
    assert_eq!(r["reset"], true, "the discontinuity is announced");
    assert!(
        r["changed"].as_array().unwrap().is_empty(),
        "indices across a reset are not comparable, so nothing is claimed"
    );
}

#[test]
fn damage_unions_old_and_new_bounds() {
    let mut f = Fixture::new();
    assert!(f.result("ui.lastDamage", json!({}))["rect"].is_null(), "quiet frame, no damage");

    f.app.badge.w += 40.0;
    f.frame();
    let d = f.result("ui.lastDamage", json!({}));
    let rect = d["rect"].as_array().expect("damage rect");
    let (x0, x1) = (rect[0].as_f64().unwrap(), rect[2].as_f64().unwrap());
    assert!(x1 > x0 && d["nodes"].as_u64().unwrap() >= 1);

    // The union must cover the badge's new right edge.
    let badge = &f.states[3];
    assert!(x1 >= (badge.pos.x + badge.size.w) as f64 - 0.001);
}
