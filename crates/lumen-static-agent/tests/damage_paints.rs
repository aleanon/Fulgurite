//! **`ui.lastDamage` is what a frame redraws, so a node that draws
//! nothing is not damage** — found by M56's widget drive
//! (`docs/plan-showcase-design.md` T56.4).
//!
//! The journal put the old and new box of every *changed* node into the
//! damage set, and a node changes when it moves. A spacer and a
//! transparent container move without a pixel changing: the renderer's
//! damage, taken from commands, never sees them. But the showcase's
//! centring stage has four spacers and a row, all as wide as the stage,
//! and when an accordion opened they merged into the stage's whole inner
//! box — 7.47x the demo's own box, on a frame whose real damage was
//! 1.00x. The move is still a change (`ui.lastChange` reports it); it is
//! no longer damage.

use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{Constraints, NodeState, StaticTree, Widget};
use lumen_static_agent::{Agent, TreeMut, TreeRef};
use lumen_static_widgets::ops::{Label, Spacer};
use serde_json::{json, Value};

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Line {
    #[widget_id("text")]
    text: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Page {
    #[widget_id("gap")]
    gap: Spacer,
    #[widget_id("line")]
    line: Line,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 600.0 };

struct Fixture {
    app: Page,
    states: [NodeState; Page::NODE_COUNT],
    styles: Vec<lumen_static::Style>,
    agent: Agent,
}

impl Fixture {
    fn new() -> Self {
        let mut app = Page::default();
        app.line.text.set("moved");
        let mut f = Fixture {
            app,
            states: [NodeState::default(); Page::NODE_COUNT],
            styles: vec![lumen_static::Style::default(); Page::NODE_COUNT],
            agent: Agent::new(),
        };
        f.frame(40.0); // seeds the baseline
        f
    }

    /// One frame with the spacer `gap_h` tall. The spacer and the row are
    /// the full 800 px wide; the label is one short word.
    fn frame(&mut self, gap_h: f32) {
        let sheet = [
            Rule::new(Selector::Id("gap"), Patch::new().width(Length::FILL).height(Length::Fixed(gap_h))),
            Rule::new(Selector::Id("line"), Patch::new().width(Length::FILL)),
        ];
        lumen_static::style::cascade_deep(&mut self.app, &sheet, &mut self.styles);
        lumen_static::pass::layout(&mut self.app, &self.styles, &mut self.states, VIEW);
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
}

fn area(r: &Value) -> f64 {
    let v: Vec<f64> = r.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    (v[2] - v[0]).max(0.0) * (v[3] - v[1]).max(0.0)
}

#[test]
fn a_node_that_draws_nothing_moves_without_damage() {
    let mut f = Fixture::new();
    let since = f.result("ui.lastChange", json!({}))["frame"].as_u64().unwrap();
    f.frame(90.0);

    // The setup, asserted: all three moved, so all three are changes.
    let changed = f.result("ui.lastChange", json!({ "since": since }))["changed"].as_array().unwrap().clone();
    let wids: Vec<&str> = changed.iter().filter_map(|c| c["wid"].as_str()).collect();
    for w in ["gap", "line", "text"] {
        assert!(wids.contains(&w), "setup: `{w}` did not change; changed were {wids:?}");
    }

    let text = f.result("ui.resolve", json!({ "selector": "#text" }))["bounds"].clone();
    let text_area = area(&text);
    let dmg = f.result("ui.lastDamage", json!({ "since": since }));
    let regions = dmg["regions"].as_array().unwrap();
    let total: f64 = regions.iter().map(area).sum();
    println!(
        "{} changed ({wids:?}); damage {} region(s), {total:.0} px² (bound: the label's old and new box, 2 x {text_area:.0} = {:.0})",
        changed.len(),
        regions.len(),
        2.0 * text_area
    );
    // The other direction: the node that draws is still damage.
    assert!(total > 0.0, "the label moved and nothing was damaged");
    assert!(
        total <= 2.0 * text_area + 0.5,
        "a spacer and a transparent row that moved were counted as damage: {total:.0} px² against the label's {:.0}",
        2.0 * text_area
    );
}
