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

## Status: E0–E4 implemented 2026-08-31

All five milestones landed in one pass; 14 new tests
(`lumen-static/tests/events.rs`, `lumen-static-agent/tests/input.rs`).
Deltas from the plan as written are recorded inline below.

## E0 — vocabulary and queue — **DONE**

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

Deliverable test: queue round-trips through serde (the replay seed) —
done, gated on `observe` so the *vocabulary* stays available in release
while only *recording* is a dev feature. (`observe` gained `dep:serde`
for this.)

## E1 — hit testing in core — **DONE**

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

**Delta — the scan is simpler than planned.** No depth tracking is
needed: all nodes containing a point form a single ancestor chain
(children never escape parents, siblings never overlap — the same
invariant that makes the audit reserve `W1003`/`W1004`), so in DFS order
the *last* containing node is the deepest. One forward scan, no
allocation. `hit.rs` says so and points at the guard test; the first
construct that breaks the invariant must revisit both places at once.

Fused-node opacity needed no code: a fused subtree has no table children
and no `hit` op, so it answers as one node — Step 6's "fuse display
cells, not interactive regions" is now enforced rather than advised.

Deliverable tests: hit ↔ layout agreement; repeater row resolution across
scroll (row 2 after a two-row scroll); miss outside the tree.

## E2 — handlers and dispatch — **DONE**

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

Deliverable tests: click mutates state through the tree; a non-interactive
node does not consume; row click receives the right row *after* scrolling
(0 then 5 — recomputed, never captured); focus follows pointer-down and
clears on a non-focusable target; keys route to focus.

`Button<S>` carries the aliasing pattern in a comment where the next
interactive widget will copy it: the handler is read out through a raw
pointer *before* `&mut S` exists, because the widget lives inside `S`.
`Reloadable for Button<S>` is hand-written (label keeps, `on_click`
untouched) — the derive's untagged-field rule would otherwise demand
`Reloadable` of an `fn` type.

## E3 — the async seam — **DONE**

`Inbox<M>`: host-owned mpsc receiver + cloneable sender. Deferred work
(threads now, an executor behind `dyn` at the shell later — see the
renderer/executor answer in `docs/plan-agent-parity.md`'s appendix)
sends *values*; the host drains per frame into an app-supplied
`fn(&mut S, M)`. The rule from the decision doc is enforced by shape:
the sender moves into the task, no code pointer does.

Deliverable test: values cross a real thread boundary and are applied by
the currently-loaded `apply` fn; draining twice yields nothing.

## E4 — agent parity P4 (rides on E0–E2) — **DONE**

`input.click {selector | x,y}`, `input.key`, `input.type`,
`input.invokeAction` (AccessKit `Action::Click` → synthesized click at
the node's center): all *enqueue* into the one queue — the agent never
calls handlers directly, so everything it does replays. Recording: the queue serializes to a session
trace; a replay harness is the seed of the `lumen-test` equivalent.

**Delta — the trait is `Host`, not `Pump`, and it hands out the tree.**
`TreeRef` borrows the host, so a caller cannot hold one *and* pass
`&mut host`; the trait therefore provides `tree()` alongside
`enqueue`/`pump`, which keeps each read borrow short and ends it before
anything mutates. Entry point is
`Agent::dispatch_input(&mut dyn Host, req)`, which delegates every
non-`input.*` method to the read-only `dispatch`.

Also shipped: `ui.probe` now uses the **core** hit test instead of the
AccessKit snapshot, so probe and click can never disagree about what is
under a point (and the snapshot's BFS depth pass, dead once probe moved,
is gone). Point resolution accepts `{x,y}` or a selector, clicking the
node's centre, so agents never compute geometry.

Deliverable tests: click by author id reaches the handler; by
coordinates; `invokeAction` as a centre click; **the synthesized trace is
exactly two ordinary pointer events and replays on a fresh host to the
same state**; typing enqueues per character; unknown key and unknown
selector both teach.

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
