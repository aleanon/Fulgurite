//! P3 tests: the ambient audit. Each lint gets a firing case *and* a
//! quiet case — a check that cannot be shown not to fire is a check an
//! agent will learn to ignore.

use fulgurite_widgets::ops::{Fixed, Label, Spacer};
use fulgurite::repeat::Repeat;
use fulgurite::{pass, Constraints, NodeState, StaticTree, Widget};
use fulgurite_agent::{Agent, TreeMut, TreeRef};
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
    styles: Vec<fulgurite::Style>,
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
        self.agent.frame_done(&mut tree, fulgurite_agent::FrameTiming::passes(10, 10));
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

/// A squeeze is not an escape: three children sharing 30 px each get
/// less than they ask for, and that surfaces as truncation (W1005), with
/// every child still inside its parent — so no W1003.
///
/// **This was W1003's tripwire until M55/T55.6**, pinning "constraints
/// flow down and every node clamps to them" as the reason W1003 was
/// reserved. Its fixture held no scroller and no negative padding, the
/// two constructs known to escape, so it could not fail (R16-F2). The
/// escapes are the two tests below.
#[test]
fn a_squeeze_surfaces_as_truncation_not_as_an_escape() {
    let mut f = Fixture::healthy();
    f.view.max_w = 30.0;
    f.app.bar.title.set("far too long for ten pixels");
    f.frame();
    let codes = f.codes();
    println!("squeezed: {codes:?}");
    assert!(codes.iter().any(|c| c == "W1005"), "the squeeze is reported as truncation: {codes:?}");
    assert!(!codes.iter().any(|c| c == "W1003"), "a squeeze is not an escape: {codes:?}");
}

// ---- M55/T55.6 (R16-F2): W1003 ships ------------------------------------

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct NarrowRow {
    a: Label,
    b: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Narrow {
    #[widget_id("row")]
    row: NarrowRow,
}

/// **A child placed outside a parent that does not clip is W1003** — the
/// geometry a node that does not stay in its box leaves behind, built
/// directly: after layout, two labels are moved 60 px past the 50 px row
/// that holds them.
///
/// T55.6 found this escape with a derived row given `width: Fixed(50)`
/// laying its 80 px labels out against the 200 px it was *offered*. M56
/// fixed exactly that (a container lays its content out across its own
/// box), so at the merge that construct no longer escaped and this test
/// could not fire; no derived container escapes now, and M57/B26's four
/// hand-written wrappers were fixed too. The lint still has to see the
/// geometry, which is what this asks of it.
#[test]
fn w1003_fires_on_escape_without_clip() {
    use fulgurite::{Length, Patch, Rule, Selector};
    let mut app = Narrow::default();
    app.row.a.set("aaa");
    app.row.b.set("bbb");
    let sheet = [Rule::new(Selector::Kind("NarrowRow"), Patch::new().width(Length::Fixed(50.0)))];
    let mut styles = pass::default_styles(&app);
    fulgurite::style::cascade_deep(&mut app, &sheet, &mut styles);
    let mut st = vec![NodeState::default(); Narrow::NODE_COUNT];
    pass::layout(&mut app, &styles, &mut st, Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 100.0 });
    // Both labels past the row's right edge, as an escaping node leaves them.
    for i in [2, 3] {
        st[i].pos.x += 60.0;
    }
    let mut agent = Agent::new();
    {
        let mut tm = TreeMut::of_root(&mut app, &styles, &st);
        agent.frame_done(&mut tm, fulgurite_agent::FrameTiming::passes(1, 1));
    }
    let tree = TreeRef::of_root(&app, &styles, &st);
    let resp = agent.dispatch(&tree, &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.lint", "params": {} }));
    let found: Vec<&Value> =
        resp["result"]["findings"].as_array().unwrap().iter().filter(|x| x["code"] == "W1003").collect();
    let at = |i: usize| (st[i].pos.x, st[i].pos.x + st[i].size.w);
    println!("row x {:?}, labels x {:?} and {:?}", at(1), at(2), at(3));
    println!("W1003 findings: {} (expected 2, one per label): {found:?}", found.len());
    assert_eq!(found.len(), 2, "each label outside the non-clipping row must be W1003");
    assert!(found.iter().all(|x| x["detail"]["parent"] == 1), "the row (node 1) is the parent escaped");
}

/// The quiet direction: a scroller's content runs past it on purpose, and
/// the scroller clips — so the same geometry is not W1003 there.
#[test]
fn w1003_quiet_inside_scrollview() {
    let mut app = ScrolledLabels::default();
    for (l, t) in [
        (&mut app.scroller.content.a, "one"),
        (&mut app.scroller.content.b, "two"),
        (&mut app.scroller.content.c, "three"),
        (&mut app.scroller.content.d, "four"),
    ] {
        l.set(t);
    }
    let f = findings_of(&mut app, 200.0, 40.0, "W1003");
    // The setup: the content is taller than its scroller.
    let styles = pass::default_styles(&app);
    let mut st = vec![NodeState::default(); ScrolledLabels::NODE_COUNT];
    pass::layout(&mut app, &styles, &mut st, Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 40.0 });
    let (content, scroller) = (&st[2], &st[1]);
    println!(
        "content bottom {} against the scroller's {}; W1003 findings: {} (expected 0)",
        content.pos.y + content.size.h,
        scroller.pos.y + scroller.size.h,
        f.len()
    );
    assert!(content.pos.y + content.size.h > scroller.pos.y + scroller.size.h, "setup: no overflow");
    assert!(f.is_empty(), "W1003 fired on a scroller's content: {f:?}");
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
        agent.frame_done(&mut tm, fulgurite_agent::FrameTiming::passes(1, 1));
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
    scroller: fulgurite_widgets::scroll::ScrollView<TallBody>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct BadNesting {
    #[widget_id("scroller")]
    scroller: fulgurite_widgets::scroll::ScrollView<fulgurite::repeat::Repeat<TallBody>>,
}

fn lint_of<W>(app: &mut W) -> Vec<String>
where
    W: fulgurite::widget::Node + Widget,
{
    let styles = pass::default_styles(app);
    let mut st = vec![NodeState::default(); W::NODE_COUNT];
    const VIEW: Constraints =
        Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 100.0 };
    pass::layout(app, &styles, &mut st, VIEW);
    let mut agent = Agent::new();
    {
        let mut tm = TreeMut::of_root(app, &styles, &st);
        agent.frame_done(&mut tm, fulgurite_agent::FrameTiming::passes(1, 1));
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
    let mut app = BigList { rows: Repeat::new(1_000_000, 24.0, fulgurite::repeat::noop_bind) };
    let codes = lint_of(&mut app);
    assert!(codes.iter().any(|c| c == "W1009"), "expected W1009, got {codes:?}");

    // Just under the ceiling is silent.
    let mut ok = BigList { rows: Repeat::new(699_000, 24.0, fulgurite::repeat::noop_bind) };
    let codes = lint_of(&mut ok);
    assert!(!codes.iter().any(|c| c == "W1009"), "699k rows at 24 px are addressable: {codes:?}");
}

#[test]
fn a_stylesheet_rule_that_does_nothing_is_w1010_and_its_working_twin_is_not() {
    // M25/T25.2. The two rules differ only in the kind's spelling, so a
    // lint firing on every rule fails this as surely as one firing on
    // none.
    use fulgurite::style::lint_sheet_deep;
    use fulgurite::{Color, Patch, Rule, Selector};

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

// ---- M55/T55.6 (R16-F1): W1002 against the nearest clip -------------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct FourLabels {
    a: Label,
    b: Label,
    c: Label,
    d: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct ScrolledLabels {
    #[widget_id("scroller")]
    scroller: fulgurite_widgets::scroll::ScrollView<FourLabels>,
}

/// The findings of one code for a tree laid out in a `w` x `h` window,
/// with each finding's node index.
fn findings_of<W>(app: &mut W, w: f32, h: f32, code: &str) -> Vec<Value>
where
    W: fulgurite::widget::Node + Widget,
{
    let styles = pass::default_styles(app);
    let mut st = vec![NodeState::default(); W::NODE_COUNT];
    let view = Constraints { min_w: 0.0, max_w: w, min_h: 0.0, max_h: h };
    pass::layout(app, &styles, &mut st, view);
    let mut agent = Agent::new();
    {
        let mut tm = TreeMut::of_root(app, &styles, &st);
        agent.frame_done(&mut tm, fulgurite_agent::FrameTiming::passes(1, 1));
    }
    let tree = TreeRef::of_root(app, &styles, &st);
    let resp = agent.dispatch(
        &tree,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.lint", "params": {} }),
    );
    resp["result"]["findings"]
        .as_array()
        .map(|a| a.iter().filter(|f| f["code"] == code).cloned().collect())
        .unwrap_or_default()
}

/// The review's fixture (R16-F1): a 40 px window holding a `ScrollView`
/// of four labels, so two of them lie below the window unscrolled. Those
/// rows are scrolled-out content, not misplaced nodes: the scroller clips
/// them and scrolling reaches them.
///
/// **The setup is asserted, not assumed**: the content must overflow the
/// window, or the quiet answer would be quiet for want of anything to say.
#[test]
fn w1002_quiet_on_scrolled_out_content() {
    let mut fired = Vec::new();
    for scroll in [0.0f32, 20.0] {
        let mut app = ScrolledLabels::default();
        for (l, t) in [
            (&mut app.scroller.content.a, "one"),
            (&mut app.scroller.content.b, "two"),
            (&mut app.scroller.content.c, "three"),
            (&mut app.scroller.content.d, "four"),
        ] {
            l.set(t);
        }
        app.scroller.scroll = scroll;
        let f = findings_of(&mut app, 200.0, 40.0, "W1002");
        // The setup: the last label's bottom is past the 40 px window.
        let styles = pass::default_styles(&app);
        let mut st = vec![NodeState::default(); ScrolledLabels::NODE_COUNT];
        pass::layout(
            &mut app,
            &styles,
            &mut st,
            Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 40.0 },
        );
        let last = st.len() - 1;
        let bottom = st[last].pos.y + st[last].size.h;
        println!(
            "scroll {scroll}: last row bottom {bottom} against a 40 px window; W1002 findings: {} \
             (expected 0) {f:?}",
            f.len()
        );
        assert!(bottom > 40.0, "setup: the content does not overflow the window");
        fired.push((scroll, f.len()));
    }
    assert_eq!(fired, [(0.0, 0), (20.0, 0)], "W1002 fired on scrolled-out content (scroll, findings)");
}

// ---- M55/T55.5 (R16-F5, R16-F6): W1012, a length the cascade repaired

/// The W1012 findings for one sheet over the healthy fixture.
fn w1012_of(sheet: &[fulgurite::Rule]) -> Vec<Value> {
    use fulgurite::style::lint_sheet_deep;
    let mut f = Fixture::healthy();
    f.frame();
    let mut found = Vec::new();
    lint_sheet_deep(&mut f.app, sheet, &mut |x| found.push(x));
    let tree = TreeRef::of_root(&f.app, &f.styles, &f.states).with_sheet(sheet, &found);
    let resp = f.agent.dispatch(
        &tree,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.lint", "params": {} }),
    );
    resp["result"]["findings"]
        .as_array()
        .expect("ui.lint returns findings")
        .iter()
        .filter(|x| x["code"] == "W1012")
        .cloned()
        .collect()
}

/// One W1012 per poisoned field, and none for legal values. The poisoned
/// sheet sets four fields in one rule and one in another, so a lint that
/// reported per rule, or per node, would count differently.
#[test]
fn a_repaired_length_is_w1012_one_per_field_and_legal_values_are_quiet() {
    use fulgurite::{Edges, Patch, Rule, Selector};

    let poisoned = [
        Rule::new(
            Selector::Kind("App"),
            Patch::new().spacing(f32::NAN).padding(Edges::all(-50.0)).radius(f32::INFINITY),
        ),
        Rule::new(Selector::Kind("label"), Patch::new().font_size(f32::NAN)),
    ];
    let found = w1012_of(&poisoned);
    let fields: Vec<&str> = found.iter().map(|x| x["detail"]["field"].as_str().unwrap_or("?")).collect();
    println!("poisoned: W1012 findings: {} (expected 4): {fields:?}", found.len());
    for x in &found {
        println!("  {}", x["message"].as_str().unwrap_or(""));
    }
    assert_eq!(fields, ["padding", "spacing", "radius", "font_size"], "one W1012 per poisoned field");

    for p in [0.0f32, 8.0] {
        let legal = [Rule::new(
            Selector::Kind("App"),
            Patch::new().spacing(6.0).padding(Edges::all(p)).radius(4.0),
        )];
        let found = w1012_of(&legal);
        println!("padding {p}: W1012 findings: {} (expected 0)", found.len());
        assert!(found.is_empty(), "a legal value was reported as repaired: {found:?}");
    }
}

/// The firing direction (M55/T55.6): a node outside the window with no
/// clipping ancestor. Nothing in this layout model puts one there by
/// itself -- which is W1003's reservation below -- so the fixture moves
/// the healthy frame's badge after layout, the way a widget that placed
/// a child wrongly would.
#[test]
fn w1002_fires_on_a_node_placed_off_root() {
    let mut f = Fixture::healthy();
    pass::layout(&mut f.app, &f.styles, &mut f.states, f.view);
    let badge = fulgurite::style::index_of_wid(App::SUB_TABLE, "badge").expect("badge exists");
    f.states[badge].pos.y = 10_000.0;
    f.states[badge].size = fulgurite::Size { w: 8.0, h: 8.0 };
    {
        let mut tree = TreeMut::of_root(&mut f.app, &f.styles, &f.states);
        f.agent.frame_done(&mut tree, fulgurite_agent::FrameTiming::passes(1, 1));
    }
    let found: Vec<Value> = f.findings().into_iter().filter(|x| x["code"] == "W1002").collect();
    println!(
        "W1002 findings: {} (expected 1); badge {:?}, root {:?}",
        found.len(),
        found.first().map(|x| x["detail"]["bounds"].clone()),
        found.first().map(|x| x["detail"]["viewport"].clone())
    );
    assert_eq!(found.len(), 1, "a node off the root with no clip must be W1002: {found:?}");
    assert_eq!(found[0]["index"], badge);
}

// ---- M55: an open popup's content is out of flow, not an escape ------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Tipped {
    #[widget_id("anchor")]
    anchor: Label,
    #[widget_id("tip")]
    tip: fulgurite_widgets::Popup<Label>,
    #[widget_id("after")]
    after: Label,
}

/// T55.6 shipped W1003 and every open tooltip fired it: a `Popup` is
/// zero-sized at its anchor and places its content below it, so the
/// content is outside its parent's rect by design. `just widget-drive`
/// found it, misattributed (its count-based slice named the next finding
/// in the list, a pre-existing W1005). The setup asserts the content does
/// lie outside the popup's rect, so the quiet answer is not quiet for
/// want of an escape.
#[test]
fn w1003_quiet_on_an_open_popups_content() {
    let mut app = Tipped::default();
    app.anchor.set("hover me");
    app.tip.content.set("a tip that is wider than nothing");
    app.tip.open = true;
    app.after.set("after");
    let f = findings_of(&mut app, 400.0, 200.0, "W1003");
    let styles = pass::default_styles(&app);
    let mut st = vec![NodeState::default(); Tipped::NODE_COUNT];
    pass::layout(&mut app, &styles, &mut st, Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 200.0 });
    let (popup, content) = (&st[2], &st[3]);
    println!(
        "popup {:?}+{:?}, content {:?}+{:?}; W1003 findings: {} (expected 0)",
        popup.pos, popup.size, content.pos, content.size, f.len()
    );
    assert!(content.size.w > popup.size.w, "setup: the content does not escape the popup's rect");
    assert!(f.is_empty(), "W1003 fired on an open popup's content: {f:?}");
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Chosen {
    #[widget_id("pick")]
    pick: fulgurite_widgets::PickList,
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
    let mut app = Chosen { pick: fulgurite_widgets::PickList::new(&["Apple", "Banana", "Cherry"]) };
    let closed = lint_of(&mut app);
    app.pick.open();
    let open = lint_of(&mut app);
    println!("closed: {closed:?}; open: {open:?}");
    assert!(!closed.iter().any(|c| c == "W1001"), "a closed picklist's list was reported invisible: {closed:?}");
    assert!(!open.iter().any(|c| c == "W1001"), "an open picklist's list was reported invisible: {open:?}");
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Selected {
    #[widget_id("sel")]
    sel: fulgurite_widgets::select::Select<()>,
}

fn fruit(r: &mut fulgurite_widgets::select::Row<()>, i: usize) {
    r.label.clear();
    r.label.push_str(["Apple", "Banana", "Cherry"][i]);
}

/// **A closed menu's list is not a collapsed collection** — the M56
/// merge. `W1007` is "a list with rows that shows none, or has no box",
/// and a closed `Select`'s menu is exactly that by design: the live drive
/// linted the demo after choosing a row and found `W1007` on
/// `#select-menu`, six rows in a 0×0 box. The collection check now
/// honours `Node::content_absent` as the leaves' checks already did.
/// Both directions: closed and open are quiet, and a list with rows and
/// no box *not* under a popup still fires
/// (`a_collection_with_items_but_no_height_is_found`).
#[test]
fn a_closed_menus_list_is_not_a_collapsed_collection() {
    let mut app = Selected { sel: fulgurite_widgets::select::Select::new(3, 30.0, fruit) };
    let closed = lint_of(&mut app);
    app.sel.open();
    let open = lint_of(&mut app);
    app.sel.close();
    let reclosed = lint_of(&mut app);
    println!("closed: {closed:?}; open: {open:?}; closed again: {reclosed:?}");
    for (when, codes) in [("closed", &closed), ("open", &open), ("closed again", &reclosed)] {
        assert!(!codes.iter().any(|c| c == "W1007"), "the {when} select's menu was reported collapsed: {codes:?}");
    }
}
