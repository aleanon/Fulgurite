# Results: the embedded profile

## The thresholds — written 2026-09-05, **before** the measurement

M13/T13.9's gate is that these are recorded first, so the gate can say
no. A profile with no measured footprint is a claim; a footprint
measured against a threshold invented afterwards is worse, because it
can only pass.

The device is T13.0's: Cortex-M4F, `thumbv7em-none-eabihf`, **128 KB
RAM, 512 KB flash**, a 320×240 SPI panel with its own GRAM and no
framebuffer.

| | Threshold | Why that number |
|---|---:|---|
| Framework `.text` | **≤ 64 KB** | An eighth of flash. The app, its face and its data need the rest; a GUI framework that takes more than a fifth of the part is not an embedded GUI framework. |
| Framework `.bss` + `.data` | **≤ 2 KB** | The framework's *own* statics. Per-tree arrays are the app's and are budgeted below; anything large here is a hidden global. |
| RAM for a 100-node tree | **≤ 24 KB** | T13.0's budget said ~19 KB for state + style + widget structs. 24 KB leaves headroom and still fits with the 8 KB stack in well under half of RAM. |
| Stack, actual depth | **≤ 4 KB** | T13.0 measured 256–272 B per level and a real screen at 4–5 deep (~1.6 KB). 4 KB is generous for that and half the 8 KB budgeted. |
| `f64` in the frame path | **zero** | The FPU is single-precision, so any `f64` becomes soft-float calls. |
| The fixture builds and runs | **yes** | The binary answer to "zero allocation, no allocator required". |

Recorded threats to these numbers, so a pass cannot be read as more than
it is: `cargo check` proves none of them — it type-checks and stops, with
no codegen, no link, no `.text`. Section sizes below come from a real
`cargo build --release` for the target.

## What is already known, 2026-09-04

Not results, just the starting position, measured while planning M10:

- The core is **9 `std::` references** from building without `std`.
- **`NodeState` is 76 B, `Style` 72 B, `NodeDesc` 48 B** — measured
  here with `size_of` on x86-64, default features, 2026-09-04. A
  200-node tree is therefore **29.6 KB** of state and style, before any
  display list or the widget structs themselves.

  Corrected the same day: this file first said 72/64/48 and cited
  `docs/results-frame-cost.md`, **which contains neither figure**. The
  numbers pre-dated C6, which added `pressed`, `hovered` and `focused`
  to `NodeState`. `scripts/check_doc_paths.sh` passed throughout,
  because it verifies a cited file *exists* and not that it says the
  thing — which is the gap in AGENT.md's numbers-stated-once rule that
  this correction exists to record.

- **`NodeDesc` is 48 B only on a 64-bit host, and only without
  `observe`.** It holds a `&'static str` and a `&'static [&'static str]`,
  so it is ~32 B on a 32-bit target; `observe` grows it to 72 B
  (`crates/lumen-static-agent/Cargo.toml`). The profile this document
  is about is the 32-bit, no-`observe` one, so **sizes must be
  re-measured on the target** rather than carried over from here.

- **The widget structs are a third resident array** and no earlier
  version of this file counted them. They are not free: a `Label` is
  larger than a `Fixed`, and the per-node budget is state + style +
  struct, not state + style.
- The alloc-free path **half** exists. `pass::layout`/`paint`/`hit_test`
  take slices, and the tests own `[NodeState; N]` arrays directly. But
  there are **zero** `[Style; N]` array literals in the workspace: all
  84 sites call `pass::default_styles`, which returns a `Vec`. There is
  no non-allocating way to construct a style array today, and adding
  one is part of the milestone rather than a starting position.

Until the first three bullets have numbers, the claim "zero allocation,
the microcontroller profile" remains **unproven** and should be written
as an intention wherever it appears.

One figure that is not a measurement of this framework but sets every
budget here: a 320×240 16 bpp **framebuffer is 150 KB**, and a full
frame over a 40 MHz SPI bus takes **31 ms**. The first is why the
profile keeps no framebuffer; the second is why damage matters more on
this device than on a desktop (T13.8).
