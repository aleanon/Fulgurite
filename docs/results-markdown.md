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

## T87.2 — `MarkdownTheme` and blockquote; K87c fired

**K87c — parts cost: fired.** The two zero-size part nodes
(`markdown-link`, `markdown-quote`, seeded with the `Accent` and `Muted`
roles) were built as planned, with the journal's all-stand-ins signing
rule and their gates green, and the row measured eight runs a side,
alternating, under `just bench-lock` (quiet: load 0.40 → 0.82, bound 8):

| build | `NODE_COUNT` | `cascade_deep (markdown x16)`, eight medians | spread |
|---|---:|---|---|
| T87.1 (`2f798f3`) | 17 | 277.57, 279.94, 278.80, 278.14, 293.52, 277.44, 276.89, 274.12 ns | **274.12–293.52 ns** |
| T87.2 with the parts | 49 | 943.86, 957.18, 945.82, 967.78, 962.20, 1004.60, 943.23, 995.16 ns | **943.23–1004.60 ns** |

Every run with the parts is above the parent's whole spread, by **3.4x**
(median of medians 278.5 → 959.7 ns): a cascade costs per node, and the
parts triple the nodes of a `Markdown`. The arm-sensitivity injection
(four extra parts per `Markdown`) was not needed — two already leave the
spread by more than its width thirty times over. The rule's response is
taken: **the parts are withdrawn** (a `Markdown` is one node, its table
and the editor's are unchanged, and the journal needs no new rule because
a `Markdown` is still a leaf) and the colours become fields on the theme.
What the parts would have cost in state was 2 × (`Style` 84 + `NodeState`
88) = **344 B per `Markdown`** (the plan's 328 B was at 80 + 84).

**One deviation from the fallback's wording**, recorded in the plan's
"What building found": the theme's `quote_color` and `link_color` are
`Option<Color>`. `Some` is the fixed colour the rule names, which does not
follow the palette. `None`, the default, derives the colour from the
node's own (`MarkdownTheme::quote_ink`, `link_ink`), which the cascade
resolves from the palette's `Text` role — because **no fixed colour reads
at AA on both of the showcase's surfaces** (white and `#1c212b`: AA needs
relative luminance ≤ 0.183 against the first and ≥ 0.245 against the
second), so a fixed default fails M57's D4 gate in one palette. Checked,
not argued: with `quote_color: Some((115, 125, 141))` as the default — the
grey closest to balanced between the two — the showcase's
`every_demo_text_reads_in_both_palettes` reports **24 runs under their
bound** (the quote's twelve words in each palette, 4.16:1 on the light
surface); with the derived default, **0 of 632**.

| quantity | value | gate |
|---|---:|---|
| `size_of::<MarkdownTheme>()` | **68 B** (seven `(f32, f32)` + two `Option<Color>`) | printed by `a_theme_scales_its_heading` |
| `size_of::<Markdown>()` | 120 → **192 B** | the same |
| `size_of::<Span>()` | 20 → **24 B** (`kind: u8`) | the same |
| heap per word | 40 → **44 B** | — |
| default display list, `DOC` and `EVERY` | folds unchanged: `0x544b680dbb850c4e`, `0xf343b0e91944032c` | gate 5 |
| a theme's `h1 = (2.0, 700)`: heading line over body line | **2.000** (40.60 / 20.30 px; bound 2.0 ± 0.0725) | `a_theme_scales_its_heading`; `heading()` ignoring the theme: **1.500** |
| a quote wrapping to 5 lines at 120 px: bars, words at the 14 px indent | **5**, **5** (bound = lines) | `a_quote_is_indented_and_barred_on_every_line`; the bar skipped: 0 |

**The quote's colour, and M57's D4 record** (`crates/lumen-static-shell/
tests/markdown_quote.rs`, a `Host` under each built-in palette): the
default quote is the body's colour at `QUOTE_ALPHA` (0xb3), so LIGHT
`(15, 23, 42, 179)` and DARK `(230, 234, 242, 179)`, which reads at
**6.69:1** and **8.04:1** on the page (bound AA 4.5). A fixed
`Some((150, 60, 20))` stays that colour under both. Painting the quote in
the node's own `fg` turns both gates red (`(15, 23, 42, 255)`, alpha 255).

**Live** (`scripts/widget_drive.sh Markdown,RichTextEditor`, all six
legs green: `Everything`, `Boundaries<256>`, the oracle, scale 2 and both
`+shaping`): the demo's least contrasting run is the quote's first word
in both of the showcase's palettes — light `'A'` at **6.1:1** (fg
`[78, 85, 101]`), dark `'A'` at **6.5:1** (fg `[173, 177, 187]`), bound a
quote word, ≥ 4.5 and the two colours different. With the quote painted
in the node's own colour the action fails on every leg: the minimum is
the heading `'Markdown'` at 14:1 in each palette. `RichTextEditor`'s two
actions print as before.

**The scale-2 leg's contrast check had been measuring nothing**, found by
that action: `ui.contrast` with a selector filtered the display list —
physical pixels, painted through `Scaled` — by the selector's logical
bounds, so at scale 2 a widget's box held almost none of its own text and
the drive's after-every-action check printed "no text in the widget's
box" and passed. The session scales the rect now
(`crates/lumen-static-shell/tests/contrast_scale.rs`: one run, `"second"`,
at scale 1 and 2; with the rect left logical, scale 2 measures **0**).

## T87.3 — links

`[text](target)` parses for every `Markdown`: the markup and target are
not drawn, the words are drawn in `MarkdownTheme::link_ink` over a 1 px
underline (2 px for the active link). `LinkedMarkdown<S>` (`Markdown<S,
true>`, built by `Markdown::linked`) makes them activate: `hit` over a
link answers `Inner { row: n }`, the pass hands `on_link` `ctx.row =
Some(n)`, and `link_target(src, n)` reads the target from the source.

| gate | measured | injection (red) |
|---|---|---|
| gate 3 `link_markup_is_not_drawn`, flipped | drawn `see\|the\|docs\|now`, links `[-, 0, 0, -]`, link 0 `https://a.example` | `scan_links` finds nothing: `see\|[the\|docs](https://a.example)\|now` |
| click at link 1's word centre (187.5, 12.2) | `handled` **true**, `last_link` **Some(1)**, `link_target(DOC, 1)` `https://lumen.example/changes` | `hit` with `inner: None`: handled false, `None`; `hit` with the previous index: `Some(0)`, `.../guide` |
| the same click 4 px past the link | `handled` **false**, state unchanged | — (the accept-and-refuse pair) |
| press on link 0, release on link 1 | handler ran **0** times; a press and release on link 1 runs it once | `hit` reporting row 0 for every link: **1** |
| `Host::cursor` over link 0 / 4 px past it | **Pointer** / **Default** | `cursor_at` answering `None`: Default over the link |
| the link's colour, LIGHT / DARK | `(32, 57, 132)` / `(139, 163, 232)` = `link_ink(Text)`; body the palette's `Text` | link painted in the node's `fg`: `(15, 23, 42)` |
| Tab, Down, Enter | focus on the link node, active `Some(0)`, `last_link` **Some(0)**, ran once | `active_row` answering `None`: ran 0 |
| `a_linked_markdown_publishes_its_links` | 1 `Link` child, label `docs`, id `instance_id(id, 0, 0)`, `Click`, bounds inside the paragraph; a plain `Markdown` 0 | no link children pushed: 0 |
| `size_of::<Span>()` | **24 B** (bound 24: `link: u16` in `kind`'s padding) | — |

Sizes: `RichText` 120 → **136 B** (the kept layout's padding origin, font
size and line ratio, which `link_at` needs and `cursor_at` is not handed),
`Markdown` 192 → **240 B** (the links `Vec`, the active link, `on_link`).

**K87b — not reached.** With links reported as `Hit::inner` on a node
that is not virtualized, `just test` is green and so is the live drive.

**Live** (`scripts/widget_drive.sh LinkedMarkdown,Markdown,RichTextEditor`,
all six legs green): a click on `#d_mdlinks_a > ~=changelog` moves the
readout `''` → `link: https://lumen.example/changes`, its damage the
readout's own box at **1.00x**, the window's pixels changed; a press on
the plain first word leaves it there (the isolate). M57's D3 pass: Tab
reaches `#d_mdlinks_a` in one press, and Down, Enter moves the readout to
`link: https://lumen.example/guide`. The a11y sweep finds no silent leaf
(each link is a `Link` with its text). With no link children pushed, every
leg fails on `` `~=changelog` matched nothing ``.
