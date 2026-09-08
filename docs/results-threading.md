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

## Verdict

**Band-split rasterisation is the one worth building; pipelining is
not.** But neither is urgent, and the reason is that both address the
same case damage tracking and the GPU backend already address:

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
