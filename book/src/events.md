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
Scroll      { x, y, dy }
Key         { key, down }
Text        { ch }
Touch       { id, x, y, phase }
Modifiers   { .. }
```

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

`focus_visible` distinguishes keyboard focus from pointer focus, so a
click does not leave a ring behind. It is one write per frame rather than
a per-node flag — there is one focus, and storing the answer per node
would have grown `NodeState` for every node of every tree to say
something about one of them.

## Where this runs

Step 1 of [the frame](./frame.md): the queue is drained before anything
else, every event goes through `pass::dispatch`, and if any handler
returns handled the tree is marked invalid. Then the bindings, then
layout.
