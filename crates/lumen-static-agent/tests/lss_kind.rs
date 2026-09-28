//! M61/T61.0 gate 3, the run-time half (`docs/plan-lss.md` §2.3): a
//! `.lss` kind selector spelled in the wrong case **compiles** -- kinds are
//! not in `NodeDesc`, so `include_lss!` cannot check them -- and the
//! agent's `W1010` names it at run time. The gap is documented; this pins
//! it, so a change that closes it (or widens it) is seen.

use lumen_static::style::lint_sheet_deep;
use lumen_static::{include_lss, pass, Constraints, NodeState, Rule, Selector, StaticTree, Widget};
use lumen_static_agent::{Agent, TreeMut, TreeRef};
use lumen_static_widgets::ops::Label;
use serde_json::{json, Value};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("title")]
    title: Label,
}

const SHEET: &[Rule] = include_lss!(Ui, "lss_kind.lss");

#[test]
#[ignore = "red until T61.3: include_lss! is a stub that emits no rules"]
fn a_wrong_case_kind_compiles_and_w1010_names_it() {
    println!("sheet: {SHEET:?}");
    assert_eq!(SHEET.len(), 2, "setup: the sheet lowered to {} rules, want 2", SHEET.len());
    assert!(matches!(SHEET[0].selector, Selector::Kind("Label")), "setup: rule 0 is {:?}", SHEET[0].selector);

    let mut ui = Ui::default();
    ui.title.set("hi");
    let styles = pass::default_styles(&ui);
    let mut states = [NodeState::default(); Ui::NODE_COUNT];
    let view = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 300.0 };
    pass::layout(&mut ui, &styles, &mut states, view);
    let mut agent = Agent::new();
    let mut tree = TreeMut::of_root(&mut ui, &styles, &states);
    agent.frame_done(&mut tree, lumen_static_agent::FrameTiming::passes(10, 10));

    let mut found = Vec::new();
    lint_sheet_deep(&mut ui, SHEET, &mut |x| found.push(x));
    let tree = TreeRef::of_root(&ui, &styles, &states).with_sheet(SHEET, &found);
    let resp = agent.dispatch(&tree, &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.lint", "params": {} }));
    let w1010: Vec<&Value> = resp["result"]["findings"]
        .as_array()
        .expect("ui.lint returns findings")
        .iter()
        .filter(|x| x["code"] == "W1010")
        .collect();
    println!("W1010: {w1010:#?}");
    assert_eq!(w1010.len(), 1, "exactly the wrong-case rule: {w1010:?}");
    assert_eq!(w1010[0]["rule"], 0, "W1010 names rule 0, `Label`");
    assert!(w1010[0]["message"].as_str().unwrap_or("").contains("matches no node"));
}
