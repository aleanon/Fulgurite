# Results: M26 — baselines, thresholds, and what each probe decided

*M26, `docs/plan-frame-and-types.md`. Owns every figure that plan cites.
Machine: i9-13900KF, governor `powersave`. Target for flash and RAM rows:
`thumbv7em-none-eabihf`.*

## The thresholds — written before anything is changed

Recorded at T26.0, 2026-09-17, so a task can be told no. Every frame row
is **eight** runs, each classified into the fast or the slow mode before
any spread is computed, with the count in each mode and the fast mode's
internal spread printed beside the delta.

| quantity | how | threshold | kill if |
|---|---|---|---|
| frame, table tree | `benches --bin crossover` | no regression beyond the arm's four-run spread | the delta exceeds the spread |
| frame, toggles | `benches --bin appearance` | same | same |
| frame, still and one-subtree | `benches --bin invalidation`, including the new place and dispatch rows | same | same |
| frame, arena rows | `benches --bin repeater`, `just stages 10000 150 20` | same | same |
| retention and paint | `benches --bin retain`, `benches --bin paint_sink` | same | same |
| cascade and lint | `benches --bin cascade`, including the new lint row | reported; a task that claims a cascade win states it is not a frame win | a claimed win inside the spread |
| raster frame | `just profile` (tally), plus the new popup-open frame | same | same |
| glass | `benches --bin glass-cost` | same | same |
| embedded flash | `scripts/check_text.sh` | ≤ +300 B over the M26 baseline | it exceeds that |
| embedded RAM | the const-asserted sizes in `fixtures/embedded` | unchanged unless the task records a new number | an unrecorded change |
| rustc time | the millisecond figure `just const-budget` prints, four builds, with its own spread | no regression beyond that spread | the delta exceeds it |
| allocation | `crates/lumen-static-shell/tests/alloc_gate.rs` | stays at zero, and covers the raster path from T26.13 | any steady-state allocation |
| backend parity | `just gpu-parity` | unchanged | any divergence |

**Why a mode rule and not a fixed bound.** M25/T25.1 wrote a 335 ns
threshold; M25/T25.12 then found the *baseline* running 1.7 µs above its
own median on some runs — and not as an outlier, as a second mode every
build exhibited. Counted into one spread it would have made every row
pass, which is a bound computed from the wrong quantity. So each arm
records both modes: how many runs landed in each, and the spread inside
the fast one. A delta is judged against the fast mode, and a task whose
runs land in one mode only says so.

**The rustc-time row is a printed figure, not the recipe's exit code.**
`just const-budget` fails only past 5 000 ms against a fixture that
builds in a fraction of a second, so the recipe is a ceiling that nothing
Phase C does could reach. The number it prints is the measurement; the
ceiling stays where it is.

## T26.0 — the baselines

*Taken 2026-09-17, eight runs per arm, i9-13900KF, governor `powersave`.
Each row gives the **fast cluster's** range and how many of the eight
runs landed in it; the slow mode is reported separately rather than
averaged in.*

**The mode rule earned itself on the first sweep.** Run 7 came back slow
on three of the five `--bin invalidation` rows — `Everything` 8.50 µs
against 5.27–5.63, `place` 2.39 against 1.01–1.08, `Boundaries, one
subtree marked` 0.89 against 0.52–0.54 — while `dispatch` in that *same
run* stayed at 1.12. A single spread over all eight would have been 3.2 µs
wide on a 5.3 µs row, and no Phase B change could ever have exceeded it.

### Frame arms

| arm | fast cluster | runs in it | slow mode |
|---|---:|---:|---|
| `--bin invalidation` Everything (the oracle) | 5.27–5.63 µs | 7/8 | 8.50 µs |
| `--bin invalidation` Boundaries, one subtree marked | 0.52–0.54 µs | 7/8 | 0.89 µs |
| `--bin invalidation` Boundaries, still frame | 0.01 µs | 7/8 | 0.02 µs |
| **`--bin invalidation` place** *(new)* | **1.01–1.08 µs** | 7/8 | 2.39 µs |
| **`--bin invalidation` dispatch, one PointerMove** *(new)* | **1.11–1.16 µs** (1.23 ns/node over 901) | 8/8 | — |
| `--bin cascade` cascade_deep (whole table) | 15,309–15,726 ns | 8/8 | — |
| `--bin cascade` cascade_subtree (90 of 901) | 1,459–1,531 ns | 8/8 | — |
| `--bin cascade` cascade_deep (showcase-shaped, 27 rules) | 66,184–66,716 ns | 8/8 | — |
| **`--bin cascade` lint_sheet_deep (same sheet)** *(new)* | **182,087–184,725 ns** | 8/8 | — |
| **`--bin crossover` either45/static** *(new)* | **210.9–214.4 ns** | 8/8 | — |
| `--bin crossover` 1k/static | 8,907–9,053 ns | 8/8 | — |
| `--bin bindings` short, memo warm (hit) | 5.35–5.45 ns | 8/8 | — |
| `--bin bindings` short, memo cold (miss) | 413.1–434.0 ns | 8/8 | — |
| **`--bin bindings` Badge, unchanged (no memo)** *(new)* | **403.7–422.3 ns** | 8/8 | — |
| **`--bin bindings` Chip, unchanged (no memo)** *(new)* | **406.7–424.3 ns** | 8/8 | — |

**The lint is the largest number here** — 183 µs against the 66 µs
cascade over the same sheet, on the same event. It is not on the frame
path, and T26.17 #10 is where it is decided.

### The popup frame — `just profile-showcase` *(new)*

| quantity | today |
|---|---|
| settled frame, damaged | **0 px** |
| popup-open frame, damaged | **691,200 px of 691,200 — 100% of the window** |
| commands across that frame | 9 → 30 |

That is T26.11's defect, stated as a measurement: a command-count change
answers `Damage::All`, so opening a 300×560 menu repaints a 1152×600
window.

### Budgets

| budget | measured | instrument |
|---|---|---|
| embedded `.text` | **26,364 B** of 65,536 (40.2%), 3 Toggle symbols linked | `scripts/check_text.sh` |
| `NodeState` / `Style` / `Ui` on the target | **76 / 72 / 1,136 B**, const-asserted | `fixtures/embedded` |
| rustc, 3,711 const nodes | **186–192 ms** (3 of 4 builds; the first was 752 ms, cold) | `just const-budget` |
| steady-state allocation | **zero**, 6 cases | `alloc_gate.rs` |

**The rustc row is why the "+10%" threshold was struck** (§ the plan's
review): the recipe fails only past 5,000 ms, which is 26× the measured
value, while the build-to-build spread is ~3%. A Phase C task is judged
against 186–192 ms, not against the ceiling.

### The arms T26.0 adds, and the injection each was shown to move under

*A new arm that does not move under its own injection is a finding about
the arm — `AGENT.md`'s rule — and is fixed before the baseline is taken.
Two of the six were found wanting this way and rebuilt.*

| arm | injection | moved by |
|---|---|---|
| `place`, `--bin invalidation` | the generated `place` returns before recursing into children | **1.01 → 0.01 µs** |
| dispatch, `--bin invalidation` | the hover and focus publishes write one node instead of sweeping every one | **1.11 → 0.03 µs** (1.23 → 0.03 ns/node) |
| `lint_sheet_deep`, `--bin cascade` | the arena walk visits nothing | **183,460 → 42,816 ns** |
| `either45`, `--bin crossover` | the hidden branch is not zeroed | **210.9 → 203.7 ns** |
| `Badge`/`Chip`, `--bin bindings` | `Badge::layout` returns a constant instead of shaping | **Badge 412.6 → 4.4 ns, Chip unmoved at 410.9** — the arm moves per widget |
| showcase popup, `just profile-showcase` | its own control: the settled frame's damaged area printed beside the popup frame's | see below |

**Two arms were wrong before they were right**, and both failures are the
shapes `AGENT.md` names:

- The `Badge`/`Chip` rows first ran against the stand-in measurer and
  printed **4.2 ns** — indistinguishable from a memo hit, because under
  `BlockMetrics` a measurement is `char_count × advance`. The arm could
  not exhibit the cost it exists to price. Rebuilt against a real face,
  where the same widgets cost **412 ns**, next to a memo hit's 5.4 and a
  memo miss's 420 — which is the claim T26.8 makes, now measurable.
- The showcase popup arm reported the **settled** frame as 100% damaged
  too, so it could not tell a still frame from a popup-open one. The
  cause was measuring the banked seed frame; the arm now banks first and
  measures a third, genuinely still frame.

**What the `Either` injection also settles.** Its move is the smallest
here — about **7 ns of a 211 ns frame, ~3%** — and that is not a weak
gate but a bound: zeroing 22 hidden nodes is 3% of this shape, so
Phase D's "cache the hide" probe cannot win more than that, and should be
judged against it before anything is built.

## Phase A — the defects

*One row per defect: what the failing test printed before the fix, and
what it prints after. No frame numbers are claimed for these; they are
correctness.*

### T26.1 — the enum derive zeroed the active variant

| | |
|---|---|
| before the fix | the active variant's container measured **0×0** where its sheet gives it 40×20, on `Boundaries<16>` |
| after | **40×20**, and the same session under `Everything` unchanged throughout |
| second gate | the invalidation oracle, whose fixture gained an enum and a variant switch: *"step 2: the display lists differ"* under the injection |
| sites inventoried | **10 hide call sites, 1 defective.** `Option` hides only on `None`, `Either` only the side it is not showing, `Popup` only when closed; the six `!visible` sites are stylesheet-driven and marked by `set_stylesheet`/`restyle_subtree`. Only the enum derive zeroed a subtree it then laid out |

**Why no test had seen it.** The stale zero is returned by the *variant's
own* relayout boundary — `can_skip() && is_clean(..) && constraints ==
limits` → `return st[i].size` — so it needs the active variant to be a
derived container, the enum to be reached, and its subtree to be clean.
An enum of leaves cannot exhibit it, and neither can any tree under
`Everything`, where nothing is ever clean.

### T26.2 — the agent reported a hidden node's kind as its container's

| | |
|---|---|
| before the fix | `ui.resolve` on `#d1`, a `Label` inside the inactive variant, answered **`Stage`** — the enum container's kind |
| after | **`label`**, and `#d3` **`fixed`**; the visible `#head` unchanged at `label` |
| the exemption's own gate | dropping the kind from `live_nodes`' identity fired *"setup: a visible node was reported hidden"* — a stand-in and its container share an address, so the kind is what separates them |
| sites moved | 6 reporting paths read `TreeRef::seeds`; `live_nodes` alone still asks the instance |

**Residue.** `role` is still read from the instance, so the same response
that now reports `kind: "fixed"` for a hidden node reports
`role: "GenericContainer"` beside it. `Seed` carries no role, so nothing
static can be read; deferred rather than papered over.

## Phase B — the confirmed optimizations

*One table per task: before, after, spread, verdict against the
threshold above.*

## Phase D — the plausible findings, decided

*One row per probe: the number it produced, the arm it produced it on,
and whether the finding was promoted or closed. A finding closed here is
closed permanently, with its number.*

| # | finding | probe result | verdict |
|---|---|---|---|
| | | | |
