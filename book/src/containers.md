# Containers and escape hatches

A derived `StaticTree` struct is the ordinary container: its fields are
its children and its shape is fixed. This chapter is about the five
containers that are *not* ordinary — one that clips, and four that exist
because the type cannot always know the shape.

## `ScrollView<W>`

One node plus its content's subtree, both on the table. The content is a
fixed shape, so there is nothing to virtualise and no arena.

What makes it a scroller is three things: the content is measured against
the viewport but **allowed to exceed it**, placed at an offset, and
clipped back to the viewport when painted *and* when hit.

`scroll` is public and a handler may write anything to it; `layout`
clamps. `axis` chooses direction — the cross axis is not scrolled, it is
fitted. And `measured_overflow` is private on purpose: it is a cache of
the last layout, kept because `on_event` has no state array and must
still know whether it is at an end, which is what decides whether a wheel
event chains to the parent.

## The four escape hatches

They exist because the type cannot know everything, and they share one
mechanism: **visibility**. A branch that is not showing still has table
entries; it is inactive rather than absent.

### `Either<A, B>` — one of two, **both resident**

Both subtrees exist in memory and in the table; `showing` picks one. That
is the trade: no allocation, no rebuild on switch, instant toggling — at
the cost of paying for both branches' storage all the time.

Its table is built in const evaluation with a compile-time assertion:

```text
Either<A, B> exceeds MAX_EITHER_NODES; raise it or split the branches
```

A tab strip is `Either` plus two buttons and application handlers —
`examples/form/src/main.rs` builds one that way, and needed no
new widget type to do it.

### `List<W>` — a `Vec` of one widget type

`items: Vec<W>`, laid out down a column with `spacing`. The count is
runtime, the type is not. Instances live in an `Arena` rather than the
const table.

### `Dyn` — a type-erased island

`Box<dyn Node>` plus the table that describes it. This is the hatch for
"the shape is decided at runtime", and it is the most expensive one,
because everything the static tree knows about shape stops at its edge.

### `Repeat<T>` — a virtualised list

The one to reach for when the data is large:

> `NODE_COUNT` grows with how much UI you **wrote**, not how much data
> you **display**.

One main-table entry, a const template, a runtime `count`, and only the
visible window materialised. `item_extent` is the uniform per-row extent
that makes the window computable without measuring every row, and the
row-configuring function is a **plain `fn` pointer** — it cannot capture,
which is what lets a hot reload re-supply it.

## What they all share: the `Arena`

Off-table instances live in an `Arena` beside the table. Two consequences
worth planning around:

**The cascade reaches them by template.** `style::cascade_deep` styles
the template, not the instance — so **per-instance styling does not
exist.** Row 7 of a `Repeat` cannot be selected differently from row 6.
If rows must differ, the difference belongs in what they contain.

**They are `alloc`, not `core`.** `List`, `Dyn` and `Repeat` are behind
the `alloc` feature. `Either` is not — it is the hatch an embedded target
can still have, which is exactly why it was worth the both-resident
trade.

## Choosing between them

| situation | hatch |
|---|---|
| two known layouts, switching | `Either` |
| n of the same thing, n small | `List` |
| n of the same thing, n large | `Repeat` |
| the shape itself is runtime data | `Dyn` |
| content taller than its box | `ScrollView` |

If you find yourself reaching for `Dyn` often, that is worth noticing:
[What it costs](./costs.md) says an interface whose shape is data is a
poor fit for a static tree, and heavy `Dyn` use is what that looks like
in practice.
