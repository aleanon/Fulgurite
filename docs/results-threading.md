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

## 3. Built (M17/T17.4, corrected by T17.5)

`feature = "threads"` on `lumen-static-shell`: `std::thread::scope`, no
dependency, and no `unsafe` — disjoint row bands are a safe split of the
pixel buffer.

**T17.4 gave each thread one equal band and measured 2.17× at 4K**, with
a curve that plateaued at eight bands and got worse beyond. That was
read as core heterogeneity — the reference 13900K has 8 performance and
16 efficiency cores, a frame ends at a barrier, so past eight bands the
extras land somewhere slower and everyone waits — and the band count was
capped at 8 on that reasoning.

**The reasoning was wrong and the cap was fixing the wrong problem.**
See §4.

## 4. What was actually limiting it (M17/T17.5)

**Every strip re-shaped every text run in the frame.**

`Surface::render` culls each command against the clip stack before doing
any work for it, and for text that check is what stands between the arm
and `FontStack::glyphs`, which shapes *and rasterises* an entire run. On
a **full** redraw there is no damage rect, so the stack was empty and
nothing was culled: each strip shaped all 192 runs of the scene. At 4K
with 8-row strips that is 270 strips × 192 runs, and the "parallel"
render came out **11× slower than one thread** — 216 ms against 19.3.

The fix is one push: **a band's own rows are a clip**, so the stack
starts with the band rect (intersected with the damage rect when there
is one). Attribution at 4K, against 19.3 ms on one thread:

| configuration | before | after |
|---|---:|---:|
| T17.4's static 8 bands | 8.91 ms | **5.93** |
| T17.4's static 32 bands | 9.92 ms | **3.61** |
| work queue, default | — | **2.60** |

**Once strips stop re-shaping, 32 bands beats 8** — the heterogeneity
penalty was real but small, and it had been swamped by a cost that grew
with the band count. The cap is gone.

*What that fix removed, and what it left — M50/T50.3 (2026-09-27).*
T17.5 removed the **whole-list** re-shape: a strip no longer shapes runs
nowhere near its rows. It left the **straddle** re-shape: a strip culls a
run against its rows padded by one em and then shapes the whole run, so
a run whose padded reach crosses a strip boundary was shaped by every
strip it touched — 2.27 shaping calls per visible run at 1080p with 8
bands and 3.63 at 4K with 32 (R8-F2; `tests/banded.rs`
`each_visible_run_is_shaped_once_per_frame`). Since T50.3 the frame
thread shapes each visible run once and fills the glyph cache before
any band starts, and the bands only blit: 1.00 at both sizes, and the
render is byte-identical to the pre-M50 one in both builds.

### The work queue

Strips are pulled from a shared queue rather than assigned up front, so
a slow core takes fewer of them and a strip containing a shadow does not
stall a whole worker. `chunks_mut` proves the strips disjoint; taking
one out of the iterator *moves* the `&mut`; a `Mutex` over the iterator
is the entire protocol. Per-thread deques and a steal handshake are for
tasks that spawn subtasks, and these do not.

Swept at 4K — workers across, strip height down:

| workers | 8 rows | 16 | 32 | 64 | 135 |
|---:|---:|---:|---:|---:|---:|
| 4 | 11.85 | 9.15 | 7.75 | 7.26 | 7.34 |
| 8 | 7.18 | 5.01 | 5.11 | 4.91 | 4.67 |
| 16 | 4.03 | 3.44 | 3.40 | 3.67 | 4.54 |
| 32 | 3.09 | **2.58** | 2.66 | 3.43 | 4.22 |

The default is `available_parallelism()` workers and four strips each,
floored at 16 rows, which lands on 2.60 — within noise of the best cell.
`LUMEN_WORKERS` and `LUMEN_STRIP_ROWS` override both.

### Where it ends up

| canvas | one thread | queue + `simd` | speed-up | GPU |
|---|---:|---:|---:|---:|
| tally window | 0.64 ms | 0.62 | 1.00× (refused) | 0.06 |
| 720p | 2.24 ms | **1.24** | 1.8× | 0.14 |
| 1080p | 4.89 ms | **1.40** | 3.5× | 0.29 |
| 1440p | 8.64 ms | **1.50** | 5.8× | 0.50 |
| 4K | 19.73 ms | **2.50** | **7.9×** | 1.11 |

**A 4K full redraw is 2.50 ms — 6.4× inside the 16 ms budget, and only
2.2× behind the GPU**, which was 18× ahead before this. §2's inference
of 6–8× was right after all; T17.4's 2.17× was measuring a defect.

### What it deliberately does not touch

- **The tally frame**, at 0.23 Mpx against a 0.25 threshold. A thread
  wake is 5–10 µs and that frame rasterises in 12, so `workers_for`
  refuses and the frame is byte-for-byte the single-threaded one.
- **The GPU crossover**, still 3–5%: 1–10% of a 1080p surface is
  0.02–0.2 Mpx, all below the threshold, so nothing in that sweep
  threads.
- **Frames with images**, which stay single-threaded because
  `&dyn ImageSource` has no `Sync` bound.

### What the gate can and cannot catch

`scripts/check_banded.sh` renders ten scenes at five sizes in both
builds and diffs the checksums — the comparison cannot happen inside one
binary, because the split is a cargo feature. It caught overlapping
strips and a band that forgets the damage rect.

**It did not catch removing the band clip, and cannot.** The drawing
loops already clamp `y` to the band, so the clip is purely an
optimisation: without it the output is identical and eleven times
slower. A correctness gate is structurally blind to that, which is worth
knowing given the size of the cliff. The sweep above is the only thing
that would show it, and it is not run per commit.

## Verdict

**Band-split rasterisation is the one worth building; pipelining is
not.** Built in M17/T17.4–T17.5; §4 has the measured result, which
matches this section's inference once a defect in the first attempt was
found. Neither is *urgent*, and the reason is that both
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
steady-state allocation, so per-thread state must be pre-allocated —
*which the shipped banded path did not do: `alloc_gate` counts per
thread and never bands, so it could not see it (R18-F4). Since M50 the
workers' own code allocates nothing (T50.3 shapes on the frame thread),
and the spawns are a stated budget of 3 per worker + 1, because a pool
that avoided them without `unsafe` would copy the rows out and back at
2.9× the cost of the frame itself (T50.4; `docs/results-allocation.md`,
"The threaded rasteriser's budget").* The
output must stay byte-identical or `damage_oracle.rs` and the agent's
journal both break — which disjoint bands give for free, since they
touch disjoint pixels. And it needs no `unsafe`: disjoint horizontal
bands are safe slices of the pixel buffer, which matters because every
crate but `lumen-static-reload` carries `#![forbid(unsafe_code)]` (since
M43/T43.1; `scripts/check_no_unsafe.sh` checks that it does).

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
