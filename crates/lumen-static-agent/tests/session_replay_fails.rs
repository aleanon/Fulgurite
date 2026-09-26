//! **A replay that diverges is an error** — M48/T48.0 and T48.10 (R13-F6,
//! `docs/plan-agent-truth.md` §1.3).
//!
//! `Recording::replay` discarded every reply (`let _ =`), `check` took no
//! stylesheet although the module doc names it as a header field whose
//! mismatch is "a hard error rather than a diff", and `export_test`
//! emitted `let _ =` per step and a placeholder comment where the
//! assertions belong. So a corrupted recording replayed `Ok`, a
//! mismatched sheet replayed `Ok`, and an exported test with a selector
//! that no longer exists compiled and passed (the review's probes
//! 580f9f6). Each case here is one of those, with its expectation
//! inverted; the accept direction is `tests/session.rs`'s round trip.

use lumen_static_widgets::ops::{Button, Label};
use lumen_static::{pass, Constraints, Event, EventCtx, InputQueue, NodeState, StaticTree, Widget};
use lumen_static_agent::session::{Header, Recording, ReplayError};
use lumen_static_agent::{Agent, AgentHost, TreeRef};
use serde_json::{json, Value};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("count")]
    count: Label<String>,
    #[widget_id("inc")]
    inc: Button<App>,
    #[widget_id("dec")]
    dec: Button<App>,
}

#[derive(Default)]
struct App {
    ui: Ui,
    n: i32,
}

impl lumen_static::event::App for App {
    // M33: this app keeps handlers on the whole of itself.
    type State = Self;
    fn state_mut(&mut self) -> &mut Self::State {
        self
    }
    // M28/T28.2: this fixture defers no work. `Never` is
    // uninhabited, so nothing can be sent and `apply` cannot
    // be called.
    type Msg = lumen_static::event::Never;
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
    fn update(&mut self) -> bool {
        // `set_str` compares before it writes, so this reports
        // precisely (T16.10).
        self.ui.count.set_str(&self.n.to_string())
    }
}

const W: u32 = 200;
const H: u32 = 200;
const VIEW: Constraints =
    Constraints { min_w: 0.0, max_w: W as f32, min_h: 0.0, max_h: H as f32 };

struct Host {
    app: App,
    styles: Vec<lumen_static::Style>,
    states: [NodeState; Ui::NODE_COUNT],
    queue: InputQueue,
    input: lumen_static::event::InputState,
}

impl Host {
    fn new() -> Self {
        let mut app = App::default();
        app.ui.inc.label = "+";
        app.ui.inc.on_click = |a: &mut App, _: &EventCtx| a.n += 1;
        app.ui.dec.label = "-";
        app.ui.dec.on_click = |a: &mut App, _: &EventCtx| a.n -= 1;
        let styles = pass::default_styles(&app.ui);
        let mut h = Host {
            app,
            styles,
            states: [NodeState::default(); Ui::NODE_COUNT],
            queue: InputQueue::new(),
            input: Default::default(),
        };
        h.pump();
        h
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
        lumen_static::event::App::update(&mut self.app);
        pass::layout(&mut self.app.ui, &self.styles, &mut self.states, VIEW);
        (1, handled)
    }
}

fn header() -> Header {
    Header {
        width: W,
        height: H,
        stylesheet: "default".into(),
        measurer: "block".into(),
        start_frame: 0,
    }
}


fn drive(h: &mut Host, agent: &Agent, rec: &mut Recording, method: &str, params: Value) -> Value {
    let reply = agent.dispatch_input(h, &json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }));
    rec.note(method, &params, &reply);
    reply
}

const PROPS: &str = "(200, 200, \"default\", \"block\")";

fn final_tree(h: &mut Host, agent: &Agent) -> Value {
    agent.dispatch_input(h, &json!({ "jsonrpc": "2.0", "id": 0, "method": "ui.getTree" }))["result"].clone()
}

#[test]
fn a_corrupted_recording_diverges() {
    let agent = Agent::new();
    let mut h = Host::new();
    let mut rec = Recording::new(header());
    for _ in 0..3 {
        drive(&mut h, &agent, &mut rec, "input.click", json!({ "selector": "#inc" }));
    }
    assert_eq!(h.app.n, 3, "setup: three clicks landed");
    // One step's selector no longer exists; another clicks empty space.
    rec.calls[1].1 = json!({ "selector": "#nope" });
    rec.calls[2].1 = json!({ "x": 199.0, "y": 199.0 });
    let mut fresh = Host::new();
    let r = rec.replay(&agent, &mut fresh, W, H, "default", "block");
    println!("replay -> {r:?}, fresh n={} / recorded {}", fresh.app.n, h.app.n);
    match r {
        Err(ReplayError::Diverged { step: 2, .. }) => {}
        other => panic!("a corrupted recording must diverge at step 2, got {other:?} (fresh n={})", fresh.app.n),
    }
}

#[test]
fn a_stylesheet_mismatch_is_refused() {
    let agent = Agent::new();
    let mut rec = Recording::new(Header { stylesheet: "dark-compact".into(), ..header() });
    rec.note("input.click", &json!({ "selector": "#inc" }), &json!({ "result": { "handled": true } }));
    let mut fresh = Host::new();
    let r = rec.replay(&agent, &mut fresh, W, H, "default", "block");
    println!("replay -> {r:?} (recorded `dark-compact`, host `default`), fresh n={}", fresh.app.n);
    assert!(matches!(r, Err(ReplayError::Mismatch { field: "stylesheet", .. })), "{r:?}");
    assert_eq!(fresh.app.n, 0, "and a refused replay runs nothing");
}

// ---- exports, run: the generated source, included and called ----------

/// The recording both exports below are made from; `missing` adds a step
/// on a selector that does not exist.
fn recorded(missing: bool) -> (Recording, Value) {
    let agent = Agent::new();
    let mut h = Host::new();
    let mut rec = Recording::new(header());
    drive(&mut h, &agent, &mut rec, "input.click", json!({ "selector": "#inc" }));
    drive(&mut h, &agent, &mut rec, "input.click", json!({ "selector": "#inc" }));
    if missing {
        drive(&mut h, &agent, &mut rec, "input.click", json!({ "selector": "#does-not-exist" }));
    }
    let tree = final_tree(&mut h, &agent);
    (rec, tree)
}

mod exported {
    use super::Host;
    include!("exported/untouched.rs");
    include!("exported/missing_selector.rs");
}

/// **The committed exports are the exporter's output**, regenerated and
/// compared byte for byte — so the two included files cannot drift into
/// hand-written tests. `LUMEN_BLESS=1` rewrites them.
#[test]
fn the_included_exports_are_what_the_exporter_writes() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/exported");
    for (file, name, missing) in [("untouched.rs", "untouched", false), ("missing_selector.rs", "missing_selector", true)] {
        let (rec, tree) = recorded(missing);
        let src = rec.export_fn(name, "Host::new()", PROPS, &tree);
        let path = dir.join(file);
        if std::env::var("LUMEN_BLESS").is_ok_and(|v| v == "1") {
            std::fs::write(&path, &src).unwrap();
        }
        let have = std::fs::read_to_string(&path).unwrap_or_default();
        assert_eq!(have, src, "{} is not the exporter's output; re-run with LUMEN_BLESS=1", path.display());
    }
}

/// Accept: an untouched export passes.
#[test]
fn an_untouched_export_passes() {
    exported::untouched();
}

/// An export whose recording clicked a selector that no longer exists
/// panics, naming its step.
#[test]
fn an_export_with_a_missing_selector_fails_naming_its_step() {
    let r = std::panic::catch_unwind(exported::missing_selector);
    let msg = match &r {
        Err(p) => p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default(),
        Ok(()) => String::new(),
    };
    println!("exported missing_selector -> {}", if r.is_ok() { "passed".into() } else { msg.clone() });
    assert!(r.is_err(), "an export with a missing selector passed");
    assert!(msg.contains("step 3"), "the panic does not name the step: {msg}");
}
