# lumen-static

A GUI framework where the widget tree's **shape lives in the type** and only
its values live in memory. A `#[derive(StaticTree)]` struct *is* the tree:
`NODE_COUNT` and the node table are computed at compile time, so the tree's
passes allocate nothing, truetype text included since M50
(`docs/results-allocation.md`), and a tree that could overflow the stack does not compile.

```rust,no_run
# extern crate lumen_static;
# extern crate lumen_static_widgets;
# extern crate lumen_static_winit;
{{#include ../../examples/minimal/src/main.rs:app}}
```

That is `examples/minimal`, whole — not an excerpt written for the book. Every
snippet in these pages is included from a file in the repository that the
build compiles and the test suite runs, so a chapter cannot describe an API
the code no longer has.

## What this book is for

The framework has three documentation surfaces already, and this is a fourth
with a different job:

| surface | answers |
|---|---|
| `README.md` | what is this, in sixty seconds |
| this book | how do I build something with it, in teaching order |
| `docs/00-handoff-static-tree.md` | what was designed, and what is still **OPEN** |
| `docs/results-*.md`, `docs/*-decision.md` | what was measured, and what it decided |

The book **cites; it does not restate.** Every figure here carries the results
document that owns it in the same sentence, because a number stated in two
places is a number that will disagree with itself. If you want to know why
something is the way it is, the citation is the answer — follow it.

## Is it fast

Yes, and the numbers live elsewhere. A 4K full redraw on the software
rasteriser is 2.50 ms against a 16 ms budget (`docs/results-threading.md`);
a steady-state frame performs zero heap allocations, gated by a counting
allocator in the test suite (`docs/results-allocation.md`).

## Where to start

If you want the shape of an application, read [Your first tree](./first-tree.md).
If you want to know why the tree is a type at all, read
[Topology in the type](./topology.md). If you are looking for the reasoning
behind a decision rather than instructions, start at
[Where the decisions live](./decisions.md).
