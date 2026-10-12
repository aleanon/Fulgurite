# Writing pixels four at a time instead of one

**Measured 2026-09-09.** Reference machine: i9-13900KF, governor
`powersave`. `benches --bin stores` for the microbenchmarks,
`just profile` for the frame. M17/T17.0.

## What was wrong

Every opaque write path in the software canvas was spelled

```rust
for px in dst.chunks_exact_mut(4) {
    px.copy_from_slice(&[c.0, c.1, c.2, c.3]);
}
```

— a four-byte store per pixel. `Canvas::clear`, `Canvas::span` (the
opaque fast path behind every rect and every straight row of every
rounded rect) and `clear_within` all used it, and `blit_0rgb` did the
matching thing on the way out: three shifts and two ors per pixel to
pack RGBA into `0RGB`.

Compiled into the library, the clear ran at **0.896 ms** for a 1080p
surface against **0.159 ms** for a `u32` fill of the same bytes — 5.6×.

**It is not the loop's shape.** The identical loop written inline in a
test crate vectorises fine and hits 0.159. What differs is surviving a
crate boundary with no LTO, which is why the fix is a wide *pattern* the
optimiser will widen either way rather than an `#[inline]`.

**And the first version of this measurement was wrong**, which is worth
recording because it nearly became the headline. It filled with a
constant colour and never read the result, so LLVM deleted the loop and
reported 0.000 ms — an 89 000× speed-up that did not exist. The second
version compared an opaque-coloured `Canvas::clear` against an
inline loop whose colour was a *constant*, so the optimiser could fold
four known bytes into a wide store on one side and not the other. Only
the third — both sides opaque, in one binary — was a comparison.

## What it is now

| path | 1080p | floor | vs floor |
|---|---:|---:|---:|
| `clear` | 0.257 ms | 0.160 (`fill`) | 1.61× |
| `blit_0rgb` | 0.570 ms | 0.150 (`copy`) | 3.81× |
| `clear` at 4K | 0.694 ms | 0.691 | **1.00×** |
| `blit_0rgb` at 4K | 2.672 ms | 0.546 | 4.89× |

`clear` now reaches the memset floor exactly at 4K. `blit_0rgb` did
**not** benefit much — 0.606 → 0.570 ms, 1.06× — and remains 3.8–4.9×
off a plain copy. Reading four pixels at a time and packing with
`from_le_bytes` was not enough for the optimiser to emit a shuffle. That
is the honest partial result and it is T17.1's target, not T17.0's
achievement.

## What it is worth on a real frame

The tally, `just profile`, before and after:

| stage | before | after | |
|---|---:|---:|---|
| `clear` | 96 µs | **10 µs** | 9.6× |
| `render(all)` | 482 µs | **346 µs** | 1.39× |
| `blit rgba->0rgb` | 76 µs | **57 µs** | 1.33× |
| **full redraw** | **654 µs** | **413 µs** | **1.58×** |
| `clear_within` | 4 µs | 0 µs | |
| `render_within` | 18 µs | 12 µs | 1.5× |
| `blit_within` | 6 µs | 7 µs | noise |
| **damaged frame** | **28 µs** | **19 µs** | **1.47×** |

`render(all)` improved without anything in the rasteriser changing:
`span` is the workhorse behind every opaque fill and every straight row
of a rounded rect, and it now writes wide.

**No `unsafe`, no dependency, no nightly, and it holds at both `no_std`
tiers** — which is the whole reason this came before the SIMD work. It
needed neither a gate exemption nor a feature.

## Why it is trusted

`crates/fulgurite-shell/tests/wide_stores.rs` writes out the loops
that were replaced and compares against them exhaustively — every
remainder shape (`w % 4` over 0..3), every start and end offset within a
row for the partial-span case, and one whole rasterised frame at
203×97, deliberately not a multiple of four.

Per `AGENT.md`'s rule, both faults were injected and both were caught:

| injection | result |
|---|---|
| drop the remainder loop in `fill_px` | 3 of 4 tests fail; `clear differs at 1x1 (w%4 = 1)` |
| swap two bytes in the blit pack | 2 of 4 fail; `blit differs at 440x1` |

In each case the tests that *should not* care stayed green — the
remainder fault left the blit test passing, and the pack fault left both
fill tests passing — which is what says they are testing different
things rather than all failing together.

## What remains

- **`blit_0rgb` is still 3.8–4.9× off a copy.** A byte shuffle is
  exactly what SIMD is for, and this is the clearest candidate after the
  blend loop.
- **`Canvas::blend`** — `(src·a + dst·inv)/255` per channel, one pixel
  at a time — is untouched and is most of what is left in
  `render(all)`. T17.1.
- **`clear` at 1080p is 1.61× off its floor** while at 4K it is 1.00×.
  Not chased; the absolute is 0.1 ms and the 4K case says the code is
  right.
