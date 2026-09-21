//! `ui.explain` — the method P4 deferred until "there is event routing
//! to narrate".
//!
//! The property under test is not the prose. It is that the answer comes
//! from running the **real hit test**, so it cannot disagree with what a
//! click actually does. A narrative assembled from properties would be a
//! second model of dispatch, free to drift from the first — the
//! probe-vs-click class of bug, one layer up.

use lumen_static_widgets::ops::{Button, Fixed, Label};
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{pass, Constraints, NodeState, StaticTree, Style, Widget};
use lumen_static_agent::{Agent, TreeRef};
use serde_json::{json, Value};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("head")]
    head: Label,
    #[widget_id("save")]
    save: Button<()>,
    #[widget_id("blank")]
    blank: Fixed,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 200.0 };

struct F {
    ui: Ui,
    styles: Vec<Style>,
    states: [NodeState; Ui::NODE_COUNT],
}

impl F {
    fn new(sheet: &[Rule]) -> Self {
        let mut ui = Ui::default();
        ui.head.set("head");
        ui.save.label = "save";
        let mut styles = pass::default_styles(&ui);
        if !sheet.is_empty() {
            lumen_static::style::cascade_deep(&mut ui, sheet, &mut styles);
        }
        let mut f = F { ui, styles, states: [NodeState::default(); Ui::NODE_COUNT] };
        pass::layout(&mut f.ui, &f.styles, &mut f.states, VIEW);
        f
    }

    fn call(&self, method: &str, params: Value) -> Value {
        let tree = TreeRef::of_root(&self.ui, &self.styles, &self.states);
        let agent = Agent::new();
        let r = agent.dispatch(
            &tree,
            &json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }),
        );
        assert!(r.get("error").is_none(), "{method}: {r}");
        r["result"].clone()
    }
}

#[test]
fn a_reachable_control_is_reported_reachable() {
    let f = F::new(&[]);
    let r = f.call("ui.explain", json!({ "selector": "#save" }));
    assert_eq!(r["reachable"], true, "{r}");
    assert!(r["reasons"].as_array().unwrap().is_empty(), "{r}");
    assert!(r["summary"].as_str().unwrap().contains("reaches"));
    // The probe point is the node's own centre, and the hit is itself.
    assert_eq!(r["hit"]["wid"], "save");
}

#[test]
fn a_hidden_control_explains_itself_and_the_hit_test_agrees() {
    // The two must not disagree: `reachable` comes from running the same
    // `pass::hit_test` a click runs, and the reason comes from the
    // style. If a future change made hiding cosmetic, this fails.
    let f = F::new(&[Rule::new(Selector::Id("save"), Patch::new().visible(false))]);
    let r = f.call("ui.explain", json!({ "selector": "#save" }));
    assert_eq!(r["reachable"], false, "{r}");
    let codes: Vec<&str> =
        r["reasons"].as_array().unwrap().iter().map(|x| x["code"].as_str().unwrap()).collect();
    assert!(codes.contains(&"hidden"), "{codes:?}");
    assert!(codes.contains(&"no-area"), "hiding also removes the area: {codes:?}");
}

#[test]
fn a_control_with_no_area_says_so_rather_than_saying_nothing() {
    let f = F::new(&[Rule::new(Selector::Id("save"), Patch::new().height(Length::Fixed(0.0)))]);
    let r = f.call("ui.explain", json!({ "selector": "#save" }));
    assert_eq!(r["reachable"], false);
    let codes: Vec<&str> =
        r["reasons"].as_array().unwrap().iter().map(|x| x["code"].as_str().unwrap()).collect();
    assert!(codes.contains(&"no-area"), "{codes:?}");
}

#[test]
fn explaining_a_point_that_belongs_to_another_node_names_that_node() {
    // "I clicked at these coordinates and #save did not fire." The
    // answer names what is actually there, with its bounds — which is
    // the whole of Lumen's `ui.explain` use case.
    let f = F::new(&[]);
    let head = &f.states[1];
    let r = f.call(
        "ui.explain",
        json!({ "selector": "#save", "at": [head.pos.x + 1.0, head.pos.y + 1.0] }),
    );
    assert_eq!(r["reachable"], false);
    let reasons = r["reasons"].as_array().unwrap();
    let occluded = reasons.iter().find(|x| x["code"] == "occluded").expect("occluded reason");
    let detail = occluded["detail"].as_str().unwrap();
    assert!(detail.contains("#head"), "names the node that is actually there: {detail}");
    assert_eq!(r["hit"]["wid"], "head");
}

#[test]
fn a_point_outside_the_tree_is_distinguished_from_an_occlusion() {
    let f = F::new(&[]);
    let r = f.call("ui.explain", json!({ "selector": "#save", "at": [-99.0, -99.0] }));
    let codes: Vec<&str> =
        r["reasons"].as_array().unwrap().iter().map(|x| x["code"].as_str().unwrap()).collect();
    assert!(codes.contains(&"nothing-there"), "{codes:?}");
    assert!(r["hit"].is_null());
}

#[test]
fn a_non_focusable_node_is_told_that_input_key_will_not_reach_it() {
    let f = F::new(&[]);
    let r = f.call("ui.explain", json!({ "selector": "#head" }));
    let codes: Vec<&str> =
        r["reasons"].as_array().unwrap().iter().map(|x| x["code"].as_str().unwrap()).collect();
    assert!(codes.contains(&"not-focusable"), "a label takes no focus: {codes:?}");
    // …but it is still reachable by pointer, and the summary says so.
    assert!(r["summary"].as_str().unwrap().contains("reaches"), "{r}");
}

#[test]
fn applied_styles_report_the_cascades_answer_and_the_painted_state() {
    let f = F::new(&[Rule::new(
        Selector::Id("save"),
        Patch::new().radius(9.0).font_size(21.0).background(lumen_static::Color(1, 2, 3, 255)),
    )]);
    let r = f.call("ui.getAppliedStyles", json!({ "selector": "#save" }));
    assert_eq!(r["radius"], 9.0);
    assert_eq!(r["font_size"], 21.0);
    assert_eq!(r["background"], json!([1, 2, 3, 255]));
    // The painted background is what actually reached the screen — the
    // same value with C6's pointer shade applied, if any.
    assert_eq!(r["state"]["painted_background"], json!([1, 2, 3, 255]), "resting");
    assert_eq!(r["state"]["pressed"], false);
}

#[test]
fn applied_styles_show_the_pressed_shade_a_screenshot_would_have() {
    // Why this pairs with `ui.screenshot`: an agent that sees a darker
    // button in a screenshot can ask *why* and get "pressed", instead of
    // guessing that the stylesheet changed.
    let mut f = F::new(&[]);
    f.states[2].pressed = true;
    pass::layout(&mut f.ui, &f.styles, &mut f.states, VIEW);
    let r = f.call("ui.getAppliedStyles", json!({ "selector": "#save" }));
    assert_eq!(r["state"]["pressed"], true);
    assert_ne!(
        r["state"]["painted_background"], r["background"],
        "the painted colour differs from the resting one while held"
    );
}
