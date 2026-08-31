//! Plan E4 / parity P4: agent input synthesis. Everything the agent does
//! goes through the host's one queue, so a session replays as a test.

use lumen_static::ops::{Button, Label};
use lumen_static::{pass, Constraints, Event, EventCtx, InputQueue, NodeState, StaticTree, Widget};
use lumen_static_agent::{Agent, Host as AgentHost, TreeRef};
use serde_json::{json, Value};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("head")]
    head: Label,
    #[widget_id("save")]
    save: Button<App>,
}

#[derive(Default)]
struct App {
    ui: Ui,
    clicks: u32,
}

fn on_save(app: &mut App, _: &EventCtx) {
    app.clicks += 1;
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 200.0 };

/// A minimal host: the queue, the layout array, the focus slot, and a
/// frame counter. Exactly the surface `Pump` asks for.
struct Host {
    app: App,
    states: [NodeState; Ui::NODE_COUNT],
    focus: Option<u32>,
    queue: InputQueue,
    frame: u64,
    /// Every event the host ever saw — the recording seed.
    trace: Vec<Event>,
}

impl Host {
    fn new() -> Self {
        let mut app = App::default();
        app.ui.head.text = "head";
        app.ui.save.label = "save";
        app.ui.save.on_click = on_save;
        let mut h = Host {
            app,
            states: [NodeState::default(); Ui::NODE_COUNT],
            focus: None,
            queue: InputQueue::new(),
            frame: 0,
            trace: Vec::new(),
        };
        h.layout();
        h
    }

    fn layout(&mut self) {
        let base = &self.app.ui as *const Ui as *const u8;
        pass::layout(base, Ui::SUB_TABLE, Ui::KIND_LIST, &mut self.states, VIEW);
    }

}

impl AgentHost for Host {
    fn tree(&self) -> TreeRef<'_> {
        TreeRef {
            base: &self.app.ui as *const Ui as *const u8,
            table: Ui::SUB_TABLE,
            kinds: Ui::KIND_LIST,
            states: &self.states,
        }
    }

    fn enqueue(&mut self, ev: Event) {
        self.trace.push(ev);
        self.queue.push(ev);
    }

    fn pump(&mut self) -> u64 {
        while let Some(ev) = self.queue.pop() {
            let sp = &mut self.app as *mut App as *mut u8;
            let base = unsafe { &raw const (*(sp as *mut App)).ui } as *const u8;
            pass::dispatch(
                sp,
                base,
                Ui::SUB_TABLE,
                Ui::KIND_LIST,
                &self.states,
                &mut self.focus,
                &ev,
            );
        }
        self.layout();
        self.frame += 1;
        self.frame
    }
}

fn call(h: &mut Host, agent: &Agent, method: &str, params: Value) -> Value {
    let req = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    agent.dispatch_input(h, &req)
}

#[test]
fn click_by_author_id_reaches_the_handler() {
    let mut h = Host::new();
    let agent = Agent::new();
    let r = call(&mut h, &agent, "input.click", json!({ "selector": "#save" }));
    assert!(r.get("error").is_none(), "{r}");
    assert_eq!(h.app.clicks, 1, "the handler ran");
    assert_eq!(r["result"]["frame"], 1, "one frame was pumped");
}

#[test]
fn click_by_coordinates_also_works() {
    let mut h = Host::new();
    let agent = Agent::new();
    let st = h.states[2];
    let (x, y) = (st.pos.x + 1.0, st.pos.y + 1.0);
    call(&mut h, &agent, "input.click", json!({ "x": x, "y": y }));
    assert_eq!(h.app.clicks, 1);
}

#[test]
fn invoke_action_is_a_click_at_the_node_centre() {
    let mut h = Host::new();
    let agent = Agent::new();
    call(&mut h, &agent, "input.invokeAction", json!({ "selector": "#save" }));
    assert_eq!(h.app.clicks, 1);
}

#[test]
fn synthesized_input_goes_through_the_queue_so_it_replays() {
    let mut h = Host::new();
    let agent = Agent::new();
    call(&mut h, &agent, "input.click", json!({ "selector": "#save" }));
    assert_eq!(
        h.trace,
        vec![
            Event::PointerDown { x: h.states[2].pos.x + h.states[2].size.w / 2.0, y: h.states[2].pos.y + h.states[2].size.h / 2.0 },
            Event::PointerUp { x: h.states[2].pos.x + h.states[2].size.w / 2.0, y: h.states[2].pos.y + h.states[2].size.h / 2.0 },
        ],
        "the agent enqueued ordinary pointer events, nothing privileged"
    );

    // Replaying that trace on a fresh host reproduces the outcome.
    let mut replay = Host::new();
    for ev in h.trace.clone() {
        replay.enqueue(ev);
    }
    replay.pump();
    assert_eq!(replay.app.clicks, h.app.clicks);
}

#[test]
fn typing_and_keys_enqueue_per_character() {
    let mut h = Host::new();
    let agent = Agent::new();
    let r = call(&mut h, &agent, "input.type", json!({ "text": "hi!" }));
    assert_eq!(r["result"]["chars"], 3);
    assert_eq!(h.trace.len(), 3);

    let r = call(&mut h, &agent, "input.key", json!({ "key": "Enter" }));
    assert!(r.get("error").is_none(), "{r}");
    assert!(matches!(h.trace.last(), Some(Event::Key { .. })));
}

#[test]
fn bad_input_params_teach() {
    let mut h = Host::new();
    let agent = Agent::new();
    let r = call(&mut h, &agent, "input.key", json!({ "key": "Meta" }));
    assert_eq!(r["error"]["code"], -32602);
    assert!(r["error"]["data"]["known"].to_string().contains("Enter"));

    let r = call(&mut h, &agent, "input.click", json!({ "selector": "#nope" }));
    assert_eq!(r["error"]["code"], -32001);
    assert!(r["error"]["data"]["available_ids"].to_string().contains("save"));
}

#[test]
fn read_methods_still_work_through_the_input_entry_point() {
    let mut h = Host::new();
    let agent = Agent::new();
    let r = call(&mut h, &agent, "app.diagnostics", json!({}));
    assert_eq!(r["result"]["nodes"], Ui::NODE_COUNT);
}
