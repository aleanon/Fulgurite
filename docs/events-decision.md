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

### The chain case, demonstrated 2026-09-20 (M28/T28.7)

This rule anticipated a *chain* and nothing had ever tested one. iced
chains with `task.then(|out| next)` — a closure held across an await,
which is a code reference in flight and breaks this rule on the first
reload.

The answer is the move this architecture already made for handlers:
identity became positional, so **the next step is an index, not a
function**. A descriptor is `(kind, step, payload)`, and the currently
loaded build dispatches on `(kind, step)`.

`just hot-demo` now asserts it end to end. v1 runs step 0 of a chain; the
pending descriptor — three `u64`s, held by the **loader process**,
outside both dylibs — survives the swap; v2 runs step 1. v1's step 1 is
`payload * 10` and v2's is `payload * 1000`, so the assertion can tell
which build interpreted the old build's in-flight work:

```
chain: v1 ran step 0 -> 5; pending descriptor (1, 1, 5) held across the swap
chain: v2 ran step 1 on payload 5 -> 5000
  ok: a chain started before the swap completed into the NEW build's step
```

**And the defect this rule exists to prevent is demonstrated rather than
argued.** Holding the next step as a function pointer captured at spawn
— v1's `hot_step` symbol — instead of as an index:

```
chain: v2 ran step 1 on payload 5 -> 50
the chain completed into V1's step function (5 * 10): the next step was
held as a code pointer captured at spawn, which is what rule 3 forbids
```

Last week's logic, running after the swap, with no crash and no warning.

**Corrected 2026-09-27 (M49/T49.10, R4-F3): that demonstration ran on the
fixture's own code.** `fixtures/hot-host` keeps the pending descriptor
itself and calls the library's `hot_step`. No `lumen_static::spawn` or
`inbox` code is on the path, so it shows the rule and not the framework
keeping it. The framework's window swap carried no work in flight until
T49.10. Now the old session saves its host's ledger beside the kept state
(`hot::save`), and the new build's `hot::boot` re-issues each descriptor
through its own `App::job_for`. `just window-reload-demo` runs the same
two injections on that path. With the re-issue dropped, the chain never
completes (`chain -`). With the continuation held as the old build's fn
pointer, it answers with v1's value (`chain 50` against `chain 1040`).
`docs/results-tasks.md` T49.10 has the output.

### The request case, 2026-10-06 (M88/T88.3)

A platform request — a screenshot, a clipboard read — is work in flight
too, but not a job: only the window can do it. The same rule decides its
shape. What travels is a `Copy` `Request` holding the application's
**tag**, never a callback; the window keeps only that tag, and the answer
is turned into a message by `App::answered` **in the build loaded when
the answer arrives**, then applied inside the same call (`Host::answer`).
Parking the converted message for the next pump — the plan's first
draft — would put a message from the old build in flight across a swap
that `hot::save` cannot carry. `just window-reload-demo` checks both
halves on a real swap: a request taken from v1 and answered after the
swap reads `v2:x` (`request_across_a_swap`; answering the pre-swap
session instead leaves `v2:`), and an answer applied on v1 before the
swap survives it as kept state, `v1:x` (`answer_then_swap`; the parked
draft reads `v2:`, "answer lost at the swap"). `docs/results-tasks.md`
T88.3 has the output.

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
  (and since M88 its progress slot) is the only legal way for deferred
  work to re-enter the app, and `Host::answer` the only way for a
  platform request's answer to: by tag, converted and applied in the
  build loaded when it arrives (M88/T88.3).
- **A continuation is an index, not a function** (M28/T28.6, T28.7). A
  chain's next step is a `step` field in a descriptor, resolved by the
  loaded build; the rule above forbids the closure form, and
  `just window-reload-demo` is where the framework is checked against it
  across a real window swap (M49/T49.10). `just hot-demo` shows the same
  shape on a fixture loader's own code.
- **A click is a press and a release on the same logical item**
  (M44/T44.2, DECIDED 2026-09-26; `docs/plan-input-semantics.md` §2.2).
  The press latches `(node, row, row_generation)`; on the release a
  handler runs only if its node contains the press, and a row's handler
  only if the row and the generation match. A release anywhere else
  clears the press shade and activates nothing. `ctx.row` is still a
  fresh index from hit testing — the latch is what checks identity, and
  "identity" is "the list did not change shape", not an item key (the
  key function §2.2 C describes is deferred until an application needs
  a click to survive a reorder). `crates/lumen-static/tests/activation.rs`
  is the gate.
