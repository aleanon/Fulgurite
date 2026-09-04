# Results: the embedded profile

*Placeholder. **No number has been measured yet** — this file exists so
`docs/plan-architecture-v2.md` M10 cites something real, and so that
anyone who comes looking for the footprint finds a statement that it
does not exist rather than a dangling link.*

## What will go here (M10/T10.4)

- `.text` and `.bss` for the alloc-free fixture at
  `thumbv7em-none-eabihf`, against a stated threshold.
- Peak RAM its `[NodeState; N]` and `[Style; N]` arrays occupy at a
  stated node count, and the per-node figure that follows from it —
  cited, not restated, wherever else it appears (AGENT.md's
  numbers-stated-once rule).
- Whether the allocator-panics fixture links and runs, which is the
  binary answer to "zero allocation, no allocator required".

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
