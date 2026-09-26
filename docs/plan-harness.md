# M12 — the test harness

*Planned 2026-09-04. Was Phase E of `docs/plan-lumen-parity.md`.
Status: **DONE 2026-09-20** — T12.3 and T12.4 landed 2026-09-04, T12.1
and T12.2 on 2026-09-20. `docs/results-harness.md` owns the numbers and
the injections.*

*The premise held: the substrate was finished, so it stayed cheap. What
was missing was ergonomics and artifacts, and that is all that was
built.*

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
- **A live gate** (`scripts/ui_check.py`, `scripts/live_gate.sh`) that
  opens a real window and asserts. The check list lives in the script
  and is deliberately not restated here — this sentence said "seven"
  and the justfile said "six" while there were eight.

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

## T12.1 — a locator and expectation API — **DONE 2026-09-20**

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
same things. **Landed as `--drive2`, and both are kept**, because the
comparison is the evidence: 7 hand-written clicks against 21 harness
steps, with the hand-written version needing a `centre` closure that
assembles `ui.getLayout` JSON and unwraps four array indices. Each of
the three properties above was injected for real —
`docs/results-harness.md` has the three failure messages.

## T12.2 — golden images with a diff artifact — **DONE 2026-09-20**

The *comparison* exists: `damage_oracle.rs` compares whole pixel
buffers. What it does on failure is `assert_eq!` two 264 KB `Vec<u8>`s
— there is not even a count of differing pixels, let alone a picture.
What is missing is the harness around it: storing a reference, updating
it deliberately, and producing something to look at when it fails.

- Store under `tests/golden/` as **PPM**, which `Canvas::to_ppm`
  already writes and which needs no dependency at all. Not the PNG
  encoder in `lumen-static-agent`: that crate depends on
  `lumen-static` with `features = ["observe"]`, not optional, and Cargo
  unifies features across a selection — so a dev-dependency on it from
  `lumen-static-shell/tests` would switch `observe` on for
  `alloc_gate.rs`'s process and grow `NodeDesc` from 48 B to 72 B. That
  is the same unification hazard M13's feature polarity is argued from.
- `LUMEN_UPDATE_GOLDENS=1` to re-record, as Lumen does.
- On mismatch write `<name>.actual.png` **and `<name>.diff.png`** — red
  over a dimmed base. The diff is the part that makes a golden usable;
  a bare "1,482 pixels differ" is a number, not a diagnosis.

**Three rules to write down with it:**

- Goldens are for the **software renderer only**, and it is
  deterministic: pure arithmetic plus `sqrt`, which IEEE-754 requires
  correctly rounded, and no GPU path to diverge from.
- **Deterministic is not portable, and the difference is the whole
  trap.** Under the `truetype` feature — which is what a window and the
  live gate actually run — `FontStack::system` takes *the first of five
  candidate faces that exists on the machine*, silently serves both
  weights from the regular face when no bold is found, and depends on
  `fontdue = "0.9"`, a caret range. A golden recorded that way records
  which fonts the recording machine had installed. So: **goldens are
  recorded and compared with `truetype` off**, and the runner refuses to
  record or compare with it on. Otherwise `LUMEN_UPDATE_GOLDENS=1`
  becomes a button that bakes in the author's font install — the exact
  "masked by a re-recorded golden" failure the next rule warns about.
  `text.rs` already says this in its own module doc: "a deterministic
  measurer is what makes golden geometry assertions possible at all."
- **A golden is not a substitute for a property.** `paint_containment`
  and `damage_oracle` catch classes of bug across a corpus; a golden
  catches one frame changing. The bugs this project actually shipped —
  a repeater painting 44 px outside its viewport, a renderer ignoring
  clips, a skipped present — were caught by properties and by the live
  gate, and would have been *masked* by a re-recorded golden.

**Gate:** deliberately break a paint (change a radius by 1 px) and check
that the diff image shows where, not merely that. **Landed**: a 1 px
radius change on a 120×80 frame is `92 of 9600 pixels differ, inside
[20, 16, 100, 64]`, and the diff image marks exactly those 92 — at the
rounded rect's four corners and nowhere else, which is the only place a
radius change can move a pixel. `docs/results-harness.md` prints the
map. Artifacts are PPM, not PNG, for the feature-unification reason
above; `LUMEN_UPDATE_GOLDENS=1` re-records; a first run **records and
does not pass**; and `compare` panics under `truetype` rather than
baking in the recording machine's font install.

## T12.3 — session recording as a test artifact — **DONE 2026-09-04**

> **Amended 2026-09-26 (M48/T48.10, R13-F6): what shipped here could not
> fail.** `replay` discarded every reply, `check` took no stylesheet
> though the header below lists it, and `export_test` emitted `let _ =`
> per step and a placeholder comment. A corrupted recording replayed
> `Ok`, a mismatched sheet replayed `Ok`, and an export clicking a missing
> selector passed. Each step now keeps its reply's `handled` and error
> code, a divergence is an error naming the step, the stylesheet is
> checked, and an export asserts every step and a final `ui.getTree`
> digest (`crates/lumen-static-agent/tests/session_replay_fails.rs`).

The invariant holds and is **already exercised for a single click at
the `Event` level** — `crates/lumen-static-agent/tests/input.rs`
replays a synthesized click's trace on a fresh host and asserts the same
state, and `docs/plan-agent-parity.md` says so. An earlier draft of this
task claimed nothing exercised it at all, which was wrong. What is
unexercised is a **multi-step session, recorded at the RPC level,
written out, and compiled**.

- Record every `input.*` the agent dispatches, behind `observe`.
- Emit it as a runnable Rust test — Lumen's `Session::export_test` is
  the reference for the shape.
- Replay against a fresh host and assert the same final state.

**A `(method, params)` list is not sufficient to replay, and the task
must carry a header.** Four things it does not capture, each of which
silently changes where a replayed click lands:

- **Viewport size.** A selector resolves to the node's laid-out centre,
  so a different viewport moves it.
- **The stylesheet**, which is not an `input.*` call at all. Padding,
  radius and `font_size` reach `NodeState` and therefore the geometry —
  and M8c exists to make mid-session swaps cheap, so a recording must
  capture them too.
- **Initial app state.** `live_gate.sh` warms the window with twenty
  clicks before any check runs, so a recording taken from a live session
  does not start at `Default`. Either the recorder refuses to start
  after frame 0, or the header says where it started.
- **The text measurer.** Label width → node size → node centre → the
  coordinate a replayed click uses. This repository has already been
  burned by exactly that mismatch (`docs/results-frame-cost.md`: the
  audit measured with the block stand-in while the window laid out with
  Noto).

Timing is *not* on the list, correctly: there is no frame clock. The
residue is that an exported test must not assert on frame numbers,
`app.perf` figures, or `input.scrollIntoView`'s step count.

**DECIDED: the RPC level.** A selector survives layout drift — restyle
the button, move it, and `#inc` still names it — where a recorded
coordinate freezes the geometry of the machine that recorded it, and the
first layout change invalidates every step. The mirror-image failure is
that a selector breaks when an author *renames* a node, and that is the
better one: it is loud, and it is a real change to the app rather than
an incidental one.

**Gate:** the `--drive` session, recorded, exported, compiled and run as
a generated test, reaching the same `count`; and a recording whose
header does not match the replay host is a **hard error, not a diff**.
If the generated test does not compile, the recording is not a script.

## T12.4 — a trace to read when a failure is not obvious — **DONE 2026-09-04**

Lumen writes `target/lumen-traces/<test>.trace.jsonl` — inputs, damage,
tree snapshots, and on failure an embedded screenshot and tree. We have
every one of those as an agent method already (`ui.lastChange`,
`ui.lastDamage`, `app.perf`, `ui.getTree`, `ui.screenshot`,
`ui.explain`); what is missing is writing them out at the moment of
failure instead of making the reader re-run and re-query.

`LUMEN_TRACE` already exists in `examples/counter-window` for two
stderr redraw dumps and appears in no document. Either subsume it or
name the new one distinctly; do not ship two.

**The one addition worth making over Lumen's:** on failure, call
`ui.explain` on the selector that failed and put the answer in the
trace. "I clicked `#save` and nothing happened" is the commonest test
failure there is, and this repository already answers it by running the
real hit test.

**Gate:** a deliberately failing test writes a trace containing the
failing selector's `ui.explain` output, and the reason is legible from
the file alone without re-running.

## The open question, answered — **DECIDED 2026-09-04**

It said to decide "when M11 is underway and the panel is real, not now."
The panel is real, so: **T12.3 and T12.4 are built; T12.1 and T12.2
stay unbuilt for now.** The review's condition for T12.1 — that a second
application exist — is met by the settings panel, so it is no longer
*unjustified*; it is simply not the next most valuable thing, and saying
so beats building it because a plan once listed it.

The argument, kept because it is the reasoning and not just the verdict:

## The original open question: should this be two tasks, not four?

The review argued for cutting T12.1 and T12.2, and the argument is good
enough to record rather than answer here:

- **T12.1 (locators)** builds ergonomics for application authors who do
  not exist yet. Its gate is "rewrite `--drive`" — one existing session,
  against `scripts/agent_client.py`, which already does this.
- **T12.2 (goldens)** is, by this document's own argument two sections
  up, the item that catches least: every bug this project actually
  shipped was caught by a property or the live gate and would have been
  *masked* by a re-recorded golden.
- **T12.3 and T12.4** are the two that retire something real — an
  unexercised claim, and a diagnosis (`ui.explain` into a failure trace)
  available no other way.

The counter-argument is that T12.1 is what makes a *second* application
cheap to test, and M11's growing settings panel is that application. So
the honest sequencing may be: **T12.3 and T12.4 whenever; T12.1 when the
panel exists; T12.2 reduced to giving `damage_oracle` a diff artifact**,
which improves a test that already earns its keep. Decide when M11 is
underway and the panel is real, not now.

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
