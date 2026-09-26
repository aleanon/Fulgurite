//! The counter — plan T5.4, the milestone this project had not had:
//! a real application, laid out, painted and rendered to pixels, driven
//! by the event system, and inspectable by the agent protocol.
//!
//! It runs headless and writes a PPM, and its assertions live in `main`
//! — so `cargo test` alone does *not* execute them. `just fixtures` runs
//! it, and `.github/workflows/ci.yml` runs `just fixtures`; that is what
//! makes the claim true. Until 2026-09-02 nothing ran it at all.
//!
//! A winit front end adds exactly one thing: window events into
//! `Host::post` and `Canvas::pixels` blitted to the surface.

// One import line for the whole vocabulary. Before the prelude existed
// this was three modules plus the root, for a forty-line app.
use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;
use lumen_static_agent::{Agent, TreeRef};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::program::{application, Program};
use lumen_static_shell::{Host, CLEAR};

// ---- the tree: topology in the type, values in the fields (§3) -------

// ANCHOR: tree
#[derive(StaticTree, Default)]
#[widget(row, spacing = 8.0)]
struct Controls {
    #[widget_id("dec")]
    dec: Button<Tally>,
    gap: Spacer,
    #[widget_id("inc")]
    inc: Button<Tally>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 12.0)]
struct Ui {
    #[widget_id("title")]
    title: Label,
    /// The counter's presentation copy. Owned `String` storage, so a
    /// binding can format into it without allocating (M3, T6.2).
    #[widget_id("value")]
    value: Label<String>,
    #[widget_id("hint")]
    hint: Label,
    controls: Controls,
}

// ANCHOR_END: tree

// ---- application state, beside the tree (not inside it) --------------

/// `#[tree]` names the widget tree; `#[bind]` says how a state field
/// reaches the screen. The derive writes the `App` impl — the tree
/// accessors *and* the update step — so the formatting is declared once,
/// next to the value it formats, instead of living in a method that has
/// to be kept in step with the fields and remembered at every call site.
///
/// Sibling fields stay the source of truth and widget fields still hold
/// the presentation copy (`docs/layout-and-storage-decisions.md` §1) —
/// owning that copy is what lets the label cache its measurement. The
/// binding is only about who keeps the two in agreement, and when.
// ANCHOR: state
#[derive(DeriveApp)]
struct Counter {
    #[tree]
    ui: Ui,
    /// **What a handler may touch** (M33). The tree is a sibling of it,
    /// so a handler cannot write a widget: every tree write is a binding.
    /// `count` is a path *inside* this field, which the binding formats.
    #[state]
    #[bind(ui.value, count, "count: {}")]
    tally: Tally,
}

/// The state, apart from the tree that presents it.
#[derive(Default)]
struct Tally {
    count: i64,
}
// ANCHOR_END: state

// ANCHOR: handlers
fn on_inc(t: &mut Tally, _: &EventCtx) {
    t.count += 1;
}

fn on_dec(t: &mut Tally, _: &EventCtx) {
    t.count -= 1;
}
// ANCHOR_END: handlers

// ANCHOR: build
/// **Construction lives here, and only here** — M31/T31.0, M49/T49.4.
///
/// The program below is described with this function, and every host is
/// built from the program: the window, a `--shot`, and `Host::reload`,
/// which re-runs it and grafts the kept state on. A swap once rebuilt
/// the application through `Default::default()`, *without* the four
/// lines below, and every button came back wired to `ignore_click`; the
/// program's boot is now the only constructor there is.
impl Counter {
    pub fn build() -> Self {
        let mut c = Counter { ui: Ui::default(), tally: Tally::default() };
        c.ui.title.set("Counter");
        c.ui.hint.set("click + or - to change the tally");
        c.ui.controls.dec.label = "-";
        c.ui.controls.dec.on_click = on_dec;
        c.ui.controls.inc.label = "+";
        c.ui.controls.inc.on_click = on_inc;
        c
    }
}
// ANCHOR_END: build

// ANCHOR: style
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
// ANCHOR_END: style

// ANCHOR: program
/// **The program, described once** — M32 (`docs/plan-program.md`).
///
/// How to build it and how it is styled, in one value. The headless host
/// below is built from it, and a window would be too
/// (`lumen_static_winit::Run`), so the two cannot be styled differently.
fn program() -> Program<Counter> {
    application(Counter::build).title("Counter").size(320, 200).style(|_: &Counter| stylesheet())
}
// ANCHOR_END: program

fn main() {
    // The same `host()` a window drives: boot, palette, and the sheet
    // from the one place a sheet comes from.
    let mut host: Host<Counter> = program().host();
    // M25/T25.2: every rule does something. `just fixtures` runs this.
    assert!(host.sheet_findings().is_empty(), "stylesheet rules that change nothing: {:?}", host.sheet_findings());
    host.pump();

    // Drive it the way a user would: click "+" three times, "-" once.
    // Coordinates come from the laid-out tree, not from guesses. One
    // pump per click, and no update call: `pump` runs the bindings
    // between dispatch and layout, so the frame cannot render a count
    // the handler has already changed.
    let agent = Agent::new();
    for (id, times) in [("#inc", 3), ("#dec", 1)] {
        for _ in 0..times {
            let (x, y) = center_of(&host, id);
            host.click(x, y);
            host.pump();
        }
    }
    assert_eq!(host.app.tally.count, 2, "three increments and one decrement");
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
        host.app.tally.count,
        diag["nodes"],
        Ui::DEPTH,
        host.list.len(),
        ink * 100.0,
    );
}

/// Ask the agent where a widget is, by author id — the same path an
/// agent driving this app would take.
fn center_of(host: &Host<Counter>, selector: &str) -> (f32, f32) {
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
