//! **A row whose bind wrote new text is laid out again** — M57/A5
//! (`docs/plan-widget-fixes.md`).
//!
//! A `Repeat`'s rows are not in the table: each is laid out at its own
//! local index `0`. The derive's clean-subtree skip asked the frame's
//! dirty set — which is indexed by the *table* — whether that row's
//! range was clean, and so was really asking about the main table's
//! first few nodes. They are nearly always clean, so a row the bind had
//! just rewritten kept last frame's size. The audit met it in a
//! scrolled `DataGrid`: "11 KB" drawn in the 37 px box "1 KB" had been
//! given (`docs/widget-audit.md` X3).
//!
//! **Why three labels in front of the list.** The row's local range is
//! `0..2`; the list itself is marked when it scrolls. If the list sat at
//! index 1, its own mark would intersect `0..2` by coincidence and the
//! row would be laid out for the wrong reason — so the list is pushed
//! past the row's range, which is where a real screen's repeaters are.

use fulgurite::invalidate::{Boundaries, Invalidation};
use fulgurite::repeat::Repeat;
use fulgurite::style::{cascade_deep, Length, Patch, Rule, Selector};
use fulgurite::widget::Node;
use fulgurite::{Constraints, EventCtx, LayoutCx, NodeState, Point, StaticTree, Style, Widget};
use fulgurite_widgets::prelude::*;

const ROW_H: f32 = 24.0;
const ROWS: usize = 40;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Row {
    #[widget_id("cell")]
    cell: Label<String>,
}

/// Text whose length depends on the row, so a scroll changes every
/// visible row's width.
fn text_of(i: usize) -> String {
    "x".repeat(1 + i % 7)
}

fn bind(r: &mut Row, i: usize) {
    r.cell.set(text_of(i));
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    a: Label,
    b: Label,
    c: Label,
    #[widget_id("list")]
    list: Repeat<Row>,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 300.0, min_h: 0.0, max_h: 600.0 };

fn list_index() -> usize {
    fulgurite::style::index_of_wid(Ui::SUB_TABLE, "list").unwrap()
}

fn frame(ui: &mut Ui, styles: &[Style], st: &mut [NodeState], inval: &Boundaries<16>) {
    let mut m = fulgurite::cx::BlockMetrics;
    let mut cx = LayoutCx::with_changed(&mut m, inval.view());
    Node::layout(ui, &mut cx, VIEW, styles, st, 0);
    Node::place(ui, Point { x: 0.0, y: 0.0 }, styles, st, 0);
}

#[test]
fn a_scrolled_row_is_laid_out_for_its_new_text() {
    let mut ui = Ui { list: Repeat::new(ROWS, ROW_H, bind), ..Default::default() };
    let sheet = [Rule::new(Selector::Id("list"), Patch::new().width(Length::Fixed(300.0)).height(Length::Fixed(240.0)))];
    let mut styles = vec![Style::default(); Ui::NODE_COUNT];
    cascade_deep(&mut ui, &sheet, &mut styles);
    let mut st = vec![NodeState::default(); Ui::NODE_COUNT];
    let li = list_index();
    assert!(li >= Row::NODE_COUNT, "setup: the list at {li} overlaps a row's local range 0..{}", Row::NODE_COUNT);

    // Frame 1: everything laid out.
    let mut inval = Boundaries::<16>::default();
    frame(&mut ui, &styles, &mut st, &inval);

    // Scroll by three rows (a ring rebinds only the slots that changed,
    // so one row would leave one stale row to find), and mark only the list — what the host does
    // for a consumed wheel.
    let ev = fulgurite::Event::Scroll { x: 10.0, y: st[li].pos.y + 10.0, dy: -3.0 * ROW_H };
    assert!(Node::on_event(&mut ui.list, &ev, &EventCtx::for_event(li as u32, ev)), "setup: the wheel was not taken");
    inval.clear();
    inval.mark(li..li + 1);
    frame(&mut ui, &styles, &mut st, &inval);
    assert!(ui.list.first_row() > 0, "setup: the scroll did not move the window");

    let mut wrong = Vec::new();
    for k in 0..ui.list.window_len() {
        let row = ui.list.first_row() + k;
        // `k` is the offset into the visible window, not the row
        // number; template node 1 is the label (0 is the row's column).
        let s = ui.list.instance_state(k, 1);
        // BlockMetrics: 0.5 em per character at the label's size.
        let want = text_of(row).len() as f32 * 0.5 * s.font_size;
        if (s.size.w - want).abs() > 0.5 {
            wrong.push(format!("row {row}: text {:?} is {} wide, laid out {}", text_of(row), want, s.size.w));
        }
    }
    println!("{} visible rows checked; {} with a stale width", ui.list.window_len(), wrong.len());
    assert!(wrong.is_empty(), "rows kept last frame's layout:\n{}", wrong.join("\n"));
}
