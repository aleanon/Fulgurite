# Results: M89 — more than one window, and none at all

*M89, `docs/plan-windows.md`. Owns every figure that plan cites. Machine:
i9-13900KF. Created by T89.0 (2026-10-06).*

## T89.0 — the baselines (2026-10-06, at `a796eb7`)

What a one-window build reads before `Win` becomes a set of slots. K89c
(`docs/plan-windows.md`, "Kill rules") holds T89.1 to these: with one
window, `just idle-count` may read no more frames than here, and `just
motion-present` no more than 2 presents per 200 motions. K89c replaces
K3 for M89 because K3's arms build neither `Win` nor a `Session`.

**Every row was run on a private nested X server** (Xephyr, 1280×900, no
window manager), as `docs/results-frame-cost.md` "Idle" requires for the
idle counts: on a shared display other windows' events are frames too.
`just motion-present` starts its own Xephyr; `just idle-count` and `just
tasks-drive` were run with `DISPLAY` pointing at one. That also kept
`tasks-drive`'s clipboard writes off the desktop's clipboard.

### `just idle-count` — one window, 60 s (`examples/tasks-demo`, release, `Boundaries`)

Two runs, back to back; the spread is min–max of the two.

| phase, 60 s | frames | loop-thread wakes | process context switches | threads alive / created |
|---|---:|---:|---:|---|
| still (nothing scheduled), run 1 | **0** | 2 | 4 | 4 / 0 |
| still, run 2 | **0** | 2 | 4 | 4 / 0 |
| a 120 ms timer (500 periods), run 1 | **1000** | 2050 | 2552 | 5 / 0 |
| a 120 ms timer (500 periods), run 2 | **1000** | 2049 | 2551 | 5 / 0 |

- **Frames are exact**: 0 still, and two a period with the timer (1000
  for 500 periods), the figures `docs/results-frame-cost.md` "Idle"
  explains. K89c's bound is these frame counts, not a spread around
  them.
- Wakes and context switches are printed for comparison; they moved by
  one between the two runs. Both are below the M47 figures in that file
  (3/9 still, 2413/2916 timer, also on Xephyr); the difference is not
  attributed, and K89c does not bound them.

### `just motion-present` — 200 motions over a still region (`examples/showcase`)

`scripts/motion_present.py`, under both strategies. The redraws column is
the setup check: the motions reached the window only if presented +
skipped is at least 20.

| leg | presents for 200 motions (bound 2) | redraws that presented nothing (floor 20) | crossing a button's edge (want 1) | covered and uncovered |
|---|---:|---:|---:|---|
| Everything | **0** | 200 | 1 | 3 presents, 1 skipped; surface == canvas |
| Boundaries | **0** | 200 | 1 | 3 presents, 1 skipped; surface == canvas |

### `just tasks-drive` — unobserved ticks

The bound is `tasks_drive.py`'s `want − 1` (`want` = elapsed ÷ 120 ms),
which read 25–26 of 26 at T47.1 (`docs/plan-timers-wake.md`, C31).

| leg | ticks after 3001 ms unobserved | want | bound (`want − 1`) |
|---|---:|---:|---:|
| Everything | **26** | 25 | 24 |
| Boundaries | **26** | 25 | 24 |
| Boundaries + shadow oracle | **26** | 25 | 24 |

`want` is rounded down, so 26 against 25 is inside one period of the
3001 ms read; the bound is what T89.1 and T89.6 are held to.

## T89.1 — one window as slot 1 (2026-10-06)

`Win` is shared state plus a `Vec<WindowSlot>` with one entry. K89c,
read on the same Xephyr setup as T89.0:

| quantity | T89.0 baseline | T89.1 | bound | verdict |
|---|---:|---:|---|---|
| idle-count still, frames in 60 s | 0, 0 | **0** | ≤ baseline | holds |
| idle-count 120 ms timer, frames in 60 s | 1000, 1000 | **1000** | ≤ baseline | holds |
| motion-present, presents for 200 motions (Everything / Boundaries) | 0 / 0 | **0 / 0** | ≤ 2 | holds |
| motion-present, skipped redraws (setup floor 20) | 200 / 200 | **200 / 200** | ≥ 20 | setup landed |

Not bounded by K89c and printed for the record: the timer phase's
loop-thread wakes read 2062 and process context switches 2564, against
2049–2050 and 2551–2552 at T89.0 (two runs) — 12 more over 60 s, about
0.6%, with the same 1000 frames. Still: 2 wakes, 4 context switches,
as at T89.0.

**Injection** (the plan's): `own_redraw` set on a flag outside the slot
that the slot's frame never reads or clears. `motion-present [Everything]:
presents 200 for 200 motions (bound 2); redraws that presented nothing 0`,
and the assertion fires by name.

## T89.3 — window programs, open and close (2026-10-06)

**The `OpenProgram` cost, recorded** (`docs/plan-windows.md`, "A window
program is a table the program carries"): `OpenProgram::session(recover)`
instantiates both `Local<_, Recovering>` and `Local<_, Crashing>` for each
table entry, so a release build **without** `reload` that declares window
programs links M36's recovery code. Counted as `just release-window-shape`
counts it (`nm -C`, defined `lumen_static_shell::recover::` symbols):

| release build, no `reload` | `recover::` symbols | bytes | `.text` |
|---|---:|---:|---:|
| `examples/minimal` (no window table; `just release-window-shape`) | **0** | 0 | 2,828,326 |
| `examples/two-windows` (one table entry) | **9** | 13,708 | 3,850,342 |

`examples/minimal` stays at 0, the bound `release-window-shape` holds;
two-windows' `.text` is not comparable with minimal's (it links the
shaper for its `Complex` detail). The reopen condition is the plan's: a
request for multi-window in a release build where M52's bytes matter.

**The live gates** (`just windows-drive` phase 2, `examples/two-windows`,
release with `reload`; Everything and Boundaries<256> alike; Xephyr):

| gate | read | bound |
|---|---|---|
| a click on `#row2`: the reply's `opened`, `#opened`, `app.windows` | `[2]`, `2`, [1] → [1, 2] | `[2]` |
| `#detail-bump` with `"window": 2`; the list untouched | detail 1; list (`0`, `2`) unchanged | 1 |
| XTest click at `#detail-bump` in window 2 | detail 2 | 2 |
| `#rtl` (Complex) ink against its box | 82.0 px in 84.0 px, \|Δ\| 2.0 | ≤ 3.0 |
| listening sockets of the process | 1 | 1 |
| panic in window 2: R pressed there (XTest) | failure cleared; window 1 never failed | cleared |
| `app.close` 2, Escape in 3, `#detail-close` in 4 | `app.windows` [1] each, list answers | [1] |
| `app.close` on window 1 | exited, < 1 ms after the reply | 2000 ms |

`just a11y-drive`'s two-windows leg: two frames for the pid, 5 and 3 push
buttons, equal to each window's agent `Button` count.

## T89.4 — data between windows, `Request::Post` (2026-10-06)

**`Request` is 32 B** (asserted in `lumen_static::request`): `Post`'s
24 B `Descriptor`, its `u32` key and the discriminant, padded to 8 — it
was 16. Per `Requests` that is 8 × 32 = 256 B, not per node; the firmware
links none of it (`nm`: 0 symbols matching
`OpenWindow|CloseWindow|Posted|WindowOpened|OpenProgram` in each
`scripts/lib/firmware.sh` binary, and the strict `just firmware-delta`
reads +0 on all four).

**The live gates** (`just windows-drive` phase 3, `examples/two-windows`,
release with `reload`; Everything and Boundaries<256> alike; Xephyr):

| gate | read | bound |
|---|---|---|
| rows 2, 1, 3 each open a detail; its `#item` / `#from` | `item 2`, `item 1`, `item 3`, each `from 1`, within 0–4 ms | the row clicked, from 1 |
| `app.windows` `posts` after the three opens | delivered 3, dropped 0 | 3 / 0 |
| idle window 4, `#send` on window 1: window 4's `app.presents` | +1 (Boundaries 2 → 3, Everything 3 → 4) | +1 |
| idle window 4: `ui.regionHash` over its `#item` (1,122 px) | changed | changed |
| `#send` to window 4 after `app.close` 4 | dropped +1, delivered unchanged | +1 / +0 |

Under "no redraw request on the target slot" the present count stays
(3 → 3) and so does the region hash, while `ui.resolve #item` already
reads the new text — `Session::answer` applies the message at once, so the
tree is ahead of the pixels; only the pixels and the present count can
see a missing redraw.

## T89.5 — per-window answers (2026-10-06)

Both producers had landed (M88's screenshot and clipboard requests,
M80/T80.5's IME area), and **no code was missing**: since T89.1 both
drain sites pass the draining slot's session with that slot's backend,
and since T89.2 `app.imeArea` reads the slot the request names. T89.5 is
the gate that holds them there, and the rustdoc that says so. The
clipboard is the process's one.

**The live gates** (`just windows-drive` phase 4, `examples/two-windows`,
release with `reload`; Everything and Boundaries<256> read alike; Xephyr):

| gate | read | bound |
|---|---|---|
| the detail's `Screenshot`, as its title shows it | `300x400 1c067c9134d0487a` | = window 2's `ui.screenshot` |
| `ui.screenshot` with `"window": 2`, FNV-1a of its RGBA | `300x400 1c067c9134d0487a` | equal |
| `ui.screenshot` with `"window": 1` | `340x440 73833b4fd5595075` | different |
| `#field` focused in window 2: `app.imeArea` `"window": 2` | `[8.0, 126.0, 9.0, 147.8]`, centre 8.5 | non-null |
| its painted caret column (window 2's screenshot) | 8.5, \|Δ\| 0.0 physical px | ≤ 1.0 |
| `app.imeArea` `"window": 1` | `null` (the list has no field) | `null`, ≠ window 2's |

Under "answer every request from slot 0's backend" the detail's
screenshot reads `340x440 73833b4fd5595075`, window 1's; under
"`app.imeArea` reads slot 0's area" window 2 answers `null`, window 1's.

## T89.6 — `daemon(boot)` and the windowless pump (2026-10-06)

**The live gates** (`just windows-drive` phase 5, `examples/daemon`,
release; Everything and Boundaries<256> alike; Xephyr). The launcher's
`app.diagnostics` identity check passed with no window open in every leg.

| gate | read | bound |
|---|---|---|
| `app.diagnostics` with no window | answered by window 0 | 0 |
| `#ticks` on window 0, one read before and one after 3.0 s with no call that runs a frame | 25–26 ticks in 3,009–3,058 ms (want 25) | `want − 1` = 24 |
| `app.windows` over those 3 s | [0] → [0, 1] at +1,048–1,179 ms → [0] at +2,267–2,389 ms; alive | tick 10 / tick 20 |
| Escape on window 0 (`KeyOutcome::Quit`) | `app.windows` [0]; 1 stderr line naming `Request::Quit` | [0]; 1 |
| `Request::Quit` at tick 40 | the port closed at about tick 39–40 (estimated from the drive's clock); exit 0 | ≥ 38 (40 less the estimate's 2); 0 |
| no endpoint at all | ran to its own Quit, exit 0 in 4.9 s | exit 0 within 10 s |

**K89d** (`just idle-count`'s daemon leg, 60 s, release, Boundaries,
Xephyr):

| leg | frames | loop-thread wakes | process context switches | bound | verdict |
|---|---:|---:|---:|---|---|
| `daemon --still` (no deadline) | **0** | 1 | 3 | 0 frames; wakes ≤ 2, the two reads' own | holds |
| `daemon --steady` (120 ms deadline, 500 periods) | **500** | 501 | 503 | periods + 1 = 501 | holds |

- **One frame a period**, where `examples/tasks-demo`'s 120 ms timer
  draws two (`docs/results-frame-cost.md`, "Idle"): the daemon's timer is
  its own deadline (`App::wake_in_ms` and `App::tick`), which needs no
  job, so there is no second, wake-driven frame. Its loop thread wakes
  about once a period (501), against about four for the window (2,049–2,062
  at T89.0–T89.1).
- **The bound is periods + 1**, not periods: an interval of 500.x periods
  holds up to 501 deadlines. The first run, bounded at the period count,
  read 501 against 500 and fired; the quantity was the fencepost, not a
  second frame (`docs/plan-windows.md`, What building found).
- **Injection** (every windowless step asks for the next at once):
  `--still` drew 2,939,304 frames in 10 s and `--steady` 2,914,247 for 83
  periods, and K89d fired by name on both.

**The injections** (the plan's three, each on the Boundaries leg):

| injection | what the drive printed |
|---|---|
| pump the windowless session only inside a slot's redraw | `#ticks 0 -> 0: 0 ticks after 3060 ms` — "the windowless session was never pumped" |
| skip the drain after a windowless frame | `#ticks 0 -> 26`, `app.windows [[0]]` — "app.windows stays [0] after tick 26 … never drained" |
| treat a windowless `wants_quit` as `Request::Quit` | (b), (c) pass; "(d) the agent's port was lost after Escape on window 0" |
| no first windowless frame asked for (the defect found) | no-endpoint leg: still running after 10 s, killed (−15) |

## T89.7 — hot reload with several windows (2026-10-06)

**The live gates** (`just hot-window-drive`'s `windows` legs, a copy of
`fixtures/hot-window`, release `hot agent shaping`; Boundaries<256> and
Everything alike; Xephyr). The copy is broken before launch, so the
window's first build fails and nothing swaps until the drive's fix.

| gate | read | bound |
|---|---|---|
| the panel opened at `app.reloads` 0, bumped 3; the primary bumped 2 | `panel 3`, `count 2` | setup |
| the first swap (fix + panel format, one write): the panel / the primary | `panel v2: 3` / `count 2`, swapped in 1.05–1.28 s | the new format, the kept count; the primary's own |
| the panel's root box against its inner size | 260 × 180 / 260 × 180 (the primary is 320 × 400) | ±1 px |
| over 1.2 s after the swap: the panel's presents / the primary's redraws | +8 / +0 (41 → 49, 13 → 13; Everything 38 → 46, 13 → 13) | ≥ +3 / +0 |
| a panel opened after the swap | `panel v2: 0` | the loaded build's format |
| v3 (the primary's format and a panic in the panel's boot, one write) | `app.reloads` 1 → 1; primary `count 2`, panel `panel v2: 0`; error names window program 1; the bar up | refused whole |

**The daemon leg** (`examples/daemon` built `hot`, `--still`, a copy): the
first build swapped (`app.reloads` 1 on window 0, `app.windows` [0]); then
the drive edited the copy, and that build landed with no window, no timer
and no agent call that runs a frame: `app.reloads` 2 after 1.0 s.

**The injections** (the plan's six, each red by name):

| injection | what the drive printed |
|---|---|
| (a) open new windows through the build the window started with | "(4) the late panel reads 'panel 0' … shows v1's format" |
| (b) `swap_all` swaps slot 0 only | "(2) … panel 'panel 3' … kept the OLD format after the swap" |
| (c) replace sessions as each one boots | "(5) the primary runs v3 ('v3 count 2') while the panel runs v2 ('panel v2: 0')" |
| (d) open pre-swap windows through `session(recover)` | "(2) … panel 'panel v2: 0' … its kept state was not carried" |
| (e) adopt every slot with slot 0's waker and size | "(3) the panel lays out at [320.0, 400.0], not its own size [260.0, 180.0]"; with the waker alone: "(3) the panel's timer redrew the primary: (10, 8) -> (10, 16)" |
| (f) `Built` calls only `ask_redraw` | daemon: "app.reloads on window 0 1 (want 2) … pending True … nothing scheduled the windowless step" |

The existing `hot`, `static` and `everything` legs passed unchanged on the
fixture with its panel. The GPU legs were not run (no adapter on the
private display; T89.7 names none).

## T89.9 — M89b's baselines, one GPU context per window (2026-10-06)

Read on the user's display (`:0`, the RTX 4070: Xephyr has no adapter),
off the monitor being read and without focus (`scripts/drive_window.sh`).
Every GPU window still makes a context of its own; T89.10 shares one, and
K89a and K89b (`docs/plan-windows.md`, "Kill rules") are judged against
these rows.

**`just resume-drive`'s `two-windows-gpu` leg** (`scripts/resume_windows.py`;
a debug `examples/two-windows` with `LUMEN_BACKEND=gpu`, the list and an
opened detail, one `LUMEN_TEST_RESUME` round trip at 8,000 ms; each window
found again by its process and title and read back with `xwd -id`):

| quantity | read | bound |
|---|---|---|
| window 1 (list, 340 × 440): differing pixels after the round trip | **0** / 149,600 (0.0000%) | 0.1% (149) |
| window 2 (detail, 300 × 400): differing pixels after the round trip | **0** / 120,000 (0.0000%) | 0.1% (120) |
| backends made before it / by it | **2 / 2** (one per window per resume) | 2 / 2 |
| setup: distinct colours before → after (list / detail) | 532 → 532 / 450 → 450 | > 1 |
| setup: the first reads, before the round trip | 4.20 s into the drive | before the hook's line |

- **The leg found a defect on its first green attempt**: both windows
  came back one flat colour, **100%** of their pixels differing (149,600
  and 120,000). A GPU backend asked its session for damage at age 1 on
  every present, so a backend made by a resume — whose target holds no
  frame — presented an empty target under a `Boundaries` session with
  nothing new to draw. Its first present now asks for age 0, the whole
  surface (`GpuBackend::present`). glasswall's `gpu-resume` leg never saw
  it: its 997 px window is resized by the window manager on the 1,080 px
  monitor, and a resize resets retention.
- **Injection** (the plan's: skip the second slot's `resume_backend`):
  window 2 differs on 120,000 / 120,000 (100.0%, 43 distinct colours, the
  X server's uninitialised window), window 1 on 0, and backends read
  "2 before the round trip and 1 by it (want 2 and 2)"; the leg fails
  naming both.
- The hook is at 8,000 ms, not glasswall's 4,000: a debug launch opening
  two contexts took most of 4 s, and the first reads straddled the round
  trip (`docs/plan-windows.md`, What building found).

**`just windows-rss`** (`scripts/windows_rss.sh`; release
`examples/two-windows`, `VmRSS` 4 s after start with one window and 4 s
after the click on `#row2` opened the second, median of three launches,
`docs/results-gpu.md` §5's method):

| backend | one window | two windows | second window's delta |
|---|---:|---:|---:|
| software | 29.6 MB | 104.0 MB | **+74.5 MB** |
| GPU, a context per window | 197.8 MB | 290.3 MB | **+91.9 MB** |
| GPU over software | +168.2 MB | | **+17.5 MB** |

Raw launches, kB (one → two): GPU 236,268 → 328,600, 202,500 → 296,624,
202,352 → 297,316; software 30,180 → 106,468, 30,304 → 106,492, 30,264 →
106,516.

- **The first GPU window's +168.2 MB** is §5's context cost (174 MB on
  `examples/minimal`, `docs/results-gpu.md` §5), re-read on this example.
- **The second window's delta is mostly not the GPU's.** The software
  canvas pays +74.5 MB for it too: the detail lays an Arabic label out,
  so its font stacks (the session's and the renderer's, one per slot)
  load fallback faces until one has Arabic. The second GPU context's own
  share is the difference of the two medians, **+17.5 MB**. K89b's
  comparison is printed both ways — half of the raw delta, 46.0 MB, and
  half of the GPU share, 8.7 MB — because a shared second window still
  pays the 74.5 MB the software one pays, so the raw form could not tell
  a shared context from a separate one (`docs/plan-windows.md`, What
  building found).

## T89.10 — one GPU context shared by the windows: K89b fired, not shipped (2026-10-06)

`GpuContext` shared by `Rc` with a surface per window was built and
measured, and **K89b fired on its quantity as written**. Following its
response, every window keeps a context of its own (M89a's behaviour, as
committed at T89.9); the implementation is kept on the side branch
`t89.10-shared-gpu-context` (`9cc2e22`), not on this one. Read on `:0`,
the RTX 4070, as T89.9.

**K89a** (`just resume-drive`'s `two-windows-gpu` leg on the shared build):

| quantity | read | bound | verdict |
|---|---|---|---|
| window 1 / window 2 differing pixels after the round trip | **0** / 149,600, **0** / 120,000 | 0.1% each | holds |
| panics across the leg | 0 | 0 | holds |
| backends made before / by the resume | 2 / 2 | 2 / 2 | setup |
| backend lines | `shared context (1 surface)`, `(2 surfaces)`, then the same after the resume; every surface Bgra8Unorm | window 2's the second surface | the context was shared |
| `#swatch` centre, each program's `ImageId(0)` (before and after) | list (208, 48, 48), detail (32, 112, 208) | each its own | holds |

**K89b** (`just windows-rss` on the shared build, median of three; T89.9's
rows are the separate-context arm):

| | separate contexts (T89.9) | shared (T89.10) | ratio | bound | verdict |
|---|---:|---:|---:|---|---|
| second GPU window's `VmRSS` delta (K89b as written) | 91.9 MB | **75.4 MB** | **82%** | < 50% | **fires** |
| software canvas's second window, same launches | 74.5 MB | 74.5 MB | | | the control |
| the second window's GPU share (GPU delta − software delta) | 17.5 MB | **0.9 MB** | **5%** | < 50% | holds |

Raw launches, kB (one → two): shared GPU 202,436 → 279,624, 202,264 →
279,440, 202,380 → 280,596; software 30,336 → 106,612, 30,372 → 106,632,
30,284 → 106,616. The first GPU window is unchanged (197.6 MB against
197.8 at T89.9).

- **Sharing removed 95% of what a second GPU window cost**, 17.5 MB to
  0.9 MB. K89b as written cannot see it: 74.5 MB of either arm's delta
  is the detail program's fallback faces, which the software canvas pays
  identically (T89.9), so the raw ratio is bounded below by 74.5 / 91.9 =
  81% whatever the GPU does — a bound computed from the wrong quantity,
  recorded at T89.9 before this measurement was taken.
- **The kill rule's response was followed regardless**, because a kill
  rule that is re-read after it fires is not one. Whether K89b should be
  judged on the GPU share — and the sharing shipped from the side branch —
  is the owner's call (`docs/plan-windows.md`, What building found).
- Not run on the shared build, because it is not shipped: `just
  gpu-parity`, and the plan's two injections (the image atlas in
  `GpuContext`; one shared `target`).
