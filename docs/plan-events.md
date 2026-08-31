# Plan: the event system

*2026-08-31. Implements `docs/events-decision.md` (fn-pointer handlers,
data at the async seam). This is the prerequisite both remaining parity
phases wait on (`docs/plan-agent-parity.md` P4/P5's input half) and the
unlock for §9's action synthesis. This is a plan, not work done.*

## Ground rules inherited from the codebase

- **One input queue** (Lumen's invariant, adopted at P4 planning): OS
  input, agent-synthesized input, and test input enter the same queue,
  so any session replays as a test. The queue exists before any shell
  does — a winit shell later *feeds* it, it does not own it.
- **`WidgetOps` stays non-generic.** New per-kind behavior arrives as
  more monomorphic `fn` pointers via `WidgetOps::new(...)` + a
  `with_event(...)` builder, exactly like `observe`/`stats` — except
  event/hit ops are *not* cfg-gated: input is a release feature.
- **The state type `S` is erased at the ops seam** the same way widget
  types already are: `Button<S>`'s monomorphized event op casts the
  state pointer back to `S`, symmetric with how every op casts
  `base + offset` to its own widget type. The safety contract on
  `WidgetOps` grows one clause: dispatch must pass the state pointer the
  tree was built for.

## E0 — vocabulary and queue

`crates/lumen-static/src/event.rs`:

```rust
pub enum Event {
    PointerDown { x: f32, y: f32 },
    PointerUp   { x: f32, y: f32 },
    PointerMove { x: f32, y: f32 },
    Scroll      { x: f32, y: f32, dy: f32 },
    Key         { key: Key, down: bool },
    Text        { ch: char },
}
```

Plain data, `Copy` where possible, serde derives behind `observe` (the
recording feature — replay is a dev concern, the types are not).
`InputQueue` = host-owned `VecDeque<Event>`; `push` from anywhere,
drained by the host once per frame. `EventCtx` carries what dispatch
learned: node index, `row: Option<u64>` for repeater content, pointer
position in node-local coordinates.

Deliverable test: queue round-trips through serde (the replay seed).

## E1 — hit testing in core

The agent's probe reconstructs hits from the AccessKit snapshot —
dev-only and allocation-heavy. Dispatch needs the real thing:
`hit_test(base, table, kinds, st, x, y) -> Option<Hit>` walking the
flat table (bounds from `NodeState`, deepest match, document order for
ties). Off-table content (repeater windows) participates via a new
per-kind op:

```rust
pub hit: fn(base, d, st, x: f32, y: f32) -> HitDetail,  // default: None
```

`Repeat`'s impl maps the point to `(row, template_node)` from its window
geometry. Fused subtrees resolve to the fused node — internals are not
hittable, which turns "fuse display cells, not interactive regions"
(Step 6's cost list) from advice into behavior. The agent's `ui.probe`
switches to this and stops building snapshots for hit tests.

Deliverable tests: hit ↔ layout agreement on the P1 fixtures; repeater
row resolution across scroll; fused node opacity.

## E2 — handlers and dispatch

First interactive leaf: `Button<S>` (generic over app state, concrete at
the use site — the `Repeat<T>` pattern, single-node `SUB_TABLE`):

```rust
pub struct Button<S> {
    pub label: &'static str,
    pub on_click: fn(&mut S, &EventCtx),
}
```

`dispatch_event<S>(state: &mut S, tree…, ev) -> bool`: hit test, then
walk the hit chain leaf→root offering the event to each kind's `event`
op; the op **copies its handler pointer out before the `&mut S`
exists** (shared-then-unique in sequence — the aliasing pattern gets a
doc + a test, since every interactive widget will repeat it). Repeater
routing: the repeater's event op re-dispatches into the hit instance
with `EventCtx.row` filled — handlers on template widgets receive the
row as an argument, never as a capture.

Focus, minimal: a `FOCUSABLE` flag bit (bit 2 is free), host-held
`focused: Option<u32>`, pointer-down sets it, `Key`/`Text` go to it.
Text *editing* waits for real text storage (`String` labels), which is
its own prerequisite and stays out of scope here.

Hot reload: `Button<S>` gets a manual `Reloadable` impl like `Label`'s
(`label` keep, `on_click` untouched) — the derive's untagged-field rule
would otherwise demand `Reloadable` of an `fn` type.

Deliverable tests: click mutates state through the tree; row click
receives the right row after scrolling; reload demo gains a button whose
keep-state survives while its handler provably runs new-dylib code.

## E3 — the async seam

`Inbox<M>`: host-owned mpsc receiver + cloneable sender. Deferred work
(threads now, an executor behind `dyn` at the shell later — see the
renderer/executor answer in `docs/plan-agent-parity.md`'s appendix)
sends *values*; the host drains per frame into an app-supplied
`fn(&mut S, M)`. The rule from the decision doc is enforced by shape:
the sender moves into the task, no code pointer does.

Deliverable test: a message sent before a simulated swap is drained
after it and interpreted by the "new" apply fn.

## E4 — agent parity P4 (rides on E0–E2)

`input.click {selector | x,y}`, `input.key`, `input.type`,
`input.invokeAction` (AccessKit `Action::Click` → synthesized click at
the node's center): all *enqueue* into the one queue — the agent never
calls handlers directly, so everything it does replays. This needs the
first mutable host surface in the agent crate: a small `Pump` trait
(`enqueue(Event)`, `pump() -> frame`) implemented by the host loop;
`Agent::dispatch` grows a `&mut dyn Pump` parameter for the input
methods only. Recording: the queue serializes to a session trace; a
replay harness is the seed of the `lumen-test` equivalent.

## E5 — later, explicitly

- `ui.explain` becomes possible once dispatch exists (trace the hit
  chain and who consumed); revisit after E4.
- Scrollable regions: the first construct that can break the
  clamp-invariant — when it lands, the reserved `W1003`/`W1004` lint
  codes become implementable (the guard test in `tests/lint.rs` will
  say so).
- Action-name introspection (`observe`-gated `&'static str` on
  interactive widgets) if agent workflows want it — priced in the
  decision doc.

## Order and size

E0+E1 are small and independent of any design risk. E2 is the heart and
carries the two things to get right early: the state-pointer erasure
contract and the copy-handler-before-borrow pattern. E3 is tiny. E4
closes parity P4. Rough shape: E0–E2 one sitting each, E3+E4 together.
