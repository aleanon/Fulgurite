# M34 — `Boundaries` by default, once the evidence says it is safe

> **Written 2026-09-23, not started. Revised the same day after a
> seven-lens review** (§4). The decision to flip the default was made on
> 2026-09-23. This milestone gathers the evidence the flip needs, then
> makes it.
>
> A turnkey window uses `Everything` unless the non-default `boundaries`
> feature is set, and every headless `Host` uses it because it is `Host`'s
> default type parameter. So **every invalidation win since M16 is worth
> nothing by default**, M29's included
> (`docs/plan-damage-precision.md` §1).
>
> There is a recorded reason for care. **Four of M26's five Phase A
> defects were invisible under `Everything`**, because a clean subtree
> is skipped before a wrong answer can be observed
> (`docs/plan-frame-and-types.md`). M25/T25.0 had the same shape. The
> strategy that saves the time is also the one that has been exercised
> less.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red.*

---

## 0. The rules for this milestone

1. **Take inventory before the flip.** List every gate with the
   strategy it runs under, and whether it *can* exercise a strategy at
   all, before the default changes. A gate that runs only under
   `Everything` has said nothing about the frames a user will see from
   now on. A gate with no strategy at all says nothing either way, and
   it is recorded as strategy-free rather than converted.
2. **Keep an escape hatch, named for its purpose.** The feature that
   selects `Everything` after the flip is a *diagnostic*. If a stale
   frame disappears when you build with it, you have found an
   invalidation defect. The rustdoc says so, in those words.
3. **Both defaults flip.** Both `lumen_static_winit::Inval` and `Host`'s
   own default type parameter change. Otherwise every headless host,
   and counter-window's hand-written window, would still run
   `Everything`, and "the default" would mean two different things.

## 1. What exists today

- **Two strategies:** `just widget-drive` and the tasks drive
  (`scripts/tasks_drive.sh`). **One strategy:** `just agent-drive`,
  which builds showcase with `--features agent` only, so it runs
  `Everything`. The first draft of this plan listed it under two
  strategies (§4, finding 1).
- `crates/lumen-static-shell/tests/invalidation_oracle.rs` compares
  `Everything` with **`Boundaries<16>`**, byte for byte, over a driven
  session. The window ships **`Boundaries<256>`**, and M26/T26.5 was a
  wrap at exactly 256 marks, so the shipped `N` has never been through
  the oracle.
- **Strategy-free, and not conversion targets:** `golden.rs` renders a
  hand-built `DisplayList` and never builds a `Host`. `fixtures/hot-host`
  has no `Host`. GPU parity grabs `minimal`'s first frame, which
  follows a resize, and a resize calls `mark_all`.
- **Who names the feature:** `examples/showcase/Cargo.toml`,
  `examples/tasks-demo/Cargo.toml`, `scripts/widget_drive.sh` (`agent`
  and `agent,boundaries`) and `scripts/tasks_drive.sh`
  (`--features boundaries`). A handful of test and source comments also
  say that `Everything` is the default.
- **Which frames can differ at all.** A consumed event and an
  `update()` that wrote both call `mark_all` in `pump_with`. On those
  frames the two strategies do the same work. Only frames marked
  precisely can differ: hover, an unconsumed press, focus, and tick-
  and host-driven writes. That stays true until M29/T29.6.

## T34.0 — the inventory, and running what has only run one way

> **DONE 2026-09-24.** The inventory, and what running it found:
>
> | gate | strategy before | now |
> |---|---|---|
> | `tests/invalidation_oracle.rs` | `Everything` vs `Boundaries<16>` | **and `<256>`, the shipped type** |
> | `tests/bind_marks.rs`, `conservative_marks.rs`, `handled_narrowing.rs` (M29) | both, by construction | both |
> | `scripts/widget_drive.sh`, `scripts/tasks_drive.sh` | both | both |
> | `scripts/agent_drive.sh` | **`Everything` only** | **both** |
> | *(M42/T42.2)* all live drives | a leg's strategy was a label the script set | each leg launched by `scripts/live/launch.py`, which checks the socket's owner and the process's own `app.diagnostics` strategy and features (`docs/plan-drive-identity.md`) |
> | `tests/golden.rs`, `fixtures/hot-host`, GPU parity | strategy-free (no `Host`, or a first frame after a resize, which marks everything) | recorded, not converted |
> | every example's headless frame, `counter-window --drive`, every test that names `Host<A>` | `Host`'s default, `Everything` | follow the flip (T34.2) |
>
> **One defect, found by asking the question M33 §2.3 left open:**
> `Host::tick` marked nothing, and `App::tick` may write any widget —
> showcase's writes its readout and reseeds the stage. Under
> `Boundaries` a tick-written label kept its old box: the first tick
> already differed from the whole-tree frame
> (`crates/lumen-static-shell/tests/tick_marks.rs`, which failed before
> the fix). An advancing tick that reports a change now marks
> everything; a zero-`dt` query, by `App::tick`'s contract, marks
> nothing.
>
> **Amended 2026-09-25, M41/T41.1** (`docs/plan-boundaries-repair.md`
> §1.1). The inventory above lists gates; it never listed the host's own
> writes, and so it missed one. **`Host::compose` wrote the focused
> field and marked nothing** — R1-F2, now S0 — so under this plan's
> default an IME commit was painted over last frame's layout: the field
> kept its box and scroll, and a 16 px empty field painted no text at
> all (`crates/lumen-static-shell/tests/compose_marks.rs`). This is the
> row that should have been here, with every other `&mut self` method on
> `Host` audited the same way at b213ae0:
>
> | method | what it writes | what it marks |
> |---|---|---|
> | `compose` | the focused node's buffer, preedit, caret | **nothing, until T41.1**; now the focused subtree and the arena flag, when the widget reports a change |
> | `pump`, `pump_with` | dispatch, `apply`, `update` | a consumed event: everything, or the consumed subtrees for a split application (M29/T29.6); pointer-state changes: old and new node; bindings: their own ranges or everything |
> | `tick` | whatever `App::tick` writes | everything, when it reports a change on a positive `dt`; nothing otherwise, which `App::tick`'s rustdoc now makes the contract (M41/T41.0) |
> | `paste` | the focused field, by posting `Text` events | each chunk's pump marks as a consumed event does; **the pumps lay out with `BlockMetrics`** (R2-F1, M41/T41.5) |
> | `set_stylesheet` | every style, and the arenas (`cascade_deep_with`) | everything |
> | `restyle_subtree` | one subtree's styles and the arenas in it (`cascade_subtree_with`) | that subtree; the arena flag is not needed, because the cascade reaches the subtree's arenas itself |
> | `set_palette` | every style, re-cascaded with the arenas | everything; nothing for an equal palette |
> | `invalidate`, `invalidate_all` | nothing: they are the reporting path | the named subtree, or everything; both set the arena flag |
> | `adopt`, `restore`, `reload` | the application, and the sheet if a source exists | everything, through `reasked`; for a host without a sheet source that mark is the only one (R5-F6, `tests/reload_oracle.rs`) |
> | `resize`, `set_scale` | the viewport | everything, and retention is reset |
> | `guard` (a caught panic) | resets the list, overlays, pointer state and queue | everything, and the arena flag |
> | `set_sheet_source` | the sheet, through `set_stylesheet` when non-empty | as `set_stylesheet` |
> | `show`, `repaint_all`, `take_damage_for_age`, `retry` | the list or retention only, never the tree | nothing, and nothing is owed |
> | `post`, `click`, `spawn`, `set_spawner`, `run_pending_jobs`, `hold_until_spawner` | the queue or the jobs; results re-enter through the inbox | nothing here; the pump that drains them marks |
> | the public `app`, `styles`, `states`, `input` fields | anything a caller writes | **nothing** — a direct write is the unreported write `Boundaries` cannot see, and the caller reports it with `invalidate` |
> | `shadow_check` (the `shadow-oracle` feature) | **lays the live widgets out again** | nothing — it repairs the frame it judges (R1-F6, M41/T41.2) |
>
> Outside `Host`, the agent's `frame_done` probes every leaf through
> `Node::probe`, whose provided body is `layout`, and so writes a
> `TextInput`'s and a `TextArea`'s scroll with nothing marked (R1-F5,
> M41/T41.4).

Write the inventory as a table in this plan, with gate, strategy and
whether it can exercise one. Then:

- **Convert `just agent-drive`** to loop over both strategies, as the
  other two drive scripts do.
- **Run the invalidation oracle at `Boundaries<256>`** as well as
  `<16>`, so the shipped type is compared byte for byte.
- **Run every example's live path under both strategies**, driven by
  input, since those are the frames rule 1 is about.

**This task is expected to find defects.** Each is fixed in its own
commit with its own injection, in the manner of M26's Phase A, and
recorded here as it lands. If it finds nothing, the commit says what was
run and how many frames were compared, so that "nothing" has a size.

**Gate:** each converted gate shows its injection under `Boundaries`.
For example, `mark` does nothing, and the converted gate fires. **The
injection has to land on a frame that can show it**: a hover, or a
`PointerDown` that no widget consumed, on a node whose hover or pressed
style differs. On a consumed frame `mark_all` hides every mark defect.
Each injected session asserts its own precondition (`pump` returned
`handled == false`) and prints the node that went stale.

## T34.1 — a shadow oracle, reporting rather than panicking

> **DONE 2026-09-24.** `Host::shadow_check` behind the shell's
> `shadow-oracle` feature (forwarded by winit and showcase); the agent's
> `app.shadow` reports `{supported, compared, first}`, and
> `supported: false` from a host without it. Headless gate
> `crates/lumen-static-shell/tests/shadow.rs`: a hover, a click and a
> leave are compared and quiet, with the compared count asserted first;
> *injection* — skip the hover mark — names the button (`node 2`,
> `field: "colour"`) on the hover frame. Live: `just agent-drive`'s
> `Boundaries` leg builds the oracle in and asserts **29 frames compared,
> none diverged**. *(M42/T42.4:* that leg now **fails without the
> oracle** — until then `if sh.get("supported"):` had no else, so a leg
> built without it passed having compared nothing (I-NEW-3). Whether the
> oracle's silence means anything — it repairs what it inspects — is
> M41's, `docs/plan-boundaries-repair.md`. *M41/T41.2b:* the oracle is
> now a third leg, `Boundaries+oracle`, beside a plain `Boundaries` leg
> built without it, so the leg that judges `Boundaries` as it ships is
> not one the oracle repairs.)*
>
> **Its first live run reported a divergence that was the checker's**:
> the host zeroes the focused node's `focus_ring` *after* layout (T15.6),
> so a node the bounded frame skipped kept last frame's zero while the
> scratch re-layout restored the sheet's value — equal pixels, unequal
> state. The oracle now runs after that rule and applies it to its
> scratch too.
>
> **Amended 2026-09-25, M41/T41.2** (`docs/plan-boundaries-repair.md`
> §1.3, §2.2). Two claims in this block and below were false. "Paint
> attributes … are resolved in layout, so this comparison covers them"
> is false for **state a widget keeps for itself** — a `TextInput`'s
> scroll, a `Repeat`'s bound rows — which is not in `NodeState`. And
> the second layout ran **before paint, on the live widgets**, so it
> repaired the frame it judged: the compose, `Repeat` and audit-probe
> cases each reported `first: None` *and* painted `Everything`'s list.
> The oracle now runs after paint and `retain.finish`, paints a scratch
> list through `Scaled`, compares `NodeState` and then the lists, and
> names a list-only difference `field: "paint"`. It still repairs the
> frames *after* the one it judges, and its rustdoc says so. The
> headless gate `tests/shadow.rs` is now the standing test that it can
> fire (four cases, four injections, recorded in T41.2's commit).


This is the one piece of new machinery. It follows from rule 2, and from
M26's lesson that `Boundaries` defects are invisible where people look.

**The mechanism is one host, not two.** `App` is not `Clone`, and a
second host would need a second application running every handler and
spawning every task twice. Instead, after the bounded frame's layout and
place, the host lays out and places **again, under an all-dirty view,
into a scratch copy of `states`**, and compares the two `NodeState`
arrays. Paint attributes such as the hover shade are resolved in layout,
so this comparison covers them. The second pass does not dispatch,
apply, update or spawn, and it leaves `last_damage` and the retained
journal untouched. It does run the layout code again, so measure memos
and `Repeat` binds execute twice. That is the known cost of a diagnostic
mode, and the rustdoc must say so.

**It reports and does not panic.** Once M36 lands, a panic in a debug
window is caught and shown as a panic screen, which would turn the
oracle's finding into a framework-location report nobody asked for. So
the oracle counts compared frames, keeps the first divergence (node
index and the differing fields), and publishes both through a new agent
method. An environment variable can make it abort instead, for a
terminal session.

**It is compiled in by its own cargo feature**, not by
`debug_assertions`. Every drive script builds `--release`, and a
debug-only oracle would be absent from the very drive meant to test it.
That is the "setup silently did not happen" shape.

**Gate:** showcase under the shadow oracle, driven by the agent. The
drive asserts that the compared-frame count is above zero before it
asserts that no divergence was found. *Injection:* skip the precise mark
for a hovered node. The drive's hover step must return a divergence that
names that node.

**Measure:** the second pass's cost per frame, in the rustdoc, citing
where it was measured.

## T34.2 — the flip

> **DONE 2026-09-24.** `Inval` and `Host`'s default are both
> `Boundaries<256>`; `everything` is the diagnostic feature; the three
> drive scripts invert their legs (`everything` for the whole-tree leg).
> **Every headless test that names `Host<A>` now runs `Boundaries`**, and
> `just test` is green — that is the flip's own evidence run. Live:
> `just widget-drive` 94 actions green both ways, `just agent-drive`
> green both ways with the shadow oracle quiet over 29 frames, and the
> tasks drive green both ways. *(Re-run at `6116a8b` by M42/T42.8 with
> each leg's process identified by its socket: all six legs green, two
> distinct binaries per drive — `docs/results-drive-identity.md` §1.)* **Measure**
> (`docs/results-damage-precision.md` T34.2): the prediction held — idle,
> hover and press frames moved; tick and click did not, by design.
>
> **Its 29 frames were judged by the oracle T41.2 replaced** — one that
> repaired every defect it could see before it compared — and are under
> re-validation in `docs/plan-boundaries-repair.md` T41.7. §5 records
> what the re-triage found.


- `Inval` becomes `Boundaries<256>`, and `Host`'s default type parameter
  becomes the same (rule 3).
- The `boundaries` feature becomes a diagnostic `everything` feature
  (rule 2). Any example can reach it as
  `--features lumen-static-winit/everything` without forwarding it.
- In the same commit: the two example manifests; `widget_drive.sh`,
  `tasks_drive.sh` and `agent_drive.sh`, with their legs inverted so
  that the `Everything` leg now asks for `everything`; the winit
  manifest comment; the `Inval` rustdoc; and every comment that says
  `Everything` is the default.

**Gate:** everything T34.0 converted, now running by default, with the
drive scripts still running both strategies.

**Measure:** showcase's frame before and after the flip, by frame class:
hover, unconsumed press, idle tick, click, and a bound-label write. The
instrument is `Host::timings3`, read through the agent, eight runs a
side. **The prediction is written before the flip**: click and
bound-label frames will show no change, because they `mark_all` either
way (§1's last bullet), until M29/T29.6. Hover, press and tick frames
are where the flip can show. The results go in
`docs/results-damage-precision.md`, with the threshold stated beside
them.

## 2. Ordering

- **M34 is independent of M33.** It can run alongside M33.
- **T34.2 must land before M29/T29.6 is measured**, because until then
  the default window runs `Everything`. A `Boundaries` window exists
  today only behind a feature. T29.6 does not need T34.1.
- **T34.1 and M36 are designed to coexist**: the oracle reports through
  the agent, so M36's catch never sees it.

## 3. What this is not

- **Not a change to what `Boundaries` marks.** That is M29.

## 4. What the review found (2026-09-23)

A seven-lens review, with an adversarial verifier per finding,
confirmed nine findings against the first draft. What changed:

| # | finding | what changed |
|---|---|---|
| 1 | `agent-drive` was listed as two-strategy, but it runs `Everything` only | §1 corrected. T34.0 converts it |
| 2 | golden images, the hot-reload demo and GPU parity cannot exercise a strategy, so converting them would have produced vacuous gates | §1: recorded as strategy-free |
| 3 | the shadow oracle had no mechanism, and two hosts would need two applications and would spawn tasks twice | T34.1: one host, with a scratch re-layout |
| 4 | a debug-only oracle would be absent from the `--release` drives | T34.1: its own cargo feature, with a compared-frame count asserted |
| 5 | "panics on the first press" cannot fire, because a consumed press `mark_all`s and an agent click is down and up in one pump | T34.0 and T34.1: the injection targets a hover or an unconsumed press, with the precondition asserted |
| 6 | the grid fixture is headless and already on `Boundaries<16>`, so "before and after the flip" on it measured nothing | T34.2: showcase by frame class, with the instrument, run count and prediction |
| 7 | only the winit alias flipped, and `Host`'s default, the scripts and the comments were missing from the edit list | rule 3. T34.2's edit list |
| 8 | the oracle had never run at the shipped `Boundaries<256>` | T34.0 |
| 9 | "no window uses the strategy" was false (two examples forward the feature), and T29.6 does not need T34.1 | §2 |

## 5. What the re-triage found (2026-09-24)

The re-triage of M38's findings against `main` at 1717808
(`docs/plan-boundaries-repair.md` §1) re-ran this milestone's evidence.
Fixes are M41's; this table records what changed here.

| # | finding | what changed |
|---|---|---|
| 1 | T34.0's inventory missed `Host::compose`, which marked nothing — an S0 in the default (R1-F2, R10-F9) | M41/T41.1 marks the focused subtree; the T34.0 table gained a row per `&mut self` method |
| 2 | the oracle repaired the frame it judged, and could not see widget-internal state (R1-F6) | M41/T41.2: runs after paint and compares display lists; T34.1 amended above |
| 3 | no test showed the oracle could fire: a scratch that cloned the states left `tests/shadow.rs` green (I-NEW-1) | M41/T41.2: three positive cases beside the quiet one |
| 4 | `agent_drive.sh`'s only `Boundaries` leg built the oracle in, so no drive judged `Boundaries` without the repair | **M41/T41.2b, done 2026-09-26**: three legs — `Everything`, a plain `Boundaries` leg that asserts `supported: false`, and a `Boundaries+oracle` leg; the per-leg check is `scripts/live/shadow.py`, one copy for every drive |
| 5 | "29 frames compared, none diverged" was measured on repaired frames | M41/T41.7 gathers it again |
