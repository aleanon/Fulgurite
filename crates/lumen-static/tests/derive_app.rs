//! T6.3: `#[derive(App)]` — the tree accessors and the update step,
//! generated from declarations at the field.

use lumen_static::ops::{Button, Label, Spacer};
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
fn recompute(c: &mut Counter) {
    c.hook_ran += 1;
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
    assert!(c.ui.value.measure_is_warm(LIMITS));

    c.update(); // nothing changed
    assert!(c.ui.value.measure_is_warm(LIMITS), "a binding that runs is not a binding that writes");
}
