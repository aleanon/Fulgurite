# Results: M29 — what one changed value costs, on a tree big enough to tell

*M29, `docs/plan-damage-precision.md`. Owns every figure that plan cites.
Machine: i9-13900KF, governor `powersave`.*

## What is already measured, and why it is not enough

`docs/results-frame-and-types.md` finding 14 measured the blast radius
inside `Boundaries`, so the strategy is held constant and only the
marking varies:

| row | µs |
|---|---|
| `Everything` (the oracle) | 6.93 / 7.84 / 6.96 / 6.88 |
| `Boundaries`, `mark_all` — what `App::update` forces | 6.95 / 7.00 / 6.93 / 6.88 |
| `Boundaries`, one subtree marked | 0.71 / 0.72 / 0.71 / 0.71 |

**6.22 µs, 9.7×** — and every one of those numbers is **layout only**,
on **901 synthetic nodes**. That is the figure this milestone exists to
re-measure honestly, because:

- **paint walks the whole table either way.** T16.8 refused a paint skip,
  and `Painter::clip_bounds` has one caller in the workspace
  (`List::paint`), so a derived container paints offscreen children;
- the trees this repository can currently measure are the tally (~20
  nodes) and a 10,000-row arm that is **virtualized**, so its rows are
  not on the table and `#[bind]` never touches them;
- shaping dominates a realistic frame (`docs/results-competitive.md`), so
  a layout-only saving can be invisible end to end.

## The thresholds — written before the fixture exists

| quantity | how | threshold | kill if |
|---|---|---|---|
| **the probe** (T29.1) | the grid fixture: one cell edited, `mark_all` against a hand-narrowed mark, **layout, paint and raster reported separately** | the *frame* moves by more than the arm's fast-cluster spread | it does not — the finding closes as promoted-but-not-worth-building |
| frame correctness | the invalidation oracle on the grid fixture | byte-identical frames under `Everything` and `Boundaries` | any divergence |
| rustc time | the figure `just const-budget` prints, four builds, with the grid fixture in the tree | no regression beyond that spread | the per-field items of T29.2 exceed it |
| allocation | `crates/lumen-static-shell/tests/alloc_gate.rs` | zero on a frame that edits a cell | anything allocates |
| embedded `.text` | `scripts/check_text.sh` | unchanged — a leaf profile has no bindings to narrow | it moves |

**Why rustc time is a gate and not a note.** T29.2 emits a marker and an
impl per field per container. `docs/results-const-eval.md` records an
O(n²) const assertion that made a 3,711-node tree **fail to compile at
all**, which is the same failure shape one size up.

## T29.0 — the fixture's own shape

*Filled when the fixture lands: table node count, arena row count, the
bound label's index, and the injection that proved the arm can fail.*

| quantity | measured |
|---|---|
| | |

## T29.1 — the probe

*Filled when it runs. Layout, paint and raster separately, `mark_all`
against hand-narrowed, eight runs, fast cluster and slow mode reported
apart. The verdict against the kill rule goes here, whichever way it
falls — a closed finding is a result.*

| stage | `mark_all` | narrowed | delta |
|---|---:|---:|---:|
| | | | |
