//! P0/P1 protocol tests: envelope, queries, selector ergonomics, hit
//! testing, diagnostics — all read-side, against a laid-out tree.

use lumen_static::ops::{Fixed, Label, Spacer};
use lumen_static::repeat::Repeat;
use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};
use lumen_static_agent::{Agent, TreeRef};
use serde_json::{json, Value};

#[derive(StaticTree, Default)]
#[widget(row, spacing = 2.0)]
struct RowT {
    icon: Fixed,
    name: Label,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 2.0)]
struct Bar {
    #[widget_id("title")]
    title: Label,
    pad: Spacer,
    #[widget_id("badge")]
    badge: Fixed,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct App {
    bar: Bar,
    #[widget_id("list")]
    list: Repeat<RowT>,
    footer: Label,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 600.0 };

fn bind(item: &mut RowT, row: usize) {
    item.icon.w = 10.0;
    item.name.text = if row.is_multiple_of(2) { "even row" } else { "odd row" };
}

struct Fixture {
    app: App,
    states: [NodeState; App::NODE_COUNT],
    agent: Agent,
}

impl Fixture {
    fn new() -> Self {
        let mut app = App { list: Repeat::new(100, 24.0, bind), ..Default::default() };
        app.bar.title.text = "hello";
        app.footer.text = "bye";
        let mut states = [NodeState::default(); App::NODE_COUNT];
        let base = &app as *const App as *const u8;
        pass::layout(base, App::SUB_TABLE, App::KIND_LIST, &mut states, VIEW);
        let mut agent = Agent::new();
        let tree = TreeRef { base, table: App::SUB_TABLE, kinds: App::KIND_LIST, states: &states };
        agent.frame_done(&tree, 12_345, 6_789);
        Fixture { app, states, agent }
    }

    fn call(&self, method: &str, params: Value) -> Value {
        let tree = TreeRef::of_root(&self.app, &self.states);
        self.agent.dispatch(
            &tree,
            &json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }),
        )
    }

    fn result(&self, method: &str, params: Value) -> Value {
        let resp = self.call(method, params);
        assert!(resp.get("error").is_none(), "unexpected error: {resp}");
        resp["result"].clone()
    }

    fn error(&self, method: &str, params: Value) -> Value {
        let resp = self.call(method, params);
        assert!(resp.get("error").is_some(), "expected error, got: {resp}");
        resp["error"].clone()
    }
}

#[test]
fn envelope_and_unknown_method() {
    let f = Fixture::new();
    let resp = f.call("ui.getTree", json!({}));
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    let err = f.error("ui.noSuchMethod", json!({}));
    assert_eq!(err["code"], -32601);
}

#[test]
fn get_tree_carries_wids_and_provenance() {
    let f = Fixture::new();
    let text = f.result("ui.getTree", json!({})).to_string();
    assert!(text.contains(r#""wid":"title""#));
    assert!(text.contains(r#""wid":"list""#));
    assert!(text.contains("tests/protocol.rs:"));
    assert!(text.contains(r#""value":"hello""#));
}

#[test]
fn get_layout_by_author_id() {
    let f = Fixture::new();
    let r = f.result("ui.getLayout", json!({ "selector": "#title" }));
    assert_eq!(r["kind"], "label");
    assert_eq!(r["wid"], "title");
    assert_eq!(r["value"], "hello");
    assert_eq!(r["size"][1], 16.0); // label height
    assert!(r["src"].as_str().unwrap().contains("tests/protocol.rs"));
    assert!(r["constraints"]["max_w"].as_f64().unwrap() > 0.0);
}

#[test]
fn selector_terms_and_conjunction() {
    let f = Fixture::new();
    // kind alone is ambiguous (three labels)…
    let err = f.error("ui.resolve", json!({ "selector": "label" }));
    assert_eq!(err["code"], -32002);
    assert!(err["data"]["candidates"].as_array().unwrap().len() >= 2);
    // …value narrows it…
    let r = f.result("ui.resolve", json!({ "selector": "label ~bye" }));
    assert_eq!(r["index"], 1 + Bar::NODE_COUNT + 1); // footer
    // …and so does a table index or provenance term.
    let r = f.result("ui.resolve", json!({ "selector": "2" }));
    assert_eq!(r["wid"], "title");
    let r = f.result("ui.resolve", json!({ "selector": "repeat @tests/protocol.rs" }));
    assert_eq!(r["wid"], "list");
}

#[test]
fn not_found_teaches() {
    let f = Fixture::new();
    let err = f.error("ui.resolve", json!({ "selector": "#save" }));
    assert_eq!(err["code"], -32001);
    let ids = err["data"]["available_ids"].as_array().unwrap();
    assert!(ids.iter().any(|v| v == "title"), "lists real ids: {ids:?}");

    let err = f.error("ui.resolve", json!({ "selector": "button" }));
    assert_eq!(err["code"], -32001);
    assert!(err["data"]["available_kinds"].to_string().contains("label"));
}

#[test]
fn probe_hits_the_deepest_node_including_instances() {
    let f = Fixture::new();
    // Title label sits at the top-left row of the bar.
    let r = f.result("ui.probe", json!({ "x": 1.0, "y": 8.0 }));
    assert_eq!(r["wid"], "title");

    // Below the bar, the repeater's instances live; a point inside the
    // first visible row must resolve to an instance (synthetic id).
    let bar_h = f.states[1].size.h as f64;
    let r = f.result("ui.probe", json!({ "x": 30.0, "y": bar_h + 4.0 }));
    let id = r["id"].as_u64().unwrap();
    assert!(id & (1 << 63) != 0, "instance id expected, got {r}");

    // Way outside: null.
    let r = f.result("ui.probe", json!({ "x": 5000.0, "y": 5000.0 }));
    assert!(r.is_null());
}

#[test]
fn probe_region_finds_intersections() {
    let f = Fixture::new();
    let hits = f.result("ui.probeRegion", json!({ "x": 0.0, "y": 0.0, "w": 800.0, "h": 10.0 }));
    let arr = hits.as_array().unwrap();
    assert!(arr.iter().any(|h| h["wid"] == "title"));
    assert!(!arr.iter().any(|h| h["value"] == "bye"), "footer is far below");
}

#[test]
fn diagnostics_and_perf() {
    let f = Fixture::new();
    let d = f.result("app.diagnostics", json!({}));
    assert_eq!(d["nodes"], App::NODE_COUNT);
    assert!(d["total_nodes"].as_u64().unwrap() > App::NODE_COUNT as u64, "instances counted");
    let rep = &d["repeaters"][0];
    assert_eq!(rep["rows"], 100);
    assert!(rep["window"].as_u64().unwrap() > 0);
    assert_eq!(d["frame"], 1);

    let p = f.result("app.perf", json!({}));
    assert_eq!(p["layout_ns"], 12_345);
    let w = f.result("ui.waitSettled", json!({}));
    assert_eq!(w["settled"], true);
}

#[test]
fn invalid_params_are_rejected() {
    let f = Fixture::new();
    let err = f.error("ui.getLayout", json!({}));
    assert_eq!(err["code"], -32602);
    let err = f.error("ui.probe", json!({ "x": 1.0 }));
    assert_eq!(err["code"], -32602);
    let err = f.error("ui.resolve", json!({ "selector": "   " }));
    assert_eq!(err["code"], -32602);
}
