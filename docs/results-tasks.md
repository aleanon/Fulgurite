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

**Landed 2026-09-19.** Two of the three arms are here; the third is in
T28.1's section, for the reason given at the end.

**`.text` on thumbv7em is not a row here, deliberately.** The inbox is
behind the `std` feature and `fixtures/embedded` does not enable it, so
that number cannot move whatever this milestone does — a bound computed
from the wrong quantity. The symbol count above is the falsifiable form
of the same claim.

### 1. The allocation gate now counts frees

The gate counted allocations and forwarded `dealloc` to the system
without counting. **A frame that frees on the frame thread and allocates
nothing read as clean** — and that is exactly the shape M28 introduces,
because draining a completed job out of an mpsc queue frees the queue's
node with no matching allocation on that thread. A one-sided counter
cannot see it.

Both counters stay per-thread, for the reason the file already records:
`cargo test` runs tests in parallel threads sharing one process, and a
global counter measured every other test's allocations.

| case | allocs/frame | frees/frame |
|---|---:|---:|
| plain tree, idle | 0 | **0** |
| `Repeat` + `List` + `Either` + `Dyn` + `Option`, idle | 0 | **0** |
| two enum nodes, idle | 0 | **0** |
| `Rebuild` retention | 0 | **0** |
| a binding that re-formats | 0 | **0** |
| a scrolling repeater | 0 | **0** |
| 50 rasterised frames | 0 | **0** |
| 20 frames, two glass panels | 0 | **0** |

Zero in both directions today, which is the baseline every later task in
this milestone is judged against.

### 2. `--bin crossover` gained drain rows

`Inbox::drain` shipped as plan-events E3 and nothing had ever priced it.
Messages are `u32` — the cheapest thing a queue can carry — so these rows
measure the **queue and the dispatch**, not a payload. A real message is
larger, so the slope is a floor, which is the honest direction for a
baseline to err in.

| row | ns (min) |
|---|---:|
| drain, empty queue | **4.72** |
| post+drain, 1 result | 25.89 |
| post+drain, 8 results | 172.89 |
| post+drain, 64 results | 1250.46 |

Per-result slope **19.4 ns** — (1250.46 − 25.89) / 63 — with an intercept
of ~6.5 ns that agrees with the empty drain's 4.72.

Against the threshold above, *"N = 0 indistinguishable from today's
frame"*: the empty drain is 4.72 ns against an idle frame of ~350 µs
(`docs/results-frame-and-types.md` T26.17), or **0.001%**. Passes.

The rows are named `post+drain`, not `drain`. The refill must happen
inside the timed closure, and calling the row "the drain" would be the
bound-computed-from-the-wrong-quantity shape: the post is a `send`, the
drain is a `try_recv`, and both touch the same queue.

### The arms T28.0 adds, and the injection each was shown to move under

*An arm that does not move under its own injection is a finding about the
arm — `AGENT.md`'s rule — and is fixed before the baseline is taken from
it. Two of M26/T26.0's six were.*

| arm | injection | moved by |
|---|---|---|
| the free counter | `dealloc` restored to forwarding without counting — the shape before this task | `the_free_counter_sees_a_drain_that_only_frees` fails by name, reporting `allocs 0, frees 0` against a required 16. The other eight cases stay green, so the pair is refuses-bad / accepts-good |
| the drain rows | `Inbox::drain` made a no-op | every row moves and the empty row falls to the harness floor: 4.48 → **0.17** ns; 1 result 24.72 → 9.33; 8 results 165.12 → 73.69; 64 results 1192.57 → 580.77 |

**The free counter is also shown able to move in the accepting
direction**, because a counter that never moves is indistinguishable
from one that does not work. Every case above reports zero, so one test
makes a free happen on purpose: a worker allocates 16 buffers and sends
them; the test's own thread drains and drops them.

```
drained 1024 bytes: allocs 0, frees 16
```

`allocs 0` is what the old gate saw, and it is the whole argument for
the second counter.

**The drain injection paid more than it was asked for.** It decomposes
the pair: post-only has a slope of **9.1 ns** per result
((580.77 − 9.33) / 63), so of the 19.4 ns the drain half is **10.3 ns**
and the post half 9.1.

### 3. The wake path — built with the mechanism it measures

The plan's injection for this arm is "drop the wake", which presupposes
a wake. Outside the `agent` feature `lumen-static-winit` built
`EventLoop::new()`, which has no user-event channel at all, so there was
nothing to drop and nothing to time. Building the arm against a path
that does not exist would have measured its absence — the "setup
silently did not happen" shape. It lands in T28.1's commit instead, and
its number is in T28.1's section below.

## T28.1 — an idle window wakes when work completes

**Landed 2026-09-19.**

| quantity | measured |
|---|---:|
| worker's send → a frame, on a window idle for 600 ms | **~3 ms** (603 ms elapsed against a 600 ms settle; a second run gave 602) |
| the fixture's deadline | 5 000 ms |

No bound was written before this was measured, which is what the
threshold table said to do: *"reported; no bound written before it is
measured"*. The deadline is not a latency budget — it is the line
between "a wake arrives" and "the loop slept through a completion", and
a real budget would need a quiet machine to mean anything. The elapsed
time prints beside it on success as well as failure, so a bound that
drifts away from its quantity is visible rather than inferred.

### The gate, split along the seam this repository already uses

| half | where | asserts |
|---|---|---|
| deterministic | `crates/lumen-static/tests/inbox_wake.rs`, in `cargo test` | sending **asks** for a wake, against a recording proxy and no window |
| live | `scripts/wake_gate.sh` → `examples/wake-gate`, `just wake-gate` | the ask reaches a real winit loop in `ControlFlow::Wait`, and a frame follows |

The live half is a binary that asserts *itself* for three reasons, all
of them ones the other live gates here already have: `cargo test` runs
on every commit with no display; it has no per-test timeout, so the
failure this gate catches — *no wake ever arrives* — would hang CI
rather than fail it; and winit wants the loop on the main thread. It
skips with exit 0 where there is no display.

### Injections

| half | injection | fires as |
|---|---|---|
| deterministic | `Sender::send` drops the wake request | `a_send_from_a_worker_asks_for_a_wake` fails by name — *"four sends asked for 0 wakes, not 4"* — and so does `a_sender_minted_before_the_waker_says_so`. The two accepting-direction tests stay green |
| live | `ProxyWaker::wake` made a no-op — the plan's own injection | the fixture reaches its deadline and exits **1**: *"no frame followed the worker's send within 5000 ms"* |

Two accepting-direction cases keep this from becoming a host that wakes
on everything: `an_inbox_with_no_waker_still_delivers` (a headless
`Host`, a bench, or an embedded target has no loop, and sending must
still work) and `post_wakes_too`.

A third asserts a real ordering hazard rather than assuming it away.
`a_sender_minted_before_the_waker_says_so`: a sender handed out *before*
`set_waker` carries no waker, the rustdoc says so, and the test pins it.
A host that minted a sender in its constructor and installed the waker
afterwards would produce exactly the late-frame symptom this task
removes, and nothing else would have noticed.

## Per task

*One section per task as it lands: what it measured, against which
threshold, and the verdict. No figure appears here that was not produced
by the arm named beside it.*

## T28.2 — the host owns the inbox, and the app declares its message

**Landed 2026-09-20.**

### The queue is bounded, and that is cheaper as well as safer

`Inbox::new` was `mpsc::channel()` — unbounded — and `drain` looped until
empty. Two failure modes from one default: a producer that outran the
frame loop grew the queue without limit, and the drain was unbounded work
inside `pump`. It is now `sync_channel(256)` whose sender **refuses**
rather than blocks, with the refusals counted, and the drain takes at
most 64 per frame.

Re-measured on the same `--bin crossover` rows T28.0 added, min of three
runs:

| row | unbounded (T28.0) | bounded | |
|---|---:|---:|---|
| drain, empty queue | 4.72 | **4.54** | −4% |
| post+drain, 1 result | 25.89 | **17.40** | −33% |
| post+drain, 8 results | 172.89 | **120.79** | −30% |
| post+drain, 64 results | 1250.46 | **946.18** | −24% |

Per-result slope **19.4 → 14.7 ns**, because an unbounded channel
allocates a node per send and frees it per receive and a `sync_channel`'s
buffer is allocated once, at construction. The bound was chosen for
safety and is faster; that is worth stating because the opposite is the
usual trade and would have been accepted here.

### The allocation gate, extended the second way

T28.0 added the free counter. This adds the case the plan asked for —
*a pump that applies results* — because every case in that file has
`Msg = Never` and therefore drains a queue nothing can write to, which
proves only that an empty drain is free.

| case | allocs | frees |
|---|---:|---:|
| 50 frames, each applying one `u32` result (1 660 applied) | **0** | **0** |

The messages carry no heap payload, so anything the allocator saw here
would belong to the queue. **The claim about channel types is measured,
not argued**: `the_free_counter_sees_a_drain_that_only_frees` drains
sixteen messages out of an unbounded `mpsc::channel` and reports
`allocs 0, frees 16` — one free per message, on the draining thread —
while this case drains fifty out of the bounded one and reports
`frees 0`. Same counter, same thread, two channel types.

### The gate

`crates/lumen-static-shell/tests/deferred.rs`, six cases, on
`Boundaries<16>` because a result that changes state must **mark** it.

| claim | measured |
|---|---|
| a result posted between frames lands on the next pump | applied once; posting alone applies nothing |
| exactly once | a second pump with nothing posted does not re-apply |
| in order | `[0,1,2,3,4]` from five sends |
| applying marks the tree | label width **0 → 80** under `Boundaries` |
| an overrun is refused, not blocked | capacity 256: accepted 256, refused 10, counter **10** |
| a backlog is spread over frames | 69 sent, 64 applied on frame one with `more_deferred()` true, the rest on frame two, still in order |

**The fixture's applied field carries no `#[bind]`**, which the plan
named as a trap before the test existed: `pump` ends with
`if self.app.update() { mark_all() }`, so a bound field would mark the
whole tree whatever `apply` did and the injection below would stay green.
The label is written by `apply` directly and nothing refreshes it.

**Injection** — apply without marking, exactly as the plan wrote it:

| strategy | result |
|---|---|
| `Boundaries<16>` | `applying_marks_the_tree_under_boundaries` fails by name: `label width 0 -> 0` |
| `Everything` | stays green — it relays out regardless |

The pair is what localises the defect to the marking rather than to the
apply.

### Two corrections to the plan

**§2.6 said the 13 hand-written `App` impls would be unchanged.** They
are not, because associated type *defaults* are unstable
(rust-lang#29661) — only default methods are stable. `type Msg` is
required, the derive emits `type Msg = Never;` for the ~27 derived apps,
and 19 hand-written impls gained two lines each. The count was also 19,
not 13.

**`App::apply` has no default body either**, which is a deliberate step
past what the plan asked for. A default that ignored the message and
returned `false` would compile for an application that declares a real
`Msg` and forgets to interpret it, and the symptom would be deferred
work that silently never lands. Requiring it means the compiler asks.
