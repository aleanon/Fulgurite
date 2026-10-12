# M48 — the agent says what is true, and costs nothing when unused

> **T48.0–T48.14 DONE 2026-09-27**, including `input.compose`'s live
> leg, built once M41/T41.2b reached the base. §6 records what building
> found.
>
> - **T48.0** — every finding red at `826c7c3`: nine checks committed
>   `#[ignore]`d or unwired; the live rows red in their own tasks.
> - **T48.1** — a reply's `frame` is `Agent::reply_frame` (the journal's
>   next); clicks 2–5 went 0 → 1 change. `input.paste` (M45) too.
> - **T48.2** — `ui.waitSettled` from the host, `null` when it cannot
>   say; pending from M47/T47.3's ledger less running subscriptions.
> - **T48.3** — `app.perf` fills from the presented frame; unmeasured is
>   `null` and NOT MEASURED, not 0 ns. Live frame p50 ~40 µs.
> - **T48.4** — a repainting action with no damage fails the drive;
>   presented px printed beside the journal ratio; three defects fixed.
> - **T48.5** — one serialiser, `observe::node_json`: fields 0 → 6 / 6,
>   dump and `ui.getTree` byte-identical.
> - **T48.6** — `ui.resolve` carries them; nine widgets gained `observe`
>   (11 generic → 2, both allowed).
> - **T48.7** — stand-ins by structure (`Node::node_count`, required):
>   nested wrappers `live` all true, 1/1 changes.
> - **T48.8** — `frame_done` allocations 4.04 → 0.00, asserted.
> - **T48.9** — core `observe` without the widgets' does not compile;
>   it found the shell's, winit's and stamp-lib's graphs.
> - **T48.10** — a diverging replay is `Diverged { step }`; the sheet is
>   checked; an export asserts every step and a final tree digest.
> - **T48.11** — `FULGURITE_AGENT_ADDR`, read by the window alone; 8 → 0
>   readers (and glasswall's, after the rebase).
> - **T48.12** — no endpoint, no journal (3 → 0 of 3 frames); 0 agent
>   symbols without the feature (66 with).
> - **T48.13** — `app.quit` in every window (exits in 7–32 ms, status 0,
>   every drive); `input.compose`, headless and live — the oracle leg
>   judged 13 frames over the compose steps, none diverged, and names
>   `#d_textinput_a` when T41.1's mark is dropped.
> - **T48.14** — code, skill and book: 33 / 33 / 33, checked in
>   `static`.

> **Written 2026-09-24.** Closes R12-F3, R13-F1, R13-F2,
> R13-F3, R13-F4, R13-F5, R13-F6, R13-F8, R13-F9, R13-F10, R13-F11,
> R14-F4, R14-F8 and R15-F10, from M38's deep review
> (`docs/results-deep-review.md`) as re-triaged against `1717808` on
> 2026-09-24. **Phase 3.** It comes after M39, which it depends on
> twice: no new method lands before the transport refuses a browser,
> and T48.4's cargo-level gate extends M39's event-loop test. It also
> comes after M41 and M42, and after M40, so the presented damage T48.4
> prints beside the journal's is in the surface's physical pixels. (M40
> §2.1 found that `ui.lastDamage` and the drive's 1.00× ratio are
> journal damage in logical pixels and do not change units.) It builds on M35's static observe walk (`observe::emit_root`,
> T35.1, `docs/plan-observe-cost.md`). That walk already gives the
> agent and `observe::dump` one traversal. What it did not give them is
> one serialiser.
>
> The review's verdict on goal (3), "a built-in agent for observability
> and control", was **not met**. M39 owns the S0 that makes the agent
> unsafe to leave running. This milestone owns the rest: the answers
> that are wrong. `ui.waitSettled` says `true` in every state. A
> click's reply names a frame that `ui.lastChange` cannot find.
> `app.perf` reports zeros that a budget check passes. The tree drops
> the checked state a screen reader announces. A replayed session
> passes with a missing selector. Each answer is well-formed, so a
> caller trusts it. That is why the review scored each one as a gate
> that proves nothing, rather than as a missing feature.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and prints the measured quantity beside its bound.*

---

## 0. The rules for this milestone

1. **"Cannot say" is an answer; a default that agrees is not.** Where
   a host cannot know something, the protocol answers `null`, never the
   value that makes a check pass. This is the provided-method trap
   AGENT.md names, applied to a reply: `settled: true`, `frame_ns: 0`
   and `Ok(())` from a replay are each a default that agreed.
2. **One source per fact.** One frame counter, one serialiser for a
   node, one environment variable for the address, and one list of
   method names that the skill and the book are checked against.
3. **New methods wait for M39/T39.1.** Of the names this milestone
   touches, only `app.quit` (T48.13) is a new method, and it can end
   the process, so it is the one that must not reach a page. Fields
   added to existing read methods (T48.2, T48.5, T48.6) and the
   corrected frame number (T48.1) add no reach, because a page cannot
   read a reply. They still land after M39 by phase.

## 1. What is wrong, at `1717808`

### 1.1 Synchronisation answers that cannot be relied on

- **One frame, two counters** (R13-F3, S2). An `input.*` reply takes
  its `frame` from `Live::pump`, which returns
  `self.host.frame_number()` (`crates/fulgurite-shell/src/session.rs:497`).
  `ui.lastChange` and `ui.lastDamage` count `Agent::frame`, which
  advances once per `frame_done` (`crates/fulgurite-agent/src/lib.rs:380-381`).
  The host counter moves twice per input RPC, once for the pump inside
  the RPC and once for the redraw after it, so the two drift apart.
  The re-triage clicked `#spawn-one` three times. The replies said
  frames 20, 24 and 28, while `lastDamage` answered from 18, 21 and 24,
  and `since = reply.frame - 1` found nothing each time.
- **`ui.waitSettled` is a constant** (R13-F1, S1). It is
  `Ok(json!({"settled": true, ..}))` under the comment "No animation
  scheduler exists" (`lib.rs:808-811`). The re-triage clicked
  `#start-slow` and got `settled: true` while `#status` still read
  `last: `; 1.2 s later it read `last: slow=99`. Since the comment was
  written, the window loop has gained both reasons to be unsettled,
  `animating || more_deferred()` (`crates/fulgurite-winit/src/lib.rs:532`)
  and `next_deadline_ms()`, and it reports neither.
- **`app.perf` reports zeros in every turnkey window** (R13-F2, S2).
  `Session::agent_frame_done` builds `FrameTiming::default()`
  (`session.rs:429`). The window's `redraw` throws away the filled
  frame that `present` returns (`crates/fulgurite-winit/src/lib.rs:500-504`).
  So every stage reads 0, and `scripts/ui_check.py`'s frame and
  regression budgets pass on those zeros (R13-F2).
- **Nothing notices a stale journal** (R13-F8, S2). `answer_agent`
  ends with `request_redraw()` (`lib.rs:650-652`), and that redraw is
  the only thing that runs `frame_done` after an input. No cargo test
  drives `answer_agent → redraw → frame_done`:
  `crates/fulgurite-winit/tests/` holds only `gpu_regions.rs` and
  `translate.rs`. The drive harness skips its damage bound when no
  region overlaps the widget (`scripts/drive/harness.py:389-390`,
  `mine = ..; if mine:`). So the review's injection `1beed32`, which
  deletes the redraw, left `just widget-drive` at exit 0 with no damage
  lines. Its verifier found that, even on the healthy build, 2 of
  Button's 4 actions print no damage line.

### 1.2 The tree drops what the widgets say

- **Two hand copies of one serialiser, both lossy** (R15-F10, S3).
  M35 moved `get_tree` onto `observe::emit_root`
  (`lib.rs:820-857`), so the agent and `observe::dump`
  (`crates/fulgurite/src/observe.rs:179-214`) walk one way. The
  JSON is still built twice, and both copies write only id, role,
  value, label, bounds, children, src and wid. Several things are set
  on the node and dropped on output: `toggled` (`toggle.rs:274`),
  numeric value and step (`slider.rs:253-256`, `field.rs:290-293`),
  `expanded` (`choose.rs:665`, `pick.rs:845`), `modal` and `hidden`
  (`observe.rs`'s `emit_node`). `dump`'s rustdoc still says the two
  "cannot drift, one being derived from the other".
- **A checked box and a moved slider are invisible** (R13-F4, S2).
  This is the same loss seen from the other side. `ui.resolve` and
  `ui.getLayout` go through `Index::summary_map`, which writes the
  same fixed set. Separately, eight widgets have no `observe` impl, so
  they read as `GenericContainer` with no text: `Badge`, `Chip`,
  `Avatar` (`trim.rs`), `ProgressBar`, `Spinner` (`indicator.rs`), and
  `Card`, `Toast`, `Tooltip` (`surface.rs`). The re-triage counted
  zero `fn observe(` in each of those.
- **The stand-in rule has a buildable residue** (R13-F5, S2).
  `live_nodes` (`lib.rs:121-148`) calls a node a stand-in when
  `node(i)` has the same address *and* kind as its parent's. Its
  rustdoc says the residue "needs a type to contain itself". But
  `Glass<Glass<Label>>` puts the inner glass at offset 0 with the same
  kind, `"glass"`. The re-triage re-ran the review's probe `c9d6760`:
  `live = [true, true, false, false]`, and `lastChange` reports 0
  changes where the control reports 1. The verifier found the same for
  `Card<Card<..>>` and `ScrollView<ScrollView<..>>`. Since M34,
  `Boundaries` is the default, so the journal this blinds now guards
  the default path.
- **Four allocations per `frame_done`, against a documented zero**
  (R14-F4, S2). `live_nodes` allocates its identity `Vec` on every
  call, and both callers make a fresh `live` vector
  (`crates/fulgurite-agent/src/journal.rs:170`, `audit.rs:557`).
  The re-triage's `benches --bin agent` printed 4.00 with and without
  findings. The bench prints; nothing asserts.
- **A second feature is silently required** (R12-F3, S3). The
  `fulgurite-winit` `agent` feature
  (`crates/fulgurite-winit/Cargo.toml:48`) builds a working agent
  that reports every crate widget as `GenericContainer` unless the
  application also turns on `fulgurite-widgets/observe`. The
  re-triage re-ran probe `3635061`: `GenericContainer` without the
  second feature, `Label` with it, and no warning in either build.
  `docs/results-widget-crate.md` ("One feature seam the move exposed")
  records the convention, and nothing enforces it.

### 1.3 A replay that cannot fail

**R13-F6, S2.** `Recording::replay` discards every reply
(`crates/fulgurite-agent/src/session.rs:171`, `let _ =`).
`Recording::check` (`:136`) takes no stylesheet, although the module
doc lists the stylesheet as one of the header fields whose mismatch is
"a hard error rather than a diff". `Recording::export_test` emits
`let _ =` for each step and ends with a placeholder comment (`:197`,
`:201`). The re-triage re-ran probes `580f9f6`: a corrupted recording
replays `Ok` (fresh `n = 1` against the recorded 3), a mismatched
sheet replays `Ok`, and an exported test with `#does-not-exist`
compiles and passes.

### 1.4 The surface as documented is not the surface as built

- **`agent_client.py quit` reports success where there is no
  `app.quit`** (R13-F9, S3). The turnkey window answers `-32601`, and
  `quit()` maps both RuntimeError and SystemExit to
  `{"ok": True}` (`scripts/agent_client.py:138-142`). Only
  counter-window implements the method.
- **Two environment variables for one address** (R13-F10, S3).
  Launchers read `FULGURITE_AGENT`: showcase (`examples/showcase/src/main.rs:737`,
  defaulting to 7654), tasks-demo (`examples/tasks-demo/src/main.rs:328`),
  `fixtures/hot-window/src/main.rs:9`, and
  `fixtures/panic-app/src/main.rs:13`. The re-triage missed the last
  one. counter-window (`examples/counter-window/src/window.rs:410`),
  `scripts/agent_client.py` and `scripts/drive/run.py` read
  `FULGURITE_AGENT_ADDR`, and the skill names only `FULGURITE_AGENT_ADDR`. So
  `FULGURITE_AGENT_ADDR=… showcase` binds 7654 while the client dials
  somewhere else.
- **The documented method list is stale, and WORSE since the review**
  (R13-F11, S4). The agent crate's module doc (`lib.rs:5-8`, "In-process
  dispatch only … input synthesis waits for the event system") is
  stale, and so is its repeater note (`:24-26`). So are the
  `waitSettled` comment (`:809`) and `AgentHost`'s "Nothing in the
  workspace implements this outside a test mock" (`:1392`); `Live` in
  `session.rs` implements it. **It is worse than the review found**
  because M34, M36 and M37 added `app.shadow`, `app.failure`,
  `app.retry` and `app.reloads`, and the skill's method list
  (`.claude/skills/verifying-apps/SKILL.md:88-95`) names none of them,
  nor `app.setPalette`. It still lists `app.quit`.
- **An agent build pays for the journal with no endpoint** (R14-F8,
  S3). `redraw` calls `agent_frame_done` on every frame
  (`crates/fulgurite-winit/src/lib.rs:525`). `Local` runs it under
  `cfg(feature = "live-agent")` alone (`session.rs:422-433`), with no
  check that `Config::agent` is set. The rustdoc on `Config::agent`
  claims inertness only "without the feature", so the doc is accurate.
  What is missing is the runtime gate, and a desktop check that a
  non-agent binary links no agent symbols.

## 2. Decisions

### 2.1 Which frame a reply names

Options: (a) the reply reports the journal frame its effect lands in,
`Agent::frame + 1`; (b) `Live::pump` calls `frame_done` itself, so the
two counters advance together; (c) keep the host counter and teach
callers to translate. **(a)**, because the reply exists to be passed
to `since`, and only the agent's counter means anything to `since`.
(b) doubles the journal's work on every input and still leaves a
redraw's `frame_done` to reconcile. (c) is the defect, documented.

`Agent::dispatch_input` takes `&self`, so (a) needs no stored state:
the reply is computed as `self.frame + 1` when it is built. That
differs from the re-triage's sketch, which recorded a pending frame.
Two inputs answered before one redraw both name the same frame,
correctly. The host's number stays in the reply as `host_frame`, for
debugging. The skill states the contract: *the reply's `frame` is the
first journal frame that includes this input; ask with
`since = frame - 1`.*

### 2.2 What "settled" means

`AgentHost` gains a provided method, `settled`, returning `None`. The
protocol reports that as `settled: null`, meaning "this host cannot
say". A default of `false` would make every poll spin to its timeout;
`true` is today's defect. `Live` answers from the host. It is settled
when all of these hold: the last frame's `animating` is false,
`more_deferred()` is false, the inbox is empty, no spawned job is in
flight, and no deadline falls within one frame. The reply lists each
term, so a caller that is told "no" learns why.

- **Subscriptions are excluded and counted.** A recurring timer would
  make a subscribed application never settle. It is reported as
  `subscriptions: n` instead.
- **Nothing counts in-flight jobs at `1717808`.** `Threads` spawns and
  forgets; `Queued::pending` exists only on the test spawner. T48.2
  adds a count to the host, incremented where it spawns and decremented
  where the inbox delivers. That is one integer per spawn, off the
  frame path.
- **No blocking wait.** A `timeout_ms` parameter would need the reply
  deferred on the loop. That is a separate change; polling stays the
  contract, as the skill's "actions do not auto-wait" says.

### 2.3 How a stand-in is recognised

| option | how | verdict |
|---|---|---|
| (a) the derive and each wrapper generate a per-index liveness answer | the container that decides to stand in also says so | correct, but it is a new walk that every hand-written wrapper must forward. A wrapper that forwards wrongly silently stops journaling what is beneath it: M26 finding 9's shape |
| **(b) structural:** `node(i)` is a stand-in iff the object it returns has a subtree *larger* than the table's subtree at `i` | a stand-in is always an ancestor object, and an ancestor's subtree strictly contains `i`. A live node's object has exactly the table's subtree. This is a fact about the table, not an inference from addresses | **chosen** |
| (c) make a wrapper's kind depend on its content | `Glass<Glass<..>>` would get a distinct kind | rejected: kinds are `&'static str` constants, and the change only moves the residue |

(b) needs one thing the `Node` trait cannot answer today: an object's
own node count, dynamically. `NODE_COUNT` is a `Widget` constant, and
`Node` is object-safe. So `Node` gains, under `observe`, a method that
answers `Self::NODE_COUNT`. It is **required**. No default can be
right, because `Node` cannot name `Self::NODE_COUNT`, and a guessed
default is the provided-method trap. `leaf_common!` and the derive emit
it. Each hand-written impl writes one line that returns its own
constant, and cannot forward it wrongly, because it names no child.
The compiler lists every impl that needs it. The table-side subtree
sizes come from one reverse pass over `NodeDesc`, cached per table.
That pass also removes the identity vector R14-F4 counts.

What it breaks: nothing outside `observe`. M35's `just
observe-inlining` must stay green, because this adds a vtable slot
without coercing any row. T48.7 runs it.

### 2.4 How the feature seam fails

Options: (a) a compile-time assertion in `fulgurite-widgets`: if
`fulgurite` has `observe` and this crate does not, stop with a
message naming `fulgurite-widgets/observe`; (b) make the winit
`agent` feature forward it, which needs winit to depend on the widgets
crate only for that; (c) a runtime warning in `ui.getTree`. **(a).**
It fires in exactly the unified graph that has the defect, and it is
as loud as the reload seam's E0277, which the review cited as the
contrast (R12-F3). (b) adds a dependency to fix a flag. (c) is a
fallback, not a fix. For (a), `fulgurite` exports a public boolean
constant that says whether `observe` is on, since a dependent crate's
`cfg` cannot see another crate's features.

What it breaks: `cargo test -p fulgurite-agent`. That build's
dev-dependency on the widgets crate lacks `observe`, which is M43's
R13-F7 (19 tests red). With the assertion in place it would fail to
**compile** instead. So T48.9 lands after M43's fix, and says so.

### 2.5 One name for the address

`FULGURITE_AGENT_ADDR`. It is what the client library, the drive runner,
counter-window and the skill already use, so a developer following the
skill reaches the right port. The window reads it in one place:
`fulgurite-winit`'s `start`, under the `agent` feature. The
variable overrides the program's own `Program::agent` address, so a
script can choose a port. A program's address is the default. With
neither set, there is no endpoint. The examples drop their own
`std::env::var` reads. `FULGURITE_AGENT` is still honoured for one
release, with a deprecation line on stderr, and the scripts set only
the new name.

## T48.0 — the gates first, red at `1717808`

Each finding becomes a check that fails today, committed with its red
output in the commit body, as M35/T35.0 and M36/T36.0 did. Each is
marked expected-to-fail until the task that closes it lands.

| finding | the check | red today |
|---|---|---|
| R13-F3 | `crates/fulgurite-shell/tests/agent_frames.rs`: five clicks on a headless session, each followed by `frame` and `agent_frame_done`; `ui.lastChange` with `since = reply.frame - 1` must name the changed label | `click k: reply frame F, changes 0 (must be >= 1)` from click 2 |
| R13-F1 | `crates/fulgurite-shell/tests/agent_settled.rs`: an animating app, and a task sleeping 100 ms | `settled=true animating=true`, `settled=true pending=1` |
| R13-F2 | a step in `scripts/tasks_drive.py`: ten clicks, then `app.perf` | `frame p50 0 ns (must be > 0), n=N` |
| R13-F8 | the redraw deleted (`1beed32`) against the event-loop test M39/T39.0 added, plus the harness | with the injection: `lastChange after click: 0`, and `widget-drive` exits 0 |
| R15-F10 | `crates/fulgurite-agent/tests/tree_fields.rs`: a checked `CheckBox`, a `Slider` at 0.25, an open `Chooser`, a `Modal`, an inactive `Either` branch | `fields present: 0 / 6` |
| R13-F4 | the same file: `ui.resolve` on the `CheckBox` before and after a click; a coverage walk over the showcase entries | `checked: null -> null`; eight kinds named `GenericContainer` |
| R13-F5 | `crates/fulgurite-agent/tests/nested_wrappers.rs`, from probe `c9d6760`: `Glass<Glass<Label>>`, `Card<Card<Label>>`, `ScrollView<ScrollView<Label>>` | `live=[true,true,false,false], changes=0/1` |
| R14-F4 | `crates/fulgurite-agent/tests/alloc_frame_done.rs`, its own binary with a counting allocator | `allocs/frame_done 4.00 / bound 0` |
| R12-F3 | a `just static` step: `cargo check -p fulgurite-widgets --features fulgurite/observe` must **fail** | the build succeeds |
| R13-F6 | `crates/fulgurite-agent/tests/session_replay_fails.rs`, from probes `580f9f6` | `replay -> Ok, fresh n=1 / recorded 3` |
| R13-F9 | `scripts/tasks_drive.sh`'s last step: `quit`, then the pid must exit | `still alive after 2000 ms` |
| R13-F10 | a `just static` script: no file outside the winit crate reads `FULGURITE_AGENT` | five files named |
| R14-F8 | a debug counter of journaled frames, on tasks-demo with no address set, over three frames | `journaled 3 / bound 0` |
| R13-F11 | a `just static` script comparing the method names in dispatch and `answer_agent` against the skill and `book/src/agent.md` | `missing: [app.failure, app.retry, app.shadow, app.reloads, app.setPalette]`, `extra: [app.quit]` |

**Effort:** M. **Depends on:** M39/T39.0 for the event-loop test.

## T48.1 — one frame counter

**Closes:** R13-F3.

**Change.** Per §2.1. Every `input.*` reply and `app.setPalette`'s
reply name `self.frame + 1` in `Agent::dispatch_input`'s result
builders, and carry the host's number as `host_frame`. `since_param`'s
rustdoc states the contract.

**Gate.** `agent_frames.rs` goes green on all five clicks, printing
`click k: reply frame F, changes N (must be >= 1)`. `scripts/drive/harness.py`
marks with the reply's frame and `just widget-drive` stays green.

**Injection.** Reply with the host's number again: clicks 2–5 go red
with 0 changes. Click 1 may pass, because the counters agree in a fresh
session; that is why the test clicks five times.

**Docs in the same commit.** The skill (`SKILL.md:123`, "the reply's
`frame` is ..."), `since_param`'s rustdoc, and `book/src/agent.md`'s
section on what changed.

**Effort:** S. **Depends on:** T48.0.

## T48.2 — `ui.waitSettled` answers from the host

**Closes:** R13-F1.

**Change.** Per §2.2. Add the provided `AgentHost` method, and
implement it on `Live` in `session.rs`. `Local` keeps the last frame's
`animating` and passes it in. The host gains its in-flight job count.
The reply is `{settled, frame, animating, more_deferred,
pending_tasks, next_deadline_ms, subscriptions}`.

**Gate.** `agent_settled.rs` goes green in both cases: `false` while
animating and then `true` once the animation ends; `false` while the
task sleeps and then `true` once its result is applied. It prints
`settled=<v> animating=<a> pending=<n>` at each step. Live: a step in
`scripts/tasks_drive.py` asserts `false` right after `#start-slow` and
`true` once `#status` reads `slow=99`. **Accept direction:** the
"after" assertions, and a still window answering `true`.

**Injection.** (1) Answer `Some(true)` always: both headless cases go
red. (2) Delete only the pending-job term: the task case goes red and
the animation case stays green, which shows each term carries its own
case. (3) Make the default answer `true`: `protocol.rs`'s waitSettled
clause, rewritten to expect `null` from a host that cannot say, goes
red.

**Docs in the same commit.** The comment at `lib.rs:809`; the "no
auto-wait" rationale in `crates/fulgurite-agent/src/harness.rs:29`
and `docs/results-harness.md` (whose premise, that there is no
scheduler, was false); the skill's synchronisation section; and
`docs/00-handoff-static-tree.md` §9.

**Effort:** M. **Depends on:** T48.1, since the reply's `frame` must
be the agent's. **Coordinate with M47/T47.3**, which rewrites the
spawner the count increments in, and with **M49/T49.10**, whose ledger
of spawned, unapplied descriptors is the same bookkeeping: when the
ledger lands, the reply's pending-task count becomes its length and this counter goes,
so there is one source for it (rule 2).

## T48.3 — `app.perf` measures, or says it did not

**Closes:** R13-F2.

**Change.** `Session::agent_frame_done` takes the frame's timings as a
plain struct of integers defined in the shell, so no agent type crosses
the library seam. `redraw` fills it from `FrameSource::timings` (layout
and paint) and from the frame `present` returns (render, present,
damaged pixels), with the whole redraw as `frame_ns`. `FrameTiming`'s
stage fields become optional, and an unmeasured stage is `null`, not
0. `scripts/ui_check.py` treats `n == 0` or an all-`null` window as
**NOT MEASURED**, which fails. Changing `Session`'s shape changes the
ABI stamp by construction (`session.rs` is hashed into it), so a
library built before this refuses to load. That is the stamp working.

**Gate.** The `tasks_drive.py` step goes green, printing
`frame p50 X ns (must be > 0), n=N`, with layout also above 0. A
`scripts/ui_check.py` self-test feeds a canned all-zero reply and an
all-`null` reply and asserts both print NOT MEASURED. **Accept
direction:** counter-window, which measured all along, still passes
`just live-gate`.

**Injection.** Pass a default timing from `redraw` again: the live
step reads `frame p50 0 ns`. Restore `ui_check.py`'s old comparison:
the self-test prints `ok` on zeros and refuses it.

**Docs in the same commit.** `FrameTiming`'s rustdoc ("a host that
lacks a stage reports zero" becomes "reports null");
`docs/00-handoff-static-tree.md` §11 ("`app.perf` now reports every
stage", true only for counter-window until now); and the skill's
performance-budgets table.

**Effort:** M. **Depends on:** T48.0. Coordinate with M49, which also
touches `Session`.

## T48.4 — a stale journal fails the drive

**Closes:** R13-F8.

**Change.** Each action in the drive inventory declares whether it
repaints, defaulting to yes. For a repainting action with no region
over the widget, `scripts/drive/harness.py` fails with "no damage over
the widget: journal stale or nothing painted" and prints the region
count. Actions that correctly paint nothing declare it: a click only
the application hears, or a hover with no hover style. The verifier's
two silent Button actions are sorted into one kind or the other, by
name, in the commit body. M39/T39.0's event-loop test gains a step:
after `input.click`, `ui.lastChange` with `since = reply.frame - 1` is
non-empty, which holds only if the redraw ran. That test's handler only
counts methods (M39/T39.0 item 2), so this step runs it against a
`Session`-backed window instead, which is the new part. That makes it
the cargo-level gate `crates/fulgurite-winit/tests/` never had.

The damage line also names what it measures. `ui.lastDamage`'s ratio
is journal damage in logical pixels (M40 §2.1), so the harness prints
beside it the damage the surface presented, from T48.3's `app.perf`
`damaged_px` for that action's frame, and fails a repainting action
whose presented damage is 0. The 1.00× figure keeps its meaning, and
stops being read as what the surface redraws.

**Gate.** The harness's per-action line reads `damage: … ratio
(journal), presented P px` or `FAIL no damage (regions=N)`. The
event-loop step prints `lastChange after click: N changes (must be >=
1)`.

**Injection.** Delete `request_redraw()` in `answer_agent` (the
review's `1beed32`). `just widget-drive` must fail on the first
repainting action, and the event-loop test fails with 0 changes.
**Accept direction:** the unmodified build passes both, with the ratio
printed.

**Docs in the same commit.** The module doc of `scripts/drive/harness.py`
("damage is bounded …" is now true of every repainting action), and
AGENT.md's M27 "1.00×" sentence, qualified to repainting actions and
to journal damage, citing `docs/damage-amplification-decision.md` as
M40/T40.1 amends it.

**Effort:** S. **Depends on:** T48.1; T48.3 (the presented damage it
prints); M39/T39.0 (the test binary); M40/T40.1, because until R1-F1 is
fixed `damaged_px` counts logical rects as physical pixels (M40 §2.1);
and M42, because the harness must say which binary it drove before its
green means anything.

## T48.5 — one serialiser, with the state it dropped

**Closes:** R15-F10.

**Change.** One function in `observe.rs` turns an emitted node into
its JSON object, given the table. `observe::dump` and the agent's
`get_tree` both call it, and the agent's copy is deleted. It adds
fields only when present, so existing scripts keep working: `toggled`
(`"true"`, `"false"`, `"mixed"`), a `numeric` object (value, min, max,
step), `expanded`, `modal`, `hidden`, `disabled`, and the advertised
action names.

**Gate.** `tree_fields.rs` asserts each field on its widget and prints
`fields present: 6 / 6`. An equality test asserts that `observe::dump`
and `ui.getTree` over the same tree are byte-identical, and prints the
first differing node if they are not. **Accept direction:** an
unchecked `CheckBox` reports `toggled: "false"`, not an absent field,
which shows the field reads live state rather than a constant.

**Injection.** (1) Drop the `toggled` insertion: the `CheckBox` row
fails. (2) Put a private copy back in the agent, without `hidden`: the
equality test fails and names the first node where the two differ.

**Docs in the same commit.** `dump`'s rustdoc ("cannot drift" is now
true, with the function named); the handoff Step 5 paragraph; the
`getTree` field list in the skill; and `book/src/agent.md` if it shows
a sample, which must be included from a file, not retyped.

**Effort:** S. **Depends on:** T48.0. It builds on M35/T35.1's
`emit_root` and changes nothing in the walk, so `just
observe-inlining` stays green.

## T48.6 — `resolve` sees it too, and eight widgets describe themselves

**Closes:** R13-F4.

**Change.** `Index::summary_map` takes the same fields from T48.5's
function, so `ui.resolve` and `ui.getLayout` report `toggled` and
`numeric`. Minimal `observe` impls, a label or a value, are added to
`Badge`, `Chip`, `Avatar`, `ProgressBar` (numeric), `Spinner` (a
progress indicator, marked busy), `Card` (label from its title),
`Toast` and `Tooltip`. `Divider` and `Skeleton` stay
`GenericContainer`, and their rustdoc says why: they have nothing to
announce.

**Gate.** In `tree_fields.rs`: a `CheckBox` clicked reads `checked`
`false -> true` through `ui.resolve`, and a `Slider` set to 0.9 reads
0.9, with before and after printed. A coverage assertion walks a
headless build of the showcase tree. Each entry's kind must either
have a role other than `GenericContainer` or appear in an explicit
allow list with a reason. It prints `N kinds, M generic, allowed K`.

**Injection.** Remove `Badge`'s new impl: the coverage assertion names
`badge`. Drop the field from `summary_map` only: the `ui.resolve`
row fails while T48.5's `getTree` row stays green, which shows the
second call path is tested on its own.

**Docs in the same commit.** The module doc of `observe.rs` ("one
implementation serves both" is now true of these fields), and the
skill.

**Effort:** M. **Depends on:** T48.5, and M43/T43.6 (R13-F7), so
that the agent crate's tests build widgets with `observe`.

## T48.7 — a stand-in is recognised by structure, not identity

**Closes:** R13-F5.

**Change.** Per §2.3. `Node` gains the required, `observe`-gated count
method. `leaf_common!` and the derive emit it, and each hand-written
impl returns its own constant. `live_nodes` compares each object's
count against the table's subtree size at `i`, and the address/kind
identity is removed.

**Gate.** `nested_wrappers.rs` goes green on all three nestings. It
asserts `live == [true; n]` and exactly one `lastChange` change after
the label's text is set, and prints `live=<vec>, changes=N/1` per
case. The existing `crates/fulgurite-agent/tests/hidden_subtree.rs`,
`off_table_visibility.rs` and `changes.rs` stay green. That is the
accept direction: hidden nodes are still hidden, and a zero-sized
first field is still live. `just observe-inlining` stays green.

**Injection.** (1) Restore the identity comparison: the nested cases go
red at `changes=0/1`. (2) Make the comparison always say "live":
`hidden_subtree.rs` goes red, which shows the new rule is what carries
the hidden case. (3) Have `Glass<W>`'s count return its content's
constant instead of its own: the nested glass case goes red. That is
the one mistake a hand-written impl could make, and it must be
visible.

**Docs in the same commit.** `live_nodes`' rustdoc (`lib.rs:100-120`:
the "needs a type to contain itself" claim was false), the
`Node` rustdoc for the new method, and the handoff where the identity
rule is described.

**Effort:** M. **Depends on:** T48.0. M41's redesigned shadow oracle
(M41/T41.2) compares display lists and `NodeState`, not the journal, so
it is unaffected. Coordinate with M55, which changes the derive's
expansion in the same phase of work.

## T48.8 — `frame_done` allocates nothing, and a test says so

**Closes:** R14-F4.

**Change.** The journal and the auditor share scratch vectors kept on
`Agent`: the `live` mask and T48.7's cached subtree sizes. `live_nodes`
takes the scratch, which is R14's sketch `2a60b9c` without the identity
vector, since T48.7 removed it.

**Gate.** `alloc_frame_done.rs` warms up five frames, then counts 100
`frame_done` calls on a 901-node tree of the shape `benches --bin
agent` builds, with and without findings. It prints
`allocs/frame_done X / bound 0` and runs in `just test`.

**Injection.** Put `let mut live = Vec::new();` back in `journal.rs`.
The test reads `2.00 / 0` and fails.

**Docs in the same commit.** `docs/results-agent-cost.md` (the
threshold is now asserted, with the test named), and `frame_done`'s
rustdoc (`lib.rs:~376`).

**Effort:** S. **Depends on:** T48.7.

## T48.9 — the feature seam fails the build

**Closes:** R12-F3.

**Change.** Per §2.4. `fulgurite` exports whether `observe` is on,
and `fulgurite-widgets` asserts at compile time that it has
`observe` whenever the core does, with a message that names the
feature to enable.

**Gate.** The T48.0 step in `just static`: `cargo check -p
fulgurite-widgets --features fulgurite/observe` must fail with
the assertion's message, and `--features observe` must build. It
prints the message it got. **Accept direction:** the same session
builds the showcase with its `agent` feature and reads `role: Label`
from `ui.getTree`, via `scripts/tasks_drive.py`.

**Injection.** Delete the assertion: the expected failure becomes
"build succeeded" and the step fails. Both directions are shown in one
session.

**Docs in the same commit.** The comment on the `agent` feature in
`crates/fulgurite-winit/Cargo.toml`; `docs/results-widget-crate.md`'s
"One feature seam" section (now enforced); and the book's agent
chapter if it names the feature.

**Effort:** S. **Depends on:** M43/T43.6 (R13-F7). Without it,
`cargo test -p fulgurite-agent` stops compiling instead of running
19 red tests. M54/T54.1 later splits `a11y` out of `observe`, and
extends this assertion to the new feature.

## T48.10 — a replay that diverges is an error

**Closes:** R13-F6.

**Change.** A recorded step keeps a summary of its reply: `handled`
and the error code. `Recording::note` takes the reply. `Recording::replay`
returns a new divergence error, naming the step, what was recorded and
what came back, when a step errors or its `handled` differs.
`Recording::check` takes the stylesheet's name and refuses a mismatch
through the existing `ReplayError::Mismatch`. `Recording::export_test`
emits a `check` call, an assertion that each step has no error and the
recorded `handled`, and a final assertion on the `ui.getTree` digest
captured at export time, instead of the placeholder comment.

**Gate.** `session_replay_fails.rs`, with probe `580f9f6`'s cases and
their expectations inverted. A corrupted recording gives a divergence
at step 2. A stylesheet mismatch gives `Mismatch`. An exported test
with a missing selector, run as a generated function, panics naming
its step. It prints `replay -> <result>, fresh n=X / recorded Y`.
**Accept direction:** `crates/fulgurite-agent/tests/session.rs`'s
round trip still replays `Ok`, and an untouched export passes.

**Injection.** Restore `let _ =` in `replay`: the corrupted case
returns `Ok` and the test fails by name. Drop the stylesheet
comparison: the mismatch case goes red.

**Docs in the same commit.** The module doc of
`crates/fulgurite-agent/src/session.rs` and `export_test`'s
rustdoc; `docs/plan-harness.md` T12.3 and `docs/results-harness.md`.

**Effort:** M. **Depends on:** nothing. **It can run before Phase 3**:
it touches only `session.rs` in the agent crate, no frame path, no
transport, and no method a page can reach. So none of the reasons for
the phase order apply to it.

## T48.11 — one environment variable for the address

**Closes:** R13-F10.

**Change.** Per §2.5. The window's `start` reads `FULGURITE_AGENT_ADDR`
when the `agent` feature is on. showcase, tasks-demo, hot-window and
panic-app drop their own reads. The drive scripts
(`scripts/agent_drive.sh`, `tasks_drive.sh`, `widget_drive.sh`,
`panic_drive.sh`, `hot_window_drive.sh`), and M42's launcher, which
passes both names until this task (M42 §4), set only `FULGURITE_AGENT_ADDR`.
`FULGURITE_AGENT` is still honoured for one release, with a deprecation
line on stderr.

**Gate.** A `just static` script: no file outside
`crates/fulgurite-winit` reads `FULGURITE_AGENT`, and it prints any line
that does. Positive control: it finds the deprecation shim itself, or
it fails as blind. The live drives listed in M39/T39.7 stay green, and
that is the accept direction.

**Injection.** Re-add showcase's `std::env::var("FULGURITE_AGENT")`: the
script names `examples/showcase/src/main.rs`.

**Docs in the same commit.** The skill's port lifecycle;
`Config::agent`'s and `Program::agent`'s rustdoc; the comments on the
justfile recipes.

**Effort:** S. **Depends on:** T48.0. **It can run before Phase 3**,
for the same reason as T48.10: it changes where an address comes from,
not what is served there.

## T48.12 — no endpoint, no journal

**Closes:** R14-F8.

**Change.** The window keeps whether an endpoint was configured, once
T48.11 has settled where the address comes from, and calls
`agent_frame_done` only when one was. A first request seeds the journal
with one `frame_done`, and the first `ui.lastChange` reports
`reset: true`. A static script builds showcase for release without the
`agent` feature and counts agent and `serde_json` symbols in the binary
with `nm -C`. The agent build is the positive control. accesskit is not
counted: M54 may make a screen-reader tree default-on for desktop
(M54 §2.3), and then accesskit is linked without the agent on purpose.

**Gate.** (1) The script prints
`fulgurite_agent symbols: 0 / bound 0 (agent build: N)`, and fails
if the agent build's N is 0, because a blind count would pass
anything. (2) The T48.0 debug counter, on tasks-demo with no address
set, prints `journaled 0 / bound 0` over three frames. With an address
set, it journals every frame: that is the accept direction.

**Injection.** Drop the endpoint check: the counter reads `3 / 0`. Add
`fulgurite-winit/agent` to showcase's default features: the symbol
count is no longer 0.

**Docs in the same commit.** `Config::agent`'s rustdoc (`None` is now
inert with the feature on as well); `docs/results-agent-cost.md`; the
F2.5 note in `docs/step5-observability.md`.

**Effort:** S. **Depends on:** T48.11.

## T48.13 — `app.quit` exists where the skill says it does

**Closes:** R13-F9.

**Change.** `answer_agent` handles `app.quit`: it replies
`{ok: true}`, sets a flag, and exits from `about_to_wait`, which has
the event loop in hand. `AgentClient.quit` swallows only a closed
connection, which is success, and re-raises an error reply.

**Gate.** `scripts/tasks_drive.sh` ends with `quit`, then asserts the
pid exits within 2 s and the port stops listening. It prints
`exited in X ms / bound 2000`. An `agent_client.py` self-test asserts
that a `-32601` reply to `quit` raises. **Accept direction:**
`just live-gate` still quits counter-window, whose `app.quit` is its
own.

**Injection.** Remove the `app.quit` arm: the process is still alive
after 2 s.

**Also `input.compose`** (assigned by the index, 2026-09-25: the
consistency review found no plan adding it). The method takes
`{text, commit}` and routes it through the same `Host::compose` path
winit uses. Without it, IME is covered only by headless gates, and the
shadow oracle can never see a live compose frame. That frame is
R1-F2's, the S0 M41/T41.1 closes. **Gate:** `agent_drive.sh`'s
Boundaries leg types "abc" by preedit and then commit. It asserts the
field's value and that the oracle reports `first: None` over those
frames, and prints `compose frames judged N / bound ≥ 2`. **Injection:**
drop T41.1's compose mark. The oracle names the node, which it can do
only after M41/T41.2. **Depends on:** M41/T41.1 and T41.2 as well.

**Docs in the same commit.** The skill's method list, now checked
(T48.14). M42's launcher, which until now kills every window but
counter-window, quits every window and asserts a zero exit, keeping
its log scan as the second check (M42 §2.5); its docstring says so.

**Effort:** S. **Depends on:** **M39/T39.1.** A method that ends the
process, reachable from a web page, is a denial of service until the
transport refuses the page.

## T48.14 — the documented surface is the built one

**Closes:** R13-F11.

**Change.** Rewrite the four stale comments in `lib.rs` (§1.4). Add a
`just static` script that extracts the method-name string literals
from `Agent::dispatch`'s and `Agent::dispatch_input`'s arms, and from
`answer_agent`'s. It diffs them against the backticked names in the
skill's "Methods that exist" paragraph and in `book/src/agent.md`'s
method groups.

**Gate.** The script prints `code N methods, skill M, book K, missing:
[..], extra: [..]` and fails when either list is non-empty.

**Injection.** Delete `app.retry` from the skill: it prints
`missing: [app.retry]`. Add a made-up `ui.bogus` to the book: it prints
`extra: [ui.bogus]`. **Accept direction:** the finished skill and book
pass, with the counts printed.

**Docs in the same commit.** The four rustdoc sites, the skill and the
book themselves.

**Effort:** S. **Depends on:** T48.2 (the `waitSettled` comment
changes with it) and T48.13 (the method set it checks is final only
once `app.quit` exists).

## 4. Boundaries of this plan

- **The transport is M39's.** Framing, binding, connections, the
  envelope and the agent's own panics are all there. This milestone
  assumes them.
- **A blocking `ui.waitSettled` with a timeout is not built** (§2.2).
  Reopen it when a caller would otherwise poll in a loop tighter than
  a frame.
- **R12-F3 was proposed for M55 by the re-triage** (a "feature seams"
  group). `assign.json` puts it here, and it stays here, because the
  seam's symptom is an agent reporting the wrong role. T48.9 is small
  and has no derive work in it. If M55 grows a general feature-seam
  check, T48.9's assertion is one instance of it.
- **R14-F4 and R14-F8 were proposed as M35 follow-ups.** M35 closed on
  the observe walk's inlining, which neither touches. Both are about
  the journal's cost, which the walk does not own, so they stay here.
- **No `observe` impl for `Divider` or `Skeleton`** (T48.6). They have
  nothing to announce, and saying so in their rustdoc is the fix.
- **No won't-fix.** Every assigned finding is closed by one task.

## 5. Kill rules and risks

This milestone makes no speed claim. R14-F4 and R14-F8 are counts
(allocations, journaled frames, symbols), gated at zero, as
`docs/results-allocation.md` argues every such path should be. The
risks, and what would reveal each:

- **The structural stand-in rule (T48.7) is wrong somewhere the table
  and an object disagree about a subtree.** `Dyn`, `List` and `Repeat`
  own off-table arenas, and their main-table counts are constants.
  T48.7's injection (3) and the existing `rows.rs` and
  `off_table_visibility.rs` would reveal it. If a container is found
  whose `node(i)` returns an object that is neither `i`'s own nor an
  ancestor's, the rule's premise is false: stop and fall back to
  §2.3's option (a).
- **`settled` is expensive to answer.** The in-flight count is one
  integer per spawn, off the frame path. `next_deadline_ms` already
  runs every `about_to_wait`. If `benches --bin agent` shows
  `waitSettled` above its spread, eight runs a side, the reply is
  recorded as costing that and nothing is gated on it: it is a poll,
  not a frame.
- **Changing `Session`'s shape (T48.3) refuses every library built
  before it.** That is intended: the ABI stamp refuses by design. It is
  a risk only if M49 lands a `Session` change in the same window
  without rebuilding the hot fixtures. `just abi-stamp` and
  `just hot-window-drive` would reveal it.
- **The feature assertion (T48.9) refuses a build nobody expected.**
  Any crate that builds the core with `observe` and the widgets crate
  without it will stop compiling. The workspace's depth-2 feature
  powerset (the R12 ledger) is what would reveal such a crate, so run
  it in the same session as T48.9.
- **The frame contract (T48.1) changes what scripts see.** A script
  that compared the reply's `frame` with the host's own count would
  break. None does: `scripts/drive/harness.py` passes it to `since`,
  which is the use the contract serves. `just widget-drive` under both
  strategies would reveal one that did.

## 6. What building found (2026-09-26)

Corrections to this plan, kept where they were wrong rather than
rewritten.

- **T48.0: two findings were larger than listed.** R13-F4's coverage
  walk found **nine** widgets with no `observe`, not eight: `Icon` too.
  R13-F11's check found the skill missing `app.reloads` as well as the
  five §1.4 lists (it is the window's, answered in `answer_agent`).
- **T48.0: the feature-seam check compiles**, so it runs in `dev-shape`
  rather than in `static`, whose contract is "sub-second, no compile".
- **T48.0: the live rows are written with their tasks.** R13-F2, R13-F8,
  R13-F9 and R14-F8 each need a window and an edit to a drive script;
  each task's commit shows its live gate red before the fix.
- **T48.1: the drive harness needed no change.** It marks with
  `ui.lastChange`'s own `frame` *before* the action, which is the
  journal's counter already and equals `reply.frame - 1` for a
  one-input action — and stays right for an action of several inputs,
  where the first reply's frame would not.
- **T48.3: the p50 regression bound cannot fail.** `ui_check.py`'s
  `check_latency` prints the p50 "regress" line and never appends its
  failure, so the bound the comment calls "what actually catches a
  change" has never failed a run. Found, not fixed: out of this
  milestone's findings, and on a loaded machine the 0.18 ms bound would
  need its own look first.
- **T48.4: the presented damage of an input was not in `app.perf` at
  all.** An agent's `input.*` pumps and presents *inside* the request
  (`Live::pump`), and the redraw after it usually finds nothing left to
  draw, so every action's journal frame reported `damaged_px` 0 (the
  T48.3 live run: p50 0). `Screen::present` now returns what it cost,
  and the session folds the request's presents into the next journal
  frame; `app.perf { since }` sums them.
- **T48.4: the new rule found three defects the skipped check hid.**
  (1) The showcase's checkbox, switch and radio never showed the flag
  they toggle: `on` is a presentation copy, seeded once on selection and
  never written back (`examples/showcase/src/main.rs`, `refresh`). (2)
  The journal never signed a `Picker`'s trigger: `node(trigger)` answers
  with the picker, so the trigger was a stand-in, and the picker's
  state-only signature held none of its text or swatch. A container
  with a visible part it paints itself is now signed by painting it
  (`journal.rs`, `owns_a_visible_part`). (3) A caret's animation pushed
  one PickList action to 107 journal frames against a 64-frame history,
  and `ui.lastDamage` answered from a truncated ring with no region; the
  ring is 256, and the drive refuses a truncated answer by name.
- **T48.4: "over the widget" is over any of its nodes.** A picker's
  popup is its own and is drawn below its trigger, so the bound is taken
  against every node of the widget's subtree that changed, where it was
  and where it is, each region against the largest node it covers.
- **T48.4: which actions paint nothing.** The review's two silent Button
  actions are not silent at this HEAD — all four print a damage line at
  1.00x (M42/T42.6's `prep` moved the hover off the button first). The
  actions declared `repaints=False` are the veils' two scrim presses
  (a request only the application hears; the veil stays).
- **T48.6: the coverage walk is over the widget crate's leaves, not the
  showcase.** The showcase is a binary crate, and no test can name its
  tree; `tree_fields.rs` builds one of each leaf kind instead — the same
  question, asked of every kind rather than of the ones a demo happens
  to place. It found `Icon` as a ninth, which now publishes an image
  labelled by its symbol. `Card` has no title of its own, so it is a
  `Group` whose content describes itself; `Tooltip` is a `Group`
  described by its tip's text.
- **T48.12: the journal is not seeded by a first request.** The plan
  had a first request seed the journal and report `reset: true`. With
  the endpoint configured the window journals every frame from the
  first, so there is nothing to seed; with none, no request can arrive.
  The debug counter is `FULGURITE_FRAME_REPORT=N` — after N frames the
  window prints `N frames drawn, M journaled` and exits — read by the
  event-loop test on a spinning window rather than on tasks-demo, which
  draws no frame unasked.
- **T48.13: the `input.compose` live leg came second.** It needed
  M41/T41.2b's live oracle legs, which reached the base after the rest of
  M48; `agent_drive.sh` now types "abc" by preedit and commit on every
  leg and reads the field back, and on the oracle leg counts the frames
  the oracle judged across the steps (13 / bound ≥ 2 — the count includes
  the caret's own frames in between, so it is an upper bound on compose
  frames and the bound is on judging *any*). `scripts/live/shadow.py`
  then requires `first: None`.
- **T48.13: M42's launcher quits every window by default**, times the
  exit against 2 s and requires the port to stop listening, so the
  `tasks_drive.sh` step the plan names is the launcher's, for every
  drive.

## Appendix: finding → task

| ID | sev now | status | task |
|---|---|---|---|
| R12-F3 | S3 | OPEN | T48.9 |
| R13-F1 | S1 | OPEN | T48.2 |
| R13-F2 | S2 | OPEN | T48.3 |
| R13-F3 | S2 | OPEN | T48.1 |
| R13-F4 | S2 | OPEN | T48.6 |
| R13-F5 | S2 | OPEN | T48.7 |
| R13-F6 | S2 | OPEN | T48.10 |
| R13-F8 | S2 | OPEN | T48.4 |
| R13-F9 | S3 | OPEN | T48.13 |
| R13-F10 | S3 | OPEN | T48.11 |
| R13-F11 | S4 | WORSE | T48.14 |
| R14-F4 | S2 | OPEN | T48.8 |
| R14-F8 | S3 | OPEN | T48.12 |
| R15-F10 | S3 | OPEN | T48.5 |
