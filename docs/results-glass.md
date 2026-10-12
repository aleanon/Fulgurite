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

## 0f. T22.4 — press deformation, and the frame loop it exposed

**Measured 2026-09-12.** No new `Material` field and no renderer change:
the widget assembles a material from its declared one and an animated
press value. At a full press the rim thickens by 55% and the refraction
deepens by 85% — chosen, not measured; they are the largest deformation
that still reads as the same panel.

Driven through `Host` — the real pump, the real dispatch, the real
`App::tick` — because `docs/plan-liquid.md` §6 requires it: a
hand-rolled tick loop passes whether or not the frame loop asks about
animation at the right moment, which is half of what this task fixes.

| | |
|---|---:|
| pixels deformed at the peak, clear of the button | **5432** |
| the same with the press ignored | **0** |
| settled frame against the pre-press frame | **0** px differ |

### Three things the fixture had to get right

**The bound was met by the button dimming itself.** Counted frame-wide,
a press changes 2416 px before the panel does anything at all — the
button's own `pressed_dim` over its 92 × 34 rect — which cleared a bound
of 400 on its own. Both injections for this task reported a "peak" of
exactly 2416, which is the button's rect and nothing else. The count is
now taken clear of the button, where the injected figure is 0.

**The click has to land on the button, not the panel.** The panel is
260 px wide and the button 92, so a click at the panel's centre lands on
the panel — which is itself pressable, so the test goes green while
proving only the trivial case. `dispatch` marks the deepest hit node
alone, so pressing the button leaves the panel's own flag `false`
forever; that is why the widget scans its **whole subtree**, and
scanning only its own node measures 0.

**The pointer does not leave when the button is released.** A resting
frame captured before any pointer event differs from the settled one by
the *hover*, and 2416 px of that was charged to the press. The resting
frame is taken with the pointer already in place.

### The derive had no way to advance anything

`DeriveApp` writes the whole `App` impl, so `tick` could only come from
the trait default and **a derived application could not animate at
all** — the panel would retarget on every press and then sit still. The
derive gained `#[app(tick = path)]`, mirroring the existing
`after_update` hook. Removing it from the fixture takes the peak to 0.

### The loop asked the wrong question at the wrong time

`fulgurite-winit` ticked, then pumped, and used the *tick's* answer
to decide whether to request another frame. A press dispatched during
the pump starts an animation that tick could not have seen, so the loop
was told nothing was moving: no redraw, and the rim jumped to its next
value on the following unrelated event. A press that stutters unless you
wiggle the mouse.

The tick stays before the pump — the bindings the pump runs are what put
a ticked value on screen — and the loop asks again afterwards with a
zero `dt`, which `App::tick` now documents as a pure query.
`Animated::tick` satisfies it without a thought: it adds
`clamp(0, 0, duration)` and returns `!settled()`.

## 0g. T22.5 — the morph, and the budget decision it opened with

**Measured 2026-09-12.** Two panels merge through a smooth minimum of
their signed distance fields:

```
h    = clamp(0.5 + 0.5·(b − a)/κ, 0, 1)
smin = b + (a − b)·h − κ·h·(1 − h)
```

### The budget: an offset, and no scale

The task's first act, and it is a measurement. `Material` was 32 B and
`Cmd::Glass`'s payload 52, against a 56 B payload ceiling set by
`Gradient`:

| encoding | `Material` | payload | `Cmd` |
|---|---:|---:|---:|
| today | 32 | 52 | 60 |
| **+ offset, 2 × `i16`** | **36** | **56** | **60** |
| + offset and scale, 3 × `i16` | 40 | 60 | **64** |
| + offset and scale, 3 × `f32` | 44 | 64 | 68 |

**The offset fits exactly and the scale does not.** The plan predicted
three `i16` would land `Material` at 36 with `Cmd` still 60; measured, it
lands at 40 because the `f32` fields force a 4-byte alignment, and `Cmd`
grows to 64 — **+7% on every command in every list**, 8 KB on a
2000-command frame.

That is far cheaper than the plan's framing of the alternative (a second
command variant, `Cmd` at 80, +33%), but it is not free, and the
repository has already made this call once: `Style::focus_ring` is a
`u8` because as an `f32` it took `Style` from 72 to 76. So **the second
blob is the same size as the first**, offset by up to ±2048 px in 1/16
px steps. A uniform scale remains available for +7% if a design ever
needs one.

### κ is arithmetic, not taste

The smooth minimum undershoots by κ/4 where the fields meet, so at a
separation `s` the midpoint sits at `s/2 − κ/4` and **the neck closes
below `s = κ/2`**. At `κ = rim` — which is what "κ ties to `rim`" reads
as — that is 6.5 px on `LIQUID`: the blobs would be apart at *half* a rim
width, and the gate `docs/plan-liquid.md` §9 writes at one rim width
would read empty on a correct renderer. `κ = 3·rim` puts the closing
point at 1.5 rim widths, between the two separations the plan gates on.

| | |
|---|---:|
| inked between the blobs at one rim width (13 px) | **60** of 60 rows |
| the same at three rim widths (39 px) | **0** |
| inside the far blob, as a control | **60** |

### Cost

Over the **same rect**, so the pixel count is identical and only the
per-pixel work differs — a morphing pair inks a larger union, and
charging that area to the morph would measure the fixture:

| rect | one blob | two | added |
|---|---:|---:|---:|
| 400×200 | 6.51 ms | 7.04 | **+8%** |
| 800×300 | 18.33 | 19.60 | **+7%** |

Two distance fields and a normalisation where there was one field. A
panel with no second blob pays none of it: the branch is on the command.

### A lone panel is byte-identical, and that is pinned by a digest

M19 records that glass keeps `fill_rounded_within`'s hard-span coverage
precisely so a panel is pixel-identical to a rounded rect of the same
geometry, and that a pure SDF ramp disagrees by up to 254/255 on a
fractional edge (`docs/plan-glass.md` §4.1). The SDF coverage path is
taken **only when a second blob is present**. The digest
`0x2e4fd518a418f744` was taken from the renderer at the commit before
T22.5, over a fixture with deliberately fractional edges, and is
unchanged by it. Forcing the SDF path for every panel moves it to
`0xc9cfb2682786da79`.

### Two bounds that were wrong, and one gate that could not fire

**The crease bound was 140× the signal.** The neck's largest
adjacent-pixel jump measures **0.1** levels; the first draft bounded it
at 14 because that felt like a visible step. Both injections this gate
exists for — one blob's gradient instead of the blend, and the distance
left undivided by the gradient's length — land at **12.5**, under the
bound and green. It ships at 4.

**The recorder's hash could not be tested through the widget.** `Glass`
emits the union of both blobs, the union determines the offset exactly,
and the rect is hashed — so changing `morph_x` through the widget
reports damage whether or not the material's own fields are in the hash,
and the injection stays green. `Painter::glass` takes a rect and a
material separately, so a caller may hold one still while the other
moves; the gate now drives `Recording` directly with the same rect both
times, where the injection reports `Damage::None`.

## 0h. T22.6 — the filter on the GPU

**Measured 2026-09-12**, NVIDIA GeForce RTX 4070 (Vulkan). Until this
task the GPU backend drew a flat rounded rect where a panel should be —
the one place in the project where the two backends visibly diverged,
which `docs/plan-glass.md` §1.3 recorded as a decision rather than an
omission.

### What it buys

Filter cost, with the same frame minus the glass command subtracted on
both sides — `benches --bin glass-cost`'s method, so the tables read
together:

| panel | software | gpu | |
|---|---:|---:|---:|
| 400×200 @ 720p | 6.83 ms | **0.12** | 55× |
| full screen @ 1080p | 147.87 | **1.44** | 103× |
| full screen @ 4K | 750.14 | **2.84** | 264× |

**A full-screen backdrop filter goes from impossible to routine**: 148 ms
is nine frames at 60 Hz, 1.44 ms is a tenth of one. That is the whole
case for the task.

The software column is larger than §2's: that table measures the
**blur**, whose capture is dilated by the frosted `3σ + 4`, and this one
measures the full liquid material — bevel, Fresnel, inner shadow,
saturation, dispersion and the adaptive tint. `docs/plan-liquid.md` §8
motivated the task by citing §2's 43 ms at 1080p; for the material M22
actually shipped the figure is 148 ms, which makes the case stronger
rather than weaker.

**The first version of this table was timing the queue.** `draw` returns
as soon as the work is submitted — which is what a GPU is for — so every
row read 0.03 ms whatever the resolution, and it reported a 27687×
speedup at 4K. `Gpu::wait` exists for exactly this and its rustdoc says
so.

### A whole screen of it

`examples/glasswall` is the stress case, laid out as a phone home
screen: **twenty-six** backdrop filters at 1290 × 2796 — a status bar, a
4 × 6 grid of clear app icons, and a floating tab bar, over a generated
picture. The icons carry a tint of 20/255, so nearly everything in one
is the wallpaper behind it refracted through a bevelled edge, which is
what the material is for. `just glasswall-bench`, re-measured
2026-09-13:

| | |
|---|---:|
| software | **153 ms** a frame — seven frames a second |
| GPU | **2.1 ms** a frame |
| damage on a frame where nothing changed | `All` |

The wallpaper is a **picture** and that is load-bearing rather than
decorative: the first version of the scene laid it out as a grid of flat
colour tiles, and a clear icon over a uniform patch of colour has
nothing to refract — bending a flat colour gives a flat colour. The
icons read as outlines on coloured squares. Both numbers above moved
slightly when it changed (139 → 153 ms software), because one `Cmd::Image`
replaced twenty-eight rects and the image is sampled per pixel.

**The wallpaper is 470 × 1018 and that bound is the GPU's, not the
design's.** `fulgurite-wgpu` atlases images into a 1024 px texture
and refuses anything larger — drawing nothing, which is `ImageSource`'s
stated answer for a missing asset — while the software canvas has no
atlas and draws any size. A 540 × 1170 wallpaper therefore rendered
correctly headless, correctly in a software window, and **not at all on
the GPU**. It is the only divergence between the two backends this
milestone found that neither the headless parity gate nor a software
test could see, because both halves of each pair agree with themselves.

The damage row is not a defect being demonstrated. `MAX_DEPS` is 4, and
`retain.rs` states the reasoning: a glass panel serialises the
rasteriser, so "a frame with more than a handful is a design problem
rather than a case to optimise". Twenty-six is well past it, every frame
is a full redraw, and **2.2 ms is that worst case** — no incremental
path is being credited here.

**The dependency cap's escape** (M51/T51.3, 2026-09-25). That full
redraw is `dilate`'s `Damage::All` escape, and until T51.3 no test
fired it: `retain.rs` said "no cap and no fallback" directly above it,
and with it deleted every suite stayed green (R9-F3,
`docs/results-deep-review.md`). `crates/fulgurite-shell/tests/glass_deps.rs`
now renders `MAX_DEPS` = 4 and 5 frosted panels with a bar moving under
the last, partial over the reported damage against full, five frames
each:

| panels | partial frames | `All` frames | worst px differing (bound 0) |
|---|---:|---:|---:|
| 4 (`MAX_DEPS`) | 5 | 0 | 0 |
| 5 (`MAX_DEPS` + 1) | 0 | 5 | 0 |
| 5, escape deleted (the injection) | 5 | 0 | **1,822** |

The injected row is the review's figure to the pixel. Software only, so
it proves the same thing on this project's llvmpipe-only machine as on
a real GPU: the rule is the core retention's, and no wgpu code runs.

### How it is structured

A texture cannot be sampled by the pass that is writing it, so the frame
is **split at every glass command**: everything under the panel, then
six blur passes ping-ponging between two scratch targets, then the panel
itself — `G + 1` main passes and `6G` blur passes. The first blur pass
reads the target directly, so no copy is needed.

The material rides a storage buffer indexed per instance, the way `Grad`
already does and for the same reason: it is 112 bytes and almost no
frame has a panel in it.

### The parity gate needs two bounds, and now that is measured

`docs/plan-liquid.md` §8 predicted that one frame-wide count cannot both
admit the blur's disagreement and refuse a missing fringe. Measured:

| | B1, whole panel | B2, rim band |
|---|---:|---:|
| bound, stated first | ≤ 10% over 8 levels | ≤ 8 levels |
| both backends as shipped | **0** of 24 000 (worst delta 3) | **0.5** |
| shader's dispersion deleted | 1.5% — **green** | **18.2** — fires |
| shader's adaptive tint deleted | 97.4% — fires | 0.2 — green |

Each bound catches exactly what the other cannot. The agreement is far
closer than the bounds allow because at the liquid preset the CPU's
downsample factor is 1, so both backends run the same three box passes
at the same widths and the only divergence is 8-bit rounding between
passes.

### The fringe measure was of the wrong quantity, twice

The bounds were pre-registered and neither has moved. What the rim-band
bound *measures* took three attempts:

1. **Mean blue-over-red across the band.** Dominated by the backdrop's
   own colour: over a blue and yellow edge it reads +110 levels whether
   or not the renderer disperses anything, and deleting dispersion moved
   it by 3.3 — under the bound, green.
2. **The same, as a difference against `dispersion: 0`.** Right idea,
   but the *mean of a signed fringe cancels*: a fringe is one sign on one
   side of an edge and the other sign on the other, and it read −1.2.
3. **Per pixel, as `|chroma(on) − chroma(off)|`, compared between the
   backends pixel by pixel.** A shader that skips dispersion contributes
   zero and this reads back the software arm's whole fringe.

And the band was in the wrong place. Profiled, dispersion's contribution
is 32, 21, 5 and 15 levels in columns 60–63 and **exactly zero** from 65
on; the first band ran 61..67, so half of it was dead columns pulling the
mean under the floor. The same mistake T22.0's Fresnel test made, for the
same reason: it is a two-to-four pixel effect.

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

**A 1600×800 panel at 26.9 ms does not fit in a frame**, on this
backend. A toolbar-sized panel is 0.4 ms and fits comfortably.

Two corrections to what this paragraph said when it was written:

- It credited **T19.4's cache** with making a static full-screen panel
  affordable. There is no cache — §5 records it as decided against on
  measurement, and this sentence outlived that decision by a milestone.
- The answer for a large panel is **M22/T22.6's GPU backend**, where the
  same full-screen filter is 1.44 ms at 1080p. §0h.

And this table measures the **blur**: its capture is dilated by
`3σ + 4`, the frosted reach, with no refraction term. The full liquid
material is a different and larger quantity — §0h measures it.

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

## 5a. A small change under a panel, through `Host` — M50/T50.0

`benches --bin glass_damage`, 2026-09-27, quiet (`just bench-lock`,
load1 2.45 before, 2.23 after, bound 8). A 24×24 label under a `Glass`
bar at 1920×1080, changed every frame, pumped through the real `Host`
and drawn over the damage it reports; median of 16 on the software
canvas with the pinned face.

| material | bar | bar px | damage the renderer receives | damage px | ms |
|---|---|---:|---|---:|---:|
| FROSTED | 1920×64 (the `examples/glass` toolbar) | 122 880 | [0,0,1920,104] | 199 680 | **4.04** |
| LIQUID | 1920×64 | 122 880 | [0,0,1920,89] | 170 880 | 10.94 |
| FROSTED | 1920×360 (a sheet) | 691 200 | [0,0,1920,400] | 768 000 | **21.63** |
| LIQUID | 1920×360 | 691 200 | [0,0,1920,385] | 739 200 | 47.63 |
| FROSTED | 1920×1080 (the window) | 2 073 600 | the window | 2 073 600 | **64.40** |
| LIQUID | 1920×1080 | 2 073 600 | the window | 2 073 600 | 145.21 |

**The renderer receives the panel's whole source region for a 576 px
change**, on every row — the panel plus its reach below it — which is
the verifier's finding (R8-F6): `dilate` grows the damage before the
renderer sees it. The cost is the panel's, not the change's.

**The T50.9 kill rule** (`docs/plan-raster-allocation.md` §5) builds the
damage-bounded variant if a small change under a frosted panel costs
more than a quarter of a 16.7 ms frame, 4.17 ms. The toolbar is 3% under
it; a frosted sheet is 5.2× over it and a full-window panel 15×. The rule
names no panel size, and a sheet is an ordinary panel, so it fires.
`LIQUID` cannot take the bounded rule at all (§0e: its tint is a mean
over the panel).

**And then building found the bounded rule unsound for every material**
(M50/T50.9, closed as measured; `tests/glass_damage_bound.rs`). A
partial redraw clears its region and redraws every command clipped to
it, so a panel pixel near the region's edge reads, outside the region,
last frame's *finished* pixels — inside the panel that is the panel's own
previous output, not the backdrop. On a 600×200 frosted panel with a
24×24 change (reach 40 px), a partial redraw over the change plus the
reach leaves **8,530 px** wrong and over the change plus twice the reach
**12,546**; the whole source leaves **0**. Growing the region to cover a
repainted pixel's reads brings more panel pixels in, so **every sound
region contains the whole panel**, and `adapt` has nothing to do with
it. The most a sound rule could shed is the reach ring around the panel
— 37% of the source in that fixture, 38% for the toolbar above, 10% for
the sheet — and only where nothing drawn after the panel reaches the
ring; the panel alone redraws to 0 px wrong in that fixture. Anything
smaller than the panel needs the backdrop kept across frames, which is
§5's cache. The ring variant is not built: it moves the toolbar under the
kill rule's line by a cost the table above already puts under it, and
does nothing for the sheet.

## 5b. The bevel — what makes it look like glass rather than frost

**Measured 2026-09-11.** M21 added `refraction` and `specular` over a
bevel of width `rim`, which is what separates
[`Material::LIQUID`](../crates/fulgurite/src/display.rs) from
`FROSTED`.

**Re-measured 2026-09-27 (M55/T55.8, R9-F11)** — frosted at σ = 3 against
the same material with M21's four bevel fields (`rim`, `refraction`,
`specular`, `light`) and nothing else. `benches --bin glass-cost`, "the
bevel" row, eight runs, median (min–max). **LOADED** (load 6.5 → 9.7 on
32 cores, over the bench lock's bound of 8):

| panel | frosted | + the bevel | the bevel |
|---|---:|---:|---:|
| 400×200 | 4.47 ms | 5.41 ms | **+21%** (17–27%) |
| 800×300 | 13.73 ms | 15.51 ms | **+12.5%** (10–14%) |

Both at σ = 3, so only the bevel differs. (Both are dearer than §1's
table because σ = 3 takes `k = 1` and runs the box passes at full
resolution — the same "a small blur is the expensive one" result.)

**Why it was re-measured.** The 2026-09-11 table (+18% and +5%) compared
`FROSTED` with `LIQUID`, which *was* the bevel then. M22 then added
Fresnel, the inner shadow, saturation, dispersion and the adaptive tint
to `LIQUID`, and the bench kept comparing against it under the comment
"so only the bevel differs" — so the column became the rest of the
liquid material and stopped reproducing. The same eight runs measured
that old comparison at **+43%** (36–52%) and **+34%** (30–61%). The
bench now builds the bevel-only arm itself. §0b owns what the whole
material costs; this section owns the bevel.

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
- **The GPU.** Superseded 2026-09-12: it runs the real filter, not the
  fallback. §0h.
- **Anything but this machine.** One CPU, `powersave`, one scene.
