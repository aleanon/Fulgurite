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
| allocation, a frame that drains | `crates/fulgurite-shell/tests/alloc_gate.rs`, extended twice: to a pump that applies results, **and to count frees** — its counting allocator increments on `alloc` only and its `dealloc` counts nothing today | **zero allocations and zero frees** on the frame thread, both printed | either is non-zero |
| frame, drain | `benches --bin crossover`, new drain row at N = 0, 1, 3 | N = 0 indistinguishable from today's frame | an application that defers nothing pays anything |
| frame, subscription diff | the same arm, with no subscriptions declared | indistinguishable from today's frame | same |
| wake path | a completion on an idle window reaching a presented frame | reported; no bound written before it is measured | — |
| embedded task symbols | `nm` on the linked firmware, printed beside the count the way `scripts/check_text.sh` prints its Toggle count | **zero** | any task symbol links into a leaf profile |
| struct sizes | the const asserts in `fixtures/embedded` | unchanged | an unrecorded change |
| crate count | `cargo tree -p fulgurite-winit --prefix none` | **unchanged** by the default spawner | the default pulls a dependency |
| outbox and subscription refusals | the counters T28.4 and T28.10 add | readable, and asserted where capacity is exceeded | an overflow is silent |

**Why the crate count is a gate here and not a note.** This workspace
prices dependencies and has refused them on that basis. By one method —
`cargo tree -p fulgurite-winit --prefix none` — clipboard costs +5
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
a wake. Outside the `agent` feature `fulgurite-winit` built
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
| deterministic | `crates/fulgurite/tests/inbox_wake.rs`, in `cargo test` | sending **asks** for a wake, against a recording proxy and no window |
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

`crates/fulgurite-shell/tests/deferred.rs`, six cases, on
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

> **Corrected 2026-09-27 (M49/T49.10, R4-F3).** What follows shows the
> *principle* on real code: a descriptor held as data is interpreted by
> whichever build is loaded when it lands. It does **not** show a
> framework property. The pending descriptor in this gate is held by the
> fixture's loader, which has no inbox and no epoch at all; no
> `fulgurite::spawn` or `inbox` code is on its path (`docs/results-deep-review.md` R4-F3 counted
> zero such symbols). Until T49.10 the framework's window swap carried no
> work at all: the old session's inbox and its jobs were dropped with it.
> The framework half now exists and is gated with a framework-side
> injection. It is in "M49/T49.10" at the end of this file.

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
| a failure the application does not model is **counted** (M47/T47.6, R7-F9) | `Threads` | `unmodelled_failures 1 (want 1) after 1 pumps (bound 500); messages []` — was 0 after 500; a modelled one leaves it at 0 |

> **Corrected 2026-09-26 (M47/T47.6).** `FailFn`'s rustdoc said the host
> "counts the drop" when an application does not model a failure, and
> nothing did: `report()` dropped it, and the panic reached stderr only.
> The spawners' failure path now counts it on a counter the inbox shares
> with every sender, read as `Host::unmodelled_failures`. Injection — the
> increment removed: `unmodelled_failures 0 (want 1) after 500 pumps`.

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

## T28.10 — subscriptions: start, stop, and identity

**Landed 2026-09-20.** Identity is `(kind, key)` — values — and the host
diffs the application's declared set each frame against what it already
started.

Hashing a recipe, as iced does, would put a stream's identity in the
code that produces it, which is the coupling
`docs/events-decision.md` rule 3 removes from chains: two builds that
format a URL differently would restart every subscription on reload, and
two that format it the same would silently share one.

### The gate

| claim | measured |
|---|---|
| a subscription that appears starts **once** | five frames, `started 1, stopped 0`, one message |
| one that vanishes stops — and stays stopped | `(1, 1)` after clearing, still `(1, 1)` two frames later |
| one whose **key** changes is stopped and restarted | `1 -> 2`: `started 2, stopped 1`, `landed == [1, 2]` |
| declaring the same identity twice is one subscription | `subs.len() == 1`, one start |
| the set refuses past capacity and counts | capacity 16: declared 16, refused 3, counter **3**, no eviction |

The "stays stopped" and "twice is one" rows are the ones that make the
others meaningful: a diff that restarted every frame would satisfy row
one's *start* and fail its count, and an application declaring from a
loop would start N copies of one stream with the set never settling.

**Injection**, as the plan wrote it — diff by kind alone, ignoring the
key:

```
after the key changed 1 -> 2: started 1, stopped 0
  left: (1, 0)
 right: (2, 1)
a changed key must stop the old stream and start a new one; diffing by
kind alone leaves the old one running and starts nothing
```

The other four stay green, which localises it to the key rather than to
the diff. That case is also the one that matters in practice: a stream
keyed on "which document is open" changes key far more often than it
appears or vanishes.

### The measurement

The threshold was *"the per-frame cost of the diff with no subscriptions
declared must be indistinguishable from today's frame"*.

| | µs |
|---|---:|
| idle frame, 10 000 nodes, before M28 (`docs/results-frame-and-types.md` T26.17) | 349.97 / 351.47 |
| the same, with the subscription diff, the inbox drain and the outbox drain all in `pump` | **351.19** (three runs: 351.19, 370.50, 378.63) |

Inside the arm's own spread — and that is the whole of M28's per-frame
cost, not the diff's alone. `App::subscriptions` defaults to `None`, so
an application with no subscriptions pays one `Option` check and the
early return happens before any allocation.

The allocation gate agrees: `50 frames each applying one result: allocs
0, frees 0`, unchanged by this task.

## T28.11 — timers, and the deadline the loop waits on

**Landed 2026-09-20.** A timer is a **deadline**, not a thread that
sleeps. A thread per timer costs a thread and a context switch per tick,
and it puts the timer's identity in a thread handle rather than in data
— the coupling the rest of this milestone is arranged to avoid.

But a deadline needs a frame to be checked on, and **an idle window
produces none**: the loop is `ControlFlow::Wait` and asks for a redraw
only while something is animating. So the host publishes its earliest
pending deadline (`Host::next_deadline_ms`) and a shell waits *until*
that instant, falling back to indefinite when nothing is pending.

The plan's first draft gated this on "a second of driven frames", which
is the one situation where the defect cannot appear — a driven loop
produces frames by definition.

### The gate, both halves

**Deterministic**: forty frames of a 25 ms tick is exactly one synthetic
second, and both quantities print beside their bounds so a setup that
silently did not tick is visible in the output rather than inferred.

```
40 frames x 25 ms: elapsed 1000.0 ms (of 1000.0), a 50 ms timer fired 20 times (of 20)
```

**The mechanism**:

| claim | measured |
|---|---|
| zero threads for a timer, counted **through T28.3's spawner seam** | `20 timer firings went through 20 spawns (one job each, no timer thread)` |
| the host publishes its earliest deadline | `None` when idle; `Some(50.0)` of a 50/200 pair; `20.0` after 30 ms; `170.0` when the soonest is cancelled; `None` again when the last is |
| a long frame catches up rather than dropping ticks | one 80 ms frame with a 25 ms timer: `fired 3 times (of 3)` |
| a nonsense period is refused and counted | 0, −5 and `NaN` refused (3 of 3); 16 ms accepted |

The thread count is measured through the seam rather than by asking the
OS, because **every thread this milestone creates goes through
`Spawner::spawn`** — which makes counting spawns a measurement a test
can actually make.

> **Corrected 2026-09-26 (M47/T47.3, R7-F5).** The first row counted
> **spawns**, and called them threads. A timer creates no thread of its
> own, but each firing is a job, and the `Threads` a window installs made
> every job a `std::thread`: strace counted 28 clones for 28 ticks of
> `tasks-demo`. The test is renamed `a_timer_firing_is_one_spawn`, and the
> thread count is now measured where threads are made — T47.3 below.

The catch-up row is what lets the deterministic half exist at all: a
timer that silently drops ticks under load has a count that depends on
the frame rate, and a count that depends on the frame rate cannot be
asserted.

> **Corrected 2026-09-26 (M47/T47.2, R7-F3, R7-F4).** The catch-up held
> for a frame, not for a stall. It was a `while` adding the period to an
> `f32` remainder, which stops moving past about 2^24 periods: `dt` 1e9 ms
> at 16 ms never returned. And each firing was a push into a 32-slot
> outbox whose refusals `Host::tick` dropped, so a 10 ms timer handed
> `tick(1000)` fired 100, delivered 32 and counted 68 nowhere. Both are
> fixed in T47.2 below; "catches up" now reads "up to the outbox's
> capacity, and counts the rest".

### Injections

| injection | fires as |
|---|---|
| a thread spawned per tick, producing no message — the plan's own | `40 spawns for 20 firings means something is spawning per tick as well` |
| the loop left waiting indefinitely (no deadline published) | `the_host_publishes_its_earliest_deadline` fails on "the soonest of the two" |

The first injection was made **precise on the second attempt**. The
first version pushed each due descriptor twice, which fired three tests
— it doubled the firings as well as the spawns, so it did not show that
the spawn count alone can see a thread-per-tick. Spawning a job that
produces no message isolates it, and only the spawn-count assertion
fires.

## T28.12 — use the thing

**Landed 2026-09-20, and it closes M28.**

`AGENT.md`, after M24: an end-to-end assembly is the only thing that can
find a capability which is present, correct in isolation, and
unreachable. Every one of M28's pieces had a passing test; none had been
used together, by an application, through a real window.

`examples/tasks-demo` spawns, batches, chains, cancels and subscribes.
`just tasks-drive` clicks every button through the agent endpoint under
**both** invalidation strategies — which is a compile-time feature of
`fulgurite-winit`, so the script builds twice.

```
== everything ==
  ok  clean start {'one': 0, 'batch': 0, 'chain': 0, 'slow': 0, 'ticks': 0, 'failed': 0, 'sub': 'off'}
  ok  spawn: exactly 1 result (bound 1) after 41 ms (budget 4000)
  ok  batch: all 3 results (bound 3) after 42 ms (budget 4000)
  ok  chain: 3 steps (bound 3) after 164 ms (budget 4000)
  ok  chain: composed to value=8 ('last: chain step=2 value=8')
  ok  cancel: slow stayed at 0 across 1000 ms (the job itself takes 400)
  ok  cancel is not permanent: one=2 (bound 2) after 41 ms (budget 4000)
  ok  subscription on: >= 3 ticks (bound 3) after 286 ms (budget 4000)
  ok  subscription off: ticks froze at 3 across 600 ms (period 120)
  ok  no failures; final {'one': 2, 'batch': 3, 'chain': 3, 'slow': 0, 'ticks': 3, 'failed': 0, 'sub': 'off'}
```

Identical under `boundaries`.

Every line asserts a **consequence** with its quantity beside its bound,
not that something changed. The cancel line is the shape that matters:
the job itself takes 400 ms and the assertion watches for 1000, so
"nothing arrived" is a claim about a window the result had ample time to
land in.

### What the assembly required that none of the unit tests did

Two pieces of wiring that every passing test had done for itself:

* **`run()` installs the waker and the spawner.** `Host::new` installs
  neither — a headless host has no loop to wake and a test wants to
  choose its own spawner — so until now a real window could ask for work
  and never get it.
* **`about_to_wait` sets `WaitUntil` from the host's deadline.** Without
  it a timer on an idle window is never checked, which is the exact
  defect T28.11 describes and which no headless test can exhibit.

This is what "use the thing" is for.

> **Corrected 2026-09-25 (M47/T47.1, `docs/plan-timers-wake.md`).** The
> second bullet was **false until T47.1**, and the drive above could not
> see it (R7-F1, R7-F2). `WaitUntil` woke the loop at the deadline and
> nothing asked for a frame, so the timer was not checked until something
> else drew one — and every read `tasks_drive.py` makes ends in a redraw
> request, so the window was never idle while the drive watched it. Both
> of M38's injections (waker uninstalled; deadline ignored) left all ten
> checks green. `examples/wake-gate` covers the `ProxyWaker` wire (T28.1)
> with its own handler and its own waker, not `run()`'s wiring.
>
> The drive now ends with two **unobserved** phases: one read after a
> sleep with no RPC traffic, answered before the frame it requests.
>
> | build | unobserved 120 ms timer, 3.0 s | unobserved 400 ms job, 1.5 s |
> |---|---|---|
> | `b213ae0` (before T47.1) | **1** tick (expected 25, bound 24) | slow 1 (want 1) |
> | T47.1 | **26** ticks (expected 25, bound 24), both strategies | slow 1 (want 1), both strategies |
> | T47.1, waker install removed (injection A) | 25 | **0** (want 1) |
> | T47.1, deadline ignored and `new_events` deleted (injection B) | **1** | — |
>
> The fix: the window fixes an absolute `WaitUntil` instant after each
> frame, from that frame's start, and `new_events` turns a wake at or past
> it into a frame. No deadline is armed while the panic screen is up, and
> the retry's first frame ticks by 0 (`docs/plan-panic-recovery.md`
> §2.2). Counts from `just tasks-drive` on 2026-09-25, machine idle
> (load 0.68); they are counts, not clocks.
>
> **Re-gathered 2026-09-26 with M42's identity line** (M47/T47.0), each
> leg naming the process it drove — `drove pid P (socket owner P),
> strategy …Everything / …Boundaries<256>, features agent[,everything],
> exe …/target/release/tasks-demo`: unobserved timer **26** (expected 25,
> bound 24) under both strategies, unobserved wake `slow 0 -> 1` under
> both. `just panic-drive`, same day, identity lines likewise: the retry
> after 2 s up added **0** ticks beyond the 2 due (bound 2) under both,
> and the loop woke **0** times while the screen was up (T47.8).

### Injections — all three fire, and one is more interesting than the others

| injection | fires as |
|---|---|
| the host ignores the application's cancel request | `a cancelled task's result landed (slow 0 -> 1); cancel did nothing` |
| the subscription toggle does not stop the timer | `the subscription kept producing after it was switched off (4 -> 6)` |
| the chain always re-runs step 0 | `chain: 3 steps (bound 3)` **passes**, then `the chain's last value should be 8 (1*2*2*2), got 'last: chain step=0 value=16' -- the steps did not compose` |

**The third is the argument for asserting consequences rather than
counts.** Three steps ran, the count was right, and the chain was
completely broken — it ran step 0 three times. A drive script that
checked "chain == 3" would have been green. The composition check
catches it because 1→2→4→8 is a value only an ordered chain can produce.

## M47/T47.3 — a pool instead of a thread per job

**Landed 2026-09-26** (`docs/plan-timers-wake.md` T47.3, R7-F5). `Threads`
keeps its name and its place (`Local::from_host` installs it) and is now
a pool: at most `Threads::cap` long-lived workers — the machine's
parallelism clamped to 2..=4 — fed through a channel, started **lazily**
(one when a job arrives and no worker is free). A panicking job is
caught inside the task, so the worker survives it.

### The kill rule, judged before the gate

Written before the build (`docs/plan-timers-wake.md` §5): if the pool's
median spawn-to-apply is slower than the thread-per-job median by more
than the per-job arm's own spread over eight runs, only the claim is
corrected. `--bin spawn` times one no-op job from `Host::spawn` to its
result applied by a pump, one arm per process, eight runs a side,
alternating, under `flock bench.lock`. **LOADED**: load average 5.1 on
32 cores before and after.

| arm | medians of eight runs (ns) | median | spread |
|---|---|---:|---:|
| thread per job (the pre-T47.3 body) | 15088 15188 15215 15245 15449 15512 16035 16180 | **15347** | 1092 |
| pool (`Threads`) | 1862 2022 2494 2532 2541 2753 2832 3068 | **2537** | 1206 |

The pool is **6.0× faster** than the arm it replaced, not slower: a
thread's creation was most of what a job cost. The kill rule does not
fire, and the pool ships.

### The gate: threads created, where threads are made

| check | measured | bound |
|---|---|---|
| `timers.rs` `twenty_firings_create_at_most_the_pool` — distinct `ThreadId`s the 20 jobs ran on | **3** (was 20) | the pool's cap, 4 here |
| `ledger.rs` `a_slow_job_does_not_hold_a_quick_one_back` | a 1 ms job lands after **23 ms** beside a 400 ms one | 300 ms |
| `just tasks-drive`, new phase 7 — one unobserved read 330 ms after a 400 ms job started, then the timer | **3** ticks, job still running; 26 of 25 at 3 s | ≥ 1; ≥ 24 |

Distinct `ThreadId`s rather than the OS task list, which the plan named:
a thread per job exits before the next firing, so the task list reads
the same before and after on the stock build and cannot go red. A
`ThreadId` is never reused within a process, so distinct ids are threads
created. The pool creates *at most* its cap, not "exactly the pool size,
once" as the plan's accepting line put it: workers start lazily, and 20
sequential firings started 3.

### The ledger — what "in flight" means now

Every job a host spawns is listed in its `spawn::Ledger` (a descriptor
and a shared `inbox::JobTrack`) and leaves it once its body has returned
**and** every result it sent has been drained — applied, or dropped as
stale. `Host::in_flight` is the ledger's length, pruned as it answers.
The sender a host hands each job is the job's own
(`Inbox::tracked_sender`): every message it sends is counted against the
job, and its drop says the body returned, so a spawner written outside
this crate ends its jobs by doing what it already does.

| `ledger.rs` | in flight |
|---|---|
| after spawn / after the body ran, result in the inbox / after the pump applied it (`Queued`) | 1 / **1** / 0 |
| a job that sends nothing, before / after its body | 1 / 0 |
| a cancelled result, drained as stale | 0 |
| three `Threads` jobs (60, 1, 2 ms) at spawn / at the end | 3 / 0 |

The middle `1` is the row a count kept by the spawner would get wrong,
and the one a reload re-issuing work needs most.

### Injections

| injection | fires as |
|---|---|
| the pool's `queue.send(task)` replaced by `std::thread::spawn(task)` — a thread per job again | `threads created 20 for 20 firings (bound 4, the pool's cap)` |
| the drain stops counting consumed results | `after the pump applied [7] 1 (want 0)`; the cancelled and `Threads` cases likewise |
| `JobTrack::settled` ignores the drain (body returned is enough) | `after the body ran 0 (want 1)` |
| the owning sender's drop does not end the job | `silent job: in flight 1 before its body ran, 1 after (want 1, 0)`, and three more |
| a pool of one worker (`with_cap(1)`) | headless: `quick job landed after 403 ms ... (bound 300 ms, pool cap 1); landed [400, 1]`; live: `no tick landed in 331 ms while a 400 ms job ran: a busy worker held the timer back` |

Restored, all green again, both strategies.

## M47/T47.2 — catch-up by arithmetic, and every firing accounted for

**Landed 2026-09-26** (`docs/plan-timers-wake.md` §2.2(c), R7-F3, R7-F4).
`Timers` holds its periods and remainders in integer microseconds, and
`advance` computes the number due in one step. It pushes at most the
outbox's free slots and counts the rest as **coalesced**, per timer
(`Timers::coalesced`) and in its return value (`Advanced { delivered,
coalesced }`), which no longer counts a refused push as fired.
`Host::tick` advances straight into the application's outbox (the timers
are taken out for the call) and keeps the coalesced total
(`Host::timer_coalesced`).

| `tests/timers.rs` | before | after | bound |
|---|---|---|---|
| `advance(1e9)` at 16 ms | did not return (200 ms watchdog) | **0.002 ms**; fired 62,500,000 = 32 delivered + 62,499,968 coalesced; remaining 16.0 | < 200 ms; remaining in (0, 16] |
| 10 ms timer, `tick(1000)` | delivered 32, counted **0** | delivered 32, counted **68** | delivered + counted = 100 |
| one hour at 16 ms | fired 225,000 | fired 225,000, 224,968 coalesced for that timer | 225,000 |
| 10 ms timer, `tick(300)` | 30 of 30, 0 counted | the same | 30, 0 |
| 80 ms frame, 25 ms timer | 3 | 3 | 3 |

**Injections.** The `f32` `while` loop put back: `advance(1e9) at a 16 ms
period did not return within 200 ms: the catch-up loop is spinning
(R7-F3)`. The host's `timer_coalesced += …` removed: `fired 100,
delivered 32, counted 0` — `68 firings were neither delivered nor
counted anywhere a caller can read`. Restored, green.

**What a caller reads.** `Host::timer_coalesced` and `Timers::coalesced`;
the agent reports no host counters today, so nothing there changed
(`docs/plan-timers-wake.md` T47.2, "What building found").

## M49/T49.10 — work in flight crosses a window swap

**Landed 2026-09-27** (`docs/plan-reload-hardening.md` T49.10, R4-F3).
The old session saves its host's work in flight beside the kept state:
`hot::save`, under `hot::RESERVED`, as `[kind, step, payload]` integers.
The new build's `hot::boot` re-issues each entry through its own
`App::job_for` after the restore (`Host::reissuable`, `Host::reissue`).
The ledger it reads is M47/T47.3's, above.

### The gate, in `just window-reload-demo`

`fixtures/chain-app` is a two-step chain from payload 4. v1's step 0 is
`p + 1` and **blocks until the loader releases it**, so it is in flight
at the swap without a race. v2's step 0 is `p + 100`. Both builds' step 1
is `x * 10`. So `chain 1040` means v2 ran both steps, and `chain 50` means
v1's step 0 ran after the swap.

```
chain, v2 with no swap: `start | chain 1040`, in flight after 0
chain, v1 clicked: `start | chain -`, in flight before the swap 1 [[9, 0, 4]]
chain, swapped: reissued Reissued { spawned: 1, already: 0, unknown: 0, refused: 0 }
chain, after the swap and a drain: `start | chain 1040` (want `chain 1040`), in flight after 0
  ok: a chain in flight at the swap completed under v2's step function
```

The first line is the accepting direction: the same chain with no swap
reaches the same frame and leaves the ledger empty. The second is the
setup assertion: without a step 0 in flight, the swap would carry an
empty ledger whichever way the code went.

### Injections

| injection | fires as |
|---|---|
| `hot::boot` re-issues nothing (the behaviour before this task) | `reissued Reissued { spawned: 0, … }`; `chain did not complete after swap`, the frame still at `chain -` |
| the continuation held as the **old build's fn pointer**: `hot::save` writes v1's `job_for` answer as a fourth integer, and `hot::boot` spawns that pointer instead of asking its own table (T28.7's injection, now on the framework path) | `chain 50`; `the chain completed into the OLD build's step 0` |

The second injection is `docs/events-decision.md` rule 3's defect, run
through the framework's own inbox, spawner and ledger for the first time.

### The selection rules, in `tests/reissue.rs`

The live gate's ledger holds one job, so it cannot reach these rules.

| at the save | listed | carried |
|---|---|---|
| applied / result in the inbox / not started / settled since the last prune / a running subscription | `[7, 11, 13, 12]` (10 pruned) | **`[11, 12]`** |

| the new build, with its own boot job `(1, 0, 5)` in flight, given `(1,0,5)`, `(1,0,5)`, `(1,0,6)`, `(99,0,1)` | |
|---|---|
| `Reissued` | `spawned 2, already 1, unknown 1, refused 0` |
| in flight / `unknown_kind` / landed | 3 / 1 / `[5, 5, 6]` |

| injection | fires as |
|---|---|
| `Ledger::unsettled` does not filter settled entries | `a job that settled since the last prune was re-issued: [11, 13, 12]` |
| `reissuable` keeps subscription jobs | `a running subscription's job was re-issued: [7, 11, 12]` |
| `reissue` does not skip a job already in flight | `Reissued { spawned: 3, already: 0, … }` against `spawned: 2, already: 1` |
| an unknown descriptor is not counted in `unknown_kind` | `a descriptor this build has no job for is counted where drain_outbox counts it`: 0 against 1 |

Restored, all green.

### What it costs

Nothing per spawn or per result. `Host::spawn`, the drain and the
ledger's `record`/`prune` are unchanged. The added work runs once per
swap: a walk of the ledger at the save and one spawn per re-issued entry.
The plan's kill rule priced the ledger's per-result work, which is
T47.3's and is judged above. It could not fire here
(`docs/plan-reload-hardening.md` T49.10, "What building found").

**Delivery across a swap is at least once.** A thread cannot be stopped,
so the old build's worker finishes into an inbox nobody drains, and the
job runs again under the new build. In the gate, v1's worker is released
after the swap, and its late `p + 1` never reaches v2's frame.


## M84/T84.7 — a stopped subscription stops

`docs/plan-images-in-motion.md` T84.7. A subscription that is no longer
declared has its job's `JobTrack::stop` set by the host's diff; the body
returns on `Sender::is_stopped`, and the drain drops what it sent
(`Host::stopped_dropped`). M84/T84.5 had pinned the gap green: **20**
messages applied and **20** job iterations in the 200 ms after the stop.

| gate (`crates/fulgurite-shell/tests/`) | measured | bound |
|---|---:|---:|
| `subscriptions.rs` `a_stopped_subscription_stops`: messages applied after the stop | **0** | 0 |
| the same: job iterations after the stop | **0** | ≤ 1 |
| the same: waiting at the stop / dropped at the drain | 3 / 3 | dropped ≥ waiting |
| `a_declared_subscription_keeps_delivering` (accept) | **50 of 50** applied, 0 dropped | 50 |
| `a_rekeyed_subscription_delivers_only_the_new_key`: old key / new key after the change | **0** / 21 | 0 / > 0 |
| `reissue.rs` `a_stopped_subscription_is_not_carried_across_a_swap` | re-issued `[14]`, not the stopped `8` (since M88/T88.1 the stopped job also leaves the ledger at its stop's pump; the filter is gated in `crates/fulgurite/tests/ledger_stop.rs`) | — |

Injections, each red by name and restored green:

| injection | fires as |
|---|---|
| the drain ignores `stop` | `messages must not be applied after the stop -- 3 were`; re-key: `old key's messages were applied` (3) |
| the host never sets `stop` | `job must return within one iteration -- it ran 20` (23 messages applied) |
| `Ledger::unsettled` keeps stopped jobs | `a stopped subscription's job was re-issued: [8, 14]` (since M88/T88.1 it fires in `ledger_stop.rs`: `a stopped job was offered for re-issue: [8, 14]`) |
| T84.5's pinned test against the new code | `pinned (T84.5): ... 0 were` — its job never asks, so it still iterated 20 times: the drain's half alone holds for a body that does not cooperate |

**Waiting before the stop is what lets the drain's injection fire.** The
ticker sends every 10 ms and the stop's frame follows the first message
by about a millisecond, so without the test's 35 ms wait the inbox is
empty at the stop and a drain that ignored the flag passed. That
something was waiting is asserted as setup, from the iterations less
what landed, which does not read the mechanism under test.

**K3, `benches --bin spawn -- pool`, eight runs a side under
`just bench-lock`, alternated, all quiet:** parent 1,747.70–3,064.70 ns
(median 2,025), T84.7 median **2,505** ns (1,853.14–3,223.75). Inside the
parent's spread; the added work is one atomic load per drained result
from a tracked job.

**Progress after stop** (M11) is not here: M88/T88.1's progress slot has
not landed, so T88.1 carries that gate and makes `settled` treat a
stopped job as settled. Today a stopped job settles the ordinary way,
once its body returns and the drain has consumed what it sent.

## M88/T88.0 — baseline: a streaming job loses what it streams

`docs/plan-platform-requests.md` T88.0. Pinned green on today's code by
`crates/fulgurite-shell/tests/sipper.rs`: one job under `Immediate`
sends 10,000 `u64`s through `Sender::send` into a one-node host, pumped
until the ledger is empty. T88.1 flips it.

| quantity (M88 baseline) | measured | pinned |
|---|---:|---|
| sends refused | **9,744** of 10,000 | ≥ 9,744 (10,000 − `Inbox::CAPACITY`) |
| largest number applied in one pump | **64** | = `Inbox::PER_FRAME` |
| value 9,999 applied | **0** times — lost | 0 |
| accept: 200 sends — refused / applied / final value | 0 / 200 / applied once | 0 / 200 / 1 |

97% of the stream and its last value never reach the application.
Raising `Inbox::CAPACITY` to 16,384 (the plan's injection) turns the pin
red by name: `pinned (T88.0): the final value is lost today -- it was
applied 1 time(s)` (refused 0, applied 10,000).

The second pin is `scripts/resume_drive.py`'s GPU leg: `ui.screenshot`
answers `this host does not render — nothing to screenshot` there,
which is why that leg reads `xwd`. T88.2 flips it (the leg now compares
`ui.screenshot` with `xwd`; `docs/results-gpu.md`, "Readback").

## M88/T88.1 — `Sender::progress`: the sipper

`docs/plan-platform-requests.md` T88.1. A job reports a stream through
`Sender::progress`, which replaces the value in a one-value slot on its
`JobTrack`; the pump takes each fresh slot before it drains the channel.
`crates/fulgurite-shell/tests/sipper.rs` is T88.0's pin, flipped.

| gate | Immediate | Queued | `Threads::with_cap(2)` | bound |
|---|---:|---:|---:|---|
| values applied in one pump (max) | **1** | **1** | **1** | ≤ 1 |
| value 9,999 applied | **1** | **1** | **1** | exactly 1 |
| refused | **0** | **0** | **0** | 0 |
| applied + `Inbox::coalesced` | 1 + 9,999 | 1 + 9,999 | 3 + 9,997 (one run) | 10,000 |

Against the M88 baseline above: refused 9,744 → **0**, per-pump maximum
64 → **1**, the final value lost → applied once.

| other arm | measured | bound |
|---|---:|---|
| unsplit `Boundaries<256>` app, a sipper of 3: the frame's label | `"0"` → **`"3"`** (1 applied + 2 coalesced) | `"3"` |
| no pump for 1 s, two sippers of 10,000 on two workers: each body returns in | **~2–4 ms** | ≤ 50 ms |
| the same run's 120 ms timer: ticks beside the sippers / control without | **24 / 24** | ±1; control ≥ 20 |
| progress after stop: the stopped job's final value applied / `stopped_dropped` | **0** / +1 | 0 / 1 |
| accept: a job that `send`s 3 values | 3 applied, 0 refused | 3 |
| `size_of::<JobTrack<u64>>()` | **56 B** (24 before: the slot's `Mutex<Option<u64>>` is 24 B, plus `fresh` and the 8 B epoch), inline in the `Arc` `tracked_sender` already allocates | — |
| `just window-reload-demo` `sipper_across_a_swap`: v1 shows `sip 5`, swap, release | **`sip 10099`** (v2's), never `sip 9999` (v1's) | `sip 10099` |
| the same, accept: v2 with no swap | `sip 10099` | `sip 10099` |

Injections, each red by name and restored green:

| injection | fires as |
|---|---|
| T88.0's pinned assertions against the flipped body | `pinned (T88.0): the final value is lost today -- it was applied 1 time(s)` (refused 0, per-pump max 1) |
| `progress` is a `send` (no slot) | per-pump max **64**, value 9,999 applied **0**, refused **9,744** (Queued) / 9,680 (Threads); the unsplit and stop arms fire too |
| `progress` blocks while its slot is full (a bounded queue of one) | `Immediate` and `Queued`: `did not finish within 30 s -- a job body blocked` (the watchdog); no-drain arm: `sipper 0's body returned in never (still blocked at 3 s)`, ticks 0 beside the sippers against 24 |
| `settled` ignores `fresh` | `Immediate`: in flight before the first pump 0, `value 9999 must be applied exactly once -- it was applied 0 time(s)` |
| the slot drain does not OR into `applied_changed` | the unsplit arm only: label `"0" -> "0"`, `the slot's change never reached mark_all (the idle skip)`; every split arm stays green, which is why the unsplit arm exists |
| the slot drain ignores `stop` | `a stopped job's final progress value was applied` (stopped_dropped +0) |
| `hot::boot` skips `Host::reissue` | `sipper_across_a_swap`: reissued `spawned: 0`, label `sip -` (`Work` is fresh, v1's worker streams into a slot nobody reads) |
| `settled` without its stop term | `reissue.rs`: `a stopped job with nothing undrained is still listed (in flight)` (listed `[8, 14]`) |

**A stopped job counts as finished** once its results and slot are
drained (the M84/T84.7 half of the mechanism). So the pump that stops a
subscription also takes its job off the ledger, though its body has not
run; `reissue.rs`'s T84.7 test asserted the opposite as its setup and
now asserts this. `Ledger::unsettled`'s stop filter guards only a result
sent between a pump's drain and its prune, which no deterministic host
can stage, so `crates/fulgurite/tests/ledger_stop.rs` builds that
state on a ledger directly (listed `[8, 14]`, unsettled `[14]`; red under
T84.7's injection).

**K88a, `benches --bin spawn -- pool`, eight runs a side under
`just bench-lock`, alternated, all quiet:** parent 1,786.43–2,871.95 ns
(median 2,379.56), T88.1 median **2,370.17** ns (1,739.01–2,486.27).
Inside the parent's spread; not fired.

**K88b, the idle floor:** the `benches-competitive` frame arm's release
binary is **byte-identical** before and after T88.1 (`cmp`). Its
application defers nothing (`Msg` uninhabited), so the slot drain's
`Some(m)` is unreachable and the compiler removes it with the
`ledger.is_empty()` guard; no timing can separate identical code, so K88b
is not engaged at T88.1.

## M88/T88.3 — the request seam (S1) and screenshot requests

`docs/plan-platform-requests.md` T88.3. An application pushes a `Copy`
`request::Request` onto its `Requests` (capacity 8); the window drains
it through `Session::take_requests` in `redraw` after the
present-or-skip decision and after every agent call, and answers through
`Session::answer`, which converts with `App::answered` in the build
loaded then and applies inside the call. `Request` is 16 B (asserted).
Clipboard requests are answered `Refused(Unsupported)` until T88.4.

| gate | measured | bound |
|---|---|---|
| `tests/requests.rs`: a click's request taken, then taken again | 1, then 0 | 1, 0 |
| answer applied inside the call | `shots` 1 before any pump; the next pump shows `shot 1 2x2` | 1 |
| unsplit app, `Boundaries<256>`, `answer(7, Text("x"))` + one pump | label `0` → `x` | `x` |
| unmodelled tag | `unmodelled_answers` 1 | 1 |
| ninth push | 8 queued, 1 refused | 1 |
| clipboard write, `clip_for` → `None` | answered `Refused(Empty)` at once, 0 handed out | 0 |
| no `requests()` | `Vec::new()`, capacity 0 | 0 |
| `alloc_gate.rs`: pump + empty `take_requests`, 100 frames | 0.00 allocs, 0.00 frees per frame | 0 |
| winit `serve_requests`: screenshot / write / read | `image 2x2 @1 16` / `Refused(Unsupported)` / `Refused(Unsupported)`; a blind backend's screenshot `Refused(Unsupported)` | as listed |
| `just window-reload-demo`, `request_across_a_swap` | `v2:` → `v2:x`; accept, no swap: `v1:x` | `v2:x` |
| `answer_then_swap` | kept `asked` `v1:x` before the swap; after v2's first frame `v1:x`; accept: `v1:x` after one pump | `v1:x` |
| `just tasks-drive`, agent path (`#shot`), every leg | `shot 1 520x640 #ae60da`, this click's colour (previous `#6bddbd`) | this click's |
| redraw path (`#shot-timer`), software, both strategies and the oracle leg | `shot 3`, both answers tick 1's `#f1e3f7` (before: `#ae60da`); presented **3**, skipped **3** | exactly 3, ≥ 1 |
| redraw path, GPU leg (RTX 4070) | `shot 3`, both answers `#f1e3f7`; presents bound not checkable on the day (below) | |
| `just panic-drive`, site 11 (`App::answered`) | `fixtures/panic-app/src/lib.rs:326`, screen drawn, retried; `--expect-panics` 11 → **12** | |
| firmware `nm`, `request::|Requests|answered` | 0 in each of the four binaries; 13 in the `requests` test binary | 0; > 0 |

**Everything's sum is 6, not 4.** The plan predicted presented + skipped
= 4 under `Everything`; it is 3 + 3 on both strategies. Each tick is a
job: the timer's deadline frame spawns it and presents nothing, and its
result lands in a second frame, so the two ticks bring two spawn frames
beside the four the plan counted.

**The GPU leg's presents bound could not be checked on 2026-10-06**: the
reference machine's display was asleep (`xset q`: "Monitor is Off"), and
a FIFO present on a sleeping display blocks about a second, so the
`#shot-timer` click itself returned after tick 1 was due (855 ms against
the leg's 250 ms) and no read could land before it. The answers were
checked after a 6 s unobserved wait; the bound is printed, not asserted,
on that leg only, and the software legs carry it.

**The `nm` count cannot see an inlined push.** Its plan injection —
`Requests` ungated and pushed from the embedded fixture's handler —
left all four counts at 0, because `push` inlines into the handler and
leaves no symbol. The same injection moves `.text` by +136 / +132 / +128
/ +132 B (`just firmware-delta`), so the strict delta, 0 on all four, is
this task's firmware gate and the `nm` count is printed beside it.

**K88b at T88.3** is not engaged, as at T88.1: the frame arm's release
binary is byte-identical before and after (`cmp`). Its application has
no `requests()`, and `Host::answer` is never called, so the
`answer_changed` take folds away.

## M88/T88.4 — clipboard reach: text, HTML, primary

`docs/plan-platform-requests.md` T88.4. `Win::serve_requests` answers
`ClipboardWrite` and `ClipboardRead` from `fulgurite_winit::Clipboard`,
which gains `set_html`/`get_html` (arboard's ungated HTML) and
`set_primary`/`get_primary` (its Linux extension traits,
`LinuxClipboardKind::Primary`). A write answers `Written` or
`Refused(Failed)`; a read `Text`/`Html` or `Refused(Empty)`; what the build
or platform cannot do at all — no `clipboard` feature, a primary selection
off X11, HTML on primary — `Refused(Unsupported)`.

| gate | measured | bound |
|---|---|---|
| K2 crate count, `cargo tree -p fulgurite-winit -e normal --prefix none \| sed 's/ (\*)$//' \| sort -u \| wc -l` | **96 → 96** (`check_default_crates`: delta 0) | 0 new |
| `scripts/probe.sh xclip` on the reference machine | `probe: xclip present (/usr/bin/xclip, xclip version 0.13)` | present |
| refuse direction: the primary leg's preflight with `xclip` off `PATH` | exit **1**, `sudo apt install xclip` printed | non-zero, non-77 |
| `just tasks-drive`, every software leg (Everything, Boundaries, Boundaries + oracle): `#copy-text` / `#copy-primary` | `Written` / `Written` | `Written` |
| `#read-text` / `#read-primary` | `A` / `B` | `A` / `B` |
| independent `xclip -o -selection primary` | `B` | `B` |
| `#copy-html` then `#read-html` | 38 B written, 38 B read, byte-identical | equal |
| accept: a second `#copy-text` (`C`) | `#read-text` `C`; `#read-primary` and `xclip` still `B` | `B` |
| `tests/requests.rs`: an HTML clip | `Payload::Html`, html 25 B and alt 7 B, both halves | both |
| winit `serve_requests`, a closed clipboard | write `Refused(Failed)`, primary text read `Refused(Failed)`, primary HTML read `Refused(Unsupported)` | as listed |

Injections, each red by name and restored green:

| injection | fires as |
|---|---|
| T88.3's pin against the new `serve_requests` (both clipboard requests `Refused(Unsupported)`) | the winit test: `left: [.., (2, "Refused(Unsupported)"), (3, "Refused(Unsupported)"), ..]`, want `Refused(Failed)` |
| `serve_requests` ignores `kind` | `#read-text shows 'B' (want 'A')` — the primary write landed on the clipboard — `the clipboard and the primary selection are not two places` |
| `Host::take_requests` does not ask `clip_for` | `setup: #copy-text's write answered text 'Refused(Empty)', not Written` |
| `scripts/probe.sh`'s `xclip` case exits 77 | `with xclip off PATH the primary leg exited 77, a skip -- want non-77` (the drive stops before any leg) |
| `Payload::from` flattens an HTML clip to its alt text | `an HTML clip must reach the window as Payload::Html with both halves` |

**The clipboard phase writes the desktop's clipboard and primary
selection**, on whatever display the drive runs on; it runs on the three
software legs (the GPU leg runs only the pixel phases, T88.2).

**The GPU leg's presents bound was again not checkable** (the display
asleep, the `#shot-timer` click 854 ms against 250), as at T88.3; this
task does not touch that leg's phases.

## M88/T88.5 — `clipboard-image`

`docs/plan-platform-requests.md` T88.5. `ClipFormat::Image`, `Clip::Image`
and the shell's `Payload::Image` exist in every build; a window built with
`fulgurite-winit`'s default-off `clipboard-image` (`clipboard` plus
arboard's `image-data`) answers them through `Clipboard::set_image` and
`get_image` — a read as `Answer::Image` with scale 1 — and without it
answers both `Refused(Unsupported)`.

| gate | measured | bound |
|---|---|---|
| K2 crate count, default build | **96 → 96** (`check_default_crates`) | 0 |
| K2 crate count, `--features clipboard-image` | **108** (+12: `image`, `png`, `moxcms`, `pxfm`, `flate2`, `miniz_oxide`, `fdeflate`, `crc32fast`, `adler2`, `simd-adler32`, `byteorder-lite`, `num-traits`) | printed |
| stripped release `showcase` (GNU strip, as `docs/results-a11y-platform.md` measured `a11y`) | 10,106,856 B → 10,428,136 B: **+321,280 B** | 3 MB (default-off either way) |
| `just clipboard-image-symbols`: `image::` symbols, default showcase | **0** | 0 |
| the same, built with `fulgurite-winit/clipboard-image` (positive control) | **168** | > 0 |
| K88d, `cargo deny --offline --features fulgurite-winit/clipboard-image check advisories` | ok (`image@0.25.10`, `png@0.18.1` in the graph) | passes |
| `just tasks-drive`, the `clipboard-image` leg: `#copy-image` | `Written` | `Written` |
| `#read-image` | **`8x4 3a67c4c731ebb3a5`**; written `8x4 3a67c4c731ebb3a5` | equal |
| accept, every software leg (built without the feature): `#copy-image` / `#read-image` | `Refused(Unsupported)` / `Refused(Unsupported)` | not `Empty` |
| `tests/requests.rs`: an image clip | `Payload::Image` 8×4, 128 B | its shape |
| winit `serve_requests`, closed clipboard: an image read | `Refused(Failed)` with the feature, `Refused(Unsupported)` without | as listed |

The symbol pattern is anchored at the crate root: the default showcase
holds 26 `fulgurite_shell::image::` symbols (the workspace's own image
store), which an unanchored `image::` would have counted.

Injections, each red by name and restored green:

| injection | fires as |
|---|---|
| `clipboard-image` a default feature | `image:: symbols in the default showcase: 168 / bound 0` — `a build without clipboard-image links the image crate`; `check_default_crates` too: 108, delta 12 |
| width and height swapped into arboard's `ImageData` | `#read-image: '4x8 3a67c4c731ebb3a5'` — `the image read back as 4x8, written 8x4 -- width and height swapped on the way` |
| an unsupported image read asked of the clipboard anyway | software leg: `#read-image 'Refused(Empty)'` — `want Refused(Unsupported) for both` |
| `Payload::from` swaps an image clip's width and height | `an image clip must keep its width, height and bytes` |
| an unused binding in `set_image`'s feature-gated body | the new `just gpu` line `clippy --features clipboard-image` fails (`unused variable: unused_probe`); the default winit clippy stays green |
