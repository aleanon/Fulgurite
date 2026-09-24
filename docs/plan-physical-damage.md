# M40 — Damage in the space the surface draws in

> **Written 2026-09-24, not started.** Closes **R1-F1** (S0), **R1-F8**,
> **R9-F5**, **R1-F7** and **R18-F5** from M38's deep review
> (`docs/results-deep-review.md`), as re-triaged against `main` at
> 1717808 on 2026-09-24. Where the review and the re-triage differ, this
> plan follows the re-triage, because the code moved in between.
>
> **Phase 1**, alongside M39: R1-F1 puts stale pixels on every window
> whose display scale is not 1.0, in the default configuration, under
> either invalidation strategy. It has no upstream dependency. **It lands
> before M41's oracle work (M41/T41.2)**, because the redesigned oracle
> compares display lists that go through `Scaled`, and because R1-F1
> changes the units of the damage a surface is handed. §2.1 records one
> thing this plan found that the re-triage did not: the agent's
> `ui.lastDamage`, and with it the drive harness's 1.00× ratio, do **not**
> change units, because neither is computed from retention.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and prints the measured quantity beside its bound.*

---

## 0. The rules for this milestone

1. **One coordinate space per quantity, named where it is defined.**
   Layout, hit testing, `NodeState` and the agent's journal are logical.
   Retention's journal, `Damage`, the display list and both rasterisers
   are physical. Every doc comment this milestone touches says which
   space its numbers are in, in words.
2. **No gate here runs at one scale.** Every pixel gate runs at 1.0,
   1.25, 1.5 and 2.0. The 1.0 arm is the control. It must print 0 both
   before and after the fix, or the gate is measuring something other
   than scale.
3. **Several steps per session, never one.** The verifier saw residue on
   3 of 12 steps (R1-F1's notes). A single-step gate can pass by
   choosing a step whose ink happens to stay inside the logical rect.
   Each step asserts that it landed and that its damage is non-empty, so
   a no-op step cannot pass.

## 1. What is wrong, at 1717808

### 1.1 Retention observes logical rects, and the surfaces apply them to physical pixels (R1-F1, S0)

`Host::pump_with` builds `Scaled::new(&mut self.list, ..)` and wraps
`Recording::new(&mut scaled, &mut self.retain)` around it
(`crates/lumen-static-shell/src/lib.rs:1260-1261`). `Recording` hashes
and bounds each command before `Scaled` multiplies it, so every region
in `Damage` is logical. Both surfaces use those regions unscaled against
a physical viewport: the software surface at
`crates/lumen-static-winit/src/surface.rs:139-170`, and the GPU surface
at `crates/lumen-static-winit/src/gpu.rs:82` through `regions_for`
(`crates/lumen-static-winit/src/lib.rs:793`). M37 moved the window onto a
`Session`, but the `Host` path is unchanged.

**The re-triage's numbers at 1717808**, on the *default* `Host`
(`Incremental` and `Boundaries<256>`) against a `Rebuild` +
`Everything` host, hovering one button: at scale 1.0 the damage is
`[42,0,78,34]` and 0 px differ; at 1.5 the damage is still
`[42,0,78,34]` while the physical rect is `[63,0,117,51]`, and 1 923 px
differ with exactly that bounding box; at 2.0, 4 272 px differ in
`[84,0,156,68]`. Swapping the two wrappers gives damage equal to the
physical rect and 0 px at all three scales, with `damage_oracle.rs`
(10/10), `scale.rs` (4/4) and `glass_oracle.rs` (7/7) still green. The
review's own figures are in `docs/results-deep-review.md` §2.1.

Two comments beside that code are wrong. `lib.rs:1255-1259` says
retention observes the stream "on the way past", which is true and is
the defect. `lib.rs:1249-1253` says `set_bounds` is given **logical**
units, and the call passes `self.size`, which is physical. **Here the
code is right and only the word is wrong**: `Scaled::clip_bounds`
(`crates/lumen-static/src/display.rs:1126-1129`) divides the list's bound
back to logical on the way out, so a container culls against its own
coordinate system. The verifier's note that the cull bound is "too large
by a factor of the scale" does not hold. The comment is corrected in
T40.1, and the code is not changed.

### 1.2 The rasterisers do not agree on a fractional edge (R1-F8, R9-F5)

Once damage is physical, a fractional edge is the ordinary case: every
edge of a 1-logical-px layout at 1.25 or 1.5 lands between pixels. Two
defects follow.

- **The glyph blit truncates its far edge** (R1-F8). `draw_run`
  computes `(x1, y1)` with a bare `as i64` on the clipped rect
  (`crates/lumen-static-shell/src/software.rs:133`). Every other command
  rounds the far edge up (`software.rs:168`, `:239`, `:531`, `:817`,
  `:1017`), and `clear_within` rounds outward. So a damage region whose
  far edge is fractional clears one row that the glyph redraw then stops
  short of, and that row stays wrong until something damages it again.
  The verifier corrected the review's attribution to this one command.
  The re-triage read the code at 1717808 and found it unchanged; it did
  not re-run the probe.
- **A 1-logical-px rect is drawn heavier than its geometry at fractional
  scale** (R9-F5). `Scaled` multiplies coordinates with no snapping
  (`display.rs:1036-1040`), and the software `fill` covers
  `floor..ceil` at full intensity. At 1.25, 1.5 and 1.75 every
  1-logical-px divider inks two full physical rows (the ratios are in
  `docs/results-deep-review.md` §2.3, R9-F5). The GPU ramps the same
  edge. `crates/lumen-static-wgpu/tests/parity.rs` uses whole-pixel
  edges only, so no gate has run a scale other than 1. **The verifier's
  correction is adopted**: `docs/results-gpu.md` §2 states its 0.2%
  bound for whole-pixel edges, and `tolerance_is_bounded_at_fractional_edges`
  tests the documented ramp difference. So "40× over the parity bound"
  measures a documented tolerance. The defect is the undocumented heavy
  hairline, and it is on the default backend.

### 1.3 Two gates that cannot fail (R1-F7, R18-F5)

- **The display scale's line into the text memo has no gate** (R1-F7,
  S3). `.with_scale(self.scale)` in `pump_with`
  (`crates/lumen-static-shell/src/lib.rs:1197-1203`) is present and
  correct. **The re-triage deleted it at 1717808** and ran
  `cargo test -p lumen-static-shell -p lumen-static-widgets
  --no-fail-fast`: 69 test binaries, 345 passed, 0 failed.
  `BlockMetrics` is linear in the scale, so no existing `Host` test can
  tell whether the scale reached the memo.
- **`Regions::push` may drop the fifth region, and no test notices**
  (R18-F5, S3). The mutant that discards `r` on the full-set branch
  (`crates/lumen-static/src/retain.rs:205`) **passes the whole dev-shape
  suite at 1717808**, a wider suite than the review ran. The test that
  covers the branch bounds the merged area, and dropping a region makes
  that bound easier to meet. The same method's rustdoc
  (`retain.rs:169-177`) says "the two regions whose merge adds the least
  area are merged". The code first merges `r` into the last slot and
  then merges a least-cost pair, so five disjoint squares end as three
  regions. That loses precision, not correctness (the verifier's side
  observation).

## 2. Decisions

### 2.1 Where damage becomes physical

**Option A — swap the wrappers (recommended).** `Recording` goes
innermost around `self.list`, and `Scaled` goes outside it. Retention then
hashes and bounds the commands the rasteriser actually draws.

**Option B — scale the regions on the way out.** `Recording` stays
where it is, and `Retention::finish` or `Host::take_damage_for_age`
multiplies each region by the scale and snaps it outward.

| | A: swap | B: scale on exit |
|---|---|---|
| retention's journal | physical | logical |
| `Host::damage`, `Host::take_damage_for_age` | physical at the source | physical only if *both* exits convert |
| a fractional edge | one rule, in the rasteriser (T40.2) | snapped in the conversion **and** covered by the rasteriser |
| glyph origins | rounded in physical space (`software.rs:126-127`), inside the region retention saw | rounded in physical space, **outside** anything retention saw, so the conversion needs a pad |
| `Glass` backdrop dependency | observed after `Scaled` has scaled `blur`, so physical | logical rect with a logical `blur`, converted later |
| command identity | the scale is part of the hash; harmless, because `Host::set_scale` resets retention (`lib.rs:787-796`) | unchanged |

**What each does to the agent and the drive harness.** The re-triage
said the agent's `ui.lastDamage` and the widget drive's 1.00× ratio
compare damage with logical widget boxes, so the fix would have to
divide by the scale. **That does not match the code.**
`ui.lastDamage` (`crates/lumen-static-agent/src/lib.rs:1313-1333`) is
derived from the agent's own journal of `NodeState` geometry
(`crates/lumen-static-agent/src/journal.rs:335-346`), which is logical,
and it never reads retention. `scripts/drive/harness.py:385-392` divides
it by a logical widget box. **So under either option `ui.lastDamage`
keeps logical units and the 1.00× ratio does not move.** Dividing by the
scale would make it wrong.

What this reveals is worse, and it is a fact about M42 and M48, not
about this fix. **The 1.00× ratio has never measured what the surface
redraws.** At scale 1.5 on 1717808 the journal reported the hover's own box
while the surface redrew a logical-sized rect at the logical position,
and the physical rect that changed was left partly stale. The one agent-visible quantity that *is* retention-derived is
`app.perf`'s `damaged_px`: the software surface counts it from the
regions it rendered (`surface.rs:170`, reported at `:204`), and the GPU
surface counts it at `gpu.rs:112`. Under either option it grows by the
scale squared, because until now it counted logical rects as physical
pixels. Nothing in `scripts/` bounds it today. Reporting the presented
damage beside the journal's, and having the harness print both, belong
to M48 (T48.3 makes `app.perf` report it, and T48.4 prints it in the
drive); M42 only records the distinction (§4).

**Recommendation: A.** It is one line. The re-triage verified that it
fixes the repro and keeps every existing oracle green, and it leaves one
coordinate space in the journal. B keeps two spaces in one structure,
and at 1.25 and 1.5 it needs outward snapping in the conversion as well
as in the rasteriser, which is two places to get the same edge wrong.
**What A breaks:** the claim in `pump_with`'s comment, and the rustdoc on
`Host::take_damage_for_age`, `Host::damage` and the retention module,
which never said which space they were in. `app.perf`'s `damaged_px`
changes units as described above.

### 2.2 How a fractional rect edge is drawn

**Option A — snap rect edges in `Scaled` (recommended).** Round each edge
of a plain rect to the nearest device pixel after scaling. If both edges
of an axis round to the same pixel, keep it one pixel wide. Round the
edges independently rather than the origin and the size, so two rects
that share an edge still share it after rounding.

**Option B — coverage-weighted edges in the software `fill`.** This
matches the GPU's ramp. It draws blurry hairlines, and it adds a blend
per edge pixel to the hottest loop in the software rasteriser.

**Recommendation: A.** It draws crisp hairlines, as platform toolkits
do. It makes the two backends agree on whole-pixel edges, so the parity
bound means something at every scale. And after T40.1 retention sees
the snapped rect, so a rect's damage is integer-edged.

**Scope.** `Scaled::rect` only. Rounded and stroked rects use the SDF
the two backends share, and the review measured them agreeing at 1.25
and 1.5 (R9-F5's evidence). Text, lines, gradients, images and glass are
unchanged.

**At every scale, including 1.0.** A fractional *layout* edge at 1.0,
such as a 31.2 px text line, over-inks by the same mechanism. The cost is
that `Scaled` stops being a pure forwarding wrapper at 1.0, and its
rustdoc says it is one. §5 has the kill rule that would restrict the
snapping to scales other than 1.0.

**What it breaks.** Pixels at fractional rect edges change on both
backends. Two gates should be unaffected, and T40.3 confirms both.
`parity.rs`'s tolerance test builds its list directly and does not paint
through `Scaled`. The golden images render hand-built lists (strategy-
and scale-free, `docs/plan-boundaries-default.md` §1).

## 3. Tasks

### T40.0 — the gates first

Each finding becomes a gate, committed before any fix. A gate that is
red at 1717808 lands with `#[ignore = "red until T40.n: <finding>"]`.
The commit body records its red output, run by name with `--ignored`,
and the task that fixes it deletes the `ignore`. The two gate-only
findings close here.

| gate | finding | at 1717808 |
|---|---|---|
| `crates/lumen-static-shell/tests/damage_scale.rs`: the default `Host` against `Rebuild` + `Everything` at 1.0, 1.25, 1.5 and 2.0. Three steps per scale: a hover, a bound-label change and an `adopt`. Partially render `take_damage_for_age(1)` onto a persistent canvas, fully render the oracle, and compare. Per step, print the regions, the changed node's physical rect, `px differing N / bound 0` and the bounding box. Assert that `hovered` is set, that the label text changed, and that the damage is non-empty | R1-F1 | **red** at 1.25 and above; 0 at 1.0 |
| `crates/lumen-static-shell/tests/partial_text_edge.rs` (truetype, with a pinned face): a text run straddling the damage `[0,87.2,360,173.2]`, painted over a canvas pre-filled from a different frame, then compared with a full render. Prints `px differing N / bound 0` and the bounding box. An integer-edged arm must print 0 | R1-F8 | **red** |
| `crates/lumen-static-shell/tests/fractional_ink.rs` (software only, so `just test` sees it): six 1-logical-px dividers through `Scaled` at 1.25 and 1.5. It asserts that each divider's ink equals `round(scale)` physical rows times its width, and prints the ink ratio beside 1.00. Its twin is a new arm in `crates/lumen-static-wgpu/tests/parity.rs` that prints the share of pixels over 8 levels beside 0.2% | R9-F5 | **red** (the ratio over 1.00) |
| `crates/lumen-static-shell/tests/scale_memo.rs`: a `Host` with `measure_scale.rs`'s non-linear `Hinted` measurer, moved to a shared test helper. It pumps at 1×, then calls `set_scale(2.0)` and resizes to twice the physical size with the logical size unchanged, pumps again, and asserts the label's box equals a fresh host's at 2×. It prints both widths beside `bound: equal`, and asserts as its setup that the widths at 1× and 2× differ under `Hinted` | R1-F7 | green. **Injection d2**, deleting `.with_scale(self.scale)`, turns it red, and a no-drag control at 1× stays equal. **Closes R1-F7** |
| `crates/lumen-static/tests/damage_regions.rs`, `every_pushed_rect_stays_covered`: every pushed rect is contained in the union of the result, or the result is `All`. It runs over the review's five disjoint squares and over 64 rects from a fixed LCG, and prints `pushed N, regions n, uncovered 0 (must be 0)` | R18-F5 | green. **Mutant push-1** (`retain.rs:205` keeps the old slot and drops `r`) turns it red, naming `[800,400,810,410]`. **Closes R18-F5**, with `Regions::push`'s rustdoc corrected to say what the full-set branch does |

**Docs in the same commit:** the `pump_with` comment at
`lib.rs:1197-1203` names `scale_memo.rs`, and
`docs/results-frame-and-types.md`'s T26.17 #12 entry gains the host-level
gate. **Effort:** M. **Depends on:** nothing.

### T40.1 — retention observes physical commands

**Closes:** R1-F1.

**Change:** in `Host::pump_with`
(`crates/lumen-static-shell/src/lib.rs:1255-1262`), put `Recording`
innermost around `self.list` and wrap it in `Scaled` (§2.1, option A).
Leave the `set_bounds` call as it is and correct its comment: the bound
is physical, and `Scaled::clip_bounds` converts it back.

**Gate:** `damage_scale.rs` from T40.0, with its `ignore` removed. It
prints 0 px at every scale on every step.

**Injection:** restore the old order, with `Scaled` innermost and
`Recording` outside. At 1.5 the gate must fire with the bounding box
`[63,0,117,51]`, and at 2.0 with `[84,0,156,68]`. The pixel counts the
re-triage measured (§1.1) are the expected sizes. **Accepts good
input:** the 1.0 arm prints 0 before and after the fix, and every scale
prints 0 with it. `damage_oracle.rs`, `scale.rs` and `glass_oracle.rs`
stay green.

**Docs in the same commit:** the `pump_with` comment
(`lib.rs:1249-1259`); the rustdoc on `Host::take_damage_for_age` and
`Host::damage`, which now say "physical pixels"; the module rustdoc of
`crates/lumen-static/src/retain.rs`, on the space `Damage` is in;
`docs/damage-amplification-decision.md`, one paragraph saying that its
1.00× ratio is journal damage in logical pixels, and is not the damage a
surface presents (§2.1); `docs/results-retention.md`, if it states the
space; `docs/00-handoff-static-tree.md` §2 (the raster step: "damage
rects only" gains its coordinate space). **Effort:** S. **Depends on:**
T40.0.

### T40.2 — the glyph blit rounds its far edge like every other command

**Closes:** R1-F8.

**Change:** in `draw_run` (`crates/lumen-static-shell/src/software.rs:133`),
take the far edge with `ceil()` before the cast, as `software.rs:1017`
already does. Write the convention once in the rasteriser's module
rustdoc: every command covers `floor(x0)..ceil(x1)` of the clipped rect,
and `clear_within` rounds outward to match. **Snapping damage regions
outward in `Retention::finish` is not done**: after T40.1 the regions are
the drawn rects, and one convention in the rasteriser is enough (§2.1).

**Gate:** `partial_text_edge.rs` from T40.0, with its `ignore` removed.

**Injection:** restore the truncating cast. The gate must fire on the one
row at the edge, with a bounding box one pixel tall at y = 173.
**Accepts good input:** the integer-edged arm is 0 px with and without
the fix. Run `damage_scale.rs` under the injection too: at 1.25 and 1.5
its bounding boxes must include a row at a fractional edge, which shows
that T40.1's gate can see this defect at a device scale.

**Docs in the same commit:** the `draw_run` comment; the `snap_out`
rustdoc in `crates/lumen-static-wgpu/src/lib.rs:150-156`, which says
only the GPU path is exact and now states that both paths follow the
floor..ceil convention; `docs/results-retention.md`, if it claims
partial equals full on software. **Effort:** S. **Depends on:** T40.1.

### T40.3 — a rect edge lands on a device pixel

**Closes:** R9-F5.

**Change:** `Scaled::rect` (`crates/lumen-static/src/display.rs:1037`)
rounds each scaled edge to the nearest integer. If both edges of an axis
round to the same value, it keeps them one pixel apart (§2.2). No other
`Painter` method changes.

**Gate:** `fractional_ink.rs` from T40.0 with its `ignore` removed, plus
the `parity.rs` arm, run by `just gpu-parity`. The software twin prints
an ink ratio of 1.00 at 1.25 and 1.5. The parity arm prints its share
of pixels over 8 levels beside the 0.2% bound.

**Injection:** remove the rounding. The software twin fires with the
over-inked ratio at 1.25. The parity arm fires over its bound.
**Accepts good input:** the 1.0 and 2.0 arms are exact before and after,
and two rects that share a fractional edge still share it: a
two-rect arm asserts no gap row and no overlap row between them.

**Docs in the same commit:** `Scaled`'s rustdoc (display.rs:997-1016,
1022-1027), which today says it is a forwarding no-op at 1.0;
`docs/results-gpu.md` §2 (what fractional edges look like now, and that
the 0.2% bound holds at 1.25 and 1.5); `docs/00-handoff-static-tree.md`
§2 (the raster step: a one-line DECIDED on how a hairline lands on a
device pixel). **Effort:** M. **Depends on:** T40.1
(retention must see the snapped rect).

## 4. Boundaries of this plan

- **`ui.lastDamage` is not changed.** It is journal damage in logical
  pixels, and it stays that. Adding the presented, retention-derived
  damage in physical pixels beside it, and having the drive harness
  print both and fail a repainting action that presents nothing, belong
  to **M48** (T48.3 and T48.4, agent truthfulness). **M42** does not
  change the damage block (M42 §4). This plan only records the
  distinction (T40.1's docs).
- **`app.perf`'s `damaged_px` changes units** in T40.1, and it is the
  only agent-visible quantity that does. No gate or script bounds it
  today (checked with `grep -rn damaged_px scripts docs`).
- **R1-F7 is closed for `Label` only.** Extending `scale_memo.rs` to
  `TextInput` and `TextArea` waits for R2-F6, whose shaped keys omit the
  scale. That is **M45**. The re-triage offered M45 as an alternative
  owner. This plan keeps R1-F7, because its fixture is scale machinery
  and not text.
- **R18-F5's over-merging is not fixed.** Five disjoint squares ending as
  three regions costs precision. T40.0 corrects the rustdoc so that it
  describes the algorithm, and the algorithm stays. Improving it would be
  a performance claim that needs its own measurement. The re-triage
  proposed M44 (test hardening) for this finding. It stays here because
  it is about `Damage`.
- **GPU-only items** (the atlas, the fallback, parity blind spots) are
  **M51**.

## 5. Kill rules and risks

- **T40.3's kill rule, written before the build.** Measure showcase's
  paint time with `Host::timings3`, eight runs a side, before and after,
  at scale 1.0. **If the after-median lies outside the before-arm's
  spread, restrict the rounding to scales other than 1.0**, keep the
  forwarding path at 1.0, and record both arms in
  `docs/results-gpu.md` §2. The defect at 1.0 (fractional layout edges)
  then stays open, and is named there.
- **What would stop the milestone:** nothing that is currently known.
  T40.1 is one line, and the re-triage has already run it against the
  existing oracles.
- **Risk: a glass panel's backdrop dependency at fractional scale.** With
  `Recording` innermost, the dependency region is computed from the
  scaled `blur`. *Revealed by:* `damage_scale.rs` gaining a glass step
  (a press on a `Glass` button) at 1.5. The alternative is a scale arm in
  `glass_oracle.rs`, whichever is cheaper to set up.
- **Risk: the GPU path at fractional scale.** `snap_out` already snaps
  each region outward, so the GPU path is expected to be exact after
  T40.1. *Revealed by:* `crates/lumen-static-winit/tests/gpu_regions.rs`
  asserting a region at 1.5, and `just gpu-parity`'s new arm.
- **Risk: a stale pixel at the window's edge.** `Damage::regions` clamps
  against a physical viewport, and until T40.1 it clamped logical rects
  against it. *Revealed by:* a `damage_scale.rs` step whose changed node
  touches the right edge of the window.

## Appendix: finding → task

| ID | sev now | status | task |
|---|---|---|---|
| R1-F1 | S0 | OPEN | T40.1 |
| R1-F7 | S3 | OPEN | T40.0 |
| R1-F8 | S2 | OPEN (read only) | T40.2 |
| R18-F5 | S3 | OPEN | T40.0 |
| R9-F5 | S2 | OPEN | T40.3 |
