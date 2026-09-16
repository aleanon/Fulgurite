# Results: M26 — baselines, thresholds, and what each probe decided

*M26, `docs/plan-frame-and-types.md`. Owns every figure that plan cites.
Machine: i9-13900KF, governor `powersave`. Target for flash and RAM rows:
`thumbv7em-none-eabihf`.*

## The thresholds — written before anything is changed

Recorded at T26.0, 2026-09-17, so a task can be told no. Every frame row
is four runs, medians, with the runs' own spread printed beside the
delta.

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
| rustc time | `just const-budget` | ≤ +10% over the M26 baseline | it exceeds that |
| allocation | `crates/lumen-static-shell/tests/alloc_gate.rs` | stays at zero, and covers the raster path from T26.13 | any steady-state allocation |
| backend parity | `just gpu-parity` | unchanged | any divergence |

**Why the spread and not a fixed bound.** M25/T25.1 wrote a 335 ns
threshold and M25/T25.12 then found a run of the *baseline* 1.7 µs above
its own median — an outlier that, counted into the spread, would have
made every row pass. Each arm's spread is therefore recorded here with
the outliers named, and a delta is judged against the undisturbed runs.

## T26.0 — the baselines

*Filled when T26.0 lands. Every row: four runs, median, and the spread
across them; the arms added by T26.0 additionally record the injection
that was shown to move them.*

| arm | median | spread | notes |
|---|---:|---:|---|
| | | | |

### The arms T26.0 adds, and the injection each was shown to move under

*A new arm that does not move under its own injection is a finding about
the arm — `AGENT.md`'s rule — and is fixed before the baseline is taken.*

| arm | injection | moved by |
|---|---|---|
| | | |

## Phase A — the defects

*One row per defect: what the failing test printed before the fix, and
what it prints after. No frame numbers are claimed for these; they are
correctness.*

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
