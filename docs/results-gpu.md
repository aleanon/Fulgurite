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

Where exactly the two curves cross is not yet known: it needs a sweep
over damaged fraction × command count rather than the four points above.
That is `docs/plan-invalidation.md` T16.13, along with the per-command
cost and the question of whether the backend should choose for itself.

## 6. The fallback, end to end

The GPU is chosen at *runtime*, not at compile time, because an adapter
can be absent on a machine the binary was built for: no driver, no
`/dev/dri`, a remote session. `pick_backend` tries the GPU when the
feature is on and `Prefer` allows it, prints why it fell back if it
cannot, and returns the software canvas. **A window always opens.**

Which one you got is printed at startup and available from
`Backend::name`, because "it is slow" should not require guessing.

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
- **The crossover is bounded but not located.** §5b shows the GPU losing
  on every damage-clipped frame and winning on large repaints; the four
  points do not say where the curves meet (T16.13).
- **Multi-window is software-only.** `Pane` still owns a `Surface` and a
  `Canvas` directly rather than a `Box<dyn Backend>`. Nothing prevents
  it; it was not needed, and a second window is a second GPU context —
  which, at 174 MB, is a decision rather than a detail.
- **Nothing is threaded.** Raster is still on the UI thread; `draw`
  returns once the work is queued, but nothing overlaps frame N's layout
  with frame N−1's raster.
