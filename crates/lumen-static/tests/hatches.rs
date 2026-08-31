//! M4: §6's escape hatches. Each gets a firing and a quiet case, and
//! `NODE_COUNT` stays a compile-time constant in every one.

use lumen_static::hatch::{Either, List, Side};
use lumen_static::ops::{Button, Fixed, Label};
use lumen_static::{pass, Constraints, EventCtx, NodeState, StaticTree, Widget};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 400.0 };

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Row {
    icon: Fixed,
    name: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Panel {
    head: Label,
    body: Label,
}

#[test]
fn option_keeps_node_count_constant_and_indices_stable() {
    // The §6 property: the table is the same whether the branch is
    // present or not, so ids, provenance and agent selectors survive a
    // toggle — the thing a key-based reconciler has to work for.
    const _: () = assert!(<Option<Panel> as Widget>::NODE_COUNT == Panel::NODE_COUNT);
    assert_eq!(<Option<Panel> as Widget>::SUB_TABLE.len(), Panel::SUB_TABLE.len());

    let mut some: Option<Panel> = Some(Panel::default());
    some.as_mut().unwrap().head.set("shown");
    let mut st = [NodeState::default(); Panel::NODE_COUNT];
    let sty = pass::default_styles(&some);
    pass::layout(&mut some, &sty, &mut st, VIEW);
    assert!(st[0].size.h > 0.0, "present branch occupies space");

    let mut none: Option<Panel> = None;
    let mut st2 = [NodeState::default(); Panel::NODE_COUNT];
    pass::layout(&mut none, &sty, &mut st2, VIEW);
    assert_eq!(st2[0].size, lumen_static::Size::default(), "absent takes no space");
    assert!(pass::hit_test(&none, &st2, 1.0, 1.0).is_none(), "and cannot be hit");
}

#[test]
fn either_holds_both_branches_so_toggling_keeps_state() {
    let mut e: Either<Panel, Row> = Either::default();
    e.a.head.set("panel head");
    e.b.icon.w = 77.0;

    let mut st = [NodeState::default(); 1];
    let sty = pass::default_styles(&e);

    pass::layout(&mut e, &sty, &mut st, VIEW);
    assert_eq!(e.showing(), Side::A);
    let a_h = st[0].size.h;

    e.show(Side::B);
    pass::layout(&mut e, &sty, &mut st, VIEW);
    assert_ne!(st[0].size.h, a_h, "the other branch has its own size");

    // Toggle back: the A branch still has everything it had. This is
    // §6's "memory for both branches is a win" made concrete.
    e.show(Side::A);
    pass::layout(&mut e, &sty, &mut st, VIEW);
    assert_eq!(e.a.head.as_str(), "panel head");
    assert_eq!(e.b.icon.w, 77.0, "the hidden branch kept its state");
    assert_eq!(st[0].size.h, a_h);

    // One table node regardless of branch count (the recorded deviation
    // from §6's A+B, and an improvement on it).
    const _: () = assert!(<Either<Panel, Row> as Widget>::NODE_COUNT == 1);
}

#[test]
fn list_arity_comes_from_the_data_but_node_count_does_not() {
    const _: () = assert!(<List<Row> as Widget>::NODE_COUNT == 1);

    let mut l: List<Row> = List::new((0..7).map(|_| Row::default()).collect());
    for (k, it) in l.items.iter_mut().enumerate() {
        it.icon.w = k as f32 + 1.0;
        it.icon.h = 10.0;
    }
    let mut st = [NodeState::default(); 1];
    let sty = pass::default_styles(&l);
    pass::layout(&mut l, &sty, &mut st, VIEW);

    assert_eq!(l.len(), 7);
    assert_eq!(l.item_state(3, 1).size.w, 4.0, "each item laid out from its own data");
    // Items stack, so the list is as tall as their sum. A row is as tall
    // as its tallest child, and the (empty) label is 16px, not the 10px
    // icon — so 7 * 16.
    assert_eq!(l.item_state(0, 0).size.h, 16.0);
    assert_eq!(st[0].size.h, 7.0 * 16.0);

    let empty: List<Row> = List::default();
    let mut st2 = [NodeState::default(); 1];
    let sty2 = pass::default_styles(&empty);
    let mut empty = empty;
    pass::layout(&mut empty, &sty2, &mut st2, VIEW);
    assert_eq!(st2[0].size.h, 0.0, "an empty list is empty, not broken");
}

// ---- events reach into off-table branches ----------------------------

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Cell {
    pick: Button<App>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    list: List<Cell>,
}

#[derive(Default)]
struct App {
    ui: Ui,
    picked: Vec<u64>,
}

impl lumen_static::event::App for App {
    type Tree = Ui;
    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
}

fn on_pick(a: &mut App, c: &EventCtx) {
    a.picked.push(c.row.expect("row arrives from hit testing"));
}

#[test]
fn clicks_route_into_list_items_with_the_row_attached() {
    let mut app = App::default();
    app.ui.list = List::new(
        (0..4)
            .map(|_| {
                let mut c = Cell::default();
                c.pick.label = "p";
                c.pick.on_click = on_pick;
                c
            })
            .collect(),
    );

    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    let sty = pass::default_styles(&app.ui);
    pass::layout(&mut app.ui, &sty, &mut st, VIEW);

    // Click the third item's button, addressed through its own geometry.
    let b = app.ui.list.item_state(2, 1);
    let (x, y) = (b.pos.x + b.size.w / 2.0, b.pos.y + b.size.h / 2.0);
    let mut focus = None;
    for ev in [
        lumen_static::Event::PointerDown { x, y },
        lumen_static::Event::PointerUp { x, y },
    ] {
        pass::dispatch(&mut app, &sty, &st, &mut focus, &ev);
    }
    assert_eq!(app.picked, vec![2], "the row is an argument, never a capture");
}
