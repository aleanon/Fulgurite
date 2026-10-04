# The paint vocabulary: gradients, paths, vector text

M82's results (`docs/plan-paint-vocabulary.md`). Every figure an M82
gate prints is recorded here once and cited from the rustdoc, the plan
and the commits. The plan-wide budgets and kill rules K1–K4 are
`docs/plan-iced-roadmap.md` §2 and §4; the firmware's four-binary table
is `docs/results-embedded.md`'s.

## Thresholds, written before anything is built (T82.0, 2026-10-04)

M82's own kill rules and the bounds its tasks state, copied from the
plan so each later row has its threshold beside it. A row's verdict is
filled in by the task that measures it, never edited here.

| rule | quantity | threshold | measured by | if it fires |
|---|---|---|---|---|
| K82a | `Gradient` payload, `tests/glass_cmd.rs`' `GradientPayload` mirror | ≤ **56 B** (`Cmd` stays 60) | T82.1, T82.2 | `corner` leaves the command (payload 52), rounded gradients deferred |
| K82b | `firmware_interactive` headroom under 81,920 B | ≥ **1,024 B** after every task; no binary over 81,920 B | every task, `just firmware-delta <parent>` and `scripts/check_text.sh` | to the owner with the symbol diff; if moved, `path`/`fills` provided under `cfg(feature = "alloc")` |
| K82b / K4 | one task's `firmware_interactive` `.text` Δ | ≤ **1,000 B** (T82.2: **400 B**; T82.1: printed, expected ≈ 0) | every task | the same |
| K82c (K3) | frame idle and paint rows, `--bin textarea` `keystroke + layout`, `--bin cascade` `cascade_deep`, eight interleaved runs, no fill and no path | inside each row's spread | T82.3, T82.5 | `ops::fill` out of line (`#[cold]`); then the fill honoured only by derive containers and `Card` |
| K82d | path-mask texels on the showcase's first GPU frame with PaintBox and VectorText, share of the glyph atlas's start size | ≤ **25%** | T82.6 | paths get their own R8 atlas and binding |
| K82e | `benches --bin vocabulary`, a 24-glyph `VectorText` at 32 px, uncached, `just bench-lock` | ≤ **250 µs** a frame | T82.8 | the software canvas gets the GPU's `MaskKey` cache |
| K2 (`vector-text`) | new crates; stripped showcase's size delta | **0** crates; against **3 MB** | T82.7 | the feature stays default-off, size recorded |
| §2.1 | `Style` / `NodeState` | exactly the measured size, expected **84 / 84** B (16.4 KB per 100 nodes against 24 KB) | T82.3 | K1 |

## T82.0 — the pins, at `b873ab0` (2026-10-04)

Gates that compile today and hold today's behaviour green, so that the
task that changes it turns them red by name.

| what | gate | at `b873ab0` | injection (red) | flipped by |
|---|---|---|---|---|
| text over a gradient | `lumen-static-shell/tests/contrast_ground.rs` | setup 1 text run over 1 gradient; measured **0**, unmeasured **1** (`"gradient"`), clipped 0 | `ground_at` reads a gradient as its middle sample: measured 1 at **5.46:1**, fails by name | T82.2 |
| `Gradient` payload | `lumen-static/tests/glass_cmd.rs` `the_gradient_payload_fits` | **56 B** of 56 (`Stops` 36 B), `Cmd` 60 B | `Stops` gains a 4-byte field: **60 B** (`Stops` 40, `Cmd` 64), fails by name | kept at 56 by T82.1, T82.2 |
| `Deferring`'s stream | `lumen-static/tests/overlay.rs` `deferring_forwards_every_command` | **6** commands in, 6 out (unchanged since M80/T80.1) | `Deferring::gradient` drops its command: 6 in, **5** out, first difference at index 2 | T82.5 (6 → 7) |
| firmware | `just firmware-delta b873ab0` | Δ **0** on all four; `firmware_interactive` **63,084 B**, headroom **18,836 B** to 81,920 | (T80.0's) | — |
