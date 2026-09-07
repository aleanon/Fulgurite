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
more room than an empty atlas has, the frame is *built again* rather than
finishing with glyphs missing. Rebuilding costs one extra shaping pass in
a frame that was already exceptional.

## 5. What has not been done

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
