//! M27/T27.10 — `Pagination`, `Tree`, `DataGrid`.
//!
//! Three widgets that answer *where the data lives* three different
//! ways, because their data differs: a pagination has none, a tree owns
//! its shape (collapsing is arithmetic over the array), and a grid owns
//! nothing — its rows are a `Repeat` the application binds.

use lumen_static::event::{Event, EventCtx, Key};
use lumen_static::ops::Label;
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, NodeState, Point, StaticTree};
use lumen_static_widgets::{Branch, DataGrid, Pagination, Tree, MAX_TREE_NODES};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 400.0 };

fn lay<W: Widget + Node>(w: &mut W) -> Vec<NodeState> {
    let styles = lumen_static::pass::default_styles(w);
    let mut st = vec![NodeState::default(); <W as Widget>::NODE_COUNT];
    let mut m = lumen_static::cx::BlockMetrics;
    {
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(w, &mut cx, VIEW, &styles, &mut st, 0);
    }
    Node::place(w, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);
    st
}

fn press_at<W: Widget + Node>(w: &mut W, local: Point) -> bool {
    let ev = Event::PointerDown {
        x: 0.0,
        y: 0.0,
        button: lumen_static::PointerButton::Left,
        clicks: 1,
    };
    let mut cx = EventCtx::for_event(0, ev);
    cx.local = local;
    w.on_event(&ev, &cx)
}

fn key<W: Node>(w: &mut W, k: Key) -> bool {
    let ev = Event::Key { key: k, down: true };
    w.on_event(&ev, &EventCtx::for_event(0, ev))
}

// ---- Pagination ------------------------------------------------------

/// **The window is centred, not paged in blocks.** A block-paged strip
/// jumps under the finger — you press "5" and the numbers shift so 5 is
/// somewhere else — which is the one thing every pagination gets wrong.
#[test]
fn the_page_window_follows_the_current_page() {
    let mut p = Pagination::new(30);
    let (first, n) = p.window();
    assert_eq!((first, n), (0, 9), "at page 0 the window should start at 0");
    // **Centred, and that is what is asserted** — the page sits in the
    // middle of its own window, away from the ends. "inside the window"
    // is satisfied by block paging too, which is what the first version
    // of this checked and why an injection that paged in blocks left it
    // green.
    for page in [10usize, 14, 19] {
        p.go(page);
        let (first, n) = p.window();
        let offset = page - first;
        assert_eq!(
            offset, n / 2,
            "page {page} sits {offset} into its window {first}..{}; centred is {}",
            first + n, n / 2
        );
    }
}

/// **And it moves by one when the page does**, which block paging does
/// not: a block window stays put for eight presses and then jumps.
#[test]
fn the_window_slides_one_page_at_a_time() {
    let mut p = Pagination::new(30);
    p.go(12);
    let a = p.window().0;
    p.go(13);
    let b = p.window().0;
    assert_eq!(
        b, a + 1,
        "the window went {a} -> {b} for one page; a block-paged strip jumps \
         under the finger, which is the one thing every pagination gets wrong"
    );
}

#[test]
fn the_window_clamps_at_both_ends() {
    let mut p = Pagination::new(30);
    p.go(0);
    assert_eq!(p.window().0, 0, "the window ran off the front");
    p.go(29);
    let (first, n) = p.window();
    assert_eq!(first + n, 30, "the window ran off the back: {first}..{}", first + n);
}

#[test]
fn a_short_list_shows_every_page() {
    let p = Pagination::new(3);
    assert_eq!(p.window(), (0, 3), "3 pages should show as 3, not padded to the maximum");
}

#[test]
fn the_arrows_step_and_stop() {
    let mut p = Pagination::new(3);
    assert!(!p.prev_page(), "previous moved from the first page");
    assert!(p.next_page() && p.next_page(), "next did not reach the last page");
    assert!(!p.next_page(), "next ran past the last page: {}", p.page());
    assert_eq!(p.page(), 2);
}

/// A press on a number goes to that page — and the *first* and *last*
/// slots are the arrows, not pages.
#[test]
fn a_press_picks_the_slot_under_it() {
    let mut p = Pagination::new(30);
    let st = lay(&mut p);
    let w = st[0].size.w;
    let each = w / 11.0; // 9 numbers + two arrows
    // Slot 1 is the first page number.
    press_at(&mut p, Point { x: each * 1.5, y: 10.0 });
    assert_eq!(p.page(), 0, "the first number is page 0, got {}", p.page());
    // Slot 3 is the third.
    press_at(&mut p, Point { x: each * 3.5, y: 10.0 });
    assert_eq!(p.page(), 2, "the third number is page 2, got {}", p.page());
    // The last slot is the next arrow.
    press_at(&mut p, Point { x: w - each / 2.0, y: 10.0 });
    assert_eq!(p.page(), 3, "the last slot should be `next`, got page {}", p.page());
    // **And the first slot is the previous arrow, not a page.** Pressed
    // from page 3 it must step back to 2 — a slot that was a *page*
    // would jump to the window's first, which is 0 here and is the
    // difference this catches.
    press_at(&mut p, Point { x: each / 2.0, y: 10.0 });
    assert_eq!(
        p.page(), 2,
        "the first slot should be `previous` (3 -> 2), got page {}",
        p.page()
    );
}

#[test]
fn the_keyboard_steps_and_jumps() {
    let mut p = Pagination::new(10);
    key(&mut p, Key::End);
    assert_eq!(p.page(), 9);
    key(&mut p, Key::Home);
    assert_eq!(p.page(), 0);
    key(&mut p, Key::Right);
    assert_eq!(p.page(), 1);
}

// ---- Tree ------------------------------------------------------------

fn sample() -> Tree {
    Tree::new(&[
        Branch::new("src", 0),
        Branch::new("lib.rs", 1),
        Branch::new("widgets", 1),
        Branch::new("chart.rs", 2),
        Branch::new("trim.rs", 2),
        Branch::new("README", 0),
    ])
}

#[test]
fn every_row_is_visible_while_everything_is_expanded() {
    let t = sample();
    assert_eq!(t.visible(), 6, "{} of 6 rows visible with all expanded", t.visible());
}

/// **Collapsing hides the whole subtree, not one row.**
#[test]
fn collapsing_hides_everything_under_it() {
    let mut t = sample();
    assert!(t.toggle(2), "`widgets` has children and should toggle");
    assert_eq!(t.visible(), 4, "collapsing `widgets` should hide its two children");
    assert!(!t.is_visible(3) && !t.is_visible(4), "a child stayed visible");
    assert!(t.is_visible(5), "`README` is a sibling and must stay");
}

/// And collapsing a root hides a nested subtree too — two levels, which
/// a walk that checked only the immediate parent would miss.
#[test]
fn collapsing_a_root_hides_its_grandchildren() {
    let mut t = sample();
    t.toggle(0);
    assert_eq!(t.visible(), 2, "only `src` and `README` should remain");
    assert!(!t.is_visible(3), "a grandchild stayed visible under a collapsed root");
}

/// **A leaf cannot be toggled**, which is the difference between a
/// chevron that does nothing and one that is not drawn.
#[test]
fn a_leaf_does_not_toggle() {
    let mut t = sample();
    assert!(!t.has_children(1), "`lib.rs` is a leaf");
    assert!(!t.toggle(1), "a leaf reported a toggle");
    assert_eq!(t.visible(), 6, "toggling a leaf changed what is visible");
}

/// A press lands on the row you can *see*, which is the arithmetic a
/// collapse is most likely to break.
#[test]
fn a_press_hits_the_visible_row_not_the_raw_index() {
    let mut t = sample();
    t.toggle(2); // hide the two children of `widgets`
    lay(&mut t);
    // The fourth visible row is now `README` (raw index 5).
    press_at(&mut t, Point { x: 40.0, y: 8.0 + 26.0 * 3.0 + 13.0 });
    // `README` is a leaf, so nothing toggles — but nothing else may
    // toggle either.
    assert_eq!(t.visible(), 4, "a press on a leaf changed the tree");
    // And a press on `widgets` (second visible row) re-expands it.
    press_at(&mut t, Point { x: 40.0, y: 8.0 + 26.0 * 2.0 + 13.0 });
    assert_eq!(t.visible(), 6, "pressing `widgets` did not expand it again");
}

/// **A press lands on the visible row, and this fixture can tell.**
///
/// The first version could not: after collapsing, the k-th visible row
/// and the k-th raw row were both leaves, so a press did nothing either
/// way and an injection reading the raw index left it green. Here the
/// two differ in *kind* — one is a parent, the other a leaf — so only
/// the right reading toggles anything.
#[test]
fn a_press_after_a_collapse_reads_the_visible_row_not_the_raw_one() {
    let mut t = Tree::new(&[
        Branch::new("src", 0),      // 0, parent
        Branch::new("lib.rs", 1),   // 1, leaf
        Branch::new("widgets", 1),  // 2, parent
        Branch::new("chart.rs", 2), // 3, leaf
        Branch::new("docs", 0),     // 4, parent
        Branch::new("book.md", 1),  // 5, leaf
    ]);
    t.toggle(0); // collapse `src`; visible is now [0, 4, 5]
    lay(&mut t);
    assert_eq!(
        t.visible(), 3,
        "setup: collapsing `src` should leave `src`, `docs` and `book.md`"
    );
    // The *second visible* row is `docs` (raw 4, a parent). The second
    // *raw* row is `lib.rs` (a leaf, and hidden).
    press_at(&mut t, Point { x: 40.0, y: 8.0 + 26.0 + 13.0 });
    assert!(
        !t.is_open(4),
        "pressing the second visible row did not collapse `docs` — it read the \
         raw index and hit a hidden leaf"
    );
    assert_eq!(t.visible(), 2, "collapsing `docs` should hide `book.md`");
}

#[test]
fn it_truncates_rather_than_refusing() {
    let many: Vec<Branch> = (0..MAX_TREE_NODES + 10).map(|_| Branch::new("x", 0)).collect();
    let t = Tree::new(&many);
    assert_eq!(t.count(), MAX_TREE_NODES);
}

// ---- DataGrid --------------------------------------------------------

#[derive(StaticTree, Default)]
#[widget(row, spacing = 8.0)]
struct Row {
    #[widget_id("cell")]
    cell: Label<String>,
}

fn bind(r: &mut Row, i: usize) {
    r.cell.set(format!("row {i}"));
}

#[test]
fn a_grid_is_two_table_nodes_whatever_its_rows() {
    assert_eq!(
        <DataGrid<Row> as Widget>::NODE_COUNT,
        2,
        "a grid over a virtualized `Repeat` is its own node and the repeater's"
    );
}

#[test]
fn the_rows_get_what_is_left_under_the_header() {
    let mut g = DataGrid::<Row>::new(&["Name", "Size"], 40, 24.0, bind);
    let st = lay(&mut g);
    let (grid, rows) = (st[0].size.h, st[1].size.h);
    assert!(grid > 0.0, "setup: the grid has no height");
    assert!(
        (grid - rows - 28.0).abs() < 0.01,
        "the rows are {rows} in a {grid} grid — the header takes 28"
    );
    assert!(
        st[1].pos.y - st[0].pos.y >= 28.0,
        "the rows start at {} and overlap the header",
        st[1].pos.y - st[0].pos.y
    );
}

/// **The grid owns no data.** Its rows come from the `Repeat`'s bind,
/// which is the application's — and the count is the application's too.
#[test]
fn the_grid_reports_its_columns_and_the_repeater_its_rows() {
    let g = DataGrid::<Row>::new(&["Name", "Size", "Modified"], 40, 24.0, bind);
    assert_eq!(g.columns(), 3);
    assert_eq!(g.column(1), "Size");
    assert_eq!(g.rows.count, 40, "the row count belongs to the repeater");
}

/// A long grid can be scrolled, which is M27/T27.4's fix reaching
/// through a wrapper: the rows must be given the grid's **resolved**
/// height, not the limits the grid was offered.
#[test]
fn a_grid_taller_than_its_box_can_scroll() {
    let mut g = DataGrid::<Row>::new(&["Name"], 60, 24.0, bind);
    let st = lay(&mut g);
    let viewport = st[1].size.h;
    assert!(
        g.rows.max_scroll(viewport) > 0.0,
        "60 rows of 24 in a {viewport} px body reported nothing to scroll — \
         the rows were measured against the limits, not against the grid"
    );
}

/// **The wheel has to *reach* the rows.**
///
/// `a_grid_taller_than_its_box_can_scroll` asserts a number the grid
/// computes; this walks the path a wheel actually takes — hit test,
/// then `on_event` on the node the hit names — because "the repeater
/// could scroll" and "a wheel over the grid scrolls it" are different
/// claims, and `just widget-drive` found the second one false while the
/// first was green (M27/T27.10).
#[test]
fn a_wheel_over_the_body_scrolls_it() {
    let mut g = DataGrid::<Row>::new(&["Name"], 60, 24.0, bind);
    let st = lay(&mut g);
    let (x, y) = (200.0, st[1].pos.y + st[1].size.h / 2.0);
    let h = Node::hit(&g, &st, 0, x, y).expect("a wheel over the body hit nothing");
    assert_eq!(h.node, 1, "the body's hit named node {}, not the rows", h.node);
    let before = g.rows.first_row();
    let ev = Event::Scroll { x, y, dy: -120.0 };
    let took = g.node_mut(h.node as usize).on_event(&ev, &EventCtx::for_event(h.node, ev));
    assert!(
        took,
        "the rows declined a wheel with {} px of scroll available",
        g.rows.max_scroll(st[1].size.h)
    );
    let st = lay(&mut g);
    assert!(
        g.rows.first_row() > before,
        "first visible row {} -> {} after a 120 px wheel over a {} px body",
        before,
        g.rows.first_row(),
        st[1].size.h
    );
}
