//! P0/P1 protocol tests: envelope, queries, selector ergonomics, hit
//! testing, diagnostics — all read-side, against a laid-out tree.

use lumen_static_widgets::ops::{Fixed, Label, Spacer};
use lumen_static::repeat::Repeat;
use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};
use lumen_static_agent::{Agent, TreeMut, TreeRef};
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
    item.name.set(if row.is_multiple_of(2) { "even row" } else { "odd row" });
}

struct Fixture {
    app: App,
    states: [NodeState; App::NODE_COUNT],
    styles: Vec<lumen_static::Style>,
    agent: Agent,
}

impl Fixture {
    fn new() -> Self {
        let mut app = App { list: Repeat::new(100, 24.0, bind), ..Default::default() };
        app.bar.title.set("hello");
        app.footer.set("bye");
        let mut states = [NodeState::default(); App::NODE_COUNT];
        let styles = pass::default_styles(&app);
        pass::layout(&mut app, &styles, &mut states, VIEW);
        let mut agent = Agent::new();
        {
            let mut tm = TreeMut::of_root(&mut app, &styles, &states);
            agent.frame_done(&mut tm, lumen_static_agent::FrameTiming::passes(12_345, 6_789));
        }
        Fixture { app, states, styles, agent }
    }

    fn call(&self, method: &str, params: Value) -> Value {
        let tree = TreeRef::of_root(&self.app, &self.styles, &self.states);
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
    assert_eq!(p["last"]["layout_ns"], 12_345);
    assert_eq!(p["last"]["paint_ns"], 6_789);
    assert_eq!(p["last"]["input_latency_ns"], serde_json::Value::Null, "no input this frame");
    assert_eq!(p["window"]["layout_ns"]["p95"], 12_345, "one sample: every percentile is it");
    assert_eq!(p["window"]["input_latency_ns"]["n"], 0);
    assert_eq!(p["samples"], 1);
    let w = f.result("ui.waitSettled", json!({}));
    assert_eq!(w["settled"], true);

    // M42/T42.1: a tree alone cannot say which process it is in.
    assert_eq!(d["process"]["supported"], false, "a tree claimed a process: {d}");
}

/// A host over the fixture, with and without `process_info`.
struct Hosted<'a>(&'a Fixture, Option<Value>);

impl lumen_static_agent::AgentHost for Hosted<'_> {
    fn tree(&self) -> TreeRef<'_> {
        TreeRef::of_root(&self.0.app, &self.0.styles, &self.0.states)
    }
    fn enqueue(&mut self, _ev: lumen_static::Event) {}
    fn pump(&mut self) -> (u64, bool) {
        (1, false)
    }
    fn process_info(&self) -> Value {
        match &self.1 {
            Some(v) => v.clone(),
            None => json!({ "supported": false }),
        }
    }
}

/// A host that does not implement `process_info` answers
/// `supported: false`, which the drive harness refuses (rule 2 of
/// `docs/plan-drive-identity.md` §0); one that does is merged into
/// `app.diagnostics` under `process`, beside the tree's own fields.
#[test]
fn diagnostics_carries_the_hosts_process_block() {
    let f = Fixture::new();
    let req = json!({ "jsonrpc": "2.0", "id": 1, "method": "app.diagnostics", "params": {} });

    struct Bare<'a>(&'a Fixture);
    impl lumen_static_agent::AgentHost for Bare<'_> {
        fn tree(&self) -> TreeRef<'_> {
            TreeRef::of_root(&self.0.app, &self.0.styles, &self.0.states)
        }
        fn enqueue(&mut self, _ev: lumen_static::Event) {}
        fn pump(&mut self) -> (u64, bool) {
            (1, false)
        }
    }
    let bare = f.agent.dispatch_input(&mut Bare(&f), &req);
    println!("default: {}", bare["result"]["process"]);
    assert_eq!(bare["result"]["process"], json!({ "supported": false }), "{bare}");
    assert_eq!(bare["result"]["nodes"], App::NODE_COUNT, "the tree's fields are gone: {bare}");

    let block = lumen_static_agent::process_block("x::Everything", &["everything"]);
    let told = f.agent.dispatch_input(&mut Hosted(&f, Some(block)), &req);
    let p = &told["result"]["process"];
    println!("implemented: {p}");
    assert_eq!(p["supported"], true, "{told}");
    assert_eq!(p["strategy"], "x::Everything", "{told}");
    assert_eq!(p["features"], json!(["everything"]), "{told}");
    assert_eq!(p["pid"], std::process::id(), "{told}");
    assert!(p["exe"].as_str().is_some_and(|e| !e.is_empty()), "{told}");
    assert_eq!(told["result"]["nodes"], App::NODE_COUNT, "{told}");
    let _ = Hosted(&f, None);
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
