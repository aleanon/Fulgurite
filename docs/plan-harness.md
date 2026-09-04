# M12 — the test harness

*Planned 2026-09-04. Was Phase E of `docs/plan-lumen-parity.md`. Status:
**not started**, and last of the three — not because it is least
valuable, but because its substrate is already finished, so it stays
cheap however long it waits.*

## What already exists

Phase E said "we have the agent protocol and the one input queue, which
is most of the substrate — a session already replays. This is small and
should ride along." That was true then and more true now:

- **One input queue.** Every synthesized input enters the same queue as
  OS input (E4/P4), so a session replays by construction.
- **233 tests** across 37 files, including the ones that matter most:
  a pixel-exact oracle (`shell/tests/damage_oracle.rs`), a
  containment property test over a fixed corpus
  (`tests/paint_containment.rs`), and an allocation gate.
- **The whole agent protocol**: selectors with teaching errors,
  `ui.getLayout`, `ui.explain`, `ui.getAppliedStyles`, `ui.lint`,
  `app.perf`, `ui.screenshot` from both the canvas and the surface.
- **A live gate** of seven checks (`scripts/ui_check.py`,
  `scripts/live_gate.sh`) that opens a real window and asserts.

**What is missing is ergonomics and artifacts, not mechanism.** Every
test above was written by assembling JSON-RPC by hand or by calling
`pass::*` directly. That is fine for framework tests and wrong for
application tests, which is the audience this milestone is for.

Lumen's `lumen-test` is 931 lines and — unlike `lumen-render`,
`lumen-text` and `lumen-style` — **its API shape is worth copying even
though the crate is not adoptable** (it depends on `lumen-core`,
`lumen-render` and `lumen-widgets`). Its `Session` is
`click`/`fill`/`press`/`expect_text`/`expect_value`/`expect_state`, and
its `Tracer` writes JSONL with a screenshot embedded on failure.

## T12.1 — a locator and expectation API

A test should read like the thing it tests:

```rust
let mut app = TestApp::new(counter(), (320, 200));
app.click("#inc")?;
app.expect_text("#value", "1")?;
```

built on `Agent::dispatch_input` and the selector grammar that already
exists. Three properties it must have, all of which this repository has
learned the hard way:

1. **Errors teach.** `resolve_one` already returns `NotFound { nearest }`
   and `Ambiguous { candidates }`; the harness must surface those rather
   than flattening them to "assertion failed".
2. **Assertions state the semantic first.** The verification skill's
   rule — "state + semantics + bounds first, pixels second" — is a
   property of the API, not of the person using it: `expect_text` should
   be easier to reach for than a screenshot comparison.
3. **No auto-wait, and say so.** There is no animation scheduler and
   `ui.waitSettled` is trivially true; a retry loop here would be
   machinery pretending to solve a problem this architecture does not
   have. If one ever does, it goes in with the scheduler.

**Gate:** `examples/counter-window --drive` — the existing hand-written
agent session — rewritten against the harness, shorter and asserting the
same things.

## T12.2 — golden images with a diff artifact

The *comparison* exists: `damage_oracle.rs` compares whole pixel buffers
and fails with the count of differing pixels. What is missing is the
harness around it: storing a reference, updating it deliberately, and
producing something to look at when it fails.

- Store under `tests/golden/`, PPM or PNG via the dependency-free
  encoder already in `lumen-static-agent::png`.
- `LUMEN_UPDATE_GOLDENS=1` to re-record, as Lumen does.
- On mismatch write `<name>.actual.png` **and `<name>.diff.png`** — red
  over a dimmed base. The diff is the part that makes a golden usable;
  a bare "1,482 pixels differ" is a number, not a diagnosis.

**Two rules to write down with it**, both already learned here:

- Goldens are for the **software renderer only**, and it is
  deterministic — there is no GPU path to diverge from.
- **A golden is not a substitute for a property.** `paint_containment`
  and `damage_oracle` catch classes of bug across a corpus; a golden
  catches one frame changing. The bugs this project actually shipped —
  a repeater painting 44 px outside its viewport, a renderer ignoring
  clips, a skipped present — were caught by properties and by the live
  gate, and would have been *masked* by a re-recorded golden.

**Gate:** deliberately break a paint (change a radius by 1 px) and check
that the diff image shows where, not merely that.

## T12.3 — session recording as a test artifact

The invariant already holds: every synthesized input goes through the
one queue, so any agent session *is* a replayable script. **Nothing
writes the recording out**, so "an agent session replays as a test" is
true and unexercised — the precise shape of claim this project keeps
finding and refusing elsewhere.

- Record `(method, params)` for every `input.*` the agent dispatches,
  behind `observe`.
- Emit it as a runnable Rust test — Lumen's `Session::export_test(fn_name,
  header) -> String` is the reference.
- Replay it against a fresh host and assert the same final state.

This is the task that retires the last unexercised claim in the parity
document, and it is the reason `app.setValue` was refused: a method that
wrote state behind the queue would produce sessions that do not replay,
and this is what makes that consequence visible instead of theoretical.

**Gate:** the `--drive` session, recorded, exported, compiled and run as
a generated test, reaching the same `count`. If the generated test does
not compile, the recording is not a script.

## T12.4 — a trace to read when a failure is not obvious

Lumen writes `target/lumen-traces/<test>.trace.jsonl` — inputs, damage,
tree snapshots, and on failure an embedded screenshot and tree. We have
every one of those as an agent method already (`ui.lastChange`,
`ui.lastDamage`, `app.perf`, `ui.getTree`, `ui.screenshot`,
`ui.explain`); what is missing is writing them out at the moment of
failure instead of making the reader re-run and re-query.

**The one addition worth making over Lumen's:** on failure, call
`ui.explain` on the selector that failed and put the answer in the
trace. "I clicked `#save` and nothing happened" is the commonest test
failure there is, and this repository already answers it by running the
real hit test.

**Gate:** a deliberately failing test writes a trace containing the
failing selector's `ui.explain` output, and the reason is legible from
the file alone without re-running.

## What M12 is not

- **Not `lumen-test` ported.** The crate depends on `lumen-core`,
  `lumen-render` and `lumen-widgets`; only its API shape transfers.
- **Not a replacement for the live gate.** `scripts/live_gate.sh` opens a
  real window; a headless harness cannot see a blit, a present or a
  buffer age. The 2026-09-04 regression — another window's pixels on
  screen while six headless-equivalent checks passed — is the standing
  argument for keeping both.
- **Not a benchmark harness.** `benches/` and `app.perf` cover that, and
  a threshold in a test is how a machine-dependent number becomes a
  flaky gate.
