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
| Framework `.bss` + `.data` | **zero symbols**, exemptions named | The framework's *own* statics. **Tightened from "≤ 2 KB" on 2026-09-07**: a bound nothing had approached told nobody anything, and what the number was standing in for is a *shape* — no allocator arena, no lazily-initialised registry, no static caches, nothing claiming RAM before `main`. `scripts/check_bss.sh` checks that, and `scripts/bss.allow` names what is allowed and why. |
| RAM for a 100-node tree | **≤ 24 KB** | T13.0's budget said ~19 KB for state + style + widget structs. 24 KB leaves headroom and still fits with the 8 KB stack in well under half of RAM. |
| Stack, actual depth | **≤ 4 KB** | T13.0 measured 256–272 B per level and a real screen at 4–5 deep (~1.6 KB). 4 KB is generous for that and half the 8 KB budgeted. |
| `f64` in the frame path | **zero** | The FPU is single-precision, so any `f64` becomes soft-float calls. |
| The fixture builds and runs | **yes** | The binary answer to "zero allocation, no allocator required". |

Recorded threats to these numbers, so a pass cannot be read as more than
it is: `cargo check` proves none of them — it type-checks and stops, with
no codegen, no link, no `.text`. Section sizes below come from a real
`cargo build --release` for the target.

## The measurement — 2026-09-05

`just firmware-size`: a **linked** `thumbv7em-none-eabihf` binary
(`fixtures/embedded/src/bin/firmware.rs`), release, one frame of the
8-node fixture with a baked face. Not a board bring-up — no memory map,
no vector table, no peripheral — but a real link, which is the only
thing that produces real sections.

| | Threshold | Measured | |
|---|---:|---:|---|
| `.text` | ≤ 64 KB | **20,524 B** (20.0 KB) | **pass**, at 31% of budget — *2026-09-05; now 22,458, see below* |
| `.bss` + `.data` | zero framework symbols | **1 symbol, 4 B, exempted** | **pass** — the only static is the fixture's own harness; still 1 symbol, 4 B |
| `.rodata` | — | 12,940 B | of which **9,010 B is the font face** (T13.7) — *now 13,016* |
| Flash, total | ≤ 512 KB | **33,677 B** (32.9 KB) | 6.4% of the part — *now 35,691* |
| RAM, 100 nodes | ≤ 24 KB | **14.5 KB** state + style | **pass**; widget structs are the app's, below |
| Stack, actual depth | ≤ 4 KB | **816 B at depth 3** | **pass**, and asserted at compile time since 2026-09-07 |
| `f64` in the frame path | zero | **zero** | the only `f64` is `observe`'s accesskit call, which needs `std` |
| Fixture builds and runs | yes | **yes** | links for the target; runs on the host with an aborting allocator |

Every threshold met. Three things worth more than the pass.

### One `f32::clamp` cost 26.5 KB — 44% of the firmware

The first link was **43,224 B of `.text`**, and a third of it was
`core::num::flt2dec` — the float *formatting* machinery, in a GUI that
formats no floats. The panic string in the binary named the culprit:
`f32::clamp` panics when `lo > hi` or a bound is NaN, and its message
formats a float.

`lumen_static::num::clamp` replaces it at all nine call sites — one of
them `Constraints::clamp`, which runs for every node of every frame:

| | before | after | |
|---|---:|---:|---|
| `.text` | 43,224 B | **20,524 B** | −52.5% |
| `.rodata` | 16,736 B | **12,940 B** | −22.7% |
| total | 60,173 B | **33,677 B** | **−26,496 B** |

The replacement matches `f32::clamp` everywhere `f32::clamp` returns,
NaN included; where `f32::clamp` panics it is total instead, which is
the better failure for a frame loop. **This is the finding that
justifies the task**: it is invisible on a desktop, where 26 KB is
nothing and nobody links for a 512 KB part, and no amount of
`cargo check` would have shown it.

### Re-measured 2026-09-10 — and the ceiling is a gate now

**The 2026-09-05 figure above drifted 9.4% and nothing noticed**, because
the `≤ 64 KB` threshold was stated in this document and enforced by
nothing. `scripts/check_bss.sh` has held `.bss` since 2026-09-07; there
was no equivalent for `.text`. There is now —
`scripts/check_text.sh`, in `just thumb` beside the `.bss` gate — and it
prints the measurement next to the ceiling rather than only "pass", so a
closing margin is visible in the output.

Found while measuring something else: whether growing `Style` by 4 bytes
would cost the embedded profile anything (`docs/plan-glass.md`'s T18.7
question). It does not — but the baseline it was measured against turned
out not to match this file.

| | `.text` | Δ | of the 64 KB ceiling |
|---|---:|---:|---:|
| 2026-09-05, T13.9 | 20,524 | — | 31.3% |
| end of M15 (`8109ad2`) | 21,970 | **+1,446** | 33.5% |
| end of M16 (`959cf2c`) | 22,458 | **+488** | 34.3% |
| end of M17 (`59a45ef`) | 22,458 | **0** | 34.3% |
| 2026-09-10 | **22,458** | | **34.3%** |

`.rodata` moved 12,940 → 13,016 (+76) and flash total 33,677 → 35,691
over the same span. `.bss` is unchanged at one exempted 4-byte symbol,
and the per-node RAM figures below are unchanged — `NodeState` is still
76 B and `Style` still 72 B, both still `const`-asserted.

**M17 cost the firmware exactly nothing**, which is the tiering claim
holding up under measurement rather than in argument: T17.0–T17.5 landed
entirely in `lumen-static-shell`, which an embedded profile does not
link. M14 and M15 together account for three quarters of the growth
(+1,446) and M16 for the rest (+488) — the core gained `Cmd::Line`,
`Cmd::Gradient`, `Cmd::Image`, overlays, popups, the focus ring and the
cursor in M15, and invalidation and paint-side culling in M16. Not
attributed below the milestone; the per-milestone split is what the gate
needs and what a reader can act on.

**What growing `Style` would cost, since it was measured here** (probe
applied and reverted, 2026-09-10):

| `Style` | `.text` | Δ flash | RAM/node | 100 nodes |
|---|---:|---:|---:|---:|
| 72 B (today) | 22,458 | — | 148 B | 14.5 KB |
| 76 B (+1 `f32`) | 22,494 | +36 | 152 B | 15.2 KB |
| 92 B (+20 B, a full glass parameter set) | 22,694 | +236 | 168 B | 16.4 KB |

Frame time at 10 000 nodes was unaffected in either direction — four
runs each, min-of-150, distributions fully overlapping (rebuild
5,202–5,442 µs baseline against 5,169–5,460 µs at 76 B). The cost of a
per-node field on this profile is RAM per node, not flash and not
frame time.

### Target sizes, asserted rather than remembered

Measured on `thumbv7em-none-eabihf` and held by `const` assertions in
`fixtures/embedded/src/lib.rs`, so the build fails if a field is added
rather than the budget quietly drifting — which is what happened to
these figures once already (see the 72/64 correction below).

- `NodeState` **76 B**, `Style` **72 B** → **148 B per node** of RAM.
  100 nodes = 14.5 KB.
- `NodeDesc` **32 B on the target**, against 48 on a 64-bit host, as
  this file predicted. It is `const` data in flash, not RAM.
- The fixture's `Ui` is **1,064 B** for 8 nodes — but **768 B of that is
  one `InlineClusters<64>`**, at 12 B per cluster. The
  `TextBuffer::Clusters` pairing takes one cluster per byte, which is
  the correct worst case and an expensive one: an embedded app with a
  text field should size `InlineText<N>` to the field, not to a round
  number.

### `.bss` is zero symbols now, not two kilobytes

The old threshold was `≤ 2 KB` and measured 0 — **a bound nothing had
approached, which tells a reader nothing**. What it was standing in for
is a shape rather than a size: this framework has no hidden global
state, no allocator arena, no lazily-initialised registry, no static
caches. The day that stops being true is the day the number matters,
and 2 KB of headroom would have hidden it.

So it is **zero framework symbols**, with `scripts/bss.allow` naming
what is allowed and arguing for it. `scripts/check_bss.sh` enforces it
against a linked binary in `just check`, and was verified by injecting
a framework static the frame path writes — it names the offender and
exits 1.

**Making the check honest exposed a defect in the fixture.** `SINK`, the
static `firmware.rs` parks the frame checksum in "so the whole frame
cannot be optimized away as dead", **had no symbol in the linked
binary at all**: as a private static never read, LLVM removed the store.
The frame survived on panic paths instead — true by accident, which is
the worst way for a published number to be true. It is `#[no_mangle]
pub` now, the guard does what its comment says, and it is the one
exemption on the list.

Confirmed separately that the measured `.text` is not dead code:
removing the `frame()` call collapses it from 21,946 B to **1,438 B**.

### The stack budget is enforced now, not just recorded

Until 2026-09-07 this row was a threshold nobody checked: the number was
written down, the depth was measured once by hand, and the two were
never connected. `desc::stack_bytes(depth)` connects them, and
`fixtures/embedded` asserts it **at compile time** against the tree that
actually has to fit — 816 B at depth 3, verified by re-injection
(lowering the bound to 512 fails the build).

Against a tree's **actual** `DEPTH`, not `MAX_DEPTH`. Those are
different numbers, and conflating them is the mistake the next section
describes.

### `MAX_DEPTH` stays 64, and the reason is a measurement

T13.0 said the embedded profile "lowers the cap 64 → 24". It does not,
because `MAX_DEPTH` is a **compile-time assertion with no runtime
cost** — the derive checks `DEPTH <= MAX_DEPTH` and nothing allocates
against it. Lowering it would only reject trees, and would reject them
on desktop too. What is true is the budget: at T13.0's 256–272 B per
level, 64 levels would be ~17 KB against an 8 KB stack, so an embedded
app should stay under ~30 deep. Every real tree here is 4–5.

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

~~Until the first three bullets have numbers, the claim "zero
allocation, the microcontroller profile" remains **unproven** and should
be written as an intention wherever it appears.~~ **Measured 2026-09-05,
above.** The claim is now a number and may be written as one.

One correction to the starting position, since it was wrong in a way
worth recording: it said "there are **zero** `[Style; N]` array literals
in the workspace: all 84 sites call `pass::default_styles`". True when
written; `pass::default_styles_into` (T13.3) is what changed it, and the
embedded fixture is the first caller to own its style array.

One figure that is not a measurement of this framework but sets every
budget here: a 320×240 16 bpp **framebuffer is 150 KB**, and a full
frame over a 40 MHz SPI bus takes **31 ms**. The first is why the
profile keeps no framebuffer; the second is why damage matters more on
this device than on a desktop (T13.8).
