# Topology in the type

A retained-mode toolkit keeps the tree in memory: nodes are heap objects,
children are `Vec<Box<dyn Widget>>`, and every frame walks pointers to find
out what the interface currently is. The shape is data, so the shape can be
anything, so nothing about it can be known before it runs.

This framework moves the shape into the type and leaves only the values in
memory. `docs/00-handoff-static-tree.md` §3 is the design argument; this
chapter is what it means when you use it.

## The tree knows its own size at compile time

Every `StaticTree` type carries two constants, and they are computed by the
derive, not measured at runtime:

```rust
# extern crate lumen_static;
# extern crate lumen_static_widgets;
use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;

{{#include ../../examples/counter/src/main.rs:tree}}
# #[derive(Default)] struct Tally { count: i64 }
# #[derive(DeriveApp, Default)] struct Counter { #[tree] ui: Ui, #[state] #[bind(ui.value, count, "count: {}")] tally: Tally }

fn main() {
    // Eight nodes: three labels, the control subtree, and its three.
    assert_eq!(Ui::NODE_COUNT, 8);
    // Three levels: Ui, Controls, and Controls' children.
    assert_eq!(Ui::DEPTH, 3);

    // And they are *constants*, so they can size an array.
    const N: usize = Ui::NODE_COUNT;
    let layout: [NodeState; N] = Default::default();
    assert_eq!(layout.len(), 8);
}
```

That last line is the whole point. `[NodeState; Ui::NODE_COUNT]` is the
entire layout cache for the application, sized by the compiler, living
wherever you put the struct. There is no arena, no allocation on the frame
path, and no possibility of the cache and the tree disagreeing about how many
nodes there are.

## What this buys, and what it costs

**Buys.** A frame allocates nothing in steady state — gated by a counting
`#[global_allocator]` in the test suite, not asserted in prose
(`docs/results-allocation.md`). The exception is text: the gate draws
block glyphs, and the truetype rasteriser and measurer a window uses
allocate per frame until M50's glyph cache lands (same file, "The
truetype path is not covered"). A tree deep enough to overflow the stack is a
compile error rather than a crash. And the node table can be walked as a flat
array instead of chased as a pointer graph
(`docs/00-handoff-static-tree.md` §5).

**Costs.** Monomorphization. Every distinct tree type is distinct code, and
the naive version of this idea generates an amount of it that stops being
funny at around a hundred nodes. §4 of the spec is about that trap and §5 is
the decision that escapes it — the tree is reified as a flat `const` table, so
the *shape* is const data rather than const *code*. If you are about to write
a deeply generic widget, read those two sections first.

## When the shape genuinely is not known

It sometimes isn't — a list whose length comes from a file, a panel that
swaps between two layouts. The type cannot know those, and the framework does
not pretend otherwise. There are four escape hatches (`Either`, `Dyn`, `List`,
`Repeat`), they are deliberately explicit, and they are the subject of
`docs/00-handoff-static-tree.md` §6 and its own chapter here.
