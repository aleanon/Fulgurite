# Damage and invalidation

Two related mechanisms with different jobs, and it is worth separating
them before anything else:

* **Invalidation** decides *which nodes to lay out and paint*. It works
  on the tree.
* **Damage** decides *which pixels to redraw*. It works on the command
  stream.

[What it costs](./costs.md) quotes the spec on why the first matters more
than any amount of traversal speed: a correct invalidation architecture
decides whether you touch three nodes or three thousand.

## `Damage` is a set of regions

```text
enum Damage {
    None,           // identical frame — skip raster and present
    Regions(..),    // these differ, and nothing else does
    All,            // first frame, resize, structural change, no retention
}
```

`None` is worth noticing on its own: an identical frame lets a shell skip
both the raster **and** the present, which is what makes an idle window
genuinely idle rather than cheaply busy.

## Why it is not a bounding box

Because a bounding box amplifies, and the amplification was measured
rather than assumed.

`List::paint` clips its items to its own node's rect, and that rect is as
wide as the **widest row**. A row whose text gets one pixel wider
therefore rewrites the clip, and a union-of-old-and-new bounding box
damages the entire column. `docs/damage-amplification-decision.md` has
the measurement and the ratio; the summary is that the two cases differ
by more than an order of magnitude.

The subtler half of that finding is worth carrying into any benchmark you
write: the arm reported a **minimum** over its iterations, which
preferentially selected the frames where the width happened *not* to
change — so the published figure described the lucky frames. A minimum
over a bimodal distribution reports the cheaper mode, not the typical
one.

## Retention is a type parameter

`Rebuild` and `Incremental`, on `Host`. Never a cargo feature, for a
reason [The component boundary](./component-boundary.md) states: `Rebuild`
is the **oracle** `Incremental` is tested against, and "same pixels for
the same input" is not a test you can write against something you have
compiled out.

`Rebuild` costs nothing when unused because its `observe` body is empty —
a call the optimizer removes rather than a branch taken per command.

## Buffer age, and why damage is kept for several frames

A shell that presents into a rotating set of buffers gets back a buffer
that is one, two or three frames old. Redrawing only *this* frame's
damage into a two-frame-old buffer composites onto the wrong base — the
classic double-buffered smear.

So retention keeps a short ring of recent damage and answers
`take_damage_for_age`, which is what a shell must actually ask. A depth
above three is either an unusual compositor or a bug.

## Where invalidation is conservative, and why

[What a frame does](./frame.md) covers the loop: a handler that ran marks
the whole tree, and a binding that wrote marks the whole tree. Both are
deliberately blunt, because a handler is `fn(&mut App, &EventCtx)` and
may touch anything — narrowing to the clicked node would narrow to where
the *event* landed rather than where the *change* did.

The precision that does exist is at the write: bindings compare before
they write, so a frame in which nothing changed is *known* to be still.

## Testing damage without testing nothing

Two shapes to avoid, both of which have already happened here:

**Damage under opaque content cannot be observed.** A half-cleared
boundary pixel beneath opaque content is overwritten by the next command,
so the artefact never survives to be compared. A GPU damage gate was
checking a real property in a situation that could not exhibit it, and
the same injection produced a visible difference only once the region was
under *translucent* content.

**Bound the right quantity.** A damage check once bounded a row's height
by the *list's* total height, making the limit two orders of magnitude
larger than anything it could observe. Print the measured quantity next
to the bound, or a vacuous one is invisible.

Both stories are in `AGENT.md`, and they are the reason the rule there is
that a test which has never failed has not been tested.
