# lumen-static

A GUI framework where the widget tree's **shape lives in the type** and
only its values live in memory. A `#[derive(StaticTree)]` struct *is* the
tree: `NODE_COUNT` and the node table are computed at compile time, so a
frame allocates nothing, and a tree that could overflow the stack does
not compile.

```rust
use lumen_static::prelude::*;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 12.0)]
struct Ui {
    #[widget_id("title")]
    title: Label,
    #[widget_id("value")]
    value: Label<String>,
    gap: Spacer,
    #[widget_id("inc")]
    inc: Button<Counter>,
}

#[derive(DeriveApp, Default)]
struct Counter {
    #[tree]
    ui: Ui,
    #[bind(ui.value, "count: {}")]
    count: i64,
}

fn on_inc(c: &mut Counter, _: &EventCtx) {
    c.count += 1;
}
```

`#[bind]` is the whole update step: the frame loop refreshes the label
from `count` between input and layout, so a handler cannot leave a stale
value on screen.

`examples/counter` is this program, complete — it renders to a PPM and
drives its own buttons through the agent protocol, so it doubles as a
test of that surface.

## Try it

```
just counter     # render a frame to target/counter.ppm
just test        # the gate: static checks, release shape, dev shape, fixtures
just bench       # benchmarks (machine-dependent; never a gate)
just hot-demo    # swap a dylib under a running app and check state survived
```

## The crates

| crate | what it is |
|---|---|
| `lumen-static` | the core: widget traits, layout, style, text, events, the §6 escape hatches |
| `lumen-static-macros` | `StaticTree`, `Reloadable`, `App` |
| `lumen-static-shell` | the frame loop (`Host`) and a software renderer, headlessly testable |
| `lumen-static-agent` | a JSON-RPC surface an agent drives the app through — the same input queue a user's mouse uses |
| `lumen-static-reload` | the `extern "C"` shims for hot reload; separate because the core contains no `unsafe` |

## Where to read next

- **Writing a widget** — the crate docs for `lumen_static`: `Node` vs
  `Widget`, the seven attributes, and what the derive emits.
- **Why any of it is shaped this way** — `docs/00-handoff-static-tree.md`,
  the design record. Sections carry DECIDED / SUPERSEDED / REFINED
  markers.
- **What is next** — `docs/plan-architecture-v2.md` is the work queue;
  `docs/README.md` indexes every document with its status and whether its
  numbers are still reproducible.

## Status

The counter runs headless and renders. There is no window yet: the shell
deliberately stops at the OS seam so the loop stays testable without a
display, and a winit front end attaches at two points (`Host::post` and
`Canvas::pixels`). Text is measured by a stand-in, not a shaper. Both are
M7 in the plan.
