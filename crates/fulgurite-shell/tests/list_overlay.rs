//! **A popup inside a `List` draws and is hit in the overlay phase** —
//! the defect M85/T85.6 recorded (`docs/plan-layout-input.md`, "What
//! building found (M85)").
//!
//! A `Popup` defers its own index from inside its own paint. Inside an
//! arena — a `List` item, a `Repeat` row, a `Dyn` island — that index is
//! the item's, and the overlay pass repainted the **main-table** node
//! with that number (`pass::paint_overlaid`), so a `Select` inside a
//! `List` drew no menu at all, culled or not. A deferral now carries its
//! arena origin the way a `Hit` does — the owner's main-table index and
//! an `Inner { row, node }` — and phase 2, the overlay hit test and the
//! pass's dismissals resolve it through `Node::arena_node`.
//!
//! Four gates through a `Host`, so the overlay list is the one its own
//! paint produced, and a fifth on the tables:
//!
//! - an open `Select` as the second item of a `List` paints its menu
//!   panel, counted in panel-coloured pixels inside the menu's rect;
//! - the same `Select` in a derived column still does (the accept
//!   direction: the main-table path is unchanged);
//! - a press on a button inside an open popup in **row 1** of a `List`
//!   is handled and runs that button's handler with `ctx.row == 1`, and
//!   a press outside every overlay closes the popup;
//! - the same popup in a `Repeat` scrolled by one row, so its ring has
//!   rotated, is reached by its *data* row (`ctx.row == 3`);
//! - every widget that defers carries `NodeDesc::DEFERS`, which `List`
//!   and `Repeat` read as a `const` before they track an item's origin.
//!
//! Each prints the rects and the counts beside their bounds.

use fulgurite::hatch::List;
use fulgurite::prelude::*;
use fulgurite::style::{Length, Patch, Rule, Selector};
use fulgurite::widget::Node;
use fulgurite::{Event, EventCtx, Renderer};
use fulgurite_shell::software::Canvas;
use fulgurite_shell::{Host, CLEAR};
use fulgurite_widgets::prelude::*;
use fulgurite_widgets::select::Row;
use fulgurite_widgets::ops::Fixed;
use fulgurite_widgets::{Popup, Select};

const SIDE: u32 = 320;
const PANEL: fulgurite::display::Color = fulgurite::display::Color(40, 60, 120, 255);

type Rect = [f32; 4];

// ---- a Select, in a List and in a column ----------------------------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct ListUi {
    #[widget_id("list")]
    list: List<Select<ListApp>>,
    /// Main-table nodes past the list, so the item-local index the
    /// popup defers names *some* main-table node and the defect draws
    /// the wrong thing rather than indexing out of the table. (Without
    /// them the old pass panicked "node index out of range".)
    #[widget_id("pad")]
    pad: Pad,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Pad {
    a: Fixed,
    b: Fixed,
    c: Fixed,
    d: Fixed,
    e: Fixed,
    f: Fixed,
    g: Fixed,
    h: Fixed,
}

#[derive(DeriveApp, Default)]
struct ListApp {
    #[tree]
    ui: ListUi,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct ColUi {
    #[widget_id("first")]
    first: Select<ColApp>,
    #[widget_id("sel")]
    sel: Select<ColApp>,
}

#[derive(DeriveApp, Default)]
struct ColApp {
    #[tree]
    ui: ColUi,
}

const OPTS: [&str; 3] = ["Apple", "Banana", "Cherry"];

fn bind<S: 'static>(r: &mut Row<S>, i: usize) {
    r.label.clear();
    r.label.push_str(OPTS[i]);
}

fn select<S: 'static>(open: bool) -> Select<S> {
    let mut s = Select::new(OPTS.len(), 20.0, bind::<S>);
    if open {
        s.open();
    }
    s
}

fn sheet() -> [Rule; 2] {
    [
        Rule::new(Selector::Kind("button"), Patch::new().width(Length::Fixed(120.0)).height(Length::Fixed(28.0))),
        // An opaque menu panel taller than its rows, so the menu has ink a
        // render can count without a font. The panel's kind reaches a
        // `Repeat` inside a `List` item; its rows' would not (the nested
        // arena, `style.rs`'s `cascade_arenas`).
        Rule::new(Selector::Kind("repeat"), Patch::new().height(Length::Fixed(100.0)).background(PANEL)),
    ]
}

fn render<A: fulgurite::event::App>(h: &Host<A>) -> Vec<u8> {
    let mut c = Canvas::new(SIDE, SIDE);
    c.clear(CLEAR);
    c.render(&h.list, SIDE, SIDE);
    c.pixels
}

/// Pixels of the panel's colour inside `r`.
fn panel_pixels(px: &[u8], r: Rect) -> usize {
    let s = SIDE as f32;
    let (x0, y0) = (r[0].max(0.0) as usize, r[1].max(0.0) as usize);
    let (x1, y1) = (r[2].min(s) as usize, r[3].min(s) as usize);
    let mut n = 0;
    for y in y0..y1 {
        for x in x0..x1 {
            let p = (y * SIDE as usize + x) * 4;
            if px[p..p + 3] == [PANEL.0, PANEL.1, PANEL.2] {
                n += 1;
            }
        }
    }
    n
}

fn rect(s: fulgurite::NodeState) -> Rect {
    [s.pos.x, s.pos.y, s.pos.x + s.size.w, s.pos.y + s.size.h]
}

/// The popup's index inside a `Select`'s own table.
fn popup_in<S: 'static>(sel: &Select<S>) -> usize {
    (0..<Select<S> as Widget>::NODE_COUNT).find(|&j| sel.node(j).floats_children()).expect("a Select holds a popup")
}

#[test]
fn an_open_select_inside_a_list_draws_its_menu() {
    let mut app = ListApp::default();
    app.ui.list = List::new(vec![select(false), select(true)]);
    let mut h: Host<ListApp> = Host::new(app, SIDE, SIDE);
    h.set_stylesheet(&sheet());
    h.pump();
    h.pump();
    let list = &h.app.ui.list;
    let p = popup_in(&list.items[1]);
    let trigger = rect(list.item_state(1, 1));
    let menu = rect(list.item_state(1, p + 1));
    let list_rect = rect(h.states[fulgurite::style::index_of_wid(<ListUi as Widget>::SUB_TABLE, "list").unwrap()]);
    let px = render(&h);
    let n = panel_pixels(&px, menu);
    let area = ((menu[2] - menu[0]) * (menu[3] - menu[1])) as usize;
    println!(
        "List: trigger {trigger:?}, menu {menu:?}, list {list_rect:?}, open {}; panel pixels in the menu {n} of {area} (bound > {})",
        !list.items[1].menu.content_absent(),
        area / 4
    );
    assert!(!list.items[1].menu.content_absent(), "setup: the menu is closed");
    assert!(menu[3] - menu[1] >= 100.0 && menu[2] > menu[0], "setup: the menu panel has no area: {menu:?}");
    assert!(menu[3] > list_rect[3], "setup: the menu {menu:?} is inside the list's clip {list_rect:?}, so it would draw inline");
    assert!(
        n > area / 4,
        "a Select inside a List drew no menu: {n} panel pixels of {area} in {menu:?} -- the overlay pass repainted a main-table node with the item's index"
    );
}

#[test]
fn an_open_select_in_a_derived_column_still_draws_its_menu() {
    let mut app = ColApp::default();
    app.ui.first = select(false);
    app.ui.sel = select(true);
    let mut h: Host<ColApp> = Host::new(app, SIDE, SIDE);
    h.set_stylesheet(&sheet());
    h.pump();
    h.pump();
    let sel = fulgurite::style::index_of_wid(<ColUi as Widget>::SUB_TABLE, "sel").unwrap();
    let p = sel + popup_in(&h.app.ui.sel);
    let menu = rect(h.states[p + 1]);
    let px = render(&h);
    let n = panel_pixels(&px, menu);
    let area = ((menu[2] - menu[0]) * (menu[3] - menu[1])) as usize;
    println!("column: menu {menu:?}; panel pixels in the menu {n} of {area} (bound > {})", area / 4);
    assert!(menu[3] - menu[1] >= 100.0, "setup: the menu panel has no area: {menu:?}");
    assert!(n > area / 4, "a Select in a derived column drew no menu: {n} panel pixels of {area}");
}

// ---- a popup of buttons in a List item: hit and dismissal -----------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct Choices {
    #[widget_id("a")]
    a: Button<PickApp>,
    #[widget_id("b")]
    b: Button<PickApp>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Item {
    #[widget_id("open")]
    open: Button<PickApp>,
    #[widget_id("menu")]
    menu: Popup<Choices>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct PickUi {
    #[widget_id("items")]
    items: List<Item>,
    #[widget_id("below")]
    below: Button<PickApp>,
    /// Empty but for the scrolled-repeater case.
    #[widget_id("rows")]
    rows: fulgurite::repeat::Repeat<Item>,
    /// Main-table nodes past the item-local indices, as in `ListUi`.
    #[widget_id("pad")]
    pad: Pad,
}

#[derive(DeriveApp, Default)]
struct PickApp {
    #[tree]
    ui: PickUi,
    /// `(ctx.row, which button)` of the last choice.
    chosen: Option<(Option<u64>, char)>,
}

fn item(open: bool) -> Item {
    let mut it = Item::default();
    it.open.label = "open";
    it.menu.content.a.label = "a";
    it.menu.content.b.label = "b";
    it.menu.content.a.on_click = |s: &mut PickApp, c: &EventCtx| s.chosen = Some((c.row, 'a'));
    it.menu.content.b.on_click = |s: &mut PickApp, c: &EventCtx| s.chosen = Some((c.row, 'b'));
    it.menu.open = open;
    it
}

fn centre(r: Rect) -> (f32, f32) {
    ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0)
}

fn pick_host() -> Host<PickApp> {
    let mut app = PickApp::default();
    app.ui.items = List::new(vec![item(false), item(true), item(false)]);
    app.ui.below.label = "below";
    let mut h: Host<PickApp> = Host::new(app, SIDE, SIDE);
    h.set_stylesheet(&[Rule::new(
        Selector::Kind("button"),
        Patch::new().width(Length::Fixed(120.0)).height(Length::Fixed(28.0)),
    )]);
    h.pump();
    h.pump();
    h
}

#[test]
fn a_press_in_a_popup_inside_a_list_lands_on_its_row() {
    let mut h = pick_host();
    // Item table: 0 column, 1 open, 2 popup, 3 choices, 4 a, 5 b.
    let b = rect(h.app.ui.items.item_state(1, 5));
    let row2 = rect(h.app.ui.items.item_state(2, 1));
    let (x, y) = centre(b);
    println!("row 1's popup button b at {b:?}; row 2's trigger at {row2:?}; pressing ({x}, {y})");
    assert!(b[2] > b[0] && b[3] > b[1], "setup: row 1's popup button has no area: {b:?}");
    assert!(h.app.ui.items.items[1].menu.open, "setup: row 1's popup is closed");
    h.post(Event::down(x, y));
    let down = h.pump();
    h.post(Event::up(x, y));
    let up = h.pump();
    // A button acts on its release, so the press itself reports
    // unhandled here as it does on any button in the table.
    println!("handled down {down}, up {up} (expected up); chosen {:?} (expected Some((Some(1), 'b')))", h.app.chosen);
    assert!(up, "the release on row 1's popup button was not handled (down {down}, up {up})");
    assert_eq!(h.app.chosen, Some((Some(1), 'b')), "the press in row 1's popup ran the wrong handler, or none");
}

#[test]
fn a_press_outside_closes_a_popup_inside_a_list() {
    let mut h = pick_host();
    let below = rect(h.states[fulgurite::style::index_of_wid(<PickUi as Widget>::SUB_TABLE, "below").unwrap()]);
    let menu = rect(h.app.ui.items.item_state(1, 3));
    // Below every popup and every row: outside each overlay.
    let (x, y) = (SIDE as f32 - 4.0, SIDE as f32 - 4.0);
    println!("menu {menu:?}, below {below:?}; pressing ({x}, {y})");
    assert!(y > menu[3] && y > below[3], "setup: the press is inside something");
    h.post(Event::down(x, y));
    let down = h.pump();
    h.post(Event::up(x, y));
    h.pump();
    let open = h.app.ui.items.items[1].menu.open;
    println!("handled {down}; row 1's popup open afterwards: {open} (expected false); chosen {:?}", h.app.chosen);
    assert!(down, "the dismissing press was not consumed");
    assert!(!open, "a press outside every overlay left the popup in row 1 open");
    assert_eq!(h.app.chosen, None, "the dismissing press ran a handler");
}

// ---- the same popup in a scrolled Repeat's ring ---------------------

/// Row 3's popup is the open one; every row's buttons report their row.
fn bind_row(it: &mut Item, i: usize) {
    *it = item(i == 3);
}

/// `Repeat` keeps its rows in a ring and `hit` reports the *data* row,
/// so an overlay inside one is addressed by data row and resolved
/// through the ring (`Node::arena_node`, M44/T44.7's mapping). Scrolled
/// by one row, data row 3 is window position 2 in slot 3 — the case a
/// raw slot index gets wrong.
#[test]
fn a_popup_in_a_scrolled_repeater_lands_on_its_data_row() {
    // `PickUi`'s list is empty here; its repeater holds the rows.
    let mut app = PickApp::default();
    app.ui.rows = fulgurite::repeat::Repeat::new(12, 28.0, bind_row);
    let mut h: Host<PickApp> = Host::new(app, SIDE, SIDE);
    h.set_stylesheet(&[
        Rule::new(Selector::Kind("button"), Patch::new().width(Length::Fixed(120.0)).height(Length::Fixed(28.0))),
        Rule::new(Selector::Id("rows"), Patch::new().width(Length::Fixed(200.0)).height(Length::Fixed(200.0))),
    ]);
    h.pump();
    h.pump();
    h.post(Event::Scroll { x: 10.0, y: 60.0, dy: -28.0 });
    h.pump();
    h.pump();
    let rows = &h.app.ui.rows;
    let (first, len) = (rows.first_row(), rows.window_len());
    let k = 3 - first;
    let b = rect(rows.instance_state(k, 5));
    let open = rows.instance(k).menu.open;
    println!("scrolled: rows {first}..{} in a ring of {len}; data row 3 at window position {k}, its popup open {open}, button b at {b:?}", first + len);
    assert_eq!(first, 1, "setup: one row of scroll did not move the window by one row");
    assert!(open && b[2] > b[0] && b[3] > b[1], "setup: data row 3's popup is not open and laid out");
    // No pixel count here: row 5's own trigger lies under b inside the
    // repeater's clip, so ink in b proves nothing. Phase 2 and the hit
    // test resolve the entry through the same `pass::overlay_node`, so
    // the press below is what tells a ring mapped wrong.
    let (x, y) = centre(b);
    h.post(Event::down(x, y));
    h.pump();
    h.post(Event::up(x, y));
    let up = h.pump();
    println!("release handled {up}; chosen {:?} (expected Some((Some(3), 'b')))", h.app.chosen);
    assert!(up, "the release on data row 3's popup button was not handled");
    assert_eq!(h.app.chosen, Some((Some(3), 'b')), "the press in data row 3's popup ran another row's handler, or none");
}

/// **Every widget that defers says so in its table** — `NodeDesc::DEFERS`,
/// which `List` and `Repeat` read as a `const` to decide whether an item
/// is painted with its arena origin at all. A deferring widget without
/// the bit inside a `List` is the defect again; the gates above go red
/// when `Popup`'s entry loses it.
#[test]
fn every_deferring_widget_carries_the_defers_bit() {
    use fulgurite::widget::table_has_flag;
    use fulgurite::NodeDesc;
    use fulgurite_widgets::{Drawer, Modal, Sheet};
    let cases = [
        ("Popup", table_has_flag(<Popup<Fixed> as Widget>::SUB_TABLE, NodeDesc::DEFERS)),
        ("Select", table_has_flag(<Select<ListApp> as Widget>::SUB_TABLE, NodeDesc::DEFERS)),
        ("Modal", table_has_flag(<Modal<Fixed> as Widget>::SUB_TABLE, NodeDesc::DEFERS)),
        ("Sheet", table_has_flag(<Sheet<Fixed> as Widget>::SUB_TABLE, NodeDesc::DEFERS)),
        ("Drawer", table_has_flag(<Drawer<Fixed> as Widget>::SUB_TABLE, NodeDesc::DEFERS)),
        // The accept direction: a widget that never defers does not carry it.
        ("Button (expected false)", !table_has_flag(<Button<ListApp> as Widget>::SUB_TABLE, NodeDesc::DEFERS)),
    ];
    println!("{cases:?}");
    let missing: Vec<_> = cases.iter().filter(|c| !c.1).map(|c| c.0).collect();
    assert!(missing.is_empty(), "these deferring widgets' tables lack NodeDesc::DEFERS (or a non-deferring one has it): {missing:?}");
}
