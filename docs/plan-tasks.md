# M28 — deferred work: tasks that run off the frame, and subscriptions that outlive it

> **Not started. Written 2026-09-18.**
>
> The ask: *"investigate how we could implement the equivalent of Task
> and subscription in iced — tasks run async, spawned one at a time,
> batched (run concurrently) or chained (run sequentially)."*
>
> **The seam already exists and the rule is already decided.**
> `crates/lumen-static/src/inbox.rs` shipped as plan-events E3: a
> host-owned receiver, a cloneable sender that moves into a worker, and
> a per-frame drain through a `fn` pointer taken from the *currently
> loaded* code. `docs/events-decision.md` states the constraint it
> serves — **work in flight may hold no code reference** — and
> `docs/plan-agent-parity.md`'s appendix already answered where a
> spawner lives: behind `dyn` at the shell, never in a widget field,
> never a new type parameter.
>
> **What is missing is everything on top**: a runtime, the three
> combinators, cancellation, subscription identity, and a way to wake an
> idle window. This plan builds those and nothing else.
>
> **One of iced's three combinators cannot be ported literally**, and §2
> is mostly about that: chaining holds a closure across an await, which
> is the one thing the boundary rule forbids.

*Each task is one commit unless it says otherwise; each ends with
`just test` green and the arm named in its **Measure** line re-run. Task
ids go in commit subjects. Every gate names the injection it must go red
under, and prints its measured quantity beside its bound.*

---

## 0. The rules for this milestone

1. **Measure before and after, on the arm the task names.** Eight runs,
   classified into the fast and slow clusters before any spread is
   computed — the mode rule `docs/results-frame-and-types.md` records,
   for the reason recorded there.
2. **A figure is cited for the case it was measured on**, never carried
   to another.
3. **Guard the budgets no frame number shows**: the allocation gate
   (`crates/lumen-static-shell/tests/alloc_gate.rs`), `.text` on
   thumbv7em, the const-asserted struct sizes, and **crate count** —
   this workspace prices dependencies (clipboard 11 crates, a11y 62,
   rfd 79, gpu 37, `docs/plan-capabilities.md`), and an async runtime is
   the largest thing anyone has proposed adding.
4. **Docs move in the same commit**, including the decision record a task
   amends. Two are load-bearing here and neither may be contradicted
   silently: `docs/events-decision.md` and `docs/plan-agent-parity.md`'s
   appendix.

## 1. What iced does, and what this takes from it

| iced | here |
|---|---|
| `Task<Message>` — a future producing messages, returned from `update` | a **descriptor**: kind, step, payload. Data, not a future |
| `Task::perform(fut, Message::Loaded)` | one descriptor spawned |
| `Task::batch([a, b])` — concurrent | N descriptors spawned together |
| `task.then(\|out\| next)` — sequential | the step field: a completion says which step is next |
| `Subscription<Message>`, identity by hashing the recipe | identity as data: a kind and a key, diffed per frame |
| a runtime the framework owns | a spawner behind `dyn` at the shell, defaulting to threads |

**What is kept:** the shape an application writes — spawn one, run
several at once, run one after another, subscribe to something
long-lived. **What changes:** every one of those is described by values
rather than by closures, for the reason §2.1 gives.

## 2. The decisions

### 2.1 Why a chain cannot be a closure

`docs/events-decision.md` rule 3: *"Anything in flight across a frame or
thread boundary — async completions, timers, input queued during a swap
— returns as data… No code reference may be held by in-flight work."*
The reason is hot reload: `fresh()` rebuilds the tree but cannot reach
into a pending future to swap the pointer it captured, so work in flight
would complete into the **old dylib's** logic — safe only because
nothing is ever `dlclose`d, still last week's code.

iced's `.then(|out| next_task)` holds a closure across an await. Ported
literally it would put a code pointer in flight and break the rule on
the first reload.

**The way out is the move this architecture already made once.** Handlers
stopped needing captures because identity became *positional*: the save
button's handler does not know which button it is, it *is* the save
button at a fixed field path. A chain stops needing a closure the same
way — **the next step is an index, not a function**:

- a descriptor is `(kind, step, payload)`;
- the application supplies one step function, reached from the currently
  loaded code, which maps a descriptor to what happens next;
- what happens next is itself data: finish, run another descriptor, or
  spawn several.

So **single** is one descriptor, **batch** is several spawned together,
and **chain** is a completion that names the next step. A reload lands
mid-chain on the new build's step function, which is the outcome the rule
exists to produce.

### 2.2 Where the runtime lives

`docs/plan-agent-parity.md` appendix #2 settled this before it was built:
a spawner is `dyn` at the **shell**, never in a widget field (a handle is
code, and the `no_std` leaf profile must not carry one) and never a new
type parameter (§8's single type-axis budget is reserved for the arena).
This plan does not reopen it.

**Three implementations, and the second is why the gates stay honest:**

| spawner | for |
|---|---|
| threads | the default: `std::thread` per task, no dependency, what E3's "threads now" meant |
| immediate | tests: runs the job synchronously at spawn, so a headless gate is byte-identical and deterministic — the same pairing `Rebuild` gives `Incremental` |
| a real async runtime | opt-in behind a feature, and only if something needs it. §0.3's crate-count budget applies |

### 2.3 Cancellation, and what a reload does to work in flight

Every spawn takes an **epoch**; a result carrying a stale one is dropped
at the drain rather than applied. That is cancellation, and it is also
the answer to "what happens to a task in flight across a dylib swap":
the host bumps the epoch, and pre-swap results are discarded instead of
being handed to code that may no longer understand them.

**Stated as a default, not left to chance.** An application that wants
the other behaviour keeps the epoch; the point is that one of the two is
written down.

### 2.4 Subscriptions are diffed by data, not by a hash of code

iced hashes a recipe to decide which streams keep running. Here identity
is `(kind, key)` — values — and the host keeps a **fixed-capacity** set,
diffing it each frame: start what appeared, stop what vanished, leave the
rest alone. Fixed capacity that refuses rather than fails is this
workspace's pattern for exactly this shape (`InputQueue<64>`,
`Overlays<16>`, `Boundaries<N>`).

## T28.0 — baselines, and the arms that do not exist

No behaviour change. Three things are unmeasured today and every later
task is judged against them:

| arm | what it must newly cover |
|---|---|
| the allocation gate | whether draining an inbox inside `Host::pump` allocates or frees on the frame thread. `mpsc::try_recv` drops heap nodes, and the gate's zero is a *claim about `pump`* |
| `benches --bin crossover` | a drain row: a frame that applies N results, against a frame that applies none |
| a wake-path measurement | how long a completion takes to reach a frame on an idle window |

**Gate — a new arm must be able to say no.** Each is shown to move under
its own injection: post results and the drain row rises; make the drain a
no-op and it falls to the empty-frame cost. **An arm whose number does
not move is a finding about the arm**, fixed before a baseline is taken
from it — two of M26/T26.0's six were, which is why this is written here
too.

**Thresholds**, owned by `docs/results-tasks.md`: the allocation gate
stays at zero for a frame that drains; no frame arm regresses beyond its
fast cluster's spread; `.text` and the const-asserted sizes unchanged for
a build that uses none of this; the crate count unchanged by default.

## T28.1 — an idle window wakes when work completes

`ControlFlow::Wait` means an idle window sleeps until input, and the
user-event loop that could wake it is built **only under the `agent`
feature** — so today a completion on an idle window would sit there
until the user moved the mouse.

Generalise the loop's user event beyond that feature: the agent's request
becomes one variant, a wake becomes another.

**Gate:** a live window with no agent feature, idle; a task completes;
the frame that applies it is presented without any input arriving.
*Injection:* drop the wake and the frame does not arrive until an event
does — the test times out by name rather than hanging.

**Measure:** the wake-path arm from T28.0.

**Docs:** `crates/lumen-static-winit/src/agent.rs`'s thread rule, which
this widens from "the socket thread" to "any thread".

## T28.2 — the host owns the inbox and drains it each frame

`Inbox<M>` exists and nothing owns one: the host has no field, and an app
that held its own could not drain it — `inbox.drain(&mut self, …)` cannot
borrow the app twice. Host-owned is the shape the decision doc already
names, and it is the one that borrows.

The drain runs where `App::update` already runs, so a result applied this
frame reaches layout this frame rather than next.

**Gate:** a result posted between frames is applied by the next pump, in
order, exactly once; draining twice yields nothing. On `Boundaries<16>`,
because a result that changes state must mark it — an unmarked change is
the stale-frame shape M25/T25.0 and M26/T26.1 both were.
*Injection:* apply without marking — the `Boundaries` case keeps last
frame's geometry while `Everything` stays green.

**Measure:** the drain row and the allocation gate from T28.0. **If the
drain allocates on the frame thread**, the inbox becomes a
fixed-capacity ring like `InputQueue` before anything is built on it —
the zero-allocation claim is gated and older than this plan.

## T28.3 — the spawner seam, and the deterministic one

A spawner behind `dyn` on the host (§2.2), with the thread-backed default
and the immediate implementation tests use.

**Gate, both directions:** under the immediate spawner a task's result is
applied by the pump that follows the spawn, deterministically, in a
headless test; under the thread spawner the same task arrives within a
bounded number of pumps. *Injection:* the immediate spawner defers
instead of running — the determinism assertion fires by name.

**Docs:** `docs/plan-agent-parity.md` appendix #2 — this implements what
it specified; the plan says so rather than restating it.

## T28.4 — one task, and a batch

A descriptor spawned; a batch spawned together. No chaining yet: this
task is the payload path and the ordering guarantee.

**Gate:** one task's result reaches the app; a batch of three all reach
it, and the frame that applies them is not the frame that spawned them
(or the immediate spawner would hide the seam). Results are applied in
arrival order, which is the only order there is — **a batch makes no
promise about completion order**, and the gate asserts that it does not
by spawning three with deliberately different durations under the thread
spawner and accepting any order.
*Injection:* drop one of the batch — the count assertion fires.

**Measure:** the drain row, N = 1 and N = 3.

## T28.5 — chaining, as a step index

§2.1's shape: a completion names the next step, the currently loaded code
interprets it.

**Gate — and this is the milestone's centre.** A chain in flight across a
**dylib swap** completes into the *new* build's step function.
`fixtures/hot-app` and `fixtures/hot-host` exist for exactly this kind of
question and already drive a swap with state in flight (`just hot-demo`).
The v1 step returns one value, the v2 step another; the chain is started
before the swap and completes after it, and the assertion is that the
value is v2's.
*Injection:* hold the next step as a function pointer captured at spawn
rather than as an index — the assertion fires with v1's value, which is
the defect the boundary rule exists to prevent, demonstrated.

**Measure:** `.text` on the embedded profile, which must not move: a
build that spawns nothing links none of this.

**Docs:** `docs/events-decision.md` gains the chain case by name — its
rule 3 anticipated it and no code had tested it until here.

## T28.6 — cancellation by epoch

§2.3. A stale result is dropped at the drain; a reload bumps the epoch.

**Gate:** a cancelled task's result never reaches the app, and an
uncancelled one still does — both directions, because a gate that drops
everything passes the first half. *Injection:* compare epochs with `>=`
instead of equality — the cancelled result arrives and the assertion
names it.

## T28.7 — subscriptions: start, stop, and identity

§2.4. The application declares what should be running; the host diffs by
`(kind, key)` against what is running and starts or stops the difference.

**Gate:** a subscription that appears is started once and not restarted
on the frames that follow; one that vanishes is stopped; one whose key
changes is stopped and a new one started. The set refuses past its
capacity rather than dropping silently, and the refusal is counted the
way `InputQueue`'s dropped events are.
*Injection:* diff by kind alone, ignoring the key — the "key changed"
case fires, because the old stream keeps running.

**Measure:** the per-frame cost of the diff on a tree with no
subscriptions at all, which must be indistinguishable from today's frame
— an application that subscribes to nothing pays nothing.

## T28.8 — a timer, without a thread per tick

The `time::every` analogue: the frame loop already has `App::tick` and a
`dt`. A timer subscription is a deadline checked on the loop, not a
thread that sleeps.

**Gate:** a 50 ms timer fires about twenty times in a second of driven
frames, and **zero** threads are spawned for it — asserted, because "it
works" and "it works without a thread each" are different claims.
*Injection:* spawn a thread per tick — the thread-count assertion fires.

## T28.9 — use the thing

`AGENT.md`, after M24: defects hide in machinery that was tested pass by
pass and never assembled. An example that spawns, batches, chains,
cancels and subscribes, driven through `just agent-drive` against a live
window, under both invalidation strategies.

**Gate:** the drive script asserts each action landed *and* its
consequence, with the quantity printed beside its bound.

## 3. What this is not

**Not async in the core.** Nothing in `crates/lumen-static` awaits; the
core stays `no_std` and the inbox stays `std`-gated. A leaf profile links
none of this.

**Not a runtime this workspace owns.** The spawner is a seam. The default
is threads because threads need no dependency.

**Not `Box<dyn Fn>` anywhere.** The widget-field ban stands
(`docs/events-decision.md`), and §2.1 extends the same reasoning to work
in flight.

**Not a message bus.** The inbox carries results of work the application
started, not a general event channel. Input has a queue already, and it
is the one the agent replays through.

## 4. Deferred, with conditions

- **A real async runtime behind the spawner seam.** When something needs
  concurrency the thread-backed default cannot express — many small
  overlapping I/O tasks, cancellation mid-await. §0.3's crate budget is
  the bar it has to clear.
- **Structured concurrency** (a batch that is cancelled as a unit).
  When an application has a screen whose work must all stop at once.
- **Streaming results** — a task that yields many values rather than one.
  A subscription is the shape for that today; a task that streams would
  need back-pressure this plan does not design.
- **Replaying task results in a session trace.** Sessions record inputs,
  deliberately (`docs/events-decision.md`, "what is given up, priced").
  A recorded task result is a different kind of input and needs its own
  decision.
