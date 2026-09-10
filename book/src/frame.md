# What a frame does

One function does the whole frame: `Host::pump`. It is short enough to
describe completely, and the order of its steps is load-bearing in three
separate places.

```text
1. drain the input queue      every event through `pass::dispatch`
2. if a handler ran           mark the tree invalid
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

## Why layout comes before the focus ring

The focus ring is written *after* layout, not before, because layout is
what resolves the ring width from the stylesheet. The first version
zeroed it beforehand and had no effect whatsoever — a bug that a reader
of the code would not have spotted, because both orderings look equally
sensible until you know which pass owns the value.

It is also one write per frame rather than a per-node flag. There is one
focus. Storing "am I the focused node, and is focus visible" per node
would have grown `NodeState` for every node of every tree to say
something about one of them, which the M15 gating policy forbids.

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
