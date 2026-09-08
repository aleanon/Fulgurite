# M16 — invalidation, and repairing the instrument that measures it

*Planned 2026-09-08. The work queue produced by the performance review
of that date (three agents: competitor-arm restoration, per-stage
profiling, resource measurement; then a serial measurement pass on a
quiet machine). Every task cites the finding that produced it so the
reason survives the fix. Each task is one commit; each ends with
`just test` green.*

**What the review established, so this plan does not re-argue it.**
lumen-static led every arm that produced a number, at 100, 1000, 3000
and 10000 rows, on both a one-row update and a full redraw — 7.7× iced,
21× egui, 24× masonry, 38× Lumen, 7.9× Qt, 6.4× GTK at 3000 rows. The
hypothesis that small N might hide a gap was refuted: the margin over
iced is *wider* at 100 rows (8.6×) than at 3000. **The ranking is not
what this plan is about.** The per-stage profile and the seven defects
are.

**Sizing.** T16.0–T16.3 are hours each. T16.4–T16.6 are a day together.
T16.7 is the large one — the milestone's centre — and T16.8–T16.10
depend entirely on it. T16.12 is a day and is the only task here whose
design is genuinely unsettled. T16.13–T16.14 are a day together.

---

## The ordering principle, decided first

**The instrument is repaired before anything is optimised, because the
instrument is what would judge the optimisation.**

This is not a general preference for tidiness. Three specific defects in
the benchmark would corrupt the measurement of this plan's own work:

- the damage figure is a min-of-200 over frames whose damage *alternates*
  between 1 562 and 57 600 px, so it reports the lucky frames — and
  every task in Phase C changes exactly that number;
- four competitor arms are byte-identical because cargo unified their
  features, so any size or RSS row derived from them is meaningless;
- the competitor runner exits 0 with no output for an arm that works,
  so an arm can silently vanish from a comparison.

Optimising first and measuring with that instrument would produce
numbers nobody could defend, including the person who produced them.
Phase A is therefore not preparatory work; it is the first three
commits.

**Corollary, and the harder half:** Phase C changes traversal, which is
the one thing every existing gate depends on. It needs an oracle, and
the project already has the pattern twice — `Rebuild` is the oracle
`Incremental` is tested against (M8b/T8b.0), and `Full` painting is what
`damage_oracle.rs` compares partial redraws to. **Invalidation gets the
same treatment: the whole-tree walk stays, as the oracle, and the
bounded walk must produce byte-identical output.** A task that cannot be
checked against the full walk does not land.

## The gating policy

Inherited unchanged from M15 (`docs/plan-capabilities.md`): a separate
crate, then a type parameter, then a cargo feature, in that order of
preference; `no_std` stays buildable at both tiers; no new per-node
field without a re-budget; every task states its cost in the same commit
it lands.

**One measurement this milestone starts from.** `NodeState` is
**exactly full at 76 bytes** — 72 of payload, then `pressed`, `hovered`,
`focused` and `focus_ring` filling the last four. There is no spare
padding, so a dirty bit costs 4 bytes on every node of every tree unless
the three bools are packed into one flags byte, which frees two at zero
size cost. `crates/lumen-static/tests/focus.rs` and
`fixtures/embedded` assert the 76 against the RAM budget in
`docs/results-embedded.md`. **Neither number moves in this milestone.**

---

# Phase A — repair the instrument

## T16.0 — commit the review harness

The review produced three instruments that exist only in the working
tree: `benches-competitive/arms/stages/` (the per-stage profiler, 24
cells: N × workload × retention), `benches-competitive/competitors.sh`
(one runner for every competitor arm), and
`benches-competitive/resources.sh` (RSS, peak RSS, size, startup,
allocation). Three files in the sibling Lumen checkout gained a
`BENCH_SIZES` / `BENCH_CHURN_SIZES` environment override so the sweep
can reach 10000 rows; their defaults are unchanged, so an unset variable
reproduces the 2026-09-05 run exactly.

Commit them, with the sibling patches listed explicitly in the body —
they are a different project and the edit must be findable from this
repository's history.

**Gate:** `just test` green; `just stages` runs to completion and its
stage sums agree with the independently-timed frame to within 1% at
every cell; `competitors.sh list` reports every arm's build status.

## T16.1 — one cargo invocation per size-sensitive binary

*Finding: four Lumen variant arms — counter, lean, no-a11y, no-dict —
are all exactly 17 593 576 bytes and the accessibility-free one still
contains accesskit and atspi strings, because they were built in one
cargo invocation and cargo unified their features. Reproduced on our own
side: one workspace build puts app and lean 16 bytes apart; one package
per invocation puts them 159 312 apart.*

Every size- or RSS-sensitive arm gets its own `cargo` invocation, and
its executable is copied out immediately because the next invocation
overwrites the same path. This is the trap this repository's own
`justfile` documents at length; the benchmark walked into it anyway,
which is worth a sentence in the script rather than only in a commit
message.

**Gate:** the four Lumen variants differ in size from one another; the
no-a11y build contains no `accesskit` string (`strings | grep`); the
lean/default gap reproduces at ~159 KB.

## T16.2 — make the damage figure deterministic

*Finding: `List::paint` clips its items to the list node's rect, and
that node's width is the widest row. A row whose text changes width
rewrites the clip, and `Diffing::step` unions old ink with new — so a
one-row change damages the whole column. At 100 rows the frame arm
alternates between 1 562 and 57 600 damaged pixels depending on which
digits row 0 contains, and min-of-200 selects the small ones.*

In the **benchmark**, pin the column width so the reported number
describes the typical frame rather than the lucky one. Pinning with a
wider sibling row already demonstrates the mechanism and makes the
damage constant across iterations.

Also fix `DIAG=1`, which calls `retain.finish()` after the loops with no
intervening observation, so `Diffing::finish` returns `Damage::All`
unconditionally and the diagnostic has never once printed what the frame
actually damaged.

**Gate:** `damaged_px` is identical on every iteration of a run; `DIAG=1`
prints a `Rect` whose area matches it.

*The **framework** half of this finding is T16.11 and is deliberately
separated: whether a container's intrinsic-width change should re-damage
content that did not move is a design question, not a benchmark fix.*

## T16.3 — the runner fails loudly, and the arm is warning-clean

*Findings: `competitors.sh run masonry` exits 0 and prints nothing while
the arm itself works and reports 32.6 µs — a silent skip, in a script
written specifically to refuse rather than skip. `gtk_init()` adopts the
environment locale and the GTK arm never resets it, so on a machine with
`LC_NUMERIC=nb_NO` it prints `9,1 us` and the parser's `([\d.]+)` reads
`9`. The published 111.5 has a decimal point, so that run was made under
`LC_ALL=C` and survived — the hazard is real, the number is fine.
`arms/frame` has an `unused_mut` warning, and its module doc says it
reports four rows when it prints seven.*

A parser that matches nothing must exit non-zero and name the arm.
`LC_ALL=C` on every path. Fix the warning and the doc.

**Gate:** an arm whose output does not parse exits non-zero; every frame
arm produces a number or a named failure; `cargo clippy` clean across
`benches-competitive`.

---

# Phase B — correct the record

## T16.4 — `results-competitive.md`

Four corrections, each citing the measurement that overturned it:

- **GTK's caveat is wrong in its stated reason.** The doc says GTK "lays
  out ~63 rows at any N". Measured, `gtk_widget_measure` returns 16 px ×
  N exactly — 1 600 / 16 000 / 48 000 / 160 000 — and the arm scales
  9.0 → 375.9 µs across the sweep. Nothing clamps. The sibling report's
  original wording was about *rasterisation* and was restated here as
  layout. The number is still a floor, for the real reason: GTK caches
  per-widget size requests so only the dirtied label re-measures, and no
  paint happens at all.
- **The four Lumen variant rows come out** until T16.1 rebuilds them.
- **§5's `Rebuild` row is not a configuration measurement.** It relabels
  two numbers from the single `Incremental` arm. Retention is a type
  parameter with a default, deliberately kept out of the feature system,
  so no build flag separates the modes and the row cannot exist as
  written. Either measure both modes in one process — which
  `arms/stages` now does — or delete the row.
- **masonry's asymmetry is the opposite of what was recorded.** The doc
  calls its number an upper bound because a *test* harness may carry
  validation overhead; in release that validation is
  `debug_assertions`-gated and off. What `TestHarness` actually adds is
  an unconditional `EnableAccessTree`, so masonry is charged for an
  accessibility rebuild every frame that lumen-static and plain Lumen
  are not — 203 AccessKit nodes at 100 rows with it on, 0 with it off.
  `probes/masonry-raw` drives it through the public `RenderRoot` in ~50
  lines and can say what the difference is worth.

Add the new N=100 and N=10000 columns, which the table has never had.

**Gate:** every row in the file is reproducible from a command in the
same file; no row cites an arm the current tree cannot build.

## T16.5 — `results-gpu.md`

*Findings, both mine from 2026-09-07.* Two corrections:

- **The size table mixes release profiles.** Its 5.29 / 12.26 MB were
  built from `examples/minimal` in the framework workspace, which sets
  no `[profile.release]` at all — unstripped, no LTO, 16 codegen units —
  while `results-competitive.md` §4's 2.7 MB comes from
  `benches-competitive`, which sets `lto=true, codegen-units=1,
  strip=true, panic="abort"`. The GPU-vs-software *delta* and the RSS
  half are sound; the absolute megabytes do not belong in a column with
  §4's. Re-measure under one profile and say which.
- **The doc reads more favourably than the measurement supports.** The
  20× at 4K is real and holds — for a *full-screen redraw*. On the
  incremental path the GPU costs 33–88 µs against software's 4.9–29.9 at
  2 002 damaged pixels and is **slower at every size tested**; even on
  the all-rows workload it wins at 100 rows (65.8 vs 128.0 µs) and loses
  by 10 000 (190.8 vs 157.6). A crossover exists and the doc does not
  mention it. State it; T16.13 measures where it is.

**Gate:** the size rows name their profile; a reader deciding whether to
enable the GPU can find the case where it loses without running
anything.

## T16.6 — the doc-identifier gate reads markdown

*Finding: the design spec's DECIDED marker states that "rigidity is per
axis (`RIGID_W`/`RIGID_H`)" and `docs/plan-architecture-v2.md:835`
repeats it. Neither identifier exists anywhere in the code — there is a
single `RIGID` bit. `scripts/check_doc_identifiers.sh` scans backticked
names in `.rs` comments only, so markdown is unchecked. This is the
exact drift class the doc-currency rule exists to prevent, in the most
load-bearing document, in the one place the gate does not look.*

Extend the gate to `docs/*.md`. **Expect it to be red**, and expect the
first pass to be mostly allowlist: docs legitimately name upstream
types, environment variables, other projects' symbols and things
documented as deliberately removed. That backlog is the finding, not an
obstacle to it — the point is that it becomes visible and stays visible.

Fix `RIGID_W`/`RIGID_H` at the source: either implement per-axis
rigidity in T16.7, where it is genuinely useful, or downgrade both
citations from DECIDED to an open question. **Do not silently drop the
sentence** — it is a design decision with a recorded rationale, and the
rule is that a rule a commit establishes goes in a doc, not in `git log`.

**Gate:** the extended gate is red on the tree before this commit and
green after; the allowlist entries each carry a reason.

---

# Phase C — the optimisations

*Ordered by measured headroom, and every one of them checked against the
whole-tree walk as its oracle.*

## T16.7 — invalidation: the dirty set and its type parameter

*Finding: `NodeDesc::RIGID` is computed by the derive macro and read by
exactly one thing in the workspace — a diagnostic counter in the agent
crate. There is no dirty propagation and no boundary check, so every
frame lays out, places and paints all N nodes regardless of what
changed. Measured, a one-row change at 10 000 rows spends 176 µs of a
231 µs frame in layout + place + paint: **76% of the frame, for 9 999
rows that did not change.** The design spec's §2 ranks this first of the
four properties that produce performance.*

This task builds the machinery only; T16.8–T16.10 consume it.

**Gating.** A type parameter on the host, mirroring `Retention` exactly:
`Everything` (a ZST, today's whole-tree walk, and the oracle) or
`Boundaries<const N: usize>` (a bounded set of dirty subtree roots,
`no_std`-clean, no allocator). An application that opts out compiles to
today's code, and both live in one binary so the oracle is always
available — the reason M8b/T8b.0 gave for refusing a cargo feature here.

**Storage.** A set of dirty *subtree roots*, not a per-node flag.
Topology is a compile-time constant and subtree ranges are contiguous,
so "this subtree changed" is a `u32` and not a bit per node. This is
what keeps `NodeState` at 76 bytes. Should a per-node bit prove
necessary anyway, the three bools pack into one flags byte first and pay
for it out of the two bytes that frees — measured, not assumed.

**The open question, stated rather than discovered.** *How does a leaf
mutation reach the set?* `Dirty<T>` already marks a `Label` dirty at the
leaf, and `#[bind]` already knows which field it wrote — so the derive
can record a node index at zero runtime cost for bound fields. **Direct
mutation (`app.ui.title.set(…)` from a handler) is not covered by that**,
and the three candidate answers each cost something:

1. a `&mut` accessor that records — changes every widget's mutation API;
2. a post-update comparison pass — O(N), which is the cost being removed;
3. dirty-by-default for any subtree a handler could have reached,
   narrowed by the dispatch target — cheap and conservative, and it
   degrades to today's behaviour rather than to a wrong frame.

**(3) is the one to build first**, because being conservative is
correct-by-construction and the other two can narrow it later. Mark (1)
and (2) OPEN in the same commit.

**Gate:** `Everything` and `Boundaries` produce byte-identical display
lists over a driven session, asserted in a test that holds both in one
process — the `damage_oracle.rs` shape; `size_of::<NodeState>()`
unchanged at 76; `just thumb` green at both `no_std` tiers.

## T16.8 — paint skips clean subtrees

*Paint is the largest single stage of a small-update frame: 121 µs at
10 000 rows, emitting commands for 9 999 rows that did not change. It is
a pure O(N) walk with no memo to soften it, which is why it costs more
than layout (55 µs), where the per-label measurement cache already
absorbs most of the work.*

**The hard part is not skipping — it is the display list.** The list is
positional: retention diffs it by index (T13.8), the change journal
signs per-node command ranges, and `paint_containment.rs` asserts clip
balance. A subtree that is skipped must still contribute *the same
commands in the same positions*, or every one of those breaks. So a
skipped subtree replays its previous commands rather than emitting
nothing — which means the previous frame's list has to be kept, and that
is a memory cost this task must state.

**Gate:** identical display list against `Everything`, command for
command, over a driven session; `damage_oracle.rs` still pixel-identical;
the retained-list memory cost recorded in a results doc of its own,
as the policy requires of any cost an application cannot decline.

---

**NOT BUILT 2026-09-08, and the reason is a decision this plan did not
know it was arguing with.**

The task assumed a skipped subtree could replay its previous commands,
and that the cost of that was memory. It is not available at any price:
**retention does not keep the previous frame's commands.** M13/T13.8
removed the stored `DisplayList` deliberately and replaced it with
`(hash, ink)` per command observed as paint streams past, because a
stored list is exactly what made damage unavailable on a host with no
list — the embedded device, where damage is worth *more* than on a
desktop, since the cost there is the SPI bus and a tenth of the screen
is a tenth of 31 ms. Rebuilding the list to skip paint would undo that.

**And the deeper problem is not storage.** What makes a partial redraw
equal a full one is that every command *intersecting the damage rect* is
drawn, in order. A clean subtree still needs its commands whenever
something else damaged the region it sits under — an unchanged card
background beneath a changed label is the ordinary case. Whether a clean
subtree intersects the damage cannot be known during paint, because
damage is what paint is producing.

So a clean subtree's commands are needed *sometimes*, and paint cannot
tell when. Skipping on cleanliness alone is not an optimisation with a
memory cost; it is wrong.

**Where the paint win actually is.** Not invalidation — **culling**.
`List::paint` clips to its own rect and then paints all N items,
including the 9 960 of 10 000 that lie outside an 800 px viewport, and
the renderer discards them one at a time afterwards. That is a spatial
question, it is what GTK and Qt already do in their paint paths, and it
belongs with T16.12, which is the same question one layer down.

It needs care the invalidation route did not: a shorter display list
changes retention's positional diff (already handled — a length change
is `Damage::All`), and a *culled* paint number must never be quoted
against an arm that does not cull. Both are recorded here so the next
person does not rediscover them.

## T16.9 — layout and place skip clean subtrees

Layout is 55 µs and place 7 µs of the same 231 µs frame. Cheaper than
paint and structurally easier: layout already has the per-label memo, so
what is being removed is the *walk*, not the work.

This is where per-axis rigidity earns its place or is dropped: a card
fixed in width and shrink in height bubbles a height change only, which
is the whole reason `RIGID_W`/`RIGID_H` was written down. Either
implement it here — and T16.6's citation becomes true — or delete the
claim.

**Gate:** identical `NodeState` array against `Everything` after every
step of a driven session; the `RIGID_W`/`RIGID_H` citation is either
implemented or removed in this commit.

**DONE 2026-09-08, in the derive rather than per widget.** The boundary
is four lines at the top of the generated container `layout`: a subtree
nothing marked, re-measured under the constraints it was already
measured under, returns `st[i].size`. Both halves are load-bearing and
both are tested — the dirty set alone would keep a stale tree across a
resize, and the constraints alone say nothing about content.

Measured on `Ten10` (901 nodes), layout only, min of 2000
(`benches --bin invalidation`):

| frame | layout | vs the oracle |
|---|---:|---:|
| `Everything` — the whole-tree walk | 5.27 µs | — |
| `Boundaries`, one subtree marked (90 of 901 nodes) | 0.69 µs | **7.6×** |
| `Boundaries`, still frame | 0.02 µs | **293×** |

`RIGID_W`/`RIGID_H` are **not** implemented and the citation was
downgraded to OPEN in T16.6 rather than left asserting an API that does
not exist. Per-axis rigidity is a narrowing of this check, not a
prerequisite for it: the boundary above skips on the whole subtree being
clean, which is the coarse case, and an axis-split would let a height
change pass a width-rigid parent without dirtying it. Worth doing when
something measures a tree where that distinction pays; nothing here does
yet.

## T16.10 — the binding step becomes proportional to what changed

## T16.10 — the binding step becomes proportional to what changed

*`update` costs 16.4 µs at 10 000 rows for a one-row change, comparing
every row's formatted output.* The derive already knows which state
field each binding reads; once T16.7 exists this is small.

**Gate:** `update` flat in N for a one-row change; `benches --bin
bindings` shows no regression on the all-rows case.

## T16.11 — the framework half of damage amplification

*The benchmark half is T16.2. This is the question that one deferred:
should a container's intrinsic-width change re-damage content that did
not move?*

Today it must, and it may be right: if the column widens, alignment can
move everything inside it. But the common case — a row whose text got
one pixel wider, in a left-aligned list — moves nothing, and currently
repaints the column.

**This is a design question and should be answered as one**, in a
decision doc, with the measurement beside it: at 100 rows the difference
is 1 562 vs 57 600 damaged pixels, a factor of 37.

**Gate:** a decision record; if a change lands, `damage_oracle.rs`
pixel-identical with a width-changing row in the tree.

**DONE 2026-09-08 — `docs/damage-amplification-decision.md`.** The rule
is wrong: a clip is not ink, so a clip change can only affect the
**symmetric difference** of the old and new rects, not their union. The
fix is specified there and deliberately not built, because it widens the
`Retention::observe` seam M13/T13.8 shaped for a host with no display
list — the same trade T16.8 refused. Reopen when an application scrolls
or animates inside a shrink-width container, where the clip moves every
frame. **The amplification never produced a wrong frame, only a large
one**, and T16.2 already removed every published number that depended on
it.

## T16.12 — the damage-clipped raster becomes O(visible)

*Now carries T16.8's half of the problem too: the paint win is culling,
not invalidation. See T16.8's outcome above.*

*Software raster grows 4.89 → 29.85 µs from 100 to 10 000 rows while the
damaged region stays fixed at 2 002 pixels, because `render_impl` walks
the entire command list testing each command against the damage rect.*

**The obvious fix is wrong and the plan should say so before someone
tries it.** "Only draw the dirty subtree's commands" breaks correctness:
an ancestor's background rect still has to repaint *under* a changed
label, and what makes a partial redraw equal a full one today is that
every command *intersecting the damage rect* is drawn, in order. The
skip must be spatial, not structural.

Two candidates, neither free:

1. **A sorted early-out.** In a column, commands are emitted in y order,
   so a damage rect's y range is a contiguous run and a binary search
   finds it. Valid only where the container guarantees non-overlapping,
   monotonic children — a property `List` and `Repeat` have and
   `Overlays` explicitly does not. It would have to be a declared
   container property, checked in debug against a linear scan.
2. **Bounding-box culling with a coarse index** — general, but it is a
   spatial data structure per frame, which is exactly the kind of
   per-frame cost the M15 policy says must not be paid by a tree that
   does not need it.

**(1) first, behind the container property**, with (2) recorded as the
general answer if a real application needs it.

**Gate:** identical pixels against today's renderer over the whole
`damage_oracle.rs` session; raster time flat in N at constant damaged
area; a debug assertion that the sorted early-out agrees with a linear
scan.

---

**DONE 2026-09-08, one layer up from where the task looked.**

Neither candidate was built. The commands the renderer walks are the
commands *paint emitted*, and paint was emitting ten thousand of them
for a viewport that can show forty. Culling there removes the work
instead of getting faster at discarding it, and it needs no spatial
index and no container ordering property.

`Painter::clip_bounds()` reports the surface — deliberately **not** the
enclosing clip stack, which would be tighter but would make every
painter track one. A superset is sound for culling: it discards less
and never more. `None` is the default and means "cull nothing", so the
SVG backend, every unit test and every benchmark arm that does not opt
in emit exactly what they emitted before.

Measured on the frame arm, one row of N changing (`+paint` is
cumulative with layout, so the paint *stage* is the difference against
the `layout` row):

| N | commands | paint stage | culled |
|---:|---|---:|---:|
| 1 000 | 1 002 → 39 | 5.6 µs | 0.9 µs |
| 3 000 | 3 002 → 39 | 16.2 µs | 1.5 µs |
| 10 000 | 10 002 → 39 | 61.5 µs | **7.7 µs** |

**8× at 10 000 rows, and it is reported as its own row rather than
replacing the old one.** Quoting a culled paint against iced or Lumen —
neither of which culls — would be comparing forty rows of work with ten
thousand, which is the kind of comparison this milestone exists to stop
making.

`List` still materializes every element in *layout*, which is what
keeps it comparable with `GtkBox` and `QVBoxLayout`. Materializing is a
layout property; painting what cannot be seen is not comparability, it
is waste, and GTK and Qt both cull in their own paint paths.

Still open: the raster-side walk. With paint culling the command list is
short, so `render_impl`'s O(commands) scan no longer dominates — but an
application that legitimately emits thousands of *visible* commands
would still meet it.

---

# Phase D — the GPU crossover

## T16.13 — measure where the GPU wins, then close the gap

*Two findings. First: the GPU is slower than the software canvas on
**every damage-clipped frame measured** — 33–88 µs against 4.9–29.9 at
2 002 damaged pixels — while being 20× faster on a full 4K redraw. Both
are true and only one is documented. Second: on the all-rows workload
GPU raster grows 65.8 → 190.8 µs from 100 to 10 000 rows while the
damaged area is capped by the viewport and barely moves, which points at
a per-command CPU cost in instance building that the software path does
not pay.*

Three pieces:

- **Sweep the crossover** on the two variables that matter — damaged
  fraction of the surface, and command count — and publish the surface,
  not a single number. The answer a user needs is "at what damage
  fraction does the GPU start winning", and it is currently unknown.
- **Find the per-command cost.** `Gpu::collect` walks and culls exactly
  as the software renderer does, so the growth has another source;
  profile it rather than guessing.
- **Then decide whether the backend should choose itself.** An automatic
  switch on damage fraction would make "fastest depending on settings"
  true without the user knowing the crossover — but a backend that
  changes under load is a backend whose frame times are bimodal.
  **OPEN**, deliberately, until the surface from step one exists.

**Gate:** a crossover surface in `docs/results-gpu.md`; the per-command
cost either explained or fixed; the auto-switch question answered with a
measurement or explicitly deferred on a stated condition.

---

**PARTLY DONE 2026-09-08.** The surface is measured and published
(`docs/results-gpu.md`, "Where the curves cross"): **the crossover is
3–5% of a 1080p surface**, arriving earlier as command count rises. It
reconciles §5b — those frames damaged 0.6% of their surface, which is
CPU territory — rather than contradicting it.

Still open, and deliberately: the per-command cost is unprofiled, and
the auto-switch question is **not** answered. A backend that changes
under load has bimodal frame times, and whether that is better than a
consistently-slower one is a judgement nobody has measured. Deferred on
a stated condition: when an application exists whose damage fraction
crosses 3–5% regularly, measure both and decide then.

---

# Phase E — the mobile question

## T16.14 — decide what mobile means, or stop claiming it

*Finding: there is no Android or iOS target anywhere in the repository —
no cdylib entry point, no NDK or Xcode configuration, no
`winit::platform` use, no mobile CI. The toolchain has Android targets
installed from unrelated work, but with no linker configured even a bare
core cross-compile cannot link.*

The project's stated goal names desktop, mobile and embedded. Two of
those are measured; the third is not, and nothing in the tree supports a
claim about it.

This is a **decision task, not a code task.** Either:

- **commit**, in which case the smallest honest first step is a core
  cross-compile with a configured NDK linker, then a `cdylib` plus
  `android-activity` glue, then the same app packaged as an APK, then a
  device — and note that `dumpsys meminfo` does not report the same
  quantity as `/proc/<pid>/status`, so mobile RSS opens its own column
  rather than joining one; or
- **defer on a stated condition**, the way T15.12 was deferred — and
  then *correct the goal* wherever it is written down, because a goal
  that names a platform nobody can measure is a claim, not a target.

**Gate:** a decision record either way, and no document left claiming
mobile support that does not exist.

**DONE 2026-09-08 — `docs/mobile-decision.md`.** Deferred on a stated
condition (*when someone intends to ship an application on a phone*),
and **the goal corrected in the meantime**: desktop and embedded
measured, core portable to mobile and unproven there. A goal naming a
platform nobody can measure is a claim, not a target.

---

## Sequence

```
Phase A  T16.0 harness ─→ T16.1 per-package ─→ T16.2 damage ─→ T16.3 loud
             │                                      │
             ▼                                      ▼
Phase B  T16.4 competitive   T16.5 gpu doc    T16.6 doc gate
             │
             ▼
Phase C  T16.7 invalidation ─┬─→ T16.8 paint
                             ├─→ T16.9 layout + place  (RIGID_W/H decided here)
                             └─→ T16.10 bindings
         T16.11 amplification  ── independent, a decision doc
         T16.12 raster         ── independent of T16.7; needs T16.2's determinism

Phase D  T16.13 GPU crossover  ── independent; wants T16.12 done first
Phase E  T16.14 mobile         ── independent, any time
```

**T16.0–T16.3 gate everything.** T16.7 gates three tasks and is the only
one that can slip the milestone.

## Risks

- **T16.7 changes traversal, which every gate depends on.** The oracle
  requirement is the mitigation and it is not optional: `Everything`
  stays in the binary forever, exactly as `Rebuild` did.
- **T16.8 needs the previous frame's display list.** That is a memory
  cost on a framework whose distinguishing number is 24.6 MB resident.
  If it is large, the task's answer may be "retain per-subtree command
  ranges, not commands", which is a different and harder design — decide
  with a measurement before building either.
- **The dirty-set entry point is genuinely open.** Option (3) is
  conservative and correct; if it turns out to be *so* conservative that
  it dirties the root on every click, T16.8–T16.10 buy nothing and the
  milestone's headline number does not move. Measure the set's size on
  the tally and the 3 000-row list before building the consumers.
- **T16.6 will produce a backlog.** Extending the identifier gate to
  markdown across ~40 documents will surface dead names in quantity. The
  allowlist must not become the place drift goes to hide; entries need
  reasons, and a doc that needs many is a doc to re-read rather than to
  allowlist.
- **Phase A produces no user-visible improvement at all** and is three
  commits of work on a benchmark. That is the correct order and it will
  still feel wrong; the ordering principle at the top is the answer.

## What this plan does not do

- **It does not chase the ranking.** lumen-static already leads every
  measured arm at every size. Nothing here is aimed at a competitor's
  number; every task is aimed at a stage of our own frame.
- **It does not touch text shaping**, which is 92% of a full-redraw
  frame at 10 000 rows. That is the largest single cost in the whole
  profile and it is *correct* work — every row's text genuinely changed.
  Making it faster is a text-stack question (caching, a different
  shaper, run coalescing) and belongs in its own milestone with its own
  measurements.
- **It does not add a Slint frame arm**, or churn workloads for masonry,
  egui, Qt and GTK. Four arms have no all-rows workload and one has no
  frame harness at all; those are gaps in the comparison, recorded in
  `docs/results-competitive.md`, and filling them is harness work with
  no effect on this framework.
