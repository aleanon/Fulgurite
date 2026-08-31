//! Step 3 correctness: the §6 scaling property, window materialization,
//! binding, scrolling, and composition of a repeater inside a derived
//! container.

use benches::Item;
use lumen_static::ops::Label;
use lumen_static::repeat::Repeat;
use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};

/// A list screen: NODE_COUNT is 3 — header, repeater, root — no matter
/// how many rows the repeater displays.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct ListApp {
    header: Label,
    list: Repeat<Item>,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 600.0 };

fn bind_icon(item: &mut Item, row: usize) {
    item.icon.w = (row % 50) as f32 + 1.0;
    item.icon.h = 10.0;
}

fn run(app: &mut ListApp, st: &mut [NodeState; ListApp::NODE_COUNT]) {
    pass::layout(app, st, VIEW);
}

#[test]
fn node_count_is_data_independent() {
    // The §6 property, as a compile-time fact.
    const _: () = assert!(Repeat::<Item>::NODE_COUNT == 1);
    const _: () = assert!(ListApp::NODE_COUNT == 3);

    // And unchanged by data size at runtime: same table, same state arena.
    for rows in [10usize, 1_000_000] {
        let mut app = ListApp { list: Repeat::new(rows, 24.0, bind_icon), ..Default::default() };
        let mut st = [NodeState::default(); ListApp::NODE_COUNT];
        run(&mut app, &mut st);
        assert_eq!(ListApp::SUB_TABLE.len(), 3);
        assert_eq!(st.len(), 3);
    }
}

#[test]
fn only_the_visible_window_materializes() {
    let mut app = ListApp { list: Repeat::new(10_000, 24.0, bind_icon), ..Default::default() };
    let mut st = [NodeState::default(); ListApp::NODE_COUNT];
    run(&mut app, &mut st);

    // The column splits 600px between header and list -> 300px viewport.
    // ceil(300/24) + 1 = 14 rows materialized, out of 10,000.
    assert_eq!(app.list.window_len(), 14);
    assert_eq!(app.list.first_row(), 0);

    // The repeater reports the viewport-clamped extent to its parent…
    let rep_idx = 2; // root=0, header=1, list=2
    assert_eq!(st[rep_idx].size.h, 300.0);
    // …not the 240,000px logical extent, which stays virtual.
}

#[test]
fn binding_reaches_visible_rows_and_scrolling_rebinds() {
    let mut app = ListApp { list: Repeat::new(10_000, 24.0, bind_icon), ..Default::default() };
    let mut st = [NodeState::default(); ListApp::NODE_COUNT];
    run(&mut app, &mut st);

    // Item template: root=0, icon=1 (first field). Row r's icon is
    // (r % 50) + 1 wide; instance k shows row first + k.
    for k in 0..app.list.window_len() {
        let icon = app.list.instance_state(k, 1);
        assert_eq!(icon.size.w, (k % 50) as f32 + 1.0, "row {k} before scroll");
    }

    // Scroll down 25.5 rows: first materialized row becomes 25.
    app.list.scroll = 25.5 * 24.0;
    run(&mut app, &mut st);
    assert_eq!(app.list.first_row(), 25);
    for k in 0..app.list.window_len() {
        let row = 25 + k;
        let icon = app.list.instance_state(k, 1);
        assert_eq!(icon.size.w, (row % 50) as f32 + 1.0, "row {row} after scroll");
    }

    // Instance geometry is shifted by row offset minus scroll, from the
    // repeater's own origin. The header is a 16px label, so the repeater
    // sits at y=16; row 25 starts 0.5 * 24 = 12px above the window top.
    assert_eq!(st[2].pos.y, 16.0);
    let inst0 = app.list.instance_state(0, 0);
    assert_eq!(inst0.pos.y, st[2].pos.y + (25.0 * 24.0 - app.list.scroll));
    assert_eq!(inst0.pos.y, 16.0 - 12.0);
}

#[test]
fn paint_covers_the_window_and_tracks_scroll() {
    let mut app = ListApp { list: Repeat::new(10_000, 24.0, bind_icon), ..Default::default() };
    let mut st = [NodeState::default(); ListApp::NODE_COUNT];
    run(&mut app, &mut st);
    let mut a = 0u64;
    pass::paint(&app, &st, &mut a);

    app.list.scroll = 480.0;
    run(&mut app, &mut st);
    let mut b = 0u64;
    pass::paint(&app, &st, &mut b);
    assert_ne!(a, b, "scrolled content must paint differently");
}

#[test]
fn empty_and_tiny_lists_are_sound() {
    for rows in [0usize, 1, 3] {
        let mut app = ListApp { list: Repeat::new(rows, 24.0, bind_icon), ..Default::default() };
        let mut st = [NodeState::default(); ListApp::NODE_COUNT];
        run(&mut app, &mut st);
        assert_eq!(app.list.window_len(), rows.min(14));
        assert_eq!(st[2].size.h, (rows as f32 * 24.0).min(300.0));
    }
}
