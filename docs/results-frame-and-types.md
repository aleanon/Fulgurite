# Results: M26 — baselines, thresholds, and what each probe decided

*M26, `docs/plan-frame-and-types.md`. Owns every figure that plan cites.
Machine: i9-13900KF, governor `powersave`. Target for flash and RAM rows:
`thumbv7em-none-eabihf`.*

## The thresholds — written before anything is changed

Recorded at T26.0, 2026-09-17, so a task can be told no. Every frame row
is **eight** runs, each classified into the fast or the slow mode before
any spread is computed, with the count in each mode and the fast mode's
internal spread printed beside the delta.

| quantity | how | threshold | kill if |
|---|---|---|---|
| frame, table tree | `benches --bin crossover` | no regression beyond the arm's four-run spread | the delta exceeds the spread |
| frame, toggles | `benches --bin appearance` | same | same |
| frame, still and one-subtree | `benches --bin invalidation`, including the new place and dispatch rows | same | same |
| frame, arena rows | `benches --bin repeater`, `just stages 10000 150 20` | same | same |
| retention and paint | `benches --bin retain`, `benches --bin paint_sink` | same | same |
| cascade and lint | `benches --bin cascade`, including the new lint row | reported; a task that claims a cascade win states it is not a frame win | a claimed win inside the spread |
| raster frame | `just profile` (tally), plus the new popup-open frame | same | same |
| glass | `benches --bin glass-cost` | same | same |
| embedded flash | `scripts/check_text.sh` | ≤ +300 B over the M26 baseline | it exceeds that |
| embedded RAM | the const-asserted sizes in `fixtures/embedded` | unchanged unless the task records a new number | an unrecorded change |
| rustc time | the millisecond figure `just const-budget` prints, four builds, with its own spread | no regression beyond that spread | the delta exceeds it |
| allocation | `crates/lumen-static-shell/tests/alloc_gate.rs` | stays at zero, and covers the raster path from T26.13 | any steady-state allocation |
| backend parity | `just gpu-parity` | unchanged | any divergence |

**Why a mode rule and not a fixed bound.** M25/T25.1 wrote a 335 ns
threshold; M25/T25.12 then found the *baseline* running 1.7 µs above its
own median on some runs — and not as an outlier, as a second mode every
build exhibited. Counted into one spread it would have made every row
pass, which is a bound computed from the wrong quantity. So each arm
records both modes: how many runs landed in each, and the spread inside
the fast one. A delta is judged against the fast mode, and a task whose
runs land in one mode only says so.

**The rustc-time row is a printed figure, not the recipe's exit code.**
`just const-budget` fails only past 5 000 ms against a fixture that
builds in a fraction of a second, so the recipe is a ceiling that nothing
Phase C does could reach. The number it prints is the measurement; the
ceiling stays where it is.

## T26.0 — the baselines

*Taken 2026-09-17, eight runs per arm, i9-13900KF, governor `powersave`.
Each row gives the **fast cluster's** range and how many of the eight
runs landed in it; the slow mode is reported separately rather than
averaged in.*

**The mode rule earned itself on the first sweep.** Run 7 came back slow
on three of the five `--bin invalidation` rows — `Everything` 8.50 µs
against 5.27–5.63, `place` 2.39 against 1.01–1.08, `Boundaries, one
subtree marked` 0.89 against 0.52–0.54 — while `dispatch` in that *same
run* stayed at 1.12. A single spread over all eight would have been 3.2 µs
wide on a 5.3 µs row, and no Phase B change could ever have exceeded it.

### Frame arms

| arm | fast cluster | runs in it | slow mode |
|---|---:|---:|---|
| `--bin invalidation` Everything (the oracle) | 5.27–5.63 µs | 7/8 | 8.50 µs |
| `--bin invalidation` Boundaries, one subtree marked | 0.52–0.54 µs | 7/8 | 0.89 µs |
| `--bin invalidation` Boundaries, still frame | 0.01 µs | 7/8 | 0.02 µs |
| **`--bin invalidation` place** *(new)* | **1.01–1.08 µs** | 7/8 | 2.39 µs |
| **`--bin invalidation` dispatch, one PointerMove** *(new)* | **1.11–1.16 µs** (1.23 ns/node over 901) | 8/8 | — |
| `--bin cascade` cascade_deep (whole table) | 15,309–15,726 ns | 8/8 | — |
| `--bin cascade` cascade_subtree (90 of 901) | 1,459–1,531 ns | 8/8 | — |
| `--bin cascade` cascade_deep (showcase-shaped, 27 rules) | 66,184–66,716 ns | 8/8 | — |
| **`--bin cascade` lint_sheet_deep (same sheet)** *(new)* | **182,087–184,725 ns** | 8/8 | — |
| **`--bin crossover` either45/static** *(new)* | **210.9–214.4 ns** | 8/8 | — |
| `--bin crossover` 1k/static | 8,907–9,053 ns | 8/8 | — |
| `--bin bindings` short, memo warm (hit) | 5.35–5.45 ns | 8/8 | — |
| `--bin bindings` short, memo cold (miss) | 413.1–434.0 ns | 8/8 | — |
| **`--bin bindings` Badge, unchanged (no memo)** *(new)* | **403.7–422.3 ns** | 8/8 | — |
| **`--bin bindings` Chip, unchanged (no memo)** *(new)* | **406.7–424.3 ns** | 8/8 | — |

**The lint is the largest number here** — 183 µs against the 66 µs
cascade over the same sheet, on the same event. It is not on the frame
path, and T26.17 #10 is where it is decided.

### The popup frame — `just profile-showcase` *(new)*

| quantity | today |
|---|---|
| settled frame, damaged | **0 px** |
| popup-open frame, damaged | **691,200 px of 691,200 — 100% of the window** |
| commands across that frame | 9 → 30 |

That is T26.11's defect, stated as a measurement: a command-count change
answers `Damage::All`, so opening a 300×560 menu repaints a 1152×600
window.

### Budgets

| budget | measured | instrument |
|---|---|---|
| embedded `.text` | **26,364 B** of 65,536 (40.2%), 3 Toggle symbols linked | `scripts/check_text.sh` |
| `NodeState` / `Style` / `Ui` on the target | **76 / 72 / 1,136 B**, const-asserted | `fixtures/embedded` |
| rustc, 3,711 const nodes | **186–192 ms** (3 of 4 builds; the first was 752 ms, cold) | `just const-budget` |
| steady-state allocation | **zero**, 6 cases | `alloc_gate.rs` |

**The rustc row is why the "+10%" threshold was struck** (§ the plan's
review): the recipe fails only past 5,000 ms, which is 26× the measured
value, while the build-to-build spread is ~3%. A Phase C task is judged
against 186–192 ms, not against the ceiling.

### The arms T26.0 adds, and the injection each was shown to move under

*A new arm that does not move under its own injection is a finding about
the arm — `AGENT.md`'s rule — and is fixed before the baseline is taken.
Two of the six were found wanting this way and rebuilt.*

| arm | injection | moved by |
|---|---|---|
| `place`, `--bin invalidation` | the generated `place` returns before recursing into children | **1.01 → 0.01 µs** |
| dispatch, `--bin invalidation` | the hover and focus publishes write one node instead of sweeping every one | **1.11 → 0.03 µs** (1.23 → 0.03 ns/node) |
| `lint_sheet_deep`, `--bin cascade` | the arena walk visits nothing | **183,460 → 42,816 ns** |
| `either45`, `--bin crossover` | the hidden branch is not zeroed | **210.9 → 203.7 ns** |
| `Badge`/`Chip`, `--bin bindings` | `Badge::layout` returns a constant instead of shaping | **Badge 412.6 → 4.4 ns, Chip unmoved at 410.9** — the arm moves per widget |
| showcase popup, `just profile-showcase` | its own control: the settled frame's damaged area printed beside the popup frame's | see below |

**Two arms were wrong before they were right**, and both failures are the
shapes `AGENT.md` names:

- The `Badge`/`Chip` rows first ran against the stand-in measurer and
  printed **4.2 ns** — indistinguishable from a memo hit, because under
  `BlockMetrics` a measurement is `char_count × advance`. The arm could
  not exhibit the cost it exists to price. Rebuilt against a real face,
  where the same widgets cost **412 ns**, next to a memo hit's 5.4 and a
  memo miss's 420 — which is the claim T26.8 makes, now measurable.
- The showcase popup arm reported the **settled** frame as 100% damaged
  too, so it could not tell a still frame from a popup-open one. The
  cause was measuring the banked seed frame; the arm now banks first and
  measures a third, genuinely still frame.

**What the `Either` injection also settles.** Its move is the smallest
here — about **7 ns of a 211 ns frame, ~3%** — and that is not a weak
gate but a bound: zeroing 22 hidden nodes is 3% of this shape, so
Phase D's "cache the hide" probe cannot win more than that, and should be
judged against it before anything is built.

## Phase A — the defects

*One row per defect: what the failing test printed before the fix, and
what it prints after. No frame numbers are claimed for these; they are
correctness.*

### T26.1 — the enum derive zeroed the active variant

| | |
|---|---|
| before the fix | the active variant's container measured **0×0** where its sheet gives it 40×20, on `Boundaries<16>` |
| after | **40×20**, and the same session under `Everything` unchanged throughout |
| second gate | the invalidation oracle, whose fixture gained an enum and a variant switch: *"step 2: the display lists differ"* under the injection |
| sites inventoried | **10 hide call sites, 1 defective.** `Option` hides only on `None`, `Either` only the side it is not showing, `Popup` only when closed; the six `!visible` sites are stylesheet-driven and marked by `set_stylesheet`/`restyle_subtree`. Only the enum derive zeroed a subtree it then laid out |

**What the fix costs, by shape.** `examples/showcase` links an
18-variant enum, so it is the binary that prices this; the embedded
fixture holds no enum and does not move.

| shape | showcase `.text` | over the pre-fix baseline |
|---|---:|---:|
| before T26.1 | 2,802,022 B | — |
| two hides **per match arm** (as first landed) | 2,815,654 B | **+13,632** |
| **the active range computed once, two hides around it** | 2,802,566 B | **+544** |

**25× for identical behaviour**, and nothing but a size measurement would
have found it: both gates pass under either shape, and neither the frame
arms nor the embedded budget moves. The per-arm version inlines
`hide_subtree` twice per variant with constant counts, so an enum pays
code size proportional to its variant count for work that is the same two
calls every time.

**Why no test had seen it.** The stale zero is returned by the *variant's
own* relayout boundary — `can_skip() && is_clean(..) && constraints ==
limits` → `return st[i].size` — so it needs the active variant to be a
derived container, the enum to be reached, and its subtree to be clean.
An enum of leaves cannot exhibit it, and neither can any tree under
`Everything`, where nothing is ever clean.

### T26.2 — the agent reported a hidden node's kind as its container's

| | |
|---|---|
| before the fix | `ui.resolve` on `#d1`, a `Label` inside the inactive variant, answered **`Stage`** — the enum container's kind |
| after | **`label`**, and `#d3` **`fixed`**; the visible `#head` unchanged at `label` |
| the exemption's own gate | dropping the kind from `live_nodes`' identity fired *"setup: a visible node was reported hidden"* — a stand-in and its container share an address, so the kind is what separates them |
| sites moved | 6 reporting paths read `TreeRef::seeds`; `live_nodes` alone still asks the instance |

**Residue.** `role` is still read from the instance, so the same response
that now reports `kind: "fixed"` for a hidden node reports
`role: "GenericContainer"` beside it. `Seed` carries no role, so nothing
static can be read; deferred rather than papered over.

### T26.18 — the lint leaves a non-agent build entirely

| quantity | before | after |
|---|---:|---:|
| `examples/minimal` `.text` (no agent) | 2,618,470 B | **2,615,910 B** (−2,560) |
| `lint_sheet_deep` symbols linked into it | 1 | **0** |
| `examples/counter` (feature on) | 1 | 1 |
| the 183 µs per `set_stylesheet` | paid by every build | paid only where an agent can read it |

**The measurement was contaminated once and had to be retaken.** Building
`minimal` and `counter` in one `cargo` invocation unifies their features,
so the "no agent" binary had the lint linked after all and the symbol
count read 1. Built alone it reads 0. The workspace is `resolver = "2"`,
so the dev-dependencies the example tests use do not leak the feature
into anything else's normal build.

### T26.4 — an `Assembled` slab seeded its hidden nodes from a live probe

| | |
|---|---|
| before the fix | a hidden checkbox in a slab carried **`FillPortion(1)`** — the enum container's width, via the stand-in `Node::node(j)` answers with |
| after | **`Fixed(24.0)`** and the checkbox's own colours, at both instances |
| gate | *injection:* restore the probe — fires, naming the instance |
| why it was latent | no `Assembled` in the tree contains an enum; the test supplies the first one |

### T26.5 — `Boundaries<N>` wrapped its length, and kept a second `is_clean`

| | |
|---|---|
| before the fix | 300 marks reported as **44** — 300 truncated to a `u8`. A set reporting zero ranges with `all == false` *is* a still frame, so at 256 marks every subtree answered clean and the frame skipped the whole tree |
| after | **300**, with the marked ranges dirty and the gaps clean |
| gates | *injection a:* restore the `u8` → "the length wrapped: 300 marks reported as 44". *injection b:* give `Boundaries` its own divergent `is_clean` → the agreement test fires, with three other range tests |
| shape of the fix | the counter widened rather than `N` capped: one value per `Host`, so the width is free, and `Boundaries<300>` now works instead of being refused |

### T26.3 — the text memo's key omitted font size and weight

| | |
|---|---|
| before the fix | a label measured **40 at 16 px and still 40 at 32 px**; **40 at weight 400 and still 40 at 800** — the memo served the old box |
| after | **80** and **80**; five identical layouts still make **one** measure call |
| gates | size dropped from the key → size and button tests fire, weight test green; weight dropped → weight test only; `Button` comparing its fold but not the key → button test only |

**Cost**, alternating a binary built before the change with one built
after, four runs each:

| quantity | before | after | verdict |
|---|---:|---:|---|
| memo hit, real face (`short`) | 5.34–5.46 ns | 5.03–5.17 ns | **not slower** — the drop is not claimed |
| memo hit, stand-in | 5.37–5.41 ns (one 5.75) | 4.98–5.47 ns | not slower |
| memo miss, stand-in | 7.12–7.23 ns (one 8.41) | 7.48–7.58 ns | **+~0.3 ns** — the wider key written on a miss |
| memo miss, real face | 413–424 ns (one 468) | 413–418 ns | unchanged — the 0.3 ns vanishes in a shape |
| `Label<String>`, host | 64 B | **72 B** | +8, the key's size and weight |
| `Button<(), String>`, host | 80 B | 80 B | unchanged: `ButtonKey` held the same fields |
| embedded `Ui`, target | 1,136 B | **1,144 B** | re-asserted; the fixture holds one `Label` |
| embedded `.text` | 26,364 B | **26,396 B** | +32, inside the +300 B allowance |

## Phase B — the confirmed optimizations

*One table per task: before, after, spread, verdict against the
threshold above.*

## Phase D — the plausible findings, decided

*One row per probe: the number it produced, the arm it produced it on,
and whether the finding was promoted or closed. A finding closed here is
closed permanently, with its number.*

| # | finding | probe result | verdict |
|---|---|---|---|
| | | | |
## T26.7 — the text widgets stop measuring what shaping returned

Measured 2026-09-19, `cargo run --release -p benches --bin bindings`,
the real-face (fontdue) arms.

`TextMeasure::shape` returns a `Size` — it always has. `input.rs`
threw it away and called `measure_weighted` on the same run on the next
line, unconditionally, every layout, *including* the frames its own
shape cache hit. `TextArea` did the same with the height.

| arm (one widget's `layout`, unchanged text) | before | after | Δ |
|---|---:|---:|---:|
| `TextInput layout, unchanged` | 401 ns | 8.6 ns | **47×** |
| `TextArea layout, unchanged` | 2,101 ns | 19.4 ns | **108×** |

Fast-cluster figures; spreads are 21 ns (5%) and 40 ns (2%) before,
0.16 ns and 0.24 ns after. Both deltas are two orders of magnitude
outside them.

**The arm the plan named could not move.** T26.7's Measure line said
`--bin repeater` and `just stages`; neither lays out a `TextInput` or a
`TextArea` at all — one walks a repeater of `Fixed`s, the other a table
of labels. A figure from either would have been a bound computed from
the wrong quantity, which is this milestone's rule 2. The arm above was
added to `--bin bindings`, beside the memo rows it is about, and against
the **real face**: with the block stand-in a measure is 4 ns and the
saving is invisible by construction.

**What it is not.** This is one widget's layout, not a frame. A screen
with a single text field saves about 0.4 µs per still frame and one
with a text area about 2 µs — worth having, and nowhere near the
`layout` stage's millisecond on a 10 000-node tree
(`just stages 10000 150 20`).

**Size, measured rather than estimated**, as the task required:

| | before | after | Δ |
|---|---:|---:|---:|
| `TextInput<String>` | 152 B | 160 B | **+8** |
| `TextArea<String>` | 160 B | 160 B | **0** — the `f32` landed in padding it already had |

**The embedded fixture's `Ui` moved, and was re-budgeted rather than
edited**: 1,144 → **1,152 B** on `thumbv7em-none-eabihf`, which is the
`TextInput<InlineText<64>>` it holds taking T26.7's +8 and nothing else.
Confirmed by the compiler — the assert is a `const` one, so the build
either agrees with the number or names it. The first run of `just
check` failed on exactly this, which is the assert doing its job.

Gate: `crates/lumen-static/tests/text_memo.rs`, a **counting**
`TextMeasure` that records every call. Five still frames must ask it
nothing; changed text must still be shaped; and the cached width must
equal the measured one, because a cached number that is wrong is worse
than a measured one. Three injections (EA–EC): restore the field's
re-measure, restore the area's, and cache a stale width — all three
redden.

## T26.8 — `Badge` and `Chip` had no memo at all

Measured 2026-09-19, `cargo run --release -p benches --bin bindings`,
the real-face arms T26.0 added for exactly this.

| arm (one widget's `layout`, unchanged text) | before | after | Δ |
|---|---:|---:|---:|
| `Badge layout, unchanged` | 383 ns | 6.9 ns | **55×** |
| `Chip layout, unchanged` | 384 ns | 7.0 ns | **55×** |

Fast-cluster figures over eight runs a side. The before arm is
*trimodal* rather than bimodal — 381–392, 409–416, 417–419 — which is
what a shaping call against a real face looks like on this machine; the
lowest cluster is quoted, so the improvement is stated against the
best case the old code ever had. The after arm's whole range is
6.84–7.29 ns, narrower than the before arm's slowest run is wide.

Against `short, memo warm (hit)` at 4.97 ns in the same run: what is
left is the fold, which is the price of not having a dirty bit.
`docs/bindings-decision.md` T7.3 priced that pair at **4.8 ns against
2,459 ns** for a short run, and this is the same trade taken in two more
widgets.

**One helper, because it was a convention.** `Button` open-coded a
fold-keyed memo, `Label` had `Measured` and a `Dirty` bit, and `Badge`
and `Chip` had nothing. `lumen_static::text::FoldedText` is now the one
type for text a widget does not own — `text` is a plain public field the
application binds, so a fold of the bytes stands in for the dirty bit.

**A breaking change the task did not name**: `Badge` and `Chip` gained a
private field, so `Badge { text }` no longer compiles. Both have `new`
now, as `Label` has had since it had a memo to keep. Five call sites in
this repository, all updated in the same commit.

Gate: `crates/lumen-static-widgets/tests/text_memo.rs`, a counting
`TextMeasure` over all four widgets — five still frames must measure
**zero** times, and the failure names the widget. Plus the two
directions that make a memo a memo: changed text misses, and a restyle
misses (the key carries the size and the weight, which is what T26.3
put in it). Three injections (ED–EF): bypass the helper in `Badge`, drop
the fold from the key, drop the key and keep only the fold. All three
redden, the first by two tests and by name.

## T26.9 — the derived container's unconditional fill

Measured 2026-09-19. **No frame win, and the arm says so.**

| arm | before (fast cluster) | after | Δ | verdict |
|---|---:|---:|---:|---|
| `--bin crossover`, `1k/static` | 11,030 ns | 10,916 ns | −114 ns (−1.0%) | **no effect** — inside the after arm's own 121 ns fast-cluster spread |

Eight runs a side after a discarded warm-up. By this milestone's rule 1
a delta inside the fast cluster's spread is no effect, never a win, and
this one is.

**What the change does remove is countable rather than timeable**, and
it is one slot per transparent container per frame:

- `DisplayList::rounded_rect` already dropped a fully transparent fill —
  it has for a long time — so the *command* never existed and a
  display-list test cannot see this change at all. The first version of
  the gate counted commands and stayed green under the injection that
  unguarded the emission, which is the finding that rewrote it.
- But `Recording`, the wrapper every retained frame paints through,
  **hashes and observes before it forwards**. So the dropped command
  still consumed a retention slot and still contributed an **ink rect to
  the frame's damage**. The task's premise — "retention hashes and
  retains a command the sink dropped" — is exactly right, and is about
  retention rather than about the list.
- `--bin retain` prices pure observation at **215 ns for 34 commands**
  (`streaming: NO list + Streaming<4096>`), about 6.3 ns each. That is
  what a transparent container costs per frame, and why it does not
  surface against an 11 µs arm.
- On the embedded profile the slot matters more than the nanoseconds:
  overflowing `Streaming<N>` degrades a whole frame to `Damage::All`.

**Kept on those grounds, not on the frame number.** It also makes
`paint` agree with `hit`, which has had the same predicate since
`examples/glass` found an empty wrapper swallowing the frame's input.

**Re-measured after T26.11**, as the task's ordering note required:
`--bin retain`'s `streaming: build + Streaming<4096>` arm is 280–291 ns,
against 278.5 ns before — unchanged, which is the expected answer,
because `Streaming` kept its single buffer and its length-change
behaviour.

Gate: `crates/lumen-static/tests/container_fill.rs` — four tests, two
counters. A painter that counts *calls* (not commands, for the reason
above) and a `Retention` that counts *slots*. Both directions: a
container given a background still paints one, and alpha 1 is not alpha
0. The injection that unguards the emission reddens all four.

## T26.11 — a command-count change is no longer `Damage::All`

Measured 2026-09-19.

| arm | before | after | Δ |
|---|---:|---:|---:|
| `tests/retain_align.rs`, popup over unchanged chrome | 120,000 px² (the viewport) | **3,200 px²** | **37.5×** |
| `just profile-showcase`, the real dropdown | 691,200 px (100.0%) | 580,680 px (**84.0%**) | 1.19× |
| `--bin retain`, `streaming: build + Incremental`, 34 cmds | 328 ns | **295 ns** | −10% |

The last row is the surprise: two buffers are *faster* than one, because
the hash comparison left the per-command path for a tight zip in
`finish`. Six runs a side, fast clusters (325–340 before, 287–299
after).

The showcase's frame improves by far less than the synthetic one, which
is the honest result: opening its dropdown also changes the trigger's
label and re-lays the stage, so much of that 84% genuinely did change.

**Sizes, measured with `size_of` rather than derived**: a slot is
**24 B** — `retain.rs` said `N * 20` and "`Streaming<256>` is 5 KB", and
both were wrong; it is 6,680 B. `Incremental` is 584 B of struct, and
its heap doubles: two slot vectors instead of one, so a 3,000-command
frame holds 144 KB rather than 72 KB. The allocation gate is unmoved —
the vectors are swapped and cleared, never reallocated, and
`shell/tests/alloc_gate.rs` still reports a steady state of zero.

**`Streaming<N>` keeps one buffer**, deliberately: the second would take
`Streaming<256>` to 13 KB, a tenth of the M13 device's RAM, for a
strategy whose purpose is a bounded allocation-free footprint. A length
change still answers `Damage::All` there.

Gate: `crates/lumen-static/tests/retain_align.rs`. The good-input half
is stated **as area**, as the task required — after this change a frame
in which everything differs reports regions covering the viewport rather
than the `All` variant, so the assertions compare covered area against
the viewport's and print both. The two injections the task named both
fire: forcing `All` on a length change (120,000 px² where 3,200 is
expected) and aligning on length alone without comparing hashes (16,500
px² of 120,000, so the all-different assertion fires).

**And the gate's own fixture was wrong first.** It coloured each rect by
its *index*, so inserting a command changed the hash of everything after
it, the suffix match never fired, and a 3,200 px² popup reported 11,200.
The colour comes from the rect now.

## T26.13, part one — the raster clip stack allocated once per frame

Measured 2026-09-19, and **the gate was written first**, which is how
this task was specified.

| | before | after |
|---|---:|---:|
| `alloc_gate.rs::rasterising_a_frame_allocates_nothing`, 50 frames | **50 allocations — 1.00 per frame** | **0** |

The existing allocation gate covered `pump`: input, layout, paint and
retention. It stopped at the display list, so the software rasteriser —
and its `Vec::with_capacity(4)` clip stack, built per frame — was
outside what any test could see, on a repository whose central
invariant is zero steady-state allocation. The first run of the new
test *is* that measurement.

The stack is a fixed array now, the shape `InputQueue` and `Overlays`
already use: **nine slots — the damage band plus eight list scopes**. A
ninth list scope is *ignored*, so the content it would have bounded is
clipped by the eight above it instead. That is the conservative
direction: too much clipping draws less, never more, and drawing
outside a clip is the bug a renderer must not have. The pops that match
refused pushes are counted and declined, so the balance survives.

**The limit was off by one first**, and the test found it: slot 0 is the
band, so the original eight-slot array left room for seven list scopes
while the comment said a ninth would be refused. A fill that should have
been bounded to 20×20 covered 48×48.

The deepest nesting in this repository is four (`Repeat` in a `Popup` in
a `Card` is three).

Four injections. injection EL — restore one `Vec` per frame — **left the gate
green on the first attempt**, because an unused `Vec::with_capacity` is
an allocation LLVM is allowed to elide; pushing to it and passing it
through `black_box` makes it real, and then the gate reports 1.00 per
frame. injection EM (the stack stops clipping) reddens
`tests/paint_containment.rs`, which is the property rather than the
allocation. injection EN (a refused push pops a real scope) and injection EO (the stack
overwrites its top instead of refusing) redden the depth tests.

## T26.13, part two — the blur allocated three buffers per glass panel

Measured 2026-09-19, gate written first again.

| | before | after |
|---|---:|---:|
| `alloc_gate.rs::a_frame_carrying_glass_allocates_nothing`, 20 frames × 2 panels | **120 allocations — 6.00 per frame, 3.00 per panel** | **0** |

Three per panel is exactly the three `Vec`s `glass_within` built: the
downsampled capture, its accumulator, and the ping-pong target. Two
panels in the fixture, so "per command" is distinguishable from "per
frame" — which is what makes 3.00 a diagnosis rather than a number.

They live on the `Canvas` now and are taken out and put back across the
call, because the rest of `glass_within` calls `self.blend_px` and a
live borrow would not compile. `fit` clears and resizes rather than
reallocating, so they only ever grow.

**The banded path keeps one scratch per worker, not per panel.** A
`Canvas`'s buffers cannot be shared across threads, so a threaded frame
builds one set per band — bounded by the worker count, where it used to
be three per panel per band.

**The first attempt allocated exactly as much as before**, because it
took the buffers and never put them back. The gate reported the same
3.00 per panel, which is the gate doing its job on the fix rather than
on the original defect. Two injections: injection EP (drop the scratch instead
of restoring it) returns to 3.00 per panel; injection EQ (`fit` reallocates one
of the three) reports **1.00 per panel**, which is the count naming
which buffer.

## T26.13, part three — the capture accumulator's write-then-read

Measured 2026-09-19, `--bin glass-cost`, five runs a side.

At `k == 1` every accumulator bucket holds exactly one pixel, so the
capture wrote four `u32`s per pixel and read them back to divide by
one — a full write-then-read of `bw * bh * 4` words to perform a copy.

| σ | k | before (fast cluster) | after | Δ |
|---:|---:|---:|---:|---:|
| 2.0 | 1 | 13.70 ms | **12.56 ms** | **−8.3%** |
| 4.0 | 1 | 14.23 ms | **13.13 ms** | **−7.7%** |

Fast-cluster spreads: 0.15 and 0.06 ms before, 0.04 and 0.02 after —
both deltas an order of magnitude outside. The `k ≥ 2` rows take the
unchanged path and are unmoved.

**The σ range is the one the task named**, and the reason it matters is
`docs/results-glass.md` §1: below about 5 the search picks a single pass
and all three passes run at full resolution, so 1:1 is exactly where the
frame can least afford a redundant copy. σ = 2 and σ = 4 are the two
rows this bench runs at `k = 1`.

Gated by the tests that already exist: an injection zeroing the new
1:1 path reddens `refraction_displaces_the_backdrop` and
`degenerate_geometry_stays_finite`, which is how the branch was
confirmed to be *reached* before anything was claimed about it — a new
branch that no test enters is a change nobody has measured.

## T26.10 — the arenas stop reading live instances

Measured 2026-09-19, `--bin repeater`, six runs a side.

| arm | before | after | verdict |
|---|---:|---:|---|
| `repeat/100000-rows` (35-row window) | 1,958 ns (fast) / 2,020 (dominant) | 1,933 ns | **at the edge of resolution** |

The fast clusters give −1.3% against a ~27 ns spread; the dominant
clusters give −4.3%. Neither is a number to quote on its own, and this
change is **not justified by the frame figure**: what it removes is one
`T::default()` per frame on a window of 35 rows, which is one `Row`
construction, and the arm cannot cleanly resolve that.

What justifies it is the correctness story, which is the same one M24
was about: `Arena::ensure` and `ensure_style` took a **live probe** and
read `Node::defaults` through it. `Node::defaults` is required to depend
on the type alone — a seed's `defaults` is literally the same `fn` — so
the probe was a live instance standing in for a type, which is the
shape that produced the `List<Option<Spacer>>` defect F3.2 records. They
take `&'static [Seed]` now.

**What part three deleted**, and the size it freed:

| | before | after |
|---|---:|---:|
| `Repeat<Fixed>` | 128 B | **120 B** |

Gone: `cascade_from_instance`, the sheet lint's `Fields::ALL` arm,
`ArenaMut::template`, `ArenaMut::seeds`'s `Option`, and `Repeat`'s
`template: Option<T>` — the recycled instance it built solely so a walk
would have a live node to ask. The 8 bytes are that field on a
zero-sized row type; a larger row frees more.

And it closed a lint hole worth more than the nanoseconds: a `Dyn`
island passed `seeds: None`, so the sheet lint's arena walk used
`Fields::ALL` and `SheetIssue::Unread` could not fire for anything
inside an island. See T26.10 part one above and
`crates/lumen-static/tests/island_seeds.rs`.
