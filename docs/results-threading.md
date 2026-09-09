# Would a second thread help, and which second thread?

**Measured 2026-09-08.** Reference machine: i9-13900KF, governor
`powersave`. `benches-competitive/arms/raster --bin bands` for the band
table; the pipelining arithmetic is over the stage figures in
`docs/results-competitive.md` §5b.

Two different things get called "threading the renderer" and they have
opposite answers. This measures both, because handoff §2 recommends one
of them in a sentence that turns out to be wrong.

## 1. Pipelining — frame N builds while frame N−1 rasterises

The classic split: the UI thread produces a display list, a raster
thread consumes the previous one. Throughput becomes
`max(framework, raster)` instead of their sum.

| frame | framework | raster | pipelined | gain |
|---|---:|---:|---:|---:|
| the tally, a real frame | 2 µs | 482 µs | 482 µs | **0.4%** |
| 10 000 rows, software raster | 210.5 µs | 30.7 µs | 210.5 µs | 12.7% |
| 10 000 rows, GPU raster | 210.5 µs | 88.7 µs | 210.5 µs | 29.6% |

**It pays only where the two halves are balanced, and on a real frame
they are not remotely.** The tally spends 2 µs of a 484 µs frame above
the rasteriser; hiding that behind the raster saves four tenths of one
percent.

### And it costs a frame of input latency

**Handoff §2 says pipelining "roughly halves perceived latency at no
algorithmic cost". That is backwards and is corrected there
(2026-09-08).** Pipelining improves *throughput*. Input→photon latency
goes **up** by one frame, because the frame a click affects is now
rasterised one period after it is built.

The live gate measures click→presented at **6.0 ms** against a 100 ms
budget (`scripts/ui_check.py`). Adding 16.7 ms would nearly quadruple
it, to buy 0.4% on the stage that is not the bottleneck.

## 2. Band-split rasterisation — the one that scales

Give each thread a horizontal slice and let it draw only what
intersects: that is `Canvas::render_within` with a different rect, so the
primitive already exists and needs no new concept.

What the bands do *not* share is the command walk — every band tests
every command against its own rect. Measuring that redundancy
sequentially bounds the parallel speed-up, because threads can divide
the wall clock but cannot remove duplicated work.

Sum of N band renders against one full render, min of 20:

| canvas | commands | 1 | 2 | 4 | 8 | 16 |
|---|---:|---:|---:|---:|---:|---:|
| 1080p | 385 | 4.93 ms | 1.01× | 1.02× | 1.02× | 1.06× |
| 4K | 1 633 | 19.21 ms | 1.01× | 1.02× | 1.02× | 1.02× |

**Between 1% and 6% overhead at sixteen bands.** The rasteriser is
per-pixel bound, not per-command bound, so splitting it is very nearly
free and threads should scale close to linearly.

### The floor

A full redraw writes `w × h × 4` bytes whatever else it does. Measured
as a straight `copy_from_slice`:

| canvas | bytes | pure write |
|---|---:|---:|
| 1080p | 7.9 MB | 0.150 ms |
| 4K | 31.6 MB | 0.546 ms |

So a 4K full redraw cannot go below ~0.55 ms however many cores are
thrown at it, against 19.2 ms today — a ceiling of about 35×. Eight
cores realistically gives 6–8×, or **~3 ms at 4K**.

That matters because it is the number that decides whether the software
canvas can hold 60 fps on a real display: **it can, threaded, without a
GPU at all.** `docs/results-raster-scaling.md` measured the unthreaded
4K full redraw at 22.9 ms, over budget.

## 3. Built, and the inference above was wrong by 3× (M17/T17.4)

`feature = "threads"` on `lumen-static-shell`: `std::thread::scope`, one
band of rows per thread, no dependency. **The prediction in §2 — 6–8×
from eight cores, ~3 ms at 4K — was inference, and it was too
optimistic.** Measured, on the same machine:

| canvas | one band | threaded | + `simd` | speed-up |
|---|---:|---:|---:|---:|
| tally window | 0.64 ms | 0.64 | 0.62 | **1.00×** (below threshold) |
| 720p | 2.24 ms | 1.49 | **1.37** | 1.50× |
| 1080p | 4.91 ms | 2.98 | **2.43** | 1.65× |
| 1440p | 8.60 ms | 3.84 | **3.95** | 2.24× |
| 4K | 19.34 ms | 8.93 | **8.89** | **2.17×** |

### Why 2.2× and not 8×

The band count sweeps like this at 4K:

| bands | 1 | 2 | 4 | **8** | 12 | 16 | 24 | 32 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| ms | 19.32 | 12.75 | 10.87 | **8.91** | 9.23 | 9.34 | 9.39 | 9.92 |

It plateaus at eight and then **gets worse**. Parallel efficiency is
already down at two bands (1.52× of a possible 2.0), which is not a
spawn-cost curve — spawn cost would punish many bands and spare few.

**A frame ends at a barrier: every band must finish before the frame
does, so the frame runs at the speed of the slowest band.** On a
heterogeneous CPU that means the slowest *core*. The reference machine
is a 13900K — 8 performance cores, 16 efficiency cores, 32 threads — so
past eight bands the extra ones land somewhere slower and the whole
frame waits for them. Equal-sized bands on unequal cores is the wrong
partition, and the fix is work-stealing rather than more bands.

The default cap is therefore **8**, not `available_parallelism()`. The
right number is the performance-core count, which `std` cannot report;
`LUMEN_BANDS` overrides it.

### The result that matters

**A 4K full redraw is 8.89 ms, inside the 16 ms budget, with no GPU.**
It was 22.88 before M17 and 19.34 after the scalar work — over budget
both times. `docs/results-raster-scaling.md`'s verdict that 4K "cannot
hold 60 fps" on the CPU no longer holds when this feature is on, and is
corrected there.

### What it does not touch, by design

- **The tally frame is unchanged**, because it is 0.23 Mpx and the
  threshold is 0.25. A thread costs 5–10 µs to wake and that frame
  rasterises in 12; threading it would be a loss, so `bands_for`
  refuses.
- **The GPU crossover is unchanged** at 3–5%, because 1–10% of a 1080p
  surface is 0.02–0.2 Mpx — all below the threshold, so no frame in that
  sweep threads at all.
- **A frame with images renders on one thread.** `&dyn ImageSource`
  carries no `Sync` bound, and adding one would change a core trait for
  a case nothing in this workspace exercises.

### Why it is trusted

Bands own disjoint rows and each draws every command whose ink reaches
those rows, in list order — the same rule that makes a partial redraw
equal a full one. So the output is identical by construction, and
`scripts/check_banded.sh` checks it anyway across ten scenes at five
sizes, including sizes where the last band is short.

The comparison cannot happen inside one binary — the split is a cargo
feature — so both builds render the same scenes and their checksums are
diffed. A hard-coded golden would rot into a rubber stamp on the next
legitimate rasteriser change; a cross-build diff cannot, because both
sides move together or the gate fires.

Injected and caught: bands overlapping by one row, and a band that
forgets the damage rect. `just threads` runs it, plus
`damage_oracle` against the threaded build, and is part of `just check`.

## Verdict

**Band-split rasterisation is the one worth building; pipelining is
not.** Built in M17/T17.4 — see §3, which corrects the size of the win
this section predicted. Neither is *urgent*, and the reason is that both
address the same case damage tracking and the GPU backend already
address:

- A frame that damages a rect is already 18 µs on the tally. Thread
  wake-up is 5–10 µs, so threading a frame like that is a loss.
- A frame that repaints most of the surface is exactly where the GPU
  wins (`docs/results-gpu.md`: the crossover is 3–5% damaged area, and
  a 4K full redraw is 1.1 ms there against 22.9 on one core).

So a threaded rasteriser is best understood as **the arm for machines
with no usable adapter** — the configuration the CPU fallback already
exists for — rather than as a competitor to the GPU path. That is a real
constituency and a real 7× for it, and it is not the first thing to
build.

**What it would cost this project.** `no_std` has no threads, so it is
desktop-only: a separate crate or a feature, under
`docs/plan-capabilities.md`'s gating policy. `alloc_gate` asserts zero
steady-state allocation, so per-thread state must be pre-allocated. The
output must stay byte-identical or `damage_oracle.rs` and the agent's
journal both break — which disjoint bands give for free, since they
touch disjoint pixels. And it needs no `unsafe`: disjoint horizontal
bands are safe slices of the pixel buffer, which matters because
`scripts/check_no_unsafe.sh` covers every crate.

## What is not measured here

- **A threaded implementation.** Every number above is sequential; the
  parallel speed-up is inferred from the band overhead and the write
  floor, and inference is not measurement. A real one could lose to
  false sharing at band boundaries, to allocator contention, or to
  scheduling jitter that a min-of-20 would hide.
- **Parallelising the framework stages.** At 10 000 rows on the GPU path
  they are 70% of the frame, so it is tempting — but layout's
  constraints are sequential (a child's position needs its predecessors'
  sizes), and paint's order *is* z-order. `Boundaries` already gets 10×
  there (`docs/results-competitive.md` §5b) with no threads and no new
  invariant, which is the cheaper lever and is already built.
