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

## T28.3 — the spawner seam, and the three implementations

**Landed 2026-09-20.** `docs/plan-agent-parity.md`'s appendix settled
the location and this plan did not reopen it: `dyn` at the shell, never
a widget field, never a fourth type parameter on `Host`.

**A job is a non-capturing `fn` pointer supplied at spawn**,
`fn(Descriptor, &Sender<M>)`, read out of the currently loaded build at
the moment of spawn and gone by the time the result returns. Not a boxed
closure: an `fn` parameter refuses a capturing closure *at the call
site, by type*, which is the same guarantee the handler fields already
have and the same reason — `fresh()` rebuilds the tree but cannot reach
into work already in flight and swap the pointer it captured.

A `Descriptor` is `(kind, step, payload)`, three `u64`s. Three
primitives rather than a framework type, because
`docs/step4-hot-reload.md`'s honesty list forbids a framework type
crossing the dylib seam — v1 and v2 each link their own copy and no
layout agreement is promised — and that is a *separate* constraint from
`docs/events-decision.md` rule 3's ban on a code reference in flight.
The plan found its first draft applying only one of the two.

### The gate — all three, and they differ in exactly one dimension: *when*

| spawner | claim | measured |
|---|---|---|
| `Immediate` | the result is applied by the pump that follows the spawn | the very next pump applies it; running is not applying |
| `Queued` | nothing runs until asked, then the next pump applies it, **with no sleep** | three frames pass with nothing landed; `run_pending_jobs()` returns 1; the next pump applies it |
| `Threads` | the same job arrives within a bounded number of pumps | **2 pumps (1 ms)** against a bound of 200 |

`Queued` exists because of the middle row. Under `Threads` "not the
frame that spawned it" is a race; under `Immediate` it is false by
construction. A review of this plan's first draft found T28.5 asking for
exactly that property with neither spawner able to give it.

`Threads`'s 200 is a **timeout, not a latency budget**, and the 100× gap
against the measured 2 would be the wrong-quantity smell if it claimed
otherwise. It does not: it is the line between "arrives" and "never
arrives", and tightening it would only make the test flaky on a loaded
machine. The count prints beside the bound, which is the part of the
rule that applies.

A fourth case covers the refusal: **a spawn with no spawner installed is
refused and counted** (`Host::unspawned()`), not queued. Queueing would
mean work that runs whenever a shell happens to install one, and a host
that never does would accumulate jobs forever.

**Injection**, as the plan wrote it — the queued spawner runs at spawn
instead of on demand:

```
queued_runs_nothing_until_asked_and_lands_on_the_next_pump --- FAILED
a queued job ran without being asked: it landed [42] across three frames
```

The other three stay green, which localises it to `Queued`'s promise
rather than to the seam.

### One addition to the plan

`Spawner::run_pending` is on the **trait**, provided, answering zero.
The plan put the method on `Queued`, but a spawner lives behind `dyn` at
the shell: a test that downcast to reach it would be asserting against a
different object than the host uses, and a harness driving a queued
spawner generically is what a deterministic test needs.

This is a provided method that is right for every implementor — the case
`AGENT.md`'s `Deferring` rule is *not* about. That warning concerns a
provided method on a **wrapper**, where forwarding is always correct and
the default acts on the wrapper instead. Nothing here wraps anything.

## T28.4 — the outbox: how an application asks for work

**Landed 2026-09-20.** The review's first blocking finding against this
plan's first draft was that **nothing could spawn a task at all**. Every
application-authored code path is `fn(&mut S, &EventCtx)`: a handler
reaches the app and nothing else, and it cannot be handed a spawner,
because a spawner handle in the app would be a code reference in saved
state — which `#[reload(keep)]` already refuses to compile.

So the request is data. Input has a queue into the frame; `spawn::Outbox`
is the queue out of it — fixed capacity, no allocation, refusing and
counting on overflow like the inbox.

### The gate

| claim | measured |
|---|---|
| a handler that pushes causes exactly one spawn, on the frame its handler ran | one click → `landed == [105]`; a second frame with no click adds nothing |
| the descriptor is spawned on **that** frame | under `Queued`, `1 job pending at the end of that frame` |
| a full outbox refuses and counts | capacity 32: queued 32, refused 4, counter **4**, and a refusal does not overwrite |
| an unrecognised `kind` is dropped and counted | the known kind ran, the unknown did not, `unknown_kind() == 1` |

**Injection**, as the plan wrote it — the outbox drained *before*
dispatch instead of after:

```
one click: 0 job(s) pending at the end of that frame
the descriptor this frame's handler pushed was not spawned on this frame:
the outbox was drained before dispatch, so the request waits for the next event
```

Both ordering tests fail by name; the capacity and unknown-kind cases
stay green, because neither depends on the ordering. That pair is what
localises the injection to the drain's position.

### A test that was wrong before the code was

The first version of the first case asserted *"spawning is not
applying"* and expected a second pump. It failed, and the test was
wrong: under `Immediate` the whole round trip closes **in one frame**,
and that is a consequence of the two drains' order rather than a
coincidence. `pump_with` dispatches, drains the outbox (T28.4), and only
later drains the inbox before `App::update` (T28.2) — so an immediate
spawner's result is already waiting when the second drain runs.

Recorded because the property is load-bearing and undocumented
elsewhere: a later task that reorders those two drains will fail this
assertion, and should.

## T28.5 — one task, and a batch

**Landed 2026-09-20.** There is no batch *type*: a batch is several
descriptors pushed together, which is what it looks like from the
outbox. That is `docs/plan-tasks.md` §1's "every one of those is
described by values rather than by closures", applied.

### The drain row at N = 1 and N = 3, which the task owed

| row | ns (min, three runs) |
|---|---:|
| drain, empty queue | 4.54 |
| post+drain, **1** result | **17.37–17.40** |
| post+drain, **3** results | **46.84** |
| post+drain, 8 results | 120.77 |
| post+drain, 64 results | 944.99 |

N = 3 sits exactly on the line the other rows describe: 17.40 + 2 ×
14.7 = 46.8 against a measured 46.84. A batch of three costs three
results and no more — there is no per-batch term, because there is no
batch object.

### The gate

| claim | spawner | measured |
|---|---|---|
| one task's result reaches the app | `Queued` | `landed == [11]` |
| three spawned together all reach it | `Queued` | all three spawned, `landed == [1, 2, 3]` |
| **a batch promises arrival, not order** | `Threads` | `arrived [1, 2, 3] after 2 pumps` (bound 500), asserted as a sorted set |

The third row is the honest half. A batch is concurrent, so asserting an
order under `Threads` would be asserting the scheduler — it would pass
on this machine and fail on a loaded one, which is worse than not
asserting it. The deterministic order claim is made under `Queued`,
where it is a real property, and the thread arm asserts only that
nothing was lost.

**Injection**, as the plan wrote it — drop one of the batch (the last
descriptor is not spawned):

```
all three were spawned, not just the first     left: 2   right: 3
setup: exactly one job was spawned             left: 0   right: 1
threads batch: arrived [1, 2] after 500 pumps (bound 500)
```

**All three fail**, including the thread arm — which was worth checking
rather than assuming, because a test that accepts any permutation is
exactly the shape that can stop noticing a missing element. It does not:
it compares a sorted set against three known values, so a lost result
fails on length.

## T28.6 — chaining, as a step index

**Landed 2026-09-20.** The milestone's central move, and the one §2.1
exists for: iced chains with `task.then(|out| next)` — a closure held
across an await. Ported literally that puts a code pointer in flight and
breaks `docs/events-decision.md`'s rule 3 on the first reload, because
`fresh()` cannot reach into a pending future to swap the pointer it
captured, so the continuation would run last week's code.

**The next step is an index, not a function** — the same move that let
positional identity replace captures in handlers. `App::job_for` now
takes the whole `Descriptor` and dispatches on `(kind, step)`, so the
step *is* the chain.

### The gate

| claim | measured |
|---|---|
| a three-step chain runs in order, each step seeing the previous step's payload | `[(0, 5), (1, 50), (2, 55)]` — 4+1=5, 5×10=50, 50+5=55 |
| an unrecognised `(kind, step)` is dropped and counted | nothing ran; `unknown_kind() == 1` |

Under the **queued** spawner, so the step boundaries are a property
rather than a race. "Finish" is the absence of a push — the same shape
as a handler that does nothing.

**Injection**, as the plan wrote it — `job_for` ignores the step field
and always returns step 0:

```
chain saw [(0, 5), (0, 6), (0, 7)]
  left: [(0, 5), (0, 6), (0, 7)]
 right: [(0, 5), (1, 50), (2, 55)]
unknown (kind, step) dropped: 0
```

**The chain loops**, exactly as the plan predicted. Both tests fail, and
the second is the more interesting: with dispatch keyed on the kind
alone, a descriptor at step 9 ran *step 0's body under step 9's name*
and the unknown-step counter stayed at zero. That is §2.1's stability
rule failing in the way that matters — a renumbering producing a
plausible-looking result rather than an error.

### One change to T28.4's API

`App::job_for(kind: u64)` became `App::job_for(d: Descriptor)`. Keyed on
the kind alone a chain cannot be dispatched at all, and the injection
above is what that looks like from the outside.

## T28.7 — a chain across a dylib swap

**Landed 2026-09-20. The milestone's centre.**

The plan's first draft claimed `fixtures/hot-app` and `fixtures/hot-host`
"already drive a swap with state in flight". They do not: the sequence is
strictly sequential — save, drop, load, fresh, restore — with no thread,
no channel, no `Host`, no pump and no inbox, and step two destroys the
entire app object while the old library is still loaded.

So the arrangement is forced by §7's sequence rather than chosen: **the
chain's state lives in the loader process**, outside both dylibs. The
dylib contributes one thing — *what a step means* — through
`hot_step(kind, step, payload) -> u64`, which takes no session pointer,
because a step is a pure function of its descriptor and handing it the
app would invite a step that mutates state the host is about to restore
over.

The descriptor crosses as **three `u64` arguments**, never as a
`Descriptor` value: `docs/step4-hot-reload.md`'s honesty list item 1
forbids a framework type crossing the seam, since v1 and v2 each link
their own copy and no layout agreement is promised. That is a *separate*
constraint from rule 3's ban on a code reference, and the plan found its
own first draft applying only one of the two.

### The gate, in `just hot-demo`

v1's step 1 is `payload * 10`; v2's is `payload * 1000`. **Both builds
agree about step 0, deliberately** — if they differed too, the assertion
could not tell "v2 ran the second step" from "v2 ran everything".

```
chain: v1 ran step 0 -> 5; pending descriptor (1, 1, 5) held across the swap
chain: v2 ran step 1 on payload 5 -> 5000
  ok: a chain started before the swap completed into the NEW build's step
```

**Injection**, as the plan wrote it — the next step held as a function
pointer captured at spawn (v1's `hot_step` symbol) rather than as an
index:

```
chain: v2 ran step 1 on payload 5 -> 50
the chain completed into V1's step function (5 * 10): the next step was
held as a code pointer captured at spawn, which is what rule 3 forbids
```

The demo exits 101. This is the defect `docs/events-decision.md` rule 3
exists to prevent — last week's logic running after a swap, with no
crash and no warning — **demonstrated rather than argued**, which is
what the rule had been missing since it was written.

### The measurement: zero task symbols, and one re-budget

`docs/results-embedded.md` owns both numbers.

`.text` on thumbv7em is deliberately not the measure: the inbox and the
spawner are behind `std` and `fixtures/embedded` does not enable it, so
that number cannot move whatever M28 does. `0 task symbols (bound 0)`,
printed beside the Toggle count, is the falsifiable form — and the gate
is shown able to fire by pointing its pattern at a module that *is*
linked (11 symbols, non-zero exit, offenders printed).

Building the firmware for the first time in this milestone also found a
**const-assert regression from M26/T26.17 finding 12**: `MeasureKey`'s
new scale field grew `Ui` from 1,152 to 1,168 B on the target, four
bytes on each of four memo-holding widgets, attributed by removing the
field and watching the number return. Re-budgeted with the argument. It
had survived four green commits because the per-commit path never
targets thumbv7em.

## T28.8 — cancellation by epoch

**Landed 2026-09-20.** Every sender is minted at an epoch; a result
carrying any other epoch is dropped at the drain and counted.

**The comparison is equality, not an ordering test**, and the injection
is why.

### The gate, both directions and then some

| claim | measured |
|---|---|
| a cancelled task's result never reaches the app | `landed` empty, `stale() == 1` |
| an uncancelled one still does | `landed == [7]`, `stale() == 0` |
| work spawned **after** a cancel arrives | `landed == [1, 2]` |
| a sender minted before the cancel cannot un-cancel itself | `landed == [20]`, `stale == 1` |
| **a reload does not cancel** | the app object replaced wholesale, epoch unchanged, `landed == [99]` |

The third row exists because the first two could both be satisfied by a
type that drops everything after the first cancel — the
gate-that-refuses-everything shape. The fourth pins the epoch being
fixed at *mint* time rather than read at send time: read at send time, a
long-running job would quietly rejoin the current epoch and deliver a
result the application asked to be rid of, which is the whole failure
the mechanism is for.

### Injections

| injection | fires as |
|---|---|
| `e <= self.epoch` instead of `e == self.epoch` — the plan's own | `a cancelled result reached the app: [7]`, and `after cancel: landed [10, 20], stale 0` |
| a reload bumps the epoch — the plan's **first draft's specification** | `replacing_the_app_does_not_cancel_work_in_flight` fails by name |

Two accepting-direction tests stay green under the first, which
localises it to the comparison rather than to the drain.

The second injection is worth its own line: it implements what the plan
originally specified, and the test says why that was wrong. A reload
that bumped the epoch made T28.7's cross-swap chain **unpassable by
construction** — the pre-swap result would be discarded before the new
build's step could interpret it — and voided §2.1's own argument, which
is precisely that a result is *data* and therefore safe for the new
build to read. `just hot-demo` is the end-to-end version of the same
claim, across a real dylib swap.

### What "cancel" honestly means

The result is dropped; **the work is not stopped**. A thread already
running runs to completion and its output is discarded. Stopping work
needs cooperation from the job body, which this milestone does not
design. Saying so is better than a `cancel` that implies more than it
does.

Nothing in the framework calls `Inbox::cancel` — it is explicit
cancellation, driven by the application, which is what makes "a reload
does not cancel" true by construction rather than by care.

## T28.9 — what happens when a job fails

**Landed 2026-09-20.** The plan had no failure story and the review was
right that one is needed *before* anything is built on top: under the
thread default a panicking job unwinds, its sender drops, and **a chain
waiting on that completion stalls forever with nothing observing it** —
no crash, no log line anyone correlates, just work that never finishes.

A job that panics and a spawn the OS refuses both produce a failure
result the application applies like any other. Data, not a callback: a
callback would be a code reference, which is what this whole milestone
is arranged to avoid.

### The gate

| claim | spawner | measured |
|---|---|---|
| a panicking job delivers a failure, and the chain does not stall | `Threads` | `[Failed(0, Panicked)] after 2 pumps / 1 ms` (bound 500 pumps) |
| the deterministic arm agrees with the shipping one | `Immediate` | same failure |
| a spawn that never started reports too | none installed | `[Failed(0, NotStarted)]`, `unspawned() == 1` |
| a job that succeeds reports no failure | `Immediate` | `[Ok(9)]` |

The second row exists because a spawner that let the panic through would
make a test written against it pass while the real one stalls — the two
arms have to agree about the thing under test or the deterministic one
is worthless.

The third is the reachable form of "the job never started": a host with
no spawner. The OS refusing a thread is the other, and both take the
same path. **It is reported, not only counted**, because a chain whose
next step never started stalls exactly as one whose job panicked does,
and the application cannot tell from the outside unless it is told.

**Injection**, as the plan wrote it — let the panic unwind unobserved:

```
panicking job: saw [] after 500 pumps / 528 ms (bound 500 pumps)
a panicking job must deliver a failure; an empty `seen` here is the chain
stalling forever, which is the defect this task exists to remove
```

It fires by **timing out against its printed bound**, which is the shape
the plan specified. The `Immediate` case stays green under it, because
the injection was aimed at the threads path only — which localises the
catch rather than the reporting.

The test silences the panic hook around the deliberate panic and
restores it immediately, so an *unexpected* panic elsewhere still
prints. The default hook is otherwise left alone: this reports the
failure, it does not hide it.
