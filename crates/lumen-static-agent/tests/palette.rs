//! M25/T25.3: `app.setPalette` reaches a host that has a palette, and a
//! host that has none says so.

use lumen_static_widgets::ops::Label;
use lumen_static::style::Palette;
use lumen_static::{pass, Constraints, Event, InputQueue, NodeState, StaticTree, Widget};
use lumen_static_agent::{Agent, AgentHost, TreeRef};
use serde_json::{json, Value};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("head")]
    head: Label,
}

#[derive(Default)]
struct App {
    ui: Ui,
}

impl lumen_static::event::App for App {
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
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 100.0 };

/// A host, with or without a palette of its own.
struct Host {
    app: App,
    states: [NodeState; Ui::NODE_COUNT],
    styles: Vec<lumen_static::Style>,
    queue: InputQueue,
    frame: u64,
    /// `None` for a host with no palette to set.
    palette: Option<Palette>,
}

impl Host {
    fn new(palette: Option<Palette>) -> Self {
        let app = App::default();
        let styles = pass::default_styles(&app.ui);
        Host {
            app,
            states: [NodeState::default(); Ui::NODE_COUNT],
            styles,
            queue: InputQueue::new(),
            frame: 0,
            palette,
        }
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
        while self.queue.pop().is_some() {}
        pass::layout(&mut self.app.ui, &self.styles, &mut self.states, VIEW);
        self.frame += 1;
        // A double that drops its queue handles nothing, and says so.
        (self.frame, false)
    }
    fn set_palette(&mut self, palette: Palette) -> bool {
        match self.palette.as_mut() {
            Some(p) => {
                *p = palette;
                true
            }
            None => false,
        }
    }
}

/// The trait's default, which a host that forgets to override gets.
struct Bare(Host);

impl AgentHost for Bare {
    fn tree(&self) -> TreeRef<'_> {
        self.0.tree()
    }
    fn enqueue(&mut self, ev: Event) {
        self.0.enqueue(ev)
    }
    fn pump(&mut self) -> (u64, bool) {
        self.0.pump()
    }
}

fn call(host: &mut dyn AgentHost, method: &str, params: Value) -> Value {
    Agent::new().dispatch_input(
        host,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }),
    )
}

#[test]
fn set_palette_by_name_reaches_the_host() {
    let mut h = Host::new(Some(Palette::LIGHT));
    let r = call(&mut h, "app.setPalette", json!({ "name": "dark" }));
    println!("{r}");
    assert_eq!(r["result"]["applied"], true, "unexpected reply: {r}");
    assert_eq!(h.palette, Some(Palette::DARK), "the host did not receive the dark palette");
    assert!(r["result"]["frame"].as_u64().unwrap_or(0) > 0, "the method did not pump a frame");
}

#[test]
fn a_host_without_a_palette_reports_not_applied() {
    let mut bare = Bare(Host::new(None));
    let r = call(&mut bare, "app.setPalette", json!({ "name": "dark" }));
    println!("{r}");
    assert_eq!(
        r["result"]["applied"], false,
        "the default claimed a palette it has no way to apply: {r}"
    );
}

#[test]
fn an_unknown_palette_is_refused_not_ignored() {
    let mut h = Host::new(Some(Palette::LIGHT));
    let r = call(&mut h, "app.setPalette", json!({ "name": "sepia" }));
    println!("{r}");
    assert_eq!(r["error"]["code"], -32602, "expected invalid params: {r}");
    assert_eq!(h.palette, Some(Palette::LIGHT), "a refused palette still changed the host");
}
