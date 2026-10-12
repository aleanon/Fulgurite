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
| allocation | `crates/fulgurite-shell/tests/alloc_gate.rs` — **which has no such arm today**; T29.3 adds one | zero on a frame that edits a cell | anything allocates |
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

*The **frame** column below is the stage sum layout + place + paint +
raster, not the pump; **T53.1 at the end of this file restates it as the
whole pump plus raster**, where the same saving is 26% of the frame, not
36%.*

| arm | layout | place | paint | raster | **frame** (stage sum) |
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

`handled` **lands on the `mark_all` row** of the table above, not the
narrowed one. *(Corrected 2026-09-23: this line used to give 66.1–66.6
µs, a figure from an earlier version of the table that the correction
missed. A review found it. The table is the only statement of the
number now.)*

`Host::pump_with` calls `mark_all()` for a consumed event at
`crates/fulgurite-shell/src/lib.rs` (`if handled { … mark_all() }`
in `pump_with`, at `:694` when this was written), and asks `App::update`
anything eighty lines later, in the same pump. `Boundaries::mark`
returns early once `all` is set. So **on the frame a click produces, the
narrowed mark T29.3 would compute is discarded before it is made**, and
the saving stated above is worth exactly nothing.

Every non-test `#[bind]` in this workspace is written by a click handler
(`examples/counter`, `minimal`, `counter-window`, `fixtures/hot-app`,
`examples/glass`). **So the measured saving is on the frames nobody
currently produces**, and the milestone's real scope is the ones
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

`crates/fulgurite-shell/tests/alloc_gate.rs` asserts zero allocations
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

*Amended 2026-09-25, M41/T41.3:* the contract this paragraph defended
did not survive M34's flip. Under the `Boundaries` default a clean
subtree is never laid out, so the bind loop was already skipped on
every frame that did not reach the list, and the stale list this
paragraph warned of was the default's behaviour (R1-F4). The contract
is now the other one: a data change is reported, through
`Repeat::generation` or `Host::invalidate` (`Repeat::bind`'s rustdoc).
The bind still runs on every layout that reaches the list, so the
allocation gate above still applies, and since T41.3 that gate runs
under `Everything` — under the default it had stopped reaching the list
at all (T41.3 below).

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

## T29.3 — the real mechanism, 2026-09-24

Eight runs per arm, forty edits each, medians per run, `just grid`.
Slow-cluster runs (two of eight in some arms, frames above 80 µs) are
set aside by the mode rule `docs/results-frame-and-types.md` records.

| arm | frame, fast cluster | what it is |
|---|---:|---|
| `mark_all` | 55.62–55.86 µs | **the control**: a host-driven write, then `invalidate_all()` — what `#[bind]` did before T29.3 |
| `bound` | 34.78–35.18 µs | the same write, marked by the binding itself (`App::update_marked`) |
| `narrowed` | 34.89–35.23 µs | T29.1's hand-marked scaffold |
| `handled` | 56.01–56.32 µs | the same edit through a click — still `mark_all`, until T29.6 |

**The mechanism lands where the scaffold did**, which is what T29.3's
measure asked. And, as §2.5 predicted, **the click-driven frame did not
move**: a consumed event still marks everything before a binding is
asked anything. That is T29.6's job, not this one's.

**What the per-site cache costs a device** (M52/T52.3, F-NEW-1). Each
`#[bind]` of a non-generic application owns one `bind::BindSite` static,
so the walk above runs once per site. On `thumbv7em-none-eabihf` that is
4 B per binding. Until M52 it sat in `.data` — its "unknown" was
`usize::MAX`, a non-zero initialiser, so each site also cost 4 B of flash
and a copy before `main` — and no gate saw it, because the gated firmware
had no `#[bind]`. It is all-zero now (`.bss`, no flash), and
`scripts/check_bss.sh` exempts it by pattern and prints the count and
bytes; the figures are in `docs/results-embedded.md` (M52). What the
walk links — the whole `Node` vtable, through `&dyn Node` — is M52/T52.5.

**And what the walk cost in flash, measured then removed** (M52/T52.5,
F-NEW-2). `node_of` coerced every table node to `&dyn Node`, which
instantiated every method of every type in a bound tree. On the
bind-only firmware (`docs/results-embedded.md`) that was 10 `Node>::hit`
and 17 `on_event` symbols in a binary that hit-tests and dispatches
nothing. The walk is `Widget::bind_walk` now — the derive's field-by-field
walk on concrete types, a leaf's own address, and the vtable walk only
for a hand-written container's children — and the same binary links
none of them. Routing the derived walk back through `&dyn Node` costs
**18,826 B** of `.text` there today. The answers are unchanged:
`tests/bind_index.rs` asks both walks every byte of a tree with an enum
on its second variant, a `None`, a `Some` and a `Select`.

## T29.6 — a click on a split application, 2026-09-24

The same probe, with a fifth arm: the click on `#bump`, on the grid's
**split twin** (`mod split` in `benches --bin grid`). Eight runs a side;
this session ran noisier than T29.3's, so every arm is shown whole and
the slow cluster (frames above 80 µs, or the `narrowed` pair near 56 µs)
is set aside by the same mode rule.

*Stage sum again (layout + place + paint + raster); T53.1 below restates
it as the whole pump.*

| arm | frame (stage sum), all eight runs (µs) |
|---|---|
| `mark_all` (control) | 57.57 57.58 57.70 57.91 58.21 60.34 · 82.98 84.56 |
| `handled` (unsplit) | 54.92 55.30 57.12 57.48 57.55 57.61 57.69 57.87 |
| `narrowed` (scaffold) | 36.79 37.19 37.22 37.25 37.34 37.59 · 55.47 58.38 |
| `bound` | 35.61 35.64 37.22 37.23 37.31 37.39 37.41 37.42 |
| **`handled/split`** | **35.19 35.24 36.51 36.87 37.01 37.04 37.06 37.16** |

**The click frame of a split application lands on the narrowed row.**
The unsplit click stays on the control's — which is correct, not a
shortfall: its handler may write any widget. So the saving §2.5 found
unreachable "on every frame a click produces" is now reached on every
click of a split application, and — for a turnkey window — once M34
flips the default strategy.

## T41.3 — a `Repeat`'s rows get their own view of what changed, 2026-09-25

`docs/plan-boundaries-repair.md` §5 set T41.3's kill rule before the
build: `just grid`, eight runs a side; if the edit frame's median lies
outside the before-arm's spread, the generation bind marks more than the
`Repeat`. The binary prints eight frames per arm per run, so each arm is
64 samples. **LOADED**: load average 8.0 before and 11.7 after, with
other agents compiling; the layout column is the one T41.3 can move.

| arm | layout µs before (min · median · max) | after |
|---|---|---|
| `mark_all` | 22.88 · 23.63 · 36.12 | 21.93 · 23.99 · 25.37 |
| `bound` | 2.51 · 2.66 · 5.34 | 1.91 · 2.03 · 2.20 |
| `narrowed` | 2.47 · 2.64 · 4.98 | 1.85 · 2.01 · 2.14 |
| `handled` | 22.90 · 23.41 · 36.74 | 21.79 · 22.26 · 26.20 |
| `handled/split` | 2.53 · 2.61 · 5.50 | 1.89 · 2.00 · 2.16 |

**The rule fires, downward.** The edit frames' medians (2.03, 2.01,
2.00 µs) lie below the before-arms' minima (2.51, 2.47, 2.53). The rule
was written against a bind marking *more*; this is the opposite, and it
is attributed rather than assumed: with the rows handed the frame's
`Changed` again (the pre-T41.3 code, 32 samples a side) the edit frame
returns to 2.66 and 2.64 µs. Before T41.3 each body row, a derived
container indexed `0..T::NODE_COUNT` in its arena, asked the frame's
*main-table* ranges whether it was clean; the edit marks `#formula`,
low in the main table, so every row overlapped it and was laid out
again. Now a row whose slot holds the same row as last layout, on a
frame that did not mark the repeater, is told nothing changed and stops
at its own boundary. The frame hashes are unchanged
(`0x37b6f8508b1fe6cf` for `mark_all` and `narrowed`, before and after),
and the `mark_all` arms, which cannot skip, did not move beyond noise.

## T34.2 — what the flip buys a showcase window, 2026-09-24

`examples/showcase` test `flip::measure_the_flip` (ignored; `cargo test
--release -p showcase measure_the_flip -- --ignored --nocapture`): the
same program's host under each strategy, 40 frames of each class, the
median frame (layout + place + paint — **three of the pump's six stages**;
T53.1 below restates it as the whole pump), eight runs a side. The prediction,
written in the plan before the flip: click frames and binding writes do
not move, because showcase's `after_update` and its tick mark everything.

| class | entry | `Everything` (µs) | `Boundaries<256>` (µs) |
|---|---|---:|---:|
| idle | Button | 0.55–0.58 | **0.33–0.34** |
| hover | Button | 0.57–0.58 | **0.53–0.54** |
| unconsumed press | Button | 0.58–0.58 | **0.53–0.54** |
| tick | ProgressBar | 0.81–0.83 | 0.80–0.82 |
| click | Button | 1.17–1.18 | 1.16–1.20 |

**The prediction held.** The frames that can move did; tick and click did
not, because showcase marks everything on both *by design* (its tick
writes the stage, its `after_update` the header). Showcase's tree is
small, so the absolute saving is too — the grid (T29.3, T29.6) is where
the size of the win shows; this row is what a user of the flagship
example gets.

## T53.1 — the same tables, as shares of the whole pump, 2026-09-26

**Every "frame" above was a stage sum.** T29.1's and T29.6's are layout +
place + paint + raster; T34.2's is layout + place + paint. `pump_with`
also runs input and the bindings, the arena cascade and `finish`, and
none of those was in any of them (R6-F2, `docs/results-deep-review.md`;
`docs/plan-frame-cost.md` T53.1). Since T53.1 the host times itself
(`Host::stage_timings`), and every probe here prints the pump, its six
stages, and the gap between them, bounded at 5% of the pump.

**The grid** (`just grid`), eight runs a side, median of each run's 40
frames; ranges are over the eight runs. **LOADED**: load average 13.6 at
the start on a 32-core machine, other agents compiling; a second run at
load 11–14 read about 1.5× higher on every column and is not used. The
ratios are what this section states, and they held in both runs.

| arm | input | **cascade** | layout | place | paint | finish | raster | stage sum (old frame) | **pump** | **frame** = pump + raster | gap (bound 5%) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `mark_all` | 0.05 | **21.12–22.07** | 23.57–23.85 | 2.35 | 29.82–30.07 | 1.62 | 1.39–1.77 | 57.29–58.04 | 78.81–79.96 | **80.32–81.46** | 0.08–0.09 |
| `bound` | 0.05 | **21.18–22.78** | 2.61–2.91 | 2.34 | 29.67–30.09 | 1.62 | 1.38–1.50 | 36.11–36.77 | 58.18–60.41 | **59.68–61.84** | 0.08 |
| `narrowed` | 0.04 | **21.02–22.28** | 2.56–2.93 | 2.34 | 29.57–30.11 | 1.62 | 1.40–1.51 | 35.87–36.81 | 57.42–59.71 | **58.82–61.12** | 0.08–0.09 |
| `handled` | 6.45–6.83 | **20.12–21.64** | 22.29–23.64 | 2.35 | 28.31–29.97 | 1.60 | 1.31–1.47 | 54.16–57.33 | 81.11–85.95 | **82.42–87.42** | 0.08–0.09 |
| `handled/split` | 6.47 | **19.95–20.69** | 2.39–2.57 | 2.28 | 27.24–27.28 | 1.54 | 1.26–1.41 | 33.23–33.47 | 60.09–60.91 | **61.43–62.30** | 0.08–0.09 |

The restated shares, from the medians of the eight runs:

| claim | on the stage sum (as published) | on the whole pump + raster |
|---|---:|---:|
| T29.1: one `#[bind]` on a host-driven frame (`mark_all` → `narrowed`) | 20.94 µs of 57.40, **36%** | 21.08 µs of 80.92, **26%** |
| T29.6: a split click (`handled` → `handled/split`) | 23.92 µs of 57.26, **42%** | 25.18 µs of 87.23, **29%** |
| the arena cascade, of a narrowed frame (`narrowed`) | not in the sum | 21.39 µs of 59.84, **36%** |
| the arena cascade, of a split click (`handled/split`) | not in the sum | 20.53 µs of 62.05, **33%** |

**The saving is the same number of microseconds, and a smaller share.**
T29.1's 20.7 µs and T29.6's narrowing are real; what the stage sum
overstated was the share, because the frame was a third larger than the
sum. **And the cascade runs on all five arms**, at ~21 µs — about what a
narrowed binding saves. `bound` and `handled/split` cascade because a
binding wrote or an event was handled; `narrowed` cascades because
`Host::invalidate` keeps `arenas_dirty`. That is R6-F3, and T53.2 is
where it is priced; the review's statement that "the cascade exceeds the
saving T29.6 was built for" is, on these medians, a tie (21.39 against
21.08).

**`gap` is 0.08–0.09 µs on every run**: the six stages account for the
pump to within the focus-ring fixup and the timers. With the cascade
left untimed (the injection in T53.1's commit), it reads 20.87–23.30 µs
against bounds of 2.81–4.51 µs, on all 40 runs.

**T34.2, restated** (`flip::measure_the_flip`, eight runs a side; the
same ranges as T34.2's table, µs; LOADED, load 5–15 during the run):

| class | entry | `Everything` pump | (layout + place + paint) | `Boundaries<256>` pump | (layout + place + paint) |
|---|---|---:|---:|---:|---:|
| idle | Button | 0.72–0.74 | 0.57–0.58 | **0.49–0.50** | 0.33–0.33 |
| hover | Button | 0.87–0.88 | 0.58–0.59 | **0.83–0.85** | 0.54–0.55 |
| unconsumed press | Button | 0.90–0.91 | 0.58–0.59 | **0.86–0.88** | 0.55–0.56 |
| tick | ProgressBar | 1.98–2.08 | 0.83–0.84 | 1.91–2.02 | 0.81–0.82 |
| click | Button | 2.56–2.59 | 1.16–1.18 | 2.56–2.61 | 1.15–1.17 |

The flip's idle saving is **0.23 µs of 0.73, 32%** on the pump, not 42%;
hover's is 4%, not 7%. The tick and click frames are **more than twice**
the sum T34.2 printed, because both run the cascade and a click runs
input and the bindings, and T34.2's prediction — that they would not
move — still holds on the pump. The idle row is the whole pump here,
and it is not zero: R6-F1 (an idle pump places, paints and finishes) is
T53.3's.


## T53.2 — the arena cascade only where a mark is, 2026-09-27

**The cascade above ran on every arm, whatever the frame touched**
(R6-F3). Since T53.2 the pump asks only the nodes inside its marked
ranges (`style::cascade_arenas_marked`; `docs/plan-frame-cost.md`
T53.2 has why that is enough). The grid's owner is the body `Repeat`,
which no narrowed arm marks, so those arms cascade nothing.

`just grid`, the pump column (µs), a baseline binary built from the
parent commit and this one, **interleaved eight invocations a side**;
each figure is one invocation's median of its eight runs. Quiet (load
2.8 on 32 cores, `just bench-lock`).

| arm | before: pump (range) | before: cascade | after: pump (range) | after: cascade | delta of medians |
|---|---:|---:|---:|---:|---:|
| `handled/split` | **55.73** (55.21–56.95) | 20.42–20.85 | **33.97** (33.86–36.20) | 0.01–0.02 | **−21.76, −39%** |
| `bound` | **55.51** (53.82–56.61) | 19.54–20.91 | **33.97** (33.37–36.23) | 0.01–0.03 | **−21.54, −39%** |

The kill rule (`docs/plan-frame-cost.md` §5) wanted the `handled/split`
pump to fall by more than that arm's spread: it fell 21.76 µs against a
before-range of 1.74 µs. The `mark_all` and `handled` arms mark
everything and still walk the whole table, as they must. **The share
T53.1 printed for the cascade of a split click, 33%, is now zero**; what
is left of that frame is paint (a whole-table walk T16.8 refused to
skip) and `place`.

The count behind it is the gate: `tests/arena_marks.rs`, 40 clicks on a
box, **0 cascades in 40** (40 in 40 before), and 40 wheels over the
repeater still 40 in 40.

## T53.3 — an idle pump does nothing, 2026-09-27

**The idle row above was not zero** (R6-F1): under `Boundaries` a pump
with nothing marked skipped layout and still placed, painted and
finished. Since T53.3 it returns early when nothing is marked, handled,
applied or owed and no pointer or focus state moved
(`docs/plan-frame-cost.md` T53.3). Quiet (load 3.3/32, `just bench-lock`).

| measure | before | after |
|---|---:|---:|
| `idle_pump.rs`, 40 idle pumps: place / paint passes | 40 / 40 | **0 / 0** |
| `flip::measure_the_flip`, idle, `Boundaries<256>` pump | 0.49–0.50 µs | **0.06 µs** |
| same, `Everything` pump (never skips) | 0.72–0.74 µs | 0.69–0.70 µs |

The T34.2 idle row's definition changes with it: under `Boundaries` it is
now the fast path's cost — the input stage and the predicate — and under
`Everything` it is still the whole walk.

**Kill rule** (§5): the predicate must not slow a busy frame by more than
its spread. `just grid`'s `bound` arm, the T53.2 binary against this one,
interleaved eight invocations a side: median pump **33.98 → 34.01 µs**
(+0.03), against a fast-cluster spread of 0.25 µs. Ships.

**The oracle over the drives, fast path on:** `just agent-drive`'s oracle
leg compared 42 frames, 28 of them skipped; `just widget-drive`'s
compared 6 243, 5 397 skipped; none diverged. A headless sweep of the
workspace with `shadow-oracle` compared 4 289 frames with the same
divergences as before the change, less one it fixed (`loop_test` wrote a
label with no mark).

## M57/A5 — laying a repeater's rows out whole costs nothing measurable

*Superseded at the merge with M39–M55 (2026-09-28):* `main`'s
M41/T41.3 had fixed the same defect with `Changed::arena_view` and a
repeater that tracks which row each slot last showed, so `in_arena` was
removed and its call sites use that. The measurement below is the
branch's, of the mechanism it replaced.

`LayoutCx::in_arena` laid out every arena node (a `Repeat` row, a `List`
item, a `Dyn`'s content) with the dirty set treated as everything changed.
The set is indexed by the table, and an arena's nodes are not in it, so the
derive's skip had been asking about the main table's first nodes. Rows the
bind rewrote kept last frame's size (`docs/widget-audit.md` X3). The gate
is `crates/fulgurite/tests/repeat_rebind.rs`. With the frame's set
passed through instead, it measured 3 of 11 visible rows stale after a
three-row scroll.

The `grid` bench's 18-row body, eight runs a side, on the same machine and
in the same session:

| arm | layout µs, before | layout µs, after |
|---|---|---|
| narrowed | 2.55–2.65 (handled/split) | 2.54–2.68 |
| handled (mark_all) | 23.30–23.36 | 23.16–23.36 |

**Verdict: inside the arm's own spread, so kept.** A window of 18 rows was
already being re-bound every frame (T29.5). Laying out the rows it
re-binds adds less than the run-to-run noise. A second pair of runs taken
back to back showed a 2 µs shift in `paint`, which no layout change can
move. That shift is the machine's drift between runs, and it is why the
table compares rows taken in one session.
