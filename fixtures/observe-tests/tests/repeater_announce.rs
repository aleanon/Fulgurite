//! A repeater's rows as the agent and AccessKit see them -- moved from
//! `crates/fulgurite/tests/scroll_containment.rs` (M43/T43.7), whose
//! header has the containment invariant this pins one surface over. It
//! needs `observe`, and core's own tests no longer build with it: see this
//! package's `Cargo.toml`.

use fulgurite::repeat::{noop_bind, Repeat};
use fulgurite::{pass, Constraints, NodeState, StaticTree, Widget};
use fulgurite_widgets::ops::{Fixed, Label};

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Row {
    icon: Fixed,
    name: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    list: Repeat<Row>,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 100.0 };

/// The agent and AccessKit must see the same list paint and hit do.
///
/// A repeater's window overruns its viewport by construction, and F1.6
/// clipped those rows from paint while `hit` had always rejected them —
/// but `observe` announced them anyway, with bounds where nothing draws
/// and no click can land. That is the probe-versus-click divergence one
/// surface over: an agent reading `ui.getTree` would see rows it could
/// not interact with (F4.7).
#[test]
fn observe_announces_only_rows_that_are_actually_on_screen() {
    use fulgurite::widget::Node;

    let mut ui = Ui { list: Repeat::new(1000, 24.0, noop_bind) };
    ui.list.scroll = 12.0;
    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    pass::layout(&mut ui, &sty, &mut st, VIEW);

    let materialized = ui.list.window_len();
    assert!(materialized >= 6, "the window overruns: {materialized} rows for a 100px viewport");

    let mut root = accesskit::Node::new(accesskit::Role::GenericContainer);
    let mut extra: Vec<(accesskit::NodeId, accesskit::Node)> = Vec::new();
    ui.list.observe(&st, 1, 1, &mut root, &mut extra);

    let announced = root.children().len();
    assert!(
        announced < materialized,
        "announced {announced} of {materialized} materialized rows — the overrun must be excluded"
    );

    // Every announced row overlaps the list's own rect.
    let list_rect = fulgurite::ops::rect_of(&st[1]);
    for k in 0..materialized {
        let r = fulgurite::ops::rect_of(&ui.list.row_state(k));
        let on_screen = fulgurite::hit::rects_overlap(list_rect, r);
        if !on_screen {
            assert!(
                r[3] <= list_rect[1] || r[1] >= list_rect[3],
                "an excluded row must be wholly outside the viewport, got {r:?}"
            );
        }
    }
}
