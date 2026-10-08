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

## selftest-slow (2026-10-09)

Its two plants (`scripts/selftest/plants/slow/`) each run `just
const-budget` three times with a quadratic planted in the const tables,
and each run compiled to the end: 350 s at `58865bc`. A timed build past
5,000 ms is red whatever it would have taken, so the build is now stopped
at 6 s (`timeout`), and stopping it is the red.

| measure | before | after |
|---|---:|---:|
| `just const-budget`, unplanted | 499–590 ms, pass | 499 ms, pass |
| `scripts/selftest/run.sh slow` | 350 s | **44 s** |
| each planted run | ran to the end, red | `stopped at 6003 ms, over the 5000 ms budget`, red |

Both plants still read `run2 red=1 run3 red=1`, the reruns the plant
exists for (R11-F5).

## idle-count (2026-10-09)

The window leg settles 20 s and then watches 60 s twice, and the two
daemon legs watch 60 s each: 266 s in `just ci`. Only the daemon legs
assert anything (K89d); the window leg's counts are the Idle figures in
`docs/results-frame-cost.md`, measured at 60 s, which a bare `just
idle-count` still does. `just live` now passes 15 s.

| `scripts/idle_count.sh 15`, on `:73` | result |
|---|---|
| as committed | exit 0, **87 s**; daemon still: frames 0, loop wakes 1 (bound 0, 2); daemon timer: 125 frames for 125 periods (bound 126) |
| injected: the still daemon's `wake_in_ms` answers 5000 ms | exit 1, `K89d FIRED`, frames 3 (bound 0), loop wakes 4 (bound 2) |

What the shorter window gives up is a stray wake rarer than one in about
15 s. The daemon's own legs and the 60 s run catch that.

## Tier 1 (2026-10-09)

`just ci-fast` is what `.github/workflows/ci.yml` runs, as three
parallel jobs: lint (`static`, `deny`, `release-shape`, `lint-fast`),
test (`test-fast`) and live (`live-fast`). `CARGO_PROFILE_DEV_DEBUG` is
`line-tables-only`, not `0`, because at `0`
`panic-app::recover every_site_is_caught_reported_and_recovered_from`
fails with "no application frame in the backtrace".

**On the reference machine, 4 CPUs** (`taskset 0-3`,
`CARGO_BUILD_JOBS=4`, `CARGO_INCREMENTAL=0`), with dependencies cached
and the workspace crates cleaned, as rust-cache leaves them:

| recipe | full debug info | line tables |
|---|---:|---:|
| `static` | 9 s | – |
| `deny` | 1 s | – |
| `release-shape` | 15 s | 15 s |
| `lint-fast` (clippy) | (inside `test-fast`) | 23 s |
| `test-fast` | 215 s (clippy included) | 143 s |
| `live-fast` | 65 s | 67 s |

**In a clean `ubuntu:24.04`**, one container per job, each on its own
4 CPUs, as user `runner`, from no cache at all (the first run's worst
case). Each container installs only what its job's steps install,
on top of git, curl, a C toolchain, python3 and sudo. What it found
before it went green:

| finding | fix |
|---|---|
| `check_locks` called a good lock stale: `--offline` with no index | asks again online, `--locked` (`3aae94c`) |
| `check_tray_cost`: `cargo tree --offline` with no sources | `check_locks` fetches them first (`2c94799`) |
| `deny`: no advisory database; resolved offline too | fetched when absent; a non-verdict is named (`2c94799`) |
| `static`: `scripts/ui_check.py` needs Pillow | `python3-pil` in the job |
| `live`: the launcher runs `ss` | `iproute2` in the job |
| `markdown_bullet a_bullet_is_centred_on_the_x_height`: 1.5 px off on DejaVu, bound 1.0 | Noto in the job; `BULLET_DROP` is a constant for one face (its rustdoc says so); deriving it from the face is not built |

**Green from nothing, at `1459051`**, three containers in parallel:

| job | steps | wall, no cache |
|---|---|---:|
| lint | `static` 15 s, `deny` 1 s, `release-shape` 18 s, `lint-fast` 40 s | 86 s |
| test | `test-fast` 183 s, 1,701 passed | 194 s |
| live | `live-fast` 146 s | 157 s |

The push waits for the slowest job, about 3 min 15 s here, before apt and
toolchain setup. A GitHub vCPU is slower than an i9 thread, and
rust-cache removes most of the dependency build, which is most of each
figure. **Not yet measured on GitHub** (no remote). One test-job run died
on `rustc interrupted by SIGSEGV` inside LLVM's bitcode reader, building
a test that compiled in the run before and the run after. That joins the
compiler SIGILL already recorded on this machine (AGENT.md, M89b) as a
host fault, not a repository one.

## `just ci` at the end (`1459051`, warm target)

`just ci-verify 1459051` green: 30 steps, 0 failed, 0 skipped.

| step | `ac98b18` s | `1459051` s |
|---|---:|---:|
| `live` | 7 060 | 889 |
| `selftest-live` | 597 | 205 |
| `adversary-plant` | 420 | 85 |
| `selftest-slow` | 341 | 43 |
| `dev-shape` | 245 (cold) | 90 |
| `selftest-build` | 216 (cold) | 119 |
| **total** | **9 236** | **1 724** |
