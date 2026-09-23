//! M12/T12.3: a recorded session replays as a test.
//!
//! The invariant has held since E4 — every synthesized input enters the
//! one queue — but nothing wrote the recording out, so "a session
//! replays as a test" was true and unexercised for anything longer than
//! a single click.

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
    // M32 prototype: this app keeps handlers on the whole of itself.
    type State = Self;
    fn state_mut(&mut self) -> &mut Self::State {
        self
    }
    // M28/T28.2: this fixture defers no work. `Never` is
    // uninhabited, so nothing can be sent and `apply` cannot
    // be called.
    type Msg = lumen_static::event::Never;
    fn apply(&mut self, m: Self::Msg) -> bool {
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

/// Drive a host, recording every call the way a shell would.
fn drive(h: &mut Host, agent: &Agent, rec: &mut Recording, method: &str, params: Value) -> Value {
    rec.note(method, &params);
    agent.dispatch_input(
        h,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }),
    )
}

#[test]
fn a_multi_step_session_replays_to_the_same_state() {
    let agent = Agent::new();
    let mut h = Host::new();
    let mut rec = Recording::new(header());

    for _ in 0..3 {
        drive(&mut h, &agent, &mut rec, "input.click", json!({ "selector": "#inc" }));
    }
    drive(&mut h, &agent, &mut rec, "input.click", json!({ "selector": "#dec" }));
    // A query is not a step: it changes nothing and replaying it would
    // assert on the recorder's answers rather than the app's.
    drive(&mut h, &agent, &mut rec, "ui.getLayout", json!({ "selector": "#inc" }));

    assert_eq!(h.app.n, 2);
    assert_eq!(rec.len(), 4, "four inputs recorded, the query excluded");

    let mut fresh = Host::new();
    rec.replay(&agent, &mut fresh, W, H, "block").expect("replays");
    assert_eq!(fresh.app.n, 2, "the replay reproduced the state");
    assert_eq!(fresh.app.ui.count.as_str(), h.app.ui.count.as_str());
}

#[test]
fn a_replay_against_a_different_viewport_is_refused_not_diffed() {
    // The failure this prevents: a selector resolves to the node's
    // laid-out centre, so a different viewport moves it and the same
    // script clicks somewhere else. A *passing* run there would be worse
    // than a failing one.
    let agent = Agent::new();
    let mut h = Host::new();
    let mut rec = Recording::new(header());
    drive(&mut h, &agent, &mut rec, "input.click", json!({ "selector": "#inc" }));

    let mut fresh = Host::new();
    let err = rec.replay(&agent, &mut fresh, W + 40, H, "block").unwrap_err();
    assert!(matches!(err, ReplayError::Mismatch { field: "viewport", .. }), "{err:?}");
    assert!(err.to_string().contains("land somewhere else"), "the error teaches: {err}");
    assert_eq!(fresh.app.n, 0, "and nothing ran");
}

#[test]
fn a_replay_with_a_different_measurer_is_refused() {
    // Label width decides node size decides node centre. This repo has
    // already been burned by exactly this mismatch, in the audit.
    let agent = Agent::new();
    let mut rec = Recording::new(header());
    rec.note("input.click", &json!({ "selector": "#inc" }));
    let mut fresh = Host::new();
    let err = rec.replay(&agent, &mut fresh, W, H, "NotoSans").unwrap_err();
    assert!(matches!(err, ReplayError::Mismatch { field: "measurer", .. }), "{err:?}");
}

#[test]
fn a_recording_that_did_not_start_at_the_beginning_is_refused() {
    // `live_gate.sh` warms a window with twenty clicks before any check
    // runs, so a recording taken from a live session does not begin at
    // Default and a fresh host cannot reproduce where it started.
    let agent = Agent::new();
    let mut rec = Recording::new(Header { start_frame: 21, ..header() });
    rec.note("input.click", &json!({ "selector": "#inc" }));
    let mut fresh = Host::new();
    let err = rec.replay(&agent, &mut fresh, W, H, "block").unwrap_err();
    assert!(matches!(err, ReplayError::NotFromStart(21)), "{err:?}");
    assert!(err.to_string().contains("record from construction"), "{err}");
}

#[test]
fn the_export_is_source_a_compiler_would_check() {
    // Source rather than a data file, so a selector that no longer
    // exists fails in the repository instead of sitting in a log nobody
    // runs. That is the difference between a script and a record.
    let agent = Agent::new();
    let mut h = Host::new();
    let mut rec = Recording::new(header());
    drive(&mut h, &agent, &mut rec, "input.click", json!({ "selector": "#inc" }));
    drive(&mut h, &agent, &mut rec, "input.key", json!({ "key": "Tab" }));

    let src = rec.export_test("replays_the_counter", "Host::new()");
    assert!(src.contains("#[test]"), "{src}");
    assert!(src.contains("fn replays_the_counter()"), "{src}");
    assert!(src.contains("\"input.click\""), "{src}");
    assert!(src.contains("\"selector\":\"#inc\""), "the params survive: {src}");
    assert!(src.contains("200x200"), "the header is in the source: {src}");
    assert!(src.contains("Re-record rather than edit"), "{src}");
    assert_eq!(src.matches("dispatch_input").count(), 2, "one call per step");
}
