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

## `Select<S>`: the dropdown

The first widget built on all of this, and the one that found out what
was not wired up.

```text
Select<S> {
    trigger: Button<S, String>,
    menu:    Popup<Repeat<Button<S, String>>>,
}
```

Four nodes, **whatever the option count** — the rows are a `Repeat`, so
a menu's length is data about what you are showing rather than a fact
about the tree's shape. One `bind` configures a recycled row's label and
its `on_click`, and one handler serves every row because `EventCtx::row`
says which was hit.

It owns **opening**: a press on the trigger is consumed in
`Node::on_event`, the hook a widget has for its own state. It cannot be
an `on_click` — that is `fn(&mut S, ..)`, and from `&mut S` a widget is
reachable only as `&mut dyn Node`.

It does not own **closing**, and that is the interesting half. A press
outside every open overlay is dismissed and consumed by
`dispatch_overlaid` before any widget sees it, so pressing the trigger
while the menu is open closes it and does *not* reopen it. That is what
every desktop toolkit does, and here it is what happens when you write
no code for it.

### Reaching inside a composite

`#[widget_id]` names a *field*. A widget that is several nodes has to
publish its own, or nothing can address its parts — not a stylesheet,
not the agent's `#id` selectors. `Select` stamps
`select::PART_TRIGGER`, `PART_POPUP` and `PART_MENU`, namespaced so they
cannot collide with an application's own ids.

## Tab, Escape, and where focus goes

Overlays are not all alike to the keyboard, so the pass asks the topmost
one what it is (M44):

* a **modal** traps Tab — `Veil` always, and a `Popup` whose `modal`
  field is set. A dialog that lets Tab walk out to the window behind it
  is one a keyboard user cannot use.
* a **menu** — neither modal nor passive — is *dismissed* by Tab, through
  the same path Escape takes, so the owner hears about it (a controlled
  `Select` clears its `open`), and Tab moves on from where focus was.
* a **passive** overlay, a tooltip, is not Tab's business at all.

An overlay that opens **takes focus**: its first focusable node — a
menu's list, a dialog's first control — so the next key acts on what just
opened, whether a key or a click opened it (every desktop dropdown
answers Down after a click). A passive one never does, and one with
nothing focusable — a chooser, whose list is drawn rather than focused —
leaves focus where it is.

When an overlay opens, the pass records the focus it opened over; when
it closes — by Escape, by a press outside, by Tab, or by its application
— focus goes back there, or to the overlay's owner if that node is gone.
Before this, Escape cleared focus and Tab over a tooltip lost it.

## What `Host` does with all this

`Host::pump` paints through `paint_overlaid` and routes input through
`dispatch_overlaid`. It did neither until M24: the machinery above
shipped in M15 with the passes tested directly and **nothing in the
front door wired to them**, so a `Popup` inside a `Host` — which is
every example and every window — painted inline, under the backgrounds
of its later siblings, and never saw Escape or a click outside.

If you are adding a capability to this framework, that is the failure
mode to watch for. It is the fourth time here: a stylesheet, an image
source, an agent socket and now the overlay phase all existed and were
unreachable from the shell applications actually use.
