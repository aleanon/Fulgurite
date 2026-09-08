# SIMD in the rasteriser: what it bought, and what bought more

**Measured 2026-09-09.** Reference machine: i9-13900KF, governor
`powersave`. `just profile` and `just profile-simd` on the tally frame,
median of 50. M17/T17.1.

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

A contiguous alternative exists: `u16x8` covers eight *channels*, which
is two whole pixels, loaded and stored contiguously with no shuffle.
That is 2× on the blend rather than 8×, and it was not built. The
remaining ceiling is small — 180 µs on one command of one frame — and
the measurement above says the arithmetic is not where the time is.

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

## What remains

- **The blend, at 2× via `u16x8`**, as above. Small ceiling.
- **`blit_0rgb` is still 3.8–4.9× off a plain copy**
  (`docs/results-wide-stores.md`). It is a byte shuffle and the clearest
  remaining SIMD candidate — and it has the same deinterleave problem,
  which is why T17.0 barely moved it.
- **Nothing was measured on a machine without AVX.** `wide` falls back
  to narrower lanes or scalar, and the 1.12× is an AVX2 number on one
  CPU.
