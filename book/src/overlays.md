# Overlays

A dropdown, a tooltip, a modal and a context menu all need the same two
things: to paint above everything, and to be hit before everything. Both
are things a widget **cannot do from inside the tree**, and understanding
why is the whole chapter.

## Why a widget cannot do it

**A widget cannot escape an ancestor's clip.** `ScrollView::paint` pushes
a clip, paints its content, pops it. A popup *inside* that subtree paints
between those two calls and is clipped by them — and a dropdown inside a
scrollable list is the commonest overlay there is.

**A widget cannot paint above its later siblings.** Paint is a pre-order
walk in declaration order, so a popup on child 2 is drawn before children
3..n and ends up underneath them.

## Why it is not a layout mode either

The obvious fix is a z-order layout mode. It costs too much here, for two
specific reasons:

* It breaks the invariant that **paint order is tree order** — which
  retention's positional diff and the agent journal's per-node command
  ranges both rest on.
* It needs a node to have two parents, in a model whose premise is that
  the parent is a `const`.

## What it actually is: a second paint phase

**Layout does not change at all.** `pass::paint_overlaid` runs the
ordinary walk, then repaints deferred subtrees *outside every clip
scope*. The command stream is phase 1 followed by phase 2, both in table
order — so it stays deterministic, and retention still holds.

Opening an overlay changes the stream's *length*, which `Incremental`
already answers with whole-frame damage. Correct, and already built.

## `Popup<W>`

The widget half:

```text
Popup<W: Widget> {
    content: W,
    open: bool,
    anchor: Anchor,   // Below | Above | Over
}
```

It is **out of flow**: it measures its content but reports **zero size**
for itself, so opening one does not move its siblings. That is the whole
point of a dropdown.

`Anchor::Below` is a dropdown, `Above` is a menu that opens upward, and
`Over` is a modal's backdrop child. `measured` is kept from `layout` so
`place` can position without re-measuring — the same pattern
`ScrollView` uses for its overflow and `Slider` for its track width.

## What it costs when nothing uses it

Nothing. `NoOverlays` is a zero-sized type whose `defer` returns `false`,
so a widget that offers to defer simply paints inline and the overlay
loop never runs. It is a **type parameter, not a feature** — the M15
gating policy's second mechanism — which is also why the whole thing is
`no_std` with no allocator: `Overlays` is a fixed array.

## The vacuous test this produced

Worth knowing if you write an overlay test. The first hit test for the
overlay phase declared the popup **before** the thing it covered, so the
ordinary walk found it and the overlay path was never exercised at all.
The test passed, and it was testing nothing (`AGENT.md`, M15/T15.0).

If you are testing that an overlay is hit first, declare it *after* what
it covers — otherwise you have written a test that ordinary tree order
already satisfies.
