# What a glass panel costs, and what blur it delivers

**Measured 2026-09-10.** Reference machine: i9-13900KF, governor
`powersave`. `cargo run --release -p benches --bin glass`, a panel over a
striped 1920×1080 backdrop, min of 20. M19/T19.3.

The figure is the *difference* between rendering the scene with the
panel and without it, so it is the panel's own cost and not the frame's.

## 1. Cost against σ — flat, and that is the downsample working

| requested σ | k | box width | delivered σ | error | panel |
|---:|---:|---:|---:|---:|---:|
| 2.0 | 1 | 4 | 2.12 | +6.1% | 11.95 ms |
| 4.0 | 1 | 8 | 4.14 | +3.6% | 12.47 |
| 6.0 | 2 | 6 | 6.27 | +4.5% | **6.85** |
| 8.0 | 2 | 8 | 8.29 | +3.6% | 6.97 |
| 12.0 | 5 | 5 | 12.25 | +2.1% | **5.21** |
| 16.0 | 5 | 6 | 15.68 | −2.0% | 5.33 |
| 24.0 | 6 | 8 | 24.86 | +3.6% | 5.47 |
| 32.0 | 7 | 9 | 31.30 | −2.2% | 5.25 |

**A 32 px blur is cheaper than a 2 px one — by 2.3×.** That is not a
mistake and it is the clearest evidence the design works. A box blur
with a running sum is already O(1) per pixel in the radius, so the box
passes cost the same whatever σ is; what changes is `k`. Below σ ≈ 5 the
search picks `k = 1` and the three passes run at full resolution; above
it the capture is downsampled and every pass runs on 1/k² of the pixels.

The practical reading: **σ below about 5 is the expensive case**, and it
is also the case where a blur is least visible. A panel that wants to be
cheap should ask for more blur, not less.

## 2. Cost against panel area — linear, at about 21 ns/px

| panel | px | cost | per px |
|---|---:|---:|---:|
| 200×80 | 16 000 | 0.42 ms | 26.2 ns |
| 400×200 | 80 000 | 1.81 | 22.6 |
| 800×300 | 240 000 | 5.21 | 21.7 |
| 1600×800 | 1 280 000 | 26.93 | **21.0** |

Linear in area, as a separable blur must be. The per-pixel figure falls
slightly with size because the capture is dilated by a fixed `3σ + 4`
reach on every side, which is proportionally more of a small panel.

**A 1600×800 panel at 26.9 ms does not fit in a frame.** A full-screen
backdrop filter is not what this is for, and T19.4's cache is what makes
a *static* one affordable — but a panel that large over content that
moves every frame costs what the table says. A toolbar-sized panel is
0.4 ms and fits comfortably.

## 3. The blur is the one SVG specifies, and it does not move the image

Three box passes at the width
`d = floor(σ · 3 · √(2π) / 4 + 0.5)` —
<https://www.w3.org/TR/SVG11/filters.html#feGaussianBlurElement>.

**The even case is not three equal boxes.** An even box has no centre
pixel, so three of them shift the image half a pixel each. The spec's
answer is two boxes of `d` centred on opposite pixel *boundaries* plus
one of `d + 1` centred on the pixel, and the even case is the common one
here — the `k` search targets a per-pass σ near 2, where `d` is 4.

Measured by the centre of mass of a blurred bar, which is at row 59.500
before the blur:

| | centre of mass |
|---|---:|
| σ = 2, 4, 6, 8 (all even `d`) | **59.500** |
| σ = 12 | 59.476 |
| three equal even-width boxes instead | **61.000** |

The last row is the injection, and 1.5 px is exactly the predicted
slide.

## 4. `k` is chosen by search, and the obvious formula is 2.4× worse

`k = ⌈σ/2⌉` is the rule the first draft of `docs/plan-glass.md`
specified. It drops the per-pass σ below the 2.0 the SVG approximation
is specified for, and it delivers:

| rule | worst error over σ ∈ [2, 32] | at |
|---|---:|---|
| `k = ⌈σ/2⌉` | **24.0%** | σ = 9.30 |
| search `k ∈ 1..=8` minimising \|k·σ_box(d) − σ\| | **10.0%** | σ = 3.85 |

Eight iterations of the real cost function, once per command rather than
per pixel. The residual 10% at σ = 3.85 is where an integer box width is
least able to hit a small σ, and it is the number the test bounds at 12%.

`software::blur_plan` is public so this table is computed from the
renderer's own function rather than a copy of it.

## 5. T19.4 — the backdrop cache, decided against on measurement

The plan's T19.4 was one buffer per panel holding its blurred backdrop,
so a frame whose damage misses the panel would pay the composite only.
**It is not built, because the measurement says it cannot pay.**

A cache must still *capture* the source region to know whether what it
cached is still valid, and must still composite. So its ceiling is
whatever the box passes cost — and `blur = 0` measures exactly that,
because the renderer skips them at zero:

| panel | σ = 12 | σ = 0 (capture + composite) | the passes |
|---|---:|---:|---:|
| 400×200 | 1.81 ms | 1.73 | **4%** |
| 800×300 | 5.21 | 5.17 | **1%** |

**One to four percent.** That does not pay for a per-panel buffer, an
invalidation rule and the correctness surface that comes with both.

There is a second reason, and it is the one that would have wasted the
day. The win the plan described — "nothing intersects the panel's `src`
→ reuse it" — **cannot happen**. M18's region set plus T19.2's
dependency rule mean a panel is repainted *only* when damage touches its
source region, and if damage touches its source region the cached
backdrop is stale by definition. The two states the cache distinguishes
are the two states the damage rule already collapsed. The only case left
is a panel whose own tint or blur animates over a still backdrop, which
is 1–4% of one panel's cost.

Recorded rather than dropped, the way `docs/results-simd.md` records the
blend that did not pay: the next person to reach for this should know it
was measured, what the ceiling was, and which of the two reasons would
have to change.

## 6. T19.5 — threading, declined for now

A band owns disjoint rows, and that is the entire safety argument behind
`Surface`. A backdrop filter captures a region reaching `3σ + 4` px past
its panel on every side — into rows another worker owns and is
concurrently writing. So `render_banded` **declines any list containing
a glass command**, and `tests/banded.rs` asserts the decline through
`Canvas::bands_for_list` rather than inferring it from pixels that
happen to match.

The plan's T19.5 was to segment the command list at glass boundaries,
reconstruct the clip stack at each, and join between segments. It is not
built, and §2 is why: **a toolbar-sized panel is 0.42 ms in total.**
Segmentation, a clip prefix per boundary and a per-segment worker count
are a day of delicate work to divide a number that is already inside the
frame budget by four.

The condition to revisit, stated rather than left as "someday": **a
frame with a panel of a few hundred thousand pixels.** §2 prices 240 000
px at 5.2 ms and 1 280 000 at 26.9, and at that size the whole rest of
the frame is threaded and this is not.

What it costs meanwhile: a frame containing any glass panel renders
single-threaded, so the *rest* of that frame loses threading too. At
4K that is the difference between 2.50 ms and 19.3
(`docs/results-threading.md`) — which is the strongest argument for
building the segmentation, and it is an argument about the rest of the
frame rather than about the panel.

## 7. What is not measured here

- **Threading.** Everything above is single-threaded. T19.5 segments the
  command list at glass boundaries; the numbers here are what it will be
  measured against.
- **A cache.** There is none, and §5 says why.
- **The GPU.** It draws the degraded fallback; `docs/plan-glass.md` §1.3
  says why.
- **Anything but this machine.** One CPU, `powersave`, one scene.
