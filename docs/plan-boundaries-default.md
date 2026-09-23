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
