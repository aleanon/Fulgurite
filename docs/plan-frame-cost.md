# M53 — what a frame costs, measured whole, and a still frame that costs nothing

> **T53.0 and T53.1 DONE 2026-09-26 (early landings; T53.0 headless
> rows only).** **T53.1:** the host times its six stages and the whole
> pump (`Host::stage_timings`); the grid, the `stages` arm and
> `flip::measure_the_flip` read it, and the gap is 0.07–0.11 µs
> everywhere. Restated in `docs/results-damage-precision.md` T53.1: one
> binding saves **26%** of the frame, not 36%; a split click **29%**,
> not 42%; and the arena cascade is **~21 µs on every grid arm**, a
> third of a narrowed frame. **T53.0:**
> `Host::pass_counts` counts cascades and passes; `arena_marks.rs`
> (clicks 40/40 cascades against a bound of 0, ignored until T53.2) and
> `idle_pump.rs` (40/40 places and paints per 40 idle pumps, ignored
> until T53.3) are red; `just grid` fails every run on a 23–30 µs gap
> the stage sum does not see; the `stages` arm fails because its host
> cascades and no stage brackets it. **The R7-F6 live gate is not
> built**: it launches through M42/T42.2, which does not exist yet.
>
> **Written 2026-09-24.** Closes R6-F1, R6-F2, R6-F3, R6-F4
> and R7-F6 from `docs/results-deep-review.md`, as re-triaged against
> `main` at `1717808` on 2026-09-24. The re-triage read the code for all
> five and measured nothing new, so every figure here is cited, not
> restated.
>
> **Phase 4** of the fixed order: after M43, whose runner and fixed
> scripts the gates here run in. It also needs **M41** before T53.3,
> because an idle fast path trusts the marks completely and M41 is what
> makes them complete, and M42 before T53.4's live gate. T53.0 and T53.1
> may land earlier, in Phase 2, for the reason given in T53.1.
>
> **R6-F2 is WORSE.** The review found that the grid probe's "frame" is
> layout + place + paint + raster and leaves out the arena cascade that
> runs on every frame of every arm, plus dispatch and finish. Since the
> review, **the same stage sum has been used for two more published
> tables** — T29.6's arms and T34.2's showcase flip — so a narrowed
> click's saving is overstated as a share in three places now, not one.
> The other four findings are the frame path itself: a click that
> touches no arena still pays the cascade (R6-F3), an idle frame under
> `Boundaries` still places, paints, hashes and finishes (R6-F1), and a
> pointer moving over nothing it changes costs one full pump and one
> whole-surface present per motion (R7-F6).

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and prints the measured quantity beside its bound.*

---

## 0. The rules for this milestone

1. **A kill rule is written before each build**, in §5, and the build is
   judged against it as written.
2. **Eight runs a side**, interleaved, for every clock figure. M26's
   Phase D found that four runs a side could not separate a real effect
   twice, and eight could (`docs/results-frame-and-types.md`).
3. **Counts gate; clocks record.** Every change here has a count that
   does not move with machine load: cascades run per click, paint passes
   per idle pump, presents per motion. The count is the committed gate.
   The clock decides whether a change ships and what the prose says, as
   M35's rule 3 put it.
4. **A published "frame" is the whole pump.** A stage column may sit
   beside it, never in place of it, and the gap between the two is
   printed and bounded (§2.1).

## 1. What is wrong, at 1717808

### 1.1 The frame the tables publish is not the frame (R6-F2, R6-F4) — WORSE

`benches/src/bin/grid.rs` prints its frame column as `l + pl + p + r`
(`:589-613`, and `:623-634` for `handled/split`) from `Host::timings3`
plus raster. `pump_with` (`crates/lumen-static-shell/src/lib.rs:993-1283`)
also runs dispatch, the drains, the bindings, and
`cascade_arenas_deep` (`:1179-1190`) before layout, and
`retain.finish()` (`:1268`) after paint. None of those are in the sum.
`docs/results-damage-precision.md:117` states one binding's cost as a
share of that sum; the review measured the gap (R6-F2;
`docs/results-deep-review.md`).

**What made it worse:** T29.6's table (`docs/results-damage-precision.md`,
"T29.6") reports every arm, including `handled/split`, as the same sum,
and the `handled/split` click still pays the cascade because the handled
branch keeps `arenas_dirty` (`lib.rs:1074`). T34.2's table (same file,
"T34.2") is headed "median frame (layout + place + paint)" and comes
from `flip::measure_the_flip` in `examples/showcase/src/main.rs`, which
sums `timings3` too. The review's own proposal for M34/T34.2 named the
missing cascade; T34.2 shipped without it.

The competitive `stages` arm (R6-F4) says stages 1–6 are "exactly what
`Host::pump_with` does, in the order it does it"
(`benches-competitive/arms/stages/src/main.rs:35`, `:319`). It builds
its host with no stylesheet (`:495`), so the cascade step does not exist
there; its only assertion is that a frame drew something (`:464`); and
the gap between its whole pump and its six-stage sum is printed and
never bounded (`:601-623`). Since the review, `pump_with` has gained the
binding marks (`update_marked`), T29.6's narrowing and T34.1's shadow
check, none of which the arm brackets.

### 1.2 A click that touches no arena still re-runs the arena cascade (R6-F3)

After T29.6 there are still four places that set `arenas_dirty`
without asking whether an arena was touched: the handled branch
(`crates/lumen-static-shell/src/lib.rs:1070-1074`, "Kept either way",
even when the mark was narrowed for a split application),
`Host::invalidate` (`:630`), an applied message (`:1154`) and a binding
that wrote `Wrote::Marked` (`:1170`). `cascade_arenas_deep`
(`crates/lumen-static/src/style.rs:1434-1442`) then asks every node in
the table for its arenas. T29.6 narrowed a split click's layout, but the
cascade sits outside the stage sum, so it is now a larger share of a
narrowed click than it was when the review priced it (R6-F3;
`docs/results-deep-review.md`, where the cascade exceeds the saving
T29.6 was built for). `docs/results-frame-and-types.md` Phase D finding 9
priced the per-node arena ask and promoted a fix that has not been
built.

### 1.3 An idle frame still does the work of a frame (R6-F1, R7-F6)

`pump_with` has no idle check. Under `Boundaries`, layout skips a clean
tree, but `place` (`lib.rs:1213`), paint through `Recording` (`:1248-1262`)
and `finish` (`:1268`) run every time. `docs/results-competitive.md`
§5b nevertheless says an idle frame under `Boundaries` "is the 0.01 µs
row" (`:388-390`), and that row (`:348`) times the layout pass alone
(R6-F1; the review measured the whole idle pump,
`docs/results-deep-review.md`). This matters more than when the review
found it, because M34/T34.2 made `Boundaries` the default for every
`Host` and every window.

In the window (R7-F6), every translated event requests a redraw
(`crates/lumen-static-winit/src/lib.rs:770-778`), `CursorMoved`
included. The redraw runs the full pump, and the surface presents on
every redraw (`crates/lumen-static-winit/src/surface.rs:117`), the GPU
backend blitting the whole target. So a pointer crossing a region it
changes nothing in costs one pump and one whole-surface present per
motion event (R7-F6; `docs/results-deep-review.md`). The comment "A
still UI idles" (`lib.rs:527-531`) is true of a still pointer only.
Nothing in M29, M34 or M37 touched this.

## 2. Decisions

### 2.1 A published frame is the whole pump, with its stages beside it

- **(a) Add the missing stages to the sum.** Every new step added to
  `pump_with` would have to be added to every probe again, which is how
  the sum went stale twice.
- **(b) Time the whole pump, and print the stages beside it.** The host
  records dispatch-to-bindings, cascade, layout, place, paint, finish
  and the whole `pump_with`. Probes read them, and each prints the stage
  sum, the pump, and the gap between them against a bound. A stage that
  nothing times shows up as gap.

**Recommended: (b).** It breaks nothing; the published shares in three
tables change, and T53.1 restates them.

### 2.2 The arena owners are a list the host keeps, not a const every widget forwards

The cascade only has work to do where an arena lives. Knowing where
needs a list of the table indices that own one.

- **(a) A const on `Widget`**, generated by the derive: Phase D finding 9's
  proposal. It has to be forwarded through every hand-written `Widget`
  impl, and Phase D judged that the risk: a wrapper that forwards it
  wrongly silently stops styling the arena beneath it.
- **(b) A list the host keeps at run time.** A full cascade already asks
  every node for its arenas, so it records the indices that answered as
  a by-product, at no extra walk. A narrowed frame then sets
  `arenas_dirty` only when a marked range contains a listed index. Any
  frame that marks everything still runs the full cascade, and so
  refreshes the list.

**Recommended: (b)**, as the re-triage recommends. It costs one
allocation per full cascade at most, and the failure Phase D named
cannot happen, because no widget declares anything. What it breaks: an
arena that **appears** inside a narrowed range — an `Option` becoming
`Some`, an enum switching to a variant that holds a `Repeat` — is not in
the list yet. §5 names that as the risk and T53.2's gate covers it.

### 2.3 An idle frame is skipped in the pump, and an empty present in the window

- **(a) Skip in the pump.** `pump_with` returns early when nothing can
  have changed what it would draw: no marks, nothing handled,
  `PointerFocus` unchanged, nothing applied, no binding wrote, no arena
  owed, and size, scale and palette unchanged since the last frame. It
  keeps the display list and reports damage `None`. **Only under a
  strategy that can skip**; `everything` stays the diagnostic that
  always walks, as M34 intended.
- **(b) Skip the present.** The window remembers whether a redraw was
  its own request or the platform's. A redraw the window asked for
  because of input, whose damage is `None`, presents nothing. Anything
  the platform asked for (an expose), and any frame whose buffer
  contents are undefined (softbuffer's age 0), still presents.
- **(c) Do not request the redraw at all.** `Session::post` would answer
  whether a motion could change anything. Cheaper, but it duplicates
  dispatch's hover and capture logic in a second place.

**Recommended: (a) and (b).** (c) is rejected for the duplication. What
(a) breaks: a widget that changes what it paints **without** a mark was
painted correctly every frame until now, because paint always ran; with
the fast path it is not. That is exactly the class of defect M41 is
removing, which is why T53.3 waits for M41 and uses M41's oracle as its
gate (§5).

## 3. Tasks

### T53.0 — the gates first

Each check is committed **red** at `1717808`, with the red output in the
commit body. Two pieces of instrumentation land here, because a count
cannot be asserted before something counts it: the host counts how many
times it ran the arena cascade and each of layout, place and paint, and
the probes time the whole `pump_with` from outside. Neither changes
behaviour.

| finding | check | red at 1717808 |
|---|---|---|
| R6-F2 | `benches --bin grid` prints `stage-sum S vs pump P, gap G (bound 5% of P)` per arm and fails when the gap exceeds the bound; it prints a cascade column | the gap is the cascade and the dispatch, on all five arms |
| R6-F4 | the `stages` arm's host gets a stylesheet with an arena rule, and asserts its `pump - sum6` gap against a stated bound, printed beside it | the cascade appears in the pump and not in the sum |
| R6-F3 | new `crates/lumen-static-shell/tests/arena_marks.rs`: a split application with a stylesheet and a `Repeat` elsewhere; 40 `Toggle` clicks print `cascades N/40 (bound 0)`; 40 wheels over the `Repeat` must give 40/40 | 40/40 on the clicks |
| R6-F1 | new `crates/lumen-static-shell/tests/idle_pump.rs`: under `Boundaries`, an idle pump runs 0 place and 0 paint passes and reports damage `None`; prints both counts | one of each per idle pump |
| R7-F6 | a live gate beside `just agent-drive` (X display or Xephyr; XTest motion, because the agent's own input path requests redraws): 200 motions over a static 40×40 px region of `examples/showcase`, under both strategies; prints `presents N for M motions (bound 2)` | about one present per motion |

**Other direction, recorded in the same commit:** 40 wheels over the
`Repeat` do cascade 40 times; a pump after `invalidate` still paints; a
motion that crosses a `Button`'s edge produces exactly one frame with
non-zero damage (the hover shade). Each shows its check can pass.

*What building found (2026-09-26, early landing).*

- **The `stages` gap cannot carry a bound.** `pump` and `sum.pump` are
  minima from two different loops on two different hosts. Given the
  sheet, the first run (load average 9, LOADED) put `pump − sum.pump`
  between **−36.93 µs and +32.07 µs**, negative on half the cells,
  where the cascade itself costs a few µs. A 10% + 1 µs bound fired on
  7 of 18 cells for noise and would have fired on a different 7 the
  next run. So the arm's R6-F4 check is **structural**: the host counts
  its arena cascades in loop C, and if it ran any, the arm's stage list must
  contain a `cascade` stage. Red on all 12 cells run (55 cascades per
  one-row and all-rows cell, 1 per idle cell — the first pump's). The
  gap stays printed and unbounded, as before. T53.1's gap bound is on
  the host's own stages against its own pump, in one iteration, which
  the grid shows is stable (below).
- **The grid's gap is stable, because it is one iteration.** Stage sum
  and pump come from the same `pump_with`: 22.6–30.9 µs on 40 of 40
  runs, against bounds of 2.8–5.1 µs.
- **The live gate (R7-F6) waits.** It launches through M42/T42.2's
  launcher, not built on this branch, so it is not written here and
  T53.0 is done for its four headless rows only.
- The instrumentation is a module, `lumen-static-shell/src/frame_cost.rs`,
  so the shell's `lib.rs` (which M41 is editing) gains five counter
  increments, one field and one accessor.

**Effort:** M. **Depends on:** M42 for the live gate's identity line
(it is launched through M42/T42.2's launcher, which also gives it exit
77 without a display); the live gate joins M43/T43.13's `just live`
list when it lands.

### T53.1 — the whole pump, timed by the host, and the shares restated

**Closes:** R6-F2, R6-F4.

**Change:** §2.1(b). The host records each stage of `pump_with` and the
whole of it, readable beside `Host::timings3`, which stays. `benches
--bin grid`, the `stages` arm and `flip::measure_the_flip` read the
host's own timings instead of re-implementing its calls, and each prints
stage sum, pump and gap. The `stages` arm's doc comments list the steps
it does not bracket. Then, eight runs a side, `docs/results-damage-precision.md`
restates T29.1's share, T29.6's table and T34.2's table as shares of the
whole pump plus raster, keeping the old columns beside the new ones and
saying which was which.

**Why it may land in Phase 2:** it changes no behaviour, only what is
measured, and M41 re-validates T34.2's evidence. Re-validated on the
stage sum, that evidence would repeat R6-F2 a fourth time.

**Gate:** T53.0's gap assertions in `grid` and `stages`.

**Injection:** force `arenas_dirty` on every pump (the review's I3): the
cascade column doubles and the pump grows by one cascade, where the old
frame column did not move at review time. Leave the cascade out of the
stage sum: the gap assertion fires, with the gap printed beside its
bound. The `stages` arm's existing `List::place` ×10 injection still
goes red, which is the accepting direction for the arms that already
worked.

**Accepts good input:** on the clean build the gap stays within its
bound on all five `grid` arms and in `stages`.

*What building found (2026-09-26).*

- **The review's I3 cannot move the grid.** "Force `arenas_dirty` on
  every pump: the cascade column doubles" assumed some arm did not
  cascade. At `0091266` all five do, every frame: `bound` and
  `handled/split` because a binding wrote or an event was handled,
  `narrowed` because `Host::invalidate` sets the flag, the other two
  for both reasons. So the injection used is the one the plan names
  second — **the cascade timed by no stage** — and it reads a 20.87–
  23.30 µs gap against bounds of 2.81–4.51 µs on 40 of 40 runs.
- **The `stages` arm's host gap cannot see that injection**, and says
  so: its `List` has one arena template whatever N is, so its cascade
  is 0.03–0.08 µs, inside the 0.25 µs floor. What catches a missing
  cascade there is T53.0's structural count, which is the arm's gate
  for R6-F4; the host gap is its gate for "a stage nothing times",
  which the grid exercises.
- **`flip::measure_the_flip` prints both figures**, so T34.2's table
  is restated rather than replaced; a tick or a click frame is more
  than twice what T34.2 printed.
- The `stages` arm's `List::place` ×10 injection was not re-run.

**Docs in the same commit:** `docs/results-damage-precision.md` T29.1,
T29.6 and T34.2; `docs/plan-damage-precision.md` T29.6 ("report the
cascade's share"); the rustdoc on `Host::timings3` and on the new
accessor; the module and `Rig::frame` docs in
`benches-competitive/arms/stages/src/main.rs`; `docs/results-competitive.md`
where it cites the `stages` arm.

**Effort:** S. **Depends on:** T53.0.

### T53.2 — the cascade runs only where an arena was touched

**Closes:** R6-F3.

**Change:** §2.2(b). `cascade_arenas_deep` records the indices that
own an arena as it walks. The four sites in §1.2 set `arenas_dirty` only
when their marked range contains a listed index; a `mark_all` frame
keeps it unconditionally. A consumed wheel over a `Repeat` is inside an
owner's range, so it keeps the cascade.

**Gate:** T53.0's `arena_marks.rs`, plus two cases the list could get
wrong: a split click whose binding turns an `Option` holding a `Repeat`
into `Some`, and an enum switched to a variant that holds one. Each must
style the new rows, checked by M41's shadow oracle over both sessions
and by reading the row's resolved style.

**Injection:** restore `self.arenas_dirty = true` in the handled branch:
40/40 against a bound of 0. Make the range check always false: the wheel
arm fires, and the oracle reports stale row styles. Skip the list's
refresh on a full cascade: the `Option` case fires.

**Accepts good input:** 40 wheels still cascade 40 times.

**Docs in the same commit:** the comments at `crates/lumen-static-shell/src/lib.rs:1070-1074`,
`:629-630` and `:1164-1170`; `docs/results-frame-and-types.md` Phase D
finding 9 (built, as a run-time list, and why not the const);
`docs/results-damage-precision.md` (the cascade's share of a click,
before and after, from T53.1's column).

**Effort:** M. **Depends on:** T53.1 (the column that measures it); M41
(its oracle, which is the check on the risky cases). **Coordinate with
M52/T52.4**, which wraps `cascade_arenas_deep`'s loop in
`cfg(feature = "alloc")`: whichever lands second rebases, and M52's
cascade-only firmware gate stays green.

*What building found (2026-09-27).*

- **The owner list fails its own risky cases, so it was not built.**
  §2.2(b) records owners during a full cascade and cascades a narrowed
  frame only when a marked range holds one. An `Option<Repeat>` that is
  `None`, and an enum on a variant without one, answer `arena_count() ==
  0` (their `node_mut` hands back the container), so the repeater that
  appears is on no list. Injected as written, both of the gate's cases go
  red: the new rows' cell is **0.0 px** against the sheet's 120.
- **What shipped instead walks the marked nodes**
  (`style::cascade_arenas_marked`, over the new `Changed::ranges`). An
  arena needs the cascade only when it is *new* — its style row is per
  template, not per row — and a new arena is a write the frame's marks
  cover, which is the completeness M41 gives layout. No list, no
  refresh, and no widget declares anything. The four sites in §1.2 are
  unchanged: they still owe the cascade, and it is paid only over what
  they marked.
- **A wheel does not need the cascade**, contrary to the comment the
  handled branch carried ("the arena cascade is what styles the new
  rows"): the rows share one style row. It still cascades 40 in 40,
  because the wheel marks its repeater and the rule asks marked nodes.
- **The shadow oracle cannot see this class.** Its second layout reads
  the same arena style row as the bounded frame, so a missed cascade is
  unstyled in both. The risky cases' oracle is therefore an `Everything`
  host — which walks the whole table whatever was marked — beside the
  `Boundaries` one, plus the row's resolved width.
- **The kill rule held:** `handled/split`'s pump fell 21.76 µs (−39%)
  against a before-spread of 1.74 µs (`docs/results-damage-precision.md`
  T53.2). The counter is 0 of 40.

### T53.3 — an idle pump does nothing

**Closes:** R6-F1.

**Change:** §2.3(a) in `pump_with`. The predicate is evaluated after the
bindings and before the cascade, from values the pump already computes.
The first frame after construction, `adopt`, a retry or a resize is
never skipped. `docs/results-competitive.md` §5b states the whole idle
pump beside the layout-only row, relabels that row "layout only", and
after this task states the idle pump's measured cost with the fast path.

**Gate:** T53.0's `idle_pump.rs`. `just agent-drive` and
`just widget-drive` under both strategies, plus the oracle leg M41
adds beside the plain `Boundaries` one (M41 rule 2 and T41.2), quiet
over every frame, with the number of skipped frames printed beside the
number of frames inspected — so a run in which nothing was skipped
cannot pass as a run in which skipping was checked.

**Injection:** make the predicate always false: `idle_pump.rs` fires
with one place and one paint pass per pump. Make it always true: the
oracle fires on the first click. And the injection that tests the
oracle rather than the fast path: a widget write with its mark removed
(M41's compose case is the model) must make the oracle fire **with the
fast path on**; if it cannot, the fast path does not ship (§5).

**Accepts good input:** a pump after `invalidate` still paints, and a
hover change still produces its frame.

**Docs in the same commit:** `docs/results-competitive.md` §5b
(`:348`, `:388-390`); `docs/results-damage-precision.md`, the T34.2 idle
row's definition; `docs/00-handoff-static-tree.md`, invalidation; the
`pump_with` rustdoc.

**Effort:** M. **Depends on:** T53.1, T53.2 (the predicate reads the
arena flag after T53.2 has narrowed it); M41.

### T53.4 — a motion that changes nothing presents nothing

**Closes:** R7-F6.

**Change:** §2.3(b) in `crates/lumen-static-winit`. `Win` records
whether the pending redraw was its own; a self-requested redraw whose
damage is `None` skips the backend's present. The software surface
still presents at buffer age 0, and the GPU backend on any frame the
platform requested.

**Gate:** T53.0's live gate, both strategies, `presents N for M motions
(bound 2)`. `docs/results-frame-cost.md`'s Idle section (M47/T47.8)
gains the per-motion row: presents and pump time per motion, before and
after.

**Injection:** always present: about one present per motion. Make the
fast path's predicate always false: the same.

**Accepts good input:** a motion that crosses a `Button`'s edge produces
exactly one frame with non-zero damage, asserted in the same gate; an
expose after another window covered this one repaints it.

**Docs in the same commit:** the comment at `crates/lumen-static-winit/src/lib.rs:527-531`
("A still UI idles"); `docs/results-frame-cost.md`, Idle;
`docs/results-competitive.md` §5b; `docs/00-handoff-static-tree.md`,
invalidation.

**Effort:** M. **Depends on:** T53.3; M47/T47.8 (the section it writes
into); M40 (damage in physical space, which the present decision reads).

## 4. Boundaries of this plan

- **Not the per-event state sweeps.** R3-F12 (dispatch sweeping the
  whole state array twice per event, and moves not merged) was in the
  review's M53 row; the assignment gives it to M44, where input
  semantics live. T53.4's per-motion row will show its share once M44
  lands.
- **Not `place`'s whole-tree walk.** `place` takes no `Changed`
  (`Host::timings3`'s rustdoc). An idle frame now skips it; a busy
  frame still walks it. Narrowing `place` is not asked for by any of
  these findings.
- **Not paint culling.** `docs/plan-damage-precision.md` §4 keeps it
  shelved on its own condition, and nothing here meets it.
- **Not the agent's timings.** M48 owns `app.perf` answering zeros
  (R13-F2). Every gate here is headless or reads counts, so none waits
  for it.
- **No won't-fix items.**

## 5. Kill rules and risks

Written before any build. Clock figures are the median of eight
interleaved runs a side; "spread" is the arm's fast-cluster width by
M35/T35.0's mechanical rule.

- **T53.2.** If the `handled/split` arm's whole pump (T53.1's column)
  does not fall by more than that arm's spread, the list is not shipped;
  the counter gate and the restated share are, and finding 9 is
  recorded as measured and not worth building. The count gate (0 of 40)
  is required either way before anything ships.
- **T53.3.** If the predicate slows a **busy** frame — the grid's `bound`
  arm — by more than its spread, the predicate is reworked before it
  ships. **If M41's oracle cannot be made to fire on a skipped frame
  that should not have been skipped (T53.3's third injection), the fast
  path does not ship at all**, and §5b is corrected with the whole idle
  pump instead: a fast path that nothing can check is the defect this
  repository has written down six times.
- **T53.4.** Count-gated only. It ships if presents fall to the bound
  and the accepting cases hold.
- **What would stop the milestone:** if M41 does not land, T53.2's
  risky cases and T53.3 have no oracle to stand on. T53.0 and T53.1 ship
  regardless, because they only measure.

Risks, and what would reveal each:

- **An arena appears inside a narrowed range** (§2.2). Revealed by
  T53.2's `Option` and enum cases.
- **Some state that changes paint has no mark.** Until now paint ran
  every frame and hid it. Revealed by M41's oracle over the drives with
  the fast path on, which is why T53.3 prints the skipped-frame count.
- **The platform discards a skipped present's buffer.** Some compositors
  treat an un-presented frame as a request to keep the old one, and some
  do not. Revealed by T53.4's expose case, and by `just gpu-parity`
  under Wayland where M43's runner has it.
- **The gap bound is too tight on a noisy machine.** Revealed by the
  clean build failing it. The bound is widened only with the reason
  written beside it, never to hide a stage.

## Appendix: finding → task

| ID | sev now | status | task |
|---|---|---|---|
| R6-F1 | S2 | OPEN | T53.3 |
| R6-F2 | S2 | WORSE | T53.1 |
| R6-F3 | S2 | OPEN | T53.2 |
| R6-F4 | S3 | OPEN | T53.1 |
| R7-F6 | S2 | OPEN | T53.4 |
