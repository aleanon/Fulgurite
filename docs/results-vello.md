# Our software rasteriser against Vello's

**Measured 2026-09-09.** Reference machine: i9-13900KF (8P + 16E, 32
threads), governor `powersave`. `just vello`, one scene, min of 30.
`vello_cpu` 0.2.0.

## Why this arm exists

Every other arm in `benches-competitive` stops at a display list —
`docs/results-competitive.md` §2 says so explicitly, and it is the right
stopping point for comparing *frameworks*. It also means **none of them
measure a rasteriser**, so the software canvas has never been compared
with anything.

`vello_cpu` is the control that was missing: a CPU 2D rasteriser, in
Rust, SIMD and multithreaded, from the people who do this full time. It
is the one arm where lumen-static is not structurally advantaged — both
sides take the same scene to the same pixels on the same core.

## What is being compared, and what is not

The scene is the card grid from `raster-scaling`: a gradient header, then
cards with a shadow, a rounded fill, two text runs and a rule. At 4K that
is 408 cards, 2 041 commands.

- **Placement is shared.** Both sides get glyph indices and advances
  from the same fontdue face, so the columns differ in glyph
  rasterisation and compositing, not in shaping.
- **Shadows differ in model.** Ours is a quadratic falloff from a signed
  distance; Vello's is a true Gaussian, cut at 2.5σ. Same silhouette,
  different pixels — this is the largest legitimate difference between
  the two frames and it sets the loosest of the three bounds below.
- **Recording is Vello's own stage.** A `RenderContext` is filled per
  frame the way our `DisplayList` is, and in `vello_cpu` recording also
  flattens paths and generates strips. It is printed separately
  (`v.rec`) and it is *included* in the vello total, because it is work
  the frame has to do. Our list is built once and reused, which is
  worth 0.2–0.4 ms of the gap at the larger sizes and is stated here
  rather than buried.
- **Neither side blits.** Both stop at their own pixel format; ours
  composites straight onto an opaque page, Vello premultiplied onto
  transparency.

## 0. Re-run 2026-09-22, after M25–M30

Every figure below reproduces. Single-threaded, no `simd`, both sides on
one core — the cleanest per-core rasteriser comparison this repository
has:

| canvas | lumen ms | vello ms | ratio |
|---|---:|---:|---:|
| tally window | 0.64 | 1.43 | **2.21×** |
| 720p | 2.24 | 5.10 | **2.27×** |
| 1080p | 4.89 | 12.96 | **2.65×** |
| 1440p | 8.66 | 23.73 | **2.74×** |
| 4K | 19.49 | 54.05 | **2.77×** |

And at full tilt (ours `simd,threads`; vello ×16): 0.62/0.32, 1.18/1.02,
1.34/2.57, 1.50/4.57, 2.46/8.47 ms — **vello wins below 1080p and loses
above it**, which is §3's worker-heuristic finding reproducing exactly.
Nothing here has moved in thirteen days, which is worth one line because
everything else in `docs/results-competitive.md` was re-measured in the
same session and one arm had silently changed underneath it.

## 1. Single-threaded

| canvas | Mpx | lumen | lumen+`simd` | vello | ratio |
|---|---:|---:|---:|---:|---:|
| tally window | 0.23 | 0.64 ms | 0.62 | 1.43 | **2.3×** |
| 720p | 0.92 | 2.23 | 2.15 | 5.06 | **2.4×** |
| 1080p | 2.07 | 4.88 | 4.66 | 12.94 | **2.8×** |
| 1440p | 3.69 | 8.58 | 8.14 | 23.77 | **2.9×** |
| 4K | 8.29 | 19.70 | 18.68 | 54.14 | **2.9×** |

**2.3–2.9× on one thread, and the ratio grows with the canvas.** It is
not a SIMD result — `simd` is worth 3–5% here, consistent with
`docs/results-simd.md` — and it is not a code-quality result either.
It is the architecture: our shadow is one closed-form command over its
own bounding box, our rounded rect is two straight spans and four corner
arcs, and neither becomes a path. Vello flattens every one of them to
Béziers and runs the general sparse-strip pipeline over the result,
because that pipeline also has to draw the things we cannot draw at all.

## 2. Multithreaded — and where we lose

`vello_cpu` uses a rayon pool with a runtime thread count; ours spawns
band workers per render with a count derived from the pixel area
(`workers_for`). Vello's best is 16 threads; below is our default
against it.

| canvas | lumen (T17.5 default) | vello ×16 | |
|---|---:|---:|---|
| tally window | 0.62 ms | **0.39** | vello 1.6× |
| 720p | 1.17 | **0.95** | vello 1.2× |
| 1080p | **1.35** | 2.56 | 1.9× |
| 1440p | **1.48** | 3.65 | 2.5× |
| 4K | **3.08** | 10.65 | 3.5× |

**Vello wins below about 1 Mpx.** That is a real result and it is worth
stating plainly, because it is the first arm in this repository that
lumen-static loses.

## 3. It is our worker heuristic, not our rasteriser

`workers_for` returns `cap.min(px / 250_000)`: one worker per quarter
megapixel, and none at all below the first. The intent was to keep
threading off frames too small to pay for it. Measured against Vello,
the throttle is costing more than it saves.

Two probes, each a one-line change, reverted after measuring:

| canvas | default | throttle removed | + floor at 20k px | vello ×16 |
|---|---:|---:|---:|---:|
| tally window | 0.62 ms | 0.62 | **0.32** | 0.39 |
| 720p | 1.17 | **0.48** | 0.44 | 0.95 |
| 1080p | 1.35 | **0.76** | 0.80 | 2.56 |
| 1440p | 1.48 | 1.28 | 1.22 | 3.65 |
| 4K | 3.08 | 2.48 | 2.86 | 10.65 |

**The throttle costs 2.4× at 720p and 1.8× at 1080p**, and the floor
costs 1.9× at the tally window. With both lifted, every size is ahead of
Vello or level with it, and 720p goes from a 1.2× loss to a 2.1× win.

The heuristic counts pixels because pixels are what a band splits. What
it cannot see is that a 720p frame with 400 cards has plenty of work for
16 workers and is being given three. **Not changed here.** T17.5 tuned
the strip height against a measured re-walk cost
(`docs/results-threading.md`), the two interact, and re-tuning them wants
its own pass over that sweep rather than a change justified by one
scene.

## 4. What Vello does that we do not

The gap in §1 is bought, and this is the price. Everything in this list
is something `vello_cpu` renders and our software canvas cannot.

| | vello_cpu 0.2 | lumen-static software canvas |
|---|---|---|
| geometry | arbitrary Bézier paths, fill + stroke, nonzero and even-odd | 9 fixed commands; no path type exists |
| strokes | joins, caps, miter limit, dashes | one round-capped line width |
| transforms | arbitrary affine, per-object | axis-aligned rects; `Scaled` divides by a scalar |
| clipping | arbitrary path, nested | axis-aligned rects |
| layers | opacity groups, masks, luminance masks | none |
| blending | the full W3C set (16 mix modes × Porter-Duff compose) | source-over |
| filters | CSS filter functions and a primitive graph (blur, drop-shadow, colour matrix, …) | none |
| gradients | linear, radial, sweep; pad/repeat/reflect; interpolation colour spaces | linear, ≤4 stops, no extend |
| images | arbitrary transform, nearest/bilinear/bicubic, extend modes | axis-aligned blit with a tint |
| text | COLR and bitmap glyphs, variable-font coords, hinting, synthetic bold, decorations with skip-ink, cross-frame glyph cache | monochrome outlines, two static faces, **no glyph cache** |
| colour | managed throughout, and an `f32` pipeline for accuracy | 8-bit straight sRGB |
| SIMD | runtime dispatch across SSE4.2 / AVX2 / NEON / WASM | compile-time `wide`, one level |

Two entries in that table are ours to be uncomfortable about rather than
proud of:

- **No glyph cache.** `FontStack::glyphs` shapes *and rasterises* every
  run every frame; `text.rs` says so and `docs/results-competitive.md`
  §6.2 already found the measuring half of it. Vello keeps rasterised
  glyphs in `Resources` across frames. On this scene the two text runs
  are 13% of Vello's frame *with* its cache; ours pays the raster again
  every frame. The GPU backend has an atlas and does not
  (`docs/results-gpu.md`), so the software canvas is the odd one out.
- **One SIMD level, chosen at compile time.** Every number in
  `docs/results-simd.md` is an AVX2 number, and a binary built with
  `simd` on a machine without AVX2 falls back to whatever `wide` picks.
  Vello dispatches at runtime.

## 5. What we do that Vello does not

Symmetrically, and none of it shows up in a frame time:

- **`no_std` at two tiers, and no allocator required.** `vello_cpu` has
  a `libm` path and can build without `std`, but a `RenderContext` is
  built out of `Vec`s. Our core links 22 458 bytes of `.text` for
  `thumbv7em-none-eabihf` (`docs/results-embedded.md`).
- **Damage.** `render_within` repaints a rect and leaves the rest of the
  frame standing — 13 µs against 303 for a full redraw on the tally
  (`docs/results-simd.md`). `vello_cpu` renders a whole pixmap; there is
  no partial-render entry point.
- **Zero `unsafe`.** Not one `unsafe` block in `crates/*/src`, enforced
  every commit by `scripts/check_no_unsafe.sh`. `vello_cpu` and
  `vello_common` are clean too — but their SIMD comes from
  `fearless_simd`, which contains **3 408** of them. That is the normal
  and reasonable way to write portable SIMD; it is the thing this
  project decided not to do (`crates/lumen-static-shell/Cargo.toml`,
  the `simd` feature).
- **12 dependencies against 48.** `lumen-static-shell` with `truetype`
  pulls 12 external crates, three of which are proc-macro build-time
  only. `vello_cpu` pulls 48.

## 6. The honest summary

**On the scene a user interface actually paints, our rasteriser is
2.3–2.9× faster than Vello's on one thread, and 1.9–3.5× faster at
1080p and above with both threaded — and it is slower below 1 Mpx
because of a worker heuristic, not because of the rasteriser.**

That comparison is fair for the scene and unfair as a general claim, in
both directions. A UI made of rectangles and text is the case our
command set was cut for, and Vello is carrying a Bézier pipeline through
it. Hand both renderers an SVG, a chart with a rotated axis, a
drop-shadow filter or a clipped arbitrary path, and there is no
comparison to make: ours does not run.

## 7. How the two frames are known to be the same frame

A cross-renderer benchmark's characteristic failure is one arm quietly
drawing less and being timed for it. Three bounds, checked at every
size, every run; each was found necessary by injection, not by reading.

| bound | measured | catches |
|---|---:|---|
| ink differs ≤ 4.0 points | 1.8 | an arm that drew nothing, or lost a large class |
| mean luminance differs ≤ 5.0 | 2.3 | a dropped class big enough to move the frame |
| worst 8×8 block differs ≤ 65 of 255 | 46 | anything, anywhere |

Per `AGENT.md`, all three were injected and the numbers are the evidence:

| injection | result |
|---|---|
| drop the shadow from the vello scene | `ink differs by 11.3 points (lumen 89.5%, vello 78.1%)` |
| drop both text runs from the vello scene | `worst 8x8 block differs by 88 of 255` |
| drop the title from the *lumen* scene | `worst 8x8 block differs by 90 of 255` |

**The second injection is why there are three bounds and not one.** The
first version of this gate checked ink only. Dropping every glyph from
the vello arm moved the ink fraction by **exactly zero** — the glyphs sit
on a card that is already inked — while making that arm **13% faster**
(53.95 → 46.80 ms at 4K). A whole-frame luminance mean did not catch it
either: it moved by 1.3, *less* than the two shadow models already
differ by, because the glyphs are 3% of the pixels. Only the local
measure sees it.

The block bound sits between two measured numbers rather than at a
multiple of one: 46 is where the shadow models genuinely disagree in the
darkest corner of a card, 88 is where the glyphs go missing. Tighter
than 46 gates on the shadow model; looser than 88 does not see the
glyphs. Both are deterministic.

And the good direction was checked in the same session: with nothing
injected, all three bounds pass at all five sizes.
