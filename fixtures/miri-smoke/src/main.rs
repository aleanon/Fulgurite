//! Release-shaped UB harness: exercises every `unsafe` path in the core
//! under one `main`, so `cargo miri run -p miri-smoke` checks the whole
//! release surface for undefined behaviour.
//!
//! Deliberately depends on `lumen-static` with **no features** and has no
//! dev-dependencies: the hot-reload and observability surfaces are not
//! part of a release build and are out of scope here.
//!
//! Every path below contains a `base + offset` cast, which is the one
//! unsafe pattern the whole architecture rests on. The dispatch section
//! additionally exercises the aliasing-critical sequence (a widget living
//! inside the app state, its handler copied out before `&mut S` exists).

use lumen_static::assemble::Assembled;
use lumen_static::fuse::Inline;
use lumen_static::hit::hit_test;
use lumen_static::inbox::Inbox;
use lumen_static::ops::{Button, Fixed, Label, Spacer};
use lumen_static::repeat::Repeat;
use lumen_static::{pass, Constraints, Event, EventCtx, NodeState, StaticTree, Widget};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 300.0, min_h: 0.0, max_h: 300.0 };

#[derive(StaticTree, Default)]
#[widget(row, spacing = 2.0)]
struct Row {
    icon: Fixed,
    name: Label,
    pick: Button<App>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 1.0, fuse)]
struct Cell {
    a: Label,
    b: Fixed,
    c: Spacer,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    head: Label,
    cell: Cell,
    save: Button<App>,
    list: Repeat<Row>,
}

#[derive(Default)]
struct App {
    ui: Ui,
    clicks: u32,
    rows: Vec<u64>,
}

fn on_save(a: &mut App, _: &EventCtx) {
    a.clicks += 1;
}

fn on_pick(a: &mut App, c: &EventCtx) {
    a.rows.push(c.row.unwrap_or(u64::MAX));
}

fn bind(r: &mut Row, _row: usize) {
    r.icon.w = 6.0;
    r.icon.h = 6.0;
    r.name.text = "row";
    r.pick.label = "p";
    r.pick.on_click = on_pick;
}

fn main() {
    let mut app = App::default();
    app.ui.head.text = "head";
    app.ui.cell.a.text = "cell";
    app.ui.save.label = "save";
    app.ui.save.on_click = on_save;
    app.ui.list = Repeat::new(500, 12.0, bind);

    let mut st = [NodeState::default(); Ui::NODE_COUNT];

    // 1. layout + paint: every kind's up/down/place/paint op, including
    //    the repeater's nested passes over its window.
    let base = &app.ui as *const Ui as *const u8;
    pass::layout(base, Ui::SUB_TABLE, Ui::KIND_LIST, &mut st, VIEW);
    let mut sum = 0u64;
    pass::paint(base, Ui::SUB_TABLE, Ui::KIND_LIST, &st, &mut sum);

    // 2. the rigid/flexible partitioned pass.
    let flex: Vec<u32> = (0..Ui::SUB_TABLE.len() as u32)
        .filter(|&i| !Ui::SUB_TABLE[i as usize].is_rigid())
        .collect();
    pass::layout_partitioned(base, Ui::SUB_TABLE, Ui::KIND_LIST, &mut st, VIEW, &flex);

    // 3. fused subtree: straight-line Inline path, separate from the ops.
    let fused = app.ui.cell.measure(VIEW);
    let mut fsum = 0u64;
    app.ui.cell.paint_inline(VIEW, Default::default(), fused, &mut fsum);

    // 4. hit testing, including descent into repeater rows.
    let list_st = st[Ui::SUB_TABLE.iter().position(|d| d.child_count == 0).unwrap()];
    let _ = list_st;
    let mut hits = 0;
    for i in 0..Ui::NODE_COUNT {
        let s = &st[i];
        if s.size.w > 0.0 && s.size.h > 0.0 {
            let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
            if hit_test(base, Ui::SUB_TABLE, Ui::KIND_LIST, &st, x, y).is_some() {
                hits += 1;
            }
        }
    }

    // 5. dispatch — the aliasing-critical path: `state` and `base` both
    //    derive from one `&mut App`, and handlers mutate `App` while the
    //    tree lives inside it.
    let mut focus = None;
    let targets: Vec<(f32, f32)> = {
        let save = &st[2 + Cell::NODE_COUNT - 1 + 1];
        let _ = save;
        (0..Ui::NODE_COUNT)
            .map(|i| {
                let s = &st[i];
                (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
            })
            .collect()
    };
    for (x, y) in targets {
        for ev in [Event::PointerDown { x, y }, Event::PointerUp { x, y }] {
            let sp = &mut app as *mut App as *mut u8;
            let b = unsafe { &raw const (*(sp as *mut App)).ui } as *const u8;
            pass::dispatch(sp, b, Ui::SUB_TABLE, Ui::KIND_LIST, &st, &mut focus, &ev);
        }
    }

    // 6. scrolled repeater: window re-materializes, rows re-bind, and a
    //    click resolves to a different row than before.
    app.ui.list.scroll = 60.0;
    let base = &app.ui as *const Ui as *const u8;
    pass::layout(base, Ui::SUB_TABLE, Ui::KIND_LIST, &mut st, VIEW);
    let inst = app.ui.list.instance_state(0, 3);
    let (bx, by) = (inst.pos.x + inst.size.w / 2.0, inst.pos.y + inst.size.h / 2.0);
    for ev in [Event::PointerDown { x: bx, y: by }, Event::PointerUp { x: bx, y: by }] {
        let sp = &mut app as *mut App as *mut u8;
        let b = unsafe { &raw const (*(sp as *mut App)).ui } as *const u8;
        pass::dispatch(sp, b, Ui::SUB_TABLE, Ui::KIND_LIST, &st, &mut focus, &ev);
    }

    // 7. runtime table assembly (§8's concat-at-startup path).
    let mut asm = Assembled::<Row>::new(8);
    let abase = asm.base();
    let (at, ak) = (asm.table.clone(), asm.kinds.clone());
    pass::layout(abase, &at, &ak, &mut asm.states, VIEW);
    let mut asum = 0u64;
    pass::paint(abase, &at, &ak, &asm.states, &mut asum);

    // 8. the async seam.
    let inbox: Inbox<u32> = Inbox::new();
    inbox.post(7);
    fn apply(a: &mut App, m: u32) {
        a.clicks += m;
    }
    inbox.drain(&mut app, apply);

    println!(
        "ok: paint={sum:#x} fused={fsum:#x} hits={hits} clicks={} rows={:?} asm={asum:#x}",
        app.clicks, app.rows
    );
}
