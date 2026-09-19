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

1. **Measure before and after, on the arm the task names** — and because
   M25/T25.12 found this arm family **bimodal**, not outlier-contaminated
   (`docs/results-appearance.md` T25.12: every build showed a second mode
   about 1.7 µs up on `still` and `hover`), the rule is a mode rule, not
   an outlier rule. Eight runs, not four. Classify each into the fast or
   the slow cluster before any spread is computed, print the count in each
   and the spread *within* the fast cluster, and compare like with like. A
   delta inside the fast cluster's spread is "no effect", never a win. A
   task whose runs land in one mode only says so.
2. **State the quantity.** A figure from `docs/results-*.md` may only be
   cited for the case it was measured on. The review's own findings cited
   an arena `place` figure for a table walk, a software damage ratio for a
   GPU change, and a 10 000-*row* state array that is 11 000 *nodes*; all
   three were corrected by verification, not by review.
3. **Guard the budgets that do not appear in a frame number**:
   `scripts/check_text.sh` (`.text`), the struct-size asserts in
   `fixtures/embedded`, rustc's time, and the allocation gate.
   **`just const-budget` is a ceiling, not a threshold**: it fails only
   past 5 000 ms against a fixture that builds in a fraction of a second,
   so a Phase C task is judged against the millisecond figure the recipe
   *prints*, recorded in the results doc with its own spread — a "+10%"
   read against the 5 000 ms ceiling would be a bound computed from the
   wrong quantity, which is the shape this milestone exists to stop
   repeating.
4. **Docs move in the same commit**, including the decision record the
   task contradicts, and **each task's Docs line names the
   `docs/00-handoff-static-tree.md` section it touches** — §2's relayout
   boundary for T26.1 and Phase D #1, §2's damage rects for T26.11 and
   T26.12, §10's zero-allocation claim for T26.13. Several tasks here
   contradict something recorded as decided; each says which, and T26.15
   reverses a rationale written in rustdoc and taught in the book.

**Regression guard.** `just test` per task; `just check` (which adds
`thumb`, `gpu`, `simd`, `threads`, `floats`, `book`) before each phase
closes.

## T26.0 — baselines and thresholds, before anything changes — **DONE 2026-09-17**

> **Six arms added, and two of them were wrong before they were right.**
> The `Badge`/`Chip` rows first ran against the stand-in measurer and
> printed 4.2 ns — a widget that reshapes every layout, priced as though
> it had a memo — because under `BlockMetrics` a measurement is
> `char_count × advance`. Rebuilt against a real face: **412 ns**, beside
> a memo hit's 5.4. The showcase popup arm reported the *settled* frame as
> 100% damaged too, so it could not tell a still frame from a popup-open
> one; it was measuring the banked seed frame, and now banks first.
>
> **Every arm was then shown to move under its own injection**, and the
> plan's own proposed injection for two of them was wrong: deleting the
> relayout boundary cannot move the `place` row (the boundary is in
> `layout`), and "force `Damage::All`" is a no-op at baseline. Both were
> replaced. Results: place 1.01 → 0.01 µs, dispatch 1.11 → 0.03 µs, lint
> 183,460 → 42,816 ns, `either45` 210.9 → 203.7 ns, Badge 412.6 → 4.4 ns
> with Chip unmoved.
>
> **Two findings fall out of the baselines themselves.** The `Either`
> injection moves only ~7 ns of a 211 ns frame, which **bounds Phase D's
> "cache the hide" probe at ~3%** before it is built. And the sheet lint
> is the largest number measured here — 183 µs against the 66 µs cascade
> over the same sheet — which is T26.17 #10's whole case.
>
> **The mode rule earned itself immediately:** one of eight runs came back
> slow on three invalidation rows while `dispatch` in that same run did
> not. Numbers in `docs/results-frame-and-types.md` T26.0.

No behaviour change. Records what every later task is judged against, in
`docs/results-frame-and-types.md`, and **adds the arms that do not exist
yet** — the review could not price five of its findings because nothing
measures them:

| arm | what it must newly cover |
|---|---|
| `benches --bin invalidation` | a `place` row (it is layout-only today, and says so), and a dispatch row that **names its entry point**: `pass::dispatch` alone prices the per-node sweeps, `Host::pump` after the event is what the `Everything`/`Boundaries` split applies to; the row measures the first and reports ns/node |
| `benches --bin cascade` | a sheet-lint row, on the 27-rule showcase-shaped sheet it already builds |
| `benches --bin crossover` | an `Either` row, so a hidden branch's cost is visible |
| `benches --bin repeater` | rows of a text leaf, so a memo's absence shows |
| a showcase profile recipe | `just profile` is fixed to counter-window's headless tally and takes no arguments, so this is a **new** recipe over the showcase's headless path, printing the damaged area beside the window area on a popup-open frame — the case T26.11 exists for |
| `benches --bin bindings` | a `Badge` and a `Chip` row, so T26.8's two unmemoized widgets are measured where the memo is measured |

**Gate — a new arm must be able to say no**, and the injection must break
what *that* arm measures. Deleting the relayout boundary moves the two
layout rows and **cannot move the place row**: the boundary lives in the
generated `layout`, and `place` receives no context and takes no skip
(review 2026-09-17, confirmed). So:

| arm | injection that must move it |
|---|---|
| `place` row | stop recursing into children in the generated `place` — the row collapses |
| dispatch row | make the hover and focus publishes touch one node instead of sweeping — the row drops by the per-node term |
| lint row | return early from the arena walk — the row collapses |
| `Either` row | stop zeroing the hidden branch — the row drops |
| `Badge`/`Chip` rows | **not** "bypass the memo" — these widgets have none, which is the point. Short-circuit the layout to return a stored size and the row must *fall* to the memo-hit cost; a row that does not move was measuring the stand-in measurer rather than shaping, which is what the first version of this arm did (`docs/bindings-decision.md`: the stand-in cannot answer what a memo is worth, so these rows run against a real face) |
| popup frame | **not** "force `Damage::All`" — a length change already answers `Damage::All` today, so that injection is a no-op at baseline and would be the "both sides agree about a frame in which nothing happened" shape. The arm carries its own control instead: it prints the **settled** frame's damaged area beside the popup frame's, so a run in which both are the window is an arm that cannot distinguish them and is fixed before use. After T26.11, the injection is: make the alignment damage the union rather than the middle — the damaged area jumps back to the window's |

An arm whose number does not move under its own injection is reported and
fixed before the baseline is taken from it, not after.

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

### T26.1 — the enum derive zeroes the active variant before laying it out — **DONE 2026-09-17**

> **One defect, nine sites correct by construction.** The task's own
> inventory was the point: of the ten hide call sites, only the enum
> derive applied the mechanism to the subtree it was *about to lay out*.
> `Option` hides only on `None`, `Either` hides only the side it is not
> showing, `Popup` only when closed — none of them can hand a live
> subtree a zero. That is now stated from the code rather than assumed,
> and it is why the fix is one macro change.
>
> **Test first, and it failed its own setup twice before it failed for
> the right reason.** The first version read the wrong node — arithmetic
> off the enum lands on the pane's child, then on the *hidden* variant —
> and its "the marked sibling relaid out" guard fired, which is the guard
> working. Addressed by `#[widget_id]` instead, the stage collapsed to
> `0×0` against the sheet's `40×20`, on `Boundaries<16>`, while the same
> session under `Everything` stayed green.
>
> **The first implementation cost 25× what the fix needs**, and only a
> code-size measurement found it. Emitting the two hides *inside each
> match arm* put 36 inlined copies with constant loop counts into an
> 18-variant enum: **+13,632 B** of `examples/showcase`'s `.text`.
> Computing the active variant's range once and hiding around it —
> two call sites, not two per arm — is **+544 B** for identical
> behaviour, with both gates unchanged. Landed 2026-09-17, right after
> the fix it replaces (`docs/results-frame-and-types.md` T26.1).
>
> **Both gates were shown to fail.** Restoring the whole-span hide fired
> `enum_relayout`'s assertion by name (left `(0.0, 0.0)`, right
> `(40.0, 20.0)`), and — run separately, because cargo stops at the first
> failing binary — the oracle fired too: *"step 2: the display lists
> differ"*. Step 2 is the scoped restyle, exactly the frame where the enum
> is reached with its own subtree clean. The oracle could not have caught
> this before: its fixture held no enum, no `Option` and no `Either`, so
> its byte-identical claim was over trees with nothing hidden in them.

`crates/lumen-static-macros/src/lib.rs:1197` calls `hide_subtree` over
`NODE_COUNT - 1` — every variant's slots, the active one included — and
then lays the active variant out. Under `Everything` layout rewrites it
the same frame. Under `Boundaries` a clean active subtree returns before
`record` runs and **keeps the zeroed geometry**: the stage of
`examples/showcase` disappears on any frame where something else is
marked. The const `STARTS` array the derive already emits names exactly
the ranges that should be hidden.

**The defect is the mechanism, not the derive** (review 2026-09-17,
confirmed). It is: *a subtree zeroed with the same `limits` its owner will
later lay it out under, in front of a clean-skip guard that keys on
`st[i].constraints == limits`*. The guard is exact —
`can_skip() && is_clean(i..i + NODE_COUNT) && st[i].constraints == limits`
returns `st[i].size` — and `ops::hide_subtree` stamps both halves of what
it tests: `size = default()` **and** `constraints = limits`.
`hatch::hide` is a byte-for-byte twin of it.

**Ten call sites, in two classes** (enumerated 2026-09-17, T26.0). The
task classifies every one; it does not fix only the one the review
happened to name:

| class | sites | why it matters |
|---|---|---|
| **runtime state** — flips without a stylesheet | the enum derive (`macros` :1197), `Either`'s two branches, `Option`'s `None`, `Popup` (`!self.open`), and `Select` through the popup it owns | the hide can flip between frames with nothing marking it |
| **stylesheet-driven** — `!visible` | the derived container (`macros` :757), `Glass`, `ScrollView`, `Image`, `TextInput`, `TextArea` | a visibility flip goes through `set_stylesheet`/`restyle_subtree`, which mark — so the guard should be unreachable, and the task **shows** that rather than assuming it |

**Reachability is the other half, and it narrows the claim.** `pump_with`
marks everything when an event is `handled`, and again when
`App::update` reports a change — so a popup opened by a *click* is
marked and lays out. What survives is a hide flipped with neither: a
programmatic mutation between frames, which is what `Host::invalidate`
exists for and what an application driving its own state does. The fix
removes the hazard whether or not a given caller reaches it.

**Fix:** hide the ranges before and after the active variant, not the
whole span; and for each other hider, either show it cannot reach the
guard or fix it the same way.

**Gate:** an enum tree on `Boundaries<16>`; settle; mark an unrelated
node; pump. **Assert the setup first** — the enum's `layout` ran this
frame — because if the enum sits under a container that is itself clean,
the derive skips it and both sides agree about a frame in which nothing
happened, which is the vacuous shape `AGENT.md` names. Then: the active
variant's size is unchanged and non-zero. One case per hider — the enum
switch, `Either::show`, `Option` `None`→`Some` — under both strategies,
because the bug is invisible under one of them. *Injection:* restore the
whole-span hide — the `Boundaries` case fires, the `Everything` case
stays green, which is the shape of the defect.

**Docs:** `docs/plan-invalidation.md` T16.7 — its gate is
"byte-identical display lists over a driven session" and could not fail
on this, because `invalidation_oracle.rs`'s fixture holds no enum and no
`Option`; the fixture gains one in this commit. Also T16.9's DONE note,
`docs/plan-select.md` T24.3, and `docs/00-handoff-static-tree.md` §2
property 1 — the relayout boundary itself, whose own entry is a
cautionary twin of this defect: it stood marked DECIDED for four days
naming two constants that never existed, because the identifier gate did
not read markdown then.

### T26.2 — the agent reports a hidden variant's kind as its container's — **DONE 2026-09-17**

> **Six reporting sites moved to the seeds; `live_nodes` kept on the
> instance, and both halves shown to fail.** Reverting the summary site
> made `#d1` — a `Label` inside the inactive variant — report **`Stage`**,
> the enum container's kind. Dropping the kind from `live_nodes`'
> identity fired *"setup: a visible node was reported hidden"*, which is
> the exemption earning its comment rather than asserting it: a stand-in
> and the node it stands in for share an address, so the kind is what
> tells them apart.
>
> **The gate was aimed at the wrong method twice**, and the second miss
> is the useful one. `ui.getTree` reports `role`, `wid`, geometry and
> **no kind at all**, so a kind assertion against it reads `null` for
> every node and catches nothing; the gate now uses `ui.resolve`, which
> goes through the changed site. Before that it had no `pass::layout` at
> all and reported `<missing>` for every id — its own setup assertion
> catching it, twice, before the code was touched.
>
> **Residue, named rather than fixed:** `role` still comes from the
> instance, so a hidden node reports `GenericContainer` while its `kind`
> is now right — visible in the same JSON response. `Seed` carries kind
> and defaults, not roles, so there is nothing static to read; closing it
> means a role on the seed or an accessibility pass that walks types.
> Deferred to §5.

`TreeRef`/`TreeMut` read kinds from the live tree, so every node of an
inactive variant answers with the container's kind — in `ui.find`, the
tree dump, the journal summary and `W1001`. This is the defect M24/T24.1
fixed in the cascade, in the surface next door.

**Fix:** carry `W::SEEDS` on `TreeRef`/`TreeMut` and read `kind` from the
seed in every **reporting** path — the tree dump, `explain`, `find`'s
kind term, the available-kinds list.

**`live_nodes` is exempt, and this is the task's trap** (review
2026-09-17, confirmed). It identifies a hidden stand-in by *address and
kind together*, and its own doc records that dropping the kind breaks
`changes.rs`: "load-bearing rather than defensive". Seed kinds are static
per index and identical for a stand-in and the slot it stands in for, so
a seed-built identity cannot tell them apart. It keeps reading the
instance.

**Gate:** a tree with an inactive variant; the agent reports that node's
own kind, **and** `live_nodes` still reports those nodes as stand-ins.
*Injection (a):* read the kind from the instance in a reporting path —
fires, naming the node. *Injection (b):* build `live_nodes`' identity
from the seed — the stand-in assertion fires by node index.

**Docs:** `docs/plan-select.md` §1 and T24.0–T24.1 ("a fact that cannot
vary per instance, reachable only through an instance, is stored in the
wrong place" — this is the last place it still is).

### T26.3 — the text memo's key omits font size and weight — **DONE 2026-09-17**

> **`MeasureKey`, whose only constructor takes the whole `Style`, now
> keys both memos in `ops.rs`.** `Label`'s `Measured` holds it beside the
> text's dirty bit. `Button` holds it beside a fold of its label, and its
> private `ButtonKey` — the right key, sitting in the same file as the
> wrong one — is deleted.
>
> **Test first, and red for the right reason.** Against a measurer whose
> width depends on weight (the stand-in ignores weight, so it could not
> have exhibited that half), a label measured 40 at 16 px and **still 40
> at 32 px**, and 40 at weight 400 and still 40 at 800. After the fix: 80
> and 80. The good-input direction held throughout: five identical
> layouts make one measure call.
>
> **Every gate shown to fail, and only its own.** Dropping the size from
> the key fired the size test and the button test, but not the weight
> test. Dropping the weight fired only the weight test. Making `Button`
> compare its fold but not the key fired only the button test, which is
> new: `Button` was already right, and that test guards the move.
>
> **One claim in this task was not carried out, and should not have been
> written.** It said the two probes would "take the same key type" so that
> matching `layout`'s measurement was enforced by construction. A probe
> memoizes nothing, so a key there would compare against nothing and
> enforce nothing. Both probes read `style[i]` as `layout` does, which is
> what actually keeps them honest, and they are unchanged.
>
> **Cost, measured in this session against a binary built before the
> change** (`docs/results-frame-and-types.md` T26.3): memo hits not slower;
> a stand-in miss about +0.3 ns and a real-face miss unchanged at ~415 ns;
> `Label<String>` 64 → 72 B on the host; the embedded `Ui` 1,136 →
> **1,144 B**, re-asserted; `.text` +32 B. `measure_is_warm` now takes
> the style it is asked about. That changed **15** test call sites, not
> the 18 first counted, which included the definition.

`Measured` keys on constraints and a dirty flag, so a restyle that
changes only the size or weight of a run reads a stale box. Nothing in
the tree trips it today — `examples/counter-window` keeps the same
`font_size` in all three sign arms — which is why it is a defect found by
reading rather than by a frame.

**The defect is in `Measured`, in `text.rs`, and its one caller is
`Label::layout` in `ops.rs`** (review 2026-09-17, confirmed — the first
draft of this task named `input.rs` and `textarea.rs`, whose keys already
carry both terms, and a "thirteen call sites" count that matches
nothing). `Measured` keys on the constraints and a caller-passed dirty
bit; `Label::layout` reads the size and weight *inside the closure*, so
they never enter the key.

**Fix: generalise the key this file already gets right.** `Button` keys
its memo on `limits`, size, weight **and** a fold of the label's bytes,
and has done since the frame-cost round; `Measured`, which `Label` uses,
keys on the constraints plus a caller-passed dirty bit and drops the
rest. So the right answer and the wrong one are in the same file. Widen
`Measured` to `Button`'s discipline — a key type whose only constructor
takes the whole `Style`, so a caller cannot build one that omits a field
— and let `Button` use it too, deleting the duplicate.

**Both `probe`s stay unmemoized**, and that is not an oversight to fix:
a probe is the audit asking what a node *wants* to be, and it must
neither read nor dirty the frame's memo. They take the same key type only
so that "the same measurement `layout` makes" is enforced by construction
rather than by a comment — which is what went wrong when a probe measured
at 16 px against a 76 px label.

**Gate:** restyle changing only `font_size` under unchanged constraints
→ the measured box changes. *Injection:* drop the size term from the key
— fires. **Record the struct size**: `Measured` is 28 B today, measured,
not 24 as the review first claimed.

**Docs:** `docs/layout-and-storage-decisions.md` §1,
`docs/text-and-scrolling-decisions.md` B2.

### T26.4 — `Assembled::default_styles` seeds from a live probe — **DONE 2026-09-17**

> **Two lines, and a fixture that had to be built to see it.** No
> `Assembled` in the tree contains an enum — `benches`' `Item`, `Card` and
> `Toggles` are flat — so the test file supplies one: a row whose enum
> shows a pane and hides a checkbox. Before the fix the hidden checkbox
> was seeded `FillPortion(1)`, the enum container's width, instead of its
> own `Fixed(24.0)`; after, both instances of the slab carry the
> checkbox's own defaults. *Injection:* restore the probe — fires by
> instance number.

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

**Fix: the counter is widened, not capped** — and that is the *other*
branch of what this task first considered, taken deliberately when the
failure turned out to be worse than "a large `N` is refused".

> **DONE 2026-09-17.** `len` is a `usize`; `Boundaries::is_clean` asks
> `self.view()`, so one implementation answers for both.
>
> **Why not the const assert this section asked for.** A cap makes
> `Boundaries<300>` fail to compile; widening makes it *work*. The
> counter is one value per `Host`, not per node, so the width costs
> nothing the per-node budget cares about — and refusing a set size the
> machinery can hold perfectly well would have been gating a limitation
> rather than fixing a defect. With the wrap gone there is nothing left
> for a compile-fail case to refuse, so none is added; Phase C's gates
> still use the trybuild suite, which does exist.
>
> **What the failure actually was**, and why it deserved the stronger
> fix: at 256 marks the length wrapped to zero, and a set reporting zero
> ranges with `all == false` *is* a still frame — so every subtree
> answered clean and a frame that had marked 256 subtrees skipped all of
> them. Silent and total.
>
> **Gates, both shown to fail:** a 300-range set counts 300 and answers
> correctly — *injection:* restore the `u8` and it reports **44**, which
> is 300 truncated. And the existing agreement test, with a second
> injection: give `Boundaries` its own divergent `is_clean` and it fires,
> along with three other range tests.

**Docs:** `docs/plan-invalidation.md` T16.7 — which describes
`Boundaries<const N: usize>` and never named the `u8`, so nothing there
was stale; the claim lived only in the field's own comment, which this
replaces.

---

## Phase B — the confirmed optimizations (T26.6–T26.13)

### T26.6 — the cascade's per-node work that is a property of the sheet — **DONE 2026-09-19**

Walk the sheet once before the node loop and record which specificity
levels it actually has; skip the passes for levels that are absent.

**The number, settled.** The review reported this twice with different
figures — 2.2× in one agent's run and 54,399 → 40,739 ns (1.33×) in
another's. Neither was citable and neither is right: **1.56×** on the
showcase-shaped 27-rule sheet and 1.12× on the 4-rule one, eight runs a
side under the mode rule (`docs/results-cascade-cost.md` T26.6). The
gap between the two arms is what a saving proportional to the rule
count looks like, which is probably how one run produced 2.2× and
another 1.33×.

**Gate:** ~~resolved styles are identical to the naive cascade~~ — the
**ordering**, asserted directly. A differential test against a naive
copy of the loop would pass under an injection that broke both sides,
because both sides would be the same code; the plan's own gate line was
that test. `crates/lumen-static/tests/cascade_levels.rs` asserts what
the four passes exist for instead: the most specific rule that matches
a node wins, whichever levels the sheet contains, with source order
within a level, a sheet missing a level, and an empty sheet. *Three
injections* (DX–DZ): a present level marked absent, the passes run in
reverse, a level skipped unconditionally — all three redden.

**Measure:** `--bin cascade`, three rows. **Not a frame win** — the
cascade runs on sheet, palette or shape change, which `style.rs` states
as its contract.

**Docs:** `docs/results-cascade-cost.md` gains the row.

### T26.7 — the text widgets measure what shaping already returned — **DONE 2026-09-19**

`input.rs:472` shapes behind a key guard and `:484` then calls
`measure_weighted` on the same run unconditionally, every layout.
`TextArea` does the same. The width is already in what `shape` returned.

**Fix:** widen the cached tuple to hold the run size and line height.
**+8 B per field** on the host (measured, not estimated); the embedded
`Ui` assert moves with it.

**Gate:** a counting measurer: calls per frame for unchanged text drop to
zero. *Injection:* narrow the cache back — the count rises and the test
names it.

**Measure:** ~~`just stages 10000 150 20` and `--bin repeater`'s new
text row~~ — **neither can move under this change.** `--bin repeater`
walks a repeater of `Fixed`s and `stages` a table of labels; neither
lays out a `TextInput` or a `TextArea` at all, so a figure from either
would have been a bound computed from the wrong quantity, which is this
milestone's own rule 2. The arm was added to `--bin bindings` instead,
beside the memo rows it is about and against the real face: **401 → 8.6
ns** for a field's still frame and **2,101 → 19.4 ns** for an area's
(`docs/results-frame-and-types.md` T26.7). Size, measured: `TextInput`
+8 B, `TextArea` **+0** — its `f32` landed in padding it already had.

### T26.8 — `Badge` and `Chip` shape on every layout, with no memo at all — **DONE 2026-09-19**

Five text widgets, four different memo keys, two with none.
`docs/bindings-decision.md` T7.3 priced the difference between a shape
and a compare at **4.8 ns against 2,459 ns** for a short run.

**Fix:** one helper taking T26.3's key type, used by all five, so the
memo is a property of the helper rather than a convention repeated
thirteen times.

**Gate:** shapes per layout for unchanged content is zero for every one
of the five. *Injection:* bypass the helper in one widget — fires by
widget name.

**Measure:** `--bin bindings`'s memo warm and miss rows, extended by
T26.0 with the `Badge` and `Chip` rows — the arm the 4.8 ns against
2,459 ns was measured on, so the before and after are comparable to the
figure this task cites.

### T26.9 — every derived container emits a background command it usually has none of — **DONE 2026-09-19, and it is not a frame win**

The generated `paint` **guards its shadow** and emits the fill
unconditionally, including when the background is fully transparent;
retention then hashes and retains a command the sink dropped. `hit`
already knows the predicate. One guard to add, not two.

**Gate:** a transparent container emits no body command, and retention
keeps no slot for it. *Injection:* unguard the emission — both counts
rise.

> **Half of that gate cannot be written as stated.** `DisplayList`
> already drops a fully transparent fill, so the *command* never exists
> and a display-list test sees nothing — the first version counted
> commands and stayed green under exactly this injection. The call is
> what the guard removes, and the *slot* is what it saves, because
> `Recording` hashes and observes **before** it forwards. So the gate
> counts calls with a counting `Painter` and slots with a counting
> `Retention` (`tests/container_fill.rs`), and the injection reddens all
> four tests.
>
> **And it is not a frame win**: `--bin crossover`'s `1k/static` moves
> −114 ns against its own 121 ns fast-cluster spread, which this
> milestone's rule 1 calls no effect. What it removes is one retention
> slot and one **ink rect** per transparent container per frame, priced
> at ~6.3 ns of observation each by `--bin retain` — kept for the
> embedded `Streaming<N>` profile, where overflow degrades a frame to
> `Damage::All`, and because it makes `paint` agree with `hit`.
> `docs/results-frame-and-types.md` T26.9.

**Measure:** `--bin crossover` and `--bin repeater`, which paint derived
containers. **Not `--bin retain` or `--bin paint_sink` for the frame
number** — both issue commands directly against a `Painter` rather than
painting a tree, and say so in their own headers, so neither can move
when the generated `paint` changes; `--bin retain` is re-run only as the
per-command retention price. Also shortens the stream for `Streaming<N>`
on the embedded profile, where overflowing `N` degrades a frame to
`Damage::All`.

**Ordering:** this makes the retained stream's *length* depend on a
node's background alpha, which `retain.rs` still answers with
`Damage::All` until T26.11 lands. Say so in the commit, and re-run
`--bin retain` after T26.11.

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

**Three commits, not one** — the arena signature change and its call
sites; the island capturing its seeds, with the cascade fallback deleted
and the lint's hole closed; the dead fields removed. A single commit here
is the milestone-sized shape `AGENT.md` measured at zero-of-eleven doc
compliance.

**Docs:** contradicts `docs/plan-select.md` T24.1's "one case still reads
an instance, deliberately" — that case is the one being deleted — and
changes the signature `docs/plan-review-fixes.md` F4.1 specifies.

### T26.11 — a command-count change is `Damage::All` — **DONE 2026-09-19**

`retain.rs:571-577`: a length change damages the window. Every popup
open and close and every enum-variant switch therefore repaints
everything, on the strategy the desktop shell defaults to.

**Fix:** two slot buffers instead of one, with `finish` aligning them
when the lengths differ — longest common prefix and suffix by hash,
damage the middle. **A slot is 24 B** — measured; `retain.rs`'s own
comment says `N * 20` and derives "`Streaming<256>` is 5 KB" from it, and
both are wrong (24 B, 6 KB). Fix the comment, and record what this task
then does to it: doubling the array makes `Streaming<256>` 12 KB against
the M13 device's 128 KB. **`Streaming<N>` does not get the second
buffer.** It would go from 6,680 B to 13 KB — a tenth of the M13
device's RAM — for a strategy whose whole purpose is a bounded,
allocation-free footprint, and an embedded frame is both simpler and
rarer to restructure. A length change still answers `Damage::All`
there. Both numbers in the comment were corrected from `size_of`: a
slot is **24 B**, not 20, and `Streaming<256>` is **6,680 B**, not 5 KB
— so the doubling this task considered would have been 13 KB, not 12.

**Gate:** a popup-open frame damages the popup's rect, not the window,
on `Incremental` + `Boundaries<16>`. **The good-input half is stated as
area, not as a variant** — after this change a frame in which every
command differs reports regions covering the viewport, not necessarily
`Damage::All`, so assert the covered area against the viewport's and
print both. *Injection:* force `All` on a length change — the first
assertion fires; *second injection:* align on length alone without
comparing hashes — the covered-area assertion fires.

**Measured:** `--bin retain`'s `streaming: build + Incremental` arm
**328 → 295 ns** — two buffers are *faster* than one, because the hash
comparison left the per-command path for a tight zip in `finish`; and
`just profile-showcase` **100.0% → 84.0%** of the window on the real
dropdown, against **37.5×** on the synthetic arm. The gap between the
two is honest: opening the showcase's dropdown also changes the
trigger's label and re-lays the stage.

**Docs:** `docs/plan-invalidation.md` T16.8 is **not** reopened, and the
first draft of this task said it was (review 2026-09-17, confirmed).
T16.8 is *paint* skipping clean subtrees, and its blocker is correctness
— "paint cannot skip on cleanliness" — not storage; this task changes how
damage is computed and gives paint no new knowledge. What moves:
`retain.rs`'s slot comment, `docs/damage-amplification-decision.md`, and
`docs/00-handoff-static-tree.md` §2's damage-rects property.

### T26.12 — the GPU backend collapses M18's region set to its bounding box — **DONE 2026-09-19, and Phase B with it**

`Prefer::Auto` picks the GPU first, and the GPU takes the bounding box of
the damage regions, which is the merging cost M18 removed.

**Fix:** `Gpu::draw` takes `&[Rect]`, at most `retain::MAX_REGIONS`, and
scissors per region.

**Gate, and where it runs.** `just test` excludes the wgpu crate, and
`just gpu-parity` is in neither `test` nor `check` — so a gate written
only there runs on nobody's commit. Assert the region set **where the
shell hands it over**, in a test `just test` runs: the backend is given
one rect per region, not their union. Pixel comparison stays in the
adapter-gated run. *Injection:* collapse to the bounding box — the
handed-over set drops to one rect and the assertion fires, while parity
stays green, which is why parity alone could not catch this.

**Also say what happens to the page clear and to overlapping regions** —
per-region clearing is the part a bounding box made trivial.

**Measure:** a headless GPU arm — `Gpu::headless` exists and
`benches-competitive` already drives a damaged-region GPU timing — not
`just profile`, which is fixed to counter-window's headless tally, takes
no arguments, and has no `gpu` feature to pass (review 2026-09-17,
confirmed).

**Ceiling, stated first, and on the right arm:** the frames this changes
are damage-clipped ones, where `docs/results-gpu.md` §5b shows the GPU
*losing* at small damaged areas; §1's 1.10 ms full-surface 4K redraw
bounds the win from above but is not the case being improved. The 141× in
`docs/damage-amplification-decision.md` is a *software* damaged-pixel
ratio and must not be restated as a GPU speed-up.

### T26.13 — the raster path allocates per strip and per glass command — **DONE 2026-09-19, in three commits**

The repository's stated invariant is zero steady-state allocation, and
`crates/lumen-static-shell/tests/alloc_gate.rs` cannot see the raster
path.

**Fix, three independent parts — and therefore not one commit:** a fixed
clip stack with a depth limit (the shape `InputQueue` and `Overlays`
already use, and the task states what happens when it overflows), a
reused glass scratch buffer, and the capture accumulator's write-then-read
removed. The last is worst in the σ range `docs/results-glass.md` §1
prices as expensive — below about 5, where the search picks a single pass
and all three run at full resolution. §1 names a range, not a preset.

**Gate — the gate first.** Extend the allocation gate to cover a raster
frame; *injection:* restore one `Vec` per strip and the gate fires. Only
then change the code.

**Measure:** `just profile`, `--bin glass-cost`.

**Docs:** `docs/results-allocation.md` gains the blind spot it never
recorded — its gate measures `Host::pump`, not a raster frame, so the
raster path was outside every zero in its table — and the row that closes
it. Also `docs/00-handoff-static-tree.md` §10's zero-allocation claim.

---

### T26.18 — the sheet lint is agent-gated, not optimised — **DONE 2026-09-17**

> Asked what the 183 µs lint should cost, the answer turned out not to be
> "less". It is a **development diagnostic whose only consumer is the
> agent** — `Host::sheet_findings` feeds `TreeRef::with_sheet`, which
> feeds `ui.lint`'s `W1010` — and `Host::set_stylesheet` ran it in every
> build, so an application that never attaches an agent paid it on every
> sheet swap for findings nothing could read.
>
> **`lumen-static-shell` gains an `agent` feature**, default-off, which
> `lumen-static-winit`'s existing `agent` feature turns on. Without it the
> lint does not run and the findings are not stored; the sheet itself is
> still kept, because `Host::set_palette` re-cascades it (M25/T25.11) for
> reasons that have nothing to do with the agent.
>
> **Measured, not asserted:** `examples/minimal` — a window with no agent
> — drops from 2,618,470 to **2,615,910 B** of `.text`, and
> `lint_sheet_deep` goes from **one linked symbol to zero**. In
> `examples/counter`, which enables the feature, it is still linked. The
> 183 µs per sheet swap goes with it.
>
> **What this does to T26.17 #10.** Optimising the lint's arena walk is
> now moot for every build that does not ask for it, and for the builds
> that do it is a development cost, not a user's. The probe stays in the
> table but its urgency is gone; the two hazards it carried — an
> under-reporting const list, and a second walk that allocates — are
> reasons to leave it alone unless an agent session is measurably slow.
>
> Nothing in the core is gated: `style::lint_sheet_deep` is generic, so a
> build that never calls it never instantiates it — which is exactly what
> the symbol count shows.

## Phase C — the compile-time guarantees (T26.14–T26.16)

These buy no frame time. They are judged on **what a real defect in this
repository's history would have been prevented**, and on rustc's time,
which `just const-budget` gates.

### T26.14 — `index_of_wid` as a `const fn` — **DONE 2026-09-19**

A typo'd or renamed `#[widget_id]` marks **nothing**: the lookup returns
`None` — it can never return a wrong index — and the frame quietly fails
to redraw. A const lookup makes it a compile error.

**Honest scope.** It is silent only for a caller that ignores the answer:
`Host::invalidate` and `Host::restyle_subtree` have returned a
`#[must_use] bool` since T16.7 for exactly this reason, and every in-tree
caller asserts it. So this buys little against the current tree and
everything against the next caller and third-party code — and the first
draft of this task claimed the wrong subtree was affected, which the
lookup cannot do (review 2026-09-17, confirmed).

**Gate:** runtime and const lookups agree for every id in the tree (the
good-input half), plus a **`compile_fail` doctest** for a typo'd id —
not trybuild, which is a dependency and a committed `.stderr` that
churns with every rustc release, for a case whose whole content is
"this does not compile". `cargo test --doc` runs it, and an injection
that makes a missing id resolve to `0` turns it red.

### T26.15 — `Seed::reads` stops defaulting to `Fields::ALL` — **DONE 2026-09-19**

The default is the W1010 lint's off switch, and it is what a new widget
gets. Make the mask a parameter, with an explicit constructor for the
deliberately unlinted cases (`Seed::WIDGET` and the wrapper padding
slots).

**This reverses a decision that is written down, and the task says so.**
`Seed::reads`' own rustdoc records the default as deliberate — a kind
that has not declared its reads "is not linted rather than wrongly
linted" — and the book teaches the two-argument constructor in its
custom-widget chapter. Both move in this commit.

**It is also source-breaking for out-of-tree widgets:** the seeds array
is a required associated const, so a third-party impl cannot avoid the
constructor. **Chosen: the mask is required, and the break is
taken deliberately.** The default is the lint's off switch; an opt-out
should be spelled, and `Seed::unlinted` spells it. The break is small
where it can be counted — 53 of this repository's 57 call sites already
declared a mask, and the four that did not are `Seed::WIDGET` and three
test fixtures. `Seed::reading` is deleted rather than kept beside the
new constructor: two ways to say it is how one of them goes stale.

**Gate:** `crates/lumen-static-widgets/tests/style_reads.rs` still
verifies every declared mask — it measures each kind's output field by
field and compares — and a widget declared without one does not compile.
That half is a **`compile_fail` doctest** beside a passing one, not
trybuild: a committed `.stderr` churns with every rustc release for a
case whose whole content is an arity error.

**Docs:** `docs/plan-appearance.md` T25.2, `Seed::reads`' rustdoc, and
the book's custom-widget chapter, whose snippet and the sentence beneath
it both change.

### T26.16 — shape asserts for hand-written impls, and duplicate ids — **DONE 2026-09-19, and Phase C with it**

Two halves, one commit:
- The table and seed lengths are const facts asserted only for derived
  trees, and the hand-written generic impls write both quantities twice.
  **"The derive half is unfalsifiable" is true of exactly one of four
  cases** (review 2026-09-17, confirmed): only the struct derive's table
  length literally *is* the node count. The struct derive's seed length
  and both of the enum derive's lengths are re-spelled sums and can
  drift, so assert those too. In a *generic* impl an anonymous const
  assert cannot name the type parameter at all, so the assert goes where
  the impl already forces a const — and the task names the injection that
  proves it fires: change one helper's length by one and the build must
  fail.
- Duplicate `#[widget_id]` among one container's fields is accepted
  silently, and every id-*addressing* operation then acts on the first
  match. **Name the mechanism:** the rejection lives on the const
  addressing path T26.14 adds, not in the derive — the derive cannot tell
  an addressing use from a styling one, and `examples/glasswall` styles
  four intentional duplicates by selector while addressing none of them.
  Styling must keep matching them all.

**Gate, both directions:** addressing a duplicated id fails to compile
*and* `glasswall` still builds and still styles all four. A gate that
refuses everything is the failure mode this rule exists for. ~~Both
halves are trybuild cases with committed output~~ — the refusing half is
a **`compile_fail` doctest** (a committed `.stderr` churns with every
rustc release), and the accepting half is a runtime test asserting an
`Id` rule matches all four of a four-way duplicate, which trybuild
cannot express at all.

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
| ~~1~~ | *probed and **closed**: `place` is 7.7 µs of a 350.7 µs idle frame at 10 000 nodes (four-run spread 0.32 µs) — **2.2%** for deleting the pass outright, and a boundary wins less. `docs/results-frame-and-types.md` T26.17 batch one* | done | the review's 7.6 µs was an arena figure; the main table's agrees to two significant figures by coincidence of scale |
| ~~2~~ | *probed and **moved to §5**: the win is 1.11 → 0.03 µs an event, real but small, and it needs a clearing discipline the sweep currently supplies for free* | done | — |
| 3 | `record_painted` re-stamps ten style-derived fields | **probed: worth building, not built in M26.** Ceiling measured by stamping nothing: `layout` 35.34 → 30.04 µs, **−15.0%**, over fast-cluster spreads of 0.12/0.07 µs; buildable share ≈ 4.2 µs | **eight** static, not nine and not ten: `focus_ring` is host-written *and* `bg` is resolved here from `pressed`/`hovered`, not copied |
| ~~4~~ | *probed and **closed**: true as a fact — `clip_bounds` has one caller, `List::paint` — and worth 7.5 ns per off-surface node, so ~1 500 authored off-surface nodes are needed to reach 10 µs. The showcase paints 25 commands, **0** off-surface* | done | T16.12's 8× is `List`'s arena loop and was correctly not borrowed: a list's length is data, a derived container's is source code |
| ~~5~~ | *probed and **closed**: borrowing instead of copying moves `layout` +0.05 µs inside a 0.12 µs spread, and makes `--bin fusion`'s `.text` **288 bytes larger** (379 032 → 379 320). A planted `panic!` proved the scan live in the arm first* | done | the correction was right: `Style` is `Copy`, 80 bytes, and LLVM already elides it |
| ~~6~~ | *probed and **closed**: removing the call outright is −107 ns on a 901-node `cascade_deep` (spread 510) and **+261 ns** on the showcase sheet (spread 382). Per node that is 0.12 ns, below the bench's 0.30 ns harness floor* | done | T24.1's 2.43× is the template, correctly not borrowed |
| 7 | sorting the sheet by specificity once | compare against T26.6, which may already capture it | mostly subsumed by T26.6 |
| ~~8~~ | *probed and **closed**: the check runs **10 times in the whole workspace suite**, over trees of 15, 9 and 4 nodes — 109 nodes cascaded in total, against a 12.4 s suite* | done | debug-only was the right frame; the remaining question was frequency, and it is ten |
| 9 | a spliced const list of which table indices own arenas, so the three deep walks stop asking every node | `--bin cascade` with the deep walks narrowed | — |
| 10 | `lint_sheet_deep` re-walks every arena per rule | the new lint row | half (b) allocates and must not ship |
| 11 | ids and classes compared by content | the 24-Id-rule arm | a `wid_hash` costs `NodeDesc` bytes the embedded fixture asserts |
| 12 | a measurer epoch for scale changes | a host test with scale and resize in one frame | the failure needs an event ordering nobody has produced |
| 13 | `FontStack::prepare` re-scans every character | a coverage-bitmap A/B | it is exactly two scans in the steady state, not one to three |
| 14 | `App::update`'s blast radius | generated per-field offsets behind a dotted mark | the 7.6×/293× cited are `Boundaries`-vs-`Everything`, a different comparison |
| 15 | a shaped-run cache in the canvas | `just profile` on the tally frame | buys nothing on all-new text; the 7,315→158 µs figure is the *culling* win |
| ~~16~~ | *moved to §5: refuted, not probed* | — | the win was attributed to F1.8, which was a stale index, not a space confusion |
| 17 | `SEEDS.len()` asserts for hand-written impls | folded into T26.16 | the derive half is unfalsifiable |
| 18 | duplicate `#[widget_id]` | folded into T26.16 | ambiguous only for addressing, never for styling |

**Fourteen live probes, not eighteen.** Row 16 is refuted and belongs to
§5; rows 17 and 18 fold into T26.16; row 7 closes with T26.6; and rows 2
and 14 are one finding seen from two subsystems. The count is stated
because a table headed "the plausible set" that silently carries a
refuted row is how a closed question gets re-opened by accident.

**Batches one and two are probed: rows 1, 3, 4, 5, 6 and 8.** Five
closed on their measured ceilings, one promoted to "worth building"
with its number and explicitly not built inside M26. Eight live probes
remain: 9, 10, 11, 12, 13, 15, and rows 2/14 as one.

All five closes share a shape worth naming: **every one was true as
written.** The review read the code correctly each time — a copy per
child, a fn pointer per node, a full cascade in a debug check — and in
each case the measured ceiling was inside the arm's own spread. Reading
finds the mechanism; only measurement finds the magnitude, and this
round's yield is five mechanisms that cost nothing.

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
  crate and therefore nightly or unsafe, and
  `docs/00-handoff-static-tree.md` §9 wants the observability namespace to
  stay a stable string path that survives a reload — which a type-level id
  would replace.
- **Clearing hover, press and focus by index instead of by sweeping**
  (M26/T26.17 #2). **Measured and deferred 2026-09-17, not refused:** the
  sweep costs **1.11 µs per pointer event over 901 nodes** (1.23 ns a
  node), and the injection that clears one index instead took it to
  **0.03 µs** — so the win is real and scales with tree size, roughly
  12 µs on a 10 000-node tree.
  **What it needs first, and why it is not done here:** the sweep is
  laundering state that clearing-by-index must replace deliberately.
  `Host.input` is `pub`, so a caller that writes `input.hovered` by hand
  and pumps is corrected today and would not be. The sweep's own
  justification is "a frame may have changed the tree under us", and
  there is no `Host::swap` to clear on — so the clearing points must be
  chosen by hand, starting with `Dyn::replace` and anything else that
  changes what an index means. A node hidden while hovered would keep its
  bit: nothing paints it, but the agent reports it and it resurfaces when
  shown again. **When input latency on a large tree is measured to
  matter**, and with that clearing discipline written into the commit.
- **A hidden node's `role`** (M26/T26.2's residue). Kinds now come from
  `Widget::SEEDS`, which is per type, but `role` is still read from the
  instance — and for a node inside an inactive variant the instance is
  the container standing in for it, so the same response reports
  `kind: "fixed"` beside `role: "GenericContainer"`. `Seed` carries kind
  and defaults, not roles, so there is nothing static to read. Closing it
  means a role on the seed, or an accessibility pass that walks types
  rather than instances. When a tool reads roles off hidden nodes.
- **Index newtypes** separating main-table from template-relative
  indices. Refuted: the defect they were offered against (F1.8) was a
  *stale* index, not a confusion of two index spaces, so the newtype
  would not have caught it. When a second index space appears that is not
  structurally identical to the first.

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

## 7. What the review of *this plan* found — 2026-09-17

Seven lenses over the plan itself — facts, gates and injections,
measurement, sequencing, ADR and docs, risk and omissions, type
guarantees — each followed by a verifier told to refute. **Ten blocking
findings confirmed, twelve major, and three "blocking" claims refuted by
their own verifier.** The plan was revised in place; this is what was
wrong with it, because a plan that quietly replaces itself teaches
nothing.

| what the first draft said | what the code says |
|---|---|
| T26.3: thirteen key sites across the two text-input files | those files' keys already carry size and weight; the defect is in the label's memo, one caller, in another file — and "thirteen" matches nothing |
| T26.5: "the repository has no compile-fail harness" | it has one, with eleven failing cases and committed expected output, run by `cargo test --workspace` — and it already carries a `#[widget_id]` case. All four Phase C gates are mechanisable |
| T26.1: the defect is the enum derive | the defect is *a subtree zeroed under the same limits the clean-skip guard compares against*; `Either` satisfies it too, and nothing in the repository exercises that |
| T26.2: read every kind from the seeds | the stand-in detector identifies hidden nodes by address **and** kind, and its doc records that dropping the kind breaks the change journal |
| T26.11: T16.8 is reopened | T16.8 is paint skipping, and its blocker is correctness, not storage. This task does not touch it |
| T26.11: a slot is 24 B, the comment says 20 | and the comment derives 5 KB from the wrong figure; both move, and this task then doubles it — which the fixed-array profile must price |
| T26.9: the generated paint emits shadow and fill unconditionally | the shadow is guarded; one guard to add, not two |
| T26.9: measure on the retention and paint-sink arms | neither paints a tree, and both say so in their headers, so neither can move |
| T26.12: measure `just profile --features gpu` | that recipe is fixed to another example and has no such feature; and its gate ran where neither `just test` nor `just check` reaches |
| T26.12: the ceiling is a full 4K redraw | that is the full-surface case; this task changes damage-clipped frames, where the GPU currently loses |
| T26.14: a typo'd id invalidates the wrong subtree | the lookup returns `None`; it cannot return a wrong index. And the two shell entry points have returned a `#[must_use]` answer since T16.7 |
| T26.15: make the mask a parameter | it reverses a rationale recorded in rustdoc and taught in the book, and breaks every out-of-tree widget's seed array — neither was named |
| T26.16: the derive half is unfalsifiable | true for one of four cases; three are re-spelled sums that can drift |
| T26.0: break the relayout boundary and the place row moves | the boundary is in `layout`; `place` takes no skip, so that injection cannot move that row |
| T26.0: force `Damage::All` and the popup frame jumps | a length change already answers `Damage::All` today, so at baseline that injection is a no-op — the "both sides agree about a frame in which nothing happened" shape |
| §0: judge against a four-run spread | the arm family is bimodal; a single spread over both modes is a bound computed from the wrong quantity |
| §0: rustc time gated at +10% | the recipe fails only past 5 000 ms against a sub-second fixture, so nothing Phase C does could reach it |

**Three "blocking" findings were refuted by their verifiers** and the
plan was *not* changed for them: that the dispatch row splits on an axis
dispatch does not have (the row is fine once its entry point is named);
that T26.11's acceptance direction is unsatisfiable (it is satisfiable,
but it had to be stated as covered area rather than as a variant); and
that the const id lookup is unreachable from its callers (it is a free
function over the table, not a method on the generic host).
