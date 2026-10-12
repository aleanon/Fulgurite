//! **A scrolled repeater routes a press to the row that was pressed** —
//! found by M56 (`docs/plan-showcase-design.md` T56.2).
//!
//! `Repeat` keeps its materialized rows in a ring: scrolling by one row
//! re-binds one slot and moves the ring's start, so window position `k`
//! lives in slot `(first + k) % len`. Paint, hit, observe and `instance`
//! all went through that mapping; `Handlers::handler` indexed the ring
//! **raw**, so after any scroll a press on row `r` was answered by some
//! other row's handler.
//!
//! Nothing noticed because every repeater until M56 gave all its rows
//! the *same* handler — a `Select`'s options, a grid's rows — and one
//! row's copy answers exactly like another's. The showcase's sidebar is
//! the first repeater whose rows differ: a group heading has no handler,
//! so a press on an entry, routed to a heading's slot, did nothing at
//! all. Rows here get one of two handlers by parity, so which one answers
//! is observable.

use fulgurite::event::{Handlers, NodeSel};
use fulgurite::prelude::*;
use fulgurite::repeat::Repeat;
use fulgurite::style::{cascade_deep, Length, Patch, Rule, Selector};
use fulgurite::widget::Node;
use fulgurite::{Constraints, EventCtx, NodeState, Point};
use fulgurite_widgets::prelude::*;

const ROW_H: f32 = 30.0;
const ROWS: usize = 40;
const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 300.0, min_h: 0.0, max_h: 300.0 };

/// Which handler answered, and for which row it believed it was.
#[derive(Default)]
struct Log {
    even: Vec<u64>,
    odd: Vec<u64>,
}

type Row = Button<Log>;

fn bind(r: &mut Row, i: usize) {
    r.on_click = if i.is_multiple_of(2) {
        |s: &mut Log, cx: &EventCtx| s.even.push(cx.row.unwrap_or(u64::MAX))
    } else {
        |s: &mut Log, cx: &EventCtx| s.odd.push(cx.row.unwrap_or(u64::MAX))
    };
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("list")]
    list: Repeat<Row>,
}

fn relayout(ui: &mut Ui, styles: &[Style], st: &mut [NodeState]) {
    let mut m = fulgurite::cx::BlockMetrics;
    let mut cx = fulgurite::LayoutCx::new(&mut m);
    Node::layout(ui, &mut cx, VIEW, styles, st, 0);
    Node::place(ui, Point { x: 0.0, y: 0.0 }, styles, st, 0);
}

#[test]
fn a_scrolled_repeater_asks_the_pressed_rows_own_handler() {
    let mut ui = Ui { list: Repeat::new(ROWS, ROW_H, bind) };
    let sheet = [Rule::new(
        Selector::Id("list"),
        Patch::new().width(Length::Fixed(300.0)).height(Length::Fixed(300.0)),
    )];
    let mut styles = vec![Style::default(); Ui::NODE_COUNT];
    cascade_deep(&mut ui, &sheet, &mut styles);
    let mut st = vec![NodeState::default(); Ui::NODE_COUNT];
    relayout(&mut ui, &styles, &mut st);

    // Scroll by exactly one row, so the ring's start moves by one — the
    // case the raw index gets wrong for every row.
    let ev = fulgurite::Event::Scroll { x: 10.0, y: 10.0, dy: -ROW_H };
    assert!(Node::on_event(&mut ui.list, &ev, &EventCtx::for_event(1, ev)), "setup: the wheel did not scroll");
    relayout(&mut ui, &styles, &mut st);
    let (first, len) = (ui.list.first_row(), ui.list.window_len());
    println!("scrolled {} px: rows {first}..{} materialized", ui.list.scroll, first + len);
    assert_eq!(first, 1, "setup: one row of scroll did not move the window by one row");

    let mut wrong = Vec::new();
    for row in first..first + len {
        let mut log = Log::default();
        let h = Handlers::<Log>::handler(&ui.list, NodeSel { i: 0, inner: Some((row as u64, 0)) })
            .unwrap_or_else(|| panic!("row {row} has no handler at all"));
        let mut cx = EventCtx::for_event(1, fulgurite::Event::up(0.0, 0.0));
        cx.row = Some(row as u64);
        h(&mut log, &cx);
        let answered_even = !log.even.is_empty();
        if answered_even != row.is_multiple_of(2) {
            wrong.push(row);
        }
    }
    println!("{} of {len} rows answered by another row's handler: {wrong:?} (bound: 0)", wrong.len());
    assert!(wrong.is_empty(), "rows {wrong:?} were answered by another row's handler");
}
