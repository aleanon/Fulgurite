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
/// **A press lands on the visible row, and this fixture can tell.**
///
/// The first version could not: after collapsing, the k-th visible row
/// and the k-th raw row were both leaves, so a press did nothing either
/// way and an injection reading the raw index left it green. Here the
/// two differ in *kind* — one is a parent, the other a leaf — so only
/// the right reading toggles anything.
///
/// A second test, a_press_hits_the_visible_row_not_the_raw_index, stood beside this
/// one until M27/T27.11's review and is gone: it pressed the *second*
/// visible row, which is the second raw row whether or not anything
/// below it was collapsed, so the raw-index injection left it green
/// too. Its other half — a press on a leaf changes nothing — is
/// `a_leaf_does_not_toggle` and the live drive's own leaf action. Two
/// tests, one of which cannot fail, is one test and a false sense of
/// two.
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
    let (grid, viewport) = (st[0].size.h, st[1].size.h);
    // **The viewport, against the grid — not the content against the
    // viewport.** This asserted `max_scroll > 0`, which its own doc
    // called "the rows get the grid's resolved height, not the limits it
    // was offered": 60 rows of 24 px is 1440, over both the grid (200)
    // and the offered limit (400), so the sign could not change and the
    // injection the doc named left it green (M27/T27.11, finding 9).
    assert!(
        (viewport - (grid - 28.0)).abs() < 0.5,
        "the rows resolved to {viewport} px inside a {grid} px grid with a \
         28 px header — they were measured against the limits they were \
         offered ({}), not against the box the grid resolved to",
        VIEW.max_h
    );
    assert!(
        g.rows.max_scroll(viewport) > 0.0,
        "60 rows of 24 in a {viewport} px body reported nothing to scroll"
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

// ---- what the T27.11 review found ------------------------------------

/// **A tree taller than its pane stays inside it** (M27/T27.11, finding 2).
///
/// The showcase seeds seven rows into a 200 px pane and they come to
/// 198 — so the over-full case, which is every real file tree, was
/// unreachable from the one fixture anybody looked at. With five more
/// rows the review measured 116 px of rows painted over the note below
/// the widget.
///
/// Asserted on the *commands*, not on a pixel: `Recording` keeps the
/// clip stack, so "nothing was drawn outside the box" is a question the
/// display list can answer exactly.
#[test]
fn a_tree_taller_than_its_pane_is_clipped_to_it() {
    use lumen_static::display::{Cmd, DisplayList};

    let mut rows = [Branch::default(); MAX_TREE_NODES];
    for (i, r) in rows.iter_mut().enumerate().take(14) {
        *r = Branch { label: LONG[i], depth: 0 };
    }
    let mut t = Tree::new(&rows[..14]);
    let styles = lumen_static::pass::default_styles(&t);
    let mut st = vec![NodeState::default(); <Tree as Widget>::NODE_COUNT];
    // A pane four rows tall, which is what a sidebar is.
    let pane = Constraints { min_w: 0.0, max_w: 260.0, min_h: 0.0, max_h: 104.0 };
    let mut m = lumen_static::cx::BlockMetrics;
    {
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(&mut t, &mut cx, pane, &styles, &mut st, 0);
    }
    Node::place(&mut t, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);
    let box_h = st[0].size.h;
    assert_eq!(box_h, 104.0, "setup: the pane did not clamp the tree");
    assert!(
        t.visible() as f32 * 26.0 > box_h,
        "setup: {} rows of 26 px do not overflow a {box_h} px pane — the \
         fixture cannot exhibit what this test is about",
        t.visible()
    );

    let mut list = DisplayList::new();
    Node::paint(&t, &st, 0, &mut list);
    // **The clip stack, walked.** `push_clip` records a command; it does
    // not trim the rects that follow — so "is it clipped" is a question
    // about what is *in force* when a run is recorded, and a test that
    // only looked at rects would pass with or without the clip.
    let mut stack: Vec<[f32; 4]> = Vec::new();
    let (mut below, mut escaped, mut lowest) = (0, 0, 0.0f32);
    for c in &list.cmds {
        match c {
            Cmd::PushClip { rect } => stack.push(*rect),
            Cmd::PopClip => {
                stack.pop();
            }
            Cmd::Text { rect, .. } => {
                if rect[1] < box_h {
                    continue;
                }
                below += 1;
                lowest = lowest.max(rect[1]);
                // Clipped away only if something in force ends above it.
                if !stack.iter().any(|c| c[3] <= rect[1]) {
                    escaped += 1;
                }
            }
            _ => {}
        }
    }
    assert!(below > 0, "setup: no row was drawn below the pane at all");
    assert_eq!(
        escaped, 0,
        "{escaped} of {below} text runs below the tree's own {box_h} px box \
         (the lowest at y {lowest}) are painted with no clip that excludes them"
    );
}

/// Fourteen labels, long enough to be visible if they escape.
const LONG: [&str; 14] = [
    "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel", "india", "juliett",
    "kilo", "lima", "mike", "november",
];

/// **The current page's number is inside its own pill** (M27/T27.11,
/// finding 1).
///
/// The pill was centred in the slot and the digit left-aligned in it, so
/// the glyph sat 3 px outside the pill — and, being painted in the
/// pill's contrast colour, on the control's own background. The review
/// counted **zero** non-background pixels inside the pill: the selected
/// page read as an empty lozenge.
///
/// Asserted on the commands, where "inside" is exact.
#[test]
fn the_current_page_number_is_inside_its_pill() {
    use lumen_static::display::{Cmd, DisplayList};

    let mut p = Pagination::new(30);
    assert!(p.go(4), "setup: the pagination did not move off page 0");
    let styles = lumen_static::pass::default_styles(&p);
    let mut st = vec![NodeState::default(); <Pagination as Widget>::NODE_COUNT];
    let mut m = lumen_static::cx::BlockMetrics;
    {
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(&mut p, &mut cx, VIEW, &styles, &mut st, 0);
    }
    Node::place(&mut p, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);

    let mut list = DisplayList::new();
    Node::paint(&p, &st, 0, &mut list);
    // The pill is the only rounded rect in the control's foreground
    // colour; "5" is the run whose text is "5".
    let fg = styles[0].color;
    let pill = list
        .cmds
        .iter()
        .find_map(|c| match c {
            Cmd::RoundedRect { rect, color, .. } if *color == fg => Some(*rect),
            _ => None,
        })
        .expect("no selection pill was painted");
    let digit = list
        .cmds
        .iter()
        .find_map(|c| match c {
            Cmd::Text { rect, text, .. } if list.str_of(*text) == "5" => Some(*rect),
            _ => None,
        })
        .expect("the current page's number was not painted at all");
    println!("pill x {:.1}..{:.1}, digit x {:.1}..{:.1}", pill[0], pill[2], digit[0], digit[2]);
    assert!(
        digit[0] >= pill[0] && digit[2] <= pill[2],
        "the digit's x {:.1}..{:.1} is not inside the pill's {:.1}..{:.1}; it is \
         painted in the pill's contrast colour, so anything outside is invisible",
        digit[0],
        digit[2],
        pill[0],
        pill[2]
    );
    // And centred, not merely inside: a digit shoved against one edge
    // would satisfy the bound above and still look wrong.
    let slack = ((digit[0] - pill[0]) - (pill[2] - digit[2])).abs();
    assert!(
        slack < 1.0,
        "the digit sits {:.1} px off the pill's centre",
        slack
    );
}

/// A page number past the old table's end is still its own number.
#[test]
fn a_page_number_is_not_clamped_to_a_table() {
    for (n, want) in [(1usize, "1"), (9, "9"), (33, "33"), (34, "34"), (1234, "1234")] {
        assert_eq!(
            lumen_static_widgets::PageLabel::new(n).as_str(),
            want,
            "page {n} rendered wrong"
        );
    }
}

/// A row built the way `DataGrid::column_width` documents: two cells
/// that each take their column. A derived container is `FILL` across
/// its main axis, so the row divides evenly for itself.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Cell {
    #[widget_id("cell-label")]
    l: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Wide {
    #[widget_id("wide-name")]
    name: Cell,
    #[widget_id("wide-size")]
    size: Cell,
}

fn bind_wide(r: &mut Wide, i: usize) {
    r.name.l.set(format!("file-{i:03}.rs"));
    r.size.l.set(format!("{} KB", i + 1));
}

/// **The header sits over the rows, not beside them** (M27/T27.11,
/// finding 4).
///
/// The grid draws its header at `k * column_width + CELL_INSET`; a row
/// that follows the same rule lines up, and this is the assertion that
/// says so — column by column, on the painted text, not on the
/// arithmetic that produced it.
#[test]
fn the_header_sits_over_a_row_that_follows_the_column_rule() {
    use lumen_static::display::{Cmd, DisplayList};

    let mut g = DataGrid::<Wide>::new(&["Name", "Size"], 40, 24.0, bind_wide);
    let each = g.column_width(320.0);
    assert_eq!(
        each, 152.0,
        "setup: two columns of a 320 px grid inset by {} on each side",
        DataGrid::<Wide>::CELL_INSET
    );
    let styles = lumen_static::pass::default_styles(&g);
    let mut st = vec![NodeState::default(); <DataGrid<Wide> as Widget>::NODE_COUNT];
    let grid = Constraints { min_w: 320.0, max_w: 320.0, min_h: 0.0, max_h: 220.0 };
    let mut m = lumen_static::cx::BlockMetrics;
    {
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(&mut g, &mut cx, grid, &styles, &mut st, 0);
    }
    Node::place(&mut g, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);

    let mut list = DisplayList::new();
    Node::paint(&g, &st, 0, &mut list);
    let x_of = |want: &str| {
        list.cmds.iter().find_map(|c| match c {
            Cmd::Text { rect, text, .. } if list.str_of(*text) == want => Some(rect[0]),
            _ => None,
        })
    };
    let pairs = [("Name", "file-000.rs"), ("Size", "1 KB")];
    for (head, cell) in pairs {
        let h = x_of(head).unwrap_or_else(|| panic!("no header {head:?} was painted"));
        let c = x_of(cell).unwrap_or_else(|| panic!("no cell {cell:?} was painted"));
        println!("{head}: header x {h:.1}, cell x {c:.1}");
        assert!(
            (h - c).abs() < 0.5,
            "the {head:?} header is at x {h:.1} and its first value at x {c:.1} — \
             {:.1} px apart, so the column reads beside its heading",
            (h - c).abs()
        );
    }
}

/// **The pointer's row is reachable without pressing anything**
/// (M27/T27.11, finding 3).
///
/// `Tree::on_event` tracked a hot row from `PointerMove` — an event a
/// widget receives only under pointer capture, which is granted only to
/// a node that consumed a `PointerDown`. So the highlight could appear
/// only while the button was still held after a fold, and two
/// screenshots of a hovered tree were byte-identical. The move is now
/// re-asked as `Event::Hover`, and this is the test that says so.
#[test]
fn a_hover_lights_the_row_under_the_pointer() {
    let mut t = sample();
    let st = lay(&mut t);
    let row_y = |k: usize| 8.0 + 26.0 * k as f32 + 13.0;

    // A move is *not* delivered: that is `Slider`'s contract and the
    // reason this is a second event rather than a flag.
    let mv = Event::PointerMove { x: 40.0, y: row_y(2) };
    assert!(
        !Node::on_event(&mut t, &mv, &EventCtx::for_event(0, mv)),
        "a bare PointerMove reached the tree; it must not — see Event::Hover"
    );

    let mut lit = |k: usize| {
        let ev = Event::Hover { x: 40.0, y: row_y(k) };
        let mut cx = EventCtx::for_event(0, ev);
        cx.local = Point { x: 40.0, y: row_y(k) };
        let moved = Node::on_event(&mut t, &ev, &cx);
        (moved, t.hot())
    };
    let (moved, hot) = lit(2);
    assert!(moved, "the first hover reported nothing changed");
    assert_eq!(hot, Some(2), "the hover lit row {hot:?}, not the one under it");
    let (moved, hot) = lit(4);
    assert!(moved, "moving to another row reported nothing changed");
    assert_eq!(hot, Some(4), "the highlight did not follow to row 4");
    // And a second hover on the same row is not a change: a tree that
    // reported one on every move would repaint the window on every
    // pointer sample.
    let (moved, _) = lit(4);
    assert!(!moved, "hovering the same row again reported a change");
    let _ = st;
}
