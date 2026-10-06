# Markdown: a store that grows, a parse that walks once

M87's results (`docs/plan-markdown.md`). Every figure an M87 gate or
arm prints is recorded here once and cited from the rustdoc, the plan
and the commits. The plan-wide budgets and kill rules K1–K4 are
`docs/plan-iced-roadmap.md` §2 and §4; the firmware's four-binary table
is `docs/results-embedded.md`'s.

## Thresholds, written before anything is built

M87's kill rules, copied from the plan so each later row has its
threshold beside it. A row's verdict is filled in by the task that
measures it, never edited here.

| rule | quantity | threshold | measured by | if it fires |
|---|---|---|---|---|
| K87 | `--bin markdown -- 64`: median of eight runs of parse + measure + place for a one-character append to 64 KB, under `just bench-lock` | **1.5 ms** (half of T8b.3's 3 ms input → presented p95, `docs/results-frame-cost.md`) | T87.6 | over: T87.6 builds the dominant split quantity only; under: decided against, with the number |
| K87a | `alloc_gate.rs`'s Markdown arm, allocations and frees per hover frame | **0** | T87.1 | the `Vec` stores are withdrawn and `MAX_SPANS` returns, sized to the 64 KB fixture |
| K87b | any existing test or drive, with a link reported as `Hit::inner` | none turns red | T87.3 | links use `Menu`'s take-once shape |
| K87c | `--bin cascade`'s `cascade_deep (markdown x16)` with the two part nodes, eight runs a side | inside T87.1's spread | T87.2 | link and quote colours become fixed `Color`s on the theme |
| K4 (M87) | each task's `.text` Δ on all four firmware binaries, `just firmware-delta <parent> firmware=0 firmware_interactive=0 firmware_bind=0 firmware_cascade=0` | **0 B** | every task | the task stops and attributes the delta by a symbol diff |

## T87.0 — the gates that compile today, at `262643a` (2026-10-06)

`crates/lumen-static-widgets/tests/markdown_m69.rs`, debug `cargo test`.

| gate | pinned at T87.0 | perturbation (red) |
|---|---|---|
| 1 `every_word_is_kept` | **192** spans of 1,000 words | `MAX_SPANS` 192 → 256: 256 |
| 2 `set_str_is_linear` | ratio **33.1** (6.4 KB 1.475 ms, 64 KB 48.850 ms; 51.2 in release), "> 25" | the carried offset: ratio 6.9 |
| 3 `link_markup_is_not_drawn` | `see\|[the\|docs](https://a.example)\|now` | a word holding `](` skipped |
| 4 `image_markup_is_not_drawn` | `![a\|chart](chart)` | the same |
| 5 `default_theme_is_todays` | `DOC` 21 text runs, fold `0x544b680dbb850c4e`; `EVERY` 10 runs, fold `0xf343b0e91944032c` | `## ` scale 1.25 → 1.3: `EVERY` red (`DOC` has no `## `) |

## T87.1 — the store grows, `set_str` walks once

| quantity | before | after | gate |
|---|---:|---:|---|
| spans kept of 1,000 words | 192 | **1,000** | gate 1, flipped to equal |
| `set_str` 64 KB / 6.4 KB, minimum of five | 33.1 | **10.0** (0.498 → 4.990 ms) | gate 2, flipped to ≤ 25 |
| `size_of::<RichText>()` (and `Markdown`) | 7,736 B | **120 B** | printed by gate 1 |
| heap per word | 0 (inline) | **40 B** (`Span` 20 + placed 12 + widths 8) | — |
| allocations per hover frame, `alloc_gate.rs` Markdown arm (14 spans) | — | **0.00** allocs, **0.00** frees (bound 0; K87a not reached) | `placed` rebuilt with `Vec::new()` in `layout`: 1.00 / 1.00 |

The inline size falls from three 192-word arrays to three `Vec` headers
and a `String`; the heap is grown when a parse needs more and keeps its
capacity across `set_str`, so an editor typing into a document allocates
only when it passes its high-water mark.

**`--bin markdown`, first row** (`cargo run --release -p benches --bin
markdown`, one run, not under `just bench-lock`; K87's verdict is
T87.6's eight-run measurement, not this):

| size | spans = words | append + `set_str` + layout, median (min) | parse | measure | place |
|---|---:|---:|---:|---:|---:|
| 1 KB | 160 = 160 | 0.051 ms (0.050) | 0.003 | 0.044 | 0.003 |
| 16 KB | 2,528 = 2,528 | 0.778 ms (0.776) | 0.045 | 0.685 | 0.047 |
| 64 KB | 10,085 = 10,085 | **3.102 ms** (3.086) | 0.178 | **2.734** | 0.189 |

Measure is 88% of the 64 KB append: the memo keys the whole text, so one
appended character re-measures every word through `FontStack`. This
single run is above K87's 1.5 ms; T87.6 decides it on eight runs.

**K87c's arm**: `--bin cascade` gains `cascade_deep (markdown x16)`, a
derived column of sixteen `Markdown`s (`NODE_COUNT` **17**, asserted
before timing; red at an expected 49), one `Kind` rule per kind the tree
will hold. One run at T87.1: median **293 ns** (288–298). T87.2 measures
it eight runs a side.

Firmware: `rich.rs` is behind `alloc`, which no leaf firmware binary
enables; Δ 0 on all four (the commit body quotes `just firmware-delta`).
The ungated **alloc-tier record** (`scripts/check_text.sh`, `embedded`
with `alloc-tier`) links no `rich` symbol either, yet moved **−128 B**
(`firmware` 44,052 → 43,924; `firmware_cascade` 64,104 → 63,980): a
symbol diff against the parent puts all of it in `fit::cut`, which now
calls `str::trim_end_matches` through a closure instead of a 198 B
out-of-line copy — the widgets crate's codegen units were re-partitioned
by `rich.rs`'s edit, not anything `rich.rs` contributes.
