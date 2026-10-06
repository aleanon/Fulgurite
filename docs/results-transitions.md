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

## T86.1 — the level in `NodeState`, and the blend in `shaded` (2026-10-06)

`Style::transition` (tens of ms, byte 82), `Patch::transition_ms`,
`Fields::TRANSITION` (bit 20, `Fields::EACH` 20 → 21),
`NodeState::pointer_mix`, `ops::MIX_LEVELS` and `ops::encode_mix`, the
blend in `ops::shaded` behind the additive core feature `transitions`
(enabled by `lumen-static-shell`, never by `fixtures/embedded`), and
`.lss` `transition-duration` (`Takes::Millis`, `Val::Millis`, `Unit::Ms`).
Nothing writes `pointer_mix` yet except tests.

| gate | prints | injection (red) |
|---|---|---|
| `crates/lumen-static-shell/tests/transition_shade.rs`, endpoints: 64 colours × `hover_dim` {0, 0.07, 0.12} × `pressed_dim` {0, 0.15} = 384 cases, at the four encoded level pairs against the bit path | **0 of 1,536** differ (bound 0); every `pointer_mix` written has bit 15 set | `shaded` ignoring the press level: **376 of 1,536** differ, first `#ffffffff`, `pressed_dim` 0.15, pressed: bit path `(216,216,216)`, blend `(255,255,255)` |
| the same file, a level between: `#c8783c`, `hover_dim` 0.12, `h = 64/127` | red channel rest **200**, mid **188**, hover **176** (strictly between) | — |
| the same file, interior: `h = p = 64` over the 384 cases against `a + ((b − a)·t + 63.5)/127` floored, in `f32` | **0 of 1,152** channels differ (bound 0) | a truncating `lerp` (`a + (b − a)·t/127`): **584 of 1,152** differ, first `#ffffffff` channel 0 got 236, want 235. The endpoint check stays **0 of 1,536** under it — truncation is exact at `t = 0` and `t = 127` |
| `crates/lumen-static/tests/cursor.rs` and `focus.rs`, sizes (K86d, K1) | `Style` **84** B (82 used), `NodeState` **88** B (88 used), **172** B/node, **16.8 KB** per 100 nodes (parent 84 / 84, 168 B, 16.4 KB; bound 84 / 88 / 172, 24 KB); `Cmd` **60**, `Event` **16** | `pointer_mix: u64`: `NodeState` **96** B (92 used), 180 B/node, fails by name |
| `fixtures/embedded/src/lib.rs` on the target | `NODE_STATE == 88`, `STYLE == 84`, `PER_NODE_RAM == 172`; `Ui` **1,216** unchanged | — |
| `lumen-static-lss` unit tests | `transition-duration: 150ms` → `transition_ms(150)`; `155ms`, `2560ms`, `150` and `150.5ms` refused at 1:27 with `Takes::Millis`'s description; accept: `0ms` → `Millis(0)`, `2550ms` → `Millis(2550)` | — |
| `scripts/check_lss_setters.sh` | `Patch` **26** setters, the table **26** | the `transition-duration` row removed: "Patch setters with no .lss declaration: ['transition_ms']", exit 1 |
| `crates/lumen-static-shell/tests/lss_parity.rs` (`pass_accept.lss` gains `transition-duration: 150ms` on `#title, #badge`) | `include_lss!` and `lss::load` agree, 6 and 7 rules | the shell's `set` arm dropped: `load` panics, "no setter `transition_ms` taking Millis(150)" |
| `crates/lumen-static/tests/sheet_lint.rs` (`W1010`) | on `#go` (a `Button`): **0** findings; on `Kind("fixed")`: `Unread(TRANSITION)` | `Fields::TRANSITION` dropped from `Button`'s seed: "a transition on a `Button` is read: [Unread(Fields(1048576))]" |
| `crates/lumen-static-widgets/tests/style_reads.rs` — `transition` changes no frame, so it counts as read exactly when `hover_dim` or `pressed_dim` is measured as read | every kind's declared mask equals the measured set (`label`, `button`, `checkbox`, `radio`, `switch`, … measure `transition`) | `Fields::TRANSITION` dropped from `Button`'s seed: the mask check fails |
| the `justfile` lint line, `cargo clippy -p lumen-static --features transitions --all-targets -- -D warnings` and its `cargo doc` | green | an unused binding inside `blend`: "unused variable: `unused_in_blend`", the line fails; `cargo clippy -p lumen-static --all-targets` (default features) stays green |
| `transition_oracle.rs` (T86.0), now comparing `pointer_mix` too | **65** frames, **0** differ | — |

**Seeds that declare `Fields::TRANSITION`**, from `grep -rlE
'HOVER_DIM|PRESSED_DIM' --include=*.rs` at landing (not `style.rs`):
**17 sites in 15 files** — the derive's container seed, core `Repeat`,
`lumen-static-widgets`' `chart.rs` (2), `input.rs`, `ops.rs` (2: `Label`,
`Button`), `pick.rs`, `rich.rs`, `scroll.rs`, `selectable.rs`, `slider.rs`,
`textarea.rs`, `toggle.rs`, `trim.rs`, `vector.rs`, and
`examples/showcase/src/nav.rs`. The plan counted twelve at `553fa5e`;
`selectable.rs`, `rich.rs` and `vector.rs` arrived with M80 and M82.

**K86a — the firmware does not pay for the blend: holds.** `NodeState`
grew, so the bounds are 64 / 96 / 64 / 96 (`just firmware-delta db138c0
firmware=64 firmware_bind=64 firmware_cascade=96 firmware_interactive=96`):

| binary | `.text` | Δ | bound | `.rodata` Δ |
|---|---:|---:|---:|---:|
| `firmware` | 31,138 | **+40** | 64 | +0 |
| `firmware_interactive` | 63,980 | **+60** | 96 | +0 |
| `firmware_bind` | 34,494 | **+36** | 64 | +0 |
| `firmware_cascade` | 33,230 | **+44** | 96 | +0 |

By a symbol diff: copy and offset changes (`_start` +44 to +48, the
fixture's `place` +8 to +12, the focus walks and `dispatch` +8 to +16 for
the 88 B stride, every `type_defaults` −2 to −12) and `cascade_range`
**+2** for `Patch::apply_to`'s branch (allowance 32). No blend symbol
links. Injection: `features = ["transitions"]` on the fixture's
`lumen-static` line prints **+324 / +344 / +320 / +328** and fails all
four; removed, the figures above return.

**The first spelling fired K86a.** With `pointer_mix: u16` (the plan's
type) the same invocation printed **+140 / +164 / +148 / +144** and failed
all four. The symbol diff put **+142** of it in `_start`: the `u16` left
two bytes of padding in the 88 B struct, and `[NodeState::default(); N]`
stopped compiling to one clear of the array and became a clear per element
plus a store per field. `pointer_mix` is a `u32` whose bits 16–31 are
always 0 — the same 88 B, no padding — and is what shipped.

**K86b — unused costs nothing: fires after its written response, on a
quiet machine too, so M86 stops at T86.1.** Every tree at `transition ==
0`, the shell enabling `transitions`; eight interleaved runs a side under
one `just bench-lock`, the base side built at `db138c0`. Two measurements,
the first ruled invalid and the second the verdict.

*The first, at landing* (bench-lock's rule called each lock quiet, load1
≤ 2.24 — but a `just ci` run was loading the machine, so the coordinator
ruled it not quiet and asked for a re-measure). With the blend inlined
into `shaded`:

| row | base median (spread) | tree | |
|---|---:|---:|---|
| `appearance` `1k-toggles/still` | 16,327 ns (14,779–17,524) | **18,664** | **above** |
| `appearance` `1k-toggles/hover` | 14,866 ns (14,774–17,326) | 16,556 | inside (+11%) |

It fired, so the response: `blend` is `#[cold] #[inline(never)]` behind
the `pointer_mix & 0x8000` test. Re-measured, three runs:

| row | base median (spread) | tree | |
|---|---:|---:|---|
| `appearance` `still` | 15,678 ns (14,980–17,878) | 16,342 | inside |
| `appearance` `hover` | 15,122 ns (14,850–17,581) | 16,017 | inside |
| frame arm `idle (floor)`, run A | 61.8 µs (55.8–62.5) | **64.0** (63.6–64.3) | **above** |
| frame arm `idle (floor)`, run B | 59.15 µs (53.3–62.4) | **63.85** | **above** |
| frame arm `idle (floor)`, run C (order reversed) | 60.50 µs (53.3–62.5) | 61.45 | inside |

The struct's growth is not the cost: the tree built with `transitions`
removed from the shell reads `still` 15,655, `hover` 15,151, `idle` 56.50
(run B) and 62.15 (run C), each on the base. What `transitions` adds, with
the blend out of line, is a load, a test and a branch in `shaded` — and
`idle (floor)` lays nothing out, so its two high runs are code placement,
not the branch executing; `layout`, which is not a K86b row, reads 20.40
against 19.20 in both runs B and C (inside the base's 19.0–20.8). Two of
three re-measured runs leave the spread, so by its written response **M86
stops at T86.1** — which the re-measure below confirms.

*The re-measure, on a quiet machine* (2026-10-06, 14:05–14:07; no
`cargo`, `rustc` or `ci.sh` process; load1 **0.99 → 1.07** and **0.90 →
1.00** across the two locks, bound 8). Three trees, each built `--release`
from a clean `git archive` and run as copied binaries, interleaved in a
rotated order (every tree first in some round): base `db138c0` (T86.0),
`e77b7b2` (T86.1, the committed `#[cold]` blend), and `1148444` (T86.1 plus
T85.1's stack fix, which touches derived layout code — measured apart so
the two cannot be confused). Median of eight (min–max):

| lock | row | base `db138c0` | `e77b7b2` | `1148444` |
|---|---|---:|---:|---:|
| 1 | `appearance` `still` | 17,364 ns (14,780–17,444) | **18,027** above | 16,441 inside |
| 1 | `appearance` `hover` | 16,552 ns (14,805–17,715) | 16,116 inside | 15,844 inside |
| 1 | frame arm `idle (floor)` | 58.60 µs (55.8–61.9) | **64.00** above | **63.40** above |
| 2 | `appearance` `still` | 16,511 ns (14,800–17,461) | **18,050** above | **18,148** above |
| 2 | `appearance` `hover` | 14,946 ns (14,761–17,394) | **17,993** above | 15,956 inside |
| 2 | frame arm `idle (floor)` | 57.55 µs (52.9–61.8) | **63.85** above | 61.70 inside |

Every lock has a K86b row above its base spread on both trees, so **K86b
fires on a quiet machine**, and by its written response **M86 stops at
T86.1; T86.1 stands** (it adds no behaviour, and the blend is what T86.2
would need). The stack fix neither causes nor cures it: `e77b7b2` and
`1148444` fire on the same rows within each other's spread. The
appearance rows are bimodal on every tree (one mode near 15,000 ns, one
near 17,500), and both modes sit higher on the trees than on the base —
base `still`'s lower mode reads 14,780–15,644, the trees' 15,696–16,544 —
so the median's crossing is a shift of the distribution, not one mode
outnumbering the other. The frame arm's other rows read inside the base
spread in lock 1 on both trees — `layout`, `+paint`, both `+render` and
`+paint (culled)` each 1.1–2.2 µs higher at the median, the two `ALL
rows` within −6 to +22 µs of base's 1,116 / 1,235; `idle (floor)`, which lays nothing out, reads
63.2–64.3 µs in seven of eight runs on `1148444` against a base maximum
of 61.9. Run C's agreement at landing was the lucky draw.
