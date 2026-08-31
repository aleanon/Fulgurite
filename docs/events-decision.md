# Decision: event handlers are `fn` pointers; the async boundary is data

*2026-08-31. Refines handoff §7's "events must be data, not closures"
(DECIDED there with Elm-style `Msg` enums as the mechanism). The
constraint §7 protects is kept; the mechanism changes because this
architecture removed the problem the mechanism existed to solve.
Companion implementation plan: `docs/plan-events.md`.*

## The decision

1. **Tree-internal event handlers are plain `fn` pointer fields** on
   widgets — `on_click: fn(&mut S, &EventCtx)` — never `Box<dyn Fn…>`.
   Across hot reload they follow the existing fresh/keep split: `fresh()`
   supplies the new dylib's pointers, `restore()` never touches them.
2. **Event payload lives in fields or dispatch-time arguments, never in
   captures.** The `fn` type makes this a compiler guarantee, not a
   discipline: an `fn` pointer cannot capture.
3. **Anything in flight across a frame or thread boundary — async
   completions, timers, input queued during a swap — returns as data**,
   interpreted by whatever code is loaded when it arrives. No code
   reference may be held by in-flight work.

## Why §7's argument no longer forces `Msg`

§7's real objection was never serialization; it was **captured state**:
`move |s| s.items.remove(idx)` strands `idx` in code-land, where
save/restore cannot see it, so the new `fresh()` hands the closure a
wrong value. That is a dynamic-tree disease — build functions capture
loop indices at build time.

This architecture removed the reason to capture:

- **Identity is positional.** The save button's handler doesn't need to
  know which button it is; it *is* the save button, at a fixed field
  path. `fn(&mut S, &EventCtx)` suffices.
- **Repeater instance data is recomputed, not captured.** `bind` re-runs
  every frame from data, and dispatch knows the row from hit testing —
  it arrives in `EventCtx.row` as an argument.

The scheme is already validated in the tree: `Repeat.bind` is an `fn`
pointer field, and the Step 4 dylib-swap demo rebinds all 47 restored
rows through the *new* library's `bind_row`. The derive preserves the
structural no-code-in-saved-bytes guarantee unchanged: an `fn` field
marked `#[reload(keep)]` fails to compile (no `Serialize`).

## Why the async boundary still gets data

`fresh()` rebuilds the tree — it cannot reach into a pending future, a
timer, or a queued task and swap the code pointer it captured. Work in
flight across a reload would complete into the *old* library's handler
(safe only because we never `dlclose`; still wrong — it is last week's
logic). So the seam rule: tasks complete with **result values** into a
host-owned inbox; the currently loaded code interprets them. This is the
one place message-shaped events survive, because it is the one place the
capture problem is real here.

## What is given up, priced

- **Introspectable actions.** `Msg::Save` names itself; an `fn` pointer
  is an address. Mitigation when needed: dev-build symbolication, or an
  optional `&'static str` action-name field on interactive widgets
  (observe-gated, like `wid`).
- **Semantic-level replay.** Sessions record *inputs* (clicks, keys),
  not messages. Chosen deliberately: input-level replay also exercises
  hit-testing and focus, which message-level replay would skip.
- **A single update choke point.** Elm's centralized `match` is a style
  some apps want; nothing prevents an app from defining one `Msg` enum
  and pointing every handler at `fn dispatch(s, ctx)` — it is just no
  longer framework-mandated.

## Binding rules going forward

- Framework interactive widgets expose handlers as `fn` types only.
  `Box<dyn Fn…>` in a widget field is the anti-pattern this decision
  bans (a capture is hidden unserializable state).
- Handler fields are code: `restore` never writes them, and they must
  never be marked `keep` (already a compile error).
- Nothing that outlives the frame may hold a code reference — the inbox
  is the only legal way for deferred work to re-enter the app.
