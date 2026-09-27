# Zero allocation in steady state — measured, and now gated

> **Machine.** i9-13900KF, governor `powersave`, turbo on, `tsc`
> clocksource, rustc 1.94.0. Absolute figures are not portable across
> machines or governors; ratios taken inside one run are. Recorded from
> 2026-09-02, when it turned out no results doc had said.

*2026-09-02, plan `docs/plan-review-fixes.md` F6.9. Harness:
`crates/lumen-static-shell/tests/alloc_gate.rs`, a counting
`#[global_allocator]` in its own test binary. Machine-independent: the
numbers here are integers.*

## The claim

`docs/00-handoff-static-tree.md` §10: *"Zero allocation for the entire UI
— not 'few', zero."* It is the property that makes the no-allocator
profile possible at all, and it is stated as a design constraint
throughout the handoff.

**Nothing checked it.** A counting allocator was reached for three times
by hand during the 2026-09-02 review, and each time it found something:

| found | where | fixed |
|---|---|---|
| 2 allocations per enum node per `layout` | the derive's scratch arena | F4.3 |
| 3 collections per frame + a `String`/`Value` per finding | the agent's journal and audit | F2.2, F2.3 |
| 1002 allocations, 9,906 bytes leaked per 1000 bound updates | `Label`'s default storage under `#[bind]` | T6.2 fix, F1.x era |

All three had shipped past a green gate.

## Results

Allocations per `Host::pump`, after five warmup frames, averaged over
100:

| path | per frame |
|---|---|
| plain tree (leaves + derived containers), idle | **0** |
| plain tree, a state change through `#[bind]` every frame | **0** |
| `Repeat` + `List` + `Either` + `Dyn` + `Option`, idle | **0** |
| `Repeat` over 10,000 rows, scrolled one row per frame | **0** |
| `Repeat` whose bind **writes text**, idle (M29/T29.5) | **0** |
| two enum-widget nodes, idle | **0** |

**The fourth row was added 2026-09-21 and is the one to read.** Every
repeater arm above it binds with `noop_bind`, so this file's zero for a
repeater was a zero for a list *whose rows are never filled in* —
`AGENT.md`'s first vacuous shape, because the only thing that allocates
on a repeater frame is a bind that writes. A repeater binds **every
materialized row on every frame**, so a bind is steady-state code, and
the obvious spelling of one allocates:

| the bind | allocs/frame, 19-row window |
|---|---|
| `row.name.set(format!("row {i}"))` | **19** (one per row) |
| `format_into(&mut row.name, format_args!("row {i}"))` | **0** |

`format_into` is the same machinery `#[bind]` generates for a table
node: a reused scratch buffer, and a comparison before the store. `set`
also compares — but `format!` has already allocated by the time `set`
sees the string, and no comparison undoes that. The allocating form is
what this gate's injection uses, and it costs 10.8 µs a frame on an
18-row window (`docs/results-damage-precision.md` T29.5).

And from `benches --bin agent`, for the dev surface:

| path | per frame |
|---|---|
| `Agent::frame_done`, few findings | **0** |
| `Agent::frame_done`, 132 findings | **0** |

## The number is zero, not a budget

Every path measures zero, so a budget would only encode a bug. That is
the whole value of asserting an integer: it cannot drift quietly the way
a microsecond figure does, it needs no reference machine, and a
regression is a failed test rather than a number someone has to notice
got worse.

## Two things that make this test correct rather than flaky

**Its own binary.** `#[global_allocator]` is per-binary, so folding this
into another test file would tax every test in it.

**A thread-local counter, not a global one.** `cargo test` runs tests in
parallel threads within one process, so a global counter measures every
*other* concurrently-running test's allocations too. An earlier version
of this file used an `AtomicUsize` and reported 0.16 allocations per
frame from code that allocates none — and reported *more* the longer it
warmed up, because more of the suite was running alongside. The
allocator hook uses `try_with`, since during thread teardown the
thread-local is gone and a panic in the allocator is not recoverable.

## Verified to have teeth

Planting one `Vec::with_capacity(8)` in `Host::pump` fails all five
tests. A gate that cannot fail is not a gate — the same lesson as
`table_is_well_formed` (`docs/results-const-eval.md`).

## The blind spot this table had until 2026-09-19 — M26/T26.13

**Every zero above was measured over `Host::pump`**, and `pump` stops at
the display list. The software rasteriser was outside all of it: the
clip stack, the blur's scratch buffers, the capture accumulator. A table
of zeros, and a whole stage of the frame that no row covered.

Extended, and the first run of the new rows is the measurement:

| arm | before | after |
|---|---:|---:|
| `rasterising_a_block_glyph_frame_allocates_nothing` (named `rasterising_a_frame_allocates_nothing` until M50/T50.1), 50 frames, **block glyphs** | **50 — 1.00 per frame** | **0** |
| `a_frame_carrying_glass_allocates_nothing`, 20 frames × 2 panels | **120 — 6.00 per frame, 3.00 per panel** | **0** |

1.00 per frame was `Vec::with_capacity(4)` for the clip stack; 3.00 per
panel was the blur's capture, its accumulator and its ping-pong target.
The fixture carries **two** panels deliberately, so "per command" is
distinguishable from "per frame" — which is what makes 3.00 a diagnosis
rather than a number. `docs/results-frame-and-types.md` T26.13 has the
rest, including the two injections that were themselves wrong first: an
unused `Vec::with_capacity` LLVM elided, and a fix that took the blur's
buffers and never put them back.

**The lesson is the table's shape, not the two defects.** A gate named
"a frame allocates nothing" that measures one stage of the frame reads
as covering all of it. What a gate measures belongs in its own name.

## The truetype path was not covered — M50/T50.1 (2026-09-25)

*Closed by M50/T50.2; the next section has the after. Kept as written,
because it is the measurement the fix was judged against.*

**The "after: 0" above is true for block glyphs only**, and the lesson
the previous paragraph draws was broken by the arm it was drawn from.
`rasterising_a_block_glyph_frame_allocates_nothing` renders on
`Canvas::new`, which draws the block-glyph stand-in, and pumps with
`BlockMetrics`. Every window renders on `Canvas::with_font` and measures
with `FontMetrics`, and nothing in this file measured that path until
M38 did (`docs/results-deep-review.md`, R8-F1).

Re-measured at `0091266` for T50.1, the gate's own `PlainApp` fixture,
320×200, 30 warm frames then 50 counted, on the frame thread:

| path | `pump` allocs | raster allocs | raster frees |
|---|---:|---:|---:|
| `Canvas::new` + `pump()` (block glyphs — what the gate draws) | 0 (0.00/frame) | 0 (0.00/frame) | 0 |
| `Canvas::with_font` + `pump_with(FontMetrics)` (truetype — what a window draws) | 230 (**4.60/frame**) | 1,290 (**25.80/frame**) | 1,290 |

Both truetype figures are the review's, unchanged. The sources are two
fontdue `Layout`s per run (the run and `cap_shift`'s probe), the
placement and glyph `Vec`s, and fontdue's accumulator and coverage
`Vec` per glyph; `measure_run` builds one more `Layout` per measure on
the pump side. The review's larger arms (a 3000-row list, `TextInput`
per event) are in `docs/results-deep-review.md` §2.2 and are not
restated here.

**So the zero this file states is a zero for block glyphs, and the
truetype path allocates on every frame that draws or measures text.**
M50 owns the fix: a glyph cache and reusable shaping scratch
(`docs/plan-raster-allocation.md` T50.2), with truetype arms in
`alloc_gate.rs` that can fail. Until then no sentence in this repository
should say a *window's* frame allocates nothing.

## The truetype path, covered — M50/T50.2 (2026-09-27)

Three arms in `alloc_gate.rs` (`mod truetype`), on the pinned face,
`Canvas::with_font` and `pump_with(FontMetrics)`, each asserting that
the frames shaped, measured and drew text before it reads a count. The
"before" column is T50.0's red run of the same arms at `80ff4c1`:

| arm | before: pump / raster allocs | after |
|---|---:|---:|
| `a_truetype_frame_allocates_nothing` — `PlainApp`, 50 changing frames | 230 / 1,290 (**4.60 / 25.80 per frame**) | **0 / 0** |
| `a_truetype_list_with_one_row_changed_allocates_nothing` — 3000 rows, 50 frames over the damage | 0 / 7,800 (**156.00 per frame**) | **0 / 0** |
| `truetype_text_input_events_allocate_nothing`, `Text` then `Backspace`, 20 cycles | 240 / 1,720 | **0 / 0** |
| the same, `Left` then `Right` | 0 / 1,800 | **0 / 0** |
| the same, a paste then five `Backspace`s | 720 / 1,720 | **0 / 0** |

Frees match allocations on every row. What made each zero:

- **The canvas keeps a coverage cache** keyed on `GlyphId` — one arena
  and an index, sized once — so a glyph is rasterised the first time it
  is drawn and blitted from the cache after. Forcing every lookup to miss
  puts **15.20 allocations a frame** back on the first arm (fontdue's
  two per glyph, 7.60 glyphs a frame).
- **The canvas shapes into a layout and placement buffer it keeps**, and
  a cap-shift table replaces the second layout per run. A fresh layout
  per run puts **4.60 a frame** back on the first arm and 18.00 on the
  list.
- **`FontMetrics` measures into a layout it keeps.** Without it the first
  arm's pump reads **230** again.

The TextInput arm is written as cycles that leave the field as they
found it: an edit that lengthens the string allocates for the string,
which is the application's data and not the frame.

**What is still outside it**, each stated where it happens:

- **A run no primary face covers** — a fallback face or a substituted
  character — collects its faces and segments, which allocates
  (`FontStack::prepare`). Latin text on the default stack does not take
  that path.
- **A glyph too large to keep** (over an eighth of the cache's arena) is
  rasterised and drawn without being kept, which allocates; it is
  counted as `CacheStats::uncached`, and `just grid` and showcase's
  headless frame print it at 0 after warm-up.
- **The threaded rasteriser** — `threads`, default-off — until
  M50/T50.3 and T50.4.

## The threaded rasteriser's budget — M50/T50.4 (2026-09-27)

**A decision, not a zero.** `render_banded` (`threads`, default-off)
spawns its workers in a `std::thread::scope` on every banded frame, and
`std` allocates **3 per worker + 1 per scope** on the frame thread for
that, freeing as many. `alloc_gate.rs` could not see it — it counts per
thread and its 320×200 fixtures never band (R18-F4) — so
`tests/alloc_gate_threads.rs` is its own binary with a process-wide
counter, asserting the canvas bands before it reads anything:

| | before (T50.0, block glyphs / pinned face) | after |
|---|---:|---:|
| frame thread, per banded frame at 8 bands | 25 / 25 | **25, bound exactly 3 × 8 + 1** |
| a worker's own code | 0 / **5,350** (shaping and rasterising) | **0** (T50.3) |
| `std` starting a thread | occasional | occasional, printed |

**Why not zero.** The plan's option (A) was a pool created once. Without
`unsafe`, which the shell forbids, a pool cannot borrow a frame's list
and pixel rows, so each banded frame it must copy the rows out and back.
`benches --bin banded_pool`, 1080p, 8 runs, quiet: the scoped banded
frame is **215.7 µs** median with a spread of 58.8; the spawns alone are
95.6; **two row copies are 614.6**. (A)'s overhead is at least copies −
spawns = 519 µs, 8.8× the spread — the kill rule in
`docs/plan-raster-allocation.md` §5 — so (B) ships: the budget.

**What the budget still catches.** The frame thread's count is asserted
equal to the budget, so an extra spawn reads 28 against 25; a worker's
own code (marked by the test-only `software::in_band_worker`) is bound
at zero, so the review's I2, one `Vec` per strip, reads 32 a frame. The
remainder is `std`'s stack-overflow handler registering each new thread
in a `BTreeMap`, whose node splits land on a spawned thread now and then
(a 544 B leaf and a 640 B internal node, by backtrace); it is printed, not
bounded, because it is `std`'s and not every frame.

## What is *not* covered

Construction. `Host::new` allocates the two parallel arrays, `Either`
allocates its branch arenas, `List` its item arena; those are one-time
and proportional to the tree. The constraint is about steady state, and
the §10 claim's own framing — a microcontroller with no allocator —
would need a construction story too (an arena or a `const` initializer).
That is not in this plan and is not claimed here.
