//! **A data grid's rows hover and select, its header scrolls them, and
//! it shows a thumb and hairlines** — M57/B51 (`docs/plan-widget-fixes.md`,
//! `docs/widget-audit.md` DataGrid) — **and it is a sheet**: cell rules,
//! a row-number gutter, and an active cell a press or the arrows move.

use lumen_static::display::Cmd;
use lumen_static::{App as DeriveApp, Event, StaticTree, Widget};
use lumen_static_shell::Host;
use lumen_static_widgets::ops::Label;
use lumen_static_widgets::DataGrid;

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
    g.align[1] = lumen_static::style::Align::End;
    app.ui.g = g;
    let mut h: Host<App> = Host::new(app, 400, 300);
    h.pump();
    h
}

fn grid(h: &Host<App>) -> lumen_static::NodeState {
    h.states[lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "g").unwrap()]
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
    h.post(Event::PointerDown { x, y, button: lumen_static::PointerButton::Left, clicks: 1 });
    h.pump();
    h.post(Event::PointerUp { x, y, button: lumen_static::PointerButton::Left });
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
    let rows = h.states[lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "g").unwrap() + 1];
    let thumbs: Vec<[f32; 4]> = h.list.cmds.iter().filter_map(|c| match c {
        Cmd::RoundedRect { rect, .. } if (rect[2] - rect[0] - 4.0).abs() < 0.01 => Some(*rect),
        _ => None,
    }).collect();
    let rows_right = rows.pos.x + rows.size.w;
    println!("rows end at {rows_right}; thumbs {thumbs:?}");
    assert_eq!(thumbs.len(), 1, "no thumb while 100 rows overflow");
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
    let mut m = lumen_static::cx::BlockMetrics;
    let w = lumen_static::cx::TextMeasure::measure_weighted(&mut m, "Size", 13.0, 600.0, f32::INFINITY).w;
    println!("'Size' spans {size_end}..{}, its column ends at {col_end}", size_end + w);
    assert!((size_end + w - col_end).abs() < 0.5, "an End header does not end with its column");
}

fn click(h: &mut Host<App>, x: f32, y: f32) {
    h.post(Event::PointerDown { x, y, button: lumen_static::PointerButton::Left, clicks: 1 });
    h.pump();
    h.post(Event::PointerUp { x, y, button: lumen_static::PointerButton::Left });
    h.pump();
}

fn key(h: &mut Host<App>, k: lumen_static::event::Key, mods: lumen_static::event::Mods) {
    h.input.mods = mods;
    h.post(Event::Key { key: k, down: true });
    h.post(Event::Key { key: k, down: false });
    h.pump();
    h.input.mods = lumen_static::event::Mods::NONE;
}

fn rows(h: &Host<App>) -> lumen_static::NodeState {
    h.states[lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "g").unwrap() + 1]
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
    let g = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "g").unwrap() as u32;
    assert_eq!(focus, Some(g), "a press on a cell left the grid unfocused, so no key can reach it");
}

/// **The keyboard moves the active cell** — which D3 recorded it could
/// not reach at all (M57), and which a sheet is mostly used through.
/// Twenty Downs from row 3 leave the body, so the row must have scrolled
/// into view; Right past the last column stays on it; Ctrl+End is the
/// last row; each move reports a selection, as a press does.
#[test]
fn the_arrows_move_the_active_cell_and_keep_it_in_view() {
    use lumen_static::event::{Key, Mods};
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
