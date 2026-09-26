//! **The tree says what the widgets say** — M48/T48.0, T48.5 and T48.6
//! (R15-F10, R13-F4, `docs/plan-agent-truth.md` §1.2).
//!
//! Widgets set state on their AccessKit node that a screen reader
//! announces — a checkbox's `toggled`, a slider's numeric value, a
//! dropdown's `expanded`, a dialog's `modal`, a branch's `hidden` — and
//! every agent reply that serialised a node wrote only id, role, value,
//! label, bounds, children, src and wid. So a checked box and a moved
//! slider were invisible to `ui.getTree`, `ui.resolve` and `ui.getLayout`
//! alike.
//!
//! Each field is asserted on the widget that sets it, and the checkbox is
//! read before and after a click: an unchecked box reports `"false"`, not
//! an absent field, which is what shows the field reads live state.

use lumen_static::event::EventCtx;
use lumen_static::hatch::{Either, Side};
use lumen_static::prelude::*;
use lumen_static::retain::Incremental;
use lumen_static::{Everything, Widget};
use lumen_static_agent::{Agent, TreeRef};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;
use serde_json::{json, Value};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Shown {
    #[widget_id("shown")]
    l: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Gone {
    #[widget_id("gone")]
    l: Label,
}

#[derive(StaticTree)]
#[widget(column, spacing = 4.0)]
struct Ui {
    #[widget_id("check")]
    check: CheckBox<Store>,
    #[widget_id("slider")]
    slider: Slider,
    #[widget_id("chooser")]
    chooser: PickList,
    #[widget_id("modal")]
    modal: Modal<Label>,
    #[widget_id("either")]
    either: Either<Shown, Gone>,
}

impl Default for Ui {
    fn default() -> Self {
        Ui {
            check: CheckBox::default(),
            slider: Slider::default(),
            chooser: PickList::default(),
            modal: Modal::default(),
            either: Either::new(Shown::default(), Gone::default()),
        }
    }
}

#[derive(Default)]
struct Store {
    on: bool,
}

#[derive(DeriveApp, Default)]
struct Fx {
    #[tree]
    ui: Ui,
    #[state]
    #[bind(ui.check.on, on)]
    store: Store,
}

type H = Host<Fx, Incremental, Everything>;

fn host() -> H {
    let mut app = Fx::default();
    app.ui.check.on_toggle = |s: &mut Store, _: &EventCtx| s.on = !s.on;
    app.ui.chooser.list.content.set_options(&["one", "two", "three"]);
    app.ui.either.a.l.set("shown");
    app.ui.either.b.l.set("gone");
    app.ui.either.show(Side::A);
    let mut h: H = Host::new(app, 480, 480);
    h.pump();
    h.pump();
    h
}

fn call(h: &H, method: &str, params: Value) -> Value {
    let tree = TreeRef::of_root(&h.app.ui, &h.styles, &h.states);
    let r = Agent::new().dispatch(&tree, &json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }));
    assert!(r.get("error").is_none(), "{method} failed: {r}");
    r["result"].clone()
}

/// The `ui.getTree` node carrying author id `wid`.
fn node(tree: &Value, wid: &str) -> Value {
    tree["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .find(|n| n["wid"] == wid)
        .cloned()
        .unwrap_or_else(|| panic!("no node #{wid} in the tree"))
}

fn click(h: &mut H, wid: &str) {
    let i = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).expect(wid);
    let s = h.states[i];
    h.click(s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    h.pump();
}

/// T48.5: every field on its widget, through `ui.getTree`.
#[test]
fn get_tree_carries_the_state_a_screen_reader_announces() {
    let mut h = host();
    let before = node(&call(&h, "ui.getTree", json!({})), "check");
    println!("unchecked: toggled {}", before["toggled"]);
    assert_eq!(before["toggled"], "false", "an unchecked box must say so, not omit the field: {before}");

    click(&mut h, "check");
    assert!(h.app.store.on, "setup: the click did not toggle the box");
    h.app.ui.slider.set_value(0.25);
    h.app.ui.chooser.open();
    h.app.ui.modal.open = true;
    h.invalidate_all();
    h.pump();

    let tree = call(&h, "ui.getTree", json!({}));
    let check = node(&tree, "check");
    let slider = node(&tree, "slider");
    let chooser = node(&tree, "chooser");
    let modal = node(&tree, "modal");
    let gone = node(&tree, "gone");
    let checks: [(&str, bool, String); 6] = [
        ("toggled", check["toggled"] == "true", check["toggled"].to_string()),
        (
            "numeric",
            slider["numeric"]["value"].as_f64().is_some_and(|v| (v - 0.25).abs() < 1e-6),
            slider["numeric"].to_string(),
        ),
        ("expanded", chooser["expanded"] == true, chooser["expanded"].to_string()),
        ("modal", modal["modal"] == true, modal["modal"].to_string()),
        ("hidden", gone["hidden"] == true, gone["hidden"].to_string()),
        (
            "actions",
            check["actions"].as_array().is_some_and(|a| a.iter().any(|x| x == "click")),
            check["actions"].to_string(),
        ),
    ];
    let present = checks.iter().filter(|c| c.1).count();
    for (name, ok, got) in &checks {
        println!("  {name:9} {} (got {got})", if *ok { "ok" } else { "MISSING" });
    }
    println!("fields present: {present} / {}", checks.len());
    assert_eq!(present, checks.len(), "fields present: {present} / {}", checks.len());
    // And a node that is showing is not marked hidden.
    let shown = node(&tree, "shown");
    assert!(shown.get("hidden").is_none(), "a showing node reported hidden: {shown}");
}

/// T48.5: `observe::dump` and `ui.getTree` over the same tree are
/// byte-identical — one serialiser, not two copies that agree today.
#[test]
fn dump_and_get_tree_are_one_serialiser() {
    let mut h = host();
    click(&mut h, "check");
    h.app.ui.slider.set_value(0.25);
    h.app.ui.chooser.open();
    h.invalidate_all();
    h.pump();
    let dump = lumen_static::observe::dump(&h.app.ui, &h.states);
    let tree = call(&h, "ui.getTree", json!({}));
    let (a, b) = (dump["nodes"].as_array().unwrap(), tree["nodes"].as_array().unwrap());
    let first = a.iter().zip(b.iter()).position(|(x, y)| x != y);
    println!(
        "dump {} nodes, getTree {} nodes, {} bytes each; first differing node: {first:?}",
        a.len(),
        b.len(),
        dump.to_string().len()
    );
    if let Some(k) = first {
        panic!("dump and ui.getTree differ at node {k}:\n dump:    {}\n getTree: {}", a[k], b[k]);
    }
    assert_eq!(dump.to_string(), tree.to_string(), "same nodes, different documents");
}

/// T48.6: `ui.resolve` is a second call path, and reads the same fields.
#[test]
fn resolve_sees_a_checked_box_and_a_moved_slider() {
    let mut h = host();
    let before = call(&h, "ui.resolve", json!({ "selector": "#check" }));
    click(&mut h, "check");
    assert!(h.app.store.on, "setup: the click did not toggle the box");
    let after = call(&h, "ui.resolve", json!({ "selector": "#check" }));
    println!("checked: {} -> {}", before["toggled"], after["toggled"]);
    assert_eq!((before["toggled"].clone(), after["toggled"].clone()), (json!("false"), json!("true")));

    let s0 = call(&h, "ui.getLayout", json!({ "selector": "#slider" }));
    h.app.ui.slider.set_value(0.9);
    h.invalidate_all();
    h.pump();
    let s1 = call(&h, "ui.getLayout", json!({ "selector": "#slider" }));
    println!("slider: {} -> {}", s0["numeric"]["value"], s1["numeric"]["value"]);
    assert!(
        s1["numeric"]["value"].as_f64().is_some_and(|v| (v - 0.9).abs() < 1e-6),
        "ui.getLayout does not report the slider at 0.9: {s1}"
    );
}

// ---- coverage: every leaf a tree can hold describes itself -------------

#[derive(StaticTree)]
#[widget(column, spacing = 2.0)]
struct Everyone {
    #[widget_id("label")]
    label: Label,
    #[widget_id("badge")]
    badge: Badge,
    #[widget_id("chip")]
    chip: Chip,
    #[widget_id("avatar")]
    avatar: Avatar,
    #[widget_id("progressbar")]
    progress: ProgressBar,
    #[widget_id("spinner")]
    spinner: Spinner,
    #[widget_id("card")]
    card: Card<Label>,
    #[widget_id("toast")]
    toast: Toast,
    #[widget_id("tooltip")]
    tooltip: Tooltip<Label>,
    #[widget_id("rule")]
    divider: Divider,
    #[widget_id("skeleton")]
    skeleton: Skeleton,
    #[widget_id("icon")]
    icon: Icon,
    #[widget_id("slider")]
    slider: Slider,
    #[widget_id("check")]
    check: CheckBox<()>,
}

impl Default for Everyone {
    fn default() -> Self {
        Everyone {
            label: Label::new("label"),
            badge: Badge::new("3"),
            chip: Chip::new("tag"),
            avatar: Avatar { initials: "AB" },
            progress: ProgressBar::new(0.4),
            spinner: Spinner::new(0.0),
            card: Card::new(Label::new("card body")),
            toast: Toast::new("saved"),
            tooltip: Tooltip::new(Label::new("hover me"), "a hint"),
            divider: Divider,
            skeleton: Skeleton::new(0.0),
            icon: Icon::default(),
            slider: Slider::default(),
            check: CheckBox::default(),
        }
    }
}

/// Kinds that stay `GenericContainer`, and why. Anything else must
/// publish a role (T48.6).
const ALLOWED_GENERIC: &[(&str, &str)] = &[
    ("rule", "a divider is decoration: it has nothing to announce"),
    ("skeleton", "a loading placeholder is decoration; the busy state belongs to what replaces it"),
];

/// T48.6: a coverage walk. Each leaf kind here either publishes a role
/// other than `GenericContainer`, or is on the allow list with a reason.
#[test]
fn every_leaf_kind_describes_itself() {
    let mut ui = Everyone::default();
    let styles = lumen_static::pass::default_styles(&ui);
    let mut states = vec![lumen_static::NodeState::default(); Everyone::NODE_COUNT];
    let view = lumen_static::Constraints { min_w: 0.0, max_w: 480.0, min_h: 0.0, max_h: 1200.0 };
    lumen_static::pass::layout(&mut ui, &styles, &mut states, view);
    let tree = TreeRef::of_root(&ui, &styles, &states);
    let r = Agent::new().dispatch(&tree, &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.getTree" }));
    let nodes = r["result"]["nodes"].as_array().cloned().expect("nodes");
    let (mut kinds, mut generic, mut allowed) = (0, Vec::new(), 0);
    // The root's own fields: a composite's part ids (`tooltip-tip`) are
    // its business, and its own role is what is asked.
    for d in <Everyone as Widget>::SUB_TABLE.iter().filter(|d| !d.wid.is_empty() && d.parent == 0) {
        let n = nodes.iter().find(|n| n["wid"] == d.wid).expect("every wid is in the tree");
        kinds += 1;
        if n["role"] != "GenericContainer" {
            continue;
        }
        if ALLOWED_GENERIC.iter().any(|(k, _)| *k == d.wid) {
            allowed += 1;
        } else {
            generic.push(d.wid);
        }
    }
    println!("{kinds} kinds, {} generic, allowed {allowed}: {generic:?}", generic.len() + allowed);
    assert!(generic.is_empty(), "kinds that publish nothing and are not allowed: {generic:?}");
}
