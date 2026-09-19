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

use lumen_static::ops::Button;
use lumen_static::repeat::Repeat;
use lumen_static::{pass, Constraints, Event, EventCtx, InputQueue, NodeState, StaticTree, Widget};
use lumen_static_agent::{Agent, AgentHost, TreeRef};
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
    head: lumen_static::ops::Label,
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

impl lumen_static::event::App for App {
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
    styles: Vec<lumen_static::Style>,
    input: lumen_static::event::InputState,
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
