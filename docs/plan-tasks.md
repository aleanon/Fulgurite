# M28 — deferred work: tasks that run off the frame, and subscriptions that outlive it

> **Not started. Written 2026-09-18; revised the same day after a
> seven-lens review** (facts, design soundness, gates and injections,
> reload and serialisation, constraints and budgets, ADRs and docs,
> omissions and risk), each lens followed by a verifier told to refute:
> **42 findings, 13 blocking confirmed.** §5 records what it found and
> what changed. **Read it before §2** — the first draft could not be
> built as written, and the plan keeps the corrections rather than
> quietly replacing itself.
>
> The ask: *"investigate how we could implement the equivalent of Task
> and subscription in iced — tasks run async, spawned one at a time,
> batched (run concurrently) or chained (run sequentially)."*
>
> **The seam exists and the rule is decided.**
> `crates/lumen-static/src/inbox.rs` shipped as plan-events E3: a
> host-owned receiver, a cloneable sender that moves into a worker, and a
> per-frame drain through a `fn` pointer taken from the *currently
> loaded* code. `docs/events-decision.md` states the constraint — **work
> in flight may hold no code reference** — and
> `docs/plan-agent-parity.md`'s appendix already put a spawner behind
> `dyn` at the shell.
>
> **The three things the review found missing were not the runtime.**
> Nothing could *spawn* (handlers reach the app, never the host); no
> message type reached the host (`App` declares none); and a reload
> bumping the epoch made the cross-swap gate unpassable. §2.3, §2.5 and
> §2.6 are the answers, and they are why this plan is longer than the
> first draft.

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
3. **Guard the budgets no frame number shows**: the allocation gate, the
   `.text` ceiling, the const-asserted struct sizes, and **crate count**.
   By `cargo tree` on `lumen-static-winit`, clipboard costs +5 crates,
   dialogs +24 and the GPU backend +37 (`docs/results-gpu.md` §5, which
   also records that `docs/plan-capabilities.md`'s +11 and +79 were
   counted another way and are not commensurable with these). **No
   figure here says what an async runtime would cost**, because nobody
   has measured one; that is why the default spawner takes no dependency
   at all.
4. **Docs move in the same commit.** Three are load-bearing and none may
   be contradicted silently: `docs/events-decision.md`,
   `docs/plan-agent-parity.md`'s appendix, and
   **`docs/00-handoff-static-tree.md` §7**, whose REFINED paragraph is
   the text this milestone makes concrete.

## 1. What iced does, and what this takes from it

| iced | here |
|---|---|
| `Task<Message>` — a future producing messages, returned from `update` | a **descriptor**: kind, step, payload. Data, not a future |
| `Task::perform(fut, Message::Loaded)` | one descriptor, pushed to the outbox (§2.5) |
| `Task::batch([a, b])` — concurrent | several descriptors pushed together |
| `task.then(\|out\| next)` — sequential | the step field: a completion says which step is next |
| `Subscription<Message>`, identity by hashing the recipe | identity as data: a kind and a key, diffed per frame |
| a runtime the framework owns | a spawner behind `dyn` at the shell, defaulting to threads |

**What is kept:** the shape an application writes. **What changes:**
every one of those is described by values rather than by closures, for
the reason §2.1 gives.

## 2. The decisions

### 2.1 Why a chain cannot be a closure, and what a descriptor may contain

`docs/events-decision.md` rule 3: *"Anything in flight across a frame or
thread boundary — async completions, timers, input queued during a swap
— returns as data… No code reference may be held by in-flight work."*
`fresh()` rebuilds the tree but cannot reach into a pending future to
swap the pointer it captured, so work in flight would complete into the
**old dylib's** logic — safe only because nothing is ever `dlclose`d,
still last week's code.

iced's `.then(|out| next_task)` holds a closure across an await. Ported
literally it puts a code pointer in flight and breaks the rule on the
first reload.

**The way out is the move this architecture already made once.** Handlers
stopped needing captures because identity became *positional*. A chain
stops needing a closure the same way — **the next step is an index, not
a function**:

- a descriptor is `(kind, step, payload)`;
- the application supplies one step function, reached from the currently
  loaded code, which maps a descriptor to what happens next;
- what happens next is itself data: finish, run another descriptor, or
  spawn several.

**The boundary has two halves, and the review found the plan applying
only one.** Rule 3 forbids a code reference. `docs/step4-hot-reload.md`'s
honesty list forbids something else: a framework type crossing the dylib
seam, because v1 and v2 each link their own copy and no layout agreement
is promised. A descriptor that crosses a *swap* therefore crosses as
primitives or as serialised bytes, the way `hot_save`/`hot_restore`
already do — which the fixture's own verbs model
(`hot_check_mark(p) -> u32`). Within one process, between threads, it is
an ordinary Rust value and neither half applies.

**Kind and step ids are stable identifiers**, append-only, and a
descriptor whose `(kind, step)` the currently loaded build does not
recognise is **dropped at the drain with a counted fallback** — exactly
what the reload model already does for an enum variant the new build
removed (`fixtures/hot-host` asserts that fallback by name). Without this
rule a renumbering silently runs the wrong step.

### 2.2 Where the runtime lives, and what a job is

`docs/plan-agent-parity.md` appendix #2 settled the location: a spawner is
`dyn` at the **shell**, never in a widget field, never a new type
parameter. This plan does not reopen it.

**What a job is, which the first draft never said.** A job body is code
by necessity, and the only form that is not a code reference *in flight*
is a non-capturing `fn` pointer supplied at spawn from the currently
loaded build: `fn(Descriptor, &Sender<M>)`. Not a boxed closure — §3
bans those, and an `fn` parameter refuses a capturing closure at the call
site by type, which is the same guarantee the handler fields already
have.

| spawner | for |
|---|---|
| threads | the default: one `std::thread` per job, no dependency. **A pool since M47/T47.3** (R7-F5): at most `Threads::cap` long-lived workers, started lazily, fed through a channel (`docs/results-tasks.md` T47.3) |
| immediate | runs the job synchronously at spawn — for tests that do not care when |
| queued | records the job and runs nothing until the test says so. **Deterministic *and* deferred**, which is the combination a test needs to assert "not the frame that spawned it" without a sleep |
| a real async runtime | opt-in behind a feature, if something ever needs it. §0.3's crate budget applies, and no figure exists for it yet |

The third row exists because the review found T28.5 asking for a
property neither of the first two can give deterministically.

### 2.3 The epoch cancels; a reload does not

**A reload does *not* bump the epoch.** The first draft said it did, and
that made the cross-swap chain — this milestone's centre — unpassable by
construction: the pre-swap result would be discarded before the new
build's step could interpret it. It also voided §2.1's own argument,
which is precisely that a result is *data* and safe for the new build to
read.

So: **the epoch is explicit cancellation, driven by the application.**
Every spawn takes one; a result carrying a stale epoch is dropped at the
drain. A reload leaves it alone.

**What "cancel" honestly means here:** the result is dropped, the work is
not stopped. A thread already running runs to completion and its output
is discarded. Stopping work needs cooperation from the job body, which
this plan does not design — §4.

### 2.4 Subscriptions are diffed by data, not by a hash of code

iced hashes a recipe to decide which streams keep running. Here identity
is `(kind, key)` — values — and the host keeps a **fixed-capacity** set,
diffing it each frame: start what appeared, stop what vanished, leave the
rest alone.

**Fixed capacity whose overflow is visible** is this workspace's pattern,
and it has three flavours, which the first draft flattened into one:
`Overlays<16>` refuses the new item and counts `refused`; `InputQueue<64>`
rejects the newest event and counts `dropped`; `Boundaries<N>` does
something different — it *widens to everything*, which is conservative
rather than lossy. A subscription set takes the first shape: refuse, and
count the refusal where a test can read it.

**A subscription's output re-enters through the inbox**, like every other
deferred result. `docs/events-decision.md`'s last binding rule — *"the
inbox is the only legal way for deferred work to re-enter the app"* — is
absolute, and this plan adds no second path.

### 2.5 How an application asks for work — the outbox

**The first draft had no answer, and the review's first blocking finding
was that nothing could spawn a task at all.** Every application-authored
code path is `fn(&mut S, &EventCtx)`: a handler reaches the app and
nothing else. It has no host, no spawner, and it cannot be given one —
a spawner handle in the app would be a code reference in saved state,
which `#[reload(keep)]` already refuses to compile.

So the request is **data**, and it travels the way input already does:

- the application owns a fixed-capacity **outbox** of descriptors;
- a handler pushes a descriptor into it — a value, no code;
- the host drains it after dispatch and hands each descriptor to the
  spawner, which pairs it with the job `fn` from the currently loaded
  build.

Input has a queue into the frame; this is the queue out of it. Overflow
refuses and counts, as §2.4 requires.

### 2.6 The message type

`Host<A: App, R, I>` is generic over the app alone, and `App` declares
`Tree`, `tree`, `tree_mut`, `update` and `tick` — **no message type**.
The first draft gave the host an inbox of `M` with no `M` in scope.

`App` gains an associated message type and an apply method. The derive
emits an uninhabited default and an empty apply, so the ~27 derived apps
and the 13 hand-written impls that defer nothing are unchanged and do not
pay for the field.

## T28.0 — baselines, and the arms that do not exist

No behaviour change. Three quantities are unmeasured and every later task
is judged against them.

| arm | what it must newly cover | the injection that must move it |
|---|---|---|
| the allocation gate | whether draining inside `Host::pump` allocates **or frees** on the frame thread. **The gate counts allocations only today** — its `dealloc` forwards to the system and counts nothing — so it gains a free counter, and the frame prints `allocs` and `frees` beside the zero bound | post a result from a worker and drain it: with the counter added, a `try_recv` that frees an mpsc node moves `frees` off zero |
| `benches --bin crossover` | a drain row: a frame applying N results against one applying none | make the drain a no-op — the row falls to the empty-frame cost |
| the wake path | how long a completion takes to reach a frame on an idle window | drop the wake — the measurement does not terminate within its printed bound |

**An arm whose number does not move under its own injection is a finding
about the arm**, fixed before a baseline is taken from it — two of
M26/T26.0's six were.

**Thresholds** are owned by `docs/results-tasks.md`.

## T28.1 — an idle window wakes when work completes

`ControlFlow::Wait` means an idle window sleeps until input, and the
user-event loop that could wake it is built **only under the `agent`
feature**. A completion on an idle window would sit there.

Generalise the loop's user event beyond that feature: the agent's request
becomes one variant, a wake another.

**Gate, split along the seam this repository already uses**, because a
windowed assertion cannot run in `cargo test` (no display in CI, no
per-test timeout, and the workspace keeps its every-commit path
windowless):

- **the deterministic half, in `cargo test`:** posting to the inbox asks
  the proxy for a wake — asserted against a fake proxy, with no window.
  *Injection:* drop the request — the assertion fires by name.
- **the live half, display-guarded**, in the shape `scripts/live_gate.sh`
  already uses: a fixture binary that owns the window and asserts
  **itself** — the redraw posts to a channel, `main` waits with a bounded
  receive and exits non-zero on timeout, printing the elapsed time beside
  its bound. Skips with exit 0 where there is no display, as the other
  live gates do.

**Measure:** the wake-path arm.

**Docs:** `crates/lumen-static-winit/src/agent.rs`'s thread rule, which
this widens from "the socket thread" to "any thread".

## T28.2 — the host owns the inbox, and the app declares its message

§2.6's associated type and apply method; the host's field; the drain in
`pump` where `App::update` already runs.

**The queue is bounded.** `Inbox::new` is `mpsc::channel()` — unbounded —
and `drain` loops until empty, which is unbounded work inside `pump`. A
producer that outruns the frame loop grows it without limit. It becomes a
bounded channel whose sender refuses rather than blocks, counting
refusals where a test can read them, and the drain takes at most a stated
number per frame.

**Gate:** a result posted between frames is applied by the next pump, in
order, exactly once; draining twice yields nothing; a producer that
overruns the capacity has its refusals counted rather than blocking the
worker. On `Boundaries<16>`, because a result that changes state must
mark it.
*Injection:* apply without marking — the `Boundaries` case keeps last
frame's geometry while `Everything` stays green. **The fixture's applied
field must carry no `#[bind]`**: `pump` ends with `if self.app.update()
{ mark_all() }`, so a bound field would mark everything and the injection
would stay green — the vacuous shape this repository keeps finding.

**Measure:** the drain row and the allocation gate.

## T28.3 — the spawner seam, and the three implementations

§2.2: `dyn` on the host, a job as a non-capturing `fn` pointer, and the
threads / immediate / queued implementations.

**Gate, all three:** under *immediate* a result is applied by the pump
that follows the spawn; under *queued* nothing runs until the test says
so, and then it is applied by the next pump — deterministically, with no
sleep; under *threads* the same job arrives within a bounded number of
pumps, the bound printed beside the count.
*Injection:* the queued spawner runs at spawn instead of on demand — the
"not the frame that spawned it" assertion fires.

**Docs:** `docs/plan-agent-parity.md` appendix #2 — this implements what
it specified.

## T28.4 — the outbox: how an application asks for work

§2.5. A fixed-capacity queue of descriptors on the app, pushed by
handlers, drained by the host after dispatch.

**Gate:** a handler that pushes a descriptor causes exactly one spawn,
on the frame after the click; a full outbox refuses and counts, and the
count is readable. *Injection:* drain the outbox before dispatch instead
of after — the descriptor pushed by this frame's handler is spawned a
frame late, and the "on the frame after the click" assertion fires.

**Docs:** `docs/00-handoff-static-tree.md` §7 — the outbox is the shape
its REFINED paragraph anticipated, and the section gains the pointer.

## T28.5 — one task, and a batch

A descriptor spawned; a batch spawned together.

**Gate:** one task's result reaches the app; three spawned together all
reach it, under the *queued* spawner so the assertion is deterministic.
Order is arrival order and **a batch promises nothing about completion
order** — the gate says so by running the three under the thread spawner
too and accepting any permutation, while asserting all three arrived.
*Injection:* drop one of the batch — the count assertion fires.

**Measure:** the drain row, N = 1 and N = 3.

## T28.6 — chaining, as a step index

§2.1's shape, in one process: a completion names the next step, and the
currently loaded code interprets it.

**Gate:** a three-step chain runs in order, each step seeing the previous
step's payload, under the queued spawner. An unrecognised `(kind, step)`
is dropped and counted rather than misinterpreted — §2.1's stability
rule, asserted.
*Injection:* ignore the step field and always run step 0 — the chain
loops and the order assertion fires.

## T28.7 — a chain across a dylib swap

**This is the milestone's centre, and the fixture work is most of the
task.** The first draft claimed `fixtures/hot-app` and `fixtures/hot-host`
"already drive a swap with state in flight". They do not: the sequence is
strictly sequential — save, drop, load, fresh, restore — with no thread,
no channel, no `Host`, no pump and no inbox, and step two destroys the
entire app object while the old library is still loaded.

**So the task builds what the gate needs**, and the arrangement is forced
by §7's sequence: **the inbox, the epoch and the pending descriptor live
in `hot-host`** — the loader process, outside both dylibs — and the
descriptor crosses the seam as primitives, the way every other verb in
that fixture already does. The dylib exposes a step verb; the host starts
a chain against v1, performs the swap, and delivers the pending
completion to **v2's** step function.

**Gate:** v1's step returns one value and v2's another; the chain starts
before the swap and completes after it; the assertion is that the value
is v2's — that is, the new build interpreted the old build's in-flight
work. Run by `just hot-demo` beside the existing assertions.
*Injection:* hold the next step as a function pointer captured at spawn
rather than as an index — the assertion fires with v1's value, which is
the defect rule 3 exists to prevent, demonstrated rather than argued.

**Measure:** not `.text` on thumbv7em — the inbox is behind the `std`
feature and the embedded fixture does not enable it, so that number
cannot move and would be a bound computed from the wrong quantity.
Instead: `nm` on the linked firmware finds **zero** task symbols, printed
beside the count the way `scripts/check_text.sh` prints its Toggle count.

**Docs:** `docs/events-decision.md` gains the chain case by name — its
rule 3 anticipated it and nothing had tested it; `docs/step4-hot-reload.md`
gains the descriptor's crossing shape.

## T28.8 — cancellation by epoch

§2.3. A stale result is dropped at the drain; a reload does **not** bump
the epoch.

**Gate, both directions:** a cancelled task's result never reaches the
app, an uncancelled one still does, and a result that survives a reload
is applied by the new build (which is T28.7's assertion seen from this
side). *Injection:* compare epochs with an ordering test instead of
equality — the cancelled result arrives and the assertion names it.

## T28.9 — what happens when a job fails

The plan had no failure story and the review was right that it needs one
before anything is built on top: under the thread default a panicking job
unwinds, its sender drops, and a chain waiting on that completion stalls
forever with nothing observing it.

A job that panics, and a spawn that cannot start a thread, both produce a
**failure result** the application applies like any other — data, not a
callback.

**Gate:** a panicking job delivers a failure to the app rather than
killing the process or stalling the chain; a spawn refused by the OS does
the same. *Injection:* let the panic unwind unobserved — the "the chain
did not stall" assertion fires by timing out against its printed bound.

## T28.10 — subscriptions: start, stop, and identity

§2.4. The application declares what should run; the host diffs by
`(kind, key)` against what is running.

**Gate:** a subscription that appears starts once and is not restarted on
the frames that follow; one that vanishes stops; one whose key changes is
stopped and a new one started. The set refuses past capacity and counts.
*Injection:* diff by kind alone, ignoring the key — the "key changed"
case fires, because the old stream keeps running.

**Measure:** the per-frame cost of the diff with no subscriptions
declared, which must be indistinguishable from today's frame.

## T28.11 — timers, and the deadline the loop waits on

A timer is a deadline, not a thread that sleeps — but **a deadline needs
a frame to be checked on, and an idle window produces none**: the loop is
`ControlFlow::Wait` and asks for a redraw only while something is
animating. The first draft gated this on "a second of driven frames",
which is the one situation where the defect cannot appear.

So the task owns the scheduling: the host publishes its earliest pending
deadline, and the loop waits *until* that instant rather than
indefinitely, falling back to today's behaviour when nothing is pending.

**Gate, two halves:**
- **deterministic:** drive 40 frames of a 25 ms tick — exactly one
  synthetic second — and assert a 50 ms timer fired **exactly 20** times,
  printing the count beside 20 and the accumulated time beside 1000.0, so
  a setup that silently did not tick is visible.
- **the mechanism:** the host's earliest deadline is what the loop waits
  on, asserted without a window; and **zero threads** are spawned for a
  timer — counted through T28.3's spawner seam, which every thread this
  milestone creates goes through, rather than by asking the OS.
  *(Corrected by M47/T47.3: what the seam counts is spawns. Each firing
  is a job, and under the shipped `Threads` each job was a thread — 28
  clones for 28 ticks. The pool that replaced it creates at most its
  cap; `docs/results-tasks.md` T47.3.)*

*Injection:* spawn a thread per tick — the seam's spawn count fires.
*Second injection:* leave the loop waiting indefinitely — the deadline
assertion fires.

## T28.12 — use the thing

`AGENT.md`, after M24: defects hide in machinery tested pass by pass and
never assembled. An example that spawns, batches, chains, cancels and
subscribes, driven through `just agent-drive` against a live window,
under both invalidation strategies.

**Gate:** the drive script asserts each action landed *and* its
consequence, with the quantity printed beside its bound.

> **Corrected 2026-09-25 (M47/T47.1).** The claim this task was credited
> with — that `run()`'s `WaitUntil` makes a timer fire on an idle window —
> was false until M47/T47.1, and this gate could not see it: every read
> the drive makes asks for a frame. `docs/results-tasks.md` T28.12 has the
> correction and the unobserved figures.

**Docs:** the book gains the chapter, or `book/src/events.md` gains the
section — decided when the surface is real rather than promised.

## 3. What this is not

**Not async in the core.** Nothing in `crates/lumen-static` awaits; the
core stays `no_std` and the inbox stays `std`-gated.

**Not a runtime this workspace owns.** The spawner is a seam, and the
default takes no dependency.

**Not `Box<dyn Fn>` anywhere.** The widget-field ban stands, and §2.1 and
§2.2 extend the same reasoning to work in flight and to job bodies.

**Not a message bus.** The inbox carries results of work the application
started. Input has a queue already, and it is the one the agent replays
through.

**Not stopping work.** Cancellation drops a result; it does not halt a
running job (§2.3).

## 4. Deferred, with conditions

- **A real async runtime behind the spawner seam.** When something needs
  what threads cannot express. §0.3's crate budget is the bar, and it
  wants a measurement nobody has taken.
- **Cooperative cancellation** — a job that can be *told* to stop.
  When a long job exists whose wasted work is worth the interface.
- **Structured concurrency**: a batch cancelled as a unit.
- **Streaming results** — a task that yields many values. A subscription
  is the shape for that today; a streaming task needs back-pressure this
  plan does not design.
- **Replaying task results in a session trace.** Sessions record inputs,
  deliberately (`docs/events-decision.md`). A recorded result is a
  different kind of input and needs its own decision — naming it here so
  the omission is deliberate.

## 5. What the review found, and what changed

Seven lenses, each verified by a second agent told to refute. **42
findings: 13 blocking confirmed, 20 major, and one "blocking" claim its
own verifier refuted.** The corrections, not the compliments:

| the first draft said | what the code says |
|---|---|
| tasks are spawned | **nothing could spawn one**: handlers are `fn(&mut S, &EventCtx)` and reach no host and no spawner. §2.5's outbox is the answer |
| the host owns an inbox of `M` | there is no `M`: `Host` is generic over the app, and `App` declares no message type. §2.6 |
| a reload bumps the epoch, discarding pre-swap results | that makes T28.7 — the milestone's centre — unpassable, and voids §2.1's own argument. **A reload no longer bumps it** |
| the hot fixtures "already drive a swap with state in flight" | they drive a swap with *saved* state: no thread, no channel, no `Host`, no pump, no inbox, and the app is destroyed before v2 loads. T28.7 now budgets the fixture work and puts the inbox in the loader |
| a timer is a deadline checked on the loop | an idle window has no frames to check it on. T28.11 now owns the scheduling |
| two spawners, threads and immediate | neither can deterministically defer, which T28.5 needs. A third, queued, is added |
| the allocation gate would catch a free | it counts allocations only; its `dealloc` counts nothing. T28.0 adds the counter |
| the inbox would become "a ring like `InputQueue`" | `InputQueue` is single-threaded with a `&mut self` push; a cross-thread queue needs a bounded channel. T28.2 says which |
| clipboard 11 crates, a11y 62, rfd 79, gpu 37, all from one doc | two counting methods and two docs; §0.3 now cites one method and says no figure exists for a runtime |
| `InputQueue`, `Overlays` and `Boundaries` all "refuse" | `Boundaries` widens to everything instead. §2.4 |
| the `.text` row proves a leaf profile carries none of this | it cannot move: the inbox is `std`-gated and the embedded fixture does not enable it. T28.7 counts symbols instead |
| §0.4 named two load-bearing docs | the handoff spec is the third, and §7 is the section this changes |
| — | **no failure story at all**: a panicking job stalls a chain forever. T28.9 |
| — | **no id stability rule**: a renumbered kind silently runs the wrong step. §2.1 |

**One blocking finding was refuted by its own verifier** and the plan is
unchanged for it: that T28.1's gate cannot be written because every live
observation here runs over the agent socket. A self-asserting fixture
binary needs no socket, and that is what T28.1 now specifies.
