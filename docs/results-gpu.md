# The GPU renderer: what it buys, what it costs, and how close it is

**Ran 2026-09-07.** Reference machine: i9-13900KF, governor `powersave`,
NVIDIA GeForce RTX 4070 (Vulkan). `cargo run --release -p raster-scaling`
in `benches-competitive/`, min of 30 after 5 warm-ups;
`cargo test -p lumen-static-wgpu` for the parity numbers.

This is the results doc for `docs/plan-capabilities.md` T15.12.

## Why this was built

`docs/results-raster-scaling.md` reversed T15.12's deferral: the software
rasteriser costs ~2.7 ms per megapixel, flat, so a full 4K redraw is
22.88 ms against a 16 ms budget. What holds the desktop together today is
damage tracking, which is load-bearing rather than an optimisation — and
which stops helping exactly when a lot changes at once.

That doc asked the question. This one answers it with the other arm
present.

## 1. Resolution, both backends

Same scene, same `DisplayList`, same process. The CPU column reproduces
`docs/results-raster-scaling.md`'s table (within 2%, which is machine
noise); it is repeated here only because a ratio needs both halves on one
row.

| canvas | Mpx | CPU | GPU | speedup | commands |
|---|---:|---:|---:|---:|---:|
| tally window (440×520) | 0.23 | 0.72 ms | 0.06 ms | 11.9× | 51 |
| 720p | 0.92 | 2.54 ms | 0.14 ms | 18.0× | 201 |
| 1080p | 2.07 | 5.60 ms | 0.29 ms | 19.3× | 481 |
| 1440p | 3.69 | 9.92 ms | 0.50 ms | 19.7× | 881 |
| 4K | 8.29 | **22.87 ms** | **1.10 ms** | 20.8× | 2041 |

> **The CPU column is pre-M17, 2026-09-09.** The software rasteriser is
> now 1.2–1.3× faster (`docs/results-simd.md`): 4K is 18.17 ms scalar,
> 17.65 with `simd`, so the speed-up at 4K is **16.1×** rather than
> 20.8×. Still over budget on the CPU, still comfortably under on the
> GPU; the conclusion is unchanged and the ratio is not.

### Re-run 2026-09-22, after M25–M30

| canvas | Mpx | CPU | GPU | speedup | commands |
|---|---:|---:|---:|---:|---:|
| tally window (440×520) | 0.23 | 0.64 ms | 0.06 ms | 10.1× | 51 |
| 720p | 0.92 | 2.24 ms | 0.14 ms | 16.2× | 201 |
| 1080p | 2.07 | 4.89 ms | 0.28 ms | 17.4× | 481 |
| 1440p | 3.69 | 8.57 ms | 0.49 ms | 17.5× | 881 |
| 4K | 8.29 | **19.48 ms** | **1.10 ms** | 17.6× | 2041 |

**The CPU side got faster and the GPU side did not move at all** — 1.10 ms
at 4K in both runs, to two decimal places, which is what a GPU doing 8.3
Mpx of nothing-much looks like. Per megapixel the software rasteriser is
now 2.32–2.43 ms against 2.7 in 2026-09-07's run, so the ratio narrowed
from 20.8× to 17.6× because *we* improved, not because the GPU regressed.
A full 4K redraw is still 117% of a 16.7 ms budget on the CPU and **6.6%
of one on the GPU**, so the conclusion this document was written to reach
is unchanged.

**`just raster-sweep` was broken the whole time between the two runs.**
`raster-scaling` gained a second binary (`bands`, M16's threading work)
and `cargo run -p` then refuses as ambiguous, so the recipe exited 101.
Fixed with `--bin`. Nothing was wrong with the measurement; it simply
could not be taken by the command that documents how to take it.

### 1b. When is the GPU actually worth it — the crossover

1080p, damaged fraction × command count, min of 20, same re-run. Each
cell is CPU/GPU in ms:

| cmds | 1% damage | 2% | 3% | 5% | 10% |
|---:|---|---|---|---|---|
| 16 | 0.03/0.04 **cpu** | 0.03/0.04 **cpu** | 0.04/0.04 **cpu** | 0.05/0.04 **gpu** | 0.06/0.04 **gpu** |
| 160 | 0.03/0.04 **cpu** | 0.03/0.04 **cpu** | 0.04/0.04 **cpu** | 0.05/0.04 **gpu** | 0.09/0.05 **gpu** |
| 1600 | 0.03/0.04 **cpu** | 0.04/0.04 **cpu** | 0.04/0.05 **cpu** | 0.05/0.05 **gpu** | 0.09/0.05 **gpu** |

**The crossover is damage, not complexity.** It sits between 3% and 5% of
the surface at every command count tested — a hundredfold change in
command count moves it not at all, because the GPU's cost here is
per-*pixel* dispatch overhead and the CPU's is per-pixel work. So the
rule a shell needs is one number, not a heuristic: below ~3% damage the
software path wins outright, and it is what a typical interaction
produces (M29's grid damages 770–990 px of 1.26 M, which is 0.06%).

**A full 4K redraw goes from 143% of a frame budget to 6.6% of one.**
That is the whole argument, and it is the one number the deferral was
missing.

Two things the table says that the headline does not:

- **The GPU is not free at small sizes.** 0.06 ms at 0.23 Mpx is mostly
  fixed cost — buffer upload, pass setup, submission — and the speedup
  climbs from 11.9× to 20.8× as that floor is amortised. A tally-sized
  window is *not* where this pays.
- **It is still linear**, at ~0.13 ms per megapixel against the CPU's
  ~2.7. A GPU rasteriser is pixel-bound too; it just has twenty times
  the fill rate.

`draw` is timed with a `Gpu::wait` after it. Without that wait the GPU
column would be measuring queue submission and reporting it as a render,
which would flatter this arm by about an order of magnitude.

## 2. Do the two backends agree?

The gate T15.12 set: *"the same frame renders pixel-comparably on both
backends (within an antialiasing tolerance, stated)."*

**A skip is not a pass (M43/T43.10).** Without an adapter the parity
tests in `crates/lumen-static-wgpu/tests/` return early, and libtest
captures their "skipping" line on a pass — so `just gpu` was green on a
machine that compared nothing (M38's R17-F6). The policy now: a plain
`cargo test` still skips, because a developer's partial run is not a gate;
`just gpu`, the recipe that claims the comparison happened, sets
`LUMEN_REQUIRE_GPU=1`, under which a missing adapter panics with
`LUMEN_REQUIRE_GPU=1 but no adapter`, and prints the adapter it used
(`adapter: <name> (<backend>, <device type>)`, from `the_gpu_half_ran`; the device type since M51, because llvmpipe is an adapter for parity and not evidence about a discrete GPU).
`LUMEN_ALLOW_SKIP=1` is the explicit way to run it without one.

> **Superseded 2026-09-27 by per-region bounds (M51/T51.1, §2b below).**
> The whole-frame bound this section states could not see the one shape
> the frame existed to hold (R9-F7), and "every command in the
> vocabulary" was false: the frame drew no `Image` (R9-F2). The table
> and prose below are kept as the record of what was claimed.

**Stated: fewer than 0.2% of pixels may differ by more than 8 levels.**
Measured, on a frame using every command in the vocabulary:

| frame | worst channel delta | pixels over 8 |
|---|---:|---:|
| `tests/parity.rs`, 240×180 | 96 | 4 (0.01%) |
| `examples/compare`, 480×320, text at three sizes | 2 | 0 (0.000%) |
| a live 320×160 window, same binary both ways | 1 | 0 (0.000%) |

The bound has ~20× headroom over the worst of these and is still tight
enough to catch a shape that moved by one pixel, which would put a whole
edge over the line rather than four corners.

**Where they differ, and why it is only there.** The software canvas
fills a straight edge in whole pixels — `floor(x0)` to `ceil(x1)` — and
antialiases only at a corner. The GPU antialiases everything, because a
signed-distance field has no reason not to. So a shape whose edge lands
*between* pixels differs by up to one pixel of ramp, and a shape on whole
pixels does not differ at all. `tolerance_is_bounded_at_fractional_edges`
isolates exactly that case and asserts the direction: the software arm is
the *more* covered one.

Nothing else differs, and three things were done to keep it that way:

- **The same shaping path.** `FontStack::place_run` was split out of
  `FontStack::glyphs` so both renderers shape through one function; the
  GPU adds only an atlas in front of `FontStack::rasterize`. Glyph
  positions are therefore identical by construction, not by agreement,
  and the text band of the comparison frame has the same 315 inked
  pixels on both arms.
- **The same coverage arithmetic.** `shader.wgsl` transcribes
  `fill_rounded_within`, `shadow_within`, `stroke_line` and
  `Stops::sample` rather than reimplementing them. `0.5 - signed
  distance` *is* the software canvas's `rad + 0.5 - dist`.
- **The same colour space.** The target is deliberately `Rgba8Unorm` and
  not `*Srgb`. Blending in linear space is the more correct thing to do
  and is not what `software::Canvas` does; matching the other arm is
  worth more here than being right in isolation.

### 2b. By region, on both adapter classes — M51/T51.1, 2026-09-27

**The machine has a working GPU again**, so the plan's "discrete
baseline owed" rows are measured, not owed: the RTX 4070 (Vulkan,
`DiscreteGpu`) on the default loader, and llvmpipe (Vulkan, `Cpu`)
through lavapipe (`VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json`).
Each gate's first line names the adapter and its device type.

`tests/parity.rs` `every_region_is_within_its_own_bound`: the frame is
ten named shapes (eleven since M82/T82.6 added a path) in disjoint boxes, each counted alone (pixels over 8
levels) against `2 × b + 2`, where `b` is the larger of the two
adapters' baselines — twice, so a doubled baseline is green and a shape
that moved a pixel is not; plus two, so a zero baseline is not a bound
of zero. Everything outside every box is the page and must agree
exactly. "Every command" is now `kind`, an exhaustive `match` over
`Cmd`: a new command does not compile there, and
`the_frame_uses_every_command` fails until the frame draws it or `kind`
names the file that does (glass: `tests/glass_parity.rs`).

| region | RTX 4070 | llvmpipe | bound |
|---|---:|---:|---:|
| rect | 0 | 0 | 2 |
| rounded rect | 0 | 0 | 2 |
| clip scope | 0 | 0 | 2 |
| text | 0 | 0 | 2 |
| shadow | **54** | 0 | 110 |
| line | 4 | 0 | 10 |
| gradient | 0 | 0 | 2 |
| translucent rect | 0 | 0 | 2 |
| image (16×16 card at 2×) | 0 | 0 | 2 |
| border | 0 | 0 | 2 |
| path, filled and stroked (M82/T82.6) | 0 | 0 | 2 |
| page, outside every box | 0 | 0 | 0 |
| whole frame, for the record | 58 (0.13%) | 0 | — |
| hairlines through `Scaled`, 1.25 and 1.5 (M40's arm) | 0, 0 | 0, 0 | 2 |

Injections, each red by region name on both adapters:

| injection | red output |
|---|---|
| binary ring coverage in the software border | `border: 56 px over 8 (bound 2)`; the whole frame, which the pre-M51 test bounded, read 60 px (0.14%) on the RTX in the T51.0 frame: green under its 86 |
| the GPU draws no `Image` | `image: 1024 px over 8 (bound 2)`; `images_agree_across_the_backends`: `image 1:1 is not drawn: inked 196/0 of 196` |
| the software canvas draws no `Image` | the same region, and `lumen-static-shell/tests/image_pixels.rs` (in `just test`): `256 of 256 px differ from their texel` |
| the image removed from the frame | `commands the parity frame does not draw: ["Image"]` |

`images_agree_across_the_backends` draws the card at 1:1, 2×, tinted 3×,
alpha 128 over a dark bar, and a fractional placement: 0 px over 8
inside each on both adapters, and every interior pixel inked. The
software twin asserts every pixel of four placements equals the texel it
names, blended as `Canvas` documents: 0 of 4,608 differ.

**The shadow's 54 px are one row, and they are a tie.**
`DisplayList::shadow` drops the box by `0.35 × spread` (3.5 px at a
spread of 10), so with whole-pixel card edges the row one pixel inside
the dropped box's bottom edge lands at a signed distance of exactly −1 —
the threshold both backends use to skip "interior" pixels (`d >= -1`
keeps a pixel in software; `d < -1` discards it in `shader.wgsl`).
Which side a GPU lands on is float noise that depends on where the quad
is: here the RTX drops the row and llvmpipe keeps it; at the pre-M51
frame's shadow position it was the reverse, which is the 56 px of
llvmpipe's whole-frame baseline in the review (`docs/results-deep-review.md`
R9-F7). The bound therefore follows the larger baseline, as
`docs/plan-gpu-fidelity.md` §2.2 says it must.

**The tie exposed a defect it is not part of, on both backends.** The
skip exists because "whatever cast the shadow paints over" its
interior — but the box is *dropped*, and the caster is not, so the rows
between the card's bottom edge and the dropped box's `d = −1` line are
skipped and nothing paints them: a column under this frame's card reads
card, card, **page, page**, shadow (30, 30, 250, 250, 171 on the red
channel, both backends identically). Every card with a shadow has a
two-pixel page-coloured seam along its bottom edge, and at some positions
a third row on the GPU. Not fixed here: it changes every shadow on both
rasterisers and every golden that holds one, so it is reported to the
index rather than folded into a parity task
(`docs/plan-gpu-fidelity.md` T51.1, "What building found").

### 2a. At a fractional display scale — M40/T40.3, 2026-09-25

**The 0.2% bound above was measured on whole-pixel edges, and nothing
painted through `Scaled` at a scale other than 1.** At 1.25 every edge of
a 1-logical-px divider lands between pixels; the software canvas filled
both boundary rows whole and the GPU ramped them, so the documented ramp
difference became every hairline in the frame (R9-F5,
`docs/results-deep-review.md`).

**What changed:** `Scaled::rect` rounds each scaled edge to the nearest
device pixel (ties to even), independently per edge, and keeps at least
one pixel per axis (`docs/plan-physical-damage.md` §2.2). A plain rect
therefore reaches both rasterisers with whole-pixel edges, which is the
case the two already agreed on. Rounded and stroked rects, text, lines,
gradients, images and glass are unchanged. **A hairline is one or two
device pixels at 1.25 and 1.5, depending on where it sits** — the price
of edges that stay shared.

| gate | before | after | bound |
|---|---:|---:|---:|
| `parity.rs` `hairlines_through_scaled_agree_at_fractional_scales`, 1.25 | 4 772 px (11.046%) over 8, worst 250 | 0 (0.000%), worst 0 | 0.2% |
| same, 1.5 | 2 853 px (6.604%), worst 210 | 0 (0.000%), worst 0 | 0.2% |
| `fractional_ink.rs`, worst divider ink / snapped geometry, 1.25 | 2.016 | 1.000 | 1.00 |
| same, six dividers' ink / geometric area, 1.25 | 1.613 | 1.067 | — (informational) |
| same, a shared fractional edge at 1.25 (edge 10 → 12.5) | overlap row 12 | no gap, no overlap | none |

The 0.2% bound therefore holds at 1.25 and 1.5 for plain rects. The 1.0
and 2.0 arms are exact before and after.

**The kill rule** (plan §5, written before the build): showcase's paint
time at 1.0, eight runs a side; if the after-median left the before-arm's
spread, the snap would be restricted to scales other than 1.0. Release,
`examples/showcase` `snap_cost::measure_the_snap`, a moving hover, paint
alone (`Host::timings3`), before and after alternated twice under
`flock`, load 1.2–1.7:

| arm | before, two sessions (µs, spread) | after (µs, median) |
|---|---|---:|
| showcase `Button`, 0 of 15 cmds a plain rect | 0.294–0.297, 0.294–0.297 | **0.295** |
| showcase `DataGrid`, 1 of 32 | 0.738–0.757, 0.737–0.749 | **0.752** |
| 2 000 rects through `Scaled(1.0)` | 21.837–21.849, 21.835–21.845 | **29.458** |

The first after session was disturbed (its `DataGrid` spread was
0.905–1.280, five times either before session's) and is not used; the
second is. Against the pooled before spread (0.294–0.297; 0.737–0.757)
**both showcase medians are inside it, so the rule does not fire** and
the snap applies at 1.0 too, where a fractional *layout* edge over-inked
by the same mechanism.

**What the rule could not see**: showcase paints almost no plain rects
(the counts above), so its arms bound nothing about a per-rect cost. The
isolated arm does: **+3.8 ns a rect** at 1.0 (21.84 → 29.46 µs for
2 000). That is the final form, which rounds by adding and subtracting
1.5 · 2²³. The first form rounded with `crate::num::floor` and a compare,
and cost 20.2 → 52.2 µs on the same arm (2.6×), branch-free or not, so
it was replaced before it shipped.

## 3. Damage on the GPU path

T15.12's second gate: *"`damage_oracle`'s partial-redraw property holds
on the GPU path too."*

It holds, and **exactly** — not within a tolerance. Rendering only a
damaged region on top of the previous frame is bit-identical to a full
redraw of the same list. Two properties make that true, and both were
found by injecting the fault rather than by reasoning:

- **The damage region is snapped outward to whole pixels.** Without it,
  the antialiased clear leaves a boundary pixel at 70% of the page colour
  over 30% of the last frame. Under opaque content that is invisible —
  the next command overwrites it — which is why the first version of the
  test passed with the snapping removed. Under *translucent* content it
  is not: the fault injection produces 23 differing pixels, up to 19
  levels. The test now damages a region inside a translucent rect for
  that reason.
- **The clip test in the shader is hard, never antialiased.** A boundary
  pixel is wholly redrawn or wholly left alone. Removing the damage scope
  from the clip stack differs on 4,845 pixels by up to 225 levels.

There is also one thing the GPU path gets for free that the softbuffer
path spends a page of `retain.rs` on: **buffer age is always 1.** The
renderer draws into a texture it owns and blits that to the swapchain, so
the surface it accumulates into is never handed to a presentation engine
and never comes back holding a frame from two presents ago. The cost is
one full-screen textured triangle per frame.

## 4. What the design is

One pipeline, one draw call, one instanced quad per command, in paint
order.

Batching by kind would be faster and would be wrong: instances rasterise
in submission order, and that ordering *is* the display list's ordering.
Sorting glyphs into their own batch would put a popup's text underneath
its own background.

Everything in the `Cmd` vocabulary is a signed-distance quad — rect,
rounded rect, shadow, line, gradient — plus two atlas lookups, for glyphs
(R8) and images (RGBA8). Gradient stops live in a storage buffer rather
than in the instance: four stops are 96 bytes, almost no frame has a
gradient in it, and paying for them per instance would make every rect
60% larger to describe something it is not.

The glyph atlas is shelf-packed and reset-on-overflow: when a frame needs
more room than the atlas has left, it is emptied and the frame is *built
again*. Rebuilding costs one extra shaping pass in a frame that was
already exceptional.

> **Corrected 2026-09-27 (M51/T51.1, R9-F1).** The paragraph above went
> on to say the frame is built again "rather than finishing with glyphs
> missing". That was false whenever a frame's glyphs did not fit an
> *empty* 1024² atlas: the second build gave up, submitted what fitted,
> and counted and logged nothing. Printable ASCII in eight sizes and two
> weights through `Scaled(2)` at 3840×2160 did it on every frame, with
> 5.0% of the text's ink missing (the review's corpus, other sizes,
> measured 10.6%).
>
> Now a frame that does not fit an empty glyph atlas **doubles it and is
> built again**, once per doubling, up to the device's
> `max_texture_dimension_2d` (32,768 on the RTX, 16,384 on llvmpipe). Only past
> that limit, or with an image set larger than the image atlas, is
> anything dropped, and then `Gpu::atlas_stats().give_ups` counts it and
> the first one is logged. The atlas never shrinks within a renderer's
> life. `tests/parity.rs`, both adapters identically:
>
> | corpus | resets / growths / give-ups, frame 1 then 2 | atlas | ink software / GPU |
> |---|---|---|---|
> | scale 1 | (0,0,0), (0,0,0) | 1024² | 74,005 / 74,005 |
> | scale 2, before | (1,–,1), (1,–,1) | 1024² | 297,133 / 282,306 |
> | scale 2, after | (1,1,0), (0,0,0) | **2048²** | 297,133 / 297,133 |
>
> One doubling is enough, so the plan's scope rule (a better packer
> first, if 2048 did not fit) did not fire. A 2048² R8 atlas is 4 MB
> against 1 MB, paid only by a renderer whose frame needed it. With
> growth disabled the scale-2 case reads `(1,0,1), (1,0,1)` and fails
> `the atlas gave up 2 times`; with the atlas started at 64 the scale-1
> case reads `(1,4,0)` and fails on its "nothing to reset, grow or give
> up" assertion, which is the old inverse check (an atlas of 64 left
> parity green) now red.
>
> **`app.perf` reports it** as `atlas_give_ups` (the newest frame's
> running count; `null` from the software canvas), since M48/T48.3 made
> `app.perf` carry measured frames at all. `just gpu-parity`'s live leg
> runs a debug showcase with `LUMEN_GPU_ATLAS_MAX=64` (a debug-only cap,
> like `LUMEN_GPU_DROP_FIRST`) on the RTX: `app.perf atlas_give_ups = 1;
> the renderer logged 1 give-up line(s)`; uncapped, `= 0` and 0 lines.
> With `app.perf` dropping the field, the capped leg fails `app.perf
> carried no atlas count`, and `protocol.rs`
> `perf_reports_the_atlas_give_up_count` fails `app.perf lost the
> renderer's give-up count`.

> **Path masks, since M82/T82.6 (2026-10-06).** A `Cmd::Path` is drawn
> as R8 coverage in the **glyph** atlas, not by stencil-then-cover: the
> backend has no stencil attachment and runs at one sample. Each
> non-transparent paint of a path is its own mask under its own
> `MaskKey::Path` (a filled and stroked path is two), rasterised on a
> miss by the software canvas's `rasterize_path` at the canvas's own
> origin and drawn by the `KIND_GLYPH` quads text uses, in `PATH_TILE`
> (256 px) tiles. The key hashes the path at its ink box's integer floor,
> so a whole-pixel scroll is an atlas hit. `tests/parity.rs`, RTX 4070
> and llvmpipe:
>
> | gate | RTX 4070 | llvmpipe | bound |
> |---|---|---|---|
> | each paint alone over the page (star fill, circle fill and stroke, rotated bar stroke), worst channel delta | 1, 1, 1, 1 | 1, 1, 1, 1 | 1 |
> | the whole scene (the circle's stroke over its translucent fill) | worst **1**, 0 px over 1 | worst **2**, 31 px over 1 | 2 (one per blend) |
> | masks rasterised: first frame / idle / changed path / moved 1 px | 1 / 0 / 1 / 0 | 1 / 0 / 1 / 0 | – / 0 / 1 / 0 |
> | a 2,000 × 100 px path: tiles, resets, growths | 8, 0, 0 | 8, 0, 0 | –, 0, 0 |
> | live, `just gpu-parity`'s PaintBox leg (1120×760) | worst **3**, 0 px over 8 | — | 1% over 8 |
> | K82d, PaintBox's first GPU frame | **15,580** texels in 2 tiles, **1.49%** of 1024² | — | 25% |
>
> The injections and their red numbers are in
> `docs/results-paint-vocabulary.md` T82.6.

## 5. What it costs, and why the CPU is still the default

Three axes, measured today. The crate count is unique packages in
`cargo tree -p lumen-static-winit --prefix none`; the binary is
`examples/minimal` built `--release`, unstripped; RSS is the median of
three launches read from `VmRSS` four seconds after start, which is the
method `docs/results-competitive.md` §3 uses.

| | crates | binary | idle RSS |
|---|---:|---:|---:|
| software (default) | 101 | 2.71 MB | 25.6 MB |
| `--features gpu`, GPU in use | 138 | 6.58 MB | **199.3 MB** |
| `--features gpu`, `LUMEN_BACKEND=software` | 138 | 6.58 MB | **26.2 MB** |

> **Corrected 2026-09-08 (M16/T16.5). The binary column was wrong, and
> wrong in the GPU's disfavour.** It first read 5.29 and 12.26 MB, taken
> from `examples/minimal` built in the *framework* workspace — which sets
> no `[profile.release]` at all: unstripped, no LTO, sixteen codegen
> units. `docs/results-competitive.md` §4's 2.7 MB comes from
> `benches-competitive`, which sets `lto=true, codegen-units=1,
> strip=true, panic="abort"`. Putting the two in one column compared a
> debug-shaped binary with a shipped one.
>
> Re-measured with both arms built under the competitive workspace's
> profile, one `cargo` invocation each, the GPU backend costs
> **+3.87 MB**, not +6.97 — the old figure overstated it by 1.8×. The
> crate count and the RSS rows were never affected.

**The price is the context, not the code.** Compiling the GPU backend in
and not using it costs 0.6 MB resident. Opening an adapter costs 174 MB,
which lands `lumen-static` squarely among the GPU-backed arms of
`docs/results-competitive.md` §3 — iced at 196.3, Lumen at 169.6 — and
confirms that doc's guess that the gap over them "is mostly a GPU
context". It was.

That is the whole case for keeping the software canvas the default. A
form at 1080p should pay 5.6 ms and save 174 MB; a chart animating at 4K
should not. The choice is per-application (`Config::backend`), per-run
(`LUMEN_BACKEND`), and per-build (the `gpu` feature), and none of the
three is the framework's to make.

**The blur scratch is paid for by glass only — M51/T51.5, 2026-09-27.**
The backdrop filter's two ping-pong targets are surface-sized
`Rgba8Unorm`, 8 bytes a pixel together: 16.6 MB at 1080p and 66.4 MB at
4K (arithmetic on their descriptors, not a driver query). `build` and
every `resize` made them whether or not a `Cmd::Glass` was ever drawn
(R9-F9). They are now made by the first frame that holds a glass
command and resized only once they exist; until then group 1 names a
1×1 placeholder, because every pass binds it. `glass_parity.rs`
`the_blur_scratch_is_made_on_the_first_glass_command`, at 320×240 then
360×260: no glass after a frame 0 B, after a resize 0 B, one panel
748,800 B, a second glass frame 748,800 B (want 0, 0, 748,800, 748,800);
before, 614,400 and 748,800 B with no glass at all. Injections: eager
allocation at `build` (614,400 B), an unguarded resize (748,800 B), and
the lazy creation removed (B1 100% of the panel over 8, B2 fringe 0.0),
each red by name. On llvmpipe the pages are lazily committed and RSS
cannot show the saving; on the RTX 4070 it is committed VRAM that this
change stops allocating, and its size is the arithmetic above — no
per-process VRAM figure was taken.

For scale against the other optional features on the same crate, by the
same count: clipboard +5 crates, dialogs +24, GPU +37. (The +11 and +79
recorded in `crates/lumen-static-winit/Cargo.toml` for the first two are
from 2026-09-05/07 on a different counting method and an older lockfile;
the ordering agrees, the absolutes do not, and neither has been re-run.)

## 5b. When the GPU *loses* — the crossover

**Added 2026-09-08 (M16/T16.5), because §1 on its own reads more
favourably than the whole measurement supports.**

§1's 20× is a *full-surface redraw*. On the incremental path — which is
what the desktop actually runs, and what `Host::pump` produces every
frame — the ordering inverts. From the per-stage profiler
(`benches-competitive/arms/stages`), raster only, µs:

| workload | damaged | N=100 | N=1 000 | N=3 000 | N=10 000 |
|---|---|---|---|---|---|
| one row, software | 2 002 px (0.6%) | **4.9** | **7.6** | **12.4** | **29.9** |
| one row, GPU | 2 002 px (0.6%) | 33.3 | 35.6 | 40.4 | 87.7 |
| all rows, software | 96 800 px (30%) | 128.0 | 128.9 | **135.4** | **157.6** |
| all rows, GPU | 96 800 px (30%) | **65.8** | **69.1** | 143.5 | 190.8 |

**The GPU loses every damage-clipped frame measured**, by 1.5–6.8×. It
wins the all-rows case at small N and loses it again by 10 000. So the
backend is not simply faster; it is faster on one axis and slower on
another, and both axes move.

Two costs are visible and they pull in opposite directions:

- **A fixed floor per frame** — buffer upload, pass setup, submission,
  and on this path a `wait`. It is what makes 2 002 damaged pixels cost
  33 µs on the GPU and 5 µs on the CPU.
- **A per-command cost that the software path does not pay.** With the
  damaged area held at 96 800 px, GPU raster grows 65.8 → 190.8 µs from
  100 to 10 000 rows while software grows 128.0 → 157.6. Both walk and
  cull the same list, so the GPU's extra growth has another source —
  instance building is the obvious suspect and has not yet been
  profiled.

**What a reader should take from this:** enable the GPU for large
repaints at high resolution — a chart animating, a canvas, a list
scrolling, anything at 1440p and above where §1's numbers apply. Leave
it off for a form or a tally, where damage is a rect and the software
canvas is several times faster *and* 174 MB lighter.

### Where the curves cross (M16/T16.13, 2026-09-08)

Swept, at 1080p, over the two variables the table above showed moving
the answer. Each cell is cpu/gpu in milliseconds, min of 20:

| commands | 1% | 2% | 3% | 5% | 10% |
|---:|---|---|---|---|---|
| 16 | 0.03/0.04 **cpu** | 0.03/0.04 **cpu** | 0.04/0.04 **cpu** | 0.05/0.04 **gpu** | 0.10/0.06 **gpu** |
| 160 | 0.04/0.06 **cpu** | 0.06/0.06 **cpu** | 0.04/0.04 **cpu** | 0.05/0.04 **gpu** | 0.09/0.05 **gpu** |
| 1 600 | 0.03/0.05 **cpu** | 0.05/0.05 **cpu** | 0.07/0.06 **gpu** | 0.08/0.06 **gpu** | 0.11/0.06 **gpu** |

**The crossover is 3–5% of the surface**, arriving earlier as the frame
gets busier.

> **Re-measured after M17, 2026-09-09: it is 5% at every command
> count.** The faster CPU rasteriser pushed the one cell that had gone
> to the GPU at 3% (1 600 commands) back to the CPU. The shape is the
> same and the rule of thumb below is unchanged; the boundary moved by
> one column, in the direction a faster CPU predicts. Below it the GPU's fixed per-frame cost — buffer upload,
pass setup, submission — dominates a job too small to amortise it.
Above it the CPU's per-pixel cost takes over and never gives it back:
§1's 20× at 4K is the same curve at 100%.

This reconciles §5b rather than contradicting it. Those numbers were
2 002 damaged pixels on a 320 000-pixel surface — **0.6%**, squarely in
the region where the CPU wins — and the all-rows row was 30%, where the
GPU won at small command counts. Both readings were correct and neither
was the whole shape.

**The rule of thumb this yields:** the GPU is the right backend when a
frame repaints more than about a twentieth of the window. A form or a
tally, which dirty a rect, should stay on the software canvas; a chart
animating, a canvas being drawn on, or a list scrolling should not.

Two things this still does not answer: the per-command cost noted above
is unprofiled, and whether the backend should switch on this threshold
by itself is deliberately open — a backend that changes under load is a
backend whose frame times are bimodal, and that trade has not been
measured. `docs/plan-invalidation.md` T16.13.

## 6. The fallback, end to end

The GPU is chosen at *runtime*, not at compile time, because an adapter
can be absent on a machine the binary was built for: no driver, no
`/dev/dri`, a remote session. `pick_backend` tries the GPU when the
feature is on and `Prefer` allows it, prints why it fell back if it
cannot, and returns the software canvas. **A window always opens.**

Which one you got is printed at startup and available from
`Backend::name`, because "it is slow" should not require guessing.

### A CPU rasteriser is not a GPU under `Auto` — M51/T51.2, 2026-09-27

**The paragraph above was false on exactly the machine it describes.**
`Gpu::build` asked wgpu for any adapter and never looked at its device
type, so where Mesa is installed and no driver works the one adapter
found was llvmpipe, and `Prefer::Auto` took it. The review measured it
at 1080p: a full frame 2.673 ms against the software canvas's 0.822
(3.3×), a caret 0.146 against 0.037 ms (3.9×), and RSS 3.7 → 219.7 MB
with 133 threads after one frame (`docs/results-deep-review.md` R9-F6,
8 alternating rounds, LOADED).

Now `accept_adapter(device_type, accept_cpu)` refuses a `Cpu` adapter
unless `accept_cpu`, before any device is made, and prints why once.
`pick_backend` passes `accept_cpu` for `Prefer::Gpu` only (the decision
is `choose_backend`), so `LUMEN_BACKEND=gpu` still opens llvmpipe on
purpose, and `Gpu::headless` accepts any adapter so parity stays
executable there. `Other` and `VirtualGpu` are accepted: NVIDIA's GL
adapter reports `Other` on this machine, and a virtualised GPU that
reports an odd type is still a GPU.

| gate | base | the device-type check deleted |
|---|---|---|
| `accept_adapter` unit table, 10 cases | 10 right | `wrong answers: [(Cpu, false)]` |
| `cpu_adapter.rs`, the decision, 5 cases | 5 right | (flag always `true`) `Auto … left: Gpu { accept_cpu: true }` |
| `cpu_adapter.rs` live, lavapipe-only loader | `Auto: software; Gpu: gpu (llvmpipe … (Vulkan, Cpu))` | `Auto: gpu (llvmpipe …)` → `Prefer::Auto opened a CPU rasteriser` |

The live half needs a CPU-only machine. `just gpu` makes this one into
one — the Vulkan loader (VK_ICD_FILENAMES) at lavapipe, the EGL vendor list at nothing —
under `LUMEN_REQUIRE_CPU_ONLY=1`, where a visible GPU fails instead of
reporting "NOT RUN". The other half, that `Auto` still takes a discrete
GPU, is live on this machine's default loader (the RTX 4070) and covered
by the table.

The live check that the two are interchangeable is `just gpu-parity`: it
builds `examples/minimal` once, runs *that one binary* twice — plain and
`LUMEN_BACKEND=software` — and reads the window back off the X server
both times. Measured 2026-09-07 at 320×160: **worst channel delta 1, no
pixel over 8.** It skips rather than fails without a display, an adapter
or `xwd`.

It exists because "rendered" and "shown" are different claims and this
project has been caught by the gap: the first partial redraw skipped the
present entirely and left another window's pixels on screen
(`crates/lumen-static-winit/src/surface.rs`). No headless test sees that.

The GPU backend deliberately answers `None` to `Backend::fetch_rgba`. It
*could* read its target back, but the target is what was rendered, not
what was presented, and that method exists to see the present.
Answering with the pre-present texture would make
`ui.screenshot { source: "surface" }` report that a frame reached the
screen when it did not. `None` means "not checkable", which is true.

### A dropped present is asked for again — M47/T47.4, 2026-09-26

`Gpu::present` returned `()` and swallowed Outdated, Lost and Timeout,
saying that dropping one frame "is what every other backend does here
too" — false, softbuffer panics. The window asks for another frame only
while something moves, so on an idle window a dropped present stayed
unpainted until input (R7-F7). `present` now answers whether the frame
got there, `GpuBackend` sets `Frame::dropped`, and the window asks again
up to `MAX_PRESENT_RETRIES` (3) times in a row (`present_retry`). The
target still holds the whole frame, so the retry shows it without a
re-render.

`just gpu-parity` gained the live half, on a debug `examples/minimal`
with the fault switch `LUMEN_GPU_DROP_FIRST=n` (320×160, idle 3 s, one
`xwd` read, against the presented GPU frame; bound 1% of pixels over a
channel delta of 8):

| n dropped | before T47.4 | after | |
|---|---|---|---|
| 0 | 0 px differ | 0 px | the accept leg |
| 2 | **51,105 px (99.8%)** | **0 px** | |
| 20 | — | 4 presents attempted in 3 s (bound 8) | the retry stops: 1 + 3 retries |

Injections: the redraw on a drop removed — `50395 px (98.428%) differ …
nothing asked for another one (R7-F7)`; the bound removed — the unit
test fires (`[true ×6]` against three), and live `the window attempted
20 presents` (bound 8). The plan's first form of the check, "more than
one distinct pixel value", could not go red: with every present dropped
the window read back 2,578 distinct values, the X server's pixels under
it, so the step compares against the real frame instead.

## 7. What has not been done

- **No layer tree, and no second thread.** Handoff §2's item 4 — "the GPU
  keeps its own retained copy… an unchanged frame is a few quads plus a
  damage rect" — is *half* built. There is one persistent target texture
  and a damage scissor; there is no per-subtree layer, no transform-only
  frame, and raster is on the UI thread.
- **No MSAA.** Coverage is analytic and single-sampled, which is what
  makes the software comparison meaningful. A backend that wanted 4× MSAA
  would be a better renderer and a worse oracle.
- **Images larger than the 1024² atlas are refused, not scaled.**
  Silently resampling someone's asset is a worse surprise than a missing
  one, and `ImageSource` already says a missing image draws nothing.
- **No text-only or shadow-only sweep.** The scene is mixed on purpose;
  attributing the 20× to a stage has not been done.
- **The crossover** is §5b's: 3–5% of the surface. This bullet said it
  was "bounded but not located" after §5b had located it (R9-F12,
  corrected by M55/T55.8), so it now points there instead of restating.
- **Multi-window is software-only.** `Pane` still owns a `Surface` and a
  `Canvas` directly rather than a `Box<dyn Backend>`. Nothing prevents
  it; it was not needed, and a second window is a second GPU context —
  which, at 174 MB, is a decision rather than a detail.
- **Nothing is threaded.** Raster is still on the UI thread; `draw`
  returns once the work is queued, but nothing overlaps frame N's layout
  with frame N−1's raster.
