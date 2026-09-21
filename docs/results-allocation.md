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
| `rasterising_a_frame_allocates_nothing`, 50 frames | **50 — 1.00 per frame** | **0** |
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

## What is *not* covered

Construction. `Host::new` allocates the two parallel arrays, `Either`
allocates its branch arenas, `List` its item arena; those are one-time
and proportional to the tree. The constraint is about steady state, and
the §10 claim's own framing — a microcontroller with no allocator —
would need a construction story too (an arena or a `const` initializer).
That is not in this plan and is not claimed here.
