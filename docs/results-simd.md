# SIMD in the rasteriser: what it bought, and what bought more

**Measured 2026-09-09.** Reference machine: i9-13900KF, governor
`powersave`. `just profile` and `just profile-simd` on the tally frame,
median of 50. M17/T17.1 and T17.2.

## The measurement came first, and it moved the target

The plan was to vectorise `Canvas::blend` — the per-pixel source-over
that everything calls. Profiling first said otherwise.

**A single `Cmd::Shadow` was 255 µs of a 346 µs full redraw — 74%.** And
the profiler had not been reporting it: `just profile`'s full-render
breakdown covered `rect`, `rounded_rect` and `text` and **omitted
`Shadow` entirely**, so its rows summed to 86 of 346 µs and the largest
single command class was unattributed. The damaged breakdown had it all
along. Fixed in the same commit.

That also puts a claim in `docs/results-frame-cost.md` in question: it
says the shadow went from 72% of a frame's render "to a tenth of it".
That is true of the *damaged* path, where the shadow is 0–1 µs. It is
not true of a full redraw, where it is still three quarters.

## Two changes, measured separately

The vector path needs a row-oriented loop — you cannot vectorise a
function that takes `(x, y)` and re-derives the row every call. So
`shadow_within` was restructured first, which is a speed-up on its own
and had to be attributed separately or SIMD would have been credited
with it.

| | shadow | `render(all)` | |
|---|---:|---:|---|
| before | 255 µs | 346 µs | |
| row-oriented, scalar | **202 µs** | 290 µs | **1.26×**, free |
| + `feature = "simd"` | **180 µs** | 270 µs | **1.12×** more |
| | | | 1.42× together |

**The free half is the bigger half.** Row-orienting removes the four
coordinate comparisons `Canvas::blend` performs per pixel — the caller
has already clamped the run to the surface, so the only bound left is
the slice's own. No feature, no dependency, and it holds at both
`no_std` tiers.

**The SIMD half is 12%, and that is the honest number.** An `f32x8`
signed-distance field computes eight pixels of `sqrt`, `max`, `min` and
six arithmetic ops per step instead of one — and buys an eighth of what
that arithmetic suggests, because **the blend is still scalar and is now
the bottleneck.** For a shadow most pixels have non-zero coverage, so
the scalar blend runs on nearly all of them.

## Why the blend was not vectorised too

Pixels are interleaved RGBA. Eight pixels' red channels are eight bytes
apart in a stride of four, so blending eight pixels at once means
deinterleaving — shuffles that `wide` does not expose, and the reason
interleaved-RGBA blending is awkward in portable SIMD generally.

A contiguous alternative exists: eight 32-bit lanes cover eight
*channels*, which is two whole pixels, loaded and stored with no
shuffle. **That was built and measured in T17.2 below: 1.03×, which is
noise.** The section there says why, and it is the reason this one gave
as a guess — two pixels per vector is not enough width to pay for the
widening.

## Why `wide`, and why a feature

**`wide` and not `core::arch`.** Intrinsics are all `unsafe`, and
`scripts/check_no_unsafe.sh` has held zero `unsafe` across every
`crates/*/src` since M0. Reaching for intrinsics would mean growing that
gate an exception, which is how a gate stops meaning anything. `wide` is
a safe wrapper and `no_std`-clean.

**A cargo feature and not a separate crate.** The M15 gating policy
ranks a crate first, then a type parameter, then a feature — and this is
one dependency and one code path inside an existing file, not a new
capability. Its cost: **+3 crates** on `lumen-static-shell` (7 → 10),
default-off.

## Why the vector path is trusted

`crates/lumen-static-shell/tests/simd_shadow.rs` writes out the loop
`shadow_within` had before T17.1 — per-pixel distance, per-pixel `blend`
with its own bounds check — and compares rendered pixels against it over
nine shapes: every eight-lane tail, runs narrower than one lane,
shadows clipped off each edge, radius 0 so there is no interior hole,
and a spread that dwarfs the shape.

It is **byte-identical**, not close, and by construction: the vector
form computes the same `f32` alpha in eight lanes and hands it back as
`f32`; the `as u8` cast and the blend stay scalar. Only the order of
exactly-specified `f32` operations differs.

Per `AGENT.md`'s rule, injected and caught:

| injection | result |
|---|---|
| drop the `d >= -1` mask in the vector path | `429 bytes differ (simd = true)` |
| stride the lanes by 7 instead of 8 | `144 bytes differ (simd = true)` |

In both cases **the scalar build stayed green**, which is what says the
tests distinguish the two paths rather than failing together. And
`the_shadow_actually_draws_something` asserts 15% of the canvas is
inked, because the comparison would otherwise pass on two blank
canvases.

`just simd` runs the byte-identity suites, `wide_stores` and
`damage_oracle` (10 tests, pixel identity) against the vector build, and
is part of `just check`.

## T17.2 — the blit, and the blend that did not pay

Both of the "what remains" items above were attempted. One worked and
one did not, and the difference is instructive: **it is deinterleaving,
not arithmetic.**

### `blit_0rgb`: 2.3×, because the swizzle is arithmetic

RGBA little-endian is `R | G<<8 | B<<16 | A<<24`; `0RGB` is
`B | G<<8 | R<<16`. So the conversion is three masks, two shifts and two
ors — **lane-uniform `u32` arithmetic with nothing to deinterleave.**
Eight pixels per step:

| | 1080p | vs copy floor |
|---|---:|---:|
| byte gather (T17.0) | 0.567 ms | 3.8× |
| the same arithmetic, scalar | 0.605 ms | slower |
| `u32x8` | **0.246 ms** | 1.58× |

**2.3×, and the scalar spelling of the identical arithmetic is 0.94× —
slower.** It is the vector width that pays, not the formulation, which
is why T17.0's four-at-a-time attempt moved it only 1.06×.

### The blend: 1.03×, and it will not get better this way

Attempted with `u32x8` over eight contiguous channels — two whole
pixels, loaded and stored without a shuffle. Measured **1.03×**, and on
a re-run 0.98×. It is noise, not a speed-up.

Two reasons, and the second is the one that closes the question:

- **Eight 32-bit lanes cover two pixels.** Interleaved RGBA means a
  vector holds `[r0,g0,b0,a0,r1,g1,b1,a1]`, so the arithmetic is eight
  lanes wide and the *pixel* throughput is two. Reaching eight pixels
  needs the channels separated, and `wide` exposes no shuffle to
  separate them.
- **The scalar blend is already 1.27 ns/px** — about four cycles for
  three multiply-add-divides plus loads and stores. It is superscalar
  work with no dependency chain, and there is very little for a
  two-wide vector to reclaim.

Recorded as a **negative result** rather than dropped: the next person
to look at this loop should know it was measured, what it cost, and
which of the two reasons would have to change.

A byproduct worth keeping: both exact-division identities for `/255`
were verified exhaustively over `0..=65025`, the full range a blend
produces — `((x+1)*257)>>16` and `(x + (x>>8) + 1) >> 8`. Neither is
needed now; both are correct if a shuffle ever makes the blend worth
revisiting.

## Where the frame stands

The tally, full redraw, median of 50:

| stage | before M17 | T17.0+T17.1 scalar | + `simd` |
|---|---:|---:|---:|
| `clear` | 96 µs | 10 | 10 |
| `render(all)` | 482 µs | 291 | **269** |
| — of which `shadow` | 255 µs | 202 | **180** |
| `blit rgba->0rgb` | 76 µs | 62 | **24** |
| **total** | **654 µs** | **363** | **303** |
| | | 1.80× | **2.16×** |

`pump(layout+paint)` is 2 µs throughout and was never the point.

## What remains

- **The shadow is still 67% of the render** (180 of 269 µs), one
  command. It is the only thing left worth attacking in this file, and
  the remaining lever is the blend inside it — see above for why that is
  blocked on deinterleaving rather than on effort.
- **Nothing was measured on a machine without AVX.** `wide` falls back
  to narrower lanes or scalar; every ratio here is an AVX2 number on one
  CPU.
