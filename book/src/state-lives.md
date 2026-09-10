# Where state lives

There are three places a value can live in an application here, and
which one you pick is a design decision the framework will not make for
you.

## The three places

**A sibling field on the state struct.** The truth. `count: i64`, the
document, the fetched rows. Nothing about it is presentational.

**A widget field on the tree.** The *presentation copy*.
`Label<String>` owns its text; `TextInput` owns its buffer and cursor;
`ScrollView` owns its offset.

**`NodeState`, in the flat array.** Geometry and resolved paint
attributes, written by layout and read by paint. Never yours to set.

`#[bind]` is the rule connecting the first two, and
[the frame loop](./frame.md) applies it between input and layout.

## Why the widget owns a copy at all

It looks redundant. The count is in `count`, and now it is also in
`ui.value` as a string. Why not format it during paint?

Because **only owned storage can invalidate a measurement cache.** A
`Label` measures its text — a real cost, involving the font stack — and
caches the result. It can only know the cache is stale if it is the thing
the new text was written to. A label that formatted during paint would
have to re-measure every frame, or cache against a value it does not own
and cannot observe.

That is also why `#[bind]`'s write goes through a comparison
(`set_if_changed`, `format_into`): writing an identical string would
throw the measurement away for nothing. `docs/layout-and-storage-decisions.md`
§1 has the full argument, and `docs/bindings-decision.md` has what the
comparison costs.

## Why paint cannot read `Style`

`layout` and `place` take `&[Style]`. `paint` and `hit` do not.

A widget therefore cannot read its own colour at paint time. Instead the
cascade's answer is *stamped into* `NodeState` during layout — `fg`,
`bg`, `radius`, `font_size`, `font_weight`, `shadow`, `pad` — and paint
reads it from the array it already carries.

This was a deliberate reversal. Threading a fourth parameter through
every `Node` method and every hatch was proposed and rejected
(`docs/text-and-scrolling-decisions.md` A2). Copying into an array the
pass already has is cheaper at every call site and it keeps the paint
signature small enough that a wrapper can forward it.

## What this means when you add a field

Ask which of the three it is:

* Does anything *derive* from it? Then it is state, and a `#[bind]`
  carries it to the screen.
* Is it what a widget renders, and does the widget measure it? Then the
  widget owns it, and something binds into it.
* Is it geometry? It is not yours — `NodeState` is written by layout.

Getting this wrong is not a compile error. The symptom is a value that
looks right in the debugger and stale on the screen, and the cause is
almost always a widget field written directly by a handler with no
binding pointing at it, so nothing re-derives it on the next frame.
