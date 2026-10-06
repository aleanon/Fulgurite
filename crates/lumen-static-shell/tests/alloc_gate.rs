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
//! **What "every path" did not include until M50 — T50.1.** The first
//! raster arms render on `Canvas::new`, which draws block glyphs, and pump
//! with `BlockMetrics`. The truetype path every window runs
//! (`Canvas::with_font`, `FontMetrics`) was not measured, and it
//! allocated on every frame that drew text
//! (`docs/results-allocation.md`, "The truetype path was not covered").
//! **Since M50/T50.2 it is covered**: the truetype arms at the bottom
//! (`mod truetype`) draw that path on the pinned face — a changing label,
//! a 3000-row list with one row changed, a text field per edit — and read
//! zero, because the canvas shapes into scratch it keeps and blits from
//! a glyph cache, and `FontMetrics` measures into a layout it keeps.
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
use lumen_static_widgets::prelude::*;
use lumen_static::repeat::{noop_bind, Repeat};
use lumen_static::invalidate::Everything;
use lumen_static::{Incremental, Rebuild};

/// **The arms that must reach layout run under `Everything`** — M41/T41.3.
///
/// Since M34/T34.2 a headless `Host<A>` is `Boundaries<256>`, and an idle
/// pump there marks nothing and lays nothing out. So every arm below that
/// exists to measure *layout* — a repeater's window and binds, `List`,
/// `Dyn`, an enum — measured a frame in which none of it ran:
/// `a_repeater_whose_bind_writes_text_allocates_nothing` stayed green
/// with its bind rewritten to `set(format!(..))`, T29.5's own injection.
/// Those arms name this type; the plain idle arm keeps the default,
/// because an idle frame that lays nothing out is the product's.
type Whole<A> = Host<A, Incremental, Everything>;
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
fn per_pump<A: lumen_static::event::App, R: lumen_static::Retention, I: lumen_static::invalidate::Invalidation>(
    host: &mut Host<A, R, I>,
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

    let mut host: Whole<_> = Host::new(app, 320, 400);
    let (a, f) = per_pump(&mut host, 100);
    assert_eq!((a, f), (0.0, 0.0), "Repeat + List + Either + Dyn + Option, idle: {a:.2} allocs/frame, {f:.2} frees/frame");
}

/// The bind that a repeater actually gets given — M29/T29.5.
///
/// **Every other repeater arm in this file uses [`noop_bind`]**, so
/// until this existed the gate proved zero allocation for a list whose
/// rows are never filled in. That is `AGENT.md`'s first vacuous shape:
/// the only thing that allocates on a repeater frame is a bind that
/// *writes*, and no arm had one.
///
/// A repeater binds **every row on every layout that reaches it** — on
/// every frame under `Everything`, and on every frame that marks or
/// passes through it under `Boundaries` (a scroll, a reported data
/// change, a sibling's change) — so a bind is on the steady-state path
/// and this gate is the one that says so. It runs under `Everything`
/// ([`Whole`]), the frame that reaches the list every time.
/// `format_into` writes through a reused scratch buffer and compares
/// before storing, which is the same machinery `#[bind]` generates for
/// a table node; `set(format!(…))` heap-allocates per row per frame and
/// is what a bind author writes first.
fn bind_row(row: &mut Row, i: usize) {
    lumen_static::bind::format_into(&mut row.name, format_args!("row {i}"));
}

#[test]
fn a_repeater_whose_bind_writes_text_allocates_nothing() {
    let mut app = HatchApp::default();
    app.ui.list = Repeat::new(10_000, 20.0, bind_row);
    let mut host: Whole<_> = Host::new(app, 320, 400);
    let (a, f) = per_pump(&mut host, 100);
    assert_eq!(
        (a, f),
        (0.0, 0.0),
        "a bind runs every row on every layout, so it is steady state: \
         {a:.2} allocs/frame, {f:.2} frees/frame over a {}-row window",
        host.app.ui.list.window_len()
    );
}

#[test]
fn scrolling_a_ten_thousand_row_list_allocates_nothing() {
    let mut app = HatchApp::default();
    app.ui.list = Repeat::new(10_000, 20.0, noop_bind);
    let mut host: Whole<_> = Host::new(app, 320, 400);
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
    let mut host: Whole<_> = Host::new(app, 320, 200);
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
///
/// **Block glyphs only — M50/T50.1.** It renders on `Canvas::new`, whose
/// text is the block-glyph stand-in, and pumps with `BlockMetrics`. Every
/// window renders on `Canvas::with_font` and measures with `FontMetrics`,
/// and that path allocated on every frame until M50/T50.2: fontdue
/// `Layout`s, the placement and glyph `Vec`s, one coverage `Vec` per
/// glyph (`docs/results-allocation.md`). This arm was named
/// `rasterising_a_frame_allocates_nothing` until T50.1, which read as
/// covering the frame every window draws; the truetype arms in `mod
/// truetype` below are the ones that do.
#[test]
fn rasterising_a_block_glyph_frame_allocates_nothing() {
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

/// **A pump that actually applies deferred results** — M28/T28.2.
///
/// Every case above has `Msg = Never`, so the drain `Host::pump` performs
/// is a `try_recv` on a queue nothing can write to: it proves the drain
/// is free when there is nothing to drain, which is the easy half.
///
/// This is the other half, and it is the one the free counter was added
/// for. The messages are `u32` — no heap payload — so anything the
/// allocator sees here belongs to the **queue**, not to the message. An
/// unbounded `mpsc::channel` allocates a node per send and frees it per
/// receive, which would put a free on the frame thread for every result
/// applied; `sync_channel`'s buffer is allocated once, at construction.
///
/// That difference is the reason `Inbox` is bounded, and it is measured
/// rather than argued: `the_free_counter_sees_a_drain_that_only_frees`
/// above drains sixteen messages out of an **unbounded** `mpsc::channel`
/// and reports `allocs 0, frees 16` — one free per message, on the
/// draining thread. This test drains fifty out of the bounded one and
/// reports `frees 0`. Same counter, same thread, two channel types.
#[test]
fn a_pump_that_applies_results_allocates_nothing() {
    use lumen_static::event::App;
    use lumen_static::prelude::*;

    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct DeferUi {
        a: Fixed,
        b: Fixed,
    }

    #[derive(Default)]
    struct Defer {
        ui: DeferUi,
        total: u64,
    }

    impl App for Defer {
    // M33: this app keeps handlers on the whole of itself.
    type State = Self;
    fn state_mut(&mut self) -> &mut Self::State {
        self
    }
        type Tree = DeferUi;
        type Msg = u32;
        fn tree(&self) -> &DeferUi {
            &self.ui
        }
        fn tree_mut(&mut self) -> &mut DeferUi {
            &mut self.ui
        }
        fn apply(this: &mut Self, m: u32) -> bool {
            this.total += m as u64;
            true
        }
    }

    let mut host: Host<Defer, Incremental> = Host::new(Defer::default(), 320, 200);
    let tx = host.sender();
    // Warm: every buffer reaches its steady-state capacity, including
    // the ones a marked-dirty frame grows.
    for i in 0..30u32 {
        tx.send(i).expect("capacity");
        host.pump();
    }

    let before = counters();
    let n = 50u32;
    for i in 0..n {
        tx.send(i).expect("capacity");
        host.pump();
    }
    let after = counters();
    let (a, f) = (after.0 - before.0, after.1 - before.1);

    println!(
        "{n} frames each applying one result: allocs {a}, frees {f} (applied total {})",
        host.app.total
    );
    assert!(host.app.total > 0, "setup: results actually landed");
    assert_eq!(
        (a, f),
        (0, 0),
        "a frame that applies a deferred result touched the allocator: {a} allocs, {f} frees"
    );
}

/// **The pixel buffer grows exactly and gives memory back** — M50/T50.0,
/// for R8-F7.
///
/// `Canvas::resize` cleared and `resize`d its `Vec`, so a grow past
/// capacity took `Vec`'s amortised doubling — one pixel wider at 4K
/// reserved a second 4K frame — and a shrink kept whatever it had. A
/// resize is not the steady state (`docs/results-allocation.md`), so
/// this bounds what a resize costs rather than asserting zero: exact
/// growth, memory returned on a large shrink, and a drag that allocates
/// only when it passes what it already holds. Each check prints its
/// capacity beside its length.
#[test]
fn resizing_the_canvas_grows_exactly_and_gives_memory_back() {
    use lumen_static_shell::software::Canvas;

    // (1) 4K, then one pixel wider: exact, in one allocation.
    let mut c = Canvas::new(3840, 2160);
    let a = counters();
    c.resize(3841, 2160);
    let n = allocs() - a.0;
    let (len, cap) = (c.pixels.len(), c.pixels.capacity());
    println!("(1) 3840x2160 -> 3841x2160: len {len}, capacity {cap} (bound: capacity == len), {n} allocations (bound 1)");
    assert_eq!((cap, n), (len, 1), "a one-pixel grow at 4K reserved {cap} bytes for {len} in {n} allocations");

    // (2) Shrink to 720p: the 4K reservation goes back.
    c.resize(1280, 720);
    let (len, cap) = (c.pixels.len(), c.pixels.capacity());
    println!("(2) -> 1280x720: len {len}, capacity {cap} (bound: capacity <= 2 x len = {})", 2 * len);
    assert!(cap <= 2 * len, "a shrink to 720p kept {cap} bytes for {len}");

    // (3) A drag of +-1 px around 800x600: an allocation only for a step
    // whose length passes everything the buffer has held since.
    let mut c = Canvas::new(800, 600);
    let (mut high, mut past) = (c.pixels.len(), 0usize);
    let a = counters();
    for k in 0..40u32 {
        let w = if k.is_multiple_of(2) { 801 } else { 800 };
        let h = 600 + (k % 3);
        let want = w as usize * h as usize * 4;
        if want > high {
            past += 1;
            high = want;
        }
        c.resize(w, h);
    }
    let n = allocs() - a.0;
    println!("(3) 40-step drag at 800x600: {n} allocations (bound {past}, the steps past capacity); len {}, capacity {}", c.pixels.len(), c.pixels.capacity());
    assert!(n <= past, "a drag allocated {n} times for {past} steps past capacity");

    // Accept: a resize to the same size allocates nothing.
    let a = counters();
    c.resize(c.width, c.height);
    assert_eq!(allocs() - a.0, 0, "a resize to the same size allocated");
    let _ = frees();
}

/// **The truetype arms — M50/T50.0, for R8-F1.**
///
/// Everything above renders block glyphs on `Canvas::new` and pumps with
/// `BlockMetrics`. A window renders on `Canvas::with_font` and measures
/// with `FontMetrics`, so these three arms draw that path, on the pinned
/// face (`tests/fonts`) so the count does not depend on the machine's
/// fonts. Each asserts its setup first — runs were shaped, the frame
/// really drew text — and prints what the frame did beside the count:
/// runs shaped, glyphs rasterised and glyphs drawn per frame.
///
/// `dev-shape` runs this binary with `--features truetype` by name and
/// `scripts/check_test_counts.sh` refuses it at zero tests, so a feature
/// split cannot compile these out and leave the file green (M43/R17-F1's
/// shape).
#[cfg(feature = "truetype")]
#[path = "fonts/pinned.rs"]
mod pinned;

/// **A warm frame with two shader brackets allocates nothing** — M83/T83.3
/// (`docs/plan-gpu-extension.md`, the allocation gate).
///
/// The uniform bytes go into the list's own `uniforms` buffer, cleared with
/// the frame like `text`, so recording a bracket reuses its capacity. The
/// list is re-recorded every frame, as a host's is, and rasterised; the
/// fallback is a clipped fill. `cfg(lumen_shader)` from this crate's
/// `build.rs`; `dev-shape` runs it with `--features truetype,shader` and
/// counts the arm present.
#[cfg(lumen_shader)]
mod shader {
    use super::{allocs, counters, frees};
    use lumen_static::display::{Color, DisplayList, ShaderId};
    use lumen_static::Renderer;
    use lumen_static_shell::software::Canvas;

    fn record(list: &mut DisplayList, k: u8) {
        list.clear();
        list.rect([0.0, 0.0, 200.0, 120.0], Color(30, 60, 90, 255));
        for (i, x) in [20.0f32, 110.0].into_iter().enumerate() {
            list.shader([x, 20.0, x + 70.0, 90.0], ShaderId(i as u16), &[k; 16]);
            list.rect([x - 10.0, 10.0, x + 80.0, 100.0], Color(200, 80, 40, 255));
            list.shader_end();
        }
    }

    #[test]
    fn a_frame_with_two_shader_brackets_allocates_nothing() {
        let mut canvas = Canvas::new(200, 120);
        let mut list = DisplayList::new();
        for k in 0..5 {
            record(&mut list, k);
            canvas.render(&list, 200, 120);
        }
        assert_eq!(list.uniforms.len(), 32, "setup: two brackets of 16 bytes each");
        let before = counters();
        for k in 0..20 {
            record(&mut list, k);
            canvas.render(&list, 200, 120);
        }
        let (n, f) = (allocs() - before.0, frees() - before.1);
        println!("20 frames with two shader brackets: {n} allocs, {f} frees (bound 0, 0)");
        assert_eq!(
            (n, f),
            (0, 0),
            "20 frames with two shader brackets: {n} allocs ({:.2}/frame), {f} frees ({:.2}/frame)",
            n as f64 / 20.0,
            f as f64 / 20.0
        );
    }
}

#[cfg(feature = "truetype")]
mod truetype {
    use super::{allocs, counters, frees, PlainApp};
    use lumen_static::event::{Event, Key};
    use lumen_static::hatch::List;
    use lumen_static::prelude::*;
    use lumen_static::{Color, Incremental, Renderer};
    use lumen_static_shell::software::Canvas;
    use lumen_static_shell::text::{text_counters, FontMetrics, FontStack, TextCounters};
    use lumen_static_shell::Host;
    use lumen_static_widgets::prelude::*;
    use lumen_static_widgets::TextInput;

    use super::pinned;

    /// The text counters are process-wide and these arms run on threads:
    /// one arm at a time, so a frame's counts are its own. The block-glyph
    /// arms above never shape, so they cannot move them. Shared with
    /// `mod complex` (M81/T81.3).
    use super::truetype_lock::ONE;

    fn face() -> FontStack {
        FontStack::from_bytes(pinned::bytes(), None).expect("the pinned face parses")
    }

    /// What `frames` frames did, per frame, for the line an arm prints.
    fn per_frame(a: TextCounters, b: TextCounters, frames: usize) -> String {
        let n = frames as f64;
        format!(
            "runs shaped {:.2}/frame, glyphs placed {:.2}/frame, runs measured {:.2}/frame, glyphs rasterised {:.2}/frame, glyphs drawn {:.2}/frame",
            (b.shaped - a.shaped) as f64 / n,
            (b.placed - a.placed) as f64 / n,
            (b.measured - a.measured) as f64 / n,
            (b.rasterised - a.rasterised) as f64 / n,
            (b.drawn - a.drawn) as f64 / n,
        )
    }

    /// **Arm (i): the gate's own `PlainApp`, a changing label every frame,
    /// full redraws.** The review measured 4.60 pump + 25.80 raster
    /// allocations per frame here.
    #[test]
    fn a_truetype_frame_allocates_nothing() {
        let _one = super::lock(&ONE);
        let f = face();
        let mut m = FontMetrics::new(f.clone());
        let mut host: Host<_, Incremental> = Host::new(PlainApp::default(), 320, 200);
        let mut canvas = Canvas::with_font(320, 200, f);
        // Warm through every value the counted frames show, so each glyph
        // they need has been seen once.
        for i in 0..30 {
            host.app.count = i;
            host.pump_with(&mut m);
            canvas.render(&host.list, 320, 200);
        }
        let t0 = text_counters();
        let (mut pump, mut raster) = ((0, 0), (0, 0));
        for i in 0..50 {
            host.app.count = i % 30;
            let a = counters();
            host.pump_with(&mut m);
            let b = counters();
            canvas.render(&host.list, 320, 200);
            let c = counters();
            pump = (pump.0 + b.0 - a.0, pump.1 + b.1 - a.1);
            raster = (raster.0 + c.0 - b.0, raster.1 + c.1 - b.1);
        }
        let t1 = text_counters();
        let what = per_frame(t0, t1, 50);
        println!(
            "truetype PlainApp, 50 changing frames: pump allocs {} frees {}, raster allocs {} frees {} (bound 0 each); {what}",
            pump.0, pump.1, raster.0, raster.1
        );
        assert!(t1.shaped - t0.shaped >= 50, "setup: the frames shaped no text ({what})");
        assert!(t1.measured - t0.measured >= 50, "setup: the changing label was never measured ({what})");
        assert!(t1.drawn - t0.drawn >= 50, "setup: no glyph was drawn ({what})");
        assert_eq!((pump, raster), ((0, 0), (0, 0)), "a truetype frame touched the allocator: {what}");
    }

    #[derive(StaticTree, Default)]
    #[widget(row, spacing = 0.0)]
    struct ListRow {
        icon: Fixed,
        name: Label<String>,
    }

    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct ListUi {
        items: List<ListRow>,
    }

    #[derive(DeriveApp, Default)]
    struct ListApp {
        #[tree]
        ui: ListUi,
    }

    /// **Arm (ii): a 3000-row list, one row changed per frame, drawn over
    /// the damage regions** — what a window presents. The review measured
    /// 6.00 pump (0.00 since M29 and M34) and 156.00 raster allocations per
    /// frame.
    #[test]
    fn a_truetype_list_with_one_row_changed_allocates_nothing() {
        let _one = super::lock(&ONE);
        let (w, h) = (800u32, 800u32);
        let f = face();
        let mut m = FontMetrics::new(f.clone());
        let mut app = ListApp::default();
        app.ui.items = List::new(
            (0..3000)
                .map(|i| {
                    let mut r = ListRow { icon: Fixed { w: 12.0, h: 12.0 }, ..Default::default() };
                    r.name.set(format!("row number {i} of the list"));
                    r
                })
                .collect(),
        );
        let mut host: Host<_, Incremental> = Host::new(app, w, h);
        let mut canvas = Canvas::with_font(w, h, f);
        let vp = [0.0, 0.0, w as f32, h as f32];
        host.pump_with(&mut m);
        canvas.render(&host.list, w, h);
        host.take_damage_for_age(1);
        let mut step = |host: &mut Host<ListApp, Incremental>, canvas: &mut Canvas, k: usize| {
            let s = if k.is_multiple_of(2) { "row number 5 of the list" } else { "row number 5 OF the list" };
            host.app.ui.items.items[5].name.set_str(s);
            // A direct write is marked by its writer: since M53/T53.3 an
            // unmarked pump is skipped outright, and this one would damage
            // nothing (the setup assertion below says so). Marked here,
            // outside the counted span; the list has no author id.
            host.invalidate_all();
            let a = counters();
            host.pump_with(&mut m);
            let b = counters();
            let mut regions = 0usize;
            for r in host.take_damage_for_age(1).regions(vp) {
                canvas.clear_within(r, Color(255, 255, 255, 255));
                canvas.render_within(&host.list, r);
                regions += 1;
            }
            let c = counters();
            ((b.0 - a.0, b.1 - a.1), (c.0 - b.0, c.1 - b.1), regions)
        };
        for k in 0..20 {
            step(&mut host, &mut canvas, k);
        }
        let t0 = text_counters();
        let (mut pump, mut raster, mut regions) = ((0, 0), (0, 0), 0);
        for k in 0..50 {
            let (p, r, n) = step(&mut host, &mut canvas, k);
            pump = (pump.0 + p.0, pump.1 + p.1);
            raster = (raster.0 + r.0, raster.1 + r.1);
            regions += n;
        }
        let t1 = text_counters();
        let what = per_frame(t0, t1, 50);
        println!(
            "truetype 3000-row list, one row changed, 50 frames over {regions} damage regions: \
             pump allocs {} frees {}, raster allocs {} frees {} (bound 0 each); {what}",
            pump.0, pump.1, raster.0, raster.1
        );
        assert!(regions >= 50, "setup: a changed row damaged nothing ({regions} regions)");
        assert!(t1.shaped - t0.shaped >= 50, "setup: the damaged row was never drawn ({what})");
        assert_eq!((pump, raster), ((0, 0), (0, 0)), "a one-row change touched the allocator: {what}");
    }

    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct EditUi {
        #[widget_id("input")]
        input: TextInput,
        #[widget_id("lbl")]
        lbl: Label<String>,
    }

    #[derive(DeriveApp, Default)]
    struct EditApp {
        #[tree]
        ui: EditUi,
    }

    /// One edit cycle that leaves the field as it found it, so a count is
    /// the frames' and not the value's growth: an edit that makes the
    /// string longer allocates for the string, which is the application's
    /// data and not the frame.
    type Cycle = fn(&mut Host<EditApp, Incremental>, &mut FontMetrics);

    fn key(host: &mut Host<EditApp, Incremental>, m: &mut FontMetrics, k: Key) {
        host.post(Event::Key { key: k, down: true });
        host.pump_with(m);
    }

    const CYCLES: &[(&str, Cycle)] = &[
        ("Text then Backspace", |h, m| {
            h.post(Event::Text { ch: 'x' });
            h.pump_with(m);
            key(h, m, Key::Backspace);
        }),
        ("Left then Right", |h, m| {
            key(h, m, Key::Left);
            key(h, m, Key::Right);
        }),
        ("paste \"hello\" then five Backspaces", |h, m| {
            h.paste_with("hello", m);
            for _ in 0..5 {
                key(h, m, Key::Backspace);
            }
        }),
    ];

    /// **Arm (iii): a focused `TextInput`, per event** — typing, caret
    /// moves, deleting and a paste, each cycle drawn over its damage. The
    /// review measured `Text` 7.00 + 106.76, `Left` 0 + 162.00, `Backspace`
    /// 5.66 + 62.28 and a paste 0.08 + 475.56 allocations per event (pump +
    /// raster).
    #[test]
    fn truetype_text_input_events_allocate_nothing() {
        let _one = super::lock(&ONE);
        let (w, h) = (400u32, 120u32);
        let f = face();
        let mut m = FontMetrics::new(f.clone());
        let mut app = EditApp::default();
        app.ui.input.set_str("the quick brown fox");
        app.ui.lbl.set_str("a label beside it");
        let mut host: Host<_, Incremental> = Host::new(app, w, h);
        let mut canvas = Canvas::with_font(w, h, f);
        let vp = [0.0, 0.0, w as f32, h as f32];
        host.pump_with(&mut m);
        key(&mut host, &mut m, Key::Tab);
        assert!(host.input.focus.is_some(), "setup: Tab focused nothing");
        canvas.render(&host.list, w, h);
        host.take_damage_for_age(1);
        let mut failures = Vec::new();
        for (name, cycle) in CYCLES {
            let mut run = |host: &mut Host<EditApp, Incremental>, canvas: &mut Canvas| {
                let a = counters();
                cycle(host, &mut m);
                let b = counters();
                let mut regions = 0usize;
                for r in host.take_damage_for_age(1).regions(vp) {
                    canvas.clear_within(r, Color(255, 255, 255, 255));
                    canvas.render_within(&host.list, r);
                    regions += 1;
                }
                let c = counters();
                ((b.0 - a.0, b.1 - a.1), (c.0 - b.0, c.1 - b.1), regions)
            };
            for _ in 0..2 {
                run(&mut host, &mut canvas);
            }
            let before = host.app.ui.input.as_str().to_string();
            let t0 = text_counters();
            let (mut pump, mut raster, mut regions) = ((0, 0), (0, 0), 0);
            for _ in 0..20 {
                let (p, r, n) = run(&mut host, &mut canvas);
                pump = (pump.0 + p.0, pump.1 + p.1);
                raster = (raster.0 + r.0, raster.1 + r.1);
                regions += n;
            }
            let t1 = text_counters();
            let what = per_frame(t0, t1, 20);
            println!(
                "truetype TextInput, {name}, 20 cycles over {regions} damage regions: \
                 pump allocs {} frees {}, raster allocs {} frees {} (bound 0 each); per cycle: {what}",
                pump.0, pump.1, raster.0, raster.1
            );
            assert_eq!(host.app.ui.input.as_str(), before, "setup: {name} did not leave the field as it found it");
            assert!(regions >= 20, "setup: {name} damaged nothing ({regions} regions)");
            assert!(t1.shaped > t0.shaped, "setup: {name} drew no text ({what})");
            if (pump, raster) != ((0, 0), (0, 0)) {
                failures.push(format!("{name}: pump {pump:?}, raster {raster:?}"));
            }
        }
        let _ = (allocs(), frees());
        assert!(failures.is_empty(), "a text-input event touched the allocator: {failures:?}");
    }
}

/// A poisoned lock is still a lock: one arm failing must not fail the
/// next one for a reason that is not its own.
#[cfg(feature = "truetype")]
fn lock(m: &'static std::sync::Mutex<()>) -> std::sync::MutexGuard<'static, ()> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// **`Shaping::Complex` frames** — M81/T81.3 (`docs/plan-shaped-paint.md`,
/// `docs/results-allocation.md` M81/T81.3).
///
/// The shaper keeps its parsed faces, shape plans, buffer and scratch in
/// the canvas's and the measurer's `ShapeScratch`, so a steady frame on a
/// `Complex` stack touches the allocator no more than a `Basic` one:
///
/// - a changing left-to-right label that needs the shaper (a combining
///   mark), full redraws: **0** (no right-to-left character, so no
///   `BidiInfo` is built); and plain Latin, which since K81a's response
///   never reaches the shaper: **0**, through fontdue;
/// - a still right-to-left screen, drawn over its damage: **0**;
/// - a changing right-to-left label, drawn over its damage: exactly what
///   `unicode_bidi` allocates for each run the shaper laid out — one
///   `BidiInfo` and one `visual_runs` per line, counted here by calling
///   them on the same strings — and nothing else. The file's one text
///   budget, with its reopen condition in the results doc: a bidi pass
///   into caller-kept buffers would make it 0.
///
/// Injection (T81.3's commit): build a shape plan per call instead of
/// from the scratch; the left-to-right arm prints its allocations.
#[cfg(feature = "shaping")]
mod complex {
    use super::{counters, PlainApp};
    use lumen_static::prelude::*;
    use lumen_static::{Color, Incremental, Renderer};
    use lumen_static_shell::software::Canvas;
    use lumen_static_shell::text::{text_counters, FontMetrics, FontStack, Shaping};
    use lumen_static_shell::Host;
    use lumen_static_widgets::prelude::*;

    use super::pinned;

    fn face() -> FontStack {
        FontStack::from_static(pinned::bytes(), None).expect("the pinned face parses").with_shaping(Shaping::Complex)
    }

    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct MarkUi {
        #[widget_id("value")]
        value: Label<String>,
    }

    #[derive(DeriveApp, Default)]
    struct MarkApp {
        #[tree]
        ui: MarkUi,
    }

    /// Thirty values of one shape, each with a combining acute, so every
    /// run needs the shaper (`needs_shaping`) and none needs `unicode_bidi`.
    /// Plain Latin no longer reaches the shaper at all since K81a's
    /// response, so it could not exercise the shaper's kept scratch.
    fn marked(i: usize) -> String {
        format!("cafe\u{301} {:02}", i % 30)
    }

    #[test]
    fn a_changing_left_to_right_label_on_a_complex_stack_allocates_nothing() {
        let _one = super::lock(&super::truetype_lock::ONE);
        let f = face();
        let mut m = FontMetrics::new(f.clone());
        let mut host: Host<_, Incremental> = Host::new(MarkApp::default(), 320, 200);
        let mut canvas = Canvas::with_font(320, 200, f);
        let mut values: Vec<String> = (0..30).map(marked).collect();
        for v in &values {
            host.app.ui.value.set_str(v);
            assert!(host.invalidate("value"), "setup: #value is not addressable");
            host.pump_with(&mut m);
            canvas.render(&host.list, 320, 200);
        }
        let t0 = text_counters();
        let (mut pump, mut raster) = ((0, 0), (0, 0));
        for i in 0..50 {
            let v = std::mem::take(&mut values[i % 30]);
            host.app.ui.value.set_str(&v);
            values[i % 30] = v;
            assert!(host.invalidate("value"), "setup: #value is not addressable");
            let a = counters();
            host.pump_with(&mut m);
            let b = counters();
            canvas.render(&host.list, 320, 200);
            let c = counters();
            pump = (pump.0 + b.0 - a.0, pump.1 + b.1 - a.1);
            raster = (raster.0 + c.0 - b.0, raster.1 + c.1 - b.1);
        }
        let t1 = text_counters();
        let (shaped, bidi) = (t1.complex - t0.complex, t1.bidi - t0.bidi);
        println!(
            "Complex, changing combining-mark label, 50 frames: pump allocs {} frees {}, raster allocs {} frees {} \
             (bound 0 each); {:.2} runs through the shaper per frame (setup >= 1), {bidi} through unicode_bidi (setup 0)",
            pump.0, pump.1, raster.0, raster.1, shaped as f64 / 50.0
        );
        assert!(shaped >= 50, "setup: the Complex frames laid nothing out through the shaper ({shaped})");
        assert_eq!(bidi, 0, "setup: a left-to-right label went through unicode_bidi");
        assert_eq!((pump, raster), ((0, 0), (0, 0)), "a Complex left-to-right frame touched the allocator");
    }

    /// **Plain Latin under `Complex` takes fontdue's path** — K81a's
    /// response: `PlainApp`'s changing label reaches the shaper 0 times and
    /// allocates nothing, as under `Basic`.
    #[test]
    fn a_changing_latin_label_on_a_complex_stack_takes_fontdue_and_allocates_nothing() {
        let _one = super::lock(&super::truetype_lock::ONE);
        let f = face();
        let mut m = FontMetrics::new(f.clone());
        let mut host: Host<_, Incremental> = Host::new(PlainApp::default(), 320, 200);
        let mut canvas = Canvas::with_font(320, 200, f);
        for i in 0..30 {
            host.app.count = i;
            host.pump_with(&mut m);
            canvas.render(&host.list, 320, 200);
        }
        let t0 = text_counters();
        let (mut pump, mut raster) = ((0, 0), (0, 0));
        for i in 0..50 {
            host.app.count = i % 30;
            let a = counters();
            host.pump_with(&mut m);
            let b = counters();
            canvas.render(&host.list, 320, 200);
            let c = counters();
            pump = (pump.0 + b.0 - a.0, pump.1 + b.1 - a.1);
            raster = (raster.0 + c.0 - b.0, raster.1 + c.1 - b.1);
        }
        let t1 = text_counters();
        println!(
            "Complex PlainApp, 50 changing frames: pump allocs {} frees {}, raster allocs {} frees {} (bound 0 each); \
             {} runs through the shaper (bound 0), {:.2} runs shaped per frame (setup >= 1)",
            pump.0, pump.1, raster.0, raster.1, t1.complex - t0.complex, (t1.shaped - t0.shaped) as f64 / 50.0
        );
        assert!(t1.shaped - t0.shaped >= 50, "setup: the frames painted no text");
        assert_eq!(t1.complex - t0.complex, 0, "plain Latin went through the shaper under Complex");
        assert_eq!((pump, raster), ((0, 0), (0, 0)), "a Complex Latin frame touched the allocator");
    }

    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct RtlUi {
        #[widget_id("title")]
        title: Label,
        #[widget_id("rtl")]
        rtl: Label<String>,
    }

    #[derive(DeriveApp, Default)]
    struct RtlApp {
        #[tree]
        ui: RtlUi,
    }

    /// Two right-to-left values of one shape: same length, same bidi
    /// classes, so each costs `unicode_bidi` the same.
    const VALUES: [&str; 2] = ["\u{5e9}\u{5dc}\u{5d5}\u{5dd} \u{5e2}\u{5d5}\u{5dc}\u{5dd}", "\u{5e2}\u{5d5}\u{5dc}\u{5dd} \u{5e9}\u{5dc}\u{5d5}\u{5dd}"];

    /// What `unicode_bidi` allocates for one single-line run of `text`, as
    /// the shaper calls it: one `BidiInfo`, one `visual_runs`.
    fn bidi_allocs(text: &str) -> usize {
        let a = counters().0;
        let info = unicode_bidi::BidiInfo::new(text, None);
        let runs = info.visual_runs(&info.paragraphs[0], 0..text.len());
        let n = counters().0 - a;
        drop((runs, info));
        n
    }

    /// One frame over its damage; (allocs, frees) of pump and raster.
    fn frame(host: &mut Host<RtlApp, Incremental>, m: &mut FontMetrics, canvas: &mut Canvas) -> ((usize, usize), (usize, usize), usize) {
        let vp = [0.0, 0.0, 320.0, 120.0];
        let a = counters();
        host.pump_with(m);
        let b = counters();
        let mut regions = 0;
        for r in host.take_damage_for_age(1).regions(vp) {
            canvas.clear_within(r, Color(255, 255, 255, 255));
            canvas.render_within(&host.list, r);
            regions += 1;
        }
        let c = counters();
        ((b.0 - a.0, b.1 - a.1), (c.0 - b.0, c.1 - b.1), regions)
    }

    #[test]
    fn right_to_left_frames_allocate_only_inside_unicode_bidi() {
        let _one = super::lock(&super::truetype_lock::ONE);
        let f = face();
        let mut m = FontMetrics::new(f.clone());
        let mut app = RtlApp::default();
        app.ui.title = Label::new("Inbox");
        app.ui.rtl.set(VALUES[0].to_owned());
        let mut host: Host<_, Incremental> = Host::new(app, 320, 120);
        let mut canvas = Canvas::with_font(320, 120, f);
        canvas.clear(Color(255, 255, 255, 255));
        host.pump_with(&mut m);
        canvas.render(&host.list, 320, 120);
        host.take_damage_for_age(1);
        let per_run: Vec<usize> = VALUES.iter().map(|v| bidi_allocs(v)).collect();
        assert!(per_run.iter().all(|&n| n == per_run[0] && n > 0), "setup: the two values cost bidi {per_run:?}");
        let per_run = per_run[0];

        // A changing right-to-left label.
        let change = |host: &mut Host<RtlApp, Incremental>, m: &mut FontMetrics, canvas: &mut Canvas, k: usize| {
            host.app.ui.rtl.set_str(VALUES[k % 2]);
            assert!(host.invalidate("rtl"), "setup: #rtl is not addressable");
            frame(host, m, canvas)
        };
        for k in 0..10 {
            change(&mut host, &mut m, &mut canvas, k);
        }
        let t0 = text_counters();
        let (mut pump, mut raster, mut regions) = ((0, 0), (0, 0), 0);
        for k in 0..40 {
            let (p, r, n) = change(&mut host, &mut m, &mut canvas, k);
            pump = (pump.0 + p.0, pump.1 + p.1);
            raster = (raster.0 + r.0, raster.1 + r.1);
            regions += n;
        }
        let t1 = text_counters();
        let (shaped, bidi) = ((t1.complex - t0.complex) as usize, (t1.bidi - t0.bidi) as usize);
        let allocs = pump.0 + raster.0;
        println!(
            "Complex, changing RTL label, 40 frames over {regions} damage regions: pump allocs {} frees {}, \
             raster allocs {} frees {}; {shaped} runs through the shaper, {bidi} of them ordered by unicode_bidi \
             x {per_run} unicode_bidi allocations per run = {} (bound: allocs == that)",
            pump.0,
            pump.1,
            raster.0,
            raster.1,
            bidi * per_run
        );
        assert!(regions >= 40, "setup: the changes damaged nothing ({regions} regions)");
        assert!(bidi >= 40, "setup: the changed label was not laid out through unicode_bidi ({bidi} of {shaped} runs)");
        assert_eq!(allocs, bidi * per_run, "a changing RTL label allocated outside unicode_bidi");

        // A still right-to-left screen.
        let (mut pump, mut raster) = ((0, 0), (0, 0));
        for _ in 0..40 {
            let (p, r, _) = frame(&mut host, &mut m, &mut canvas);
            pump = (pump.0 + p.0, pump.1 + p.1);
            raster = (raster.0 + r.0, raster.1 + r.1);
        }
        let held = host.list.cmds.iter().any(|c| matches!(c, lumen_static::Cmd::Text { text, .. } if host.list.str_of(*text) == VALUES[1]));
        println!(
            "Complex, still RTL screen, 40 frames: pump allocs {} frees {}, raster allocs {} frees {} (bound 0 each); \
             the list holds the RTL run: {held}",
            pump.0, pump.1, raster.0, raster.1
        );
        assert!(held, "setup: the still screen holds no right-to-left run");
        assert_eq!((pump, raster), ((0, 0), (0, 0)), "a still RTL screen touched the allocator");
    }
}

/// The lock the truetype arms share — `mod truetype`'s, reachable from
/// `mod complex`, whose arms read the same process-wide text counters.
#[cfg(feature = "truetype")]
mod truetype_lock {
    pub static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());
}
