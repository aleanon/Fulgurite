# Results: M29 — what one changed value costs, on a tree big enough to tell

*M29, `docs/plan-damage-precision.md`. Owns every figure that plan cites.
Machine: i9-13900KF, governor `powersave`. Re-measured 2026-09-21 after a
seven-lens review found the first fixture measuring something other than
what it said — §"What the review corrected" records what moved and why.*

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
| allocation | `crates/lumen-static-shell/tests/alloc_gate.rs` — **which has no such arm today**; T29.3 adds one | zero on a frame that edits a cell | anything allocates |
| embedded `.text` | `scripts/check_text.sh` | unchanged — a leaf profile has no bindings to narrow | it moves |

**Why rustc time is a gate and not a note.** T29.2 emits a marker and an
impl per field per container. `docs/results-const-eval.md` records an
O(n²) const assertion that made a 3,711-node tree **fail to compile at
all**, which is the same failure shape one size up.

## T29.0 — the fixture's own shape

`benches --bin grid` (`just grid`), measured 2026-09-21.

| quantity | measured |
|---|---|
| table nodes | **2,227** |
| of those, with a non-degenerate box | **407** |
| of those, off the 1400×900 surface | **220** (9.9%) |
| virtualized rows materialized, of 5,000 declared | **18**, viewport 296 px |
| the bound label | `#formula`, table index 2, inside a toolbar container |
| cell text | varied across five shapes, measured through a **real face** (`FontMetrics`, fontdue) |
| damage from one edit | **770–990 px** of a 1400×900 (1.26 M px) surface |

**Damage is the value's digit count, not run-to-run variation.** A wider
`#formula` shifts `#status` beside it, so a frame that adds a digit
damages the toolbar row rather than the label alone. The first edit after
a fresh host damages the whole surface; the per-run median excludes it.

### Four ways the fixture could not say what it said

The first three were found by injection during T29.0 and are recorded
because each is one of `AGENT.md`'s two named vacuous shapes. **The last
four were found by a seven-lens review after T29.0 had shipped**, which
is the more useful fact: the fixture was green, its gates had been shown
to fail, and it was still measuring something other than what it claimed.

| what was wrong | how it was found | the fix |
|---|---|---|
| the identity gate compared `"= 0"` with `"= 7"` — **the same character count**, so a stale box is the same size and the two arms agree however wrong the mark is | the injection that marks a different subtree **passed** | compare at a value whose formatted width differs (`1_234_567`) |
| the bound label hung **directly off the root**, and a container lays out *all* its children — so any mark anywhere relaid it out and no stale box could exist | the same injection passed again, after the first fix | the label moved into a toolbar container, which is also what a spreadsheet looks like |
| the raster column called `Host::render`, which draws the whole list — no shell does that | reading the column against what a shell does | `take_damage_for_age` + `clear_within`/`render_within` |
| **the body was virtualized by declaration only**: nothing bounded the sheet, so a column child got `remaining_main.max(0.0)` = 0, the `Repeat` was laid out at 1400×**0** and materialized **one** row of its five thousand | review; `window_len()` had never been asked | a stylesheet bounds both panes, and `window_len() > 1` and `viewport() > 0` are now asserted |
| **nothing shaped**: `Host::pump` measures with `BlockMetrics`, a fixed advance per byte, so "varied text" was varied strings through a stand-in that cannot see them | review, against the same trap `benches --bin bindings` names for the memo arms | the probe drives `FontMetrics` over a real face, and says so loudly if none is found |
| **the "layout" column was layout + place** — what `Host::timings` has always returned | review | `Host::timings3` reports the three passes apart; `place` takes no `Changed` and is a floor |
| **both arms wrote the bound value directly**, which is the one way a real application never changes it | review | `arm_c` reaches the same edit through a click and a handler |

### The gates, each shown to fail

| gate | injection | result |
|---|---|---|
| the bound label is on the table | point the binding at a row inside the virtualized body | fires: *"#formula must be a TABLE node — in an arena it would never go through `#[bind]`"* |
| the two arms draw the same frame | mark a sheet cell instead of the formula bar | fires: hashes `0x4f88…cd01` against `0xec8f…cd01`, *"the two arms do not draw the same frame"* |
| the body actually virtualizes | remove the stylesheet's `#sheet` height — the state the fixture shipped in | fires: *"the body materialized 1 row(s) — it is not virtualizing anything"* |
| the click reaches the handler | point `#bump`'s centre outside its own box | fires: *"the click did not reach on_bump — arm C measures nothing"* |
| — good input | the correct mark, the shipped sheet, the real centre | identical hashes, 18 rows, `cell 0 -> 1` |

## T29.1 — the probe: **the finding survives, and is worth a third less than published**

Eight runs per arm, forty edits each, medians per run. **All 24 runs
landed in the fast cluster** — the machine was quiet, unlike the first
session.

| arm | layout | place | paint | raster | **frame** |
|---|---:|---:|---:|---:|---:|
| `mark_all` — what `#[bind]` forces | 23.15–23.60 µs | 2.48–2.50 | 30.04–30.18 | 1.43–1.62 | **57.12–57.76 µs** |
| `narrowed` — the hand-marked scaffold | **2.52–2.77 µs** | 2.47–2.52 | 30.07–30.18 | 1.40–1.51 | **36.52–36.84 µs** |
| `handled` — the frame a real click makes | 23.09–23.54 µs | 2.48–2.51 | 30.09–30.23 | 1.39–1.56 | **57.11–57.66 µs** |

*Measured after T29.5 corrected the fixture's row bind. The table two
versions of this document ago had a `narrowed` layout of 13.4 µs, 10.8
of which was the fixture's own `format!` allocations — see T29.5.*

**Against the kill rule:** the frame had to move by more than the arm's
fast-cluster spread — 0.64 µs for `mark_all`, 0.32 µs for narrowed. It
moved by **20.7 µs**, thirty times the larger spread. **The finding
survives.**

**What one `#[bind]` costs on a host-driven frame, stated once: 20.7 µs,
36% of the frame.** Layout alone goes 23.3 → 2.6 µs, **8.9×** — which
lands close to the 9.7× M26 measured on 901 synthetic nodes, once the
quantity is the same one (layout, without `place`, without a repeater's
per-frame bind sitting in the floor).

### What the `handled` arm settles, and it is the milestone's central fact

`handled` is **66.1–66.6 µs — the `mark_all` row**, not the narrowed one.

`Host::pump_with` calls `mark_all()` for a consumed event at
`crates/lumen-static-shell/src/lib.rs:694`, and asks `App::update`
anything at `:776` — eighty lines later, same pump. `Boundaries::mark`
returns early once `all` is set. So **on the frame a click produces, the
narrowed mark T29.3 would compute is discarded before it is made**, and
the 20.5 µs is worth exactly nothing.

Every non-test `#[bind]` in this workspace is written by a click handler
(`examples/counter`, `minimal`, `counter-window`, `fixtures/hot-app`,
`examples/glass`). **So the measured 31% is the saving on the frames
nobody currently produces**, and the milestone's real scope is the ones
that remain: `Host::tick` (which marks nothing), host-driven mutation,
and M28's task completions — except that those go through
`applied_changed`, which marks everything too (`:767`).

This is not an argument against the work. It is the quantity that has to
be stated beside it, and the first version of this probe could not state
it because both of its arms wrote the value from outside.

### `place` is a floor, and is 7% not 92%

`place` is **2.49 µs, identical in all three arms** — it takes no
`Changed` and walks every node, so no mark narrows it. It is 6.8% of the
narrowed frame — and now the *largest* part of the narrowed layout+place
cost, at 2.49 µs against layout's 2.6.

It is called out because the first version of this probe folded it into
the layout column, which is what `Host::timings` returns: the published
"2.4 µs narrowed layout" was almost entirely `place`, and read as
residue a better mark would remove. `Host::timings3` now separates them.

## T29.5 — the 12.2 µs was real, and it was not where this document said

**The claim published 2026-09-21 was: "`Repeat::layout` takes no
`Changed`… 12.2 µs, 91% of the narrowed arm's remaining layout, is the
repeater redoing work nothing asked it to redo."** The measurement was
right and the attribution was wrong. Splitting it four ways:

| the fixture's row bind | narrowed layout | `mark_all` layout |
|---|---:|---:|
| `set(format!(…))` — what shipped | 13.4 µs | 34.0 µs |
| `format_into(…)` — no allocation | **2.6 µs** | **23.3 µs** |
| `noop_bind` — window still bound and laid out | 1.1 µs | — |
| no body at all (`BODY_H = 0`) | 1.23 µs | 21.7–24.3 µs |

**10.8 µs of the 12.2 was the fixture's own `format!`, one heap
allocation per row per frame.** The framework's part — re-binding and
re-laying an 18-row window on a frame where nothing changed — is
**~1.5 µs** (2.6 against `noop_bind`'s 1.1), and the per-row `layout`
call is very nearly free, because the measure memos already hold.

The three-way split is the whole finding: `noop_bind` keeps the window
materialized and still calls `layout` on all 72 of its nodes, and costs
almost exactly what having no body at all costs. **A repeater re-laying
out a clean window is not expensive. A repeater re-running a bind that
allocates is.**

### Why nothing caught it

`crates/lumen-static-shell/tests/alloc_gate.rs` asserts zero allocations
per steady-state frame and has two repeater arms — **both binding with
`noop_bind`**. So the zero it proved for a repeater was a zero for a
list whose rows are never filled in, which is `AGENT.md`'s first vacuous
shape: the only thing that allocates on a repeater frame is a bind that
*writes*, and no arm had one.

`a_repeater_whose_bind_writes_text_allocates_nothing` is the arm that
now covers it. Its injection is the ordinary spelling:

| the bind | result |
|---|---|
| `format_into(&mut row.name, format_args!("row {i}"))` | passes — 0.00 allocs/frame |
| `row.name.set(format!("row {i}"))` | **fires: 19.00 allocs/frame, 19.00 frees/frame over a 19-row window** |

One allocation per row per frame, with the quantity printed beside the
bound. `docs/results-allocation.md` owns that table.

### What was decided against, with its number

**Skipping the bind loop on a clean frame is worth ~1.5 µs and is not
built.** `Repeat` binds every row every frame *by contract* — it is what
makes a data change visible without an invalidation call, and it is
documented on `Repeat::bind`. Skipping it when the repeater's subtree is
clean would make a list silently stale for any application that mutates
its data without marking, which under the `Everything` default would
work and under `Boundaries` would not — the worst kind of difference.
1.5 µs does not buy that.

**The fix is the one that needed no contract change:** a bind that does
not allocate, a gate that refuses one that does, and the shape
documented where the author is already looking.

### Paint is the frame, but culling is not the answer the share suggests

Paint is 30.1 µs — **82% of the narrowed frame** — and unchanged across
all three arms, exactly as T16.8's refusal predicts.

**The reopen condition in `docs/plan-damage-precision.md` §4 is *not*
met, and the first version of this document said it was.** That
condition is about **off-surface nodes**, and this fixture has **220 of
2,227 — 9.9%**. A `clip_bounds` cull skips what is off the surface; it
keeps every one of the 2,007 on-screen nodes, including the ~1,800 whose
boxes are degenerate. Paint's share of the frame is not evidence about
paint's *cullable* share, and reading one as the other is the
"bound computed from the wrong quantity" shape in a deferral.

### Raster is not the problem

One edit damages 770–990 px of a 1,260,000 px surface and costs 1.45 µs.
The stage the original question guessed at is already surgical; the
stages that are not are the two walks above it.

## What the review corrected

A seven-lens review with an adversarial refuter per lens (2026-09-21)
judged 56 findings and confirmed 49. What it changed in *this* document:

| the first version said | what it is |
|---|---|
| layout 22.95 → 2.42 µs, **9.6×**, "the 9.7× holds up" | that column was layout **+ place**. With the passes apart, a body that virtualizes, text that shapes and a bind that does not allocate: **23.3 → 2.6 µs, 8.9×** |
| frame −20.6 µs, **39%** | **−20.7 µs, 36%** — the delta survived almost exactly, across two rebuilds of the fixture |
| "5,000 virtualized rows" in a column headed *measured* | a constructor argument. **18** were materialized, and before the stylesheet, **one** |
| "varied across five shapes, so shaping is present" | no shaper ran. It does now |
| "paint is 88% of the narrowed frame… the reopen condition is met" | paint is **62%**, and the condition is about off-surface nodes, of which there are **220** |
| the probe measured "what one `#[bind]` costs" | it measured what one costs **on a frame with no handled event**, which no application produces. `arm_c` now measures the other one |

**None of it moved the verdict.** The kill rule asked for a frame delta
above the arm's spread and got forty times it, on a fixture that is now
bigger, slower and more honest than the one that first passed.
