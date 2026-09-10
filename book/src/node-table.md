# The const node table

Every `StaticTree` type has a `SUB_TABLE`: an array of `NodeDesc`,
computed at compile time, describing the shape of the tree that type
denotes. It is the piece of the original design that survived a
supersession, and it survived because of what got keyed on it.

## What a `NodeDesc` holds

```text
first_child   u32     index into the same table
parent        u32
next_sibling  u32
child_count   u16
flags         u8      RIGID | CONTAINER | FOCUSABLE
wid           &str    #[widget_id("save")]
classes       &[&str] #[widget_class("row")]
src           SrcLoc  (observe builds only)
```

Structure as indices, not pointers. The whole table is `const`, so it is
in the binary rather than on a heap, and two applications of the same
type share it.

## Traversal does not use it

This is the part that surprises people who read spec §5 first. The
original decision was to walk the flat table. That half was **superseded
by measurement** (`docs/results-traversal.md`): recursive descent over
the widget struct, writing into a flat state array, beat iterating the
table at every size tested.

So layout and paint recurse over the struct. The table stayed anyway.

## What the table is actually for

**Index addressing.** A node has a stable `usize` for the whole frame,
and everything that needs to *name* a node uses it:

* `[NodeState; NODE_COUNT]` — the layout cache is a parallel array, so
  `st[i]` is node `i`'s geometry, with no lookup.
* The agent protocol resolves a selector to an index and reports layout,
  styles and hits by it.
* The change journal and the ambient audit key their entries on it.
* The cascade walks it linearly — the one pass that still does — and it
  runs only when the sheet or the shape changes, never per frame.

**Static facts about shape.** `parent` and `next_sibling` are what a
relayout boundary walk needs; `RIGID` is the const-folded rigidity that
[What it costs](./costs.md) named as one of the design's two real
advantages. A dynamic tree checks a flag at runtime, per node, per frame.
Here the check is not present.

## Indices shift; ids do not

The table is generated from the source, so **inserting a field renumbers
every node after it**. That is fine for a frame — indices are stable
within one — and fatal for anything that persists across an edit.

Which is why `#[widget_id]` exists and why it is not gated behind the
`observe` feature: it is the stable address an agent, a test or a
stylesheet uses, precisely because table indices are not stable under
source edits. If you find yourself writing down a node index, you
probably want an id.

## Off-table constructs

`Repeat`, `List`, `Either` and `Dyn` hold instances that the table cannot
describe, because their count or their type is not known at compile time.
Those live in an `Arena` beside the table rather than in it, and the
cascade reaches them by *template* rather than by node
(`style::cascade_deep`).

The consequence is worth stating twice, because it surprises people:
**per-instance styling does not exist.** Row 7 of a `Repeat` cannot be
selected differently from row 6. Rows differ by what they contain, not by
what a stylesheet says about them.
