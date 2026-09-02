# Plan: agent-observability parity with Lumen

*2026-08-31. Target: what `lumen-agent` gives an agent driving a Lumen
app, mapped onto this architecture. Evidence surveyed:
`Lumen/crates/lumen-agent/src/lib.rs` (35 JSON-RPC methods),
`lumen-core/src/semantics.rs` (structural `NodeHandle` identity, selector
resolution with nearest-miss/ambiguity errors),
`Lumen/docs/plan-agent-observability.md` rev2 and
`Lumen/docs/review-agent-observability-2026-08.md` (the ambient-audit
thesis and its review scars) — all four in Lumen's tree, not ours;
verified present 2026-09-02. This is a plan, not work done.*

## What Lumen's surface actually is

Grouped, from `lumen-agent`:

| Group | Methods |
|---|---|
| Tree & geometry | `ui.getTree`, `ui.getLayout`, `ui.probe`, `ui.probeRegion`, `ui.getWindows`, `ui.getMenu`, `ui.getCursor` |
| Visual | `ui.screenshot`, `ui.getStyles`, `ui.getAppliedStyles`, `ui.animations` |
| Explanation | `ui.explain`, `ui.lint`, `ui.getDeps`, `ui.whatDependsOn`, `ui.lastChange`, `ui.lastDamage` |
| App-level | `app.diagnostics`, `app.logs`, `app.perf`, `app.command`, `app.setValue`, `app.systemRequests` |
| Input | `input.click/type/key/scroll/hover/drag/drop/gesture/invokeAction/setLocale` |
| Synchronization | `ui.waitFor`, `ui.waitSettled` |

Three structural properties matter more than the method list:

1. **One input queue.** Synthesized input enters the same queue as OS
   input, so any agent session replays as a `lumen-test`. Parity means
   preserving this invariant, not the method names.
2. **Selector ergonomics.** `resolve_one` returns `NotFound { nearest }`
   and `Ambiguous { candidates }` — the error *teaches* the agent. Cheap,
   high leverage; copy it.
3. **The 2026-08 lesson: interrogative → ambient.** Lumen's review found
   its strong query surface still required a hypothesis ("name the node
   you suspect"); the fix is a per-frame ambient audit pushing findings
   into a log the agent already reads. We should build the ambient layer
   *first-class from the start* rather than retrofit it.

Doctrine we already share: "a lean build implies no agent" — Lumen
feature-gates the whole surface; our `observe` feature is the same
decision, already verified zero-cost.

## What we have, and our two structural advantages

Step 5 built: AccessKit `TreeUpdate` + JSON dump (roles, values, bounds,
kinds, children), source provenance, row-stable repeater ids. No
transport, no queries, no input, no lint, no screenshot.

Advantages Lumen lacks, to preserve through every phase:

- **Source provenance.** `NodeDesc.src` (file:line, const-stamped) —
  nothing like it exists in Lumen's semantics. Every parity response
  that names a node should carry `src`, making findings directly
  actionable ("the button at `src/sidebar.rs:42`") — §9's design goal.
- **The state array is diffable.** Lumen answers "what changed" through
  a reactive dependency graph (`ui.getDeps`, `ui.lastChange`). We have
  no such graph — but two flat arrays (`NodeState` now vs last frame)
  answer the same question by memcmp-and-report, cheaper and complete.

## Identity gap to close first

Lumen's `NodeHandle` is structural (path-derived, survives arena
recycling); authors add `StableId`s (`.id("stats-inc")`). Our table
index is perfectly stable across frames *and* reloads — but **shifts
when the source changes** (inserting a field renumbers everything after
it). An agent's identify → edit → reload → re-check loop crosses exactly
that boundary. Resolution:

- **Author ids** via a field attribute: `#[widget_id("save")]` interning
  into `NodeDesc` (dev-gated like `src`). The primary agent-facing
  address.
- **Selectors re-resolve** after every reload; raw indices documented as
  frame-stable, not edit-stable.
- Provenance is the fallback identity ("the node from `app.rs:30`") —
  it moves with the edit, which is what the agent wants.

## Phases

Ordered by dependency, not value; P1 and P3 are the value.

### P0 — transport and session (small)

**DONE 2026-08-31** (with P1, one commit). `crates/lumen-static-agent`:
JSON-RPC 2.0 `Agent::dispatch(&TreeRef, &Value)` with Lumen's envelope;
the host wrapper stayed thinner than planned — `TreeRef` borrows
base/table/kinds/states per call, `Agent` holds only frame + timings.
WebSocket deferred as planned.

`lumen-static-agent` crate: JSON-RPC 2.0 `dispatch(&mut App?, &Value) ->
Value` mirroring Lumen's envelope, method names, and error codes where
semantics coincide (agent skills and muscle memory transfer). In-process
dispatch first; WebSocket behind a feature exactly like Lumen's `ws`.
Requires a host-side "app" wrapper (base ptr + table + kinds + states +
frame counter) — the first time we name that object; keep it thin.

### P1 — query parity from existing data (the bulk of read-side parity)

**DONE 2026-08-31.** Shipped: `ui.getTree` (dump + `wid` + `src`),
`ui.getLayout` (incl. constraints), `ui.probe`/`probeRegion` (deepest-hit
over the snapshot, repeater instances included), `ui.resolve` (added —
explicit resolution is useful on its own), `app.diagnostics` (incl.
per-repeater rows/window via `size_of_set`), `ui.waitSettled`,
`app.perf`. Author ids landed as `#[widget_id("save")]` on fields,
stamped into `NodeDesc.wid` (observe-gated, same zero-cost builder
pattern as `src`). Selector grammar as planned (`#id`, kind, `~text`,
`@file:line`, index; space = conjunction) with teaching errors
(`-32001` + available ids/kinds/files, `-32002` + candidates).
Documented P1 limits: selectors resolve main-table nodes only
(instances via probe/getTree); `waitSettled` is trivially true until
animations exist. 9 protocol tests.

All from table + `NodeState` + AccessKit snapshot, no new subsystems:

- `ui.getTree` — the Step 5 dump, plus author ids and per-node `src`.
- `ui.getLayout` — constraints/size/pos for one node (we can also return
  *constraints*, which Lumen doesn't expose; keep it).
- `ui.probe` / `ui.probeRegion` — hit test from bounds + document order
  (z is document order until layers exist).
- Selector language + `resolve_one` with nearest-miss and ambiguity
  reporting, selector grammar: `#author-id`, `kind`, `label~="text"`,
  index paths, and `@file.rs:42` (provenance selectors — ours alone).
- `app.diagnostics` — table length, kind count, rigid fraction,
  repeater windows (count/first/len), memory of table+state.
- `ui.waitSettled` — trivially true until animations exist; ship the
  method so agent loops are portable.
- `app.perf` — layout/paint pass timings we already measure in benches.

### P2 — "what changed": state diffing instead of a dependency graph

**DONE 2026-08-31.** `crates/lumen-static-agent/src/journal.rs`: each
`frame_done` records per-node geometry *and* per-node paint checksums,
diffs against the previous frame, and appends to a 64-frame ring.
Shipped `ui.lastChange { since }` (default: previous frame) and
`ui.lastDamage { since }` — the latter was a bonus the same diff paid
for. Findings carry `wid`, `kind` and `src`, so "what changed" names the
line to edit.

Design points worth keeping:
- **Two flags, not one.** Checksum inequality is a superset of geometry
  inequality (paint folds geometry), so changes classify as `geometry`,
  `content`, or `both`. The `content` case is what a pure geometry diff
  would miss: `"hello"` → `"world"` is invisible to bounds.
- **Structural fingerprint.** A changed table shape (hot reload, swapped
  tree) re-seeds the baseline and flags `reset: true` for that frame —
  indices across the discontinuity are not comparable, so nothing is
  claimed. This is the honest version of what a naïve index diff would
  report as "everything changed".
- **Truncation is reported.** An agent polling slower than the ring gets
  `truncated: true` plus `oldest_frame`, never a silent half-answer.
- **Reverted changes still report.** Coalescing keeps the union of flags
  observed in the interval, with `old` at `since` and `new` now; a node
  that moved and moved back did change, and the agent wants the flicker.

Limits recorded: repeater *instances* are not individually diffed — a
scroll or row edit surfaces as a `content` change on the repeater node
(its paint op folds the window), with per-window detail available from
`app.diagnostics`. Cost is one extra paint dispatch per node per frame,
dev-surface only; no AccessKit snapshot is built at frame time (only per
query). If the future invalidation work wants this diff as its
acceptance oracle, the journal moves down into `lumen-static`.

`ui.lastChange { since_frame }` from double-buffered `NodeState` diff +
paint-checksum per subtree; report changed nodes with old/new geometry
and `src`. Covers the *agent-facing* purpose of Lumen's
`ui.getDeps`/`whatDependsOn`/`lastDamage` without a reactive graph —
and doubles as the acceptance instrument for future invalidation work
(the diff is ground truth for what *should* have been dirty).
`ui.getDeps` itself: **no equivalent, by architecture** — record as
out-of-scope, not TODO.

### P3 — ambient audit (`ui.lint` + `app.logs`)

**DONE 2026-08-31.** `crates/lumen-static-agent/src/audit.rs` runs at
every frame boundary; `ui.lint` reads the cached current set (queries
cost nothing) and open/close transitions push into an `app.logs` ring.
Lumen's three rev2 scars adopted rather than re-earned, each pinned by a
test: findings keyed `(code, node)` structurally (two invisible labels
both report); presence-diff dedup (fix-then-re-break reports twice); a
standing finding logs once, not per frame.

**The intrinsic-size probe.** Several checks need to know what a node
*wants* to be. Instead of adding vocabulary to every widget, the audit
calls the existing `up` op twice under two different generous
constraints: equal answers mean the node is sized by its content
(natural size known), growing answers mean it is a filler. That one
trick powers both `W1001` and `W1005` — and is what keeps a legitimately
collapsed spacer from being reported as invisible content, the class of
false positive Lumen's review warned would otherwise be the first thing
an agent ever sees. Probing is leaves-only (a container's `up` reads
child sizes out of the state array, so re-measuring it means nothing).

Shipped codes, each with a firing *and* a quiet test:

| code | finding |
|---|---|
| `W1001` | intrinsic content laid out with zero area — in the tree, absent on screen |
| `W1002` | non-empty node entirely outside the root's bounds |
| `W1005` | intrinsic content allotted less room than it needs (quantifies what is missing) |
| `W1006` | nothing with both area and content anywhere — the blank frame |
| `W1007` | a collection with items that occupies no space |

**Two planned checks turned out unreachable and were reserved, not
shipped.** `W1003` (child escapes parent) and `W1004` (sibling overlap)
cannot occur in this layout model: constraints flow down and every node
clamps to them, so a child is never allotted more than its slot, and
containers place siblings sequentially by measured size. Squeezing
surfaces as `W1005` instead. The audit's own test suite discovered this
— the overflow test failed with a correct truncation finding in hand —
and it is now an invariant guard (`overflow_is_impossible_here_...`)
that will fail the day a scroll region, absolute positioning, or a
minimum exceeding the parent's maximum makes the codes implementable.
This is the direct application of Lumen's "finished but unreachable
analysis" lesson, applied *before* shipping rather than after.

`NodeStats` (a new `observe`-gated `WidgetOps` hook: `items`,
`materialized`) is what lets `W1007` distinguish a legitimately empty
list from a broken one — geometry alone cannot, and that distinction is
the difference between a useful check and a false positive on the most
common startup state.

Cost per frame: the journal's one paint dispatch per node plus two `up`
dispatches per leaf, into a scratch buffer that stops allocating after
the first frame. No AccessKit snapshot is built at frame time. Dev
surface only.

Adopt Lumen's corrected design wholesale — its rev2 scars are free
lessons: findings keyed `(code, node)` (never code alone), per-frame
presence-diff dedup (not a monotonic seen-set), edge-triggered logs,
and audit cost bounded by caching whatever measurement it repeats.
Findings carry `src` — a lint that names the file and line beats
Lumen's version at its own goal. Start with lints computable from
geometry alone:

- zero-area or offscreen interactive/labelled nodes
- children escaping parent bounds; sibling overlap
- text truncation (measured width > laid-out width)
- blank frame (no visible painted node in viewport)
- repeater pathologies (window empty with count > 0; extent 0)

Deferred with named dependencies: contrast, opacity, invisible-ink
(need styling/paint data that doesn't exist yet — when styling arrives,
wire these *into the audit on day one*; Lumen's contrast analyzer sat
finished-but-unreachable for months).

### P4 — input and actions (needs the event system)

**DONE 2026-08-31**, as milestone E4 of `docs/plan-events.md` (design:
`docs/events-decision.md`). Shipped `input.click`, `input.hover`,
`input.scroll`, `input.key`, `input.type`, `input.invokeAction` — all
*enqueueing* into the host's one queue rather than calling handlers, so
an agent session replays as a test (asserted: the trace of a synthesized
click is two ordinary pointer events, and replaying it on a fresh host
reproduces the state). Targets are `{x,y}` or any P1 selector, clicking
the node's centre. New `Host` trait (`tree`/`enqueue`/`pump`) is the
first mutable host surface; `ui.probe` moved onto the core hit test so
probe and click cannot disagree.

The prerequisite is the `Msg` event dispatch + hit-testing + focus that
§9's action synthesis already waits on. When it lands: `input.click/
type/key/scroll`, `app.setValue`, `input.invokeAction` (AccessKit
`Action` becomes real), all injecting into **the one input queue** so
sessions replay as tests (Lumen's invariant, adopted verbatim). Design
the queue with recording in mind from the first commit.

### P5 — visual (needs a renderer)

`ui.screenshot` arrives with the composed renderer (§11: take Lumen's
`lumen-render` — its `Renderer` trait renders a display list to pixels
headlessly, which is exactly the agent's need). `ui.getStyles`/
`getAppliedStyles`/`ui.animations` arrive with styling/animation, if
ever — style may stay code-side in this framework, in which case
`src` provenance already answers "why does it look like this" better
than a cascade dump would.

### Explicitly not planned

- `ui.explain` (Lumen's "why didn't the click land" narrative) — 
  revisit after P4 when there is event routing to narrate; P2's diff +
  P3's lints cover most of its use ambiently.
- `input.setLocale`, `ui.getMenu`, `app.systemRequests`,
  multi-window — no corresponding subsystems; parity is not a goal
  where the feature itself doesn't exist.

## Sequencing note

P0+P1 are implementable this week from existing data. P2 is small and
should ride with P1 (the double-buffer is trivial). P3 is where the
agent value concentrates — Lumen's own review says the ambient layer is
what its strong interrogative surface was missing. P4/P5 are gated on
the event system and renderer, which are prior roadmap items with their
own justification; parity rides along rather than driving them.

---

## Appendix: the three architecture questions (answered 2026-08-31)

Asked while planning parity; recorded because the answers shape P0/P4.

**1. Is there a build function receiving a runtime?** No. Lumen's
`App::new(build)` runs `fn(&mut BuildCx) -> Element` against a host
`Runtime` every rebuild. Here the tree is a plain struct constructed
once (`Default`/`fresh()`); topology is the type, there is nothing to
re-run, and no runtime handle exists to pass. The only build-like hooks
are `fresh()` (reload constructor) and repeater `bind` fns. Consequence:
parity items premised on rebuild+reactivity (`ui.getDeps`) have no
analogue and are replaced by state diffing (P2).

**2. Renderer / async-executor injection?** Yes, at the shell, and the
architecture keeps them out of widget signatures entirely. Lumen threads
`Headless<R: Renderer, E: Spawner, P: PlatformConfig>` generics through
everything including agent dispatch. Here `WidgetOps` fns are monomorphic
fn pointers over concrete types, so paint must target a concrete
`DisplayList` — the renderer consumes it *after* the pass and can be
`&mut dyn Renderer` (or a shell generic) without touching the tree,
table, or ops. Same for an executor: nothing in the core awaits; a
`dyn`-injected spawner lives in the host/session struct, never in widget
fields (executor handles are code, i.e. `#[reload(fresh)]` territory,
and the no_std leaf profile must not carry them). §8's one-type-axis
budget stays reserved for the arena; renderer and executor should be
`dyn` at the shell, not new type parameters.

**3. Where does application state live?** In the tree, by design (§3:
values in the fields — `Label.text`, `Repeat.scroll`; tests prove no
copies exist). Non-widget state sits in sibling fields of an outer
struct (the hot-reload fixture's `Session { ui, counter, mode }`), still
one offset-addressable allocation. The app/view distinction is drawn by
`#[reload(keep/fresh/reset)]` annotations, not by storage location.
Lumen is the opposite pole: state in a host-owned keyed `Runtime` store,
widgets as ephemeral `Element`s referencing `Signal`s — which is what
buys Lumen `ui.getDeps` and tier-2 hot patch, and what this design
traded for zero-diff identity and the serialize-the-tree reload model.
