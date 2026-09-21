//! M24/T24.2 — two things a repeater has to do that nothing checked.
//!
//! Both were found by *looking at a rendered menu*, not by a test:
//! eighteen rows laid out 932 px wide inside a 300 px panel, on no
//! background at all. Injecting each fault back afterwards left the
//! whole suite green, which is a finding about the suite — `Repeat` had
//! tests for its window, its memo and its clipping, and none for the box
//! its rows go in or for its own fill.

use lumen_static::display::{Cmd, Color, DisplayList};
use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;
use lumen_static::repeat::Repeat;
use lumen_static::style::{cascade_deep, Length, Patch, Rule, Selector, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, NodeState, Point};

/// Deliberately much wider than the repeater is styled to be: the bug
/// is that rows took *this* width instead of the repeater's.
const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 960.0, min_h: 0.0, max_h: 400.0 };
const PANEL_W: f32 = 300.0;
const ROW_H: f32 = 20.0;
const PANEL_BG: Color = Color(0x22, 0x33, 0x44, 0xff);

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Row {
    #[widget_id("cell")]
    l: Label<String>,
}

fn bind(r: &mut Row, i: usize) {
    r.l.set(format!("row {i}"));
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("list")]
    list: Repeat<Row>,
}

fn laid_out() -> (Ui, Vec<Style>, Vec<NodeState>, DisplayList) {
    let mut ui = Ui { list: Repeat::new(18, ROW_H, bind) };
    let sheet = [
        Rule::new(
            Selector::Id("list"),
            Patch::new()
                .width(Length::Fixed(PANEL_W))
                .height(Length::Fixed(200.0))
                .background(PANEL_BG)
                .radius(6.0),
        ),
        // A `FILL` row: it takes whatever width it is given, so its
        // measured width *is* the box the repeater handed it.
        Rule::new(Selector::Id("cell"), Patch::new().width(Length::FILL).height(Length::Fixed(ROW_H))),
    ];
    let mut styles = vec![Style::default(); Ui::NODE_COUNT];
    cascade_deep(&mut ui, &sheet, &mut styles);
    let mut st = vec![NodeState::default(); Ui::NODE_COUNT];
    let mut m = lumen_static::cx::BlockMetrics;
    let mut cx = lumen_static::LayoutCx::new(&mut m);
    Node::layout(&mut ui, &mut cx, VIEW, &styles, &mut st, 0);
    Node::place(&mut ui, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);
    let mut list = DisplayList::new();
    Node::paint(&ui, &st, 0, &mut list);
    (ui, styles, st, list)
}

/// **Rows go in the repeater's own box, not in the one it was offered.**
///
/// `Repeat::layout` measured rows against `limits.max_w` — the space its
/// *parent* had — and only then resolved its own width. A menu styled
/// 300 px wide inside a 960 px window laid every row out at 960, so each
/// label centred itself in a box three times the panel and sat outside
/// it. The width a repeater resolves to never depends on a row, so it
/// can be computed first, and now is.
#[test]
fn rows_are_laid_out_inside_the_repeaters_own_box() {
    let (_ui, _sty, st, list) = laid_out();
    let list_i = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "list").unwrap();
    let panel = st[list_i].size.w;
    assert_eq!(panel, PANEL_W, "setup: the repeater is not the width the sheet asked for");
    assert!(VIEW.max_w > panel * 2.0, "setup: the view must be much wider than the panel");

    // Every row's ink, from what was actually painted.
    let widest = list
        .cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::Text { rect, .. } => Some(rect[2] - rect[0]),
            _ => None,
        })
        .fold(0.0f32, f32::max);
    let rightmost = list
        .cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::Text { rect, .. } => Some(rect[2]),
            _ => None,
        })
        .fold(0.0f32, f32::max);
    println!(
        "panel {panel} px wide, view {} px; widest row {widest} px, rightmost edge {rightmost}",
        VIEW.max_w
    );
    assert!(widest <= panel + 0.5, "a row is {widest} px wide in a {panel} px panel");
    assert!(
        rightmost <= panel + 0.5,
        "a row reaches x={rightmost}, past the {panel} px panel"
    );
}

/// **A repeater paints its own fill**, which every other container does
/// — the derive, `ScrollView`, `Glass` — and this one did not. Two
/// halves had to be wrong for it to show: `paint` drew no rect, and
/// `layout` never called `record_painted`, so `st[i].bg` was never
/// filled in from the resolved style either. `paint` has no `&[Style]`,
/// so that array is the only route.
#[test]
fn a_repeater_paints_its_own_background() {
    let (_ui, _sty, st, list) = laid_out();
    let list_i = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "list").unwrap();
    let r = lumen_static::ops::rect_of(&st[list_i]);
    assert_eq!(st[list_i].bg, PANEL_BG, "the resolved background never reached NodeState");
    assert_eq!(st[list_i].radius, 6.0, "nor did the radius");

    let found = list.cmds.iter().any(|c| match c {
        Cmd::RoundedRect { rect, color, radius } => {
            *color == PANEL_BG && *radius == 6.0 && rect[0] == r[0] && rect[2] == r[2]
        }
        _ => false,
    });
    let fills = list
        .cmds
        .iter()
        .filter(|c| matches!(c, Cmd::RoundedRect { .. } | Cmd::Rect { .. }))
        .count();
    println!("repeater rect {r:?}, bg {:?}; {fills} fills in the list", st[list_i].bg);
    assert!(found, "the repeater's panel was never painted");
}
