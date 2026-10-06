# Transitions: hover, press and focus that move

M86's results (`docs/plan-transitions.md`). Every figure an M86 gate
prints is recorded here once and cited from the rustdoc, the plan and
the commits. The plan-wide budgets and kill rules K1–K4 are
`docs/plan-iced-roadmap.md` §2 and §4; the firmware's four-binary table
is `docs/results-embedded.md`'s.

## Thresholds, written before anything is built (T86.0, 2026-10-06)

M86's kill rules and the bounds its tasks state, copied from the plan so
each later row has its threshold beside it. A row's verdict is filled in
by the task that measures it, never edited here.

| rule | quantity | threshold | measured by | if it fires |
|---|---|---|---|---|
| K86a | each firmware binary's `.text` Δ against the parent, `just firmware-delta <parent> firmware=64 firmware_bind=64 firmware_cascade=96 firmware_interactive=96` if `Style` or `NodeState` grew, `firmware=0 firmware_bind=0 firmware_cascade=32 firmware_interactive=32` if neither did | **0 B**, or ≤ **64 B** of struct Δ (copy and offset changes only); plus ≤ **32 B** on `firmware_cascade` and `firmware_interactive` for `Patch::apply_to`'s branch, attributed by symbol diff; `firmware_interactive` ≤ **81,920 B** | T86.1 | the task stops and finds what linked by a symbol diff; the feature is not made default |
| K86b | `benches --bin appearance` rows `still` and `hover`, and `benches-competitive/run.sh -p lumen-static-frame`'s `idle (floor)`, eight runs a side under `just bench-lock`, every tree at `transition == 0` with the shell enabling `transitions` | inside each row's T86.0 spread (below) | T86.1 onward | the blend moves out of line (`#[cold]`) behind the `pointer_mix != 0` test and is re-measured; if a row still fires, M86 stops at T86.1 and the number is recorded |
| K86c | the new `transition` case in `benches --bin appearance` (the two toggles carry an advancing `pointer_mix`) against `hover` | ≤ **1.10 ×** `hover` | T86.2, T86.4 | T86.4 does not ship: `type_defaults` stay at 0, transitions stay opt-in per sheet, the number is recorded |
| K86d | `size_of::<Style>()`, `size_of::<NodeState>()`, `PER_NODE_RAM` after T86.1 | ≤ **84 / 88 / 172 B** (K1's end-of-plan maxima; 16.8 KB per 100 nodes against 24 KB) | T86.1 | `pointer_mix` becomes a `u8` with 4-bit levels; the quantisation is printed |
| K86e | any action's `ui.waitSettled` in T86.4's full `just widget-drive` | no timeout at 1,000 ms with no transition running | T86.4 | a settle defect: it blocks T86.4 until fixed |
| K4 | one task's `firmware_interactive` `.text` Δ, `just firmware-delta <parent>` | ≤ **1,000 B**; every binary ≤ 81,920 B | every task | the task stops and attributes the delta by a symbol diff |
| K1 | `size_of::<Cmd>()`, `size_of::<Event>()` | **60 / 16 B**, unchanged | T86.1 | the variant moves to a side store |

## T86.0 — the stepped oracle, and the baselines, at `c25dae0` (2026-10-06)

| gate | prints today | injection (red) | used by |
|---|---|---|---|
| `crates/lumen-static-shell/tests/transition_oracle.rs`: one script on a `Host<_, Incremental, Everything>` and a `Host<_, Incremental, Boundaries<256>>` (a `Button`, a `TextInput`, a `Switch`) — hover `#go`, press, release, hover `#sw`, Tab to `#field`, each followed by twelve `tick(16.0)` + `pump()`; every frame's `NodeState` arrays field for field and the lists' `hash_from(0)` | setup landed on every move (hovered **1** = `#go`, pressed **1**, released with clicks **1**, hovered **3** = `#sw`, focus **2** = `#field` with `focus_visible`); **65** frames compared (5 × 13), **0** differ | the `after.hovered != before.hovered` marks deleted from `pump_with` (T25.0): red under `Boundaries` at step 0 frame 0, node 1 (`#go`), on the field **`bg`** | T86.1–T86.3 |

The Tab order puts the field before the switch so one Tab from the
button a press focused lands on it — the fifth move is one event and
the count stays 5 × 13.

**Firmware baseline** (`scripts/firmware_delta.sh c25dae0 firmware=0
firmware_interactive=0 firmware_bind=0 firmware_cascade=0`, tests and
docs only, so Δ **0** on all four). K86a compares T86.1 with its parent.

| binary | `.text` | of 81,920 | headroom | `.rodata` |
|---|---:|---:|---:|---:|
| `firmware` | 31,098 | 38.0% | 50,822 | 13,348 |
| `firmware_interactive` | **63,920** | **78.0%** | **18,000** | 21,890 |
| `firmware_bind` | 34,458 | 42.1% | 47,462 | 13,952 |
| `firmware_cascade` | 33,186 | 40.5% | 48,734 | 14,136 |

**K86b's baseline**, eight runs under one `just bench-lock` (quiet by its
rule: load1 6.58 before, 5.97 after, bound 8 — but a `just ci` run was on
the machine, and runs 3 and 6 read 1.5–1.9 × the rest on both bench
rows). Median of the eight medians, (min–max of them):

| row | median | spread |
|---|---:|---:|
| `appearance` `1k-toggles/still` | 17,342 ns | 15,604–29,002 |
| `appearance` `1k-toggles/hover` | 15,779 ns | 15,573–28,839 |
| frame arm `lumen-static/idle (floor)` (`R=1`) | 62.25 µs | 56.4–102.2 |

The spreads are wide because the machine was shared; K86b is judged
against a fresh base side taken in the same lock as the tree it judges,
interleaved, which is how M85 measured K85a and K85c.
