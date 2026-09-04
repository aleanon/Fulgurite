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
- **`NodeState` is 72 B, `Style` 64 B, `NodeDesc` 48 B**
  (`docs/results-frame-cost.md` measured the first two in another
  context). A 200-node embedded tree is therefore ~27 KB of state and
  style, before any display list.
- The alloc-free path already exists: `pass::layout`/`paint`/`hit_test`
  take slices, and 233 tests already pass caller-owned arrays to them.

Until the first three bullets have numbers, the claim "zero allocation,
the microcontroller profile" remains **unproven** and should be written
as an intention wherever it appears.
