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
    /// **Frees, counted since M28/T28.0.**
    ///
    /// The gate counted allocations only, and `dealloc` forwarded to the
    /// system without counting — so a frame that *frees* on the frame
    /// thread and allocates nothing read as zero, clean. That is exactly
    /// the shape M28 introduces: draining a result out of an mpsc queue
    /// frees the queue's node, on the frame thread, with no matching
    /// allocation. A one-sided counter cannot see it.
    ///
    /// Zero here is the same claim as zero above, and for the same
    /// reason: a steady-state frame that touches the allocator at all
    /// forecloses the no-allocator profile, whichever direction it
    /// touches it in.
    static FREES: Cell<usize> = const { Cell::new(0) };
}

fn allocs() -> usize {
    ALLOCS.with(Cell::get)
}

fn frees() -> usize {
    FREES.with(Cell::get)
}

/// Both counters at once, so a case reads them in one place and prints
/// the pair beside its bound — `AGENT.md`'s rule about a bound with its
/// quantity next to it.
fn counters() -> (usize, usize) {
    (allocs(), frees())
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
        let _ = FREES.try_with(|c| c.set(c.get() + 1));
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

/// Allocations **and frees** per pump, after warming up — M28/T28.0.
///
/// Generic over retention (M8b): the gate must hold for both, and
/// `Incremental` is the one that could break it — it copies the display
/// list every frame, which allocates until the retained buffers have
/// grown to the frame's size. That is what "after warming up" covers,
/// and asserting zero afterwards is what proves the copy reuses them.
///
/// Returns the pair because the second half is the one M28 needs:
/// draining a completed job out of an mpsc queue **frees** the queue's
/// node on the frame thread and allocates nothing, so a counter that
/// watched only `alloc` would report a clean frame.
fn per_pump<A: lumen_static::event::App, R: lumen_static::Retention>(
    host: &mut Host<A, R>,
    n: usize,
) -> (f64, f64) {
    for _ in 0..5 {
        host.pump();
    }
    let before = counters();
    for _ in 0..n {
        host.pump();
    }
    let after = counters();
    ((after.0 - before.0) as f64 / n as f64, (after.1 - before.1) as f64 / n as f64)
}

#[test]
fn an_idle_frame_allocates_nothing() {
    let mut host: Host<_, Incremental> = Host::new(PlainApp::default(), 320, 200);
    let (a, f) = per_pump(&mut host, 100);
    assert_eq!((a, f), (0.0, 0.0), "plain tree, idle: {a:.2} allocs/frame, {f:.2} frees/frame");
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
    let before = counters();
    for i in 0..20 {
        host.app.count = i;
        host.pump();
    }
    let after = counters();
    assert_eq!(
        (after.0 - before.0, after.1 - before.1),
        (0, 0),
        "a binding that re-formats reuses its buffer: allocs {} frees {}",
        after.0 - before.0,
        after.1 - before.1
    );
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
    let (a, f) = per_pump(&mut host, 100);
    assert_eq!((a, f), (0.0, 0.0), "Repeat + List + Either + Dyn + Option, idle: {a:.2} allocs/frame, {f:.2} frees/frame");
}

#[test]
fn scrolling_a_ten_thousand_row_list_allocates_nothing() {
    let mut app = HatchApp::default();
    app.ui.list = Repeat::new(10_000, 20.0, noop_bind);
    let mut host: Host<_, Incremental> = Host::new(app, 320, 400);
    for _ in 0..5 {
        host.pump();
    }
    let before = counters();
    for i in 0..100 {
        host.app.ui.list.scroll = i as f32 * 20.0;
        host.pump();
    }
    let after = counters();
    assert_eq!(
        (after.0 - before.0, after.1 - before.1),
        (0, 0),
        "a scrolling repeater materializes from its own arena: allocs {} frees {}",
        after.0 - before.0,
        after.1 - before.1
    );
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
    let (a, f) = per_pump(&mut host, 100);
    assert_eq!((a, f), (0.0, 0.0), "two enum nodes, idle: {a:.2} allocs/frame, {f:.2} frees/frame");
}

#[test]
fn the_retention_seam_allocates_nothing_in_either_mode() {
    // Both halves of the M8b type parameter, on the same tree, so a
    // regression in the retained copy cannot hide behind the oracle.
    let mut rebuild: Host<_, Rebuild> = Host::new(PlainApp::default(), 320, 200);
    let (a, f) = per_pump(&mut rebuild, 100);
    assert_eq!((a, f), (0.0, 0.0), "Rebuild keeps nothing: {a:.2} allocs/frame, {f:.2} frees/frame");

    let mut incr: Host<_, Incremental> = Host::new(PlainApp::default(), 320, 200);
    // Drive it through the widths it will use, so the retained command
    // list and text buffer reach their steady-state capacity.
    for i in 0..30 {
        incr.app.count = i;
        incr.pump();
        incr.take_damage_for_age(1);
    }
    let before = counters();
    for i in 0..100 {
        incr.app.count = i % 30;
        incr.pump();
        incr.take_damage_for_age(1);
    }
        let after = counters();
    assert_eq!(
        (after.0 - before.0, after.1 - before.1),
        (0, 0),
        "the retained copy reuses its buffers: allocs {} frees {}",
        after.0 - before.0,
        after.1 - before.1
    );
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

    let before = counters();
    for i in 0..50 {
        host.app.count = i % 30;
        host.pump();
        canvas.render(&host.list, 320, 200);
    }
    let (n, f) = (allocs() - before.0, frees() - before.1);
    assert_eq!(
        (n, f),
        (0, 0),
        "50 rasterised frames: {n} allocs ({:.2}/frame), {f} frees ({:.2}/frame)",
        n as f64 / 50.0,
        f as f64 / 50.0
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
    let before = counters();
    for _ in 0..20 {
        canvas.render(&list, 200, 120);
    }
    let (n, f) = (allocs() - before.0, frees() - before.1);
    assert_eq!(
        (n, f),
        (0, 0),
        "20 frames with two glass panels: {n} allocs ({:.2}/frame, {:.2}/panel), \
         {f} frees ({:.2}/frame)",
        n as f64 / 20.0,
        n as f64 / 40.0,
        f as f64 / 20.0
    );
}

/// **The free counter must be able to see a free** — M28/T28.0.
///
/// A counter that never moves is indistinguishable from a counter that
/// does not work, and every assertion above reports zero. This is the
/// other direction: the exact shape M28 introduces, asserted to be
/// visible *before* anything in M28 is built.
///
/// A worker allocates and sends; **this** thread drains and drops. The
/// allocation is charged to the worker's counter and the free to ours,
/// so the frame thread shows `allocs 0, frees N` — a frame that touches
/// the allocator while every existing assertion here reads clean.
///
/// That asymmetry is the whole reason the counter was added. M28's
/// `Host::pump` will drain completed jobs out of exactly such a queue,
/// and `docs/00-handoff-static-tree.md` §3's invariant is that a
/// steady-state frame does not touch the allocator — in either
/// direction.
#[test]
fn the_free_counter_sees_a_drain_that_only_frees() {
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let worker = std::thread::spawn(move || {
        for i in 0..16u8 {
            // Allocated on the worker's thread, so the worker's ALLOCS
            // moves and ours does not.
            let _ = tx.send(vec![i; 64]);
        }
    });
    worker.join().expect("the worker finished");

    let before = counters();
    let mut drained = 0usize;
    while let Ok(v) = rx.try_recv() {
        drained += v.len();
        // `v` drops here, on this thread.
    }
    let after = counters();
    let (a, f) = (after.0 - before.0, after.1 - before.1);

    println!("drained {drained} bytes: allocs {a}, frees {f}");
    assert_eq!(drained, 16 * 64, "setup: every message arrived");
    assert_eq!(a, 0, "the drain must not allocate on this thread; it allocated {a} times");
    assert!(
        f >= 16,
        "the free counter did not see 16 frees from a drain that performs them — it \
         reported {f}, so the counter cannot gate what M28 is about to add"
    );
}
