# Layout and input: `Flexible`, `pin`, `pop`, culling, capture

M85's results (`docs/plan-layout-input.md`). Every figure an M85 gate
prints is recorded here once and cited from the rustdoc, the plan and
the commits. The plan-wide budgets and kill rules K1–K4 are
`docs/plan-iced-roadmap.md` §2 and §4; the firmware's four-binary table
is `docs/results-embedded.md`'s.

## Thresholds, written before anything is built (T85.0, 2026-10-06)

M85's kill rules and the bounds its tasks state, copied from the plan so
each later row has its threshold beside it. A row's verdict is filled in
by the task that measures it, never edited here.

| rule | quantity | threshold | measured by | if it fires |
|---|---|---|---|---|
| K85a | layout calls per child in `crates/lumen-static/tests/flexible.rs`, every case; the frame arm's `ALL rows: layout` row (`R=1 benches-competitive/run.sh -p lumen-static-frame`, `just bench-lock`, eight runs a side) with `Flexible` unused | **1** per child; inside the parent's eight-run spread | T85.0 (the count), T85.1 | `Flexible` is not shipped; the count and the row are recorded here |
| K85b | `benches --bin scroll_cull`, the culled median against the uncull arm's eight-run spread | below the spread's minimum | T85.8 | T85.8 is not shipped; the number is recorded |
| K85c | `benches --bin retain` and the frame arm's `idle (floor)` and `+render (damage)` rows, eight runs a side | inside each row's spread | T85.7 | T85.7 is reverted, and culling is decided against on X1's ground |
| K85d | M85's cumulative `firmware_interactive` `.text`, `just firmware-delta <T85.0 sha> firmware_interactive=1024` at each task | ≤ **1,024 B** (5.1% of the 19,998 B headroom under 80 KB at the M56 merge) | T85.1–T85.4, T85.7, T85.8 | the task stops; the owner re-budgets or defers `Flexible`'s derive branch |
| K85e | real capture-phase cases in T85.5's survey (shipped code or a `docs/widget-audit.md` row whose workaround exists *because* events only bubble) | ≥ **2** opens its own plan | T85.5 | fewer than two: decided against, the survey is the evidence |
| K4 | one task's `firmware_interactive` `.text` Δ, `just firmware-delta <parent>` | ≤ **1,000 B** (T85.2 and T85.4: Δ **0** on all four); every binary ≤ 81,920 B | every task | the task stops and attributes the delta by a symbol diff |
| K1 | `size_of::<Length>()`; `Style` / `NodeState` | **8 B**; equal to the parent commit's (84 / 84 B at `2fcb000`, 16.4 KB per 100 nodes against 24 KB) | T85.0, T85.1 | the variant is packed or the owner re-budgets |

## T85.0 — the gates that compile today, at `2fcb000` (2026-10-06)

Gates that compile today and hold today's behaviour green; each names
the injection it was watched red under.

| gate | prints today | injection (red) | used by |
|---|---|---|---|
| `crates/lumen-static/tests/flexible.rs`: a derived 300 px row `[CountingLeaf(Shrink, 80 px), CountingLeaf(FILL)]`, laid out once | row **300** px; layouts **1, 1**; widths **80 / 220** | scan 2's `d.layout` emitted twice in the derive: the fill reads **2** layouts, fails by name | T85.1 adds the `Flexible` cases |
| the same file, accept: three `Shrink` children | layouts **1, 1, 1** (widths 40 / 50 / 60) — unchanged under the injection, which never reaches a row with no fill | — | — |
| `crates/lumen-static/tests/cursor.rs` `a_length_is_eight_bytes` | `Length` **8 B**, `Style` 84 B | `FillPortion(u64)`: `Length` **16 B**, `Style` 104 B, fails by name | T85.1 |
| `crates/lumen-static-shell/tests/pop_visibility.rs`, setup half, under `Everything` and `Boundaries<256>` | viewport **100** px in a **300** px window; 15 wheel steps of 20 px, each handled, offset 20 → 300; the target enters the window at step **3** (y 280..300) and the viewport at step **13** (y 80..100); identical under both strategies | a viewport as tall as the window: first in the window and in the viewport both step 3, "inside the window and outside the viewport" **none**, fails by name | T85.3's arrival-step gate |
| firmware, `just firmware-delta 2fcb000 firmware=0 firmware_interactive=0 firmware_bind=0 firmware_cascade=0` | Δ **0** on all four | — | — |

**M85's firmware baseline** (`just firmware`, `scripts/check_text.sh`,
at this commit; tests and docs only, so equal to `2fcb000`). K85d's
cumulative check compares every later M85 task with this commit.

| binary | `.text` | of 81,920 | headroom | `.rodata` |
|---|---:|---:|---:|---:|
| `firmware` | 30,606 | 37.4% | 51,314 | 13,344 |
| `firmware_interactive` | **63,480** | **77.5%** | **18,440** | 21,886 |
| `firmware_bind` | 34,014 | 41.5% | 47,906 | 13,948 |
| `firmware_cascade` | 32,694 | 39.9% | 49,226 | 14,132 |
