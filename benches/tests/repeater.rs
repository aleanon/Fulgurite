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
    let sty = pass::default_styles(&*app);
    pass::layout(app, &sty, st, VIEW);
}

#[test]
fn node_count_is_data_independent() {
    // The §6 property, as a compile-time fact.
    const _: () = assert!(Repeat::<Item>::NODE_COUNT == 1);
    const _: () = assert!(ListApp::NODE_COUNT == 3);

    // And unchanged by data size at runtime: same table, same state arena.
    // 699,050 rather than 1,000,000: past `count * item_extent = 2^24`
    // the f32 scroll offset cannot select every row, and `ui.lint` says
    // so (`W1009`, `Repeat::MAX_ADDRESSABLE_EXTENT`). The property under
    // test here is that `NODE_COUNT` is data-independent, which a
    // reachable row count demonstrates just as well.
    for rows in [10usize, 699_000] {
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

    // Under the `Length` model the header is `Shrink` (16px of text) and
    // the list is `Fill`, so the viewport is 600 - 16 = 584px and
    // ceil(584/24) + 1 = 26 rows materialize, out of 10,000.
    assert_eq!(app.list.window_len(), 26);
    assert_eq!(app.list.first_row(), 0);

    // The repeater reports its viewport to the parent…
    let rep_idx = 2; // root=0, header=1, list=2
    assert_eq!(st[rep_idx].size.h, 584.0);
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
    let mut a = lumen_static::DisplayList::new();
    pass::paint(&app, &st, &mut a);

    app.list.scroll = 480.0;
    run(&mut app, &mut st);
    let mut b = lumen_static::DisplayList::new();
    pass::paint(&app, &st, &mut b);
    assert_ne!(a.hash_from(0), b.hash_from(0), "scrolled content must paint differently");
}

#[test]
fn empty_and_tiny_lists_are_sound() {
    for rows in [0usize, 1, 3] {
        let mut app = ListApp { list: Repeat::new(rows, 24.0, bind_icon), ..Default::default() };
        let mut st = [NodeState::default(); ListApp::NODE_COUNT];
        run(&mut app, &mut st);
        // Only what is on screen materializes, and an empty list
        // materializes nothing…
        assert_eq!(app.list.window_len(), rows.min(26));
        // …but a list is `Fill`: it occupies its scrollport whether or
        // not it has rows, which is what a list pane should do. Its
        // *content* extent stays virtual.
        assert_eq!(st[2].size.h, 584.0);
    }
}
