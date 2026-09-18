# Results: M28 — what deferred work costs

*M28, `docs/plan-tasks.md`. Owns every figure that plan cites. Machine:
i9-13900KF, governor `powersave`. Target for flash and RAM rows:
`thumbv7em-none-eabihf`.*

## The thresholds — written before anything is built

Recorded at T28.0, so a task can be told no. Frame rows are eight runs,
classified into the fast and slow clusters before any spread is computed
— the mode rule `docs/results-frame-and-types.md` records, for the reason
recorded there.

| quantity | how | threshold | kill if |
|---|---|---|---|
| allocation, a frame that drains | `crates/lumen-static-shell/tests/alloc_gate.rs`, extended to a pump that applies results | **zero** on the frame thread | anything allocates or frees there |
| frame, drain | `benches --bin crossover`, new drain row at N = 0, 1, 3 | N = 0 indistinguishable from today's frame | an application that defers nothing pays anything |
| frame, subscription diff | the same arm, with no subscriptions declared | indistinguishable from today's frame | same |
| wake path | a completion on an idle window reaching a presented frame | reported; no bound written before it is measured | — |
| embedded `.text` | `scripts/check_text.sh` | **unchanged** for a build that spawns nothing | it moves |
| struct sizes | the const asserts in `fixtures/embedded` | unchanged | an unrecorded change |
| crate count | `cargo tree` on the default feature set | **unchanged** by the default spawner | the default pulls a dependency |

**Why the crate count is a gate here and not a note.** This workspace
prices dependencies and has refused them on that basis: clipboard 11
crates, a11y 62, rfd 79, gpu 37 (`docs/plan-capabilities.md`). An async
runtime is the largest thing anyone has proposed adding, which is why the
default spawner is `std::thread` and a runtime is opt-in behind a
feature.

## T28.0 — the baselines

*Filled when T28.0 lands. Every row: the fast cluster's range, how many
of the eight runs landed in it, and the slow mode reported separately.*

| arm | fast cluster | runs in it | slow mode |
|---|---:|---:|---|
| | | | |

### The arms T28.0 adds, and the injection each was shown to move under

*An arm that does not move under its own injection is a finding about the
arm — `AGENT.md`'s rule — and is fixed before the baseline is taken from
it. Two of M26/T26.0's six were.*

| arm | injection | moved by |
|---|---|---|
| | | |

## Per task

*One section per task as it lands: what it measured, against which
threshold, and the verdict. No figure appears here that was not produced
by the arm named beside it.*
