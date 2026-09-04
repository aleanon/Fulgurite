//! Plan E4 / parity P4: agent input synthesis. Everything the agent does
//! goes through the host's one queue, so a session replays as a test.

use lumen_static::ops::{Button, Label};
use lumen_static::{pass, Constraints, Event, EventCtx, InputQueue, NodeState, StaticTree, Widget};
use lumen_static_agent::{Agent, AgentHost, TreeRef};
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

impl lumen_static::event::App for App {
    type Tree = Ui;
    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
}

fn on_save(app: &mut App, _: &EventCtx) {
    app.clicks += 1;
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 200.0 };

/// A minimal host: the queue, the layout array, the focus slot, and a
/// frame counter. Exactly the surface the agent's `Host` trait asks for
/// — and the only implementation of it in the workspace, which is why
/// `input.*` has no coverage outside this file (F6.5).
struct Host {
    app: App,
    states: [NodeState; Ui::NODE_COUNT],
    styles: Vec<lumen_static::Style>,
    focus: Option<u32>,
    queue: InputQueue,
    frame: u64,
    /// Every event the host ever saw — the recording seed.
    trace: Vec<Event>,
}

impl Host {
    fn new() -> Self {
        let mut app = App::default();
        app.ui.head.set("head");
        app.ui.save.label = "save";
        app.ui.save.on_click = on_save;
        let styles = pass::default_styles(&app.ui);
        let mut h = Host {
            app,
            states: [NodeState::default(); Ui::NODE_COUNT],
            styles,
            focus: None,
            queue: InputQueue::new(),
            frame: 0,
            trace: Vec::new(),
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
        self.trace.push(ev);
        self.queue.push(ev);
    }

    fn pump(&mut self) -> u64 {
        while let Some(ev) = self.queue.pop() {
            pass::dispatch(&mut self.app, &self.styles, &mut self.states, &mut self.focus, &ev);
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

// ---- M9: the agent types into a real field ---------------------------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Form {
    #[widget_id("name")]
    name: lumen_static::input::TextInput,
    #[widget_id("submit")]
    submit: Button<FormApp>,
}

#[derive(Default)]
struct FormApp {
    ui: Form,
    submits: u32,
}

impl lumen_static::event::App for FormApp {
    type Tree = Form;
    fn tree(&self) -> &Form {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Form {
        &mut self.ui
    }
}

/// The same minimal host, over the form.
struct FormHost {
    app: FormApp,
    styles: Vec<lumen_static::Style>,
    states: [NodeState; Form::NODE_COUNT],
    queue: InputQueue,
    focus: Option<u32>,
}

impl FormHost {
    fn new() -> Self {
        let mut app = FormApp::default();
        app.ui.submit.label = "go";
        app.ui.submit.on_click = |a: &mut FormApp, _: &EventCtx| a.submits += 1;
        let styles = pass::default_styles(&app.ui);
        let mut h = FormHost {
            app,
            styles,
            states: [NodeState::default(); Form::NODE_COUNT],
            queue: InputQueue::new(),
            focus: None,
        };
        h.layout();
        h
    }
    fn layout(&mut self) {
        pass::layout(&mut self.app.ui, &self.styles, &mut self.states, VIEW);
    }
}

impl AgentHost for FormHost {
    fn tree(&self) -> TreeRef<'_> {
        TreeRef::of_root(&self.app.ui, &self.styles, &self.states)
    }
    fn enqueue(&mut self, ev: Event) {
        self.queue.push(ev);
    }
    fn pump(&mut self) -> u64 {
        while let Some(ev) = self.queue.pop() {
            pass::dispatch(
                &mut self.app,
                &self.styles,
                &mut self.states,
                &mut self.focus,
                &ev,
            );
            self.layout();
        }
        1
    }
}

fn fcall(h: &mut FormHost, agent: &Agent, method: &str, params: Value) -> Value {
    agent.dispatch_input(h, &json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }))
}

#[test]
fn an_agent_can_focus_a_field_and_type_into_it() {
    // The whole M9 chain in one test: `input.click` focuses (C1's
    // pointer-down focus move), `input.type` routes to the focused node
    // (C1), the field consumes it through `Node::on_event` (C3), and the
    // application reads it directly (B8). Before M9 every one of those
    // characters was enqueued, drained and dropped.
    let mut h = FormHost::new();
    let agent = Agent::new();

    let r = fcall(&mut h, &agent, "input.click", json!({ "selector": "#name" }));
    assert!(r.get("error").is_none(), "{r}");
    assert_eq!(h.focus, Some(1), "the field took focus");

    fcall(&mut h, &agent, "input.type", json!({ "text": "Ada" }));
    assert_eq!(h.app.ui.name.as_str(), "Ada");

    fcall(&mut h, &agent, "input.key", json!({ "key": "Backspace" }));
    assert_eq!(h.app.ui.name.as_str(), "Ad");

    fcall(&mut h, &agent, "input.key", json!({ "key": "Home" }));
    fcall(&mut h, &agent, "input.type", json!({ "text": "!" }));
    assert_eq!(h.app.ui.name.as_str(), "!Ad", "Home moved the caret to the start");
}

#[test]
fn typing_into_a_field_does_not_activate_the_form() {
    // The two-pass split: the field claims the keystrokes, and the
    // application's *click* handler is untouched. Before the split,
    // routing every event to `Handlers` would have made each character
    // press the submit button.
    let mut h = FormHost::new();
    let agent = Agent::new();
    fcall(&mut h, &agent, "input.click", json!({ "selector": "#name" }));
    fcall(&mut h, &agent, "input.type", json!({ "text": "abc" }));
    assert_eq!(h.app.submits, 0, "typing is not submitting");

    // Enter is not the field's, so it reaches the ancestor chain — and
    // there is no handler on the column, so nothing fires. Clicking the
    // button still does.
    fcall(&mut h, &agent, "input.key", json!({ "key": "Enter" }));
    assert_eq!(h.app.submits, 0);
    fcall(&mut h, &agent, "input.click", json!({ "selector": "#submit" }));
    assert_eq!(h.app.submits, 1);

    // And Enter on the *focused button* activates it (C1).
    fcall(&mut h, &agent, "input.key", json!({ "key": "Enter" }));
    assert_eq!(h.app.submits, 2, "keyboard activation");
}

#[test]
fn tab_moves_focus_between_the_field_and_the_button() {
    let mut h = FormHost::new();
    let agent = Agent::new();
    fcall(&mut h, &agent, "input.key", json!({ "key": "Tab" }));
    let first = h.focus.expect("Tab focused something");
    fcall(&mut h, &agent, "input.key", json!({ "key": "Tab" }));
    let second = h.focus.expect("and moved on");
    assert_ne!(first, second, "Tab visits both focusables");
    fcall(&mut h, &agent, "input.key", json!({ "key": "Tab" }));
    assert_eq!(h.focus, Some(first), "and wraps");
}
