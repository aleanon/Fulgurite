# Results: CI time (2026-10-08)

The owner asked for `just ci` to fit a GitHub runner -- "a few minutes
MAX" -- and chose (2026-10-08) to keep the full suite local (`just ci`)
with GitHub running a fast subset. This file owns the numbers.

## Where the time went (`ac98b18`, the last green record)

| step | s |
|---|---:|
| `live` | 7 060 (of which `widget-drive` 6 247) |
| `selftest-live` | 597 |
| `adversary-plant` | 420 |
| `selftest-slow` | 341 |
| `dev-shape` | 245 |
| `selftest-build` | 216 |
| everything else | < 65 each |
| **total** | **9 236** |

## Fix 1: one write per agent reply

`docs/results-agent-transport.md` §8: every request cost ~41.6 ms of
Nagle and delayed ACK. One `Boundaries` leg ~1 000 s -> 158 s.

## Fix 2: hover intent delivered, not slept for

After fix 1, 119 s of the 158 s leg was `time.sleep`: 76 s the
secondary-button leg's 0.7 s rest after each hover (109 of them), so the
hover intent `LINGER_MS` later could not land between its two hashes; 22 s
`_settled_hash` polling every 100 ms. `input.hover` with `rest: true`
delivers the intent at once (`Host::rest_now`), and `_settled_hash`
reads the hash, pumps once through `ui.waitSettled`, and reads it again
20 ms later until two reads agree.

**It first waited for `settled`, and that was wrong**: a toast is ticked
on every frame of its 3 s life, so the window reports unsettled while its
pixels hold still, and the wait outlived the toast it was to measure. The
scale-2 leg at `59c32d3` failed `Toast/Save shows a toast` with "no damage
over the widget, 128 frames presented" (175 polls, about 3 s); 3 of 3
runs alone red, the harness before `014d321` green. Without the gate, 3
of 3 green, and both full legs below.

| measure | value |
|---|---|
| `rest_now_delivers_hover_intent_without_waiting`, injected (`rest_now` leaves the clock) | tooltip open `false` -- red by name |
| widget drive `Tooltip`, harness hovering without `rest` or the sleep | 2 FAIL (the intent landed inside the next action) |
| one full `Boundaries` leg, both fixes | **61 s**, 0 failed, 184 actions, secondary checked 88 |
| one full `Boundaries` leg, settle without the gate | 60 s, 0 failed |
| one full `Boundaries+scale2` leg (`LUMEN_DRIVE_FULLCHECK=1`) | 85 s, 0 failed |

The 61 s was taken while nothing else ran a window but with a stopped CI
run's machine just released; it is indicative, not a K86b measurement.

The sleeps left in `scripts/drive/widgets/{image,video,surface}.py` time
real playback and a toast's dismissal; they are what those actions test.

## A hung step

The `e446ea2` run's `gpu` step hung for **21 h**: every test in
`lumen-static-wgpu`'s `parity` binary blocked on a futex, 2 GB of GPU
memory held, beside the NVIDIA driver's own `vkcf`/`vkps`/`vkrt` threads.
**The cause is no reachable X server**, not T89.10: the run's `DISPLAY`
(`:73`) had gone.

| `parity` binary, 30 s timeout | hung |
|---|---:|
| `DISPLAY=:0` | 0 / 38 |
| `DISPLAY=` | 8 / 8 |
| `DISPLAY=`, `--test-threads=1` | 0 / 1 (3.6 s) |
| `DISPLAY=`, at `a8f5b2c` (before T89.10) | 5 / 5 |
| `DISPLAY=`, under `cargo nextest` (a process per test) | 0 / 1 |

So: concurrent headless device creation in one process, with no display,
deadlocks inside the driver. `just ci` now bounds every step
(`CI_STEP_TIMEOUT`, default 3600 s), so a hang costs an hour at most and
names its step.

## nextest

The workspace's default shape, built, `--tests --lib --bins`, `DISPLAY=:0`:

| runner | wall | tests |
|---|---:|---|
| `cargo test` (410 binaries, one at a time) | 39.7 s | 1 731 passed |
| `cargo nextest run` (a process per test) | 7.85 s | 1 731 passed, 5 skipped |

The 5× is test execution only; most `cargo test` steps in `just ci` are
compile-bound. nextest does not run doctests, so `cargo test --doc` runs
beside it. `scripts/check_test_counts.sh` lists tests from a `--no-run`
build and needed no change.

**Adopted (2026-10-09)** where it pays or where it prevents a hang:

- `dev-shape`'s workspace run, `cargo test --workspace --exclude
  lumen-static-wgpu`, measured 36.5 s for 1,715 tests. It is now `cargo
  nextest run` (1,701 tests, 7.4 s) plus `cargo test --doc` (14
  doctests, 2.2 s). Three nextest runs were all green, with no flakes.
- `gpu`'s whole-binary runs (`-p lumen-static-wgpu`, the `shaping`
  `parity`, the `shader` `shader`/`parity`/`alloc`). `just gpu` with
  `DISPLAY` empty passes now; under `cargo test` it hung at the `shaping`
  `parity` line, killed after 8 min.
- `.config/nextest.toml`: a test is flagged slow at 60 s and killed at
  4 min. The slowest test in the workspace takes 7.1 s (a trybuild UI test).
- `cargo-nextest` is a required tool: `scripts/probe.sh nextest`, in `just
  ci`'s probe, exits 1 with the install line when it is missing.
- Kept on `cargo test`:
  - the single-crate and single-test lines that grep `--nocapture`
    output or `--list`, which are compile-bound;
  - `scripts/test_crates.sh`, which builds each crate alone to catch
    feature unification and parses libtest's counts.

| injection into `just dev-shape` | result |
|---|---|
| a test asserting `1 + 1 == 3` | exit 100, `FAIL lumen-static-text::zz_inject injected_failure` |
| a test sleeping 30 s, limit set to 2 s × 2 | exit 100, `TIMEOUT [4.003s] … injected_hang` |
| a doctest asserting `1 + 1 == 3` | exit 101, `lib.rs - (line 1) ... FAILED` |
| `PATH` without `cargo-nextest`, `probe.sh nextest` | exit 1, the install line |

## `just ci` after the fixes (`59c32d3`, warm target)

| step | `ac98b18` (cold) s | `59c32d3` s |
|---|---:|---:|
| `live` | 7 060 | 1 175 |
| of which `widget-drive`, six legs | 6 247 | 395 |
| `selftest-live` | 597 | 205 |
| `adversary-plant` | 420 | 85 |
| `selftest-slow` | 341 | 352 |
| **total** | **9 236** | **2 138** |

The cold run at `071f32a` (whose `live` stopped at once on the list
check) measured the compile-bound steps: `dev-shape` 267, `selftest-build`
227, `tray-symbols` 112. `live`'s largest scripts after `widget-drive` are
`idle-count` (266 s, a timed idle window by design), `hot-window-drive`
(105) and `tasks-drive` (72).
