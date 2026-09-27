//! P3 tests: the ambient audit. Each lint gets a firing case *and* a
//! quiet case — a check that cannot be shown not to fire is a check an
//! agent will learn to ignore.

use lumen_static_widgets::ops::{Fixed, Label, Spacer};
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

/// Pins the reason W1003 (overflow) is reserved rather than shipped:
/// constraints flow down and every node clamps to them, so a squeezed
/// layout cannot push a child outside its parent — it can only leave the
/// child too small, which is W1005.
///
/// If a later layout mode (scroll content, absolute positioning, a
/// minimum exceeding the parent's maximum) breaks this invariant, this
/// test fails and W1003 becomes implementable.
///
/// **W1004 is no longer among them.** It shipped 2026-09-10 with
/// `#[widget(stack)]` — the layout mode it was reserved against — and
/// `stack_overlap.rs` covers it. The invariant below is unchanged for
/// sequential containers, which is exactly why the code is scoped to
/// them.
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

// ---- M8/A4 + A7: what a scroll region does to the lints -------------

/// A scroller with content taller than it.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct TallBody {
    a: Fixed,
    b: Fixed,
    c: Fixed,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Scrolled {
    #[widget_id("scroller")]
    scroller: lumen_static_widgets::scroll::ScrollView<TallBody>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct BadNesting {
    #[widget_id("scroller")]
    scroller: lumen_static_widgets::scroll::ScrollView<lumen_static::repeat::Repeat<TallBody>>,
}

fn lint_of<W>(app: &mut W) -> Vec<String>
where
    W: lumen_static::widget::Node + Widget,
{
    let styles = pass::default_styles(app);
    let mut st = vec![NodeState::default(); W::NODE_COUNT];
    const VIEW: Constraints =
        Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 100.0 };
    pass::layout(app, &styles, &mut st, VIEW);
    let mut agent = Agent::new();
    {
        let mut tm = TreeMut::of_root(app, &styles, &st);
        agent.frame_done(&mut tm, lumen_static_agent::FrameTiming::passes(1, 1));
    }
    let tree = TreeRef::of_root(app, &styles, &st);
    let resp = agent.dispatch(
        &tree,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.lint", "params": {} }),
    );
    resp["result"]["findings"]
        .as_array()
        .map(|a| a.iter().map(|f| f["code"].as_str().unwrap_or("").to_owned()).collect())
        .unwrap_or_default()
}

#[test]
fn content_taller_than_its_scroller_is_not_reported_as_truncated() {
    // A4's worry: `W1005` fires on any `Shrink` node given less than it
    // asked for, "which inside a scroller is the normal case". It does
    // not fire here, and the reason is structural rather than a special
    // case in the lint: a scroller lays its content out against an
    // *unbounded* main axis (A5), so nothing inside is squeezed and
    // there is no truncation to report.
    let mut app = Scrolled::default();
    for f in [&mut app.scroller.content.a, &mut app.scroller.content.b, &mut app.scroller.content.c]
    {
        f.w = 20.0;
        f.h = 400.0;
    }
    let codes = lint_of(&mut app);
    assert!(
        !codes.iter().any(|c| c == "W1005"),
        "a scroller's overflowing content is not truncation: {codes:?}"
    );
}

#[test]
fn a_repeat_inside_a_scroller_is_reported_as_w1008() {
    // A7. The debug assert in `ScrollView::layout` catches this in a dev
    // build; the lint is what a release build has.
    let mut app = BadNesting::default();
    app.scroller.content.count = 100_000;
    app.scroller.content.item_extent = 24.0;
    let codes = lint_of(&mut app);
    assert!(codes.iter().any(|c| c == "W1008"), "expected W1008, got {codes:?}");
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct BigList {
    #[widget_id("rows")]
    rows: Repeat<TallBody>,
}

#[test]
fn a_list_longer_than_f32_can_address_is_reported_as_w1009() {
    // The ceiling deferred twice without a number
    // (`docs/step3-repeaters.md`): `scroll / item_extent` is f32, which
    // holds consecutive integers only to 2^24, so past
    // count * extent = 16,777,216 some rows have no offset that selects
    // them. 1,000,000 rows at 24 px is 24,000,000 — and it is the
    // configuration this repository's own repeater bench used.
    let mut app = BigList { rows: Repeat::new(1_000_000, 24.0, lumen_static::repeat::noop_bind) };
    let codes = lint_of(&mut app);
    assert!(codes.iter().any(|c| c == "W1009"), "expected W1009, got {codes:?}");

    // Just under the ceiling is silent.
    let mut ok = BigList { rows: Repeat::new(699_000, 24.0, lumen_static::repeat::noop_bind) };
    let codes = lint_of(&mut ok);
    assert!(!codes.iter().any(|c| c == "W1009"), "699k rows at 24 px are addressable: {codes:?}");
}

#[test]
fn a_stylesheet_rule_that_does_nothing_is_w1010_and_its_working_twin_is_not() {
    // M25/T25.2. The two rules differ only in the kind's spelling, so a
    // lint firing on every rule fails this as surely as one firing on
    // none.
    use lumen_static::style::lint_sheet_deep;
    use lumen_static::{Color, Patch, Rule, Selector};

    let mut f = Fixture::healthy();
    f.frame();
    let ink = Color(20, 20, 20, 255);
    let sheet = [
        Rule::new(Selector::Kind("Label"), Patch::new().color(ink)),
        Rule::new(Selector::Kind("label"), Patch::new().color(ink)),
    ];
    let mut found = Vec::new();
    lint_sheet_deep(&mut f.app, &sheet, &mut |x| found.push(x));

    let tree = TreeRef::of_root(&f.app, &f.styles, &f.states).with_sheet(&sheet, &found);
    let resp = f.agent.dispatch(
        &tree,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.lint", "params": {} }),
    );
    let w1010: Vec<&Value> = resp["result"]["findings"]
        .as_array()
        .expect("ui.lint returns findings")
        .iter()
        .filter(|x| x["code"] == "W1010")
        .collect();
    println!("{w1010:#?}");
    assert_eq!(w1010.len(), 1, "exactly the misspelled rule: {w1010:?}");
    assert_eq!(w1010[0]["rule"], 0);
    assert!(w1010[0]["message"].as_str().unwrap_or("").contains("matches no node"));

    // A view that carries no sheet reports none: the host supplies it or
    // nothing does.
    let bare = TreeRef::of_root(&f.app, &f.styles, &f.states);
    let resp = f.agent.dispatch(
        &bare,
        &json!({ "jsonrpc": "2.0", "id": 2, "method": "ui.lint", "params": {} }),
    );
    assert!(
        !resp["result"]["findings"].as_array().unwrap().iter().any(|x| x["code"] == "W1010"),
        "a view without a sheet reported a sheet finding"
    );
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Chosen {
    #[widget_id("pick")]
    pick: lumen_static_widgets::PickList,
}

/// **A closed popup's content is absent on purpose, not squeezed** — M57.
///
/// Every closed chooser reported its list as `W1001` ("present in the
/// tree, absent on screen") in the live window, all the time: a closed
/// `Popup` zeroes its subtree exactly as a parent that squeezed it would.
/// The popup now says so (`Node::content_absent`). Both directions: the
/// closed list is quiet, the open one is quiet, and a list with content
/// and no box *not* under a closed popup still fires — the invisible-
/// content test above is that case for a label.
#[test]
fn a_closed_popups_content_is_not_reported_as_invisible() {
    let mut app = Chosen { pick: lumen_static_widgets::PickList::new(&["Apple", "Banana", "Cherry"]) };
    let closed = lint_of(&mut app);
    app.pick.open();
    let open = lint_of(&mut app);
    println!("closed: {closed:?}; open: {open:?}");
    assert!(!closed.iter().any(|c| c == "W1001"), "a closed picklist's list was reported invisible: {closed:?}");
    assert!(!open.iter().any(|c| c == "W1001"), "an open picklist's list was reported invisible: {open:?}");
}
