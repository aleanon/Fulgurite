//! **A data grid's rows hover and select, its header scrolls them, and
//! it shows a thumb and hairlines** — M57/B51 (`docs/plan-widget-fixes.md`,
//! `docs/widget-audit.md` DataGrid).

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
    // "Size" ends where its column does.
    let each = h.app.ui.g.column_width(g.size.w);
    let col_end = g.pos.x + 8.0 + each * 2.0;
    let size_end = h.list.cmds.iter().find_map(|c| match c {
        Cmd::Text { rect, .. } if rect[1] < g.pos.y + 28.0 && rect[0] > g.pos.x + each => Some(rect[0]),
        _ => None,
    }).unwrap();
    let mut m = lumen_static::cx::BlockMetrics;
    let w = lumen_static::cx::TextMeasure::measure_weighted(&mut m, "Size", 13.0, 600.0, f32::INFINITY).w;
    println!("'Size' spans {size_end}..{}, its column ends at {col_end}", size_end + w);
    assert!((size_end + w - col_end).abs() < 0.5, "an End header does not end with its column");
}
