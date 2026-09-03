//! P3 tests: the ambient audit. Each lint gets a firing case *and* a
//! quiet case — a check that cannot be shown not to fire is a check an
//! agent will learn to ignore.

use lumen_static::ops::{Fixed, Label, Spacer};
use lumen_static::repeat::Repeat;
use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};
use lumen_static_agent::{Agent, TreeMut, TreeRef};
use serde_json::{json, Value};

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct RowT {
    icon: Fixed,
    name: Label,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Bar {
    #[widget_id("title")]
    title: Label,
    #[widget_id("pad")]
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
}

fn bind(item: &mut RowT, _row: usize) {
    item.icon.w = 8.0;
    item.icon.h = 8.0;
}

struct Fixture {
    app: App,
    states: [NodeState; App::NODE_COUNT],
    styles: Vec<lumen_static::Style>,
    agent: Agent,
    view: Constraints,
}

impl Fixture {
    /// A healthy app: a titled bar over a list with rows.
    fn healthy() -> Self {
        let mut app = App { list: Repeat::new(50, 24.0, bind), ..Default::default() };
        app.bar.title.set("hi");
        let styles = pass::default_styles(&app);
        Fixture {
            app,
            states: [NodeState::default(); App::NODE_COUNT],
            styles,
            agent: Agent::new(),
            view: Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 600.0 },
        }
    }

    fn frame(&mut self) {
        pass::layout(&mut self.app, &self.styles, &mut self.states, self.view);
        let mut tree = TreeMut::of_root(&mut self.app, &self.styles, &self.states);
        self.agent.frame_done(&mut tree, lumen_static_agent::FrameTiming::passes(10, 10));
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

    fn findings(&self) -> Vec<Value> {
        self.result("ui.lint", json!({}))["findings"].as_array().unwrap().clone()
    }

    fn codes(&self) -> Vec<String> {
        self.findings().iter().map(|f| f["code"].as_str().unwrap().to_owned()).collect()
    }

    fn logs(&self, since: u64) -> Vec<Value> {
        self.result("app.logs", json!({ "since": since }))["entries"]
            .as_array()
            .unwrap()
            .clone()
    }
}

fn of_code<'a>(findings: &'a [Value], code: &str) -> Option<&'a Value> {
    findings.iter().find(|f| f["code"] == code)
}

#[test]
fn a_healthy_frame_is_quiet() {
    let mut f = Fixture::healthy();
    f.frame();
    assert!(f.findings().is_empty(), "no findings on a healthy frame: {:?}", f.findings());
}

#[test]
fn a_collapsed_spacer_is_not_reported_as_invisible() {
    // The false positive that would otherwise be the first thing an agent
    // ever sees: fillers legitimately collapse to nothing.
    let mut f = Fixture::healthy();
    f.app.bar.title.set("a very long title that eats the whole row's width");
    f.frame();
    let codes = f.codes();
    let invisible_on_pad = f
        .findings()
        .iter()
        .any(|x| x["code"] == "W1001" && x["wid"] == "pad");
    assert!(!invisible_on_pad, "spacer must not be flagged: {codes:?}");
}

#[test]
fn invisible_content_is_found_and_names_its_source_line() {
    let mut f = Fixture::healthy();
    // Give the bar no width at all: the title has text but no room.
    f.view.max_w = 0.0;
    f.frame();

    let findings = f.findings();
    let inv = of_code(&findings, "W1001").expect("W1001 expected: {findings:?}");
    assert_eq!(inv["wid"], "title");
    assert_eq!(inv["kind"], "label");
    assert!(inv["src"].as_str().unwrap().contains("tests/lint.rs"), "names the line to edit");
    assert!(inv["detail"]["natural"][0].as_f64().unwrap() > 0.0);
    assert_eq!(inv["detail"]["size"][0].as_f64().unwrap(), 0.0);
    assert!(inv["message"].as_str().unwrap().contains("absent on screen"));
}

#[test]
fn truncation_reports_how_much_room_is_missing() {
    let mut f = Fixture::healthy();
    // 60px of row split between three children => 20px each; this text
    // wants far more.
    f.view.max_w = 60.0;
    f.app.bar.title.set("far too long for twenty pixels");
    f.frame();

    let findings = f.findings();
    let t = of_code(&findings, "W1005").expect("W1005 expected");
    assert_eq!(t["wid"], "title");
    let missing = t["detail"]["missing"][0].as_f64().unwrap();
    assert!(missing > 0.0, "missing width is quantified: {t}");
    assert!(t["detail"]["natural"][0].as_f64().unwrap() > t["detail"]["size"][0].as_f64().unwrap());
}

#[test]
fn a_collection_with_items_but_no_height_is_found() {
    let mut f = Fixture::healthy();
    f.app.list.item_extent = 0.0; // rows exist, each is zero tall
    f.frame();

    let findings = f.findings();
    let c = of_code(&findings, "W1007").expect("W1007 expected: {findings:?}");
    assert_eq!(c["wid"], "list");
    assert_eq!(c["detail"]["items"], 50);
    assert_eq!(c["detail"]["materialized"], 0);
    assert!(c["message"].as_str().unwrap().contains("item extent"));
}

#[test]
fn an_empty_collection_is_legitimate_and_silent() {
    // The distinction geometry alone cannot make: zero rows is fine, zero
    // extent is a bug. Only NodeStats can tell them apart.
    let mut f = Fixture::healthy();
    f.app.list.count = 0;
    f.frame();
    assert!(!f.codes().contains(&"W1007".to_owned()), "empty list is not a defect: {:?}", f.codes());
}

#[test]
fn a_blank_frame_announces_itself() {
    let mut f = Fixture::healthy();
    f.app.bar.title.set("");
    f.app.list.count = 0;
    f.view.max_h = 0.0;
    f.frame();
    let findings = f.findings();
    assert!(of_code(&findings, "W1006").is_some(), "W1006 expected: {findings:?}");
}

/// Pins the reason W1003 (overflow) and W1004 (sibling overlap) are
/// reserved rather than shipped: constraints flow down and every node
/// clamps to them, so a squeezed layout cannot push a child outside its
/// parent — it can only leave the child too small, which is W1005.
///
/// If a later layout mode (scroll content, absolute positioning, a
/// minimum exceeding the parent's maximum) breaks this invariant, this
/// test fails and the reserved codes become implementable.
#[test]
fn overflow_is_impossible_here_so_squeezing_surfaces_as_truncation() {
    let mut f = Fixture::healthy();
    // Squeeze the main axis of the bar: three children sharing 30px
    // cannot all have what they ask for.
    f.view.max_w = 30.0;
    f.app.bar.title.set("far too long for ten pixels");
    f.frame();

    for (i, d) in App::SUB_TABLE.iter().enumerate().skip(1) {
        let (c, p) = (&f.states[i], &f.states[d.parent as usize]);
        let slack = 0.5;
        assert!(
            c.pos.x + c.size.w <= p.pos.x + p.size.w + slack
                && c.pos.y + c.size.h <= p.pos.y + p.size.h + slack
                && c.pos.x + slack >= p.pos.x
                && c.pos.y + slack >= p.pos.y,
            "node {i} escaped its parent — the layout model changed, so W1003 is now \
             implementable: child {:?} parent {:?}",
            c,
            p
        );
    }

    assert!(
        of_code(&f.findings(), "W1005").is_some(),
        "the squeeze is reported as truncation instead: {:?}",
        f.findings()
    );
}

#[test]
fn findings_are_keyed_by_code_and_node_not_by_code_alone() {
    // Lumen's O0.1b blocker: with a (code, None) key, the second finding
    // of a code is swallowed. Two labels, both invisible, must both report.
    #[derive(StaticTree, Default)]
    #[widget(row, spacing = 0.0)]
    struct Two {
        #[widget_id("a")]
        a: Label,
        #[widget_id("b")]
        b: Label,
    }

    let mut app = Two::default();
    app.a.set("aaa");
    app.b.set("bbb");
    let mut states = [NodeState::default(); Two::NODE_COUNT];
    let view = Constraints { min_w: 0.0, max_w: 0.0, min_h: 0.0, max_h: 100.0 };
    let styles = pass::default_styles(&app);
    pass::layout(&mut app, &styles, &mut states, view);

    let mut agent = Agent::new();
    {
        let mut tm = TreeMut::of_root(&mut app, &styles, &states);
        agent.frame_done(&mut tm, lumen_static_agent::FrameTiming::passes(1, 1));
    }
    let tree = TreeRef::of_root(&app, &styles, &states);
    let resp = agent.dispatch(
        &tree,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.lint", "params": {} }),
    );
    let findings = resp["result"]["findings"].as_array().unwrap();
    let invisible: Vec<&Value> = findings.iter().filter(|f| f["code"] == "W1001").collect();
    assert_eq!(invisible.len(), 2, "both nodes report: {findings:?}");
    assert!(invisible.iter().any(|f| f["wid"] == "a"));
    assert!(invisible.iter().any(|f| f["wid"] == "b"));
}

#[test]
fn transitions_are_edge_triggered_and_repeat_after_a_fix_and_re_break() {
    // Lumen's O0.3 scar: a monotonic seen-set would report the reopen
    // only once, ever.
    let mut f = Fixture::healthy();
    f.frame();
    let start = f.agent.frame;

    // break…
    f.view.max_w = 0.0;
    f.frame();
    let opened: Vec<Value> = f.logs(start).into_iter().filter(|e| e["event"] == "opened").collect();
    assert!(opened.iter().any(|e| e["code"] == "W1001"), "opened logged: {opened:?}");

    // …fix…
    let before_fix = f.agent.frame;
    f.view.max_w = 800.0;
    f.frame();
    let closed: Vec<Value> =
        f.logs(before_fix).into_iter().filter(|e| e["event"] == "closed").collect();
    assert!(closed.iter().any(|e| e["code"] == "W1001"), "closed logged: {closed:?}");
    assert!(f.findings().is_empty(), "and the finding is gone");

    // …break again: it must report a second time.
    let before_rebreak = f.agent.frame;
    f.view.max_w = 0.0;
    f.frame();
    let reopened: Vec<Value> =
        f.logs(before_rebreak).into_iter().filter(|e| e["event"] == "opened").collect();
    assert!(reopened.iter().any(|e| e["code"] == "W1001"), "re-opened: {reopened:?}");
}

#[test]
fn a_persisting_finding_logs_once_not_every_frame() {
    let mut f = Fixture::healthy();
    f.view.max_w = 0.0;
    f.frame();
    let after_first = f.agent.frame;
    for _ in 0..5 {
        f.frame();
    }
    assert!(f.logs(after_first).is_empty(), "a standing finding stays quiet in the log");
    assert!(!f.findings().is_empty(), "…while remaining true in ui.lint");
}
