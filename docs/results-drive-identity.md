# Results: M42 — which process the live evidence drove

*M42/T42.8, `docs/plan-drive-identity.md` §2.7. Owns the re-run of the
"green both ways" evidence that ten commits cite, and the HEAD run under
the full identity check. Counts and identities, not timings: machine
load does not change a row (uptime was 1.6–8.0 during the runs, other
agents compiling; nothing here is a clock reading).*

## §0 What the re-run could and could not say

R18-F1 left ten commits' live evidence **unverifiable**: a leftover
window holding the port answered for both "strategy" legs, and nothing
recorded which process a leg drove. The plan's §1.1 table lists them.
Each was re-run **at its own SHA**, in a scratch worktree
(`scratch-m42-rerun`, removed afterwards), with **that commit's** drive
bodies and feature lists, each leg launched by HEAD's
`scripts/live/launch.py --identity socket`. That mode asserts:

- **the port was free before launch** (the launcher refuses one that
  answers);
- **the launched pid owned the listening socket** — the kernel's answer
  (`ss -ltnpH`) — when the drive began. While that process lives no
  other can bind the port (the transport sets no SO_REUSEPORT; §5 of
  the plan names that as the kill condition), and the stop step checks
  it is still alive, so it owned the socket for the whole drive;
- **the legs are different binaries**: every leg's binary was built with
  the features its leg names in that commit's script and copied aside,
  and the hashes below differ between the two legs of every drive.

The old binaries predate `app.diagnostics`' `process` block (T42.1), so
**what strategy each old binary ran is established by construction** —
its features, and a hash that differs from the other leg's — **not by
the process reporting it**. HEAD's rows below have the process's own
answer as well.

**Two limits, as the plan requires them stated:**

- **tasks-drive** re-establishes only *which process* was driven. R7-F2
  (M47) shows it is vacuous for `run()`'s wiring.
- **The shadow half of agent-drive** re-establishes only that the oracle
  was *present* (`supported`, `compared > 0`). R1-F6 and I-NEW-1 (M41)
  decide whether its silence means anything.

**Verdict: every leg of every cited drive is green at its own commit,
driving the process it launched.** No commit's claim is false. Of the
ten, one leg set differs from what "both ways" might be read to mean:
`5161612` (T33.3) cites agent-drive, which at that SHA ran **one** leg,
`Everything`, as `docs/plan-boundaries-default.md` T34.0 already records.

## §1 The ten commits, each leg

`binary sha256` is the first eight hex digits of the leg's binary.
"What the drive printed" is the drive's own last summary line; every leg
also printed `app log: 0 panics` (panic-drive's recovery legs: `7
panics (expected 7)`, the seven provoked sites — the timer step that
provokes an eighth came later, M47/T47.1).

| commit | task | drive | leg | cargo features | binary sha256 | pid = socket owner | what the drive printed | verdict |
|---|---|---|---|---|---|---|---|---|
| `8e662a4` | tasks-demo sheet fix | tasks | everything | `(none)` | `a40244e3` | 1135232 = 1135232 | ok  no failures; final {'one': 2, 'batch': 3, 'chain': 3, 'slow': 0, 'ticks': 3, 'failed': 0, 'sub': 'off'} | pass |
| `8e662a4` | tasks-demo sheet fix | tasks | boundaries | `boundaries` | `626bb05d` | 1155178 = 1155178 | ok  no failures; final {'one': 2, 'batch': 3, 'chain': 3, 'slow': 0, 'ticks': 3, 'failed': 0, 'sub': 'off'} | pass |
| `5161612` | T33.3 | widget | Everything | `agent` | `fbefe4dc` | 1159051 = 1159051 | widget-drive [Everything]: 39 widgets, 94 actions, 31 without an isolate, 0 failed | pass |
| `5161612` | T33.3 | widget | Boundaries | `agent,boundaries` | `117d1ffa` | 1163532 = 1163532 | widget-drive [Boundaries]: 39 widgets, 94 actions, 31 without an isolate, 0 failed | pass |
| `5161612` | T33.3 | agent | Everything | `agent` | `fbefe4dc` | 1171938 = 1171938 | agent-drive: Label -> Switch, menu 300x560 px, press observable | pass |
| `af3a3ee` | T33.4 | tasks | everything | `(none)` | `e3ed9e1a` | 1172092 = 1172092 | ok  no failures; final {'one': 2, 'batch': 3, 'chain': 3, 'slow': 0, 'ticks': 3, 'failed': 0, 'sub': 'off'} | pass |
| `af3a3ee` | T33.4 | tasks | boundaries | `boundaries` | `5f627922` | 1172280 = 1172280 | ok  no failures; final {'one': 2, 'batch': 3, 'chain': 3, 'slow': 0, 'ticks': 3, 'failed': 0, 'sub': 'off'} | pass |
| `cc3c6b6` | T29.6 | widget | Everything | `agent` | `7d758bba` | 1173027 = 1173027 | widget-drive [Everything]: 39 widgets, 94 actions, 31 without an isolate, 0 failed | pass |
| `cc3c6b6` | T29.6 | widget | Boundaries | `agent,boundaries` | `7fc67785` | 1224294 = 1224294 | widget-drive [Boundaries]: 39 widgets, 94 actions, 31 without an isolate, 0 failed | pass |
| `10b7362` | T34.0 | agent | Everything | `agent` | `8b3ca7f6` | 1227254 = 1227254 | agent-drive: Label -> Switch, menu 300x560 px, press observable | pass |
| `10b7362` | T34.0 | agent | Boundaries | `agent,boundaries` | `bb18a1fa` | 1227475 = 1227475 | agent-drive: Label -> Switch, menu 300x560 px, press observable | pass |
| `e176eb3` | T34.1 | agent | Everything | `agent` | `4caa0b2e` | 1227776 = 1227776 | agent-drive: Label -> Switch, menu 300x560 px, press observable | pass |
| `e176eb3` | T34.1 | agent | Boundaries | `agent,boundaries,shadow-oracle` | `03489f79` | 1228029 = 1228029 | agent-drive: shadow oracle compared 28 frames, none diverged | pass |
| `6116a8b` | T34.2 | widget | Everything | `agent,everything` | `e513d400` | 1228314 = 1228314 | widget-drive [Everything]: 39 widgets, 94 actions, 31 without an isolate, 0 failed | pass |
| `6116a8b` | T34.2 | widget | Boundaries | `agent` | `3c857ee1` | 1231322 = 1231322 | widget-drive [Boundaries]: 39 widgets, 94 actions, 31 without an isolate, 0 failed | pass |
| `6116a8b` | T34.2 | agent | Everything | `agent,everything` | `e513d400` | 1275349 = 1275349 | agent-drive: Label -> Switch, menu 300x560 px, press observable | pass |
| `6116a8b` | T34.2 | agent | Boundaries | `agent,shadow-oracle` | `1ffd7ff5` | 1275624 = 1275624 | agent-drive: shadow oracle compared 28 frames, none diverged | pass |
| `6116a8b` | T34.2 | tasks | everything | `everything` | `553d2245` | 1275774 = 1275774 | ok  no failures; final {'one': 2, 'batch': 3, 'chain': 3, 'slow': 0, 'ticks': 3, 'failed': 0, 'sub': 'off'} | pass |
| `6116a8b` | T34.2 | tasks | boundaries | `(none)` | `45027630` | 1275962 = 1275962 | ok  no failures; final {'one': 2, 'batch': 3, 'chain': 3, 'slow': 0, 'ticks': 3, 'failed': 0, 'sub': 'off'} | pass |
| `da19334` | T35.1/T35.2 | widget | Everything | `agent,everything` | `b4914cd5` | 1278009 = 1278009 | widget-drive [Everything]: 39 widgets, 94 actions, 31 without an isolate, 0 failed | pass |
| `da19334` | T35.1/T35.2 | widget | Boundaries | `agent` | `ee068a35` | 1326409 = 1326409 | widget-drive [Boundaries]: 39 widgets, 94 actions, 31 without an isolate, 0 failed | pass |
| `2677168` | T36.3 | panic | everything | `agent,reload,fulgurite-winit/everything` | `5db17b0b` | 1391677 = 1391677 | panic-drive [everything]: all seven sites caught, shown and recovered | pass |
| `2677168` | T36.3 | panic | boundaries | `agent,reload` | `55fc582a` | 1391963 = 1391963 | panic-drive [boundaries]: all seven sites caught, shown and recovered | pass |
| `2677168` | T36.3 | panic | norecover | `agent` | `5e833f49` | 1392388 = 1392388 | app exit status: 101 (expected nonzero) | pass |
| `6b5a292` | T37.1 | widget | Everything | `agent,everything` | `f9ebfa7b` | 1339765 = 1339765 | widget-drive [Everything]: 39 widgets, 94 actions, 31 without an isolate, 0 failed | pass |
| `6b5a292` | T37.1 | widget | Boundaries | `agent` | `a32d1f61` | 1386709 = 1386709 | widget-drive [Boundaries]: 39 widgets, 94 actions, 31 without an isolate, 0 failed | pass |
| `6b5a292` | T37.1 | agent | Everything | `agent,everything` | `f9ebfa7b` | 1389571 = 1389571 | agent-drive: Label -> Switch, menu 300x560 px, press observable | pass |
| `6b5a292` | T37.1 | agent | Boundaries | `agent,shadow-oracle` | `83e3437a` | 1389857 = 1389857 | agent-drive: shadow oracle compared 28 frames, none diverged | pass |
| `6b5a292` | T37.1 | tasks | everything | `everything` | `b0038e93` | 1390761 = 1390761 | ok  no failures; final {'one': 2, 'batch': 3, 'chain': 3, 'slow': 0, 'ticks': 3, 'failed': 0, 'sub': 'off'} | pass |
| `6b5a292` | T37.1 | tasks | boundaries | `(none)` | `aba84bfd` | 1390945 = 1390945 | ok  no failures; final {'one': 2, 'batch': 3, 'chain': 3, 'slow': 0, 'ticks': 3, 'failed': 0, 'sub': 'off'} | pass |
| `6b5a292` | T37.1 | panic | everything | `agent,reload,fulgurite-winit/everything` | `3f308ff8` | 1392746 = 1392746 | panic-drive [everything]: all seven sites caught, shown and recovered | pass |
| `6b5a292` | T37.1 | panic | boundaries | `agent,reload` | `06c378e5` | 1393078 = 1393078 | panic-drive [boundaries]: all seven sites caught, shown and recovered | pass |
| `6b5a292` | T37.1 | panic | norecover | `agent` | `ee8a1a77` | 1393494 = 1393494 | app exit status: 101 (expected nonzero) | pass |

Pairs whose hashes match are the same feature set, as they should be:
at `5161612` and `6116a8b`/`6b5a292` the agent-drive `Everything` leg
and the widget-drive `Everything` leg build identical features. Within
every drive the two legs differ.

## §2 HEAD, under the full identity check

`scripts/*.sh` at M42's last commit, each leg checked by the launcher
against the process's own `app.diagnostics` (T42.1) as well as the
socket. The identity line is copied verbatim (paths shortened to the
worktree).

| drive | leg | identity line | what the drive printed | verdict |
|---|---|---|---|---|
| widget-drive | Everything | `drove pid 1393837 (socket owner 1393837), strategy fulgurite::invalidate::Everything, features agent,everything, exe target/release/showcase` | widget-drive [Everything]: 39 widgets, 94 actions, 31 without an isolate, 0 failed; actions 94, l... | pass |
| widget-drive | Boundaries | `drove pid 1400629 (socket owner 1400629), strategy fulgurite::invalidate::Boundaries<256>, features agent, exe target/release/showcase` | widget-drive [Boundaries<256>]: 39 widgets, 94 actions, 31 without an isolate, 0 failed; actions ... | pass |
| agent-drive | Everything | `drove pid 1426274 (socket owner 1426274), strategy fulgurite::invalidate::Everything, features agent,everything, exe target/release/showcase` | agent-drive: shadow: supported=False expected=False, compared 0 | pass |
| agent-drive | Boundaries | `drove pid 1426637 (socket owner 1426637), strategy fulgurite::invalidate::Boundaries<256>, features agent,shadow-oracle, exe target/release/showcase` | agent-drive: shadow: supported=True expected=True, compared 30 | pass |
| tasks-drive | everything | `drove pid 1426759 (socket owner 1426759), strategy fulgurite::invalidate::Everything, features agent,everything, exe target/release/tasks-demo` | ok  no failures; final {'one': 2, 'batch': 3, 'chain': 3, 'slow': 1, 'ticks': 29, 'failed': 0, 's... | pass |
| tasks-drive | boundaries | `drove pid 1427112 (socket owner 1427112), strategy fulgurite::invalidate::Boundaries<256>, features agent, exe target/release/tasks-demo` | ok  no failures; final {'one': 2, 'batch': 3, 'chain': 3, 'slow': 1, 'ticks': 29, 'failed': 0, 's... | pass |
| panic-drive | everything | `drove pid 1427744 (socket owner 1427744), strategy fulgurite::invalidate::Everything, features agent,everything,reload, exe target/release/panic-app` | panic-drive [everything]: all seven sites caught, shown and recovered | pass |
| panic-drive | boundaries | `drove pid 1428170 (socket owner 1428170), strategy fulgurite::invalidate::Boundaries<256>, features agent,reload, exe target/release/panic-app` | panic-drive [boundaries]: all seven sites caught, shown and recovered | pass |
| panic-drive | norecover | `drove pid 1428492 (socket owner 1428492), strategy fulgurite::invalidate::Boundaries<256>, features agent, exe target/release/panic-app` | panic-drive [norecover]: app exit status: 101 (expected nonzero) | pass |
| hot-window-drive | only | `drove pid 1429179 (socket owner 1429179), strategy fulgurite::invalidate::Boundaries<256>, features agent,reload, exe target/hot-live/ws/target/release/hot-window` | hot-window-drive: PASS | pass |

Every leg also printed `app log: N panics` equal to its declared count
(0; panic-drive 8 and 8 and 1; hot-window-drive 1).

## §3 How it was run

At each SHA the scratch worktree was checked out, and for each leg:
`cargo build --release -p <pkg> --features <that commit's leg features>`,
the binary copied to `target/rerun/bins/<sha>-<drive>-<leg>-<pkg>` and
hashed, then `launch.py --identity socket --exe <copy> -- <drive>`, where
the drive is that commit's own: `scripts/drive/run.py` with
the strategy variable set as the old script set it (widget), the
heredoc body extracted from `scripts/agent_drive.sh` (agent),
`scripts/tasks_drive.py` (tasks), `scripts/panic_drive.py` and the
norecover heredoc (panic). Leg lists were read from each script's
`run_one` lines; tasks-drive's two feature sets from its `if` (`boundaries`
before T34.2, `everything` after).

**One re-run artefact, found and corrected:** the first panic-drive runs
went red with `site 1: location (lib.rs, 159) != (lib.rs, 177)`. The
launcher runs a drive with HEAD's worktree as its directory, and the old
`panic_drive.py` reads `fixtures/panic-app/src/lib.rs` by a relative
path — HEAD's file, whose line numbers moved. Re-run with the drive's
directory set to the scratch worktree, all six panic legs are green
(above). No other drive reads a relative file.
