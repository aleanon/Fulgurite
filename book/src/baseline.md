# The retained-mode baseline

You have built an application now. This part is about why it is shaped
the way it is — and the honest place to start is that **most of the
architecture is not novel at all.**

The design record for everything in this part is
`docs/00-handoff-static-tree.md`. This chapter carries no figures: where
a claim rests on a measurement, it names the document that owns the
number rather than repeating it.

## The pipeline everyone converged on

Flutter, browser compositors, Slint and GPUI all run some version of
this, and so does fulgurite:

```text
UI thread
  1. Retained node tree        persistent identity + state
  2. Invalidation              dirty flags, relayout boundaries
  3. Layout                    constraints down, sizes up
  4. Paint to display lists    cached per unchanged subtree
                 │
Raster thread    ▼
  5. Raster and composite      damage rects only
```

Spec §2 calls this "the floor", and means it. The interesting decision in
this framework is *how the tree is stored*, not what the pipeline does
with it.

## Four properties, in rough order of importance

**Nothing rebuilds by default.** A mutation sets a dirty flag that
propagates upward only until it reaches a **relayout boundary** — a node
whose own size provably cannot change, because it is fixed or because
its parent gave it tight constraints. A text change deep inside a long
list costs the distance to the nearest boundary, not the size of the
tree.

**Layout is single-pass.** Constraints flow down, sizes flow back up, and
the parent positions the child. No node reads its own final position
while laying out. That is the property that makes layout linear instead
of iterative reflow, and it is why [Layout](./layout.md) could be
described as four ordered steps rather than as a solver.

**Stages skip independently.** Dirty for paint but not for layout skips
layout. A node that only moved skips both.

**The GPU keeps its own retained copy.** An unchanged frame is a few
quads over a persistent texture plus a damage rectangle.

## What is actually built

The spec is a design record, not a status page, so it is worth being
blunt about the gap:

* Relayout boundaries: the node table carries the parents, the rigidity
  bit and contiguous subtree ranges the property needs. Retention is a
  **type parameter** on `Host` — `Rebuild` or `Incremental` — and never a
  cargo feature, deliberately: `Rebuild` is the oracle the incremental
  path is tested against, and you cannot test against an oracle you have
  compiled out.
* The GPU backend keeps a persistent target texture that damage is
  scissored into, so a partial frame really is a few quads over a
  retained image. There is **no layer tree**: no per-subtree texture and
  no transform-only frame. `docs/results-gpu.md` measures what the built
  half is worth.
* Rasterisation is split across bands rather than pipelined behind the
  UI thread. That is not the threading the spec originally recommended,
  and `docs/results-threading.md` records why the recommendation was
  backwards — pipelining raises input-to-photon latency by a frame and
  pays only when the two halves are balanced, which on a real frame here
  they are not.

## Two implementation details that matter as much

**Flat contiguous state storage**, addressed by index rather than by
chasing pointers. `[NodeState; N]` for the whole application.

**Recursive descent over the widget struct**, writing into that flat
array — *not* iteration over a flat table. The spec originally said the
opposite and was superseded by measurement
(`docs/results-traversal.md`); the const table survives because index
addressing is what the agent surface, the change journal and the ambient
audit are all keyed on.

## The caveat the spec puts on itself

> This design pays real costs in memory and in structural-change work.
> For a UI where everything changes every frame, immediate mode can win.
> That is not our target.

Worth taking at face value. If your interface is a visualiser redrawing
everything on every frame with no stable identity between them, a retained
tree is overhead you will not get back — and none of the next chapter's
arguments will change that.
