//! T6.3: `#[derive(App)]` — the tree accessors and the update step,
//! generated from declarations at the field.

use lumen_static_widgets::ops::{Button, Fixed, Label, Spacer};
use lumen_static::{App as AppDerive, EventCtx, StaticTree};

// ANCHOR: tree
#[derive(StaticTree, Default)]
#[widget(column, spacing = 2.0)]
struct Ui {
    #[widget_id("value")]
    value: Label<String>,
    #[widget_id("ratio")]
    ratio: Label<String>,
    #[widget_id("shout")]
    shout: Label<String>,
    gap: Spacer,
    #[widget_id("go")]
    go: Button<Counter>,
}
// ANCHOR_END: tree

// ANCHOR: projection
/// A `with =` projection: `fn(&mut Target, &Source)`.
fn shout(dst: &mut Label<String>, src: &str) {
    lumen_static::bind::set_if_changed(dst, &src.to_uppercase());
}
// ANCHOR_END: projection

// ANCHOR: hook
/// **Returns whether it changed anything** (M27/T27.3), mirroring
/// `App::tick`. It returned `()` until then, which left the derive no
/// choice but to mark every pump changed — and a frame that never
/// settles damages the whole window on every interaction.
fn recompute(c: &mut Counter) -> bool {
    c.hook_ran += 1;
    // This hook only counts; it changes nothing the frame can see.
    false
}
// ANCHOR_END: hook

// ANCHOR: state
#[derive(AppDerive, Default)]
#[app(after_update = recompute)]
struct Counter {
    #[tree]
    ui: Ui,
    #[bind(ui.value, "count: {}")]
    count: i64,
    // Extra format arguments are written out in full; the annotated
    // field is always the first.
    #[bind(ui.ratio, "{} of {}", self.total)]
    done: u32,
    #[bind(ui.shout, with = shout)]
    name: String,
    total: u32,
    hook_ran: u32,
}
// ANCHOR_END: state

// ANCHOR: handler
fn bump(c: &mut Counter, _: &EventCtx) {
    c.count += 1;
}
// ANCHOR_END: handler

#[test]
fn update_is_generated_from_the_declarations() {
    // ANCHOR: assertions
    use lumen_static::event::App;

    let mut c = Counter { total: 10, name: "ada".into(), ..Default::default() };
    c.update();

    assert_eq!(c.ui.value.as_str(), "count: 0");
    assert_eq!(c.ui.ratio.as_str(), "0 of 10", "extra args reach the format");
    assert_eq!(c.ui.shout.as_str(), "ADA", "`with =` projection ran");
    assert_eq!(c.hook_ran, 1, "after_update hook ran once");

    c.count = 7;
    c.done = 3;
    c.update();
    assert_eq!(c.ui.value.as_str(), "count: 7");
    assert_eq!(c.ui.ratio.as_str(), "3 of 10");
    assert_eq!(c.hook_ran, 2);
    // ANCHOR_END: assertions
}

#[test]
fn the_tree_accessors_are_generated_too() {
    use lumen_static::event::App;
    use lumen_static::Widget;

    let mut c = Counter::default();
    assert_eq!(c.tree().value.as_str(), "");
    c.tree_mut().value.set("direct".into());
    assert_eq!(c.ui.value.as_str(), "direct");
    // The associated type resolved to the `#[tree]` field's type.
    assert_eq!(<Counter as App>::Tree::NODE_COUNT, Ui::NODE_COUNT);
}

#[test]
fn a_handler_change_is_visible_after_the_next_update() {
    use lumen_static::event::App;

    let mut c = Counter::default();
    c.ui.go.on_click = bump;
    c.update();
    assert_eq!(c.ui.value.as_str(), "count: 0");

    // What dispatch does: run the handler, then let the loop update.
    bump(&mut c, &EventCtx::at(0));
    assert_eq!(c.ui.value.as_str(), "count: 0", "not yet — update owns the moment");
    c.update();
    assert_eq!(c.ui.value.as_str(), "count: 1");
}

#[test]
fn a_quiet_update_leaves_the_measurement_memo_warm() {
    use lumen_static::event::App;
    use lumen_static::{BlockMetrics, Constraints, LayoutCx, Node, NodeState, Style};

    const LIMITS: Constraints =
        Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 400.0 };

    let mut c = Counter::default();
    c.update();
    let mut m = BlockMetrics;
    let mut cx = LayoutCx::new(&mut m);
    let (style, mut st) = ([Style::default()], [NodeState::default()]);
    c.ui.value.layout(&mut cx, LIMITS, &style, &mut st, 0);
    assert!(c.ui.value.measure_is_warm(LIMITS, &style[0], 1.0));

    c.update(); // nothing changed
    assert!(c.ui.value.measure_is_warm(LIMITS, &style[0], 1.0), "a binding that runs is not a binding that writes");
}

// ---- M32: a source path, and that it stays narrowable ----------------

/// A nested state struct, the shape a split application has.
#[derive(Default)]
struct Nested {
    depth: Depth,
}

#[derive(Default)]
struct Depth {
    n: i64,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct DeepUi {
    #[widget_id("deep")]
    deep: Label<String>,
}

#[derive(AppDerive, Default)]
struct DeepApp {
    #[tree]
    ui: DeepUi,
    /// `depth.n` is a path **within** this field, so the binding formats
    /// `self.state.depth.n` — two levels down, which the format form
    /// could not reach before M32 and which a split application needs.
    #[bind(ui.deep, depth.n, "n = {}")]
    state: Nested,
}

#[test]
fn a_binding_can_name_a_path_inside_the_annotated_field() {
    let mut app = DeepApp::default();
    assert!(app.update(), "the first update writes `n = 0`");
    assert_eq!(app.ui.deep.as_str(), "n = 0");

    app.state.depth.n = 41;
    assert!(app.update(), "a changed value writes");
    assert_eq!(app.ui.deep.as_str(), "n = 41", "the binding read two levels down");
}

/// **The property that makes the source path worth having.**
///
/// A split application — state in its own struct, out of a handler's
/// reach — could previously only bind through a `with =` hook, and
/// `docs/plan-damage-precision.md` §2.4 lists those as *permanently
/// unnarrowable*: the hook returns nothing, so the derive reports `true`
/// unconditionally and `Host::pump_with` marks the whole tree.
///
/// The source-path form keeps `format_into`'s comparison, so an update
/// that changes nothing reports `false` — which is what lets the mark be
/// narrowed at all. Without this assertion the split would buy M29's win
/// on the handler path and hand it straight back here.
#[test]
fn a_source_path_binding_still_reports_precisely() {
    let mut app = DeepApp::default();
    app.state.depth.n = 7;
    assert!(app.update(), "setup: the value changed, so the update writes");

    assert!(
        !app.update(),
        "a second update wrote nothing and must say so — a `with =` hook would \
         answer `true` here and mark the whole tree every frame"
    );
    app.state.depth.n = 8;
    assert!(app.update(), "and it still reports a real change");
}

/// A widget whose value is not text: the value form's target.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct ValueUi {
    #[widget_id("pane")]
    pane: Fixed,
}

#[derive(Default)]
struct Knobs {
    blur: f32,
}

#[derive(AppDerive, Default)]
struct ValueApp {
    #[tree]
    ui: ValueUi,
    /// The value form, M33/T33.1: no format string, so `blur` is copied
    /// into the target through `BindValue` — which compares first.
    #[bind(ui.pane.w, blur)]
    state: Knobs,
}

#[test]
fn a_value_binding_copies_and_reports_precisely() {
    let mut app = ValueApp::default();
    app.state.blur = 12.0;
    assert!(app.update(), "a changed value writes");
    assert_eq!(app.ui.pane.w, 12.0, "the value reached the target");
    assert!(
        !app.update(),
        "a second update wrote nothing and must say so — `with =` would answer `true`"
    );
    app.state.blur = 3.0;
    assert!(app.update(), "and it still reports a real change");
    assert_eq!(app.ui.pane.w, 3.0);
}
