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
