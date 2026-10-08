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
delivers the intent at once (`Host::rest_now`), and `_settled_hash` asks
`ui.waitSettled` before two reads 20 ms apart.

| measure | value |
|---|---|
| `rest_now_delivers_hover_intent_without_waiting`, injected (`rest_now` leaves the clock) | tooltip open `false` -- red by name |
| widget drive `Tooltip`, harness hovering without `rest` or the sleep | 2 FAIL (the intent landed inside the next action) |
| one full `Boundaries` leg, both fixes | **61 s**, 0 failed, 184 actions, secondary checked 88 |

The 61 s was taken while nothing else ran a window but with a stopped CI
run's machine just released; it is indicative, not a K86b measurement.

The sleeps left in `scripts/drive/widgets/{image,video,surface}.py` time
real playback and a toast's dismissal; they are what those actions test.

## A hung step

The `e446ea2` run's `gpu` step hung for **21 h**: every test in
`lumen-static-wgpu`'s `parity` binary blocked on a futex, 2 GB of GPU
memory held. It did not reproduce in 33 runs afterwards (2.1-2.4 s
each), and the shared context of T89.10 is `Rc` per window, not shared
across the test threads. It ran while the real-desktop tray drive and
its GPU windows were up. Whatever the cause, a step had no bound.
