# Events, focus and keyboard navigation

## One queue

OS input, agent-synthesised input and test input all enter the same
`InputQueue`. That is the whole design and it buys one specific thing:

> anything an agent does replays as a test

The queue is host-owned and exists before any shell does. A winit front
end — or a headless one, or the agent — *feeds* it; it does not own it.
*The agent protocol*, later in the book, is what makes this pay off.

Events are plain data and serialise under `observe`, so a session can be
recorded. That is why several things you might expect to be tracked
*beside* the queue are carried *on* the events instead:

* **Click counts.** `PointerDown` carries `clicks: 1 | 2 | 3`, counted by
  the host because counting needs a clock and the core is `no_std` — and
  carried on the event so a recorded session replays as the same
  double-click it was recorded as.
* **Modifier state.** `Modifiers` is an event in the queue rather than a
  flag beside it, for the same reason: state held outside the queue makes
  a recorded session replay wrong.

## The event vocabulary

```text
PointerDown { x, y, button, clicks }
PointerUp   { x, y, button }
PointerMove { x, y }
PointerCancel { x, y }
Scroll      { x, y, dy }
Key         { key, down }
Text        { ch }
Touch       { id, x, y, phase }
Modifiers   { .. }
Action      { node, action }
```

`Action` is what an assistive technology asks of a node — `Default`,
`Increment`, `Decrement`, `SetValue(f32)`, `Expand`, `Collapse`, `Focus`
(M54/T54.4). It is delivered like a key, but from `node` instead of the
focused node: offered to `on_event` up the chain, and a `Default` then
runs the activation handler exactly as Enter does, so a screen reader's
"press" runs the same handler a click does. `Focus` moves focus the way
Tab does. It goes through the one queue, so a session a screen reader
drove replays as a test; `Host::action` queues one, and the agent's
`input.invokeAction` sends the same.

`Event` is 16 bytes and that is asserted. It is worth knowing why:
`Touch::id` is a `u32` rather than a `u64` because the wider id makes the
variant 20 bytes and the enum 24, which would take `InputQueue` from
roughly one kilobyte to one and a half. The windowing layer truncates the
platform's id, which is a per-session sequence and never comes close to
overflowing.

That is the M15 gating policy in miniature: a per-item cost is either
shown to fit or re-budgeted, never paid silently.

## Handlers, and what `capture` is for

A handler is `fn(&mut State, &EventCtx)` — see [State, bindings and
handlers](./bindings.md). `EventCtx` carries what the handler needs to
know about the event, and the input state carries `capture`: the node
that is currently receiving pointer events regardless of where the
pointer is.

Capture is what makes dragging a slider work when the pointer leaves the
track. It is an index rather than a reference, deliberately, because that
is what survives a hot reload.

## What counts as a click

A click is a **press and a release on the same item**. The press latches
what it landed on — the node, the row if it was on one, and a
*generation* a list bumps whenever its items may have changed — and the
release activates a handler only if that handler's node contains the
press as well. So dragging off one button and releasing on another
activates neither; pressing one row and releasing on another picks
nothing; and a row inserted above the pointer between press and release
cancels the click rather than delivering it to the item that moved under
a pointer that never did. The rule is recorded in
`docs/events-decision.md`, and keyboard activation (Enter, Space) is not
affected by it.

## Focus

Focus is the framework's, not each widget's. `NodeDesc` carries a
`FOCUSABLE` flag, the pass walks focusable siblings for tab order, and
the **focus ring is a `Style` property**.

That last one is a deliberate choice with a real consequence: a
stylesheet sets `focus_ring` once and *every* focusable widget gets one,
including custom widgets that implement nothing at all. A visible focus
indicator is the difference between a keyboard user being able to use an
application and not, and this is the cheapest way to make it the default
rather than a thing each widget must remember.

**Disabled is a style too** (M57/A16). `Patch::disabled(true)` makes
a node, and everything inside it, refuse hits, focus and activation. It
paints faded, and the agent reports it as disabled. It is a `Style`
field for the reason the ring is: it cascades, so a rule can switch a
whole form section off.

`focus_visible` distinguishes keyboard focus from pointer focus, so a
click does not leave a ring behind. The ring itself is drawn by the host,
once per frame, around the focused node — no widget draws its own
(M57/A6; the frame chapter says why). Visibility is one field on the
input state rather than a per-node flag — there is one focus, and storing the answer per node
would have grown `NodeState` for every node of every tree to say
something about one of them.

## Where this runs

Step 1 of [the frame](./frame.md): the queue is drained before anything
else, every event goes through `pass::dispatch`, and if any handler
returns handled the tree is marked invalid. Then the bindings, then
layout.

## Deferred work: spawn, batch, chain, cancel, subscribe

Everything above happens inside one frame. Work that *cannot* — a file
read, an HTTP request, a timer — goes through a different seam, and the
shape of that seam is decided by one rule:

> **No code reference may be held by in-flight work.**
> — `docs/events-decision.md`, rule 3

`fresh()` rebuilds the widget tree on a hot reload, but it cannot reach
into a pending future and swap the pointer that future captured. Work in
flight would complete into the *old* library's logic — safe, because the
old library is never unloaded, and still last week's code.

So everything at this seam is **data**.

### The two queues

A handler is `fn(&mut S, &EventCtx)`: it reaches the application and
nothing else. It has no host, no spawner, and it cannot be given one. So
asking for work is a value pushed into a queue, exactly as input is a
value pushed into one:

| direction | type | carries |
|---|---|---|
| into the frame | `inbox::Inbox<A::Msg>` | results, applied by `App::apply` |
| out of the frame | `spawn::Outbox` | requests, as `Descriptor { kind, step, payload }` |

The host drains the outbox *after* dispatch, so a descriptor a handler
pushed this frame is spawned on this frame, and pairs each one with a
job `fn` that `App::job_for` returns **from the currently loaded build**.
That is the moment the code pointer is read, and it is gone again before
the result comes back.

### A chain is an index, not a closure

iced writes `task.then(|out| next)`. That closure is held across an
await, which is precisely what rule 3 forbids. The way out is the move
this architecture already made for handlers: identity became
*positional*, so **the next step is an index**.

A completion says which step produced it; `App::apply` decides what
happens next, and what happens next is itself data — finish, or another
descriptor. `App::job_for` dispatches on `(kind, step)`, so the step
*is* the chain.

A window's hot swap relies on this. The old session saves every job its
host had in flight as `(kind, step, payload)`, and the new build spawns
each one through its **own** `App::job_for`. A chain whose first step
was running at the swap therefore finishes under the new step function.
`just window-reload-demo` checks this across a real dylib swap with
`fixtures/chain-app`. If the re-issue is dropped, the chain never
finishes. If the continuation is held as the old build's function
pointer, it finishes with v1's answer, which is rule 3's defect shown
rather than described (`docs/results-tasks.md` T49.10).

**Across a swap, delivery is at least once.** The old build's worker is
not stopped, because a thread cannot be. It finishes into an inbox that
nobody drains, and its job runs again under the new build, so a job with
a side effect may perform it twice. A running subscription is not
re-issued: the new build starts whatever its application declares.

`just hot-demo` shows the same shape on a fixture's own loader, which
holds the descriptor itself. No framework code is on that path.

### Batches, cancellation and subscriptions

There is no batch *type*: a batch is several descriptors pushed
together. It promises that all of them run and **nothing about the order
their results arrive in**.

Cancellation is an **epoch**. Every sender is minted at one, and a
result carrying any other is dropped at the drain and counted. What that
honestly means: the result is dropped, *the work is not stopped* — a
thread already running runs to completion and its output is discarded.
A hot reload deliberately does **not** bump the epoch, because a result
is data and the new build can read it.

A subscription's identity is `(kind, key)` — values, diffed each frame
against what is already running. iced hashes a recipe; hashing code
would mean two builds that format a URL differently restart every
subscription on reload.

A timer is a **deadline**, not a thread that sleeps. The host publishes
its earliest pending deadline and the window's loop waits until that
instant — without which a timer on an idle window would never be
checked, because `ControlFlow::Wait` produces no frames. Waking is not
enough on its own: the timers are ticked by a frame, so the window also
asks for one when it wakes there. Until M47 it did not, and a 120 ms
timer on an idle window ticked once in three seconds while the live
gate stayed green (`docs/results-tasks.md` T28.12 has the figures and
why the gate could not see it).

### Failure is a result too

A job that panics, and a spawn the OS refuses, both deliver a failure
through `App::failed` and are applied like any other message. Left
alone, a panicking job unwinds, its sender drops, and a chain waiting on
that completion stalls forever with nothing observing it.

### Seeing it work

`examples/tasks-demo` does all five, and `just tasks-drive` clicks every
button through the agent endpoint under both invalidation strategies,
checking each action's *consequence* rather than that something changed.
The numbers it prints — and what each of M28's parts cost — are in
`docs/results-tasks.md`.
