//! **`input.invokeAction` does what it names** — M54/T54.4
//! (`docs/plan-a11y-platform.md` §2.4, R15-F2).
//!
//! It was `input.click` under another name, so `increment` on a slider
//! was a click at its centre — which, on a slider, sets the value to the
//! middle of the track. Now it queues `Event::Action` for the node the
//! selector names, the event a screen reader's request becomes, and the
//! slider steps by exactly one `step`.
//!
//! The same through a `Glass`: an action is delivered by table index, so
//! it reaches a slider inside a wrapper without the wrapper being asked —
//! the reason §2.4 gives for the hook being an event and not a method a
//! wrapper would have to forward.

use lumen_static::{pass, Constraints, Event, InputQueue, NodeState, StaticTree, Widget};
use lumen_static_agent::{Agent, AgentHost, TreeRef};
use lumen_static_widgets::{Glass, Slider};
use serde_json::{json, Value};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct Ui {
    #[widget_id("bare")]
    bare: Slider,
    #[widget_id("glass")]
    glass: Glass<Slider>,
}

#[derive(lumen_static::App, Default)]
struct App {
    #[tree]
    ui: Ui,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 240.0, min_h: 0.0, max_h: 200.0 };

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
        app.ui.bare = Slider::new(0.25);
        app.ui.glass.content = Slider::new(0.25);
        let styles = pass::default_styles(&app.ui);
        let mut h = Host {
            app,
            states: [NodeState::default(); Ui::NODE_COUNT],
            styles,
            input: Default::default(),
            queue: InputQueue::new(),
            frame: 0,
        };
        pass::layout(&mut h.app.ui, &h.styles, &mut h.states, VIEW);
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
            handled |= pass::dispatch(&mut self.app, &self.styles, &mut self.states, &mut self.input, &ev);
        }
        pass::layout(&mut self.app.ui, &self.styles, &mut self.states, VIEW);
        self.frame += 1;
        (self.frame, handled)
    }
}

fn invoke(h: &mut Host, selector: &str, action: &str) -> Value {
    let req = json!({ "jsonrpc": "2.0", "id": 1, "method": "input.invokeAction",
                      "params": { "selector": selector, "action": action } });
    Agent::new().dispatch_input(h, &req)
}

#[test]
fn increment_steps_a_slider_by_exactly_one_step() {
    let mut h = Host::new();
    let (before, step) = (h.app.ui.bare.value(), h.app.ui.bare.step);
    let r = invoke(&mut h, "#bare", "increment");
    let after = h.app.ui.bare.value();
    println!("bare slider: {before} -> {after} (step {step}), reply {r}");
    assert_eq!(r["result"]["handled"], true, "{r}");
    assert!((after - before - step).abs() < 1e-6, "moved {} , not one step of {step}", after - before);
}

#[test]
fn an_action_reaches_a_slider_inside_a_glass() {
    let mut h = Host::new();
    let (before, step) = (h.app.ui.glass.content.value(), h.app.ui.glass.content.step);
    // The slider is the glass's content: the node after the glass's own.
    let r = invoke(&mut h, "#glass > slider", "increment");
    let after = h.app.ui.glass.content.value();
    println!("slider in a Glass: {before} -> {after} (step {step}), reply {r}");
    assert_eq!(r["result"]["handled"], true, "{r}");
    assert!((after - before - step).abs() < 1e-6, "moved {}, not one step of {step}", after - before);
}

#[test]
fn set_value_and_decrement_and_an_unknown_action() {
    let mut h = Host::new();
    let r = invoke(&mut h, "#bare", "setValue");
    assert!(r.get("error").is_some(), "setValue without a value was accepted: {r}");
    let req = json!({ "jsonrpc": "2.0", "id": 1, "method": "input.invokeAction",
                      "params": { "selector": "#bare", "action": "setValue", "value": 0.8 } });
    let r = Agent::new().dispatch_input(&mut h, &req);
    println!("setValue 0.8 -> {} ({r})", h.app.ui.bare.value());
    assert!((h.app.ui.bare.value() - 0.8).abs() < 1e-6);
    invoke(&mut h, "#bare", "decrement");
    println!("decrement -> {}", h.app.ui.bare.value());
    assert!((h.app.ui.bare.value() - (0.8 - h.app.ui.bare.step)).abs() < 1e-6);
    let r = invoke(&mut h, "#bare", "juggle");
    assert!(r.get("error").is_some(), "an unknown action was accepted: {r}");
}
