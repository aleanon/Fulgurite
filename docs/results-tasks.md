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
| allocation, a frame that drains | `crates/lumen-static-shell/tests/alloc_gate.rs`, extended twice: to a pump that applies results, **and to count frees** — its counting allocator increments on `alloc` only and its `dealloc` counts nothing today | **zero allocations and zero frees** on the frame thread, both printed | either is non-zero |
| frame, drain | `benches --bin crossover`, new drain row at N = 0, 1, 3 | N = 0 indistinguishable from today's frame | an application that defers nothing pays anything |
| frame, subscription diff | the same arm, with no subscriptions declared | indistinguishable from today's frame | same |
| wake path | a completion on an idle window reaching a presented frame | reported; no bound written before it is measured | — |
| embedded task symbols | `nm` on the linked firmware, printed beside the count the way `scripts/check_text.sh` prints its Toggle count | **zero** | any task symbol links into a leaf profile |
| struct sizes | the const asserts in `fixtures/embedded` | unchanged | an unrecorded change |
| crate count | `cargo tree -p lumen-static-winit --prefix none` | **unchanged** by the default spawner | the default pulls a dependency |
| outbox and subscription refusals | the counters T28.4 and T28.10 add | readable, and asserted where capacity is exceeded | an overflow is silent |

**Why the crate count is a gate here and not a note.** This workspace
prices dependencies and has refused them on that basis. By one method —
`cargo tree -p lumen-static-winit --prefix none` — clipboard costs +5
crates, dialogs +24 and the GPU backend +37 (`docs/results-gpu.md` §5).
`docs/plan-capabilities.md` records +11 and +79 for clipboard and rfd by
an older count; **the two methods are not commensurable and this doc uses
the first**. No figure here says what an async runtime would cost,
because nobody has measured one — which is the reason the default spawner
takes no dependency at all rather than a reason to prefer it.

## T28.0 — the baselines

*Filled when T28.0 lands. Every row: the fast cluster's range, how many
of the eight runs landed in it, and the slow mode reported separately.*

**`.text` on thumbv7em is not a row here, deliberately.** The inbox is
behind the `std` feature and `fixtures/embedded` does not enable it, so
that number cannot move whatever this milestone does — a bound computed
from the wrong quantity. The symbol count above is the falsifiable form
of the same claim.

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
