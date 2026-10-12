//! **The agent can press any button** — M90/T90.1 (G25,
//! `docs/plan-desktop-integration.md`). `input.click`,
//! `input.pointerDown` and `input.pointerUp` take `button: "left" |
//! "right" | "middle"` (default `"left"`) and echo it. Until T90.1
//! every one of them queued `PointerButton::Left` whatever it was asked,
//! so no drive could right-click at all.
//!
//! **Rewritten by T90.2** (second review G22): after dispatch's
//! secondary-button filter no widget receives a `Right` or `Middle`
//! pointer event, so a probe that recorded the button it was handed
//! would record nothing. What arrives now is the observable: a `Right`
//! press is one `Event::ContextRequest` at the pressed point, which the
//! probe consumes; its release, and every `Middle` event, reach nothing
//! and answer `handled: false`, with the reply still echoing the button;
//! an omitted button is still a `Left` press and release, read from
//! `ctx.event` with `ctx.button` printed beside it.

use fulgurite::desc::{NodeDesc, NO_PARENT};
use fulgurite::display::Painter;
use fulgurite::event::{EventCtx, Handlers, NodeSel, PointerButton};
use fulgurite::observe::accesskit;
use fulgurite::style::Seed;
use fulgurite::widget::{Node, Widget};
use fulgurite::{pass, Constraints, Event, InputQueue, LayoutCx, NodeState, Size, StaticTree, Style};
use fulgurite_agent::{Agent, AgentHost, TreeRef};
use serde_json::{json, Value};

/// Records the button of every `PointerDown` and `PointerUp` it is
/// handed, read from `ctx.event`, with `ctx.button` beside it — and every
/// `ContextRequest`, by its point, which it consumes.
#[derive(Default)]
struct Probe {
    seen: Vec<(&'static str, PointerButton, PointerButton)>,
    requests: Vec<(f32, f32)>,
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
        fulgurite::style::Fields::of(&[fulgurite::style::Fields::VISIBLE]),
    )];
}

impl Node for Probe {
    fn layout(&mut self, _cx: &mut LayoutCx, limits: Constraints, style: &[Style], st: &mut [NodeState], i: usize) -> Size {
        fulgurite::ops::record_painted(st, i, limits, Size { w: 80.0, h: 40.0 }, &style[i])
    }
    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        out.rect(fulgurite::ops::rect_of(&st[i]), st[i].bg);
    }
    fn on_event(&mut self, _ev: &Event, cx: &EventCtx) -> bool {
        let (kind, button) = match cx.event {
            Event::PointerDown { button, .. } => ("down", button),
            Event::PointerUp { button, .. } => ("up", button),
            Event::ContextRequest { x, y } => {
                self.requests.push((x, y));
                return true;
            }
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
    fulgurite::leaf_common!(Probe, 0);
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

impl fulgurite::event::App for App {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self::State {
        self
    }
    type Msg = fulgurite::event::Never;
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
    input: fulgurite::event::InputState,
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

/// What each method should deliver for each button, after T90.2's
/// filter: `(presses and releases the probe is handed, context requests,
/// handled)`.
fn expected(method: &str, asked: Option<&str>) -> (&'static [&'static str], usize, bool) {
    match (button(asked), method) {
        (PointerButton::Left, "input.click") => (&["down", "up"], 0, true),
        (PointerButton::Left, "input.pointerDown") => (&["down"], 0, true),
        (PointerButton::Left, _) => (&["up"], 0, true),
        // A right press is one request at its point; its release is
        // nothing.
        (PointerButton::Right, "input.click" | "input.pointerDown") => (&[], 1, true),
        (PointerButton::Right, _) => (&[], 0, false),
        (PointerButton::Middle, _) => (&[], 0, false),
    }
}

/// **Each of the three methods delivers what its button means** —
/// T90.2's rewrite of T90.1's gate. `"right"` arrives as one
/// `ContextRequest` at the pressed point (and its release as nothing),
/// `"middle"` as `handled: false`, and an omitted or `"left"` button as a
/// `Left` press and release (the accept direction: every caller that
/// predates T90.1 passes none). The reply echoes the button every time.
#[test]
fn each_method_delivers_what_its_button_means() {
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
            let at = (r["result"]["at"][0].as_f64().unwrap() as f32, r["result"]["at"][1].as_f64().unwrap() as f32);
            let handled = r["result"]["handled"].as_bool().unwrap();
            let (kinds, requests, want_handled) = expected(method, asked);
            let probe = &h.app.ui.probe;
            println!(
                "{method:<17} button {:<14} -> handled {handled}, presses {:?}, requests {:?} (pressed at {at:?}); echo {}",
                format!("{asked:?}"),
                probe.seen,
                probe.requests,
                r["result"]["button"]
            );
            let got: Vec<&str> = probe.seen.iter().map(|s| s.0).collect();
            if got != kinds {
                wrong.push(format!("{method} {asked:?}: the probe was handed {:?}, want {kinds:?}", probe.seen));
            }
            for &(kind, got, _) in &probe.seen {
                if got != want {
                    wrong.push(format!("{method} asked {asked:?}: the probe read {got:?} on its {kind}"));
                }
            }
            if probe.requests.len() != requests || probe.requests.iter().any(|&p| p != at) {
                wrong.push(format!("{method} {asked:?}: requests {:?}, want {requests} at {at:?}", probe.requests));
            }
            if handled != want_handled {
                wrong.push(format!("{method} {asked:?}: handled {handled}, want {want_handled}"));
            }
            assert_eq!(r["result"]["button"], echo, "{method}: the reply does not echo the button");
        }
    }
    assert!(wrong.is_empty(), "a button did not arrive as it means:\n{}", wrong.join("\n"));
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
        assert!(
            h.app.ui.probe.seen.is_empty() && h.app.ui.probe.requests.is_empty(),
            "{method}: a refused button still queued {:?} {:?}",
            h.app.ui.probe.seen,
            h.app.ui.probe.requests
        );
        let r = call(&mut h, method, json!({ "selector": "#probe", "button": 2 }));
        assert_eq!(r["error"]["code"], -32602, "{method}: a number for a button: {r}");
    }
}
