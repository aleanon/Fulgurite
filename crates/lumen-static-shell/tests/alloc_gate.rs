//! **Gate: a steady-state frame allocates nothing.**
//!
//! The design's central constraint (`docs/00-handoff-static-tree.md` §3,
//! §8) is zero heap allocation in steady state — the property that makes
//! a no-allocator profile possible at all. Nothing checked it. A
//! counting allocator was reached for three times by hand during the
//! 2026-09-02 review and found two violations that had shipped: the enum
//! derive allocated two `Vec`s per node per `layout`, and the agent's
//! journal allocated three collections per frame
//! (`docs/results-allocation.md`).
//!
//! The number asserted is **zero**, not a budget. Every path here is at
//! zero today, so a budget would only encode a bug.
//!
//! Its own binary, because `#[global_allocator]` is per-binary and would
//! otherwise tax every other test. Counters are read outside any
//! formatting — an earlier ad-hoc version measured a `println!` from the
//! previous probe leaking into the window.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    /// Per-thread, not global. `cargo test` runs tests in parallel
    /// threads sharing one process, so a global counter measures every
    /// other test's allocations too — which is how an earlier version of
    /// this file reported 0.16 allocations per frame from code that
    /// allocates none, and reported *more* the longer it warmed up.
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn allocs() -> usize {
    ALLOCS.with(Cell::get)
}

struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        // `try_with`: during thread teardown the TLS is gone, and a
        // panic in the allocator is not recoverable.
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}
#[global_allocator]
static A: Counting = Counting;

use lumen_static::hatch::{Dyn, Either, List};
use lumen_static::prelude::*;
use lumen_static::repeat::{noop_bind, Repeat};
use lumen_static::{Incremental, Rebuild};
use lumen_static_shell::Host;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Row {
    icon: Fixed,
    name: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Pane {
    a: Label,
    b: Fixed,
}

#[derive(StaticTree)]
enum Slot {
    Text(Label),
    Block(Fixed),
}

impl Default for Slot {
    fn default() -> Self {
        Slot::Text(Label::default())
    }
}

/// Plain: leaves and derived containers only.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 2.0)]
struct PlainUi {
    title: Label,
    value: Label<String>,
    gap: Spacer,
    go: Button<PlainApp>,
}

#[derive(DeriveApp, Default)]
struct PlainApp {
    #[tree]
    ui: PlainUi,
    #[bind(ui.value, "count: {}")]
    count: i64,
}

/// Every off-table construct at once.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct HatchUi {
    list: Repeat<Row>,
    items: List<Row>,
    tabs: Either<Pane, Row>,
    island: Dyn<HatchApp>,
    maybe: Option<Fixed>,
}

#[derive(DeriveApp, Default)]
struct HatchApp {
    #[tree]
    ui: HatchUi,
}

/// Count allocations over `n` pumps, after warming up.
///
/// Generic over retention (M8b): the gate must hold for both, and
/// `Incremental` is the one that could break it — it copies the display
/// list every frame, which allocates until the retained buffers have
/// grown to the frame's size. That is what "after warming up" covers,
/// and asserting zero afterwards is what proves the copy reuses them.
fn per_pump<A: lumen_static::event::App, R: lumen_static::Retention>(
    host: &mut Host<A, R>,
    n: usize,
) -> f64 {
    for _ in 0..5 {
        host.pump();
    }
    let before = allocs();
    for _ in 0..n {
        host.pump();
    }
    let after = allocs();
    (after - before) as f64 / n as f64
}

#[test]
fn an_idle_frame_allocates_nothing() {
    let mut host: Host<_, Incremental> = Host::new(PlainApp::default(), 320, 200);
    assert_eq!(per_pump(&mut host, 100), 0.0, "plain tree, idle");
}

#[test]
fn a_frame_carrying_a_state_change_allocates_nothing() {
    let mut host: Host<_, Incremental> = Host::new(PlainApp::default(), 320, 200);
    for _ in 0..5 {
        host.pump();
    }
    // Warm the label's capacity at the widths it will use.
    for i in 0..20 {
        host.app.count = i;
        host.pump();
    }
    let before = allocs();
    for i in 0..20 {
        host.app.count = i;
        host.pump();
    }
    let after = allocs();
    assert_eq!(after - before, 0, "a binding that re-formats reuses its buffer");
}

#[test]
fn a_frame_holding_every_hatch_allocates_nothing() {
    let mut app = HatchApp::default();
    app.ui.list = Repeat::new(10_000, 20.0, noop_bind);
    app.ui.items = List::new((0..8).map(|_| Row::default()).collect());
    app.ui.tabs = Either::new(Pane::default(), Row::default());
    app.ui.island = Dyn::new(Pane::default());
    app.ui.maybe = Some(Fixed { w: 4.0, h: 4.0 });

    let mut host: Host<_, Incremental> = Host::new(app, 320, 400);
    assert_eq!(per_pump(&mut host, 100), 0.0, "Repeat + List + Either + Dyn + Option, idle");
}

#[test]
fn scrolling_a_ten_thousand_row_list_allocates_nothing() {
    let mut app = HatchApp::default();
    app.ui.list = Repeat::new(10_000, 20.0, noop_bind);
    let mut host: Host<_, Incremental> = Host::new(app, 320, 400);
    for _ in 0..5 {
        host.pump();
    }
    let before = allocs();
    for i in 0..100 {
        host.app.ui.list.scroll = i as f32 * 20.0;
        host.pump();
    }
    let after = allocs();
    assert_eq!(after - before, 0, "a scrolling repeater materializes from its own arena");
}

#[test]
fn an_enum_widget_allocates_nothing() {
    // The enum derive allocated two Vecs per node per layout until
    // F4.3, in framework-generated code.
    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct EnumUi {
        one: Slot,
        two: Slot,
    }
    #[derive(DeriveApp, Default)]
    struct EnumApp {
        #[tree]
        ui: EnumUi,
    }
    let mut app = EnumApp::default();
    // One of each variant, so both index ranges are exercised.
    app.ui.two = Slot::Block(Fixed { w: 8.0, h: 8.0 });
    let mut host: Host<_, Incremental> = Host::new(app, 320, 200);
    assert_eq!(per_pump(&mut host, 100), 0.0, "two enum nodes, idle");
}

#[test]
fn the_retention_seam_allocates_nothing_in_either_mode() {
    // Both halves of the M8b type parameter, on the same tree, so a
    // regression in the retained copy cannot hide behind the oracle.
    let mut rebuild: Host<_, Rebuild> = Host::new(PlainApp::default(), 320, 200);
    assert_eq!(per_pump(&mut rebuild, 100), 0.0, "Rebuild keeps nothing");

    let mut incr: Host<_, Incremental> = Host::new(PlainApp::default(), 320, 200);
    // Drive it through the widths it will use, so the retained command
    // list and text buffer reach their steady-state capacity.
    for i in 0..30 {
        incr.app.count = i;
        incr.pump();
        incr.take_damage_for_age(1);
    }
    let before = allocs();
    for i in 0..100 {
        incr.app.count = i % 30;
        incr.pump();
        incr.take_damage_for_age(1);
    }
    assert_eq!(allocs() - before, 0, "the retained copy reuses its buffers");
}

/// **Gate: rasterising a frame allocates nothing either** — M26/T26.13.
///
/// The gate above covers `pump`: input, layout, paint and retention. It
/// stops at the display list. Everything after it — the software
/// rasteriser, its clip stack, its glass scratch buffers — was outside
/// what any test could see, on a repository whose stated invariant is
/// zero steady-state allocation.
///
/// Written **before** the code it judges, which is the order this task
/// was specified in: the first run of this is the measurement of what
/// the raster path costs today.
#[test]
fn rasterising_a_frame_allocates_nothing() {
    use lumen_static::Renderer;
    use lumen_static_shell::software::Canvas;

    let mut host: Host<_, Incremental> = Host::new(PlainApp::default(), 320, 200);
    let mut canvas = Canvas::new(320, 200);
    // Warm: the canvas, the list and every buffer either reaches its
    // steady-state capacity here.
    for i in 0..30 {
        host.app.count = i;
        host.pump();
        canvas.render(&host.list, 320, 200);
    }

    let before = allocs();
    for i in 0..50 {
        host.app.count = i % 30;
        host.pump();
        canvas.render(&host.list, 320, 200);
    }
    let n = allocs() - before;
    assert_eq!(
        n, 0,
        "50 rasterised frames allocated {n} times — {:.2} per frame",
        n as f64 / 50.0
    );
}

/// **And a frame carrying glass** — M26/T26.13, part two.
///
/// The test above holds no `Cmd::Glass`, so it cannot see the blur's
/// scratch buffers: `glass_within` builds three `Vec`s — the capture,
/// its accumulator, and the ping-pong target — **per glass command**.
/// Written before the fix, so its first run is the measurement.
#[test]
fn a_frame_carrying_glass_allocates_nothing() {
    use lumen_static::display::{Color, DisplayList, Material};
    use lumen_static::Renderer;
    use lumen_static_shell::software::Canvas;

    let mut canvas = Canvas::new(200, 120);
    let mut list = DisplayList::new();
    list.rect([0.0, 0.0, 200.0, 120.0], Color(30, 60, 90, 255));
    // Two panels, so "per command" is distinguishable from "per frame".
    list.glass([20.0, 20.0, 90.0, 90.0], 8.0, Material { blur: 6.0, ..Material::default() });
    list.glass([110.0, 20.0, 180.0, 90.0], 8.0, Material { blur: 6.0, ..Material::default() });

    for _ in 0..5 {
        canvas.render(&list, 200, 120);
    }
    let before = allocs();
    for _ in 0..20 {
        canvas.render(&list, 200, 120);
    }
    let n = allocs() - before;
    assert_eq!(
        n, 0,
        "20 frames with two glass panels allocated {n} times — {:.2} per \
         frame, {:.2} per panel",
        n as f64 / 20.0,
        n as f64 / 40.0
    );
}
