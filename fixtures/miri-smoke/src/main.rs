//! Release-shaped harness: exercises the whole core in one `main`.
//!
//! Originally a UB probe for the `base + offset` casts. Those are gone —
//! the release core contains no `unsafe` at all — so this is now a
//! smoke test that every recursive path runs: layout, paint, hit
//! testing, dispatch, a scrolled repeater, runtime assembly, and the
//! inbox. It stays because `cargo miri run -p miri-smoke` remains the
//! cheapest whole-core check if the tooling is ever fixed.

use lumen_static::assemble::Assembled;
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

impl lumen_static::event::App for App {
    type Tree = Ui;
    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
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
    r.name.set("row");
    r.pick.label = "p";
    r.pick.on_click = on_pick;
}

fn main() {
    let mut app = App::default();
    app.ui.head.set("head");
    app.ui.cell.a.set("cell");
    app.ui.save.label = "save";
    app.ui.save.on_click = on_save;
    app.ui.list = Repeat::new(500, 12.0, bind);

    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    let sty = pass::default_styles(&app.ui);

    // layout + paint, including the fused subtree and the repeater's
    // nested per-instance layout.
    pass::layout(&mut app.ui, &sty, &mut st, VIEW);
    let mut sum = lumen_static::DisplayList::new();
    pass::paint(&app.ui, &st, &mut sum);

    // hit testing across the whole tree.
    let mut hits = 0;
    for i in 0..Ui::NODE_COUNT {
        let s = &st[i];
        if s.size.w > 0.0 && s.size.h > 0.0 {
            let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
            if pass::hit_test(&app.ui, &st, x, y).is_some() {
                hits += 1;
            }
        }
    }

    // dispatch over every node's centre.
    let mut focus = None;
    let targets: Vec<(f32, f32)> = (0..Ui::NODE_COUNT)
        .map(|i| (st[i].pos.x + st[i].size.w / 2.0, st[i].pos.y + st[i].size.h / 2.0))
        .collect();
    for (x, y) in targets {
        for ev in [Event::PointerDown { x, y }, Event::PointerUp { x, y }] {
            pass::dispatch(&mut app, &sty, &st, &mut focus, &ev);
        }
    }

    // scrolled repeater: window re-materializes and a click resolves to
    // a different row.
    app.ui.list.scroll = 60.0;
    pass::layout(&mut app.ui, &sty, &mut st, VIEW);
    let inst = app.ui.list.instance_state(0, 3);
    let (bx, by) = (inst.pos.x + inst.size.w / 2.0, inst.pos.y + inst.size.h / 2.0);
    for ev in [Event::PointerDown { x: bx, y: by }, Event::PointerUp { x: bx, y: by }] {
        pass::dispatch(&mut app, &sty, &st, &mut focus, &ev);
    }

    // runtime assembly (§8's concat-at-startup path).
    let mut asm = Assembled::<Row>::new(8);
    asm.layout(VIEW);
    let mut asum = lumen_static::DisplayList::new();
    asm.paint(&mut asum);

    // the async seam.
    let inbox: Inbox<u32> = Inbox::new();
    inbox.post(7);
    fn apply(a: &mut App, m: u32) {
        a.clicks += m;
    }
    inbox.drain(&mut app, apply);

    println!(
        "ok: cmds={} hits={hits} clicks={} rows={:?} asm-cmds={} depth={}",
        sum.len(),
        app.clicks,
        app.rows,
        asum.len(),
        Ui::DEPTH,
    );
}
