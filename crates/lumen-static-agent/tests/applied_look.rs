//! M25/T25.5: `ui.getAppliedStyles` reports what a widget with a look
//! paints, and nothing it does not.

use lumen_static::ops::Label;
use lumen_static::{pass, CheckBox, Constraints, NodeState, StaticTree, Widget};
use lumen_static_agent::{Agent, TreeRef};
use serde_json::{json, Value};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("check")]
    check: CheckBox<()>,
    #[widget_id("caption")]
    caption: Label,
}

fn styles_of(selector: &str) -> Value {
    let mut ui = Ui::default();
    ui.check.on = true;
    ui.caption.set("caption");
    let styles = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    pass::layout(
        &mut ui,
        &styles,
        &mut st,
        Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 100.0 },
    );
    let tree = TreeRef::of_root(&ui, &styles, &st);
    let r = Agent::new().dispatch(
        &tree,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.getAppliedStyles",
                 "params": { "selector": selector } }),
    );
    println!("{selector}: {r}");
    r["result"].clone()
}

#[test]
fn a_toggle_reports_its_look_and_not_the_colours_it_does_not_paint() {
    let r = styles_of("#check");
    let look = r.get("look").expect("a toggle reports its resolved look");
    for part in ["body", "on_body", "mark", "focus"] {
        assert!(look.get(part).is_some(), "the look is missing `{part}`: {look}");
    }
    assert!(r.get("background").is_none(), "a toggle reported a background it does not paint: {r}");
    assert!(r.get("color").is_none(), "a toggle reported a colour it does not paint: {r}");
    assert!(r["state"].get("painted_background").is_none(), "a stale painted background: {r}");
    assert!(r.get("radius").is_some(), "the radius is still the node's, and still reported: {r}");
}

#[test]
fn a_widget_without_a_look_reports_its_cascaded_colours() {
    let r = styles_of("#caption");
    assert!(r.get("look").is_none(), "a label has no look: {r}");
    assert!(r.get("color").is_some() && r.get("background").is_some(), "a label's colours: {r}");
}
