//! **A part its owner paints is journaled** — M48/T48.4.
//!
//! A `Picker`'s trigger is a node on the table that the `Picker` itself
//! draws: `node(trigger)` answers with the picker, so the journal took
//! the trigger for a stand-in and never signed it, and the picker's own
//! signature — its resolved state, not its paint — held none of the
//! trigger's text or swatch. Choosing a colour changed what the trigger
//! shows and `ui.lastChange` reported nothing. Found by `just
//! widget-drive` once a repainting action with no damage over its widget
//! began to fail (the check had been skipped whenever nothing overlapped).
//! The same change must also *damage* the picker (the M56 merge).

use fulgurite::{pass, Constraints, NodeState, StaticTree, Widget};
use fulgurite_agent::{Agent, FrameTiming, TreeMut, TreeRef};
use fulgurite_widgets::ColorPicker;
use serde_json::json;

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 300.0 };

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("pick")]
    pick: ColorPicker,
}

#[test]
fn choosing_a_colour_is_a_change_on_the_picker() {
    let mut ui = Ui::default();
    let styles = pass::default_styles(&ui);
    let mut states = vec![NodeState::default(); Ui::NODE_COUNT];
    pass::layout(&mut ui, &styles, &mut states, VIEW);
    let mut agent = Agent::new();
    agent.frame_done(&mut TreeMut::of_root(&mut ui, &styles, &states), FrameTiming::default());
    let since = agent.frame;

    let before = ui.pick.pad.content.color;
    ui.pick.pad.content.color = before + 3;
    pass::layout(&mut ui, &styles, &mut states, VIEW);
    agent.frame_done(&mut TreeMut::of_root(&mut ui, &styles, &states), FrameTiming::default());

    let tree = TreeRef::of_root(&ui, &styles, &states);
    let r = agent.dispatch(
        &tree,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.lastChange", "params": { "since": since } }),
    );
    let changed: Vec<(u64, String)> = r["result"]["changed"]
        .as_array()
        .expect("changed")
        .iter()
        .map(|c| (c["index"].as_u64().unwrap(), c["kind"].as_str().unwrap_or("").to_string()))
        .collect();
    let pick = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "pick").unwrap() as u64;
    println!("swatch {before} -> {}: changed {changed:?} (must include the picker, index {pick})", before + 3);
    assert!(changed.iter().any(|(i, _)| *i == pick), "the trigger's new swatch was not journaled: {changed:?}");

    // **And it damages where the picker is** — the M56 merge. Journaled
    // is half of it: `ui.lastDamage` keeps only changes to nodes that
    // draw (M56's `paints` rule), and the part-signing branch skipped
    // setting that flag, so the change above was reported with no region
    // at all and the live drive's "a cell sets the value" failed on all
    // three pickers that paint their trigger.
    let d = agent.dispatch(
        &tree,
        &json!({ "jsonrpc": "2.0", "id": 2, "method": "ui.lastDamage", "params": { "since": since } }),
    );
    let regions = d["result"]["regions"].as_array().expect("regions").clone();
    let at = &states[pick as usize];
    let over = regions.iter().filter(|r| {
        let r: Vec<f32> = r.as_array().unwrap().iter().map(|v| v.as_f64().unwrap() as f32).collect();
        r[0] < at.pos.x + at.size.w && r[2] > at.pos.x && r[1] < at.pos.y + at.size.h && r[3] > at.pos.y
    }).count();
    println!("damage regions {regions:?}; {over} over the picker at {:?}+{:?}", at.pos, at.size);
    assert!(over > 0, "the picker's change damaged nothing over it: {regions:?}");
}
