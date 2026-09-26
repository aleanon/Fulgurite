# M41 — `Boundaries`, completed after the flip

> **T41.0–T41.1 DONE 2026-09-25** (Phase 1). **T41.2–T41.6 DONE
> 2026-09-25**; T41.2b (the `agent_drive.sh` legs) and T41.7 wait for
> M42.
>
> - **T41.2**: the oracle runs after paint, paints a scratch list through
>   `Scaled`, and names a list-only difference `field: "paint"`;
>   `tests/shadow.rs` shows it firing in three cases, under four
>   injections. Its first headless sweep found a divergence in showcase
>   (the DataGrid after a wheel, node 217), fixed by T41.3.
> - **T41.3**: `Repeat::generation`, bound from state, marks exactly the
>   repeater; rows, `List` items and `Dyn` islands get a view in their
>   own index space (`Changed::arena_view`). `just grid`'s edit frame
>   2.64 → 2.01 µs (`docs/results-damage-precision.md` T41.3).
>   `alloc_gate.rs`'s layout arms had been vacuous since M34's flip.
> - **T41.4**: `TextInput` and `TextArea` override `probe`; a sweep over
>   23 leaves shows no leaf's paint moves under a probe.
> - **T41.5**: `Host::paste_with`; `Session::paste` passes the session's
>   measurer (8 `m`s: 131.2 px, not 80).
> - **T41.6**: `table_is_well_formed` requires pre-order and a last
>   child's link of 0; the review's I3 now fails with 52 const errors.
>
> - **T41.0**: ten gates. Six red and ignored until their task —
>   compose (T41.1), the oracle's widget-internal case (T41.2), `Repeat`
>   data cases 2–3 (T41.3), the audit under `Boundaries` (T41.4), paste
>   (T41.5), the table counterexamples (T41.6). Four green, each with an
>   injection, closing I-NEW-2 and R1-F3's residue (`tick_marks.rs` and
>   `App::tick`'s contract), R18-F6 (`invalidate_model.rs`: 47 814
>   disagreements under mark-2), R18-F8 (clicks landed 3/3; I4a turns
>   both headline tests red), R5-F6 (`reload_oracle.rs`: 0 / 0, and
>   7 770 px on the sourceless arm with `reasked`'s mark deleted).
> - **T41.1**: `Host::compose` marks the focused subtree. Closes R1-F2
>   (S0) and R10-F9; the commit frame goes from x = None (nothing
>   painted) to −64, equal to `Everything`, and the host's writes are
>   audited in `docs/plan-boundaries-default.md` T34.0.
> - §6 records what building found.
>
> **Written 2026-09-24.** Closes **R1-F2** and **R10-F9**
> (one defect, now S0), **R1-F6**, **R1-F4**, **R1-F5**, **R2-F1**,
> **R10-F4**, **R2-F5**, **R1-F3**'s residue, **R18-F6**, **R18-F8**,
> **R5-F6**, and the re-triage's **I-NEW-1** and **I-NEW-2**. Sources:
> M38's deep review (`docs/results-deep-review.md`) and its re-triage
> against `main` at 1717808 on 2026-09-24. Where the two differ, this plan
> follows the re-triage.
>
> **Phase 2**, after M42 for its evidence, and running alongside M43.
> **Two tasks run now, in Phase 1**: T41.0 (the gates) and T41.1 (the
> compose mark). They are headless and need no drive harness, and T41.1
> closes an S0 in the default configuration. T41.2, the oracle redesign,
> waits for M40, because the oracle compares display lists painted
> through `Scaled`. **T41.7, the re-validation of M34's evidence, waits
> for M42**, because a drive result says nothing until the harness can
> say which binary it drove. M45 follows this milestone because of
> `Host::paste` (T41.5).
>
> **The review's order was "M41 before the flip", and the flip happened
> anyway.** R1-F2 was filed as a prerequisite of M34/T34.0, and R1-F6
> asked for the oracle to be redesigned before T34.2. T34.0's inventory
> (10b7362) missed `Host::compose`. T34.1's oracle (e176eb3) shipped
> exactly as R1-F6 predicted. T34.2 (6116a8b) flipped both defaults,
> citing "the shadow oracle quiet over 29 frames". **Those frames were
> judged by an oracle that repairs the frame it inspects** (§1.3). This
> plan fixes what the flip exposed, rebuilds the instrument, and then
> gathers the flip's evidence again. `docs/plan-boundaries-default.md`
> is amended in place, not rewritten (T41.2, T41.7). §2.1 asks whether
> the default should go back to `Everything` in the meantime.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and prints the measured quantity beside its bound.*

---

## 0. The rules for this milestone

1. **Every `Boundaries` gate compares against `Everything`, and asserts
   that its case can fail.** It asserts that the action landed
   (`handled`, a changed count, focus on the node meant). It also asserts
   that the `Everything` arm *moved*: the field scrolled, the row text
   changed. A case in which neither arm changes shows nothing about the
   strategy.
2. **A gate that judges `Boundaries` does not compile the shadow oracle
   in, unless it is a gate about the oracle.** Even redesigned, the
   oracle's second layout runs on the live widgets and repairs the frames
   *after* the one it judges (§2.2). A drive with the oracle in reports a
   defect once and then hides it.
3. **Every host method that writes the tree outside the pump's own mark
   sites marks what it wrote**, and the inventory of those methods lives
   in `docs/plan-boundaries-default.md`'s T34.0 table, where the missing
   `compose` row should have been.
4. **A drive result is evidence only when the harness can say what it
   drove**: strategy, features and pid, from M42. Until then a drive run
   is a smoke test.

## 1. What is wrong, at 1717808

### 1.1 Writes the host makes that mark nothing (R1-F2 = R10-F9, WORSE, now S0; R1-F3's residue; I-NEW-2)

**`Host::compose` never marks.** It calls `node_mut(i).compose(c)` and
returns (`crates/lumen-static-shell/src/lib.rs:1356-1363`). The window
routes `Ime::Preedit`, `Ime::Commit` and `Ime::Disabled` to
`Session::compose` (`crates/lumen-static-winit/src/lib.rs:742-744`,
`crates/lumen-static-shell/src/session.rs:299-302`), and the redraw that
follows pumps an empty queue. So nothing marks the field, and under
`Boundaries` its layout, which computes the scroll offset and the
clusters, is skipped. **WORSE since 6116a8b**, which made
`Boundaries<256>` the default for `Host` and for the window. R1-F2 and
R10-F9 are the same defect, seen from the frame and from the missing
test. **The re-triage's numbers at 1717808**: a `Fixed(70)` field, focus
asserted, and a commit wider than the field. `Everything` paints the text
at x = −66 (scrolled) and `Boundaries<256>` paints it at x = 8. The list
hash differs on 3 consecutive frames. A second probe
(`b_compose_marks.rs`) gives field widths of 16 → 16 under `Boundaries`
against 16 → 104 under `Everything`, with empty damage. Adding one
`mark_node` in `compose` makes all three frames equal. R10-F9's original
point also still stands: no test calls `Host::compose`, the path the
window uses.

**`Host::tick` marks now, and its contract has a residue** (R1-F3,
FIXED by 10b7362). `lib.rs:1327-1331` marks everything when a tick
reports a change with a positive `dt`. A tick that writes the tree and
returns `false`, or that writes on a zero `dt`, is still unmarked, and
`App::tick`'s rustdoc (`crates/lumen-static/src/event.rs:938-963`) does
not say that either is forbidden. **Correction to the re-triage**: it
says the window's first frame after an idle wait has a zero `dt`. The
window resets `last_frame` only on a hot swap (`winit/src/lib.rs:458`),
so the zero-`dt` frames are the first frame and the first frame after a
swap. An idle wait produces one long `dt`, which is M47's concern.

**`tick_marks.rs` passes when the tick never runs** (I-NEW-2). The
re-triage replaced `self.app.tick(dt_ms)` with `false` in `Host::tick`.
Both arms then drew an empty label with identical hashes, and the test
passed. That is the "setup silently did not happen" shape.

### 1.2 State that only layout writes, which a skipped layout never refreshes (R1-F4, WORSE reach; R1-F5; R2-F1 and R10-F4)

All three have one shape: a widget keeps state that paint reads, and
only `layout` refreshes it. Under `Everything` the next frame's layout
hides any wrong write. Under `Boundaries` an unmarked node keeps the
wrong value.

- **`Repeat` binds only inside layout** (R1-F4, S1). `repeat.rs:49-50`,
  `:138-139` and `:417` promise that "a data change needs no
  invalidation call". Under the default that promise is false. **The
  re-triage's numbers at 1717808**: a bind that reads a thread-local,
  changed from 0 to 100 with nothing queued. The unhandled pump gives
  unequal list hashes, and `Host::invalidate` on the list makes them
  equal. **Reach grew with M29/T29.6 (cc3c6b6)**: a split application's
  consumed click marks only the consumed subtree
  (`lib.rs:1063-1067`). In probe f4b a button's handler changes the data
  a `Repeat` in a sibling container reads. The click lands
  (`handled = true`, `clicks = 1`), `Boundaries` marks one range, and the
  list differs from `Everything`'s. A bind cannot capture, so global
  data is the only way a `Repeat` can show changing data. No shipped
  example does this, which is why this is S1 and not S0. The verifier's
  second defect, arena rows of differing heights keeping stale geometry
  after `Host::invalidate`, was not re-run.
- **The agent audit's probe writes a live `TextInput`** (R1-F5, S1). The
  provided `Node::probe` is `self.layout(..)`
  (`crates/lumen-static/src/widget.rs:258-268`). `TextInput` and
  `TextArea` do not override it, and their `layout` writes `self.scroll`
  from the limits it is given (`crates/lumen-static-widgets/src/input.rs:505-513`,
  `textarea.rs:490-495`). The audit probes every leaf at 1e6 and 2e6 px
  (`crates/lumen-static-agent/src/audit.rs:511-527`). **The re-triage's
  numbers at 1717808**: under `Boundaries` with the agent, the scrolled
  field paints at x = −148 and then at x = 8 on the second unrelated
  hover. `Everything` with the agent, and `Boundaries` without it, stay
  at −148. The widget drive's `Boundaries` leg is plain `agent`
  (`scripts/widget_drive.sh:51`), so this reaches a live gate today.
- **A paste lays out with the stand-in measurer** (R2-F1 and R10-F4, one
  mechanism). `Host::paste` pumps with `BlockMetrics` for every 64-char
  chunk and once at the end (`lib.rs:1422-1433`). `Session::paste`
  (`session.rs:294-297`) calls it, although the `Local` session owns
  the real `FontMetrics`. **The re-triage's numbers at 1717808**: eight
  pasted `m`s give w = 80 against 131.2 typed, and in R10-F4's case a
  click at x = 84.5 puts the caret at byte 19 of 19 against 9 typed. **New
  at 1717808**: under `Boundaries` the frame after a paste does not lay
  the field out again at all, because nothing marks it. So keying the
  memo on the measurer, the review's other half, is not enough on its
  own.

### 1.3 An oracle that repairs what it inspects (R1-F6, WORSE; I-NEW-1)

`Host::shadow_check` (`lib.rs:284-325`) lays out and places the **live**
widgets again under `Changed::ALL`, into a scratch copy of `states`, and
compares five groups of `NodeState` fields. It runs at `lib.rs:1242-1245`,
**before paint**. Two consequences follow, both predicted by R1-F6 before
T34.1 was built:

- **It is blind to widget-internal state.** A `TextInput`'s scroll and a
  `Repeat`'s row text are not in `NodeState`.
- **It repairs the frame it judges.** Its layout recomputes the scroll
  and re-runs the binds, and paint then draws the repaired widgets.

**The re-triage measured both at 1717808**, with
`--features shadow-oracle`. The compose, `Repeat` and audit-probe cases
each report `first: None`, **and** each case's list is then equal to
`Everything`'s. Without the feature, each list is unequal. **WORSE**,
because T34.2 cites this oracle as evidence, and because
`scripts/agent_drive.sh:152` runs its only `Boundaries` leg as
`agent,shadow-oracle`. So "29 frames compared, none diverged"
(`docs/plan-boundaries-default.md`, the T34.1 and T34.2 DONE blocks) was
measured on repaired frames.

**No test shows the oracle can fire** (I-NEW-1). The re-triage replaced
the scratch layout with `scratch.clone_from(&self.states)`, so the
oracle compared the states with themselves.
`crates/lumen-static-shell/tests/shadow.rs` still printed "compared 5
frames, first divergence None" and passed. The file's header says it
checks that the oracle names the node when a mark is wrong. Its one test
asserts only `compared > 0` and `first.is_none()`. The positive case
exists only in e176eb3's commit body. `agent_drive.sh:141-145` asserts
the same two things.

### 1.4 Gates whose setup can silently fail (R18-F6, R18-F8, R2-F5, R5-F6)

- **`Boundaries::mark` can shrink an earlier ancestor** (R18-F6, S2).
  Deleting `e = e.max(re);` (`crates/lumen-static/src/invalidate.rs:282`)
  passes the whole dev-shape suite at 1717808, including
  `--features shadow-oracle --test shadow`. **The re-triage keeps S2
  over the verifier's S3**, because this line now guards every window. A
  trap for the new test: `Boundaries::default()` starts all-dirty, so a
  test that does not call `clear()` passes under the mutant.
- **The invalidation oracle's headline tests do not check that their
  clicks land** (R18-F8, S3). `drive()` calls `host.click` at
  `crates/lumen-static-shell/tests/invalidation_oracle.rs:178` and
  asserts nothing. `the_shipped_boundaries_256_produces_the_same_frames_too`
  (`:264`), the test the flip cites for the shipped type, inherits the
  gap.
- **`table_is_well_formed` accepts tables whose `subtree_range` is
  wrong** (R2-F5, S2). `crates/lumen-static/src/widget.rs:408` allows a
  last child's next sibling to "point at a later node" (`:458-461`). The
  re-triage re-ran the review's four counterexamples at 1717808, and all
  four are still accepted. **The reach grew**: `subtree_range` now
  drives the default's precise marks (`lib.rs:1067`, and `mark_node` at
  `:1498-1505`). Not re-checked: whether M35's static observe walk
  still follows sibling chains, where the review's derive mutation
  crashed.
- **No test can see `adopt` or `restore` lose their mark** (R5-F6, S3,
  PARTIAL). `Host::reasked` (`lib.rs:459-466`) calls `set_stylesheet`,
  which marks everything at `:673`, and then marks everything again. The
  re-triage deleted `reasked`'s own mark: `reload_sheet.rs`,
  `program.rs` and `scripts/window_reload_demo.sh` all stayed green.
  **Found while writing this plan**: every one of those hosts has a sheet
  source. A host built with `Host::new` has none (`lib.rs:531`), and for
  it `reasked`'s own mark is the only one. So the injection should fire
  on a host without a sheet source. That is predicted from reading and
  has not been run.

## 2. Decisions

### 2.1 Should the default go back to `Everything` until M41 lands?

**The case for reverting.** The review asked for M41 before the flip,
and the flip shipped on evidence that §1.3 shows was gathered on repaired
frames. An S0 (compose) is in the default today. Two S1s are in the
default too, one of them in every agent build (R1-F5). Under `Everything`
all three disappear, because each is a skipped layout. The repository's
rule is that docs describe reality, and "the bounded strategy is the
default, on this evidence" is not reality until T41.7. Reverting is
mechanical: T34.2's edit list, backwards.

**The case against.** Reverting fixes nothing and re-hides everything.
M34 exists because four of M26's five Phase A defects were invisible
under `Everything`, and every defect in §1 was found *because* the
default had flipped. A revert moves every headless `Host<A>` back to the
strategy that cannot show them. It is also two churn commits over the
same dozen files, T34.2's and then its re-flip, and the handoff's
DECIDED marker would flip twice. The S0 is one precise mark with a
headless gate (T41.1), and it can land before any revert would. After
T41.1, the known defects left in the default need either global data
changed by a click in a sibling container (R1-F4, which no shipped
example does) or an agent build (R1-F5).

**Recommendation: keep `Boundaries` as the default, land T41.0 and
T41.1 now, out of phase, and set a tripwire.** Revert, using T34.2's
edit list backwards and recording it here, if either of these happens:

- T41.1 has not landed when M42 starts, or
- T41.7's re-validation finds a divergence in a shipped example that is
  not fixed in the same session.

Until T41.7 closes, the handoff's DECIDED marker carries one qualifying
line: "evidence under re-validation, `docs/plan-boundaries-default.md`
T34.2 as amended". That line is added in T41.2.

**What this breaks:** nothing now. If the tripwire fires, M29/T29.6's
narrowing stops mattering by default until the re-flip, as it did before
M34.

### 2.2 How the oracle inspects without repairing

The live oracle has to run in one host. `App` is not `Clone`, and
T34.1's reason still holds: a second host would run every handler and
spawn every task twice. Snapshotting and restoring the widgets is out
too, because widgets are not `Clone`. So any second layout runs on the
live widgets, and it changes what the *next* frame starts from. The
choice is which frame it is allowed to change.

| option | the frame it judges | the frames after |
|---|---|---|
| **today**: second layout before paint, `NodeState` only | repaired before it is painted, and widget state unseen | repaired |
| **A (recommended)**: after paint and `retain.finish`; scratch layout, place and paint into a scratch list through `Scaled` only; compare the lists, then `NodeState` | the bounded pass's own output, judged whole | repaired, and said so |
| B: lockstep second host (replay) | exact | exact, and each handler and task runs twice |

**Recommendation: A for the live oracle, and B for headless gates.** B
is already how `tick_marks.rs` works, and it is how every gate in this
plan is written. It is exact because a test's application has no side
effects outside itself.

Under A:

- **What is compared.** First the scratch list's `hash_from(0)` against
  the bounded list's. On a mismatch, the first differing command index,
  and the node whose paint emitted it. The scratch paint records each
  node's command range, so the attribution is a lookup. The `NodeState`
  comparison stays as a second, more specific field, and a new field
  value `"paint"` means the lists differed and the states agreed.
- **What is left alone.** Retention: the scratch paint goes through
  `Scaled` but not through `Recording`. The retained journal and
  `last_damage`. The overlays, which get a scratch overlay set.
- **What it cannot avoid.** The second pass leaves the widgets repaired
  for the *next* frame. The rustdoc says so. A defect shows up as one
  report on its first frame, not as pixels that persist. That is why
  rule 2 keeps an oracle-free `Boundaries` leg in every drive.

**What A breaks:** the oracle's cost roughly doubles, because a paint is
added to the layout and place it already pays. The oracle is a
diagnostic behind its own feature, so this is recorded and not gated.
`Divergence` gains no fields, only a new `field` value.

### 2.3 How a `Repeat` learns its data changed

**(a) A `Repeat` is never skipped.** The derive's skip check ORs in a
const for any container whose subtree holds a `Repeat`. This keeps the
promise in the rustdoc, and gives up M29's grid savings: the grid's body
is a `Repeat`, and T29's numbers would need re-measuring.

**(b) A data change is reported, and reporting it is cheap
(recommended).** The `Repeat` gains a generation counter that takes part
in binding. The state bumps it, M33's `#[bind]` source path writes it,
and T29.3's per-site marks then mark exactly that `Repeat`. A marked
`Repeat` lays out its arena under `Changed::ALL`, which also answers the
verifier's arena-geometry defect. `Host::invalidate` on the list stays
as the other way to report. The redesigned oracle turns a change that
was not reported into a `"paint"` divergence on its first frame.

**Recommendation: (b).** The verifier's reading settles it. `Repeat` is
not an exception to the contract that an unmarked write goes unseen
under `Boundaries`. It only claimed in a comment to be one. (b) makes the
comment true of the contract, rather than bending the strategy to fit
the comment. **What (b) breaks:** the rustdoc promise at `repeat.rs:49-50`,
`:138-139` and `:417`, and any application that relied on it (none of
the shipped examples do, since their binds depend only on the index).
The comments in `tests/alloc_gate.rs` that assume binds run on every
frame also change.

### 2.4 How a paste gets the real measurer

`Host::paste` gains a sibling that takes the caller's measurer and uses
`pump_with` for the chunk pumps and the final one. `paste` stays, as
that sibling called with `BlockMetrics`, for headless callers.
`Session::paste` for `Local` passes the session's own `FontMetrics`, and
the reload crate's hot session (`crates/lumen-static-reload/src/lib.rs:754`)
forwards in the same way. **Dropping the final pump**, which would leave
the last chunk for the caller's next frame, was considered and rejected.
Every headless caller reads the field straight after `paste` returns.
The measurer identity in the memo keys (the review's other half) is
M45's (§4).

## 3. Tasks

### T41.0 — the gates first

**Phase 1.** It is headless and needs neither M40 nor M42.

Every finding becomes a gate before any fix. A gate that is red at
1717808 lands with `#[ignore = "red until T41.n: <finding>"]`. The
commit body records its red output, run by name with `--ignored`, and
the fixing task deletes the `ignore`. A gate that is green at 1717808
records its injection's red output instead. Five findings are gates, or
gates plus a rustdoc contract, and close here.

| gate | finding | at 1717808 |
|---|---|---|
| `crates/lumen-static-shell/tests/compose_marks.rs`, in `tick_marks.rs`'s shape: a content-sized `TextInput` followed by a label, focused by a click with focus asserted, under the default `Host` and under `Host<_, Incremental, Everything>`. It commits a string wider than the field, then runs a preedit and a cancel, and repeats for a composite (`SearchField`). Per frame, for 3 frames, it prints both list hashes and the field's text x. It asserts `compose` returned `true`, and that `Everything`'s x is below the field's padding, so the case can fail. A short commit that does not scroll is a control. A routing test types Tab, a preedit, a commit and `as_str` through `Host::compose`, not through the widget's inherent method | R1-F2, R10-F9 | **red** |
| `tests/shadow.rs`, `the_oracle_names_a_widget_internal_divergence`: a test-only leaf whose layout reads a `Cell` into an internal offset that paint uses, and whose `NodeState` does not change. The `Cell` is changed between pumps with no mark. It asserts `compared > 0`, then that `first` names that leaf | R1-F6 | **red** (`first: None`) |
| `crates/lumen-static-shell/tests/repeat_data.rs`, three cases: (1) a global changed between pumps, with the change reported and with nothing reported; (2) the f4b case, a split application whose click in a sibling container changes the data; (3) rows of differing height after a data change. It prints marks, row text rects and hashes, and asserts `handled`, that the click counter moved, and that `Everything`'s row text changed | R1-F4 | **red** on (2) and (3) |
| `crates/lumen-static-agent/tests/audit_is_inert.rs`, a `Boundaries<256>` case: a content-sized `TextInput` and a fill-width `TextArea` holding scrolled text, driven in the window's order (pump, `frame_done`, an unrelated hover, pump) for 3 frames against the same session without `frame_done`. It prints the text x in both arms, and asserts that `Everything`'s x is scrolled and that the hover landed. Built without `shadow-oracle` (rule 2) | R1-F5 | **red** (x = 8 against −148) |
| `crates/lumen-static-shell/tests/paste_measurer.rs`, which promotes the review's `r2_paste.rs`: paste and type the same text, then `pump_with` a proportional measurer, under both strategies. It asserts equal width and cluster count and prints `paste w=.. typed w=..` beside `bound: equal`. It repeats with a 100-char paste, which crosses the 64-char chunk pump, and with R10-F4's click at x = 84.5, comparing caret bytes | R2-F1, R10-F4 | **red** |
| `crates/lumen-static/tests/table_invariants.rs`: the review's four counterexamples as negative cases, checked against descendants computed from parent links. Per case it prints the verdict beside the oracle's mismatch list | R2-F5 | **red** (all four accepted) |
| `tests/tick_marks.rs`: in the loop, assert the tick ran (the counter equals k, and the label's length equals k) and that the hash changed from the previous tick. Print `tick k: label len k, hash changed: true`. **With it, `App::tick`'s rustdoc states the contract**: a tick that writes the tree returns `true`, and never writes on a zero `dt` | I-NEW-2, R1-F3 | green. Injection (B), where `Host::tick` never calls the application, turns it red with "setup: the tick did not run". Deleting `mark_all` in `Host::tick` still turns it red at the equality. **Closes both** |
| `crates/lumen-static/tests/invalidate_model.rs`: `a_nested_mark_does_not_shrink_its_ancestor` starts from `clear()`, asserts clean as its setup, marks 2..10, then 3..4, then 20..21, and checks 5..6 and 9..10 dirty and 12..13 clean. `boundaries_matches_a_bitset_model` runs 1 000 seeded mark sequences against a reference bitset and prints `sequences 1000, disagreements 0` | R18-F6 | green. Deleting `invalidate.rs:282` fails both, and the model prints a count above 0. Omitting `clear()` fails the setup assertion instead of passing. **Closes R18-F6** |
| `invalidation_oracle.rs` `drive()`: before each step-0 click, record the count, then assert it moved by 1 and that `pump` returned `handled`. After the session assert the final count, and print `clicks landed 3/3, final count 2` for both headline tests | R18-F8 | green. The review's I4a (`Length::Fixed(220.0)` on the card) turns both headline tests red with "click 0 did not land". **Closes R18-F8** |
| `crates/lumen-static-shell/tests/reload_oracle.rs`: hover, press and open a popup, then `adopt` a second application whose sheet colour, label and handler differ. A persistent damage-driven canvas under `Boundaries<256>` is compared with a full render under `Everything`, and it prints `differing px: N / 0`. Two arms: a host with a sheet source, and a host built with `Host::new`, which has none. A `restore` arm is behind the `reload` feature | R5-F6 | expected green. Deleting `reasked`'s own mark must turn the sourceless arm red. For the sourced arm the injection is removing the mark `set_stylesheet` makes in `reasked`'s path. **If the sourceless arm is red at 1717808, that is a defect in `adopt`, and it gets its own task here before anything closes.** Otherwise **closes R5-F6** |

**Docs in the same commit:** the `App::tick` rustdoc (`event.rs:938-963`),
the headers of `tick_marks.rs` and `invalidation_oracle.rs`, the
rustdoc of `Boundaries::mark` (`invalidate.rs`, on what
the absorb loop guarantees), and the rustdoc of `Host::reasked`, which
says that its mark is the only one for a host without a sheet source.
**Effort:** M. **Depends on:** nothing.

### T41.1 — `compose` marks what it changed

**Phase 1**, straight after T41.0: it is the S0 in the default
configuration, and its gate is headless.

**Closes:** R1-F2, R10-F9.

**Change:** in `Host::compose` (`lib.rs:1356-1363`), keep the widget's
answer. When it is `true`, call `mark_node` on the focused index (the
same precise mark the pointer-state block uses, `lib.rs:1093-1105`), and
set `arenas_dirty`, as the consumed path does. **Then audit every
`&mut self` method on `Host` for the same omission**, and write the
result as rows in `docs/plan-boundaries-default.md`'s T34.0 table: the
method, what it writes, and what it marks. The seed list, from
`lib.rs` at 1717808, is `compose`, `paste`, `set_palette`,
`set_stylesheet`, `restyle_subtree`, `invalidate`, `invalidate_all`,
`adopt`, `restore`, `resize`, `set_scale`, `tick`, and outside `Host`,
the agent's `frame_done`, which probes (T41.4).

**Gate:** `compose_marks.rs` from T41.0, with its `ignore` removed.

**Injection:** delete the new mark. The `Boundaries` arm fires on the
commit frame, printing x = −66 against x = 8, and the `Everything` arm
stays green, which shows that the gate sees a strategy-specific defect.
125aa35's injection (both `Node::compose` impls return `false`) turns the
routing test red. **Accepts good input:** a compose with nothing focused
returns `false` and marks nothing (asserted from `last_marks`), and the
short commit is equal both ways with and without the fix.

**Docs in the same commit:** `Host::compose`'s rustdoc (it marks the
focused subtree); the T34.0 inventory rows above, with the compose row
naming this defect the way T34.0 named the tick;
`docs/00-handoff-static-tree.md` §11, beside the DECIDED marker;
`docs/plan-desktop.md` T14.8, on routing now being gated; `Node::compose`'s
rustdoc, if it says the host handles invalidation. **Effort:** S.
**Depends on:** T41.0.

### T41.2 — an oracle that judges the frame it did not touch

**Closes:** R1-F6, I-NEW-1.

**Change:** as in §2.2, option A. `shadow_check` moves after paint and
`retain.finish` in `pump_with`. It lays out, places and paints into a
scratch `DisplayList` through `Scaled` only, with scratch overlays. It
compares list hashes first and `NodeState` second, and it records the
first differing command and its node. `scripts/agent_drive.sh` gains a
plain `Boundaries` leg (`agent` alone) that asserts `app.shadow` answers
`supported: false`. The oracle leg becomes a third leg, so that no drive
can quietly acquire the oracle. Both legs declare their expected
features to M42's launcher, and the `supported` assertions are M42/T42.4's
per-leg check, not a second copy of it.

**Gate:** `tests/shadow.rs`, which becomes the standing test that the
oracle can fire. It has four cases:

- `a_correct_session_is_compared_and_quiet`, unchanged, for good input.
- `the_oracle_names_a_widget_internal_divergence` from T41.0, with its
  `ignore` removed. It must name the leaf with `field: "paint"`.
- `a_wrong_mark_is_named`: a direct write to `h.app.ui.title` with no
  mark (the canonical unreported write). It must name the label's node,
  with `size` or `paint`.
- `a_zero_dt_tick_write_is_named`: a tick that writes on a zero `dt`,
  against the contract T41.0 wrote down. It must be named on that frame.

Each case prints `divergence at frame F, node N ("<wid>"), field X`.

**Injection:** four, each against the mechanism.

1. Move the call back before paint. The widget-internal case goes quiet,
   and its `first.is_some()` assertion fires.
2. Drop the list comparison. The same case fires.
3. Replace the scratch layout with a clone of `states`. Both the
   widget-internal and the wrong-mark cases fire.
4. T34.1's own injection, skipping the precise hover mark. It is still
   named, with `colour` or `paint`.

**Accepts good input:** the correct session stays quiet with
`compared > 0`.

**Docs in the same commit:**

- `shadow_check`'s rustdoc: what it compares, when it runs, and that it
  repairs the frames after the one it judges.
- The header of `tests/shadow.rs`.
- The comments in `scripts/agent_drive.sh`.
- **`docs/plan-boundaries-default.md`**, amended in place. The T34.1 DONE
  block gains an "Amended 2026-…, M41/T41.2" paragraph: its claim that
  "Paint attributes … are resolved in layout, so this comparison covers
  them" is false for widget-internal state, and the second pass repaired
  the frame it judged. The T34.2 DONE block gains a line: its 29 frames
  were judged by that oracle and are under re-validation in T41.7. A new
  §5, "What the re-triage found (2026-09-24)", carries the table of what
  changed, as §4 carries the first review's.
- `docs/00-handoff-static-tree.md` §11: the DECIDED marker's qualifying
  line (§2.1).
- The vacuous-gate record, a row: "the shadow oracle (M34/T34.1): it
  re-ran the code it checked before the output it checked, so it
  repaired every defect it could see". It goes into the I-NEW-1 row of
  `docs/results-deep-review.md` §4's dated addendum (M43/T43.15 creates
  the addendum if this lands first), not into `AGENT.md`'s table, which
  M43 §2.8 keeps to one summary row that cites §4.

**Effort:** M. **Depends on:** T41.0, and M40's T40.1 and T40.3, so the
scratch paint goes through the corrected `Scaled` path; M42/T42.4, for
the per-leg `supported` check the new `agent_drive.sh` legs use.

**Split on 2026-09-25 (§6): T41.2 is the oracle and its headless gate;
T41.2b is the drive legs.** TODO(T41.2b, waits for M42/T42.4): give
`scripts/agent_drive.sh` a plain `Boundaries` leg (`agent` alone,
asserting `app.shadow` answers `supported: false`) and make the oracle
leg a third leg, both declaring their features to M42's launcher. Not
built here, because the `supported` assertions are T42.4's per-leg
identity check and a second copy of it would be the thing M42 exists to
remove. Until T41.2b lands, `agent_drive.sh`'s only `Boundaries` leg is
still `agent,shadow-oracle`, so no live drive judges `Boundaries`
without the oracle's repair of the frames after a defect (§0 rule 2).

**T41.2b done 2026-09-26.** `agent_drive.sh` runs three legs:
`Everything`, `Boundaries` built `agent` alone, and `Boundaries+oracle`
built `agent,shadow-oracle`; a leg name's `+oracle` suffix is dropped
before the launcher compares strategies. The per-leg `supported` check
moved out of the drive's heredoc into `scripts/live/shadow.py`, one copy
that T41.7 also gives the widget and tasks drives, and it runs whether
the drive passed or not, so a failing drive still reports what its
oracle saw. The copy is not a second identity check: the launcher's
feature check (T42.2) refuses a mis-built leg first, and `shadow.py` is
the drive asking the running process, as T42.4 had it.

### T41.3 — a `Repeat` whose data changed is marked

**Closes:** R1-F4.

**Change:** as in §2.3 (b). `Repeat` gets a generation counter that a
`#[bind]` can write, and T29.3's per-site marks carry it. When a
`Repeat` is marked, its arena is laid out under `Changed::ALL`. This is
the one generation on `Repeat`: M44/T44.2 reads the same counter for
its click latch, and adds only a bump when the count changes. The
rustdoc at `repeat.rs:49-50`, `:138-139` and `:417`, and on
`Repeat::bind`, says that a data change must be reported, either
through a bound generation or through `Host::invalidate`.

**Gate:** `repeat_data.rs` from T41.0, with its `ignore` removed. Case
(1)'s unreported arm stays in the file, asserting the stale frame, so
the contract is pinned rather than implied.

**Injection:** leave the `Repeat` out of the site table. Case (2) fires
with unequal hashes. Remove the arena's `Changed::ALL`, and case (3)
fires on row geometry. **Accepts good input:** a click with no data
change still narrows. The test prints the ranges marked and asserts one
range, which shows the fix did not fall back to `mark_all`.

**Docs in the same commit:** the `Repeat` rustdoc above;
`tests/alloc_gate.rs`'s comments; `docs/results-damage-precision.md`
T29.5 and T29.6, where they describe a repeater frame;
`docs/00-handoff-static-tree.md` §6 "Repeaters"; the book's chapter on
lists, if it repeats "no invalidation call".

**Effort:** M. **Depends on:** T41.0, T41.2 (the oracle names an
unreported change).

### T41.4 — a probe leaves no trace in what paint reads

**Closes:** R1-F5.

**Change:** `TextInput` and `TextArea` override `Node::probe`. The
override computes the size from the shaped run and writes neither the
scroll offset, nor the padding, nor the caret. If the memo is cold, it
shapes into a local. The provided `Node::probe`'s rustdoc says that a
widget whose `layout` writes state that paint reads must override it.

**Gate:** the `audit_is_inert.rs` `Boundaries` case from T41.0, with
its `ignore` removed. Beside it goes a sweep: for every leaf in the agent
tests' widget fixture, paint, probe at the audit's two limits, paint
again, and compare hashes. This catches the next widget with this shape,
which the two named cases would not.

**Injection:** delete `TextInput`'s override. The gate fires on the
second hover with x = 8 against −148. Delete `TextArea`'s override and
the gate fires on its vertical scroll. The sweep fires on both.
**Accepts good input:** `Everything` with the agent, and `Boundaries`
without it, print identical x throughout.

**Docs in the same commit:** `Node::probe`'s rustdoc; the `TextInput`
and `TextArea` rustdoc; `audit.rs`'s module docs, where it claims to
"leave no trace"; `docs/plan-review-fixes.md` F2.1; the agent part of
`docs/00-handoff-static-tree.md` §9.

**Effort:** S. **Depends on:** T41.0.

### T41.5 — a paste measures with the window's measurer

**Closes:** R2-F1, R10-F4.

**Change:** as in §2.4. `Host` gains a paste that takes a measurer
(`paste_with`). `paste` delegates to it with `BlockMetrics`.
`Session::paste` for `Local` passes the session's `FontMetrics`, and
the hot session forwards.

**Gate:** `paste_measurer.rs` from T41.0, with its `ignore` removed.
**Injection:** make the new method pump with `BlockMetrics`. The gate
fires at 80 against 131.2, under both strategies, because the memo keeps
the stand-in's box. **Accepts good input:** paste equals typing at 8
and at 100 characters, and the typed control is unchanged by the
injection.

**Docs in the same commit:** `Host::paste`'s rustdoc, whose claim of
"no second edit path" is true again; `Session::paste` in
`crates/lumen-static-shell/src/session.rs`; `docs/plan-desktop.md` T14.5.

**Effort:** S. **Depends on:** T41.0. **M45 depends on this task.**

### T41.6 — a malformed table does not compile

**Closes:** R2-F5.

**Change:** `table_is_well_formed` (`widget.rs:408`) gains the two
clauses from the review's probe e510bdd. The table is pre-order: node
j's parent is j − 1 or an ancestor of j − 1. And a last child's next
sibling is 0. The "or points at a later node" allowance and its comment
(`:458-461`) go. Every derived, spliced and `Assembled` table in the
workspace must still pass, and `cargo check --workspace --all-targets`
checks that, because the derive asserts the function on every
expansion.

**Gate:** the `table_invariants.rs` negative cases from T41.0, with
their `ignore` removed.

**Injection:** re-apply the review's derive mutation I3 (3051257), in
which the last child links to its parent's end. `cargo check --workspace
--all-targets` must fail with const-evaluation errors. **Accepts good
input:** without I3 the workspace checks with no const errors, and the
positive cases in `table_invariants.rs` pass.

**Docs in the same commit:** `table_is_well_formed`'s rustdoc; the
macros crate's claim that "the spliced table is well-formed"
(`crates/lumen-static-macros/src/lib.rs:29-31`);
`docs/verification-notes.md` Tier 0; the header of
`table_invariants.rs`.

**Effort:** S. **Depends on:** T41.0.

### T41.7 — gather M34's evidence again

**Closes:** nothing new. It re-validates T34.2, which the brief for
this milestone requires as its own task, and it carries the §2.1
tripwire.

**Change:** after M42, with T41.1–T41.6 landed, run every live drive
with the redesigned oracle and without it:

- `just agent-drive`, all three legs.
- `just widget-drive`, with a third leg built `agent,shadow-oracle`.
- `scripts/tasks_drive.sh`, with an oracle leg.
- Every example's live path that T34.0 listed.

For each run, record three things from the harness's M42 identity line:
the strategy, the features, and the pid. Also record the frames compared
and the divergences. **Every divergence becomes a defect, fixed in its
own commit with its own injection** and recorded in this plan, in the
manner of M26's Phase A. If there are none, the commit says how many
frames were compared across how many actions, so that "nothing" has a
size. Headless, run `cargo test --workspace --features
lumen-static-shell/shadow-oracle` once, and record the compared-frame
total.

**Gate:** each oracle leg asserts `supported`, then `compared > 0`, then
`first == None`, in that order. Each plain `Boundaries` leg asserts
`supported: false`.

**Injection:** build showcase with T34.1's hover-mark skip. The oracle
leg must name the hovered node, and the plain leg must still pass,
which shows that the two legs are different binaries. **Accepts good
input:** the unmodified build is quiet with `compared > 0`.

**Docs in the same commit:** `docs/plan-boundaries-default.md`'s T34.2
block gains "Re-validated 2026-…, M41/T41.7" with the figures, and the
§5 table from T41.2 is completed. In `docs/00-handoff-static-tree.md`
§11, the qualifying line comes off, or, if the tripwire fired, the
marker records the revert. `docs/results-damage-precision.md` T34.2
needs a change only if a fix moved a frame class it reports. Any frame
time this task records is the whole pump, from M53/T53.1's host timing,
which M53 lands early for this reason; the stage sum T34.2 used leaves
out the cascade (R6-F2).

**Effort:** M. **Depends on:** M42 (all of it), T41.1–T41.6; M53/T53.1
if it records a frame time.

## 4. Boundaries of this plan

- **Measurer identity in the text memo keys is M45's**: R2-F1's
  "defence in depth", R10-F4's second half, and R2-F6, R2-F10 and
  R10-F12, which share the key. T41.5 removes the one place a window
  mixes two measurers, and M45 makes the keys survive the next one.
  Dropping `limits.max_w` from `TextInput`'s shaped key
  (`input.rs:474-479`) is part of that key work too, and T41.4 does not
  touch the key.
- **An `input.compose` agent method is not planned** in M39–M55: M48
  adds no input method (its rule 3), and M45 builds on T41.1's headless
  gate. Without it no live drive can exercise IME, so T41.7 covers
  composition headlessly only, through T41.1's gate. Whether to add one
  is the owner's call.
- **R2-F9, which asks that `Assembled` tables call the same check, is
  M46's.** T41.6 tightens the check that R2-F9 would call.
- **`set_stylesheet` skipping an equal sheet** (R5-F6's option (b)) is
  not done. It is a performance change with no measurement behind it.
  The sourceless arm in T41.0 makes `reasked`'s mark visible without
  changing the product.
- **R1-F3 is FIXED by 10b7362.** Only the contract residue is here
  (T41.0), and the redesigned oracle is what would notice a violation
  (T41.2).
- **Owners that differ from the re-triage's suggestion, kept as
  `assign.json` has them.**
  - R2-F1 and R10-F4: the re-triage suggested M45. They stay here,
    because under `Boundaries` a stale box persists until something
    marks the field (§1.2).
  - R10-F9: the re-triage suggested promoting it to a T34.3. It is T41.1,
    which is the same thing under this milestone's number.
  - R5-F6: the re-triage suggested M39's gates. It stays here, because
    it is a `Boundaries` gate.
- **Not a change to what `Boundaries` marks** beyond the missing sites.
  Narrowing is M29.
- **Won't fix:** none.

## 5. Kill rules and risks

- **T41.3's kill rule, written before the build.** Run `just grid`,
  eight runs a side, on the edit frame and the idle frame. If the edit
  frame's median lies outside the before-arm's spread, the generation
  bind is marking more than the `Repeat`. Stop and find out why before
  landing, because (b) was chosen to keep M29's numbers.
- **What would stop the milestone:** T41.7 finding a class of divergence
  with no local fix, for example many widgets whose paint depends on
  state that only layout writes. The §2.1 tripwire then fires, the
  default reverts, and M41 is re-planned around that class.
- **Risk: the list comparison is noisy.** A rule that rewrites state
  after layout, such as the focus-ring rule that T34.1's first live run
  tripped over, would report a checker's divergence. *Revealed by:*
  `a_correct_session_is_compared_and_quiet`, and by T41.7's first run,
  which reads each divergence before fixing anything.
- **Risk: the tightened table check refuses a real table.** *Revealed
  by:* `cargo check --workspace --all-targets` in T41.6. The review's
  probe e510bdd reported that the workspace still checks.
- **Risk: the hot session does not pass a measurer.** *Revealed by:*
  `scripts/hot_window_drive.sh` pasting into a field after a swap, if
  M49 has made that drive reliable. Otherwise the gap is named in T41.5's
  commit.
- **Risk: M42 slips.** T41.7 waits, and the tripwire's first condition
  still applies.

## 6. What building found (2026-09-25)

Written while building T41.0 and T41.1, at b213ae0. The code under
`crates/`, `examples/` and `fixtures/` is unchanged since 1717808 (the
one difference is `scripts/doc-identifiers.allow`), so every finding
was re-checked as the re-triage left it and none had moved.

- **Two gates as specified could not fail, for one shared reason.** A
  derived container skips at the top of its *own* `layout`, and a leaf
  is laid out whenever its parent is. So the f4b case with the `Repeat`
  directly under the root (`repeat_data.rs` case 2) was green: the
  click marked the button, the root's range overlaps it, and the root
  laid its leaf `Repeat` out again. The audit case with the fields
  beside the hovered button was green for the same reason. Each gate now
  puts the victim in a sibling container, as the re-triage's own probes
  had, and says why in its fixture's doc comment. **The shape a
  `Boundaries` gate needs is "the write lands in a container the frame
  did not mark"**, and a fixture with one container cannot produce it.
- **The short-commit control is red without T41.1 too** (§3, T41.1's
  "accepts good input"). The field is content-sized, as T41.0 asks, so
  every commit changes its box: `Commit("ab")` gives w = 32 under
  `Everything` against a stale 16 under `Boundaries<256>`. The plan's
  "equal both ways with and without the fix" holds only for a fixed-width
  field. The control is kept, ignored with the others, and after T41.1 it
  shows the mark is not specific to a scroll.
- **The stale field paints no text at all**, which is worse than the
  re-triage's x = 8. An empty content-sized field is 16 px wide, all of it
  padding, so its inner rect has no area and `DisplayList::text_weighted`
  drops the run: the gate prints `x=None` for every `Boundaries` frame.
- **`TextInput` has no public cluster count**, so `paste_measurer.rs`
  compares what the clusters produce instead: the box, the painted text
  x (the scroll, which the caret's cluster x decides) and the caret byte
  a click lands on. Red at b213ae0: w = 80 pasted against 131.2 typed (8
  chars); x = −412 against −532 (100 chars); a click at 84.5 puts the
  caret at byte 19 of 19 pasted against 10 typed. Identical under both
  strategies, because the stale memo is served either way.
- **`TextArea`'s half of R1-F5 needs a measurer that reports clusters.**
  `BlockMetrics` reports none, so under it an area's caret never leaves
  line 0 and nothing scrolls vertically: the gate's setup assertion
  fired. `audit_is_inert.rs` carries a local `Blocks` measurer (the
  stand-in's advance and wrap, with clusters). With it the probe's 1e6 px
  width unwraps the paragraph, and the area paints at y = 41.2 against
  5.2.
- **R1-F4's arena-geometry defect was re-run, and it has a mechanism.**
  A `Repeat` lays its rows out with the frame's `LayoutCx`, whose
  `Changed` ranges are main-table indices, while a row's own indices are
  its arena slot's (`0..T::NODE_COUNT`). A derived row therefore asks
  whether an unrelated main-table range is clean, and after
  `Host::invalidate("list")` it keeps last frame's box: 40 px against
  224 (`repeat_data.rs` case 3). §2.3 (b)'s "a marked `Repeat` lays out
  its arena under `Changed::ALL`" is the fix, as written.
- **R5-F6's prediction held.** The sourceless arm is green at b213ae0, so
  there is no `adopt` defect. Deleting `reasked`'s own mark turns it red
  at 7 770 px, and the `restore` arm at 8 640 px over three frames. The
  sourced arm needs `set_stylesheet`'s mark deleted as well, and is then
  red at 11 520 px.
- **A pre-existing seam, not this milestone's:** `cargo test -p
  lumen-static-agent --test audit_is_inert` fails its three original tests
  at b213ae0 when run alone. `Label`'s `probe` override is behind
  `lumen-static-widgets/observe`, which only a workspace build unifies on.
  `just test` runs the workspace shape, so the gate is honest there. This
  is the same kind of seam as M30's `counter-window --drive2` finding.

### While building T41.2–T41.6 (2026-09-25, at 0091266)

- **T41.2's first injection fired where the plan did not predict.**
  Moving the call back before paint turns every case red at *setup*:
  before paint, `self.list` still holds the previous frame, so frame 0
  compares against an empty list. The plan's predicted failure — the
  widget-internal case going quiet because the second layout repaired
  it — needs T34.1's comparison too (no list check): with both, it fails
  `the oracle repaired the frame it judged`, the assertion added for
  exactly that shape.
- **The oracle names the deepest differing node, not the first.** A leaf
  that grows grows every ancestor, so the first `NodeState` difference
  in table order was the root (`a_wrong_mark_is_named` named node 0,
  `size`). It now descends the first differing chain to its deepest
  node.
- **The redesigned oracle found a divergence in a shipped example on its
  first headless sweep** (§2.1's tripwire condition). `cargo test
  --workspace --features lumen-static-shell/shadow-oracle` with
  `LUMEN_SHADOW_ABORT=1`: showcase's
  `a_wheel_over_the_grid_scrolls_it_on_the_stage` aborts with node 217
  (`grid-rows`, the DataGrid's `Repeat`), `field: "paint"`, on the wheel
  frame. The wheel is consumed, so the `Repeat`'s own range is marked
  (`Marks { all: false, ranges: 1 }`), the `Repeat` re-binds its rows to
  the scrolled window, and each row — a derived container in the
  `Repeat`'s arena — asks the frame's main-table ranges whether *its*
  arena indices are dirty, answers clean, and keeps last frame's layout.
  That is §6's arena-geometry mechanism (T41.3), reached by a wheel on a
  shipped example rather than by global data. T41.3 fixes it; the
  result is recorded there. The old oracle could not see it: its second
  layout ran before paint and re-laid the rows. The other aborts in the
  sweep are tests that write the tree unmarked on purpose (`loop_test`,
  `pointer_invalidation`, `invalidation_oracle`'s skip proof,
  `repeat_data`'s unreported arm, `shadow.rs`) and tests that count
  layouts or allocations (`palette_host`, `alloc_gate`), which a second
  layout changes by construction.

- **T41.3's rows got a view in their own index space, not only
  `Changed::ALL` when marked.** A row the repeater lays out unmarked —
  because a sibling changed, or the ring moved a new row into its slot —
  was also being asked about main-table ranges. So a row gets `ALL` when
  the repeater is marked, its `generation` moved, or its slot holds a new
  row, and `NONE` otherwise. The `NONE` half is what moved `just grid`'s
  edit frame from 2.64 to 2.01 µs, which fired §5's kill rule
  *downward*; the attribution is in `docs/results-damage-precision.md`
  T41.3.
- **The accept direction is zero ranges, not one.** A split
  application's click on a button whose hover is already set marks
  nothing: the handler writes only state, and the hover did not move.
  The click that changes the data marks two (the arriving hover and the
  list).
- **Two T41.3 gates had to be tightened by their own injections.**
  Making the repeater ignore its own mark left case 3 green, because the
  injection is not strategy-specific: `Everything`'s rows kept last
  frame's box too, and the two arms agreed about a stale row. Case 3 and
  case 4 now assert that `Everything`'s row *widened* (40 → 224 px).
- **`tests/alloc_gate.rs`'s layout arms had been vacuous since M34's
  flip.** Under the `Boundaries<256>` default an idle pump lays nothing
  out, so the repeater, `List`, `Dyn` and enum arms measured a frame in
  which none of their layout ran: T29.5's own injection (a bind that
  calls `format!`) stayed green. Those arms run under `Everything` now,
  and the injection fires again at 19 allocs/frame on a 19-row window.
- **`List` and `Dyn` had the same index-space mistake, and T41.3 fixed
  it there too.** Both handed their arena content the frame's `Changed`.
  `repeat_data.rs` case 5 (a `List` of container items after
  `Host::invalidate`) was red at 40 against 224 px; case 6 (a `Dyn`
  island) was *green* on its first draft, because the island sat at main
  index 2 and its mark, 2..3, happened to overlap the island's own arena
  indices 0..3. With two nodes ahead of it the injection fires (tail at
  x = 12 against 92). One helper, `Changed::arena_view`, now gives all
  three owners the same view.

## Appendix: finding → task

| ID | sev now | status | task |
|---|---|---|---|
| I-NEW-1 | S2 | OPEN | T41.2 |
| I-NEW-2 | S3 | OPEN | T41.0 |
| R1-F2 | S0 | WORSE | T41.1 |
| R1-F3 | S4 | FIXED (residue) | T41.0 |
| R1-F4 | S1 | WORSE | T41.3 |
| R1-F5 | S1 | OPEN | T41.4 |
| R1-F6 | S1 | WORSE | T41.2 |
| R10-F4 | S2 | OPEN | T41.5 |
| R10-F9 | S1 | WORSE | T41.1 |
| R18-F6 | S2 | OPEN | T41.0 |
| R18-F8 | S3 | OPEN | T41.0 |
| R2-F1 | S1 | OPEN | T41.5 |
| R2-F5 | S2 | OPEN | T41.6 |
| R5-F6 | S3 | PARTIAL | T41.0 |
