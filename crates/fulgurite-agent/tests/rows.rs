//! M27/T27.0 — addressing a *repeater row*, which nothing could do.
//!
//! A repeater's rows are not on the const table: N materialized rows
//! share the template's single `NodeDesc`, so there is one `wid` for all
//! of them however many exist. The agent enumerates them anyway, under a
//! synthetic id, and then every action method refused them — by name, in
//! as many words: *"selector resolved to virtualized content; pass x/y
//! instead"*.
//!
//! Taking that advice is what `scripts/agent_drive.sh` did, and the
//! `ROW_H * 8` arithmetic it produced is the first vacuous shape AGENT.md
//! names: a click computed from a box, one renamed row away from landing
//! on the wrong widget and passing anyway.
//!
//! Two things had to change and this file holds one test for each:
//!
//! 1. **the selector could not name a row.** `~text` was the one term
//!    that matches an instance node at all — it reads the AccessKit node
//!    rather than the table, so it needs no index — but it consulted
//!    `value()` alone, and `Button` publishes `set_label`. It missed by
//!    one field.
//! 2. **the action could not act on one.** `point_param` required a
//!    table index solely to recompute a centre that the entry's own
//!    bounds already carried.

use fulgurite_widgets::ops::Button;
use fulgurite::repeat::Repeat;
use fulgurite::{pass, Constraints, Event, EventCtx, InputQueue, NodeState, StaticTree, Widget};
use fulgurite_agent::{Agent, AgentHost, TreeRef};
use serde_json::{json, Value};

/// A menu row: the same shape `Select` uses, which is the thing this
/// exists to make drivable.
type Row = Button<App, String>;

/// Two of these are a **prefix pair** — `Rich` and `RichEditor` — which
/// is the shape `~=` exists for (M27/T27.12): under `contains` alone,
/// "the row named Rich" stops being expressible the moment the second
/// one exists.
const NAMES: [&str; 6] = ["Alpha", "Beta", "Gamma", "Delta", "Rich", "RichEditor"];
const ROW_H: f32 = 20.0;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("head")]
    head: fulgurite_widgets::ops::Label,
    #[widget_id("menu")]
    menu: Repeat<Row>,
}

#[derive(Default)]
struct App {
    ui: Ui,
    /// Which row's handler ran, and how many times anything ran.
    picked: Option<usize>,
    clicks: u32,
}

impl fulgurite::event::App for App {
    // M33: this app keeps handlers on the whole of itself.
    type State = Self;
    fn state_mut(&mut self) -> &mut Self::State {
        self
    }
    // M28/T28.2: this fixture defers no work. `Never` is
    // uninhabited, so nothing can be sent and `apply` cannot
    // be called.
    type Msg = fulgurite::event::Never;
    fn apply(_: &mut Self, m: Self::Msg) -> bool {
        match m {}
    }
    type Tree = Ui;
    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
}

/// Every row is the same `fn`; `EventCtx::row` is what tells it apart.
fn bind(r: &mut Row, i: usize) {
    r.label = NAMES[i].to_string();
    r.on_click = |a: &mut App, cx: &EventCtx| {
        a.picked = cx.row.map(|r| r as usize);
        a.clicks += 1;
    };
}

/// Tall enough that all six rows materialize: a row outside the viewport
/// is not announced at all, and that is a *different* problem (T27.0
/// landing 3) which this file must not accidentally be testing.
const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 400.0 };

struct Host {
    app: App,
    states: [NodeState; Ui::NODE_COUNT],
    styles: Vec<fulgurite::Style>,
    input: fulgurite::event::InputState,
    queue: InputQueue,
    frame: u64,
}

impl Host {
    fn new() -> Self {
        let mut app = App::default();
        app.ui.head.set("header text");
        app.ui.menu = Repeat::new(NAMES.len(), ROW_H, bind);
        let styles = pass::default_styles(&app.ui);
        let mut h = Host {
            app,
            states: [NodeState::default(); Ui::NODE_COUNT],
            styles,
            input: Default::default(),
            queue: InputQueue::new(),
            frame: 0,
        };
        h.layout();
        h
    }

    fn layout(&mut self) {
        pass::layout(&mut self.app.ui, &self.styles, &mut self.states, VIEW);
    }
}

impl AgentHost for Host {
    fn tree(&self) -> TreeRef<'_> {
        TreeRef::of_root(&self.app.ui, &self.styles, &self.states)
    }
    fn enqueue(&mut self, ev: Event) {
        self.queue.push(ev);
    }
    fn pump(&mut self) -> (u64, bool) {
        let mut handled = false;
        while let Some(ev) = self.queue.pop() {
            handled |= pass::dispatch(
                &mut self.app,
                &self.styles,
                &mut self.states,
                &mut self.input,
                &ev,
            );
        }
        self.layout();
        self.frame += 1;
        (self.frame, handled)
    }
}

fn call(h: &mut Host, agent: &Agent, method: &str, params: Value) -> Value {
    let req = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    agent.dispatch_input(h, &req)
}

/// **The setup itself, asserted.** Six rows at 20 px in a 400 px window
/// must all be materialized; if the layout silently produced none, every
/// test below would agree about a menu that is not there — which is the
/// "the setup silently did not happen" shape, and the reason this is a
/// test and not a comment.
#[test]
fn all_six_rows_are_materialized() {
    let h = Host::new();
    let agent = Agent::new();
    let mut h = h;
    let r = call(&mut h, &agent, "ui.getTree", json!({ "depth": 6 }));
    let nodes = r["result"]["nodes"].as_array().expect("a tree");
    let labelled: Vec<&str> =
        nodes.iter().filter_map(|n| n.get("label").and_then(Value::as_str)).collect();
    for name in NAMES {
        assert!(
            labelled.iter().any(|l| l.contains(name)),
            "row {name:?} was never announced; the tree holds {labelled:?}"
        );
    }
}

/// (1) The selector reaches a row by the text it shows.
#[test]
fn a_row_resolves_by_the_text_it_shows() {
    let mut h = Host::new();
    let agent = Agent::new();
    let r = call(&mut h, &agent, "ui.resolve", json!({ "selector": "~Beta" }));
    assert!(r.get("error").is_none(), "`~Beta` did not resolve: {r}");
    let b = r["result"]["bounds"].as_array().expect("bounds");
    let (w, h_) = (
        b[2].as_f64().unwrap() - b[0].as_f64().unwrap(),
        b[3].as_f64().unwrap() - b[1].as_f64().unwrap(),
    );
    assert!(w > 0.0 && h_ > 0.0, "the row resolved to an empty box: {w}x{h_}");
    // Printed beside the bound: a row is ROW_H tall, and a resolution
    // that quietly returned the *menu's* box would be 6x that.
    assert!(
        (h_ - ROW_H as f64).abs() < 1.0,
        "resolved height {h_} is not one row ({ROW_H}); the whole menu is {}",
        ROW_H as f64 * NAMES.len() as f64
    );
}

/// (2) And clicking it reaches that row's handler — *that* row's.
#[test]
fn clicking_a_row_by_text_reaches_its_own_handler() {
    let mut h = Host::new();
    let agent = Agent::new();
    let r = call(&mut h, &agent, "input.click", json!({ "selector": "~Delta" }));
    assert!(r.get("error").is_none(), "clicking `~Delta` failed: {r}");
    assert_eq!(h.app.clicks, 1, "no handler ran at all");
    assert_eq!(
        h.app.picked,
        Some(3),
        "the click landed on row {:?}, not Delta (3)",
        h.app.picked
    );
}

/// The same term still matches a `value`, which is all it ever matched.
///
/// Without this the widening is only ever exercised in one direction, and
/// a `label()`-only implementation would pass every other test here.
#[test]
fn matching_by_value_still_works() {
    let mut h = Host::new();
    let agent = Agent::new();
    let r = call(&mut h, &agent, "ui.resolve", json!({ "selector": "~header" }));
    assert!(r.get("error").is_none(), "`~header` no longer resolves the Label: {r}");
}

/// A gate that accepts everything is not a gate: text nothing shows must
/// still miss, and must not silently click something else.
#[test]
fn text_no_row_shows_matches_nothing() {
    let mut h = Host::new();
    let agent = Agent::new();
    let r = call(&mut h, &agent, "input.click", json!({ "selector": "~Omega" }));
    assert!(r.get("error").is_some(), "`~Omega` matched something: {r}");
    assert_eq!(h.app.clicks, 0, "a selector that matched nothing still clicked");
}

/// **`~=` is exact and `~` is not** (M27/T27.12).
///
/// One widget's name can be a prefix of another's — `RichText` and
/// `RichTextEditor` — and under `contains` "the row named RichText"
/// stops being expressible the moment the second exists. Both
/// directions here: the loose form must still match both, or the fix is
/// a grammar that lost a capability rather than gaining one.
#[test]
fn an_exact_text_selector_picks_one_of_two_prefixed_rows() {
    let h = Host::new();
    let a = Agent::new();
    let tree = TreeRef::of_root(&h.app.ui, &h.styles, &h.states);
    let loose = a.dispatch(
        &tree,
        &json!({"jsonrpc":"2.0","id":1,"method":"ui.resolve",
                "params":{"selector":"~Rich"}}),
    );
    assert_eq!(
        loose["error"]["code"], -32002,
        "`~Rich` must stay ambiguous across `Rich` and `RichEditor`: {loose}"
    );
    let exact = a.dispatch(
        &tree,
        &json!({"jsonrpc":"2.0","id":2,"method":"ui.resolve",
                "params":{"selector":"~=Rich"}}),
    );
    assert!(
        exact.get("result").is_some(),
        "`~=Rich` must name exactly the row that says `Rich`: {exact}"
    );
    let longer = a.dispatch(
        &tree,
        &json!({"jsonrpc":"2.0","id":3,"method":"ui.resolve",
                "params":{"selector":"~=RichEditor"}}),
    );
    assert!(
        longer.get("result").is_some(),
        "`~=RichEditor` must name the other one: {longer}"
    );
    assert_ne!(
        exact["result"]["id"], longer["result"]["id"],
        "the two exact selectors named the same node"
    );
}

/// **M24/T24.4: a row is addressable by its index.**
///
/// The one thing a part id cannot reach. An arena row is a template
/// instantiated per index and carries no `#[widget_id]`, so until `%k`
/// the only ways to name one were the text it happened to show — safe
/// exactly while that text is unique — and arithmetic over a resolved
/// box, which is the shape `agent_drive.sh` used and AGENT.md's table
/// names twice.
#[test]
fn a_row_resolves_by_its_index() {
    let h = Host::new();
    let a = Agent::new();
    let tree = TreeRef::of_root(&h.app.ui, &h.styles, &h.states);
    for (k, want) in NAMES.iter().enumerate() {
        let r = a.dispatch(
            &tree,
            &json!({"jsonrpc":"2.0","id":1,"method":"ui.resolve",
                    "params":{"selector":format!("#menu > %{k}")}}),
        );
        let label = r["result"]["label"].as_str().unwrap_or("");
        assert_eq!(
            label, *want,
            "`#menu > %{k}` named {label:?}, not the {k}th row: {r}"
        );
    }
}

/// And the row is *inside its owner*: the descendant step reaches an
/// arena, which it could not before — a row has no table index, so a
/// filter written over the table's parent chain skipped every one.
#[test]
fn a_descendant_step_reaches_a_repeaters_rows() {
    let h = Host::new();
    let a = Agent::new();
    let tree = TreeRef::of_root(&h.app.ui, &h.styles, &h.states);
    let inside = a.dispatch(
        &tree,
        &json!({"jsonrpc":"2.0","id":1,"method":"ui.resolve",
                "params":{"selector":"#menu > ~=Gamma"}}),
    );
    assert!(
        inside.get("result").is_some(),
        "`#menu > ~=Gamma` matched nothing; a row is not reachable through its \
         owner: {inside}"
    );
    // And the step still *excludes*: the header is not in the menu.
    let outside = a.dispatch(
        &tree,
        &json!({"jsonrpc":"2.0","id":2,"method":"ui.resolve",
                "params":{"selector":"#menu > #head"}}),
    );
    assert!(
        outside.get("error").is_some(),
        "`#menu > #head` resolved; the step matches things outside its owner: \
         {outside}"
    );
}

/// A bare `%k` names one row *per repeater*, and this tree has one — so
/// it resolves. The point of the pairing with an owner is what happens
/// when there are two, which `ui.resolve` reports rather than guessing.
#[test]
fn a_row_index_past_the_end_matches_nothing() {
    let h = Host::new();
    let a = Agent::new();
    let tree = TreeRef::of_root(&h.app.ui, &h.styles, &h.states);
    let r = a.dispatch(
        &tree,
        &json!({"jsonrpc":"2.0","id":1,"method":"ui.resolve",
                "params":{"selector":format!("%{}", NAMES.len() + 3)}}),
    );
    assert!(
        r.get("error").is_some(),
        "a row index past the last row resolved to something: {r}"
    );
}

// ---- two repeaters, which is what makes "inside" mean anything -------

const LEFT: [&str; 3] = ["one", "two", "three"];
const RIGHT: [&str; 3] = ["red", "green", "blue"];

type TwoRow = Button<TwoApp, String>;

fn bind_left(r: &mut TwoRow, i: usize) {
    r.label = LEFT[i].to_string();
}

fn bind_right(r: &mut TwoRow, i: usize) {
    r.label = RIGHT[i].to_string();
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Two {
    #[widget_id("left")]
    left: Repeat<TwoRow>,
    #[widget_id("right")]
    right: Repeat<TwoRow>,
}

#[derive(Default)]
struct TwoApp {
    ui: Two,
}

impl fulgurite::event::App for TwoApp {
    // M33: this app keeps handlers on the whole of itself.
    type State = Self;
    fn state_mut(&mut self) -> &mut Self::State {
        self
    }
    // M28/T28.2: this fixture defers no work. `Never` is
    // uninhabited, so nothing can be sent and `apply` cannot
    // be called.
    type Msg = fulgurite::event::Never;
    fn apply(_: &mut Self, m: Self::Msg) -> bool {
        match m {}
    }
    type Tree = Two;
    fn tree(&self) -> &Two {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Two {
        &mut self.ui
    }
}

/// **`#owner > %k` names *that* owner's row** (M24/T24.4).
///
/// With one repeater in the tree, "is this row inside this owner" cannot
/// be wrong — an injection answering `true` for every row left the
/// single-repeater tests green, which is a finding about them. Two
/// repeaters is the smallest fixture where the question has an answer.
#[test]
fn a_row_index_is_scoped_to_the_repeater_that_owns_it() {
    let mut app = TwoApp::default();
    app.ui.left = Repeat::new(LEFT.len(), ROW_H, bind_left);
    app.ui.right = Repeat::new(RIGHT.len(), ROW_H, bind_right);
    let styles = pass::default_styles(&app.ui);
    let mut states = vec![NodeState::default(); Two::NODE_COUNT];
    pass::layout(&mut app.ui, &styles, &mut states, VIEW);
    let a = Agent::new();
    let tree = TreeRef::of_root(&app.ui, &styles, &states);

    let name = |sel: &str| -> String {
        let r = a.dispatch(
            &tree,
            &json!({"jsonrpc":"2.0","id":1,"method":"ui.resolve",
                    "params":{"selector":sel}}),
        );
        r["result"]["label"].as_str().unwrap_or("").to_string()
    };
    assert_eq!(name("#left > %0"), "one", "`#left > %0` named the wrong row");
    assert_eq!(name("#right > %0"), "red", "`#right > %0` named the wrong row");
    assert_eq!(name("#left > %2"), "three");
    assert_eq!(name("#right > %2"), "blue");

    // And a bare `%0` is now ambiguous — two repeaters have one — which
    // the agent must *report* rather than pick one and be believed.
    let bare = a.dispatch(
        &tree,
        &json!({"jsonrpc":"2.0","id":2,"method":"ui.resolve",
                "params":{"selector":"%0"}}),
    );
    assert_eq!(
        bare["error"]["code"], -32002,
        "a bare `%0` with two repeaters in the tree should be ambiguous: {bare}"
    );
}
