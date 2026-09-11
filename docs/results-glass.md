# What a glass panel costs, and what blur it delivers

**Measured 2026-09-10.** Reference machine: i9-13900KF, governor
`powersave`. `cargo run --release -p benches --bin glass-cost`, a panel over a
striped 1920×1080 backdrop, min of 20. M19/T19.3.

The figure is the *difference* between rendering the scene with the
panel and without it, so it is the panel's own cost and not the frame's.

## 0. The command's byte budget

Measured 2026-09-11. Stated here because `docs/plan-liquid.md` §1 cites
it and a plan is not where numbers live.

| | bytes |
|---|---:|
| `Material` | 28 |
| `Cmd::Glass` payload (`rect` 16 + `radius` 4 + `Material`) | 48 |
| `Gradient`'s payload — **what sets `Cmd`'s size** | 56 |
| `Cmd` | 60 |

**Every strength is a `u8`, and that was the difference between fitting
and not.** M22 adds four unitless 0..=1 fractions; as `f32` they take
`Cmd` from 60 to 64, which is four bytes on every command in every
list — about 8 KB on a 2 000-command frame. As `u8` all four fit with
room left over. The same trade `Style::focus_ring` already made and
recorded.

`Material` is 28 rather than 26 because of two bytes of alignment
padding, which is why `tests/glass_cmd.rs` enforces the field list with
an exhaustive destructure rather than an arithmetic check — a new field
stops that test compiling.

## 0b. T22.0 — Fresnel, the inner shadow, saturation

**Re-measured 2026-09-12**, same machine and method as §2. **This is
the milestone's only cost table** — §0d and §0e cite it rather than
restating a column, per `AGENT.md`'s "numbers are stated once".

| panel | bevel alone | + T22.0 | | + T22.2 | | + T22.3 | |
|---|---:|---:|---:|---:|---:|---:|---:|
| 400×200 | 5.33 ms | 5.69 | **+7%** | 6.27 | **+10%** | 6.26 | **−0%** |
| 800×300 | 14.97 | 16.07 | **+7%** | 17.43 | **+8%** | 17.63 | **+1%** |

Each "added" is against the column to its left, so they compose. Read
end to end: the whole of M22's material work costs **+18%** over M21's
bevel at either size.

The T22.0 column read +9% and +13% when it was first measured on
2026-09-11. It is restated rather than annotated because **T22.1 raised
both of its columns**: the sub-pixel read costs the bevel baseline as
much as it costs the full material, so the ratio fell while neither side
got cheaper. A stale ratio beside a live one is the drift this doc
exists to prevent.

Each column also had to gain an explicit "off" for every *later* field,
or it stops measuring what it is named for: `LIQUID` carries dispersion
and adapt now, so the T22.0 row sets both to zero.

"Bevel alone" is M21's material — no inner shadow, neutral saturation.
Fresnel cannot be separated from it, because it is derived from
`specular` rather than given its own field; that is what "zero bytes"
buys and what it costs.

The cost grows with the panel because **saturation is the one effect
here that is not rim-band only** — it applies to every pixel the panel
covers, while Fresnel and the shadow fall out with the bevel parameter.

### Fresnel is a two-to-three pixel effect, and that is the point

Schlick's fifth power is a *grazing*-angle term. Measured on a 13 px
rim: 2.5 px inside the boundary the bevel normal's z component is 0.59
and the term is 0.011; at the boundary z falls to 0.28 and it is 0.18.
So the reflection lives in the outermost two or three pixels — which is
exactly why it reads as an *edge* rather than as a tint.

The first version of its test sampled the whole rim band and measured a
lift of 0.3 against a threshold of 12. The band matters.

## 0c. T22.1 — the refracted read is sub-pixel

**Measured 2026-09-12.** Until M22/T22.1 the refracted coordinate was
truncated to a whole surface pixel before sampling, and the downsample
factor is 1 at the liquid blur — where the bilinear mapping's four taps
already collapse to one. So the refraction was nearest-neighbour and its
rim carried a staircase.

Against the example's dock panel, sub-pixel versus the integer read:

| | |
|---|---:|
| pixels that differ | **766** of 285 200 |
| worst channel delta | **9** |

Small, and stated because it is inherited: every later task and the
future GPU parity baseline sit on top of it.

It is also a **prerequisite for T22.2**. A sub-pixel difference between
three channels cannot survive truncation to the same pixel, so dispersion
on an integer read is not a weak fringe — it is a dithered speckle.
Measured: a 0.35 px shift of the panel moves 240 of 800 rim pixels with
the float read and 80 with the integer one.

One trap it set on the way in. The sampler's argument changed from a
pixel *index* to a pixel *centre*, and the mapping kept its own `+ 0.5`
— shifting every read half a pixel and sliding the blur's centre of mass
from 59.5 to 59.0. `tests/glass_blur.rs`'s odd/even test caught it
immediately, which is the value of having written it.

## 0d. T22.2 — dispersion

**Measured 2026-09-12.** One `u8`, `Material::dispersion`, scaled so 255
displaces the outer channels by `MAX_DISPERSION = 0.25` of the
refraction either way. `LIQUID` carries 90.

### What it costs

**+10% / +8%** — §0b's table. Nearly flat across panel size, because
dispersion is rim-band only; saturation is not, which is why §0b's
T22.0 column is the one that grows.

That is three taps where there was one, and it is only 10% because
the sampler grew a **per-channel entry point**. Calling the
three-channel read three times would triple the coordinate setup and do
36 loads where 12 suffice: one clamp-and-floor is amortised over three
channels, and dispersion needs three different coordinates but only one
channel from each.

### What it looks like

| | |
|---|---:|
| rim mean, blue − red, over a white-to-dark step | **+27.3** |
| the same rim with `dispersion: 0` | **0.00** chroma |

Signed, because **the ordering is the entire claim**. Normal dispersion
puts blue further out than red; inverted, the fringe is the same size
and lands on the wrong side of the edge, where an eye reads it as colour
noise rather than as glass. A test that measured only "the channels
differ" would pass on the inverted renderer.

Mapped directly with a one-pixel stripe sweep: the outermost rim pixel's
**blue** channel responds to source columns 67–84 and its **red** to
85–100, a clean ~18 px separation at refraction 40 — which is
`2 × 40 × 0.25`, as designed.

### Two bounds that were nearly wrong

**The source expression.** `glass_source` widens to
`3σ + refraction·(1 + dispersion·0.25) + 4`, and the widening is real:
it bounds the capture *and* the damage dependency, so a short source
blurs the outer fringe from edge-extended data — identically in a full
and a partial render, which is why no other gate in the milestone can
see it.

But **the `+ 4` slack hides the term at any ordinary refraction**: at
`LIQUID`'s 11 px the widening is 2.75 px, comfortably inside 4. The
first gate written for this was built on `LIQUID`, measured 0 of 240,
and was testing the slack. The shipped one runs at refraction 100, where
the widening is 25 px, and moves **48 of 48** rim pixels with a worst
blue delta of 139 — 0 of 48 when the term is removed.

**The clamp guard.** Where the capture is truncated by the window edge,
three displacements clamp at three different distances and a grey ramp
grows a coloured band. The guard falls back to the shared read, and it
tests the **extremes, not the green coordinate** the plan named: green
is the midpoint, so blue can clamp while green is inside. Measured on a
panel flush with the left edge at refraction 60 — worst chroma in the
clamped band:

| | |
|---|---:|
| guarded | **0** |
| guard deleted | **35** |
| guard on the green coordinate only, as first planned | **35** |

The fringe immediately outside the band measures 52, asserted in the
same test: a guard that refuses everything would satisfy the first half
alone.

## 0e. T22.3 — the adaptive tint

**Measured 2026-09-12.** One `u8` (`adapt`) and one `Color`
(`tint_over_light`). `LIQUID` carries `adapt: 190` and a dark
`(10, 10, 14, 26)` over bright content. **Cost −0% / +1%** (§0b): one
reduction over the panel's blurred array against a per-pixel composite
is not measurable at these sizes.

It is a pure function of the captured backdrop — no stored value, no
damping — for the three reasons in `docs/plan-liquid.md` §5, the sharpest
being that `render_within` runs once per damaged region, so anything
accumulated in the renderer would advance a variable number of times per
frame.

### The sweep

24 backdrops from 24 to 231, measuring the panel's interior **minus the
backdrop it sits on** — the difference, because reading the interior
alone confounds the tint with the backdrop as both rise together.

| | |
|---|---:|
| total travel | **31.0** levels, +16.0 → −15.0 |
| largest single step | 2.0 levels |
| strictly monotonic | yes, all 23 steps |
| crosses neutral | yes, between backdrop 141 and 150 |

All three properties are asserted, because each alone accepts a renderer
the others reject: monotonic accepts a *constant*, travel accepts a
jump, and neither notices the interesting region was never entered.

### The pre-registered exit, and what it caught

The bound was fixed before the measurement: **less than one output level
of movement per one-pixel scroll**, principled rather than round — below
the quantisation of a `u8` channel nothing can be seen to flicker. Over
a hard light/dark boundary scrolled 90 px through the panel:

| | |
|---|---:|
| worst per-frame move, 120 px panel | **0.886** levels |
| worst per-frame move, 40 px panel | **1.000** levels |
| bound | 1.0 |

**It passes, and it passes narrowly.** The movement scales as
`contrast × width / area`, so it grows as the panel shrinks: at 40 px
tall the same scroll sits exactly on the bound. A materially shorter
panel over full-contrast content would exceed it, and that is the stated
limit of the stateless design rather than a margin to spend.

**The first implementation sat *on* the bound at 120 px**, and the cause
was not the mean at all: the blended tint was rounded back to a `Color`,
so one step of its `u8` alpha moved the panel's interior by a whole
output level. Keeping the tint as four `f32` through the composite
removed the staircase and took the figure to 0.886. A continuous
function quantised at the last step is not a continuous function.

### How much of the tint is chosen by content the panel does not cover

The renderer reduces a *blurred* mean, and every sample inside the panel
mixes in backdrop from up to 3σ outside it. Measured as the tint shift
between two scenes with an identical backdrop under the panel and
opposite surroundings — which is the contamination and nothing else:

| | |
|---|---:|
| leak, reducing over the panel | **1.00** levels |
| bound, set first at 20% of the sweep's travel | 6.2 |
| leak, reducing over the whole capture instead | **8.11** |

The third row is the injection, and it is the reason the bound is worth
having: reducing over the dilated capture is the *quiet* failure here —
the wrong colour, equally wrong in every render, invisible to every
other gate.

### The region is the panel's rect, and the plan was wrong about why

Reducing over `bounds` — the rect intersected with the clip — makes a
clipped panel compute a different mean from an unclipped one and then
differ on **4896 of 4896** shared pixels, worst channel delta 5.

The plan expected that to surface as a *damage-oracle* failure. It does
not. `Recording::glass` registers the whole `glass_source` rect as a
dependency, so any partial redraw touching the panel already carries a
clip containing all of it and `clipped(rect, clip)` returns the rect
unchanged — the injection has nothing to bite on. A `PushClip` in the
list is what distinguishes the two, and it is a real situation: a glass
panel inside a `ScrollView` is clipped by its viewport every frame.

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

## 5b. The bevel — what makes it look like glass rather than frost

**Measured 2026-09-11.** M21 added `refraction` and `specular` over a
bevel of width `rim`, which is what separates
[`Material::LIQUID`](../crates/lumen-static/src/display.rs) from
`FROSTED`.

| panel | frosted | liquid | the bevel |
|---|---:|---:|---:|
| 400×200 | 4.56 ms | 5.38 | **+18%** |
| 800×300 | 13.71 | 14.42 | **+5%** |

Both at σ = 3, so only the bevel differs. (Both are dearer than §1's
table because σ = 3 takes `k = 1` and runs the box passes at full
resolution — the same "a small blur is the expensive one" result.)

### Why a *third* normal component is the whole trick

A rounded-rect SDF gives a 2D gradient, and along each straight edge
that gradient is **constant**. A highlight built on one is a uniform
stripe down the edge that switches off at the corners — an outline, not
a surface.

The bevel lifts the normal out of the plane. With `u` running 1 at the
boundary to 0 at `rim` px inside:

```
n = (u·nx, u·ny, sqrt(1 − u²))
```

— a spherical cap, already unit, singular nowhere, and no division. It
varies **across** the rim as well as around it, so the highlight sweeps
the way a curved edge does. `tests/glass_bevel.rs` pins it by injection:
forcing `nz = 0` kills the highlight outright at every light angle,
because a purely horizontal normal never points at a light that has any
elevation.

The refraction rides the same parameter: the backdrop is sampled at
`p + n₂ · refraction · u²`, so it is strongest at the rim and zero in
the interior. Measured: **240 of 390 rim pixels move**, and **0 interior
pixels do**.

### What this is not

Not Apple's. No chromatic dispersion, no adaptive tint that reads the
backdrop's luminance, no light that responds to device motion, and one
specular lobe rather than the several a real edge shows. What it has is
the part that makes a panel read as a solid object: an edge with
thickness, which bends what is behind it and catches the light.

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
