# Does the software rasteriser scale to a real display?

**Ran 2026-09-07.** Reference machine: i9-13900KF, governor
`powersave`. `cargo run --release -p raster-scaling`, min of 30 after 5
warm-ups.

## Why this was measured

Every render number in this project was taken at 400×800 or 440×520 —
`docs/results-competitive.md`, `docs/results-frame-cost.md`, the live
gate. **Resolution was the one variable never swept**, and it is the
variable a CPU rasteriser is most sensitive to: its cost is per *pixel*,
not per node.

The question it settles is whether a GPU backend is a future
optimisation or a present requirement.

## The numbers

One scene — cards with shadows, two text runs and a rule each, plus a
gradient header — scaled with the canvas so the *content* is comparable
rather than the empty space.

| canvas | Mpx | full redraw | per Mpx | commands |
|---|---:|---:|---:|---:|
| tally window (440×520) | 0.23 | 0.72 ms | 3.14 | 51 |
| 720p | 0.92 | 2.53 ms | 2.74 | 201 |
| 1080p | 2.07 | 5.59 ms | 2.69 | 481 |
| 1440p | 3.69 | **9.75 ms** | 2.64 | 881 |
| 4K | 8.29 | **22.88 ms** | 2.76 | 2041 |

> **Superseded for the CPU column, 2026-09-09 (M17)** —
> `docs/results-simd.md`. The rasteriser got 1.2–1.3× faster: a 4K full
> redraw is **18.17 ms** scalar and **17.65 ms** with the `simd`
> feature, against the 22.88 below, and per-megapixel is ~2.2 rather
> than ~2.7. **The verdict does not change** — 4K is still over the
> 16 ms budget, damage tracking is still load-bearing, and the GPU
> backend the numbers below justified is still justified. The rows are
> kept because they are what T15.12's reversal was argued from.

**~2.7 ms per megapixel, flat across a 36× range.** The rasteriser is
pixel-bound and scales linearly, which is what a software rasteriser
should do and exactly why resolution is the variable that matters.

## The verdict

Against the live gate's 16 ms ceiling, for a **full** redraw:

- 1080p — 5.6 ms, 35% of a frame. Comfortable.
- 1440p — 9.8 ms, **61% of a frame**. Tight, with layout, paint and
  present still to pay.
- 4K — 22.9 ms. **Over budget.** 60 fps is impossible; so is 43.

And with M10b's scale factor a 2× HiDPI 1080p *logical* window is 4.1
Mpx physical — 11 ms, which is the 1440p row under another name.

**Damage tracking is therefore load-bearing, not an optimisation.**
What keeps the desktop inside its budget today is that a typical frame
dirties a rect rather than a screen. That holds for a form or a tally.
It does not hold for an application where a lot changes at once — a
chart animating, a canvas being drawn on, a list scrolling — which is
the case a GPU backend exists for.

This reverses `docs/plan-capabilities.md` T15.12's deferral, which was
argued from the 0.23 Mpx row alone.

## What it does not say

- **Nothing about the embedded profile.** That device has no GPU, and
  its budget is the SPI bus at 31 ms a frame
  (`docs/results-embedded.md`), not the rasteriser.
- **Nothing against the software canvas as the default.** It is 24.6 MB
  resident against 68–321 MB for the GPU-backed arms in
  `docs/results-competitive.md` §3, and most of that gap is a GPU
  context. A form at 1080p should keep paying 5.6 ms and saving 44 MB.
- **Nothing about text specifically.** The scene is mixed on purpose;
  a text-only or shadow-only sweep would attribute the cost, and has
  not been run.
