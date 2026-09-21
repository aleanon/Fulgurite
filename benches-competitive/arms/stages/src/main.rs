//! **Where does a frame's time actually go?**
//!
//! The frame arm next door reports four *cumulative* numbers — `layout`,
//! `+paint`, `+render (full)`, `+render (damage)` — because that is the
//! shape Qt's and GTK's harnesses report and a competitive table has to
//! be comparable. Cumulative numbers answer "are we fast"; they do not
//! answer "which stage is it". This arm answers the second question:
//! every stage of one frame, timed separately, over the three axes that
//! change the answer.
//!
//! - **N** — 100, 1000, 3000, 10000 rows.
//! - **Workload** — `one_row` (one row's text changes: a counter, a
//!   clock), `all_rows` (every row changes: the measurement that denies
//!   every framework its text cache), and `idle` (nothing changes). The
//!   frame arm measures all three, as its `best`, `best_all` and floor
//!   loops. `idle` is the only one that reaches `Damage::None`, which is
//!   what makes the `skipped` accounting checkable rather than assumed.
//! - **Retention** — `Rebuild` and `Incremental`, the type parameter
//!   from `docs/plan-architecture-v2.md` M8b.
//!
//! # The stages, and what each one brackets
//!
//! | stage | what runs inside the bracket |
//! |---|---|
//! | `dispatch`  | `pass::dispatch` for a `PointerDown` + `PointerUp` pair, which hit-tests into the list |
//! | `update`    | `App::update` — the binding step, model to widget |
//! | `layout`    | `Node::layout` on the root |
//! | `place`     | `Node::place` on the root |
//! | `paint`     | `DisplayList::clear` plus `Node::paint` through `Scaled` and `Recording` |
//! | `damage`    | `Retention::finish` |
//! | `raster.sw` | `Canvas::clear_within` plus `Canvas::render_within`, over the damage rect |
//! | `blit`      | `Canvas::blit_0rgb_within`, over the same rect — the shell's present-side copy |
//! | `raster.gpu`| `Gpu::draw` plus `Gpu::wait`, over the same list and rect. **Not in any sum**: it is an alternative to `raster.sw`, not an addition to it |
//!
//! The first six are exactly what `Host::pump_with` does, in the order it
//! does it, and the `pump` row measures that function independently so a
//! reader can see whether this decomposition is faithful to the shipped
//! loop.
//!
//! # Honesty about skipped work
//!
//! A stage is timed **as executed**, never as a hypothetical. Under
//! `Incremental` with one row changing, the damage rect is a few hundred
//! pixels and `raster.sw` rasterises only those — so the number is small
//! *because the work was not done*, and the `px` column says how many
//! pixels were actually touched out of the 320 000 in the viewport. When
//! the damage is empty the stage does nothing at all and the note reads
//! `skipped-nothing-changed`. Under `Rebuild` the damage is always
//! `Damage::All`, so the same stage rasterises the whole viewport and
//! the note reads `full-viewport`.
//!
//! # Reading the `damage` line — it is not decoration
//!
//! Every cell prints the `Damage` retention reported and the pixel count
//! that follows from it, because under `Incremental` the raster stages
//! mean nothing without it. It also exposes something a cost table
//! alone would hide, seen here on the very first run and reproduced
//! deterministically with no timing involved:
//!
//! **A one-row change damages the whole container whenever it changes
//! the container's intrinsic width.** `List::paint` wraps its items in
//! `push_clip(rect_of(...))` of the list node, and the list node's width
//! is the widest row. Row 0's text changing width by a single pixel
//! therefore rewrites that clip, and the frame's damage becomes the
//! whole column: with the system face this arm shapes with, N = 100
//! alternates between 57 600 px and 1 562 px — the latter being the area
//! the changed row actually occupies — depending on which digits the row
//! happens to contain. Pinning the column's width with a wider sibling
//! row makes it the small figure on every frame, which is the check that
//! identified the clip as the cause. The two figures are this arm's own
//! deterministic output and are reproduced by running it; no timing is
//! involved in either.
//!
//! Nothing here is a defect of the *measurement*; it is the framework
//! being measured, and it is why this arm reports pixels beside
//! microseconds. It also bears on `docs/results-competitive.md`'s
//! `+render (damage)` row, whose min-of-200 will preferentially select
//! the frames where the width did not change.
//!
//! # The accounting, at three levels
//!
//! Stages must add up to something a reader can check, so three sums are
//! printed beside three independently-measured wholes:
//!
//! - `sum` (stages 1-8) against `frame`, an outer timer around the same
//!   frame function in its own loop;
//! - `sum` against `frame.inner`, the per-iteration total from the
//!   instrumented loop — this pair isolates the *min-of-mins* effect,
//!   because each stage's minimum may come from a different iteration
//!   and the sum of minima is therefore a lower bound on any single
//!   frame;
//! - `sum.pump` (stages 1-6) against `pump`, `Host::pump_with` measured
//!   in its own loop on its own `Host`. This one is a **fidelity** check
//!   rather than an accounting one — it asks whether the six brackets
//!   above add up to the loop the framework actually ships — and it is
//!   not expected to be zero: `pump_with` also pops its events off an
//!   `InputQueue` and runs the focus-ring fixup, and the arm's six
//!   stages are cut by five extra `Instant::now()` calls, which are
//!   barriers as well as timers.
//!
//! An unexplained gap is a finding, not a rounding error. The known
//! contributors are named in the header the program prints, so a reader
//! can see which part of a residual is already spoken for.
//!
//! # Method
//!
//! Min of `iters` after `warm` warm-ups, per stage, per cell — the same
//! discipline as the frame arm (which uses 200 and 20, and so does this
//! by default). Min rather than mean: everything above the minimum is
//! something else the machine was doing.
//!
//! Run it with `just stages`, or:
//!
//! ```text
//! cd benches-competitive
//! cargo run --release -p lumen-static-stages -- [Ns] [iters] [warm]
//! # e.g. cargo run --release -p lumen-static-stages -- 100,1000 50 5
//! # LUMEN_NO_GPU=1 skips the GPU stage without skipping the rest.
//! ```

use std::fmt::Write as _;
use std::time::Instant;

use lumen_static::event::InputState;
use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;
use lumen_static::{Damage, DisplayList, Incremental, LayoutCx, Rebuild, Recording, Retention, Scaled};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::{FontMetrics, FontStack};
use lumen_static_shell::{Host, CLEAR};
use lumen_static_wgpu::Gpu;

/// The raster surface: the same 400x800 the frame arm and
/// `docs/results-competitive.md` use, so `raster.sw` here and
/// `+render` there describe the same pixels.
const W: u32 = 400;
const H: u32 = 800;

/// The height the column is laid out against.
///
/// **Not `H`, and not infinite.** `List::layout` hands each item
/// `max_h - sum`, so laying 10 000 rows out against 800 px squashes
/// everything past the fortieth into zero height — a different and much
/// cheaper layout problem, which is the trap the frame arm's comment
/// about natural row heights warns about. It is also not `f32::INFINITY`
/// (which is what the frame arm passes) because `Host::limits` derives
/// its constraints from the window size and cannot express one: a finite
/// bound large enough never to bind is the only value the arm and the
/// `Host` can *both* use, and the `tree_h` printed for every cell is
/// there so a reader can confirm it never came close.
const LAYOUT_H: u32 = 1 << 22;

/// Where the dispatched pointer events land: inside the list, a few rows
/// down. `List::hit` scans items in order, so a hit near the top is the
/// cheap end of that walk — stated because it is a property of the
/// number, not a property of the framework.
const HIT: (f32, f32) = (200.0, 120.0);

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    rows: List<Label<String>>,
}

/// The application: a model, and the tree that presents it.
///
/// `App::update` is written by hand rather than by `#[bind(...)]`
/// because there is no binding attribute for a list — but it does
/// exactly what a generated binding does, `set_if_changed` per target,
/// comparing the *formatted output* rather than trusting a dirty bit
/// (`lumen_static::bind`). That comparison is why the `update` stage
/// costs O(N) even in the `one_row` workload, and reporting that is half
/// the point of having the stage.
#[derive(Default)]
struct Rows {
    ui: Ui,
    model: Vec<String>,
}

impl App for Rows {
    // M28/T28.2: this fixture defers no work. `Never` is
    // uninhabited, so nothing can be sent and `apply` cannot
    // be called.
    type Msg = lumen_static::event::Never;
    fn apply(&mut self, m: Self::Msg) -> bool {
        match m {}
    }
    type Tree = Ui;

    fn tree(&self) -> &Ui {
        &self.ui
    }

    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }

    fn update(&mut self) -> bool {
        // Reports whether ANY row changed (T16.10). `|=` rather than a
        // short-circuiting `||`: every row must still be written, and
        // this is the binding step the profiler is timing.
        let mut changed = false;
        for (dst, src) in self.ui.rows.items.iter_mut().zip(self.model.iter()) {
            changed |= lumen_static::bind::set_if_changed(dst, src);
        }
        changed
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Load {
    /// Nothing changes.
    ///
    /// The frame arm reports an idle floor for the same reason and says
    /// why: a framework that spends 251 µs on a frame where nothing
    /// happened is telling you something that a "cost of the change"
    /// number alone would hide. Here it does a second job — it is the
    /// only workload that reaches `Damage::None`, so it is what proves
    /// the `skipped` accounting is real and not a branch nobody takes.
    Idle,
    /// One row of N changes — a counter, a clock, a progress bar.
    OneRow,
    /// Every row changes — the frame no text cache can serve.
    AllRows,
}

impl Load {
    fn name(self) -> &'static str {
        match self {
            Load::Idle => "idle",
            Load::OneRow => "one_row",
            Load::AllRows => "all_rows",
        }
    }
}

/// Apply the workload to the model. **Outside every timed bracket**, and
/// in place: `format!` per row per iteration would allocate 10 000 times
/// a frame and put an allocator in the middle of a measurement that is
/// not about one.
fn mutate(model: &mut [String], load: Load, k: usize) {
    match load {
        Load::Idle => {}
        Load::OneRow => {
            if let Some(s) = model.first_mut() {
                s.clear();
                let _ = write!(s, "row 0 - {k}");
            }
        }
        Load::AllRows => {
            for (i, s) in model.iter_mut().enumerate() {
                s.clear();
                let _ = write!(s, "row {i} - {k}");
            }
        }
    }
}

/// The eight stages that sum, in the order a frame runs them.
const STAGES: [&str; 8] =
    ["dispatch", "update", "layout", "place", "paint", "damage", "raster.sw", "blit"];

/// Everything one cell's frame needs, so the frame body exists **once**
/// and both the instrumented loop and the independent whole-frame loop
/// call the same code. Two copies of a frame loop is two things to keep
/// in step, and the one that drifts is always the one nobody reads.
struct Rig<R: Retention> {
    app: Rows,
    styles: Vec<Style>,
    states: Vec<NodeState>,
    list: DisplayList,
    retain: R,
    input: InputState,
    metrics: FontMetrics,
    canvas: Canvas,
    /// The shell's 0RGB surface buffer — what `blit` writes into.
    buf: Vec<u32>,
    /// The damage rect the last frame actually rasterised, clipped to
    /// the viewport. `None` means nothing was drawn.
    last_area: Option<[f32; 4]>,
    /// What retention reported for the last frame, before clipping --
    /// `Damage::All` reads differently from a rect that happens to cover
    /// the viewport, and the difference is the whole `Rebuild` story.
    last_damage: Damage,
}

impl<R: Retention> Rig<R> {
    fn new(n: usize, font: &FontStack) -> Self {
        let model: Vec<String> = (0..n).map(|i| format!("row {i}")).collect();
        let items: Vec<Label<String>> = model.iter().map(|s| Label::new(s.clone())).collect();
        let mut app = Rows { ui: Ui::default(), model };
        app.ui.rows.items = items;
        let styles = pass::default_styles(&app.ui);
        Rig {
            styles,
            states: vec![NodeState::default(); <Ui as Widget>::NODE_COUNT],
            list: DisplayList::new(),
            retain: R::default(),
            input: InputState::default(),
            metrics: FontMetrics::new(font.clone()),
            canvas: Canvas::with_font(W, H, font.clone()),
            buf: vec![0u32; (W * H) as usize],
            last_area: None,
            last_damage: Damage::None,
            app,
        }
    }

    fn limits() -> Constraints {
        Constraints { min_w: 0.0, max_w: W as f32, min_h: 0.0, max_h: LAYOUT_H as f32 }
    }

    /// One whole frame, stamping `ts[0..=8]` around the eight stages.
    ///
    /// Stages 1-6 mirror `Host::pump_with` call for call and in its
    /// order. What `pump_with` also does and this does not: pop the
    /// events off an `InputQueue` (they are handed to `dispatch`
    /// directly here) and zero the focused node's `focus_ring` when
    /// focus is not keyboard-visible. Both are named in the header as
    /// part of the `pump` gap rather than smuggled into a stage.
    fn frame(&mut self, ts: &mut [Instant; 9]) {
        ts[0] = Instant::now();

        // 1. dispatch — a press and a release, hit-tested into the list.
        for ev in [Event::down(HIT.0, HIT.1), Event::up(HIT.0, HIT.1)] {
            pass::dispatch(&mut self.app, &self.styles, &mut self.states, &mut self.input, &ev);
        }
        ts[1] = Instant::now();

        // 2. the binding step.
        self.app.update();
        ts[2] = Instant::now();

        // 3. layout.
        {
            let mut cx = LayoutCx::new(&mut self.metrics);
            self.app.ui.layout(&mut cx, Self::limits(), &self.styles, &mut self.states, 0);
        }
        ts[3] = Instant::now();

        // 4. place.
        self.app.ui.place(Point::default(), &self.styles, &mut self.states, 0);
        ts[4] = Instant::now();

        // 5. paint — the display-list build, through the same two
        //    wrappers the host uses. At a scale of 1.0 `Scaled` forwards,
        //    and under `Rebuild` `Recording` forwards to an empty
        //    `observe`; both are in the bracket because both are in the
        //    shipped frame.
        self.list.clear();
        {
            let mut scaled = Scaled::new(&mut self.list, 1.0);
            let mut out = Recording::new(&mut scaled, &mut self.retain);
            self.app.ui.paint(&self.states, 0, &mut out);
        }
        ts[5] = Instant::now();

        // 6. what changed.
        let damage = self.retain.finish();
        ts[6] = Instant::now();

        // 7. the software raster, over exactly what changed.
        let area = damage.clip_to([0.0, 0.0, W as f32, H as f32]);
        if let Some(a) = area {
            self.canvas.clear_within(a, CLEAR);
            self.canvas.render_within(&self.list, a);
        }
        ts[7] = Instant::now();

        // 8. the present-side copy into the surface buffer.
        if let Some(a) = area {
            self.canvas.blit_0rgb_within(&mut self.buf, a);
        }
        ts[8] = Instant::now();

        self.last_area = area;
        self.last_damage = damage;
    }
}

fn us(d: std::time::Duration) -> f64 {
    d.as_secs_f64() * 1e6
}

/// One (N, workload, retention) cell's results.
struct Cell {
    /// Minimum observed cost of each of [`STAGES`], microseconds.
    stage: [f64; 8],
    /// Minimum of the per-iteration totals from the instrumented loop.
    inner: f64,
    /// Minimum whole frame from an independent loop with one outer timer.
    frame: f64,
    /// Minimum `Host::pump_with`, from its own loop on its own `Host`.
    pump: f64,
    /// Minimum `Gpu::draw` + `Gpu::wait`; `None` when there is no
    /// adapter, or when every frame's damage was empty and the stage
    /// therefore never ran. `gpu_available` tells the two apart.
    gpu: Option<f64>,
    gpu_available: bool,
    /// Pixels the GPU stage drew, measured in its own loop.
    gpu_px: u64,
    /// Pixels the raster stages actually touched.
    px: u64,
    /// Commands in the frame's display list.
    cmds: usize,
    /// The laid-out height of the whole column, so a reader can see that
    /// `LAYOUT_H` never bound.
    tree_h: f32,
    damage: Damage,
}

fn run_cell<R: Retention>(
    n: usize,
    load: Load,
    font: &FontStack,
    gpu: Option<&mut Gpu>,
    iters: usize,
    warm: usize,
) -> Cell {
    let mut rig: Rig<R> = Rig::new(n, font);
    let mut ts = [Instant::now(); 9];

    // Warm-up: the shaping cache, the allocator's high-water mark, the
    // retention buffer's growth, and the branch predictor. The first
    // frame's damage is `All` under either retention because nothing is
    // retained yet, so a measured first frame would be measuring that.
    for k in 0..warm {
        mutate(&mut rig.app.model, load, k);
        rig.frame(&mut ts);
    }

    // Loop A: per-stage minima, and the per-iteration total.
    //
    // **Every loop below replays this same `k` range**, not a fresh one.
    // The workload is a function of `k`, and so therefore is the damage:
    // give loop B a different range and it can rasterise a different
    // number of pixels than loop A did, at which point `frame` and `sum`
    // are two numbers about two frames and the accounting check compares
    // nothing. Caught by running with one warm-up, where the two ranges
    // straddled the frame at which the column's width settled and the
    // two differed by 15x.
    let mut stage = [f64::INFINITY; 8];
    let mut inner = f64::INFINITY;
    for k in warm..(warm + iters) {
        mutate(&mut rig.app.model, load, k);
        rig.frame(&mut ts);
        for i in 0..8 {
            stage[i] = stage[i].min(us(ts[i + 1] - ts[i]));
        }
        inner = inner.min(us(ts[8] - ts[0]));
    }

    // The shape of the frame, read after the timed loop so nothing here
    // is inside a bracket.
    let area = rig.last_area;
    let px = area.map_or(0, |a| ((a[2] - a[0]) * (a[3] - a[1])).max(0.0) as u64);
    let cmds = rig.list.cmds.len();
    let tree_h = rig.states[0].size.h;
    let damage = rig.last_damage;
    assert!(cmds > 0, "the frame drew nothing, so it measured nothing");

    // Loop B: the whole frame, one outer timer, no per-stage stamps.
    let mut frame = f64::INFINITY;
    for k in warm..(warm + iters) {
        mutate(&mut rig.app.model, load, k);
        let t = Instant::now();
        rig.frame(&mut ts);
        frame = frame.min(us(t.elapsed()));
    }

    // Loop C: the shipped frame loop, on its own `Host`, so the
    // decomposition above has something to be checked against that this
    // file did not write.
    //
    // The `Host` is constructed 400 x LAYOUT_H because `Host::limits` is
    // the surface size divided by the scale; nothing is *presented*
    // through it, so the height is a constraint and not a viewport.
    //
    // **The raster runs here too, untimed.** `Host::pump_with` stops at
    // the display list, so the obvious loop -- pump, pump, pump -- keeps
    // the tree, the styles and the state array resident and measures
    // layout and paint on a cache no real frame ever has. The frame this
    // arm decomposes writes 320 000 pixels between one layout and the
    // next. Rastering the host's list the same way is what makes `pump`
    // a like-for-like check on the decomposition instead of a floor for
    // a different program: the two loops then differ only in where the
    // timers are.
    let mut host: Host<Rows, R> = {
        let mut rig2: Rig<R> = Rig::new(n, font);
        let app = std::mem::take(&mut rig2.app);
        Host::new(app, W, LAYOUT_H)
    };
    let mut hm = FontMetrics::new(font.clone());
    let mut pump = f64::INFINITY;
    for k in 0..(warm + iters) {
        mutate(&mut host.app.model, load, k);
        host.post(Event::down(HIT.0, HIT.1));
        host.post(Event::up(HIT.0, HIT.1));
        let t = Instant::now();
        host.pump_with(&mut hm);
        if k >= warm {
            pump = pump.min(us(t.elapsed()));
        }
        if let Some(a) = host.damage().clip_to([0.0, 0.0, W as f32, H as f32]) {
            rig.canvas.clear_within(a, CLEAR);
            rig.canvas.render_within(&host.list, a);
            rig.canvas.blit_0rgb_within(&mut rig.buf, a);
        }
    }

    // The GPU raster, over the same list and the same rect. Its own
    // loop: it is an alternative to `raster.sw`, so timing it inside the
    // frame would double-count the raster and inflate every sum.
    //
    // `wait` after every draw, deliberately. `draw` returns as soon as
    // the work is *queued*; a benchmark that did not wait would be
    // timing a queue submission and calling it a render, which is the
    // note `Gpu::wait` itself carries.
    //
    // No separate warm-up: the glyph atlas, the quad buffer and the
    // pipeline are per-`Gpu`, and the `Gpu` is built once for the whole
    // run and reused by every cell, so it is warm before the first cell
    // finishes. The **first** cell's GPU row is the one to distrust.
    let gpu_available = gpu.is_some();
    let mut gpu_px = 0u64;
    let gpu_us = gpu.and_then(|g| {
        let mut best = f64::INFINITY;
        for k in warm..(warm + iters) {
            mutate(&mut rig.app.model, load, k);
            rig.frame(&mut ts);
            let Some(a) = rig.last_area else { continue };
            // What this stage drew, reported from this stage rather than
            // borrowed from loop A's last frame: a px column that came
            // from a different loop is a px column that can be wrong.
            gpu_px = ((a[2] - a[0]) * (a[3] - a[1])).max(0.0) as u64;
            let t = Instant::now();
            g.draw(&rig.list, Some(a), Some((CLEAR, a)));
            g.wait();
            best = best.min(us(t.elapsed()));
        }
        best.is_finite().then_some(best)
    });

    Cell {
        stage,
        inner,
        frame,
        pump,
        gpu: gpu_us,
        gpu_available,
        gpu_px,
        px,
        cmds,
        tree_h,
        damage,
    }
}

/// One data line. Columns are whitespace-separated and no field contains
/// whitespace, so `awk '!/^#/ {print $1, $5}'` is a valid reader; the
/// padding is only there for a human.
fn row(stage: &str, n: usize, load: Load, ret: &str, us: f64, px: &str, note: &str) {
    println!(
        "{stage:<12} {n:>6} {:<9} {ret:<12} {us:>10.2} {px:>9}  {note}",
        load.name()
    );
}

fn report(n: usize, load: Load, ret: &str, c: &Cell) {
    let total_px = (W as u64) * (H as u64);
    println!(
        "\n# cell n={n} workload={} retention={ret} cmds={} tree_h={:.0} \
         damage={:?} damaged_px={} of {total_px}",
        load.name(),
        c.cmds,
        c.tree_h,
        c.damage,
        c.px,
    );

    let raster_note = if c.px == 0 {
        "skipped-nothing-changed"
    } else if c.px >= total_px {
        "full-viewport"
    } else {
        "damage-clipped"
    };
    for (i, name) in STAGES.iter().enumerate() {
        let (px, note) = match i {
            6 | 7 => (c.px.to_string(), raster_note),
            _ => ("-".to_string(), "-"),
        };
        row(name, n, load, ret, c.stage[i], &px, note);
    }

    let sum8: f64 = c.stage.iter().sum();
    let sum6: f64 = c.stage[..6].iter().sum();
    row("sum", n, load, ret, sum8, "-", "sum-of-stage-minima-1..8");
    row("frame.inner", n, load, ret, c.inner, "-", "instrumented-per-iteration-total");
    row("frame", n, load, ret, c.frame, "-", "independent-outer-timer");
    row("sum.pump", n, load, ret, sum6, "-", "sum-of-stage-minima-1..6");
    row("pump", n, load, ret, c.pump, "-", "Host::pump_with-independent");
    match c.gpu {
        Some(g) => row("raster.gpu", n, load, ret, g, &c.gpu_px.to_string(), "NOT-in-any-sum"),
        None if c.gpu_available => {
            row("raster.gpu", n, load, ret, f64::NAN, "0", "skipped-nothing-changed")
        }
        None => row("raster.gpu", n, load, ret, f64::NAN, "-", "no-adapter"),
    }

    // The gaps, printed rather than left for the reader to subtract --
    // a gap nobody computes is a gap nobody notices.
    println!(
        "# gap n={n} workload={} retention={ret} \
         frame-sum={:+.2}us inner-sum={:+.2}us pump-sum6={:+.2}us",
        load.name(),
        c.frame - sum8,
        c.inner - sum8,
        c.pump - sum6,
    );
}

/// Cost of one `Instant::now()`, so a reader can size the instrumentation
/// against the stages it separates. Nine of these are inside every
/// instrumented frame.
fn timer_overhead_ns() -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..1000 {
        let a = Instant::now();
        let b = Instant::now();
        best = best.min((b - a).as_secs_f64() * 1e9);
    }
    best
}

fn main() {
    let ns: Vec<usize> = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "100,1000,3000,10000".into())
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let iters: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(200);
    let warm: usize = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(20);

    let font = FontStack::system().expect("a system font: every stage here shapes text for real");

    // The GPU is optional by construction, not by feature: no adapter is
    // the configuration the software canvas exists for, and this arm
    // reports it as a `no-adapter` note rather than failing.
    let mut gpu = if std::env::var_os("LUMEN_NO_GPU").is_some() {
        None
    } else {
        Gpu::headless(W, H).map(|mut g| {
            g.set_font(font.clone());
            g
        })
    };
    let adapter = gpu.as_ref().map_or_else(
        || {
            if std::env::var_os("LUMEN_NO_GPU").is_some() {
                "disabled (LUMEN_NO_GPU)".to_string()
            } else {
                "none (no adapter)".to_string()
            }
        },
        |g| g.adapter().to_owned(),
    );

    println!("# lumen-static per-stage frame profiler (benches-competitive/arms/stages)");
    println!("# method: min of {iters} iterations after {warm} warm-ups, per stage, per cell");
    println!("# raster surface: {W}x{H}; layout limits: {W} x {LAYOUT_H} (natural row heights)");
    println!("# gpu adapter: {adapter}");
    println!("# Instant::now(): {:.1} ns, nine of them per instrumented frame", timer_overhead_ns());
    println!("# stages 1-8 sum; raster.gpu is an ALTERNATIVE to raster.sw and is in no sum");
    println!("# reading the gaps:");
    println!("#   frame-sum   the accounting check. `sum` adds eight minima that may each come");
    println!("#               from a different frame, so it is a LOWER bound and this should be");
    println!("#               a small positive number. A large one is unaccounted work.");
    println!("#   inner-sum   the same check within one loop; isolates min-of-minima alone.");
    println!("#   pump-sum6   a FIDELITY check, not an accounting one: stages 1-6 against");
    println!("#               Host::pump_with, which is the shipped loop this arm re-creates.");
    println!("#               It is not expected to be zero. Host::pump_with also pops its");
    println!("#               events off an InputQueue and runs the focus-ring fixup, neither");
    println!("#               of which the arm brackets; and the arm's stages 1-6 are cut by");
    println!("#               five extra Instant::now() calls, which are barriers as well as");
    println!("#               timers. Its loop rasterises the host's list untimed so that both");
    println!("#               sides meet the same cache. A residual beyond those is a finding.");
    println!("# columns: stage n workload retention us px note");
    println!("# a NaN in the us column means the stage did not run; the note says why.");

    for &n in &ns {
        for load in [Load::Idle, Load::OneRow, Load::AllRows] {
            let c = run_cell::<Rebuild>(n, load, &font, gpu.as_mut(), iters, warm);
            report(n, load, "rebuild", &c);
            let c = run_cell::<Incremental>(n, load, &font, gpu.as_mut(), iters, warm);
            report(n, load, "incremental", &c);
        }
    }
}
