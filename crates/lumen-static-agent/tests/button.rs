//! **The agent can press any button** — M90/T90.1 (G25,
//! `docs/plan-desktop-integration.md`). `input.click`,
//! `input.pointerDown` and `input.pointerUp` take `button: "left" |
//! "right" | "middle"` (default `"left"`) and echo it. Until this task
//! every one of them queued `PointerButton::Left` whatever it was asked,
//! so no drive could right-click at all.
//!
//! The probe reads the button **from `ctx.event`** — the event the widget
//! was handed — and prints `ctx.button` beside it, so the gate sees what
//! arrived, not what the agent meant to send.

use lumen_static::desc::{NodeDesc, NO_PARENT};
use lumen_static::display::Painter;
use lumen_static::event::{EventCtx, Handlers, NodeSel, PointerButton};
use lumen_static::observe::accesskit;
use lumen_static::style::Seed;
use lumen_static::widget::{Node, Widget};
use lumen_static::{pass, Constraints, Event, InputQueue, LayoutCx, NodeState, Size, StaticTree, Style};
use lumen_static_agent::{Agent, AgentHost, TreeRef};
use serde_json::{json, Value};

/// Records the button of every `PointerDown` and `PointerUp` it is
/// handed, read from `ctx.event`, with `ctx.button` beside it.
#[derive(Default)]
struct Probe {
    seen: Vec<(&'static str, PointerButton, PointerButton)>,
}

impl Widget for Probe {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];
    const KIND: &'static str = "probe";
    fn type_defaults() -> Style {
        Style::default()
    }
    const SEEDS: &'static [Seed] = &[Seed::new(
        Self::KIND,
        Self::type_defaults,
        lumen_static::style::Fields::of(&[lumen_static::style::Fields::VISIBLE]),
    )];
}

impl Node for Probe {
    fn layout(&mut self, _cx: &mut LayoutCx, limits: Constraints, style: &[Style], st: &mut [NodeState], i: usize) -> Size {
        lumen_static::ops::record_painted(st, i, limits, Size { w: 80.0, h: 40.0 }, &style[i])
    }
    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        out.rect(lumen_static::ops::rect_of(&st[i]), st[i].bg);
    }
    fn on_event(&mut self, _ev: &Event, cx: &EventCtx) -> bool {
        let (kind, button) = match cx.event {
            Event::PointerDown { button, .. } => ("down", button),
            Event::PointerUp { button, .. } => ("up", button),
            _ => return false,
        };
        self.seen.push((kind, button, cx.button));
        true
    }
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::GenericContainer);
    }
    lumen_static::leaf_common!(Probe, 0);
}

impl<S> Handlers<S> for Probe {
    fn handler(&self, _sel: NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        None
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("probe")]
    probe: Probe,
}

#[derive(Default)]
struct App {
    ui: Ui,
}

impl lumen_static::event::App for App {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self::State {
        self
    }
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

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 200.0 };

/// The smallest `AgentHost`: the queue, the layout, `pass::dispatch`
/// (`tests/input.rs`'s shape).
struct Host {
    app: App,
    states: [NodeState; Ui::NODE_COUNT],
    styles: Vec<Style>,
    input: lumen_static::event::InputState,
    queue: InputQueue,
    frame: u64,
}

impl Host {
    fn new() -> Self {
        let app = App::default();
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

fn call(h: &mut Host, method: &str, params: Value) -> Value {
    let req = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    Agent::new().dispatch_input(h, &req)
}

fn button(name: Option<&str>) -> PointerButton {
    match name {
        None | Some("left") => PointerButton::Left,
        Some("right") => PointerButton::Right,
        Some("middle") => PointerButton::Middle,
        Some(o) => unreachable!("{o}"),
    }
}

/// **Each of the three methods delivers the button it was asked for**,
/// and an omitted one is `Left` (the accept direction: every caller that
/// predates T90.1 passes none). The reply echoes the button.
#[test]
fn each_method_delivers_the_button_it_was_asked_for() {
    let mut wrong = Vec::new();
    for asked in [Some("right"), Some("middle"), Some("left"), None] {
        let want = button(asked);
        let echo = asked.unwrap_or("left");
        for method in ["input.click", "input.pointerDown", "input.pointerUp"] {
            let mut h = Host::new();
            let mut params = json!({ "selector": "#probe" });
            if let Some(b) = asked {
                params["button"] = json!(b);
            }
            let r = call(&mut h, method, params);
            assert!(r.get("error").is_none(), "{method} {asked:?}: {r}");
            let seen = &h.app.ui.probe.seen;
            let kinds: &[&str] = match method {
                "input.click" => &["down", "up"],
                "input.pointerDown" => &["down"],
                _ => &["up"],
            };
            // Setup landed: the probe was handed exactly the events the
            // method queues, so the button below is one it read.
            assert_eq!(seen.iter().map(|s| s.0).collect::<Vec<_>>(), kinds, "setup: {method} reached the probe as {seen:?}");
            for &(kind, got, ctx) in seen {
                println!(
                    "{method:<17} button {:<8} -> {kind:<4} ctx.event button {got:?}, ctx.button {ctx:?} (want {want:?}); echo {}",
                    format!("{asked:?}"),
                    r["result"]["button"]
                );
                if got != want {
                    wrong.push(format!("{method} asked {asked:?}: the probe read {got:?} on its {kind}"));
                }
            }
            assert_eq!(r["result"]["button"], echo, "{method}: the reply does not echo the button");
        }
    }
    assert!(wrong.is_empty(), "the button asked for did not arrive:\n{}", wrong.join("\n"));
}

/// **A button the framework has no name for is refused**, naming the
/// value, by all three methods — and nothing reaches the probe.
#[test]
fn an_unknown_button_is_refused_by_name() {
    for method in ["input.click", "input.pointerDown", "input.pointerUp"] {
        let mut h = Host::new();
        let r = call(&mut h, method, json!({ "selector": "#probe", "button": "side" }));
        println!("{method} button \"side\": {}", r["error"]);
        assert_eq!(r["error"]["code"], -32602, "{method}: {r}");
        assert!(r["error"]["message"].as_str().unwrap_or("").contains("side"), "{method}: the refusal does not name the value: {r}");
        assert!(h.app.ui.probe.seen.is_empty(), "{method}: a refused button still queued {:?}", h.app.ui.probe.seen);
        let r = call(&mut h, method, json!({ "selector": "#probe", "button": 2 }));
        assert_eq!(r["error"]["code"], -32602, "{method}: a number for a button: {r}");
    }
}
