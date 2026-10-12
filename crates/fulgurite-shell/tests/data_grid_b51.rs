//! **A data grid's rows hover and select, its header scrolls them, and
//! it shows a thumb and hairlines** — M57/B51 (`docs/plan-widget-fixes.md`,
//! `docs/widget-audit.md` DataGrid) — **and it is a sheet**: cell rules,
//! a row-number gutter, and an active cell a press or the arrows move.

use fulgurite::display::Cmd;
use fulgurite::{App as DeriveApp, Event, StaticTree, Widget};
use fulgurite_shell::Host;
use fulgurite_widgets::ops::Label;
use fulgurite_widgets::DataGrid;

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    #[widget_id("g")]
    g: DataGrid<Label<String>>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

fn bind(l: &mut Label<String>, i: usize) {
    l.set(format!("row {i}"));
}

fn host() -> Host<App> {
    let mut app = App::default();
    let mut g = DataGrid::new(&["Name", "Size"], 100, 24.0, bind);
    g.align[1] = fulgurite::style::Align::End;
    app.ui.g = g;
    let mut h: Host<App> = Host::new(app, 400, 300);
    h.pump();
    h
}

fn grid(h: &Host<App>) -> fulgurite::NodeState {
    h.states[fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "g").unwrap()]
}

#[test]
fn rows_hover_and_a_press_selects_once() {
    let mut h = host();
    let g = grid(&h);
    // Row 3: under the 28 px header, 24 px rows.
    let (x, y) = (g.pos.x + 60.0, g.pos.y + 28.0 + 24.0 * 3.5);
    h.post(Event::PointerMove { x, y });
    h.pump();
    let hot = h.app.ui.g.rows.hot();
    h.post(Event::PointerDown { x, y, button: fulgurite::PointerButton::Left, clicks: 1 });
    h.pump();
    h.post(Event::PointerUp { x, y, button: fulgurite::PointerButton::Left });
    h.pump();
    let picked = h.app.ui.g.take_selected();
    println!("hot {hot:?}; picked {picked:?}; selected {:?}", h.app.ui.g.selected());
    assert_eq!(hot, Some(3), "hover did not reach the row");
    assert_eq!(picked, Some(3), "a press did not select the row");
    assert_eq!(h.app.ui.g.take_selected(), None, "a selection was taken twice");
    assert_eq!(h.app.ui.g.selected(), Some(3));
}

#[test]
fn a_wheel_over_the_header_scrolls_the_rows() {
    let mut h = host();
    let g = grid(&h);
    h.post(Event::Scroll { x: g.pos.x + 60.0, y: g.pos.y + 10.0, dy: -48.0 });
    let handled = h.pump();
    println!("wheel over the header: handled {handled}, scroll {}", h.app.ui.g.rows.scroll);
    assert!(handled && h.app.ui.g.rows.scroll > 0.0, "a wheel over the header scrolled nothing");
}

#[test]
fn a_thumb_beside_the_rows_and_the_header_over_its_column() {
    let h = host();
    let g = grid(&h);
    let rows = h.states[fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "g").unwrap() + 1];
    // The scrollbar's track and thumb, each 6 px wide; the thumb is the
    // shorter.
    let bars: Vec<[f32; 4]> = h.list.cmds.iter().filter_map(|c| match c {
        Cmd::RoundedRect { rect, .. } if (rect[2] - rect[0] - 6.0).abs() < 0.01 => Some(*rect),
        _ => None,
    }).collect();
    let rows_right = rows.pos.x + rows.size.w;
    println!("rows end at {rows_right}; track and thumb {bars:?}");
    assert_eq!(bars.len(), 2, "no scrollbar while 100 rows overflow");
    let thumbs = [if bars[0][3] - bars[0][1] < bars[1][3] - bars[1][1] { bars[0] } else { bars[1] }];
    assert!(thumbs[0][0] >= rows_right, "the thumb is drawn over the rows");
    // Hairlines between the rows on show: a 172 px body of 24 px rows
    // has seven boundaries in it.
    let seps = h.list.cmds.iter().filter(|c| matches!(c, Cmd::Rect { rect, .. }
        if (rect[3] - rect[1] - 1.0).abs() < 0.01 && rect[1] > g.pos.y + 28.0)).count();
    println!("{seps} row separators");
    assert!(seps >= 6, "no hairlines between the rows ({seps})");
    // "Size" ends a cell's inset inside its column's right rule — the
    // columns start past the row-number gutter, where the rows' box does.
    let each = h.app.ui.g.column_width(g.size.w);
    let col_end = rows.pos.x + each * 2.0 - DataGrid::<Label<String>>::CELL_INSET;
    let size_end = h.list.cmds.iter().find_map(|c| match c {
        Cmd::Text { rect, .. } if rect[1] < g.pos.y + 28.0 && rect[0] > g.pos.x + each => Some(rect[0]),
        _ => None,
    }).unwrap();
    let mut m = fulgurite::cx::BlockMetrics;
    let w = fulgurite::cx::TextMeasure::measure_weighted(&mut m, "Size", 13.0, 600.0, f32::INFINITY).w;
    println!("'Size' spans {size_end}..{}, its column ends at {col_end}", size_end + w);
    assert!((size_end + w - col_end).abs() < 0.5, "an End header does not end with its column");
}

fn click(h: &mut Host<App>, x: f32, y: f32) {
    h.post(Event::PointerDown { x, y, button: fulgurite::PointerButton::Left, clicks: 1 });
    h.pump();
    h.post(Event::PointerUp { x, y, button: fulgurite::PointerButton::Left });
    h.pump();
}

fn key(h: &mut Host<App>, k: fulgurite::event::Key, mods: fulgurite::event::Mods) {
    h.input.mods = mods;
    h.post(Event::Key { key: k, down: true });
    h.post(Event::Key { key: k, down: false });
    h.pump();
    h.input.mods = fulgurite::event::Mods::NONE;
}

fn rows(h: &Host<App>) -> fulgurite::NodeState {
    h.states[fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "g").unwrap() + 1]
}

/// **A press makes the cell under it active, not just its row** — reported
/// from the live window: "the datagrid is just a list".
///
/// The press lands on the rows, which record the row and decline it; it
/// bubbles to the grid with the grid's own `local`, and the column is the
/// grid's arithmetic. Pressed in the *second* column of row 3, so neither
/// a grid that ignores the column (0) nor one that reads it from the
/// rows' coordinates instead of its own can agree by accident.
#[test]
fn a_press_makes_the_cell_under_it_active() {
    let mut h = host();
    let (r, each) = (rows(&h), h.app.ui.g.column_width(grid(&h).size.w));
    let (x, y) = (r.pos.x + each * 1.5, r.pos.y + 24.0 * 3.5);
    click(&mut h, x, y);
    let focus = h.input.focus;
    println!("pressed column 1, row 3 at ({x}, {y}): active {:?}, focus {focus:?}", h.app.ui.g.active());
    assert_eq!(h.app.ui.g.take_selected(), Some(3), "setup: the press did not select row 3");
    assert_eq!(h.app.ui.g.active(), Some((3, 1)), "the press did not make its cell active");
    let g = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "g").unwrap() as u32;
    assert_eq!(focus, Some(g), "a press on a cell left the grid unfocused, so no key can reach it");
}

/// **The keyboard moves the active cell** — which D3 recorded it could
/// not reach at all (M57), and which a sheet is mostly used through.
/// Twenty Downs from row 3 leave the body, so the row must have scrolled
/// into view; Right past the last column stays on it; Ctrl+End is the
/// last row; each move reports a selection, as a press does.
#[test]
fn the_arrows_move_the_active_cell_and_keep_it_in_view() {
    use fulgurite::event::{Key, Mods};
    let mut h = host();
    let (r, each) = (rows(&h), h.app.ui.g.column_width(grid(&h).size.w));
    click(&mut h, r.pos.x + each * 0.5, r.pos.y + 24.0 * 3.5);
    h.app.ui.g.take_selected();
    key(&mut h, Key::Right, Mods::NONE);
    key(&mut h, Key::Right, Mods::NONE);
    let after_right = h.app.ui.g.active();
    for _ in 0..20 {
        key(&mut h, Key::Down, Mods::NONE);
    }
    let (active, scroll, body) = (h.app.ui.g.active(), h.app.ui.g.rows.scroll, rows(&h).size.h);
    let reported = h.app.ui.g.take_selected();
    let top = 23.0 * 24.0 - scroll;
    println!("Right, Right: {after_right:?}; 20 Downs: {active:?}, scroll {scroll}, row 23 at {top}..{} of a {body} px body", top + 24.0);
    assert_eq!(after_right, Some((3, 1)), "Right moved past the last column, or not at all");
    assert_eq!(active, Some((23, 1)), "Down did not move the active cell's row");
    assert_eq!(reported, Some(23), "a key's selection was not reported as a press's is");
    assert!(top >= -0.01 && top + 24.0 <= body + 0.01, "row 23 is at {top} in a {body} px body: not scrolled into view");
    key(&mut h, Key::End, Mods::CTRL);
    println!("Ctrl+End: {:?}", h.app.ui.g.active());
    assert_eq!(h.app.ui.g.active(), Some((99, 1)), "Ctrl+End is not the last row");
}

/// **It draws cells**: a rule at every column boundary, through the header
/// and the body, and a row number beside each row.
#[test]
fn it_draws_column_rules_and_row_numbers() {
    let h = host();
    let (g, r) = (grid(&h), rows(&h));
    let each = h.app.ui.g.column_width(g.size.w);
    // A rule is one whole pixel ending on its boundary (X10: a hairline
    // on a half-pixel edge renders two pixels wide), so it is found by
    // its right edge.
    let rules: Vec<f32> = h.list.cmds.iter().filter_map(|c| match c {
        Cmd::Rect { rect, .. } if (rect[2] - rect[0] - 1.0).abs() < 0.01 && rect[0].fract() == 0.0
            && rect[1] <= g.pos.y + 0.01 && rect[3] >= g.pos.y + g.size.h - 0.01 => Some(rect[2]),
        _ => None,
    }).collect();
    let want: Vec<f32> = (0..=2).map(|k| (r.pos.x + each * k as f32).round()).collect();
    let numbers: Vec<String> = h.list.cmds.iter().filter_map(|c| match c {
        Cmd::Text { rect, text, .. } if rect[2] <= r.pos.x + 0.01 => Some(h.list.str_of(*text).to_string()),
        _ => None,
    }).collect();
    println!("column rules at {rules:?} (want {want:?}); gutter numbers {numbers:?}");
    for x in &want {
        assert!(rules.iter().any(|r| (r - x).abs() < 0.5), "no rule through the sheet at x {x}: {rules:?}");
    }
    assert!(numbers.len() >= 6 && numbers[0] == "1" && numbers[1] == "2", "the gutter does not number the rows: {numbers:?}");
}

/// The thumb's box as drawn: the shorter of the scrollbar's two 6 px bars.
fn thumb_rect(h: &Host<App>) -> [f32; 4] {
    let bars: Vec<[f32; 4]> = h.list.cmds.iter().filter_map(|c| match c {
        Cmd::RoundedRect { rect, .. } if (rect[2] - rect[0] - 6.0).abs() < 0.01 => Some(*rect),
        _ => None,
    }).collect();
    assert_eq!(bars.len(), 2, "setup: no scrollbar drawn");
    if bars[0][3] - bars[0][1] < bars[1][3] - bars[1][1] { bars[0] } else { bars[1] }
}

/// **The thumb drags** — reported from the live window: the scrollbar
/// could be neither dragged nor clicked. It was drawn and nothing took a
/// press on it.
///
/// Grabbed at its middle, as drawn, and dragged 40 px: the rows scroll by
/// the drag times the content's share of the track, the release ends the
/// drag, and a move after it scrolls nothing — a drag that outlived its
/// release is the combobox's capture defect in a scrollbar.
#[test]
fn the_thumb_drags_the_rows_and_lets_go() {
    let mut h = host();
    let t = thumb_rect(&h);
    let (x, y) = ((t[0] + t[2]) / 2.0, (t[1] + t[3]) / 2.0);
    h.post(Event::PointerDown { x, y, button: fulgurite::PointerButton::Left, clicks: 1 });
    let took = h.pump();
    h.post(Event::PointerMove { x: x + 30.0, y: y + 40.0 });
    h.pump();
    let dragged = h.app.ui.g.rows.scroll;
    h.post(Event::PointerUp { x: x + 30.0, y: y + 40.0, button: fulgurite::PointerButton::Left });
    h.pump();
    // **Then a press on a cell, moved over the cells.** A move after the
    // release reaches the grid only under capture, so a drag that forgot
    // to end shows only once the grid holds the pointer again — which a
    // press on a cell gives it. The first version moved with nothing
    // pressed and stayed green with the release arm deleted.
    // In the second column: a press that changes the active cell is taken,
    // and one that does not is declined and captures nothing.
    let (r, each) = (rows(&h), h.app.ui.g.column_width(grid(&h).size.w));
    let (cx, cy) = (r.pos.x + each * 1.5, r.pos.y + 30.0);
    h.post(Event::PointerDown { x: cx, y: cy, button: fulgurite::PointerButton::Left, clicks: 1 });
    h.pump();
    let pressed_capture = h.input.capture;
    h.post(Event::PointerMove { x: cx + 10.0, y: cy + 60.0 });
    h.pump();
    h.post(Event::PointerUp { x: cx + 10.0, y: cy + 60.0, button: fulgurite::PointerButton::Left });
    h.pump();
    let after = h.app.ui.g.rows.scroll;
    // The expected scroll, from the grid's own geometry: 40 px of a track
    // the thumb can travel, mapped onto the content the view can travel.
    let body = rows(&h).size.h;
    let (content, len) = (100.0 * 24.0, (t[3] - t[1]) + 4.0);
    let want = 40.0 / (body - len) * (content - body);
    println!("press on the thumb taken {took}; dragged 40 px -> scroll {dragged} (want {want}); a press and a move on the cells after it leave {after}; capture {:?}", h.input.capture);
    assert!(took, "the press on the thumb was not taken");
    assert!(pressed_capture.is_some(), "setup: the press on a cell did not give the grid the pointer, so no move could reach it");
    assert!((dragged - want).abs() < 1.0, "a 40 px drag scrolled {dragged}, not {want}");
    assert_eq!(after, dragged, "a press and a move on the cells after the release still dragged the rows");
    assert_eq!(h.input.capture, None, "the release left the grid holding the pointer");
}

/// **A press on the track moves the thumb there.**
#[test]
fn a_press_on_the_track_jumps_the_thumb_to_it() {
    let mut h = host();
    let (t, body) = (thumb_rect(&h), rows(&h));
    // Near the bottom of the track, well below the thumb.
    let (x, y) = ((t[0] + t[2]) / 2.0, body.pos.y + body.size.h - 10.0);
    click(&mut h, x, y);
    let t2 = thumb_rect(&h);
    let centre = (t2[1] + t2[3]) / 2.0;
    println!("thumb {}..{} -> {}..{} after a press at y {y}; scroll {}", t[1], t[3], t2[1], t2[3], h.app.ui.g.rows.scroll);
    assert!(h.app.ui.g.rows.scroll > 0.0, "a press on the track scrolled nothing");
    assert!((centre - y).abs() < 12.0 || t2[3] >= body.pos.y + body.size.h - 3.0, "the thumb did not move to the press: centred at {centre}, pressed at {y}");
}
