# M26 — the defects under the frame path, what to make faster, and what the type system should have caught

> **Not started. Written 2026-09-17** from the multi-agent review of
> 2026-09-16: 14 agents over 7 subsystems, each finder followed by an
> adversarial verifier that re-opened the code. **42 findings — 20
> confirmed, 18 plausible, 4 refuted.** §6 records the verdicts, the
> corrections the verifiers made to their own side's numbers, and the
> four the review killed.
>
> **Defects first, and they are not optimizations.** Three confirmed
> findings are shipped bugs of the class M25/T25.0 was: state that goes
> stale under `Boundaries` and is correct under `Everything`, so every
> test passes and the window is wrong. They are T26.1–T26.5 and they land
> before anything is made faster.
>
> **The order after that is evidence, not appetite.** Phase B is the
> confirmed optimizations, each with a bench arm named before it is
> built. Phase C is the compile-time guarantees. Phase D is a measurement
> round that decides which of the 18 plausible findings are worth
> building at all — several of them had their claimed win cut by their
> own verifier, and three cited a number measured on a different quantity.

*Each task is one commit unless it says otherwise; each ends with
`just test` green and the arm named in its **Measure** line re-run. Task
ids go in commit subjects. Every gate names the injection it must go red
under, and prints its measured quantity beside its bound.*

---

## 0. The rule for this milestone

Four things every task does, because the review found the repository's own
numbers borrowed across quantities nine times:

1. **Measure before and after, on the arm the task names**, four runs,
   medians, with the runs' own spread printed beside the delta. A change
   whose delta sits inside that spread is reported as "no effect", never
   as a win.
2. **State the quantity.** A figure from `docs/results-*.md` may only be
   cited for the case it was measured on. The review's own findings cited
   an arena `place` figure for a table walk, a software damage ratio for a
   GPU change, and a 10 000-*row* state array that is 11 000 *nodes*; all
   three were corrected by verification, not by review.
3. **Guard the budgets that do not appear in a frame number**:
   `scripts/check_text.sh` (`.text`), the struct-size asserts in
   `fixtures/embedded`, `just const-budget` (rustc's time — the ceiling
   `docs/results-const-eval.md` records), and the allocation gate.
4. **Docs move in the same commit**, including the decision record the
   task contradicts. Several tasks here reopen a DECIDED question; each
   says which.

**Regression guard.** `just test` per task; `just check` (which adds
`thumb`, `gpu`, `simd`, `threads`, `floats`, `book`) before each phase
closes.

## T26.0 — baselines and thresholds, before anything changes

No behaviour change. Records what every later task is judged against, in
`docs/results-frame-and-types.md`, and **adds the arms that do not exist
yet** — the review could not price five of its findings because nothing
measures them:

| arm | what it must newly cover |
|---|---|
| `benches --bin invalidation` | a `place` row (it is layout-only today, and says so), and a dispatch row: one `PointerMove` under `Everything` and under `Boundaries` |
| `benches --bin cascade` | a sheet-lint row, on the 27-rule showcase-shaped sheet it already builds |
| `benches --bin crossover` | an `Either` row, so a hidden branch's cost is visible |
| `benches --bin repeater` | rows of a text leaf, so a memo's absence shows |
| `just profile` | a popup-open frame on the showcase, the case T26.11 exists for |

**Gate — a new arm must be able to say no.** For each arm added, break
the thing it measures and watch the number move: delete the relayout
boundary in the generated `layout` and the `place` row must move; force
`Damage::All` and the popup frame must jump to the window's area. An arm
whose number does not move under its own injection is reported and fixed
before the baseline is taken, not after.

**Thresholds**, written here and owned by the results doc: no arm
regresses by more than its own four-run spread; `.text` ≤ +300 B over the
M26 baseline; `just const-budget` ≤ +10% rustc time; the allocation gate
stays at zero; struct sizes unchanged unless the task says otherwise and
records the new number.

---

## Phase A — the defects (T26.1–T26.5)

Each of these lands **test first**: the failing test is committed in the
same commit as the fix, and the commit body carries the output it
produced before the fix.

### T26.1 — the enum derive zeroes the active variant before laying it out

`crates/lumen-static-macros/src/lib.rs:1197` calls `hide_subtree` over
`NODE_COUNT - 1` — every variant's slots, the active one included — and
then lays the active variant out. Under `Everything` layout rewrites it
the same frame. Under `Boundaries` a clean active subtree returns before
`record` runs and **keeps the zeroed geometry**: the stage of
`examples/showcase` disappears on any frame where something else is
marked. The const `STARTS` array the derive already emits names exactly
the ranges that should be hidden.

**Fix:** hide the ranges before and after the active variant, not the
whole span.

**Gate:** an enum tree on `Boundaries<16>`; settle; mark an unrelated
node; pump; the active variant's size is unchanged and non-zero. Both
strategies asserted, because the bug is invisible under one of them.
*Injection:* restore the whole-span hide — the `Boundaries` case fires,
the `Everything` case stays green, which is the shape of the defect.

**Docs:** `docs/plan-invalidation.md` T16.9 (whose measurement table has
no enum case), `docs/plan-select.md` T24.3.

### T26.2 — the agent reports a hidden variant's kind as its container's

`TreeRef`/`TreeMut` read kinds from the live tree, so every node of an
inactive variant answers with the container's kind — in `ui.find`, the
tree dump, the journal summary and `W1001`. This is the defect M24/T24.1
fixed in the cascade, in the surface next door.

**Fix:** carry `W::SEEDS` on `TreeRef`/`TreeMut` and read `kind` from the
seed, as the cascade does.

**Gate:** a tree with an inactive variant; the agent reports that node's
own kind. *Injection:* read the kind from the instance again — fires,
naming the node.

**Docs:** `docs/plan-select.md` §1 and T24.0–T24.1 ("a fact that cannot
vary per instance, reachable only through an instance, is stored in the
wrong place" — this is the last place it still is).

### T26.3 — the text memo's key omits font size and weight

`Measured` keys on constraints and a dirty flag, so a restyle that
changes only the size or weight of a run reads a stale box. Nothing in
the tree trips it today — `examples/counter-window` keeps the same
`font_size` in all three sign arms — which is why it is a defect found by
reading rather than by a frame.

**Fix:** a key type whose only constructor takes the whole `Style`, so a
caller cannot build one that omits a field. Thirteen call sites across
`input.rs` and `textarea.rs` build the key by hand today.

**Gate:** restyle changing only `font_size` under unchanged constraints
→ the measured box changes. *Injection:* drop the size term from the key
— fires. **Record the struct size**: `Measured` is 28 B today, measured,
not 24 as the review first claimed.

**Docs:** `docs/layout-and-storage-decisions.md` §1,
`docs/text-and-scrolling-decisions.md` B2.

### T26.4 — `Assembled::default_styles` seeds from a live probe

T24.1's defect in a fourth place: the slab builds its styles from
`C::default()` instead of `C::SEEDS`, so an `Assembled` whose content
holds an enum or a hidden subtree cascades from the container's defaults.
Latent — no in-tree fixture has one — and two lines to fix.

**Gate:** an `Assembled` whose `C` contains an enum; its hidden nodes get
their own kinds' defaults. *Injection:* restore the probe — fires.

**Docs:** `docs/plan-review-fixes.md` F4.2.

### T26.5 — `Boundaries<N>` truncates its length, and keeps a second copy of `is_clean`

The length is a `u8` with no bound on `N`, so `Boundaries<300>` silently
truncates; and `Boundaries::is_clean` is a second implementation of
`Changed::is_clean` kept honest by a unit test rather than by
construction.

**Fix:** a const assert on `N`, and one implementation called by both.

**Gate:** the existing agreement test, plus a documented compile-fail
injection (`Boundaries<300>` must not compile; the repository has no
compile-fail harness, so this one is run by hand and recorded in the
commit body).

**Docs:** `docs/plan-invalidation.md` T16.7.

---

## Phase B — the confirmed optimizations (T26.6–T26.13)

### T26.6 — the cascade's per-node work that is a property of the sheet

Walk the sheet once before the node loop and record which specificity
levels it actually has; skip the passes for levels that are absent.

**First, settle the number.** The review reported this twice with
different figures — 2.2× in one agent's run and 54,399 → 40,739 ns
(1.33×) in another's. Neither is citable. Reproduce both on
`benches --bin cascade`, four runs, and record what is true.

**Gate:** resolved styles are identical to the naive cascade for every
sheet in the workspace, asserted node by node. *Injection:* drop a level
that is present — styles differ, the assertion names the node.

**Measure:** `--bin cascade`, three rows. **Not a frame win** — the
cascade runs on sheet, palette or shape change, which `style.rs` states
as its contract.

**Docs:** `docs/results-cascade-cost.md` gains the row.

### T26.7 — the text widgets measure what shaping already returned

`input.rs:472` shapes behind a key guard and `:484` then calls
`measure_weighted` on the same run unconditionally, every layout.
`TextArea` does the same. The width is already in what `shape` returned.

**Fix:** widen the cached tuple to hold the run size and line height.
**+8 B per field** on the host (measured, not estimated); the embedded
`Ui` assert moves with it.

**Gate:** a counting measurer: calls per frame for unchanged text drop to
zero. *Injection:* narrow the cache back — the count rises and the test
names it.

**Measure:** `just stages 10000 150 20` and `--bin repeater`'s new text
row.

### T26.8 — `Badge` and `Chip` shape on every layout, with no memo at all

Five text widgets, four different memo keys, two with none.
`docs/bindings-decision.md` T7.3 priced the difference between a shape
and a compare at **4.8 ns against 2,459 ns** for a short run.

**Fix:** one helper taking T26.3's key type, used by all five, so the
memo is a property of the helper rather than a convention repeated
thirteen times.

**Gate:** shapes per layout for unchanged content is zero for every one
of the five. *Injection:* bypass the helper in one widget — fires by
widget name.

### T26.9 — every derived container emits a background command it usually has none of

The generated `paint` emits shadow and fill unconditionally; retention
then hashes and retains a command the sink dropped. `hit` already knows
the predicate.

**Gate:** a transparent container emits no body command, and retention
keeps no slot for it. *Injection:* unguard the emission — both counts
rise.

**Measure:** `--bin retain`, `--bin paint_sink`. Also shortens the stream
for `Streaming<N>` on the embedded profile, where overflowing `N`
degrades a frame to `Damage::All`.

### T26.10 — the arenas stop reading live instances

`Dyn` can capture `W::SEEDS` at construction exactly as it captures
`W::SUB_TABLE`; `Arena::ensure`/`ensure_style` can take `&'static [Seed]`
instead of a probe. Together they delete `cascade_from_instance`, the
lint's `Fields::ALL` hole, `Repeat`'s `template` field and a per-frame
`T::default()`.

**Gate:** a `Dyn` island's kinds come from seeds, asserted on a hidden
variant inside one; the lint no longer reports every field as read for an
island. *Injection:* restore the instance read — fires.

**Measure:** `--bin cascade`, `--bin repeater`.

**Docs:** contradicts `docs/plan-select.md` T24.1's "one case still reads
an instance, deliberately" — that case is the one being deleted — and
changes the signature `docs/plan-review-fixes.md` F4.1 specifies.

### T26.11 — a command-count change is `Damage::All`

`retain.rs:571-577`: a length change damages the window. Every popup
open and close and every enum-variant switch therefore repaints
everything, on the strategy the desktop shell defaults to.

**Fix:** two slot buffers instead of one, with `finish` aligning them
when the lengths differ — longest common prefix and suffix by hash,
damage the middle. **A slot is 24 B** (measured; `retain.rs`'s own
comment says 20 and is wrong — fix it in this commit), so this doubles
the retained array.

**Gate:** a popup-open frame damages the popup's rect, not the window,
on `Incremental` + `Boundaries<16>`; and a frame that genuinely changes
everything still reports `Damage::All`. *Injection:* force `All` on a
length change — the first assertion fires; *second injection:* align on
length alone without comparing hashes — the second fires.

**Measure:** `--bin retain`, and `just profile` on the new popup frame.

**Docs:** `docs/plan-invalidation.md` T16.8 is **reopened** — its stated
blocker was that retention keeps no display list, and this keeps none,
only a second array of hashes. `docs/damage-amplification-decision.md`.

### T26.12 — the GPU backend collapses M18's region set to its bounding box

`Prefer::Auto` picks the GPU first, and the GPU takes the bounding box of
the damage regions, which is the merging cost M18 removed.

**Fix:** `Gpu::draw` takes `&[Rect]`, at most `retain::MAX_REGIONS`, and
scissors per region.

**Gate:** `just gpu-parity` stays byte-identical, and a test asserts one
scissor per region rather than one for the union. *Injection:* collapse
to the bounding box — the region-count assertion fires while parity stays
green, which is why parity alone could not catch this.

**Measure:** `just profile` with `--features gpu`. **Ceiling, stated
first:** `docs/results-gpu.md` prices a full 4K redraw at 1.10 ms, so
this cannot buy more than that on the worst frame — and the 141× in
`docs/damage-amplification-decision.md` is a *software* damaged-pixel
ratio and must not be restated as a GPU speed-up.

### T26.13 — the raster path allocates per strip and per glass command

The repository's stated invariant is zero steady-state allocation, and
`crates/lumen-static-shell/tests/alloc_gate.rs` cannot see the raster
path.

**Fix, three independent parts:** a fixed clip stack with a depth limit
(the shape `InputQueue` and `Overlays` already use), a reused glass
scratch buffer, and the capture path's write-then-read removed at the
preset `docs/results-glass.md` §1 identifies as the expensive one.

**Gate — the gate first.** Extend the allocation gate to cover a raster
frame; *injection:* restore one `Vec` per strip and the gate fires. Only
then change the code.

**Measure:** `just profile`, `--bin glass-cost`.

**Docs:** `docs/results-allocation.md`'s "what is not covered" shrinks by
what this closes.

---

## Phase C — the compile-time guarantees (T26.14–T26.16)

These buy no frame time. They are judged on **what a real defect in this
repository's history would have been prevented**, and on rustc's time,
which `just const-budget` gates.

### T26.14 — `index_of_wid` as a `const fn`

A typo'd or renamed `#[widget_id]` is a silent no-op today: the wrong
subtree is invalidated or restyled and the frame quietly fails to
redraw. A const lookup makes it a compile error.

**Honest scope, from the verifier:** every in-tree caller already
asserts the `Option`, so this buys nothing against the current tree and
everything against the next caller and third-party code.

**Gate:** runtime and const lookups agree for every id in the workspace;
a documented compile-fail injection for a typo'd id.

### T26.15 — `Seed::reads` stops defaulting to `Fields::ALL`

The default is the W1010 lint's off switch, and it is what a new widget
gets. Make the mask a parameter, with an explicit constructor for the
deliberately unlinted cases (`Seed::WIDGET` and the wrapper padding
slots).

**Gate:** `crates/lumen-static-widgets/tests/style_reads.rs` still
verifies all 30 measured masks; a widget declared without a mask does not
compile (manual injection, recorded).

**Docs:** `docs/plan-appearance.md` T25.2.

### T26.16 — shape asserts for hand-written impls, and duplicate ids

Two halves, one commit:
- `SUB_TABLE.len() == NODE_COUNT` and `SEEDS.len() == NODE_COUNT` are
  const facts asserted only for derived trees. The four hand-written
  generic impls write both quantities twice. **The derive half is
  unfalsifiable** — there the two are the same expression — so this is
  scoped to hand-written impls, where it is real.
- Duplicate `#[widget_id]` among one container's fields is accepted
  silently, and every id-*addressing* operation then acts on the first
  match. Reject it for addressing; **styling must keep matching them
  all**, which is what `examples/glasswall` relies on with four
  intentional duplicates.

**Gate, both directions:** the duplicate case fails to compile *and*
`glasswall` still builds. A gate that refuses everything is the failure
mode this rule exists for.

---

## Phase D — T26.17: decide the 18 plausible findings by measurement

**None of these is built on the strength of the review.** Each gets the
smallest probe that can move its number, in the order below; the probe is
run, recorded in the results doc, and the finding is either promoted to a
task or closed with its number. Budget one commit per batch of three.

**Kill rule, written first:** no promotion without a delta that beats its
arm's four-run spread, and no probe that cannot exhibit the effect it is
testing — the two vacuous shapes `AGENT.md` records. A probe that comes
back inside the spread closes the finding permanently.

| # | finding | probe | the correction already known |
|---|---|---|---|
| 1 | `place` has no relayout boundary | the new `place` row, with a size-unchanged term added to the guard | the review's guard was incomplete; the 7.6 µs it cited is an arena figure |
| 2 | dispatch writes hover/press/focus over every node per event | the new dispatch row | the array is `NODE_COUNT`, not rows; `docs/results-competitive.md` already prices dispatch at 0.03–0.04 µs, so expect little |
| 3 | `record_painted` re-stamps ten style-derived fields | stamp the static ones where the cascade runs | nine fields, not ten — `focus_ring` is rewritten per frame by the host |
| 4 | paint culling stops at the arena boundary | a long scrolled column of derived containers | T16.12's 8× is `List`'s arena loop; do not borrow it |
| 5 | the generated scans copy a whole `Style` per child | codegen diff on `--bin fusion` | plausibly zero; `Style` is `Copy` and LLVM may elide it |
| 6 | `Seed::defaults` is a fn pointer per node | an A/B against a const default style value | T24.1's 2.43× is the template, not the prediction |
| 7 | sorting the sheet by specificity once | compare against T26.6, which may already capture it | mostly subsumed by T26.6 |
| 8 | `cascade_subtree`'s debug check runs a full cascade | an exact diff-based check | debug-only: the win is test-loop time, not frame time |
| 9 | a spliced const list of which table indices own arenas, so the three deep walks stop asking every node | `--bin cascade` with the deep walks narrowed | — |
| 10 | `lint_sheet_deep` re-walks every arena per rule | the new lint row | half (b) allocates and must not ship |
| 11 | ids and classes compared by content | the 24-Id-rule arm | a `wid_hash` costs `NodeDesc` bytes the embedded fixture asserts |
| 12 | a measurer epoch for scale changes | a host test with scale and resize in one frame | the failure needs an event ordering nobody has produced |
| 13 | `FontStack::prepare` re-scans every character | a coverage-bitmap A/B | it is exactly two scans in the steady state, not one to three |
| 14 | `App::update`'s blast radius | generated per-field offsets behind a dotted mark | the 7.6×/293× cited are `Boundaries`-vs-`Everything`, a different comparison |
| 15 | a shaped-run cache in the canvas | `just profile` on the tally frame | buys nothing on all-new text; the 7,315→158 µs figure is the *culling* win |
| 16 | index newtypes separating main-table from template-relative indices | — | the win was attributed to F1.8, which was a stale index, not a space confusion |
| 17 | `SEEDS.len()` asserts for hand-written impls | folded into T26.16 | the derive half is unfalsifiable |
| 18 | duplicate `#[widget_id]` | folded into T26.16 | ambiguous only for addressing, never for styling |

Three of these (7, 17, 18) are duplicates of tasks above and close with
them; two (2, 14) are the same finding seen from two subsystems.

---

## 4. What this is not

**Not a rewrite of the cascade.** T26.6 and the probes under it change
when work happens, never what a rule resolves to.

**Not a second attempt at appearance on the widget.** M25 answered that
twice, the second time with the look cached (`docs/plan-appearance.md`
T25.12).

**Not a GPU milestone.** T26.12 restores M18's regions on a backend that
already exists; nothing here changes what the GPU can draw.

## 5. Deferred, with conditions

- **A compile-time sheet lint.** Refuted for now: the arena half cannot
  be const, so a const check could only assert a weaker property than the
  runtime lint, and would contradict `docs/plan-appearance.md` T25.2.
- **Typed logical/physical pixels.** The interior cannot carry them —
  `Event` is `no_std` core and size-asserted — so only the shell seam is
  typeable. When a second backend arrives.
- **A cross-crate id registry.** Interning ids as types needs a registry
  crate and therefore nightly or unsafe; the component-boundary decision
  (`docs/00-handoff-static-tree.md` §9) also wants ids to stay strings.

## 6. What the review found, and what its verifiers corrected

14 agents, 7 subsystems, 42 findings: **20 confirmed, 18 plausible, 4
refuted.** The verifiers' corrections are the part worth keeping:

| corrected | from | to |
|---|---|---|
| `place`'s cost | 7.6 µs "per idle frame" | an arena `List::place` figure; the table walk is unmeasured |
| dispatch's per-event cost | "~760 KB per mouse move" | ~100× smaller: the array is `NODE_COUNT`, not rows |
| the GPU region win | "141×" | a software damaged-pixel ratio; the GPU ceiling is 1.10 ms |
| a retention slot | 20 B (from `retain.rs`'s own comment) | 24 B, measured |
| `Measured` | 24 B | 28 B, measured |
| the amplification win | 37× | the wrong quantity: that pair is changed-width versus unchanged-width |
| `record_painted` | ten static fields | nine; `focus_ring` is per frame |
| the sheet-lint const check | a compile-time W1010 | impossible for the arena half; **refuted** |
| index newtypes | "would have caught F1.8" | F1.8 was a stale index; **refuted** |

The four refuted findings are in §5 or dropped: the const sheet lint, a
defaulted associated const on `Widget` carrying the table-shape proof
(the form proposed does not work on stable), the index newtypes, and
typed logical/physical pixels.
