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

## T26.12 — the GPU was handed a bounding box, not M18's regions

Measured 2026-09-19.

`Prefer::Auto` picks the GPU first, and the shell handed it
`Damage::clip_to` — whose own doc says it is "the bounding box of every
region" and that it is "kept for callers that can only take one rect".
The GPU can take several: a quad carries its own clip in its vertex
data, so a region costs one more pass over the command list and **no
scissor API at all**.

| | handed over | area |
|---|---:|---:|
| two 50×50 changes at opposite corners of an 800×600 window, before | 1 rect | 452,400 px² |
| the same, after | **2 rects** | **5,000 px² — 90× less** |

**Regions are disjoint by construction**, which is what makes drawing
per region sound: `retain::Regions` merges overlapping rects as they
are added (`retain.rs`, "Overlapping regions are merged"). Nothing is
written twice, so a translucent command cannot blend into the same pixel
from two regions. The page clear follows the same rule — one opaque quad
at the head of each region's instances, exactly as the single-rect form
put one at the head of the frame.

**Where the gate lives, and why.** `just test` excludes the wgpu crate
and `just gpu-parity` runs in neither `test` nor `check`, so a gate
written beside the pixels runs on nobody's commit. The assertion is on
what the **shell hands over** — `lumen_static_winit::regions_for`, which
sits outside the `gpu` feature precisely so a test can reach it without
wgpu — in `crates/lumen-static-winit/tests/gpu_regions.rs`, which
`dev-shape` runs.

Injection (collapse to `clip_to`): the handed-over set drops to one rect
and the assertion fires by name. **`just gpu-parity` is green either
way** — worst channel delta 1, 0 px (0.000%) over 8 — which is exactly
why parity alone could not have caught this, and why the task said so
before it was built.

**The profile's own number was the merged one too**: `Frame::damaged_px`
read the single rect the backend used to be given. It sums the regions
now, so a profile reports the quantity this change is about rather than
the one it replaced.

**Ceiling, from `docs/results-gpu.md`**: the frames this changes are
damage-clipped ones, where §5b shows the GPU *losing* at small damaged
areas; §1's 1.10 ms full-surface 4K redraw bounds the win from above and
is not the case being improved. The 141× in
`docs/damage-amplification-decision.md` is a **software** damaged-pixel
ratio and is not a GPU speed-up.

## T26.14 — `index_of_wid` as a `const fn`

Measured 2026-09-19. **No change to rustc's time**, which is what Phase
C is judged on.

| | `just const-budget`, 3,711 nodes |
|---|---|
| before | 331, 353, **202, 202** ms |
| after | 215, 215, **184, 197** ms |

The first runs of each set are warm-up: the recipe rebuilds its
dependency after a source change, and that lands inside what it prints.
The settled runs are 202 ms against 184–197, and the recipe's own
run-to-run spread is larger than the difference. **No effect** — as
expected, because the fixture never calls `index_of_wid`. The ceiling
(5,000 ms) is untouched, and this is the millisecond figure the recipe
prints rather than a percentage read against that ceiling, which would
be a bound computed from the wrong quantity.

**What it buys** is a `wid_index!(Tree, "id")` that resolves at compile
time. A typo'd or renamed `#[widget_id]` marks *nothing*: the lookup
returns `None` — it cannot return a *wrong* index — and the frame
quietly fails to redraw.

**Honest scope, restated from the task**: it is silent only for a caller
that ignores the answer. `Host::invalidate` and `Host::restyle_subtree`
have returned a `#[must_use] bool` since T16.7 for exactly this reason
and every caller in this repository asserts it, so this buys little
against the current tree and everything against the next caller and
third-party code.

Gate: `crates/lumen-static/tests/wid_index.rs` — and the good-input half
is the one with work in it. It walks the **table**, asserting that the
const and runtime lookups agree for every id a tree carries, so an id
added later is covered without anyone remembering to add it. The
compile-fail half is a `compile_fail` doctest on the macro. Two
injections: making a missing id resolve to 0 (the doctest compiles, and
the compile-fail case fails) and making the const lookup return `i + 1`
(the agreement test fires, naming the node).

## T26.15 — the lint's off switch stops being the default

The mask in `Seed::reads` defaulted to `Fields::ALL`, and the
constructor's own rustdoc recorded that as deliberate: a kind that had
not declared its reads "is not linted rather than wrongly linted".

**Reversed.** `ALL` is the `W1010` lint's off switch, and it was what a
*new* widget got by doing nothing — so the widget most likely to carry a
mistargeted rule was the one the lint could not speak about. `Seed::new`
takes the mask; `Seed::unlinted` spells the opt-out for the cases where
"reads everything" is honest (`Seed::WIDGET`, a wrapper's padding slots,
a fixture that is not about styling).

**The size of the break, counted rather than estimated**: 57 call sites
in this repository, of which **53 already declared a mask** and four did
not — `Seed::WIDGET` and three test fixtures. `Seed::reading` is deleted
rather than kept beside the new constructor, because two ways to say it
is how one of them goes stale.

Source-breaking for out-of-tree widgets, and taken deliberately: the
seeds array is a required associated const, so a third-party impl cannot
avoid the constructor.

No measurement: this buys no frame time and does not touch a const-eval
path. Phase C's other judgement — rustc's time — is unmoved, and the
task claims nothing there.

Gate: `lumen-static-widgets/tests/style_reads.rs` still measures every
kind's output field by field and compares it to the declared mask; the
new half is a `compile_fail` doctest beside a passing one, so a widget
declared without a mask does not build.

**Three documents move with it**, all of which recorded the old
rationale: `Seed::new`'s rustdoc, `docs/plan-appearance.md` T25.2, and
the book's custom-widget chapter, whose snippet gained the argument and
whose paragraph beneath it now says why.

## T26.16 — shape asserts, and the duplicate id

Two halves, 2026-09-19.

**The lengths.** The derive asserted `table_is_well_formed` and the
depth bound, and neither says the array lengths agree with
`NODE_COUNT`. The plan's claim that "the derive half is unfalsifiable"
is true of **exactly one of four** cases, and the injections confirm
it: changing the struct derive's `NODE_COUNT` fires the **seeds**
assert, not the table one, because the table array is sized by
`NODE_COUNT` and therefore tracks it. The seeds array and both of the
enum derive's lengths are re-spelled sums and can drift.

| injection | what fires |
|---|---|
| struct derive's `NODE_COUNT` off by one | "the spliced seeds array's length is not this tree's NODE_COUNT" |
| enum derive's `NODE_COUNT` off by one | the seeds assert, then the table assert, then `table_is_well_formed` |

**The duplicate id.** A duplicated `#[widget_id]` is accepted silently
and every id-*addressing* operation acts on the first match — a
`Host::invalidate` that marks one of two, forever. The rejection lives
on the const addressing path (`wid_index!`), **not in the derive**,
because the derive cannot tell an addressing use from a styling one:
`examples/glasswall` styles four intentional duplicates by selector and
addresses none of them.

Both directions, as the task demanded, because a gate that refuses
everything is the failure this rule exists for:

* addressing a duplicated id does not compile — a `compile_fail`
  doctest; an injection accepting any count turns it green, which is how
  it was confirmed to be doing work;
* `Id` styling still matches **all four** of a four-way duplicate
  (`tests/wid_index.rs`), and `glasswall` still builds.

One injection was wrong before the code was: the first attempt at the
accepting-injection allowed counts of `1 | 4`, and the doctest's tree
carries **two** duplicates, so nothing moved. An injection aimed at a
number the fixture does not have proves nothing — the same shape as a
bound computed from the wrong quantity, one level down.

## T26.17 — Phase D, batch one: findings 1, 3, 4

The kill rule, as `docs/plan-frame-and-types.md` §3 wrote it before any
probe ran: **no promotion without a delta that beats its arm's four-run
spread, and a probe that comes back inside the spread closes the finding
permanently.** All three numbers below are min-of-150-iterations after
20 warm-ups, on `just stages`, whose `us` column is microseconds.

### Finding 1 — `place` has no relayout boundary: **closed**

`place`, 10 000 nodes, the idle/rebuild arm, four runs:

| run | 1 | 2 | 3 | 4 | spread |
|---|---|---|---|---|---|
| `place` (µs) | 7.84 | 7.75 | 7.72 | 7.52 | **0.32** |

The frame on the same arm is **350.72 µs** (two runs: 351.47, 349.97),
so `place` is **2.2% of an idle frame** — and that is the figure for
deleting `place` entirely. A relayout boundary wins strictly less than
deleting the pass, because it still has to ask each subtree whether it
may skip.

The finding is closed on its own ceiling rather than on a built
boundary, which is the cheaper half of the kill rule: an upper bound
that is already small does not need the optimization built to be
decided. It also confirms the plan's own correction — the review cited
7.6 µs for this finding, which is an **arena** figure; the main table's
`place` costs 7.7 µs at 10 000 nodes, and the numbers agreeing to two
significant figures is a coincidence of scale, not evidence.

### Finding 3 — `record_painted` re-stamps ten style-derived fields, eight of them static: **worth building, not built in M26**

Probed by measuring the **ceiling**: `record_painted` was cut down to
stamp nothing at all — an injection that is incorrect by construction,
which is exactly what makes it an upper bound. Six runs a side,
`layout`, 10 000 nodes, idle/rebuild, sorted:

| | runs (µs) | fast cluster | median |
|---|---|---|---|
| as shipped | 35.23 35.34 35.35 37.04 37.18 37.52 | 35.23–35.35 (0.12) | **35.34** |
| stamping nothing | 29.98 30.03 30.04 30.04 30.05 42.04 | 29.98–30.05 (0.07) | **30.04** |

**−5.30 µs, or −15.0% of `layout`**, against fast-cluster spreads of
0.12 and 0.07 µs — two orders of magnitude clear of the bar. Against the
whole idle frame it is **1.5%**.

The achievable share is smaller than the ceiling. `record_painted`
performs **ten** writes — `fg`, `bg`, `radius`, `focus_ring`,
`border_color`, `border_width`, `font_size`, `font_weight`, `shadow`,
`pad` — and two of them are genuinely per-frame: `focus_ring` is
rewritten per frame by the host, and `bg` is *resolved* here from
`pressed`/`hovered` (C6), not copied. **Eight are static**, so the
buildable win is roughly four fifths of 5.30 µs, or **4.2 µs**.

The plan's own correction to the review — "nine fields, not ten, because
`focus_ring` is rewritten per frame" — was itself one short: it counted
the writes and caught the host's field, and missed that `bg` is the one
write in the function that is not a copy at all. Counting a list is not
reading it.

It is recorded as worth building and **not built in M26**, because the
implementation splits `record_painted`'s inputs across two passes — the
static fields stamped where the cascade runs, the dynamic two left here
— and every existing caller of the cascade becomes a site that must
stamp. That is a correctness surface, and M25/T25.0 is what a stale
stamp under `Boundaries` looks like when it goes wrong.

First run of this probe was three runs a side and read *slower*
stamping nothing (43.59 42.09 30.01 against 37.52 37.22 35.61). Three
runs could not separate the arm's two clusters; six could. A bimodal arm
sampled below its own period is the "bound computed from the wrong
quantity" shape wearing a different hat — the quantity was right, the
sample was not.

### Finding 4 — paint culling stops at the arena boundary: **closed**

True as a fact and worthless as an optimization, which the probe
separates and reading could not.

`Painter::clip_bounds` has exactly one caller in the whole workspace —
`List::paint` (`crates/lumen-static/src/hatch.rs`). The generated
`paint` for a derived container culls nothing. So the finding's premise
holds.

What it is worth is bounded by two measured numbers:

| quantity | measured |
|---|---|
| paint, per node | **7.00 ns** at 1 000 and 3 000 nodes, 7.50 ns at 10 000 (7.15 / 21.37 / 75.06 µs) |
| painted commands wholly outside the surface, showcase settled frame | **0 of 25** |

Culling saves at most 7.5 ns per off-surface node, less the rect test it
adds to every node. Reaching even 10 µs — the smallest thing the
`stages` arm resolves above its spread — needs **~1 500 authored,
off-surface nodes**. The authored tree is written by hand: the
showcase, the largest in the repository, paints 25 commands and none of
them is off-surface. Node counts get large only in arenas, and arenas
already cull (T16.12).

The plan's warning was the load-bearing part: T16.12's 8× is `List`'s
arena loop and must not be borrowed for the derived tree, because the
two differ by exactly the property that makes culling pay — a `List`'s
length is data, a derived container's is source code.

## T26.17 — Phase D, batch two: findings 5, 6, 8

Three more probed, three closed. All three were *true as written* and
worth nothing when measured, which is the distinction this round exists
to draw. Each probe measured a **ceiling** — the work removed outright,
which no implementation can beat — so a ceiling inside the arm's spread
closes the finding without the optimization ever being built.

### Finding 5 — the generated scans copy a whole `Style` per child: **closed**

True in the source. The derive emits `let cs = style[at];` once per
child at two sites in the fill-distribution scan
(`crates/lumen-static-macros/src/lib.rs`), and `Style` is **80 bytes**
(`crates/lumen-static/tests/cursor.rs` pins it).

Rewritten to `let cs = &style[at];` at both sites, `layout`, 10 000
nodes, idle/rebuild, six runs sorted:

| | runs (µs) | fast cluster |
|---|---|---|
| as shipped (copy) | 35.23 35.34 35.35 37.04 37.18 37.52 | **35.23**–35.35 |
| borrow | 35.28 35.60 37.19 37.54 37.60 43.17 | **35.28**–35.60 |

**+0.05 µs**, inside the fast cluster's own 0.12 µs spread. No effect,
and by this repository's rule that is never a win.

The codegen diff the plan asked for says the same thing from the other
side: `--bin fusion`'s `.text` goes **379 032 → 379 320 bytes**, so
removing the copy made the binary **288 bytes larger**. LLVM was
already eliding it; spelling the borrow only perturbed register
allocation.

**The probe carries its own setup assertion**, because "the scan is
cold in this arm" is exactly the vacuous shape `AGENT.md` names first.
A `panic!` was planted in the rewritten scan and the arm was run: it
panicked, in `arms/stages`, at 10 000 nodes. The path is live, so the
+0.05 µs is a measurement of the thing and not of its absence.

### Finding 6 — `Seed::defaults` is a fn pointer per node: **closed**

The ceiling: `cascade_range`'s `let mut s = (seeds[i].defaults)();`
replaced by a loop-invariant `Style::default()`, which removes the
indirect call *and* lets LLVM hoist the initializer out of the loop
entirely — strictly more than a const `Style` field could win, since
that would still load 80 bytes per node.

`--bin cascade`, four runs each, min column:

| arm | as shipped (ns) | ceiling (ns) | delta | four-run spread |
|---|---|---|---|---|
| `cascade_deep`, whole table, 901 nodes | 12 287 12 258 12 391 12 768 | 12 210 12 150 12 334 12 492 | **−107** | 510 |
| `cascade_deep`, showcase-shaped sheet | 40 507 40 653 40 890 40 786 | 41 196 40 796 40 768 40 995 | **+261** | 382 |

Both inside the spread, and the showcase arm came back *slower*. Per
node the whole-table delta is **107 ns / 901 = 0.12 ns**, which is below
the bench's own harness floor of 0.30 ns — the quantity printed beside
the bound, and the bound is smaller than the ruler.

The rustdoc on `Seed` already gave the reason the thunk exists —
`Default::default` is not a `const fn`, so the field cannot be a
`Style` — and the measurement now says there is nothing to trade for.

### Finding 8 — `cascade_subtree`'s debug check runs a full cascade: **closed**

True, and the plan's correction was already the right frame: the check
is `#[cfg(all(debug_assertions, feature = "alloc"))]`, so the win could
only ever be test-loop time. The probe asks the one question that
settles it — **how often does it run, and over how many nodes?**

A counter planted at the check, `cargo test --workspace -- --nocapture`:

| tree size | calls |
|---|---|
| 15 nodes | 6 |
| 9 nodes | 3 |
| 4 nodes | 1 |
| **total** | **10 calls, 109 nodes cascaded** |

Ten calls in the entire workspace suite, which runs in **12.4 s**. A
release whole-table cascade is 12.29 µs for 901 nodes — 13.6 ns/node —
so even at a generous debug multiplier the check's total cost across
every test in the repository is tens of microseconds. An exact
diff-based check would replace tens of microseconds with fewer tens of
microseconds.

The first attempt at this probe reported **zero** calls and would have
closed the finding for the wrong reason. `cargo test` captures a
passing test's stderr, so the counter was swallowed; `-- --nocapture`
is what made it visible. A probe that reports zero is the "setup
silently did not happen" shape, and zero is the one result that must
always be disbelieved before it is used.

## T26.17 — Phase D, batch three: findings 9, 10, 11

The round's yield. Two findings promote — and they turn out to be **one
defect measured twice** — and one closes on cost after its named arm was
found unable to exhibit it.

### Findings 9 and 10 — the per-node arena ask: **promoted, and they are the same defect**

Finding 9 is "the three deep walks ask every node whether it owns an
arena"; finding 10 is "`lint_sheet_deep` re-walks every arena per rule".
Both are the same virtual call — `Node::arena_count()` through
`tree.node_mut(i)` — made once per node in the first case and once per
node **per rule** in the second.

Priced directly. A temporary arm timed `cascade_arenas_deep` alone on
`Ten10`, a 901-node tree with **no arenas at all**, so every call
answers zero:

| the walk alone, 901 nodes, no arenas | 5 277 / 5 345 / 5 389 / 5 397 ns |
|---|---|
| **per node** | **5.9 ns** |

And the two arms it sits inside, min of four runs each:

| arm | as shipped | walk removed | delta | four-run spread |
|---|---|---|---|---|
| `cascade_deep`, whole table | 12 150 ns | 7 338 ns | **−4 812 (−40%)** | ~400 |
| `cascade_deep`, showcase sheet | 41 395 ns | 36 293 ns | **−5 102 (−12%)** | ~250 |
| `lint_sheet_deep`, showcase sheet | **166 124 ns** | **40 486 ns** | **−125 638 (−75.6%, 4.1×)** | 1 394 |

The lint is the headline: **three quarters of it is asking 901 nodes,
27 times each, whether they own an arena** — 24 327 asks at 5.2 ns,
which agrees with the 5.9 ns/node measured in isolation. `lint_sheet_deep`
runs on every `Host::set_stylesheet`, so this is 126 µs on a sheet swap
spent on nodes that have none.

**A code-layout artifact nearly closed finding 9 wrongly.** The first
walks-removed run showed the whole-table arm dropping 4.6 µs and the
showcase arm dropping 0.2 µs — the same function, on the same tree, with
only the sheet different, which is impossible. Rather than pick the
convenient number, the walk was given its own arm and timed alone: 5.3 µs.
Adding that arm to the binary also moved the showcase arm's *baseline*
by ~700 ns and its walks-removed figure from 40.3 µs to 36.3 µs — the
extra arm shifted code alignment. The figures above are all from the one
binary that contains the isolating arm, so they are comparable with each
other. **Two arms that disagree about the same quantity are a fact about
the arms**, and the third, isolating arm is what settled which was
lying.

The fix both findings share is the one the plan proposed: a spliced
const list of the table indices that own arenas. `Ten10`'s is empty, so
the walk becomes a no-op; a tree with two repeaters visits two nodes
instead of 901. The plan's caution on finding 10 stands and is now
unnecessary — half (b), hoisting the walk out of the rule loop by
accumulating per-rule state, would allocate; narrowing the walk needs no
allocation at all and fixes both sites.

### Finding 11 — ids and classes compared by content: **closed on cost**

**The arm the plan named cannot exhibit this finding.** `benches --bin
cascade`'s 24-`Id`-rule sheet runs against `Ten10`, which carries no ids
— so `d.wid` is `""`, `d.wid == *id` compares lengths 0 and 3, and
returns false without a memcmp. That is already what a hash compare
costs. Measuring there would have produced a green probe and a false
close, which is the first vacuous shape `AGENT.md` names: the setup
silently did not happen.

Three arms were built instead, 21 624 comparisons each (901 nodes ×
24 rules), min of three runs:

| shape | ns | per compare |
|---|---|---|
| every node carries an id sharing a 12-char prefix — the worst case | 31 863 | 1.47 ns |
| ids sparse, one node in forty — the shape a real tree has | 5 148 | 0.24 ns |
| `u64` hash compare | 2 986 | 0.14 ns |

Against the worst case a hash is **10.7×**. Against the shape the
framework actually has it is **5 148 → 2 986 ns, −2.16 µs** per full
sheet-versus-table match — which does clear the arm's 61 ns spread, so
it passes the kill rule's bar.

It is declined on the other side of the ledger, which the plan named
first: a `wid_hash` is **8 bytes per node of `.rodata`**, +800 B on the
100-node embedded fixture whose flash is asserted
(`docs/results-embedded.md`), and it buys 2.16 µs on a **sheet swap**,
which is an event and not a frame. Recorded rather than deleted, with
the number that would reopen it: a tree whose nodes carry dense,
same-length, shared-prefix ids pays 1.47 ns a compare instead of 0.24,
and at that point the 10.7× is worth 8 bytes a node.

## T26.17 — Phase D, finding 12: a confirmed defect, not an optimization

The one finding in the plausible set that was never about speed. It is
**reachable, it was reachable all along, and the ordering is the
ordinary one** — which is the opposite of the plan's own prediction that
"the failure needs an event ordering nobody has produced".

`MeasureKey` carried the limits, the font size and the weight (T26.3).
It did not carry the display scale, and the measurer's answer depends on
it: `FontMetrics` measures at `size * scale` and divides the answer back
down, precisely because a hinted face's advances are **not** linear in
size — 16 px doubled is not 32 px, and its rustdoc has said so since
`docs/results-frame-cost.md` recorded "POSITIVE" rendering as
"POSITIV".

What made it look unreachable is true as far as it goes: a scale change
alone moves the logical viewport, so the limits move with it and the key
changes. But `winit`'s `ScaleFactorChanged` delivers a new scale **and**
a new physical size in the same event, and the ordinary case — a window
dragged from a 1× monitor to a 2× one — holds the *logical* size
constant while doubling the physical one. Limits unchanged, scale
doubled, memo hit, stale box.

Reproduced before it was fixed:

```
1x: 129; 2x under identical limits: 129 (measurer calls 1 -> 1)
```

One measurer call for two different displays.

**The fix**: `MeasureKey` gains the scale, `LayoutCx` carries it
(`with_scale`, `scale()`), and `Host::pump_with` hands over its own. The
scale is a **required** argument of `MeasureKey::of`, for the reason
that made that constructor take the whole `Style` rather than a size:
a parameter a caller may omit is a field that will be omitted, and this
omission is invisible until a display changes under a running window.
`LayoutCx`'s default of 1.0 is correct for every headless caller,
because the defect needs a scale that *changes* and only a windowed host
has one.

Both halves, as the rule requires:

| direction | test | result |
|---|---|---|
| refuses bad input | `a_scale_change_under_the_same_limits_re_measures` | red before the fix, green after |
| accepts good input | `an_unchanged_scale_still_hits_the_memo` | green throughout — one measurer call for two identical layouts |

**Injection**: `MeasureKey::of` made to discard the scale it is handed
(`scale: { let _ = scale; 0 }`). The first test fails by name and prints
`1x: 129; 2x under identical limits: 129`; the second stays green, so
the injection is aimed at the key's field and not at the memo's
existence.

The test brings its own measurer for the reason T26.3's did:
`BlockMetrics` is linear in size, so its logical answer is identical at
every scale and it **cannot exhibit this defect**. A test written
against it would have passed with the bug in place — the first vacuous
shape, where both sides agree about a frame in which nothing happened.
