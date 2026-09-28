# What a frame does

One function does the whole frame: `Host::pump`. It is short enough to
describe completely, and the order of its steps is load-bearing in three
separate places.

```text
1. drain the input queue      every event through `pass::dispatch`
2. if a handler ran           mark the tree invalid
   if pointer or focus moved  mark the nodes it moved between
3. run the bindings           `App::update` — the derive's work
4. layout, then place         constraints down, sizes up, parent positions child
5. paint                      into a display list, through Scaled and Recording
6. close the frame            retention reports what changed
```

## Why the bindings sit at step 3

A handler has just changed state, and layout is about to measure what
that state renders as. Running the bindings between the two is what makes
a binding **impossible to forget**: there is no call site to remember,
because the loop is the call site.

The alternative — updating the label inside the handler — is the thing
that produces a frame showing a count the handler has already changed.
That failure cannot be expressed here.

## Why the host draws the focus ring

There is one focus, so there is one ring, and the host draws it: after
the tree and its overlays have painted, from `input.focus` and whether
focus is keyboard-visible, in the palette's `focus` colour. It is an
outline with a gap, outside the node, so it can neither cover the
control nor merge with a body of its own colour.

Until M57 each widget drew its own ring, as a filled rect underneath
itself in its text colour, from a `focused` flag the pass published —
and the host hid it for pointer focus by zeroing the ring width after
layout. Every part of that failed somewhere
(`docs/widget-audit.md` X4 and bug 1): a strip with no body became a
solid block, a slider became a solid bar, a white-labelled button got a
white ring, and the flag was one event late, so clicking one button lit
the ring on the button before it. Reading `input.focus` in the one place
that draws the ring is what makes the two unable to disagree.

It still costs no per-node state: the width is `Style::focus_ring`, a
`u8`, and zero opts a node out.

## Why paint is wrapped twice

```text
DisplayList  ←  Recording  ←  Scaled  ←  tree.paint()
```

`Scaled` converts logical geometry to physical on the way in. `Recording`
lets retention observe every command as it passes, so that closing the
frame yields the damage without a second walk. At a scale of 1.0 `Scaled`
is a forwarding wrapper, and under `Rebuild` retention forwards to an
empty observer the optimizer deletes — so the two wrappers cost nothing
when they are not doing anything.

This is also where a wrapper rule that has already cost this project a
shipped defect applies — *Writing your own widget*, later in the book,
is where that rule is stated.

## Invalidation is conservative here, on purpose

Steps 2 and 3 both mark the whole tree invalid, and the reason is in the
handler's signature. A handler is `fn(&mut App, &EventCtx)`, so it may
touch any field of the application. Narrowing the mark to the node that
was *clicked* would narrow it to where the event landed rather than to
where the change did — and those are different nodes the moment a button
edits a label somewhere else, which is the ordinary case.

So it degrades to a whole-tree walk rather than to a wrong frame. The
precise entry point is `#[bind]`, which knows statically which tree field
it writes; narrowing beyond that is open work
(`docs/plan-invalidation.md` T16.7).

**Step 2's second line is precise, and it is the one that was missing.**
A hover, a press, a release, or focus becoming visible runs no handler at
all, yet each changes what one node paints — its shade, its ring. So the
host compares that state before and after the queue drains and marks
exactly the node the pointer or focus left and the one it reached. Before
M25/T25.0 nothing marked it, and a host that skips clean subtrees painted
a hovered button in its resting colour (`docs/plan-appearance.md` T25.0).

There is one piece of precision already: `format_into` compares before it
writes, so a frame in which no binding changed anything is *known* to be
still. An idle window, an animation between ticks and a repeat key that
changed nothing all land there and cost nothing.

## Nothing here allocates

Not "few allocations" — zero, in steady state, gated by a counting
`#[global_allocator]` in its own test binary
(`docs/results-allocation.md`). The display list is reused, the state
array is fixed-size and owned by the host, and layout writes into it in
place.

That is the property [What it costs](./costs.md) named as one of the two
this design actually buys, and this loop is where it is spent or kept.
