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

`benches --bin grid`, measured 2026-09-21.

| quantity | measured |
|---|---|
| table nodes (the frozen grid) | **2,226** |
| virtualized rows, through `Repeat` | 5,000 |
| the bound label | `#formula`, table index 2, inside a toolbar container |
| cell text | varied across five shapes, so shaping is present |
| damage from one edit | **512–640 px** of a 1400×900 surface |

### Two ways the fixture could not fail, both found by injection

**Neither was visible by reading it.** Each is the shape `AGENT.md` names
first: the setup could not exhibit the effect.

| what was wrong | how it was found | the fix |
|---|---|---|
| the identity gate compared `"= 0"` with `"= 7"` — **the same character count**, so a stale box is the same size and the two arms agree however wrong the mark is | the injection that marks a different subtree **passed** | compare at a value whose formatted width differs (`1_234_567`) |
| the bound label hung **directly off the root**, and a container lays out *all* its children — so any mark anywhere relaid it out and no stale box could exist | the same injection passed again, after the first fix | the label moved into a toolbar container, which is also what a spreadsheet looks like |

### The gates, each shown to fail

| gate | injection | result |
|---|---|---|
| the bound label is on the table | point the binding at a row inside the virtualized body | fires: *"#formula must be a TABLE node — in an arena it would never go through `#[bind]`"* |
| the two arms draw the same frame | mark a sheet cell instead of the formula bar | fires: hashes `0x4f88…cd01` against `0xec8f…cd01`, *"the two arms do not draw the same frame"* |
| — good input | the correct mark | identical hashes, `0x4f888817f53bcd01` |

## T29.1 — the probe: **the finding survives**

Eight runs per arm, forty edits each, medians per run; the fast cluster
below, with the slow mode reported apart. The machine was building other
work during this run, which is why three `mark_all` runs and one
`narrowed` run landed in the slow mode — the fast clusters are stable
across two sessions (22.94–23.16 µs in the first, 22.95–23.12 in this
one), so the figure is the cluster, not the mean.

| stage | `mark_all` (what `#[bind]` forces) | narrowed by hand | delta |
|---|---:|---:|---:|
| layout | 22.95–23.12 µs | **2.39–2.42 µs** | **−20.55 µs, 9.6×** |
| paint | 27.72–28.36 µs | 27.68–27.83 µs | unchanged |
| raster (damage-bounded) | 1.51–1.63 µs | 1.49–1.54 µs | unchanged |
| **frame** | **52.20–53.12 µs** | **31.60–31.79 µs** | **−20.6 µs, 1.65×** |

*Runs in the fast cluster: `mark_all` 5 of 8 (slow mode 70.17, 71.50,
100.82); narrowed 7 of 8 (slow mode 51.48).*

**Against the kill rule:** the frame had to move by more than the arm's
fast-cluster spread — 0.92 µs for `mark_all`, 0.19 µs for narrowed. It
moved by **20.6 µs**, twenty-two times the larger spread. **The finding
survives, and T29.2–T29.4 are justified.**

**What one `#[bind]` costs today, stated once:** 39% of this frame.

### What the probe also settled

**Paint is now the frame.** After narrowing, paint is 27.7 µs of a
31.6 µs frame — **88%** — and it is unchanged between the arms, exactly
as T16.8's refusal predicts: a derived container paints its children
whether or not anything changed, and `Painter::clip_bounds` has one
caller in the workspace.

That meets the reopen condition `docs/plan-damage-precision.md` §4 wrote
for paint culling. M26 Phase D closed that finding as "true and
worthless" on the arms it had; this arm is 2.5× larger and says paint is
where the next 20 µs is.

**Raster is not the problem and the damage machinery works.** One edit
damages 512–640 px of a 1,260,000 px surface and costs 1.5 µs — so the
stage the original question guessed at is already surgical, and the
stages that are not are the two walks above it.
