# M34 — `Boundaries` by default, once the evidence says it is safe

> **Written 2026-09-23, not started.** The decision to flip the default
> was made on 2026-09-23. This milestone gathers the evidence the flip
> needs, then makes it.
>
> `lumen_static_winit::Inval` is `Everything` unless the non-default
> `boundaries` feature is set. So **every invalidation win since M16 is
> worth nothing in a turnkey window**, including M29's
> (`docs/plan-damage-precision.md` §1). The feature exists because
> nothing outside `lumen-static`'s own tests had ever instantiated
> `Boundaries` (M27/T27.3).
>
> The reason to be careful is recorded, not guessed. **Four of M26's
> five Phase A defects were invisible under `Everything`**, because a
> clean subtree is skipped before a wrong answer can be observed
> (`docs/plan-frame-and-types.md`). M25/T25.0 was the same shape. The
> strategy that saves the time is also the one that has been exercised
> less.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red.*

---

## 0. The rules for this milestone

1. **Inventory before the flip.** Every gate that runs under only one
   strategy is listed, with the strategy it runs under, before the
   default changes. A gate that runs only under `Everything` has said
   nothing about the frames a user will now see.
2. **Keep an escape hatch, named for its purpose.** The feature that
   selects `Everything` after the flip is a *diagnostic*. If a stale
   frame goes away when you build with it, you have found an
   invalidation defect. The rustdoc says that, in those words.
3. **Measure the turnkey window, not the arm.** The claim this
   milestone makes is about a window opened with `run` or
   `application(..).run()` (M32).

## 1. What exists today

- `just widget-drive`, `just agent-drive` and the tasks drive
  (`scripts/tasks_drive.sh`) each run under **both** strategies.
- `crates/lumen-static-shell/tests/invalidation_oracle.rs` compares
  byte-identical frames under both, over a driven session that hovers,
  presses and includes an enum.
- `examples/showcase` and `examples/tasks-demo` forward a `boundaries`
  feature. No other example does.
- The golden images (`crates/lumen-static-shell/tests/golden.rs`, M12)
  and the GPU parity gate have **not been checked** for the strategy
  they run under. That is T34.0's first question.

## T34.0 — the inventory, and running what has only run one way

List every test, gate and drive script with the strategy it runs under.
Wherever a gate ran under `Everything` only, and the property it checks
could differ under `Boundaries`, run it under both. That covers golden
images, GPU parity, the hot-reload demo, every example's headless frame,
and the M29 grid fixture.

**This task is expected to find defects.** Each one found is fixed in
its own commit with its own injection, following M26's Phase A, and
this plan records them here as they land. If it finds nothing, the
commit says what was run and how many frames were compared, so that
"nothing" has a size.

**Gate:** none of its own. It turns existing gates into two-strategy
gates. Each one it converts shows its injection under `Boundaries`:
break the mark (for example, `mark` does nothing), and the converted
gate must fire.

## T34.1 — a shadow oracle for debug windows

This is the one piece of *new* machinery. It follows from rule 2 and
from M26's lesson that `Boundaries` defects are invisible where people
look. In a debug build, with an environment variable set, the window
renders each frame under both strategies and panics with the first
differing node when the frames disagree. `examples/showcase` under the
shadow oracle, driven by the agent, is the gate.

*Injection:* skip marking the target of a press. The shadow oracle
panics on the first press, naming the node.

**Measure:** what the check costs per frame. It is off by default, and
its rustdoc must state the cost.

## T34.2 — the flip

`Inval` becomes `Boundaries<256>` by default. The `boundaries` feature
becomes a diagnostic `everything` feature, per rule 2. The examples'
forwarding features change with it.

**Gate:** everything T34.0 converted, now running by default. The
drive scripts keep running both strategies.

**Measure:** a turnkey window's frame before and after the flip, on the
grid fixture and on showcase. Record it in
`docs/results-damage-precision.md` beside T29.1's arms. That result
answers "what did M16 through M29 buy a user", which nothing has
measured yet.

## 2. Ordering

M34 is independent of M33, and it can run in parallel with it. **It
must land before M29/T29.6 is measured**, because T29.6's number is
meant to describe a window, and until the flip no window uses the
strategy it narrows.
