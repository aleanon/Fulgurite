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
just tally         # the counter, in a window
just tally-frame 7 # one frame to target/tally.ppm, at count 7
just tally-drive   # drive it through the agent surface and assert
just run-agent     # the window plus JSON-RPC on 127.0.0.1:9230 (scripts/agent_client.py)
just test          # the gate: static checks, release shape, dev shape, fixtures
just bench         # benchmarks (machine-dependent; never a gate)
just hot-demo      # swap a dylib under a running app and check state survived
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

`just tally` opens a window. `examples/counter-window` is the only crate
with an OS dependency, and it is two functions long — window events into
`Host::post`, `Canvas::pixels` blitted to the surface — because the shell
was built to stop exactly there. The same tree, layout, paint and
renderer run headlessly under `--headless`, which is how the example is
checked in CI. The window also serves the agent protocol, so an agent
can click it, read the tree and take a screenshot from a shell:
`.claude/skills/verifying-apps` is the ladder.

Text is real (`fontdue`, behind the shell's `truetype` feature); the
default is still a deterministic stand-in so tests can assert on
geometry. Adopting Lumen's `lumen-text` and `lumen-render` is the rest of
M7.
