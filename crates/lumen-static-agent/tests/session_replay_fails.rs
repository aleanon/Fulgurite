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
use lumen_static_agent::session::{Header, Recording};
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
    rec.note(method, &params);
    agent.dispatch_input(h, &json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }))
}

#[test]
#[ignore = "red until T48.10: R13-F6, replay discards every reply"]
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
    let r = rec.replay(&agent, &mut fresh, W, H, "block");
    println!("replay -> {r:?}, fresh n={} / recorded {}", fresh.app.n, h.app.n);
    assert!(r.is_err(), "a corrupted recording replayed Ok, fresh n={} / recorded 3", fresh.app.n);
}

#[test]
#[ignore = "red until T48.10: R13-F6, check takes no stylesheet"]
fn a_stylesheet_mismatch_is_refused() {
    let agent = Agent::new();
    let mut rec = Recording::new(Header { stylesheet: "dark-compact".into(), ..header() });
    rec.note("input.click", &json!({ "selector": "#inc" }));
    let mut fresh = Host::new();
    let r = rec.replay(&agent, &mut fresh, W, H, "block");
    println!("replay -> {r:?} (recorded `dark-compact`, host `default`), fresh n={}", fresh.app.n);
    assert!(r.is_err(), "a stylesheet mismatch replayed Ok");
    assert_eq!(fresh.app.n, 0, "and a refused replay runs nothing");
}

#[test]
#[ignore = "red until T48.10: R13-F6, an export asserts nothing"]
fn an_export_asserts_every_step() {
    let agent = Agent::new();
    let mut h = Host::new();
    let mut rec = Recording::new(header());
    drive(&mut h, &agent, &mut rec, "input.click", json!({ "selector": "#inc" }));
    drive(&mut h, &agent, &mut rec, "input.click", json!({ "selector": "#does-not-exist" }));
    let src = rec.export_test("replays_the_counter", "Host::new()");
    let lets = src.matches("let _ =").count();
    let placeholder = src.contains("Assertions on final state go here");
    println!("export: {lets} discarded replies, placeholder {placeholder}\n{src}");
    assert_eq!(lets, 0, "the export discards {lets} replies");
    assert!(!placeholder, "the export ends with a placeholder where its assertion belongs");
}
