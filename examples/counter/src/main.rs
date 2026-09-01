//! The counter — plan T5.4, the milestone this project had not had:
//! a real application, laid out, painted and rendered to pixels, driven
//! by the event system, and inspectable by the agent protocol.
//!
//! It runs headless and writes a PPM, so it is verifiable in CI. A winit
//! front end adds exactly one thing: window events into `Host::post` and
//! `Canvas::pixels` blitted to the surface.

use lumen_static::ops::{Button, Label, Spacer};
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{Edges, EventCtx, StaticTree, Widget};
use lumen_static_agent::{Agent, TreeRef};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::{Host, CLEAR};

// ---- the tree: topology in the type, values in the fields (§3) -------

#[derive(StaticTree, Default)]
#[widget(row, spacing = 8.0)]
struct Controls {
    #[widget_id("dec")]
    dec: Button<App>,
    gap: Spacer,
    #[widget_id("inc")]
    inc: Button<App>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 12.0)]
struct Ui {
    #[widget_id("title")]
    title: Label,
    /// The counter's presentation copy. Owned `String` storage, so the
    /// update step can format into it (M3).
    #[widget_id("value")]
    value: Label<String>,
    #[widget_id("hint")]
    hint: Label,
    controls: Controls,
}

// ---- application state, beside the tree (not inside it) --------------

#[derive(Default)]
struct App {
    ui: Ui,
    count: i64,
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

fn on_inc(a: &mut App, _: &EventCtx) {
    a.count += 1;
}

fn on_dec(a: &mut App, _: &EventCtx) {
    a.count -= 1;
}

impl App {
    fn new() -> Self {
        let mut a = App::default();
        a.ui.title.set("Counter");
        a.ui.hint.set("click + or - to change the tally");
        a.ui.controls.dec.label = "-";
        a.ui.controls.dec.on_click = on_dec;
        a.ui.controls.inc.label = "+";
        a.ui.controls.inc.on_click = on_inc;
        a.update();
        a
    }

    /// The update step: application state reaches the screen here.
    /// Sibling fields are the source of truth; widget fields hold the
    /// presentation copy, and owning that copy is what lets the label
    /// cache its measurement (`docs/layout-and-storage-decisions.md` §1).
    fn update(&mut self) {
        self.ui.value.set(format!("count: {}", self.count));
    }
}

fn stylesheet() -> Vec<Rule> {
    vec![
        // A column of shrink-height children, padded away from the edge.
        Rule::new(Selector::Kind("Ui"), Patch::new().padding(Edges::all(16.0))),
        // The control row spans the width so the spacer can push the
        // buttons apart — `Fill` doing the job flexbox would need
        // `justify-content` for.
        Rule::new(Selector::Kind("Controls"), Patch::new().width(Length::FILL)),
    ]
}

fn main() {
    let mut host = Host::new(App::new(), 320, 200);
    host.set_stylesheet(&stylesheet());
    host.pump();

    // Drive it the way a user would: click "+" three times, "-" once.
    // Coordinates come from the laid-out tree, not from guesses.
    let agent = Agent::new();
    for (id, times) in [("#inc", 3), ("#dec", 1)] {
        for _ in 0..times {
            let (x, y) = center_of(&host, id);
            host.click(x, y);
            host.pump();
            host.app.update();
            host.pump();
        }
    }
    assert_eq!(host.app.count, 2, "three increments and one decrement");
    assert_eq!(host.app.ui.value.as_str(), "count: 2");

    // Render the frame to pixels.
    let mut canvas = Canvas::new(320, 200);
    canvas.clear(CLEAR);
    host.render(&mut canvas);
    let ink = canvas.ink_fraction(CLEAR);
    assert!(ink > 0.01, "the frame is not blank ({ink:.3} inked)");

    let path = std::env::args().nth(1).unwrap_or_else(|| "counter.ppm".into());
    std::fs::write(&path, canvas.to_ppm()).expect("write frame");

    // The agent sees the same frame the renderer drew.
    let tree = TreeRef::of_root(&host.app.ui, &host.styles, &host.states);
    let lint = agent.dispatch(
        &tree,
        &serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "app.diagnostics", "params": {}
        }),
    );
    let diag = &lint["result"];

    println!(
        "counter: count={} nodes={} depth={} cmds={} ink={:.1}% -> {path}",
        host.app.count,
        diag["nodes"],
        Ui::DEPTH,
        host.list.len(),
        ink * 100.0,
    );
}

/// Ask the agent where a widget is, by author id — the same path an
/// agent driving this app would take.
fn center_of(host: &Host<App>, selector: &str) -> (f32, f32) {
    let agent = Agent::new();
    let tree = TreeRef::of_root(&host.app.ui, &host.styles, &host.states);
    let r = agent.dispatch(
        &tree,
        &serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "ui.getLayout",
            "params": { "selector": selector }
        }),
    );
    let l = &r["result"];
    let (x, y) = (l["pos"][0].as_f64().unwrap(), l["pos"][1].as_f64().unwrap());
    let (w, h) = (l["size"][0].as_f64().unwrap(), l["size"][1].as_f64().unwrap());
    ((x + w / 2.0) as f32, (y + h / 2.0) as f32)
}
