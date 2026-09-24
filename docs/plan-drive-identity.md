# M42 — the drive harness knows which process it drove

> **Written 2026-09-24, not started.** Closes I-NEW-3, R18-F1, R18-F2,
> R18-F3, R18-F9 and R18-F11. They come from M38's deep review
> (`docs/results-deep-review.md` §2.3, §4, §5), re-triaged against
> `main` at 1717808 on 2026-09-24. Where the review and the re-triage
> differ, the re-triage is used, and §1 says where this plan's own
> reading of 1717808 differs from both.
>
> **Phase 2, and first in it.** M41 re-validates T34.2's evidence with
> these scripts, so this milestone's identity check has to land before
> M41 gathers any evidence. T42.1 edits the agent's bind site, which M39
> rewrites, so T42.1 waits for M39. M43 runs alongside. M43's `just live`
> and `just ci` call the scripts this milestone rebuilds, and T42.2
> implements the skip exit code that M43 §2 decides.
>
> **The finding that sets the order**: R18-F1 was reproduced live at
> 1717808. A leftover showcase held the port. Both "strategy" legs of
> `just widget-drive` failed to bind, drove that one leftover process,
> and printed `0 failed`, with exit 0. Ten commits since ac91d51 cite
> "green both ways" (§1.1). The unmodified scripts do build two distinct
> binaries today, so those claims are **unverifiable, not shown false**.
> This milestone makes them checkable (T42.1–T42.4), then checks them
> (T42.8).

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that turns it red,
prints the measured quantity beside its bound, and shows in the same
session that it accepts good input.*

---

## 0. The rules for this milestone

These scripts are what every other milestone's live evidence stands on,
so the rules are stricter than usual.

1. **A live gate names the process it drove.** Each leg prints the pid,
   the invalidation strategy and the feature set that the *process*
   reports. It fails if any of them differs from what the script
   launched. A label the script sets for itself proves nothing
   (LUMEN_DRIVE_STRATEGY today).
2. **"Cannot say" is never "no".** If a check's subject is compiled out
   of the build, the check fails, unless the leg says it expects the
   subject to be absent. `panic_drive.sh` already does this for
   `app.failure`. `agent_drive.sh` does not do it for `app.shadow`
   (I-NEW-3).
3. **Coverage is compared against something the drive did not write.**
   A count taken from the inventory and checked against the same
   inventory is a tautology (R18-F2).
4. **The app's exit status and its log are part of the verdict.** A
   panic after the last check is still a panic (R18-F9).
5. **Every gate here is shown red by a plant that lives in the
   repository and re-runs.** A scratch SHA does not count. The plants
   go under `scripts/selftest/live/`, using the harness M43/T43.0
   builds (§2.6). The review's injections were scratch commits (I5a
   `1a1f478`, I5b/I5c `9623a3e`, I6 `481773e`). They showed the defects
   once, and nothing can show them again.

## 1. What is wrong, at 1717808

### 1.1 The harness cannot tell which process it drove (R18-F1)

- The strategy is only a label. `scripts/drive/run.py:40` reads
  LUMEN_DRIVE_STRATEGY from the environment. No agent method reports
  the strategy.
- `app.diagnostics` (`crates/lumen-static-agent/src/lib.rs:1336-1363`)
  answers from the tree alone: frame, node counts, repeaters and byte
  sizes. It reports no pid, no strategy and no features.
- A bind failure is only logged. Two sites do this, and both then
  return from the socket thread while the window keeps running:
  - `crates/lumen-static-winit/src/agent.rs:66`
  - `examples/counter-window/src/agent_socket.rs:126`
- `agent_client.py wait-port` succeeds against any listener
  (`scripts/widget_drive.sh:39`).
- The other drives do not wait for the port at all. They sleep a fixed
  interval instead: `agent_drive.sh:40` (4 s), `tasks_drive.sh:40`
  (3 s), `panic_drive.sh:22` (3 s) and `hot_window_drive.sh:43` (2 s).
- Every live script uses a fixed default port (7655, 7656, 7658, 7661
  and 7671), so two worktrees running drives collide.
- Every live script uses a fixed log path under `/tmp`, so two
  worktrees also overwrite each other's logs. `agent_drive.sh:36` goes
  further and writes both of its own legs to one file, so the
  `Everything` leg's log is gone by the time anyone looks.

The review's premise has since happened. T34.2 (6116a8b) made exactly
the leg edit the finding warned about:

- `widget_drive.sh:50-51`
- `agent_drive.sh:151-152`
- `tasks_drive.sh`
- `panic_drive.sh`

The re-triage re-ran the stranger case at 1717808. Both legs logged
`agent: cannot bind`, both drove the stranger, and both exited 0. It
also hashed the three showcase builds the scripts make. **These are
the re-triage's numbers: `3cf80479` (`agent,everything`), `404f3d91`
(`agent`) and `b680e8cb` (`agent,shadow-oracle`).** They are distinct,
so the unmodified legs are distinct builds.

The ten commits whose "both ways" or live-drive evidence rests on this
harness, and which drives each one cites:

| commit | task | cites |
|---|---|---|
| 8e662a4 | tasks-demo sheet fix | tasks-drive |
| 5161612 | T33.3 | widget-drive, agent-drive |
| af3a3ee | T33.4 | tasks-drive |
| cc3c6b6 | T29.6 | widget-drive |
| 10b7362 | T34.0 | agent-drive |
| e176eb3 | T34.1 | agent-drive (and its shadow half) |
| 6116a8b | T34.2 | widget-drive, agent-drive, tasks-drive |
| da19334 | T35.1/T35.2 | widget-drive |
| 2677168 | T36.3 | panic-drive |
| 6b5a292 | T37.1 | widget-, agent-, panic- and tasks-drive |

### 1.2 A check that says nothing when its subject is compiled out (I-NEW-3)

`scripts/agent_drive.sh:142` is `if sh.get("supported"):` and has no
`else`. The agent's own rustdoc (`lib.rs:1474-1481`) says that
`supported: false` exists so that a drive does not mistake "cannot say"
for "no divergence". The drive then makes exactly that mistake. The
re-triage dropped `shadow-oracle` from the `Boundaries` leg. The run
exited 0, printed `agent-drive: both strategies`, and differed from the
baseline only by the missing "compared N frames" line. **The
re-triage's baseline was 264 frames compared.** (The re-triage cited
`:141`. The line is `:142`.)

This plan makes the drive refuse a leg that has no oracle. It does
**not** make the oracle's silence meaningful. The oracle repairs the
frames it inspects (R1-F6), and it has no standing test that it can
fire (I-NEW-1). Both belong to M41.

### 1.3 Coverage is measured against the inventory itself (R18-F2), and there are fewer assertions than the docs claim (R18-F3)

`scripts/drive/harness.py:357` collects `seen`. `:431-433` builds
`missing` from the same `actions` list that the loop above it has just
walked. The loop either adds each action to `seen` or raises, so
`missing` is empty every time control reaches that line.

Nothing pins how many entries or actions there are. The review deleted
Radio's inventory and one Switch row, and the run stayed green at 38/92
against 39/94 (I5b/I5c; `docs/results-deep-review.md` §4). Nothing ties
the widgets `lumen-static-widgets` exports to an inventory. The review
listed eighteen exported types with none. Select, TextArea and
ScrollView are among them, and all three are interactive.

R18-F3: `docs/plan-widget-parity.md:442` and AGENT.md:181-182 say
"three assertions per action". Three things weaken that:

- Check (1), `:364-370`, is skipped for `static` actions.
- When check (1) does run, it passes on *any* non-empty readout, and
  `changed` is the whole `ui.lastChange` set, not filtered to "this
  widget" as its comment says.
- Check (3), `:419`, runs only when an isolate exists.

The review's counts are in `docs/results-deep-review.md` §2.4 (R18-F3).
They are not restated here.

### 1.4 The app's exit status is thrown away (R18-F9)

`scripts/live_gate.sh:18` is `wait "$pid" || true`. Its header says
"Exit code is the verdict". It also launches through `cargo run`
(`:9`), so `$pid` is cargo's pid, not the window's. Cargo passes the
child's status on, so this has not mattered yet. It will matter the
moment a check compares pids.

The drive scripts kill the app and then call `wait || true`:

- `widget_drive.sh:46-47`
- `agent_drive.sh:147-148`
- `tasks_drive.sh:42-43`
- `panic_drive.sh:24-25`

So a panic after the last drive action is discarded there too. The
review's I6 (`481773e`) made counter-window panic on `app.quit`, and
the gate stayed green.

### 1.5 The adversary is tested "once per tranche" on paper (R18-F11)

`docs/plan-widget-parity.md` §5 (:567-574) says the adversary is run
once per tranche against a planted weakness. Only T27.10's run is
recorded (:948 onward). The plant exists only as prose: `_page(c) <=
c._p0` in place of `== c._p0 - 1`, in `scripts/drive/widgets/data.py:77`.
The directory `scripts/drive/plants/` does not exist.

**A correction to the re-triage's gate.** The plant makes the drive
*blinder*. With the plant applied and nothing else changed, the drive
is green. So a recipe that "applies it, runs widget-drive Pagination,
expects failure" would refuse the good input. T42.7 specifies a
two-arm gate instead.

## 2. Decisions

### 2.1 Where the process reports its identity

The options:

- **(a)** A new `app.identity` method.
- **(b)** New fields in `app.diagnostics`, which is what the re-triage
  asked for.
- **(c)** No app support, with the harness identifying the process from
  the socket alone.

**Recommended: (b), backed by (c) as an independent second check.**
`app.diagnostics` gains a `process` object with four fields:

- `strategy`: the `type_name` of the host's `Invalidation`;
- `features`: a `cfg!`-built list, `everything`, `shadow-oracle`,
  `reload`, `agent` (M55/T55.4 later removes the `everything` feature;
  the strategy field then carries that fact alone);
- `pid`;
- `exe`: `current_exe`.

`Agent::dispatch` answers `app.diagnostics` from the tree alone, so the
new fields come from a new `AgentHost::process_info`. The request is
merged in the host-aware branch, next to `app.shadow`
(`lib.rs:432`).

**Provided or required.** The method is provided, and its default
returns `{"supported": false}`. AGENT.md warns that a provided method
is wrong for a *wrapper*. Every `AgentHost` implementation is a leaf
that holds a host, and the harness refuses `supported: false` (rule 2).
So a window that forgets to implement the method fails loudly on its
first drive, and there are no test hosts to update.

The two live implementations are:

- `lumen_static_shell::session::Live`, where the strategy is the shell's
  `Inval` alias, `session.rs:49-51`;
- counter-window's `Live`, which wraps `Host<Tally>` and so uses the
  host's default.

**(c) is kept because it does not trust the app.** The launcher asks
the kernel which pid owns the listening socket (`ss -ltnpH`). That also
makes (c) the only identity check available for the historical re-run
in T42.8, where the binaries predate (b).

What (b) breaks: nothing that exists. It adds one key to
`app.diagnostics`, and `protocol.rs::diagnostics_and_perf` gains an
assertion for it.

### 2.2 What a failed bind does

The options:

- **(a)** Keep logging.
- **(b)** Exit with a distinct status, 3.
- **(c)** Panic.

**Recommended: (b), always.** A window that was built with an agent
endpoint, and cannot serve it, is misleading whoever asked for the
endpoint. Exit 3 is distinct from a panic (101) and from a kill (143).

What (b) breaks: two `just tally` windows on the default port. The
second one now exits and names the port, where today it runs without
an endpoint. The fix for the user is LUMEN_AGENT_ADDR. This lands
after M39/T39.1, which moves the bind into one transport in the agent
crate. M39 does not change whether the endpoint is on by default;
M48/T48.11 does (with no address configured there is no endpoint). So
(b) applies whenever an address was configured.

### 2.3 One launcher, in Python

**Recommended: one launcher, `scripts/live/launch.py`, used by all eight
live scripts.** Those are `widget_drive`, `agent_drive`, `tasks_drive`,
`panic_drive`, `hot_window_drive`, `wake_gate`, `gpu_parity` and
`live_gate`. Python is chosen over a bash function because every check
it makes is a JSON comparison. The launcher does the following, in
order:

1. Checks for a display, and exits 77 if there is none (M43 §2).
2. Picks a free port unless one is given, and **refuses a port that
   already answers** before it launches anything.
3. Starts the binary itself, not through cargo, and writes the log under
   `target/live/<script>-<leg>.log`.
4. Waits for the port **while checking that its own child is still
   alive**, and prints the log tail if the child dies.
5. Checks that the socket's owner is the child (§2.1 (c)), and that the
   `process` block's pid, strategy and features match the leg's
   declared expectation (§2.1 (b)).
6. Runs the drive.
7. Checks that the child is still alive, scans its log for
   `panicked at`, and only then stops it.

The scripts keep their drive bodies. What they lose is their own
launching code.

Free ports by default remove the collision class entirely. This
matters most for M43's `just ci`, which runs while a developer's
windows are open. The display check exits 77, not 0, as M43 §2 decides.
The launcher is where that code lives. M43 owns the policy that reads
it.

### 2.4 Coverage is pinned by name, and the exported widgets are tied to inventories

**Recommended: the manifest pins names, not only counts.**
`scripts/drive/expected.json` lists every entry and each entry's action
names, plus the three counts R18-F3 needs: actions, actions with a
landed check, and actions with an isolate. `run.py` compares what it
discovers against the manifest before any window opens:

- a removal fails;
- an addition fails until the manifest is edited, so the change is
  visible in the diff.

The second half is a new `just static` check,
`scripts/check_drive_coverage.sh`. Every `pub use` widget type in
`crates/lumen-static-widgets/src/lib.rs` must either have an inventory
or appear in `scripts/drive/undriven.allow` with a reason and an owner.
The allow file starts with the review's eighteen names.

Select, TextArea and ScrollView are entered as owed. §4 says why their
inventories are not written here.

### 2.5 Quitting, not killing, when the window supports it

A kill hides the app's own exit status. Every turnkey window lacks
`app.quit` (R13-F9), and R13-F9 belongs to M48, which lands after M39.
Until then, the launcher checks liveness and the log before it kills.
It uses `app.quit` where a window answers it, which today means
counter-window, so `live_gate.sh`. When M48 lands, the launcher quits
every window and asserts a zero exit, and the log scan stays as a
second check.

### 2.6 The plants share M43's harness

M43/T43.0 builds `scripts/selftest/`. A plant is a scripted edit in a
scratch worktree. The harness asserts that the edit applied, that the
check went red, and that the red output names the expected string.
This milestone adds `scripts/selftest/live/`, run by
`just selftest-live`, which needs a display and exits 77 without one.
T42.0 therefore depends on T43.0. That is one small task, and the
alternative is two harnesses for one idea.

### 2.7 What the re-run in T42.8 checks, and what a red means

The ten commits are re-run **at their own SHAs**, with the launcher's
socket-ownership check (§2.1 (c)), because their binaries have no
`process` block. Their drive bodies are run unchanged. Each leg asserts
three things:

- the port was free before launch;
- the launched pid owned the socket for the whole drive;
- the two legs' binaries hash differently, and each was built with the
  features its leg names.

HEAD is then run with the full identity check.

A leg that is red at its own commit is **recorded, not fixed at that
commit**. The claim in that commit body is then marked false in
AGENT.md, and HEAD's result decides whether any code needs to change.

## 3. Tasks

### T42.0 — the gates first

**Closes:** nothing by itself. It commits the red state of every
finding below.

Add `scripts/selftest/live/` with one plant per finding, on M43/T43.0's
harness. Each plant states the output it expects once the fix is in:

| plant | edit | expected once fixed | at 1717808 |
|---|---|---|---|
| stranger | start a showcase on the port, then run `widget_drive.sh Switch` on it | `port already answering` before any build is driven | green (exit 0) |
| leg-drop (I5a) | `run_one Everything agent` in `widget_drive.sh` | `expected Everything, got …Boundaries<256>` | green |
| shadow-absent | drop `shadow-oracle` from `agent_drive.sh`'s `Boundaries` leg | `expected the shadow oracle in this build` | green |
| shadow-unexpected | add `shadow-oracle` to the `Everything` leg | `shadow oracle present in a leg that does not expect it` | green |
| inventory-drop (I5b/I5c) | delete Radio from `widgets/toggles.py`'s `ALL`, and delete one Switch row | `manifest expects Radio`, `Switch: missing action` | green (38/92) |
| export-unowned | remove one name from the undriven allow file | the static check names it | no check exists |
| hover-noop | remove the hover shade from the showcase sheet | `Button/hovering is visible: changed in subtree 0` | green (passes on the readout) |
| quit-panic (I6) | counter-window panics on `app.quit` | `live-gate: app exited 101` | green |
| late-panic | a showcase handler panics on a frame after the last drive action | `app log: 1 panic` | green |

The inventory-drop and export-unowned plants need no display. They run
in `just selftest`, the static half of the harness.

**Gate:** `just selftest-live` prints `plant <name>: stayed green —
expected red` for each row. At 1717808 it exits non-zero with every row
listed. The commit body records that output.

The harness's accepting direction is checked in the same session. An
unplanted run of each script is green, and the harness reports "0
plants, all baselines green".

**Injection** (for the harness itself): a plant whose edit does not
apply, for example a `sed` pattern that matches nothing. It must
report `plant <name>: edit did not apply`, not "stayed green". AGENT.md
warns that "the injection itself can be wrong", and this is that case.

**Docs in the same commit:** the header of `scripts/selftest/live/`
lists the plants, the findings they stand for, and their expected
output.

**Effort:** M. **Depends on:** M43/T43.0.

### T42.1 — the process reports what it is

**Closes:** nothing alone. It is the app half of R18-F1, which closes in
T42.2.

**Change:**

- Add `AgentHost::process_info` (§2.1), provided, with its default
  `{"supported": false}`.
- The agent merges it into `app.diagnostics` under `process`, in the
  host-aware branch, `Agent::dispatch_input`.
- Implement it for the shell's `Live` (`session.rs:465`), where the
  strategy is `type_name::<Inval>()` and the features are the shell's
  `cfg!` set.
- Implement it for counter-window's `Live`.
- The bind failure exits 3 after printing the address (§2.2). At
  1717808 there are two sites, `crates/lumen-static-winit/src/agent.rs:66`
  and `examples/counter-window/src/agent_socket.rs:126`. After
  M39/T39.1 both servers call one transport, so the exit goes into its
  bind step, and M39/T39.2's loopback refusal exits the same way.

**Gate:**

- `crates/lumen-static-agent/tests/protocol.rs`: `diagnostics_and_perf`
  asserts that a host with no `process_info` answers `supported: false`.
- A new shell test builds a `Live` over `Host<_, _, Everything>` and
  asserts that the strategy ends in `Everything`, printing the full
  string. The `Boundaries<256>` counterpart does the same.
- A winit test binds a port, then starts the socket thread on the same
  port. The thread must exit with status 3; the test runs it in a
  child process and prints the status.

**Injection:**

- Return the *default* strategy's name instead of `type_name::<Inval>()`.
  The `Everything` test fails and prints `…Boundaries<256>`.
- Replace the exit with the old `return`. The bind test reports "child
  still running after 2 s".

**Accepts good input:** both strategy tests pass unmodified, and a bind
on a free port serves.

**Docs in the same commit:**

- the `AgentHost` and `app.diagnostics` rustdoc;
- `.claude/skills/verifying-apps/SKILL.md` (the `app.diagnostics`
  fields);
- the `serve` rustdoc (the exit status).

**Effort:** S. **Depends on:** M39/T39.1 (the transport) and T39.2
(its bind step).

### T42.2 — one launcher, and every leg checks the process it drove

**Closes:** R18-F1.

**Change:**

- Add `scripts/live/launch.py` (§2.3) and migrate all eight live
  scripts to it.
- Each leg declares its expected strategy and features. The launcher
  prints the following, and fails on any mismatch:
  `drove pid P (socket owner P), strategy S, features F, exe E`.
- `run.py` no longer reads LUMEN_DRIVE_STRATEGY. It takes the strategy
  from the launcher, which took it from the process.

**Gate:** the stranger and leg-drop plants from T42.0.

- The stranger plant fails before driving, with `port already answering
  on 127.0.0.1:P (owner pid Q)`.
- The leg-drop plant fails with `expected Everything, got
  lumen_static::invalidate::Boundaries<256>`.

**Accepts good input:** on a free port, the unmodified `widget_drive.sh
Switch` prints two distinct pids and two distinct strategies, and is
green.

**Injection** (mechanism, beyond the plants): the stranger case has
three independent defences, and each is shown alone.

- Remove the pre-launch refusal. The stranger plant must still fail,
  now with `child exited 3: cannot bind`.
- Also revert T42.1's exit in the scratch build. It must still fail,
  now on `socket owner Q is not the launched pid P` and on the
  diagnostics pid.
- Disable the socket-owner comparison as well. It must fail on the
  diagnostics pid alone.

**Docs in the same commit:**

- the headers of all eight scripts, including "skips with 77";
- AGENT.md's M27 entries: "both ways" now names the check that makes it
  true;
- `docs/plan-boundaries-default.md`'s T34.0 inventory rows for the
  drives;
- `docs/results-deep-review.md` §5's `widget_drive.sh` row, set to
  "shown to fail: yes, by the stranger and leg-drop plants".

**Effort:** M. **Depends on:** T42.0, T42.1.

### T42.3 — the app's exit and log are part of the verdict

**Closes:** R18-F9.

**Change:**

- `live_gate.sh` runs the built binary rather than `cargo run`, so
  `$pid` is the app.
- It asks the app to quit and then waits with a plain `wait`. A
  non-zero status fails the run with `live-gate: app exited N`.
- For the drives, the launcher's stop step (§2.3 step 7) checks
  liveness and scans the log before it kills. A process that died
  mid-drive, or a log that contains `panicked at`, fails the leg and
  prints the panic line.

**Gate:** the quit-panic and late-panic plants.

- `live_gate.sh` prints `app exit status: 0` on good input.
- Every leg prints `app log: 0 panics`.

**Injection:** the plants, plus reverting the `wait` to `|| true`. The
quit-panic plant must then go green again, which shows that the gate
is the `wait` and not the log scan.

**Docs in the same commit:** the `live_gate.sh` header. The
`scripts/live/launch.py` docstring gets the stop sequence and the §2.5
note about M48.

**Effort:** S. **Depends on:** T42.2.

### T42.4 — a leg that expects the oracle fails without it

**Closes:** I-NEW-3.

**Change:**

- The `agent_drive.sh` Python block receives the leg's expected feature
  set from the launcher.
- In a leg that expects the oracle, it asserts `supported is True`,
  then that `compared > 0`, then that `first is None`, in that order.
- In a leg that does not expect it, it asserts `supported is False`.
- It prints `shadow: supported=S expected=E, compared N` for every leg.

This is the `app.failure` pattern `panic_drive.sh` already uses.

**Gate:** the shadow-absent and shadow-unexpected plants.

- Shadow-absent fails with `expected the shadow oracle in this build`.
- Shadow-unexpected fails with `shadow oracle present in a leg that
  does not expect it`.

**Accepts good input:** the unmodified legs pass, and print
`supported=False expected=False` and `supported=True expected=True,
compared N` respectively.

**Injection** (mechanism): restore the bare `if sh.get("supported"):`.
Shadow-absent must go green again.

**Docs in the same commit:**

- the comment at `agent_drive.sh:138-141`;
- `docs/plan-boundaries-default.md` T34.1: the live-gate sentence says
  that the leg now fails without the oracle, and points to M41 for
  what "quiet" means.

**Effort:** S. **Depends on:** T42.2.

### T42.5 — coverage against a manifest, and exported widgets against inventories

**Closes:** R18-F2.

**Change:**

- Add `scripts/drive/expected.json` (§2.4).
- `run.py` compares it with the discovered inventories before
  connecting to anything, and prints `inventories: E entries / A actions
  (manifest E/A)`.
- Delete the unreachable coverage block (`harness.py:430-433`) rather
  than leaving a check that cannot fire. The docstring's claim (lines
  10-13) moves to the manifest.
- Add `scripts/check_drive_coverage.sh` to `just static`. It prints
  `widgets exported N, driven D, allowed A, missing 0`.
- Add `scripts/drive/undriven.allow`. Each line is a type name, an
  owner and a reason.

**Gate:** the inventory-drop plant fails with `manifest expects Radio`
and `Switch: missing action 'track changes colour'` **before any window
opens**. The export-unowned plant makes the static check name the
removed widget.

**Accepts good input:** the unmodified tree passes both.

**Injection** (mechanism): make `run.py` read the manifest's counts
from the discovered inventories instead of from the file. Inventory-drop
must go green again, which shows the manifest is what the gate stands
on.

**Docs in the same commit:**

- the `harness.py` docstring;
- AGENT.md:182 ("coverage as an assertion"): it now names the manifest;
- `docs/plan-widget-parity.md:594`: the twelve un-retrofitted widgets
  are now the allow file's entries.

**Effort:** M. **Depends on:** T42.0.

### T42.6 — a landed check on the widget, and the real counts

**Closes:** R18-F3.

**Change:**

- Check (1) records the readout before the action and compares it with
  the readout after.
- It filters `ui.lastChange` to the widget's subtree, with the range
  resolved once through `ui.resolve`.
- It passes only on a change inside the subtree or a readout that
  actually moved.
- It prints `changed in subtree: k nodes, readout before -> after`.
- The summary line prints `actions A, landed-checked L, isolated I`,
  and the manifest from T42.5 pins all three.

Actions that legitimately change nothing on the widget stay `static`.
Each one is listed in the manifest, so the count of unchecked actions is
visible and cannot grow silently.

**Gate:** the hover-noop plant fails with `Button/hovering is visible:
changed in subtree 0, readout unchanged`.

**Accepts good input:** the baseline passes and prints the three
counts.

**Injection** (mechanism): drop the subtree filter. Hover-noop must go
green again, because the readout moves elsewhere on the stage.

**Docs in the same commit:**

- AGENT.md:181-182: "three assertions per action" becomes a sentence
  that cites the manifest's counts rather than restating them;
- `docs/plan-widget-parity.md:442`;
- the check (1) comment in `harness.py`.

**Effort:** M. **Depends on:** T42.5. Coordinate with M48/R13-F8, which
changes the damage block of the same loop (§4).

### T42.7 — the planted weakness as a recipe that proves it is a weakness

**Closes:** R18-F11.

**Change:**

- Store the T27.10 plant as `scripts/drive/plants/t27_10_pagination.patch`:
  `_page(c) <= c._p0` in place of `== c._p0 - 1`.
- Store the widget-side defect it hides, "make the first slot go to the
  window's first page" (`data.py:83`'s own injection), as a second
  patch beside it.
- Add `just adversary-plant`, which runs two arms in a scratch worktree:
  - **(A)** the defect with the real drive. The drive must go **red**.
  - **(B)** the defect with the planted drive. The drive must stay
    **green**.

  Both arms together show that the plant is a real blind spot, which
  is what makes it a fair test of an adversarial reviewer. §1.5 says
  why one arm is not enough.

**Gate:** `just adversary-plant` prints `plant t27_10_pagination: real
drive red (N failed), planted drive green`. It exits non-zero if either
arm is wrong.

**Injection:**

- Empty the plant patch. Arm (B) goes red, and the recipe reports `plant
  does not blind the drive`.
- Empty the defect patch. Arm (A) goes green, and the recipe reports
  `defect not caught by the real drive`.

**Accepts good input:** the real patches, exit 0.

**Docs in the same commit:**

- `docs/plan-widget-parity.md` §5 (:567-574) and AGENT.md's M27 entry
  ("tested once per tranche") say that the adversary ran once, for
  T27.10, and that the plant now re-runs.

**Effort:** S. **Depends on:** T42.2. The recipe uses the launcher.

### T42.8 — re-run the evidence the ten commits cite, and record it

**Closes:** nothing new. It settles what R18-F1 left unverifiable.

**Change:** no code. At each of the ten SHAs in §1.1, in a scratch
worktree, run the drives that commit cites. Use T42.2's launcher in
socket-identity mode (§2.7), with each commit's own drive bodies and
feature lists. Then run all five drives at HEAD with the full identity
check.

Record the results in a new results doc, `results-drive-identity.md`
in `docs/`, one row per commit and drive, with:

- the pids and socket owners of both legs;
- the two binary hashes;
- the counts each drive printed;
- pass or fail.

**Two limits on what the re-run shows,** both stated in the doc:

- **tasks-drive** re-establishes only *which process* was driven. R7-F2
  (M47) shows it is vacuous for `run()`'s wiring.
- **The shadow half of agent-drive** re-establishes only that the
  oracle was *present*. R1-F6 and I-NEW-1 (M41) decide whether its
  silence means anything.

**Gate:** the results doc's table has a row for every (commit, drive)
pair in §1.1, and a HEAD row for each drive under the full check.
T42.2's printed identity line is copied verbatim into each row.

A commit whose leg is red at its own SHA gets a **false** mark next to
its claim in AGENT.md, in the same commit as the doc. This is §2.7's
rule, and a red leg does not reopen the commit.

**Injection:** none of its own. The gates it runs were shown red in
T42.2–T42.4.

**Accepts good input:** a HEAD run that is green both ways, with two
distinct pids.

**Docs in the same commit:**

- the new results doc;
- AGENT.md's M33, M34, M36 and M37 entries cite it next to "both ways";
- `docs/plan-boundaries-default.md` T34.2's live-evidence sentence
  cites it.

**Effort:** M, mostly machine time. **Depends on:** T42.2–T42.4. It
must be finished **before M41 gathers any evidence**.

## 4. Boundaries of this plan

- **Not the shadow oracle's design.** R1-F6 (it repairs what it
  inspects) and I-NEW-1 (no standing test that it fires) are M41's.
  T42.4 only refuses a leg that has no oracle.
- **Not the damage bound's `if mine:` hole, nor a `repaints` flag.**
  Both are R13-F8, in M48. T42.6 edits the same loop, and whichever
  lands second rebases.
- **Not `app.quit` in turnkey windows** (R13-F9, M48), and **not the
  split between LUMEN_AGENT and LUMEN_AGENT_ADDR** (R13-F10, M48).
  The launcher passes both until M48 settles on one.
- **Not tasks-drive's blindness to `run()`'s wiring** (R7-F2, M47). The
  launcher makes that drive's legs *identifiable*. It does not make
  them observe an idle window.
- **Not inventories for Select, TextArea and ScrollView.** They enter
  `undriven.allow` with owners. A drive written now would have to steer
  around defects that other milestones own: keyboard row picking (R3-F6,
  M44) and paste into a TextArea (R8-F4 and R10-F5, M45). An inventory
  that avoids the broken actions would be vacuous for exactly those
  actions. Each owning milestone writes the inventory when its fix
  lands, and removes the allow entry in the same commit.
- **Not the damage block's units.** `ui.lastDamage` and the 1.00× ratio
  are journal damage in logical pixels (M40 §2.1), so they have never
  measured what the surface redraws. Printing the presented damage
  beside them is M48/T48.4's, with R13-F8 below.
- **Not `just live` or `just ci`**, which are M43's (T43.13). The
  launcher exits 77 on a missing display. What 77 means to an
  aggregate is M43's policy.

## 5. Kill rules and risks

There is no performance work here, so there is no performance kill
rule. What would stop the milestone:

- **M39 changes the transport so that a second process can share a
  port**, for example with SO_REUSEPORT or a unix socket. Then "port
  already answering" is the wrong refusal. *Revealed by:* the stranger
  plant going green after M39 lands. The launcher then refuses on
  socket ownership alone.
- **A commit in the re-run does not build any more** (toolchain, or a
  vendored crate gone from the offline cache). *Revealed by:* T42.8's
  build step. Such a commit is recorded as "not re-runnable" with the
  error, which is weaker than a verdict, and the doc says so.
- **`ss` cannot see the owner** (different user namespace, or missing
  capability). *Revealed by:* the launcher printing an empty owner on
  the baseline. The launcher then falls back to
  `/proc/net/tcp` inode matching against `/proc/<pid>/fd`, and the
  plant is run again under the fallback.
- **The plants rot as the scripts change.** A plant whose edit no longer
  applies must fail as "edit did not apply" (T42.0's own injection), so
  rot is loud. It is still a maintenance cost, and it is the price of
  rule 5.

## Appendix: finding → task

| ID | sev now | status | task |
|---|---|---|---|
| I-NEW-3 | S2 | OPEN | T42.4 |
| R18-F1 | S2 | OPEN | T42.2 (app half T42.1; evidence re-run T42.8) |
| R18-F2 | S2 | OPEN | T42.5 |
| R18-F3 | S3 | OPEN | T42.6 |
| R18-F9 | S2 | OPEN | T42.3 |
| R18-F11 | S4 | OPEN | T42.7 |
