//! Plan E4 / parity P4: agent input synthesis. Everything the agent does
//! goes through the host's one queue, so a session replays as a test.

use lumen_static_widgets::ops::{Button, Label};
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
    input: lumen_static::event::InputState,
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
            input: Default::default(),
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
            Event::down(h.states[2].pos.x + h.states[2].size.w / 2.0, h.states[2].pos.y + h.states[2].size.h / 2.0),
            Event::up(h.states[2].pos.x + h.states[2].size.w / 2.0, h.states[2].pos.y + h.states[2].size.h / 2.0),
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
    name: lumen_static_widgets::input::TextInput,
    #[widget_id("submit")]
    submit: Button<FormApp>,
}

#[derive(Default)]
struct FormApp {
    ui: Form,
    submits: u32,
}

impl lumen_static::event::App for FormApp {
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
    input: lumen_static::event::InputState,
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
            input: Default::default(),
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
    fn dropped_inputs(&self) -> Option<u32> {
        Some(self.queue.dropped())
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
            self.layout();
        }
        (1, handled)
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
    assert_eq!(h.input.focus, Some(1), "the field took focus");

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
    let first = h.input.focus.expect("Tab focused something");
    fcall(&mut h, &agent, "input.key", json!({ "key": "Tab" }));
    let second = h.input.focus.expect("and moved on");
    assert_ne!(first, second, "Tab visits both focusables");
    fcall(&mut h, &agent, "input.key", json!({ "key": "Tab" }));
    assert_eq!(h.input.focus, Some(first), "and wraps");
}


// ---- input.fill and input.scrollIntoView ----------------------------

#[test]
fn fill_replaces_a_field_in_one_call() {
    // What `app.setValue` would have done, through the queue instead:
    // real events, so the session still replays as a test. Measured
    // before it existed: 23 RPCs to replace a 20-character value, and
    // the agent had to read the old value first to know how many
    // backspaces to send.
    let mut h = FormHost::new();
    let agent = Agent::new();
    fcall(&mut h, &agent, "input.click", json!({ "selector": "#name" }));
    fcall(&mut h, &agent, "input.type", json!({ "text": "ada@lovelace.example" }));
    assert_eq!(h.app.ui.name.as_str(), "ada@lovelace.example");

    let r = fcall(&mut h, &agent, "input.fill",
        json!({ "selector": "#name", "text": "grace@hopper.example" }));
    assert!(r.get("error").is_none(), "{r}");
    assert_eq!(h.app.ui.name.as_str(), "grace@hopper.example");
    assert_eq!(r["result"]["cleared"], 20, "it reported what it removed");
    assert_eq!(r["result"]["chars"], 20);
}

#[test]
fn fill_into_an_empty_field_types_without_clearing() {
    let mut h = FormHost::new();
    let agent = Agent::new();
    let r = fcall(&mut h, &agent, "input.fill", json!({ "selector": "#name", "text": "hi" }));
    assert_eq!(r["result"]["cleared"], 0);
    assert_eq!(h.app.ui.name.as_str(), "hi");
}

#[test]
fn fill_clears_a_field_when_given_nothing() {
    let mut h = FormHost::new();
    let agent = Agent::new();
    fcall(&mut h, &agent, "input.fill", json!({ "selector": "#name", "text": "abc" }));
    fcall(&mut h, &agent, "input.fill", json!({ "selector": "#name", "text": "" }));
    assert_eq!(h.app.ui.name.as_str(), "", "an empty fill is a clear");
}

#[test]
fn fill_handles_multi_byte_characters_one_backspace_each() {
    // `cleared` counts *characters*, and `TextInput::backspace` removes
    // one whole character — byte arithmetic here would leave half of a
    // multi-byte char behind and panic the next slice.
    let mut h = FormHost::new();
    let agent = Agent::new();
    fcall(&mut h, &agent, "input.fill", json!({ "selector": "#name", "text": "aé漢🎉" }));
    assert_eq!(h.app.ui.name.as_str(), "aé漢🎉");
    let r = fcall(&mut h, &agent, "input.fill", json!({ "selector": "#name", "text": "z" }));
    assert_eq!(r["result"]["cleared"], 4, "four characters, not ten bytes");
    assert_eq!(h.app.ui.name.as_str(), "z");
}

#[test]
fn fill_refuses_rather_than_typing_into_the_wrong_control() {
    // The hazard this guard exists for: a fill whose click misses sends
    // a run of backspaces to whatever *does* hold focus. Destructive and
    // silent, so it is refused with an error that names the way out.
    let mut h = FormHost::new();
    let agent = Agent::new();
    fcall(&mut h, &agent, "input.fill", json!({ "selector": "#name", "text": "keep me" }));

    // Move the field out of reach the way a scroll would.
    h.states[1].size = lumen_static::Size::default();
    let r = fcall(&mut h, &agent, "input.fill", json!({ "selector": "#name", "text": "lost" }));
    let msg = r["error"]["message"].as_str().unwrap_or("");
    assert!(msg.contains("scrollIntoView"), "the error names the way out: {msg}");
    assert_eq!(h.app.ui.name.as_str(), "keep me", "and nothing was typed anywhere");
}

/// A form inside a scroll region, taller than its viewport.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct ScrolledBody {
    #[widget_id("top")]
    top: Label,
    #[widget_id("field")]
    field: lumen_static_widgets::input::TextInput,
    #[widget_id("r1")]
    r1: Label,
    #[widget_id("r2")]
    r2: Label,
    #[widget_id("r3")]
    r3: Label,
    #[widget_id("bottom")]
    bottom: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct ScrolledUi {
    #[widget_id("sc")]
    sc: lumen_static_widgets::scroll::ScrollView<ScrolledBody>,
}

#[derive(Default)]
struct ScrolledApp {
    ui: ScrolledUi,
}

impl lumen_static::event::App for ScrolledApp {
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
    type Tree = ScrolledUi;
    fn tree(&self) -> &ScrolledUi {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut ScrolledUi {
        &mut self.ui
    }
}

struct ScrolledHost {
    app: ScrolledApp,
    styles: Vec<lumen_static::Style>,
    states: [NodeState; ScrolledUi::NODE_COUNT],
    queue: InputQueue,
    input: lumen_static::event::InputState,
}

const TIGHT: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 60.0 };

impl ScrolledHost {
    fn new() -> Self {
        let mut app = ScrolledApp::default();
        {
            let b = &mut app.ui.sc.content;
            b.top.set("top");
            b.r1.set("row one");
            b.r2.set("row two");
            b.r3.set("row three");
            b.bottom.set("bottom");
        }
        let styles = pass::default_styles(&app.ui);
        let mut h = ScrolledHost {
            app,
            styles,
            states: [NodeState::default(); ScrolledUi::NODE_COUNT],
            queue: InputQueue::new(),
            input: Default::default(),
        };
        h.layout();
        h
    }
    fn layout(&mut self) {
        pass::layout(&mut self.app.ui, &self.styles, &mut self.states, TIGHT);
    }
}

impl AgentHost for ScrolledHost {
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
            self.layout();
        }
        (1, handled)
    }
}

fn scall(h: &mut ScrolledHost, agent: &Agent, method: &str, params: Value) -> Value {
    agent
        .dispatch_input(h, &json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }))
}

#[test]
fn scroll_into_view_brings_a_control_back_within_reach() {
    // `input.click` resolves a selector to the node's *centre*, and a
    // centre outside the viewport is a point the hit test correctly
    // rejects — so a control that has scrolled away cannot be reached at
    // all. This is the way back.
    let mut h = ScrolledHost::new();
    let agent = Agent::new();

    // Scroll to the bottom with real wheel events.
    for _ in 0..10 {
        scall(&mut h, &agent, "input.scroll", json!({ "selector": "#sc", "dy": -30.0 }));
    }
    assert!(h.app.ui.sc.scroll > 0.0, "it scrolled");

    let before = scall(&mut h, &agent, "ui.explain", json!({ "selector": "#field" }));
    assert_eq!(before["result"]["reachable"], false, "the field is out of view: {before}");

    let r = scall(&mut h, &agent, "input.scrollIntoView", json!({ "selector": "#field" }));
    assert!(r.get("error").is_none(), "{r}");
    assert_eq!(r["result"]["reachable"], true, "and now it is: {r}");
    assert!(r["result"]["steps"].as_u64().unwrap() >= 1);

    // The point of getting it back: it can be filled.
    let f = scall(&mut h, &agent, "input.fill", json!({ "selector": "#field", "text": "typed" }));
    assert!(f.get("error").is_none(), "{f}");
    assert_eq!(h.app.ui.sc.content.field.as_str(), "typed");
}

#[test]
fn scroll_into_view_on_something_already_visible_does_nothing() {
    let mut h = ScrolledHost::new();
    let agent = Agent::new();
    let r = scall(&mut h, &agent, "input.scrollIntoView", json!({ "selector": "#top" }));
    assert_eq!(r["result"]["steps"], 0, "no gesture was needed");
    assert_eq!(r["result"]["reachable"], true);
    assert_eq!(h.app.ui.sc.scroll, 0.0, "and nothing moved");
}

#[test]
fn scroll_into_view_terminates_when_a_target_cannot_be_shown() {
    // A scroller clamped at its end cannot move further. The loop is
    // bounded so a target it can never reveal ends the call rather than
    // spinning — and reports `reachable: false` honestly.
    let mut h = ScrolledHost::new();
    let agent = Agent::new();
    h.states[2].size = lumen_static::Size::default(); // collapse the content
    h.states[3].size = lumen_static::Size::default(); // and the target
    let r = scall(&mut h, &agent, "input.scrollIntoView", json!({ "selector": "#field" }));
    assert!(r.get("error").is_none(), "{r}");
    assert_eq!(r["result"]["reachable"], false, "reported, not spun on");
}

#[test]
fn fill_reports_a_control_that_did_not_accept_the_text() {
    // Found live: `input.fill` on a `Label` answered `cleared: 1,
    // chars: 3` and changed nothing. The check is the *outcome* — read
    // the value back — rather than "does this look editable": a Label is
    // not, a Button is focusable and is not either, and a field may
    // filter what it accepts. Only the outcome knows.
    let mut h = FormHost::new();
    let agent = Agent::new();
    let r = fcall(&mut h, &agent, "input.fill", json!({ "selector": "#submit", "text": "xyz" }));
    let msg = r["error"]["message"].as_str().unwrap_or("");
    assert!(msg.contains("did not accept"), "a button is not fillable: {r}");
    assert_eq!(r["error"]["data"]["wanted"], "xyz");
}

// ---- R3-F2: typing more than the queue holds (M44/T44.5) ------------

/// **`input.type` delivers every character and says how many arrived.**
///
/// The agent posts one event per character into the host's queue, which
/// holds `InputQueue::CAP` (64) and counts the rest as dropped, and then
/// pumps once — so a 100-character string left 64 in the field while the
/// reply said 100 (R3-F2, `docs/results-deep-review.md`).
#[test]
fn typing_more_than_the_queue_holds_delivers_every_character() {
    let mut h = FormHost::new();
    let agent = Agent::new();
    fcall(&mut h, &agent, "input.click", json!({ "selector": "#name" }));
    assert_eq!(h.input.focus, Some(1), "setup: the click did not focus the field");
    let text: String = (0..100).map(|k| (b'a' + (k % 26) as u8) as char).collect();
    let r = fcall(&mut h, &agent, "input.type", json!({ "text": text }));
    assert!(r.get("error").is_none(), "{r}");
    let landed = h.app.ui.name.as_str().chars().count();
    println!(
        "input.type of 100: field holds {landed}, reply says {}, queue CAP {}",
        r["result"]["chars"],
        InputQueue::<64>::CAP
    );
    assert_eq!(landed, 100, "characters were lost in the queue");
    assert_eq!(r["result"]["chars"], 100, "the reply does not say what arrived");
    assert_eq!(h.app.ui.name.as_str(), text, "the characters arrived out of order or altered");
}

/// **`input.fill` over a value as long as the new one.** Forty
/// backspaces and forty characters is eighty events in one pump.
#[test]
fn fill_of_forty_over_forty_replaces_the_value() {
    let mut h = FormHost::new();
    let agent = Agent::new();
    let old: String = "x".repeat(40);
    let new: String = (0..40).map(|k| (b'a' + (k % 26) as u8) as char).collect();
    fcall(&mut h, &agent, "input.fill", json!({ "selector": "#name", "text": old }));
    assert_eq!(h.app.ui.name.as_str(), old, "setup: the first fill (40 events) did not land");
    let r = fcall(&mut h, &agent, "input.fill", json!({ "selector": "#name", "text": new }));
    println!(
        "input.fill 40 over 40: field {:?} ({} chars), reply {}",
        h.app.ui.name.as_str(),
        h.app.ui.name.as_str().chars().count(),
        r
    );
    assert!(r.get("error").is_none(), "{r}");
    assert_eq!(h.app.ui.name.as_str(), new, "the fill did not replace the value");
}

/// **The reply says what arrived, not what was sent** (M44/T44.5). A
/// queue already nearly full when `input.type` starts drops part of it;
/// the reply must count only what reached the field, and name the loss.
#[test]
fn a_reply_counts_what_arrived_when_the_queue_drops_some() {
    let mut h = FormHost::new();
    let agent = Agent::new();
    fcall(&mut h, &agent, "input.click", json!({ "selector": "#name" }));
    assert_eq!(h.input.focus, Some(1), "setup: the click did not focus the field");
    // Sixty harmless events already queued: four slots left.
    for _ in 0..60 {
        h.queue.push(Event::Modifiers(lumen_static::Mods::NONE));
    }
    let r = fcall(&mut h, &agent, "input.type", json!({ "text": "abcdefghij" }));
    let landed = h.app.ui.name.as_str().chars().count();
    println!(
        "input.type of 10 into a queue with 4 free: field holds {landed}, reply chars {} sent {} dropped {}",
        r["result"]["chars"], r["result"]["sent"], r["result"]["dropped"]
    );
    assert_eq!(landed, 4, "setup: the queue did not drop the tail");
    assert_eq!(r["result"]["chars"], 4, "the reply claims characters that never arrived");
    assert_eq!(r["result"]["dropped"], 6, "the reply does not name the loss");
}
