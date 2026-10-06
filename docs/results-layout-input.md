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

## T85.1 — `Length::Flexible` (2026-10-06)

The variant, its two `.lss` spellings, and the derive's scan 2 as one
body run in two rounds (`ops::Round`; why not two bodies is
`docs/plan-layout-input.md`, "What building found").

| gate | prints | injection (red) |
|---|---|---|
| `flexible.rs`, `[Flexible(1) 80 px, FILL]` in 300 px | row **300**; layouts **1, 1**; **80 / 220** | Flexible treated as `FillPortion` in scan 2 (counted with the fills, accepted by the fills' round): **150 / 150**, fails by name |
| `flexible.rs`, `[Flexible(1) 400 px, FILL]` | layouts **1, 1**; **150 / 150** | the fills' round run first: the fill reads **300** (80 / 300 in the case above), fails by name |
| `flexible.rs`, `[Flexible 80, Flexible 400]` — the stated limit | layouts **1, 1**; **80 / 150** | under the first injection: **150 / 150**, fails by name |
| layout count, every case above (K85a) | **1** for every child | a `Flexible` child laid out a second time under its measured size: **2** in all three cases, fails by name |
| `flexible.rs` `flexible_moves_no_struct` | `Length` **8 B**; `Style` **84**, `NodeState` **84** (parent 84 / 84); **16.4 KB** per 100 nodes (parent 16.4 KB, bound 24 KB) | `Flexible(u64)`: `Length` **16 B**, `Style` 104, 18.4 KB, fails by name |
| `lumen-static-lss` `every_property_lowers_to_its_setter` | `width: flexible` → `Flexible(1)`, `height: flexible(2)` → `Flexible(2)` | the keyword lowered as `FillPortion(1)`: fails by name. Deleting the shell's or the macro's `Flexible` arm is a compile error (both matches are exhaustive) |
| live, `scripts/widget_drive.sh Layout` (Everything, Boundaries, Boundaries+oracle, scale 2, both `+shaping` legs) | row **300.0** px on **1** line; flexible **80.0** px (bound min(80, 150.0) = 80.0), fill **220.0** px (bound 220.0); D5 parity 1 of 1 byte-identical; oracle 596 frames, none diverged | the first injection: flexible **150.0**, fill 150.0, FAIL by name on every leg |
| firmware, `just firmware-delta a21135b` (K4) and `just firmware-delta a21135b firmware_interactive=1024` (K85d) | `firmware` **+484**, `firmware_interactive` **+436**, `firmware_bind` **+436**, `firmware_cascade` **+484**; `.rodata` +0; headroom **18,004 B** | — |

**The spellings that did not fit K4**, each `just firmware-delta a21135b
print` on `firmware_interactive`: 2a and 2b emitted per field **+1,616**;
a two-round `for` over one body **+1,628** (unrolled), **+2,040** with the
array `black_box`ed; the per-field 2a borrowing `Style` **+1,580**; a loop
over the child roots through `node_mut` **+1,396** here and **+20,026** on
the paint-only `firmware`, which never linked the `dyn Node` machinery;
the one body with its round's choices as data **+756**; and with them out
of line in `Round::limits`, what shipped, **+436**.

**K85a** — the frame arm, `a21135b` against this tree, eight interleaved
invocations a side under one `just bench-lock` (quiet; load1 2.22 before,
2.18 after, bound 8), `Flexible` unused:

| row | base median (spread) | T85.1 median | verdict |
|---|---:|---:|---|
| `ALL rows: layout` (K85a's row) | 1,159.70 µs (1,105.2–1,169.3) | 1,161.30 | inside |
| `idle (floor)` | 59.40 µs (53.2–62.5) | 59.05 | inside |
| `ALL rows: +paint` | 1,283.60 µs (1,220.2–1,291.8) | 1,285.15 | inside |
| `layout` | 20.20 µs (20.1–20.5) | 20.25 | inside |
| `+paint` | 40.95 µs (40.6–41.3) | 41.25 | inside |
| `+render (damage)` | 77.70 µs (77.3–77.9) | 77.10 | below the spread (faster) |

K85a holds: one layout per child in every case, and its row inside its
spread. `+render (damage)` reads 0.6 µs under its base's minimum; the
change touches layout only (no paint or render code moved), so it is
recorded and not attributed. K1 holds (`Length` 8 B, `Style` and
`NodeState` unchanged). K4 and K85d hold (436 of 1,000; 436 of M85's
cumulative 1,024).
