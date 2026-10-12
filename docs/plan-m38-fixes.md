# M39–M55 — fixing what M38 found

> **Written 2026-09-24; DONE 2026-09-27** on branch `m39-m55-fixes`,
> `just ci-verify 29a2fe7` green (28 steps, 0 failed, 0 skipped). Every
> task landed except T49.12, which waits for M56's showcase. Each plan's
> header records its results, and its "What building found" notes
> record where building overturned it. This is the index of seventeen
> fix plans, M39–M55. Together they close every finding of M38's deep
> review (`docs/results-deep-review.md`) that is still open, and seven
> found since. Each milestone has its own plan doc, with tasks, gates,
> injections and a finding → task appendix. This page gives the order,
> the decisions the plans make that the owner may want to overrule, and
> one table that maps every finding to its task.
>
> **The plans are written against `main` at 1717808, not against the
> review's `ac91d51`.** While M38 ran, thirty commits landed M32–M37 and
> M29/T29.2–T29.6, about 10k lines. So before any plan was written, every
> finding was re-triaged against 1717808 (§1). Where the review and the
> re-triage disagree, the plans follow the re-triage.

## 1. The re-triage

**Method.** Nine agents took one subsystem each. For every finding
marked S0 or S1, and every finding they would call FIXED, they did not
rely on reading: they cherry-picked or re-created the reviewer's probe on
1717808 and ran it. A FIXED finding whose fixing commit has a test that
does not catch *this* finding's repro was classed PARTIAL, because that
is the vacuous-gate shape AGENT.md warns about. Batch I also audited six
gates added since `ac91d51`, one injection each.

**Result.** The re-triage covers 189 records: the review's 182 findings
and seven new ones.

| status at 1717808 | count |
|---|---|
| OPEN | 165 |
| WORSE (reach or severity grew) | 14 |
| PARTIAL | 7 |
| FIXED | 3 |

**Fixed** (3):
- **R4-F1**, by M37: a live window now reloads itself.
- **R5-F5**, by M32: one styling source.
- **R1-F3**, by T34.0: `tick_marks.rs` catches its repro. Its dt=0
  residue stays in M41.

**S0 now** (3):
- **R1-F1**: damage is logical, but the surface is physical.
- **R14-F1**: any web page can drive a dev app. Re-tested at 1717808 on
  *both* agent servers, including the turnkey window's.
- **R1-F2**, which the re-triage raised from S1. `Host::compose` never
  marks, and since M34/T34.2 made Boundaries the default, an IME commit
  paints stale text in every window. R10-F9 is the same defect, found
  independently, and one task closes both.

**What the landed work changed.** Four findings of the review predicted
what the landed commits would do, and the re-triage found each one
happened:

1. **The flip landed before the repair.** The review put R1-F2 and R1-F6
   ahead of M34's flip. T34.2 flipped the default anyway.
   - T34.1's shadow oracle **repairs the frames it inspects** (R1-F6).
     The compose, `Repeat` and audit-probe defects all report `first:
     None` with the oracle compiled in.
   - `agent_drive.sh`'s only Boundaries leg compiles the oracle in, so
     the "29 frames quiet" that T34.2 cites was measured on repaired
     frames.
   - T29.6's narrowing also widened R1-F4's reach to ordinary clicks.
2. **The new gates were written under the old pressures.** Of the six
   new gates audited, four are vacuous:
   - I-NEW-1: the shadow oracle has no test showing it can fire.
   - I-NEW-2: `tick_marks.rs` passes when the tick never runs.
   - I-NEW-3: agent-drive's Boundaries leg passes with the oracle absent.
   - I-NEW-4: M36's golden runs in no gate.

   The mutants from R18-F6 survive the new M29/M33/M34 tests too.
3. **Replacement code rebuilt old shapes.**
   - M37's ABI stamp compares type *sizes*, not layout (R4-F5). The
     re-triage reordered two `Cmd` variants, rebuilt only the library,
     got "stamp agreed", and every command was misread.
   - `Reloadable::fresh()` still calls `Application::build`, while the
     window boots `program()`. That is two construction paths again
     (R5-F10).
   - Widget state is now lost on every live swap, not only in a headless
     fixture (R5-F2).
   - The reload crate's `unsafe` grew from 11 to 36 lines, and miri
     still reaches none of it (R4-F8).
4. **No embedded gate saw three regressions**, because the gated
   firmware never binds or dispatches:
   - F-NEW-1: `#[bind]` puts a static per binding in `.data`.
   - F-NEW-2: `bind::node_of` links every node's `hit` and `place`
     through the vtable.
   - F-NEW-3: M36's recovery code is linked into release windows built
     without `reload`, which contradicts `docs/plan-panic-recovery.md`
     rule 3.

**One correction the plan writers made to the review, worth knowing
before reading any plan.** `ui.lastDamage` and the drive harness's
"1.00× of the widget's box" come from the agent's journal of *logical*
node geometry, not from what retention presents. So M40's fix does not
change their units. It also means the 1.00× ratio celebrated in
M27/T27.3 **has never measured what the surface redraws** (M40 §1; M42
and M48 carry the consequence).

## 2. The milestones

| M | title | findings | S0/S1 | tasks | phase |
|---|---|---|---|---|---|
| M39 | Agent transport | 6 | R14-F1 | 8 | 1 |
| M40 | Physical-space damage | 5 | R1-F1 | 4 | 1 |
| M41 | Boundaries, completed after the flip | 14 | R1-F2≡R10-F9, R1-F4, R1-F5, R1-F6, R2-F1 | 8 | 2 (T41.0–T41.1 in 1) |
| M42 | Drive-harness and live-gate identity | 6 | — | 9 | 2 |
| M43 | Static gates, CI and dependency hygiene | 27 | — | 16 | 2 |
| M44 | Input semantics and keyboard reach | 15 | R3-F1, R10-F3 | 11 | 3 |
| M45 | Text correctness | 18 | R10-F1, R10-F2, R8-F4 | 13 | 3 |
| M46 | Hash identity | 6 | — | 4 | 3 |
| M47 | Timers, wake and window lifecycle | 11 | R7-F1, R7-F2 | 9 | 3 (T47.1 earlier) |
| M48 | Agent truthfulness and cost | 14 | R13-F1 | 15 | 3 |
| M49 | Reload hardening and state | 14 | — | 14 | 3 |
| M50 | Software raster and allocation truth | 9 | R8-F1 | 10 | 4 |
| M51 | GPU fidelity and footprint | 8 | — | 7 | 4 |
| M52 | Embedded and release-shape footprint | 8 | — | 8 | 4 |
| M53 | Frame-path cost | 5 | — | 5 | 4 |
| M54 | Accessibility and platform | 9 | — | 10 | 5 |
| M55 | Derive, lint and documentation DX | 12 | — | 9 | 5 |
| | **total** | **187** (+2 closed) | | **160** | |

The plan files:

| M | plan | M | plan |
|---|---|---|---|
| M39 | `docs/plan-agent-transport.md` | M48 | `docs/plan-agent-truth.md` |
| M40 | `docs/plan-physical-damage.md` | M49 | `docs/plan-reload-hardening.md` |
| M41 | `docs/plan-boundaries-repair.md` | M50 | `docs/plan-raster-allocation.md` |
| M42 | `docs/plan-drive-identity.md` | M51 | `docs/plan-gpu-fidelity.md` |
| M43 | `docs/plan-gate-hygiene.md` | M52 | `docs/plan-embedded-footprint.md` |
| M44 | `docs/plan-input-semantics.md` | M53 | `docs/plan-frame-cost.md` |
| M45 | `docs/plan-text-correctness.md` | M54 | `docs/plan-a11y-platform.md` |
| M46 | `docs/plan-hash-identity.md` | M55 | `docs/plan-derive-dx.md` |
| M47 | `docs/plan-timers-wake.md` | | |

The numbering is the review's own (`docs/results-deep-review.md` §8), kept
so that its citations still resolve. **The numbers are names, not the
order.** §3 gives the order.

## 3. Order

The phases are fixed. Within a phase, milestones run in parallel unless a
dependency below says otherwise. The consistency review built the
task-level dependency graph across all 160 tasks, and found **no
cycles**.

**Phase 1: the S0s, now.**
- **M39**, the agent transport. It closes R14-F1.
- **M40**, physical-space damage. It closes R1-F1.
- **M41's T41.0 and T41.1**, the gates and the compose mark. They close
  R1-F2/R10-F9, and they run now because they are headless and close an
  S0 in the default configuration.

**Phase 2: gates you can trust, then the repair.**
1. **M42** (drive-harness identity) lands before any drive evidence is
   gathered. Its T42.8 re-runs the "green both ways" evidence that ten
   commits cite, each at its own SHA.
2. **M41's T41.2**, the oracle redesign, needs M40. **T41.7** re-validates
   M34/T34.2's evidence and waits for all of M42.
3. **M43** (static gates and CI) runs alongside M41 and M42. It
   interleaves with M42 without a cycle: T43.0 → T42.0 → T42.2 → T43.13.
   Its `just ci` (T43.13) is the runner every later milestone's gates run
   in.

**Phase 3: correctness by subsystem, after M41 and M42.**
- M44 input, M45 text, M46 hash identity, M47 timers and wake, M48 agent
  truth, M49 reload.
- M45 follows M41 because of `paste_with` (T41.5).
- M48 follows M39, and T48.13 adds write methods only after T39.1.
- M44's T44.2 reuses M41/T41.3's `Repeat` generation.

**Phase 4: performance and footprint claims, after M43.**
- M50 raster and allocation, M51 GPU, M52 embedded and release shape,
  M53 frame cost.
- M52's first real task, the interactive firmware entering the size
  gates, stands on M43/T43.4.

**Phase 5:** M54 accessibility and platform, then M55 derive, lint and
documentation DX. M55's second `Local` parameter comes after M52/T52.7's.

**May land early.** Each of these depends on nothing it would jump, and
each plan says why it may go first:
- T44.7, T45.12, T46.3, T47.1, T50.1, T50.8, T51.3, T51.6, M48's T48.10
  and T48.11, and M53's T53.0 and T53.1.
- **T47.1**, a timer on an idle window never firing (R7-F1), is the one
  most worth pulling forward. It is five lines, touches no invalidation
  path, and was tested at 25–26 of 26 expected ticks by the re-triage.

## 4. Decisions the plans make that the owner may want to overrule

Each is decided, with its reasoning, in the named plan's §2. They are
collected here because each one changes behaviour a user or a later
milestone will see.

1. **Keep Boundaries as the default while M41 repairs it** (M41 §2.1).
   The review's order was repair first, then flip. The plan does not
   revert. It sets a tripwire instead: revert if T41.1 (the compose
   mark) has not landed when M42 starts, or if T41.7's re-validation
   finds a divergence in a shipped example that is not fixed in the same
   session.
2. **The agent transport closes on any non-JSON line, binds loopback
   only unless overridden, and defers a per-session token** (M39 §2). A
   token protects only against other local users, and every client would
   have to read it. The plan states the condition for reopening this.
3. **A skipped live gate exits 77 and counts as a failure** unless
   FULGURITE_ALLOW_SKIP=1 is set, and `just ci` runs in a clean worktree,
   because there is no git remote (M43 §2).
4. **Arabic and other complex scripts are honestly downgraded now.** The
   plan does not build a shaped painter. `ComplexText` stops claiming to
   paint what it cannot shape, and a shaped painter is proposed as its
   own milestone (M45 §2.1).
5. **Panic recovery becomes a type, not a runtime field**, so that a
   release window without `reload` links none of it. This amends M36's
   rule 3 (M52 §2.2).
6. **The hot-reload ABI stamp hashes the framework's source**, not type
   layouts. `offset_of!` into an enum variant is still E0658 on the
   pinned toolchain (M49 §2.1).
7. **The invalidation strategy becomes a default type parameter, not a
   feature.** Both `everything` features go, and an application picks
   its strategy in code. This is a breaking change to how a host type is
   spelled (M55, R12-F5).
8. **Accessibility is on by default on desktop, subject to a kill rule
   written before the build** (M54 §2).
   - It ships default-off if the idle frame moves outside its eight-run
     spread, or if the stripped showcase binary grows by more than 3 MB.
   - T15.7, T15.10 and T15.11 are un-marked from DONE rather than
     assembled.
9. **Palette accents change value.** No text-role field is added to
   `Palette`, because a new field would break every palette literal
   (M54 §2).

## 5. Rules every plan follows

These are AGENT.md's binding rules, applied to fix work. They are listed
so that a task can be checked against them without reading seventeen
docs.

- **T<n>.0 is always "the gates first."** Each finding becomes a test or
  check that goes red at 1717808, and its red state goes in the commit
  body. This follows M35/T35.0 and M36/T36.0.
- **Every gate names its injection.** The injection breaks the mechanism,
  not a value. The gate is also shown to accept good input.
- **Every quantity is printed beside its bound.** Numbers are stated once,
  in the owning results doc, and cited everywhere else.
- **Docs move in the same commit as the code.** When a fix falsifies a
  claim in the handoff, a results doc or rustdoc, that claim is fixed in
  the fixing commit.
- **Evidence that depends on the drive harness waits for M42.** Evidence
  that depends on the shadow oracle waits for M41/T41.2's standing
  can-fire test. After M43/T43.13, a milestone is not done until
  `just ci` is green at its last commit.

## 6. Kept current while two sessions work

Another session is working in this repository in parallel (M56, the
showcase redesign, on `m56-showcase-design`). It has already fixed
R3-F5 independently (`af902b9`, with its own injected gate). More of
this will happen. So, **every milestone's T<n>.0 re-checks its findings
at the then-current HEAD before it writes a gate**:

- A finding fixed elsewhere is recorded with the fixing commit.
- That commit's gate is run against *this* finding's repro. If the gate
  cannot catch it, the finding is PARTIAL, as in the re-triage (§1). The
  task shrinks to closing that gap.
- A finding whose code has moved is re-located before its gate is
  written. The re-triage found every M38 file:line stale where M32–M37
  had touched.

## 7. Gaps the consistency review found, and where they went

The review found five things every plan assumed and none owned. The
index assigned each one on 2026-09-25:

| gap | now owned by |
|---|---|
| an `input.compose` agent method, without which no live frame exercises R1-F2's path | M48/T48.13, after M41/T41.1–T41.2 |
| the bench lock that M44, M45, M46 and M55 cite | M43/T43.13 (`just bench-lock`) |
| a qemu and thumb-target capability probe for `just ci` | M43/T43.13 (exit 77 when absent) |
| a drive inventory for `ScrollView` | M44/T44.10 |
| the GPU atlas give-up count in `app.perf` | M51/T51.1, after M48/T48.3 |

## Appendix — every finding, its milestone and its task

One row per finding, generated from the plans' own appendices, so
this table and the plans cannot disagree by construction. `sev now` and
`status` are the re-triage's. R10-F9 and R1-F2 are one defect, closed
by one task.

| finding | sev now | status at 1717808 | milestone | task |
|---|---|---|---|---|
| R1-F1 | S0 | OPEN | M40 | T40.1 |
| R1-F2 | S0 | WORSE | M41 | T41.1 |
| R1-F3 | S4 | FIXED | M41 | T41.0 |
| R1-F4 | S1 | WORSE | M41 | T41.3 |
| R1-F5 | S1 | OPEN | M41 | T41.4 |
| R1-F6 | S1 | WORSE | M41 | T41.2 |
| R1-F7 | S3 | OPEN | M40 | T40.0 |
| R1-F8 | S2 | OPEN | M40 | T40.2 |
| R2-F1 | S1 | OPEN | M41 | T41.5 |
| R2-F2 | S2 | OPEN | M46 | T46.1 |
| R2-F3 | S2 | OPEN | M46 | T46.1 |
| R2-F4 | S3 | OPEN | M46 | T46.1 |
| R2-F5 | S2 | OPEN | M41 | T41.6 |
| R2-F6 | S2 | OPEN | M45 | T45.2 |
| R2-F7 | S3 | OPEN | M46 | T46.2 |
| R2-F8 | S4 | OPEN | M46 | T46.3 |
| R2-F9 | S4 | OPEN | M46 | T46.3 |
| R2-F10 | S4 | OPEN | M45 | T45.2 |
| R3-F1 | S1 | OPEN | M44 | T44.1 |
| R3-F2 | S2 | OPEN | M44 | T44.5 |
| R3-F3 | S2 | OPEN | M44 | T44.2 |
| R3-F4 | S2 | OPEN | M44 | T44.3 |
| R3-F5 | S2 | OPEN | M44 | T44.7 |
| R3-F6 | S2 | PARTIAL | M44 | T44.10 |
| R3-F7 | S2 | OPEN | M44 | T44.9 |
| R3-F8 | S3 | OPEN | M43 | T43.6 |
| R3-F9 | S3 | OPEN | M44 | T44.1 |
| R3-F10 | S3 | OPEN | M44 | T44.7 |
| R3-F11 | S4 | OPEN | M44 | T44.5 |
| R3-F12 | S3 | OPEN | M44 | T44.6 |
| R4-F2 | S2 | PARTIAL | M49 | T49.12 |
| R4-F3 | S2 | OPEN | M49 | T49.10 |
| R4-F4 | S2 | OPEN | M43 | T43.1 |
| R4-F5 | S3 | PARTIAL | M49 | T49.1 |
| R4-F6 | S4 | PARTIAL | M49 | T49.2 |
| R4-F7 | S3 | OPEN | M49 | T49.11 |
| R4-F8 | S3 | WORSE | M49 | T49.3 |
| R5-F1 | S3 | OPEN | M49 | T49.5 |
| R5-F2 | S2 | WORSE | M49 | T49.5 |
| R5-F3 | S3 | OPEN | M49 | T49.6 |
| R5-F4 | S3 | OPEN | M49 | T49.7 |
| R5-F6 | S3 | PARTIAL | M41 | T41.0 |
| R5-F7 | S3 | OPEN | M49 | T49.8 |
| R5-F8 | S3 | OPEN | M49 | T49.9 |
| R5-F9 | S4 | OPEN | M49 | T49.13 |
| R5-F10 | S3 | PARTIAL | M49 | T49.4 |
| R6-F1 | S2 | OPEN | M53 | T53.3 |
| R6-F2 | S2 | WORSE | M53 | T53.1 |
| R6-F3 | S2 | OPEN | M53 | T53.2 |
| R6-F4 | S3 | OPEN | M53 | T53.1 |
| R6-F5 | S3 | OPEN | M50 | T50.5 |
| R6-F6 | S4 | OPEN | M43 | T43.12 |
| R7-F1 | S1 | OPEN | M47 | T47.1 |
| R7-F2 | S1 | OPEN | M47 | T47.1 |
| R7-F3 | S2 | OPEN | M47 | T47.2 |
| R7-F4 | S2 | OPEN | M47 | T47.2 |
| R7-F5 | S2 | OPEN | M47 | T47.3 |
| R7-F6 | S2 | OPEN | M53 | T53.4 |
| R7-F7 | S2 | OPEN | M47 | T47.4 |
| R7-F8 | S3 | OPEN | M47 | T47.5 |
| R7-F9 | S3 | OPEN | M47 | T47.6 |
| R7-F10 | S4 | OPEN | M47 | T47.1 |
| R7-F11 | S4 | OPEN | M47 | T47.8 |
| R8-F1 | S1 | OPEN | M50 | T50.1 |
| R8-F2 | S3 | OPEN | M50 | T50.3 |
| R8-F3 | S2 | OPEN | M45 | T45.11 |
| R8-F4 | S1 | OPEN | M45 | T45.7 |
| R8-F5 | S2 | OPEN | M45 | T45.7 |
| R8-F6 | S3 | OPEN | M50 | T50.9 |
| R8-F7 | S3 | OPEN | M50 | T50.6 |
| R8-F8 | S3 | OPEN | M50 | T50.8 |
| R8-F9 | S4 | OPEN | M55 | T55.8 |
| R8-F10 | S4 | OPEN | M50 | T50.7 |
| R9-F1 | S2 | OPEN | M51 | T51.1 |
| R9-F2 | S2 | OPEN | M51 | T51.1 |
| R9-F3 | S2 | OPEN | M51 | T51.3 |
| R9-F4 | S2 | WORSE | M43 | T43.9 |
| R9-F5 | S2 | OPEN | M40 | T40.3 |
| R9-F6 | S2 | OPEN | M51 | T51.2 |
| R9-F7 | S2 | OPEN | M51 | T51.1 |
| R9-F8 | S3 | OPEN | M51 | T51.4 |
| R9-F9 | S3 | OPEN | M51 | T51.5 |
| R9-F10 | S3 | OPEN | M51 | T51.6 |
| R9-F11 | S4 | OPEN | M55 | T55.8 |
| R9-F12 | S4 | OPEN | M55 | T55.8 |
| R10-F1 | S1 | OPEN | M45 | T45.1 |
| R10-F2 | S1 | OPEN | M45 | T45.4 |
| R10-F3 | S1 | OPEN | M44 | T44.1 |
| R10-F4 | S2 | OPEN | M41 | T41.5 |
| R10-F5 | S2 | OPEN | M45 | T45.7 |
| R10-F6 | S2 | OPEN | M45 | T45.7 |
| R10-F7 | S2 | OPEN | M45 | T45.5 |
| R10-F8 | S2 | OPEN | M45 | T45.8 |
| R10-F9 | S1 | WORSE | M41 | T41.1 |
| R10-F10 | S2 | OPEN | M45 | T45.12 |
| R10-F11 | S3 | OPEN | M45 | T45.4 |
| R10-F12 | S2 | OPEN | M45 | T45.3 |
| R10-F13 | S2 | OPEN | M45 | T45.9 |
| R10-F14 | S3 | OPEN | M45 | T45.6 |
| R10-F15 | S3 | OPEN | M45 | T45.9 |
| R11-F1 | S2 | OPEN | M52 | T52.2 |
| R11-F2 | S2 | OPEN | M52 | T52.2 |
| R11-F3 | S2 | OPEN | M52 | T52.6 |
| R11-F4 | S2 | OPEN | M52 | T52.1 |
| R11-F5 | S2 | OPEN | M43 | T43.5 |
| R11-F6 | S3 | OPEN | M43 | T43.4 |
| R12-F1 | S2 | OPEN | M43 | T43.7 |
| R12-F2 | S2 | OPEN | M43 | T43.8 |
| R12-F3 | S3 | OPEN | M48 | T48.9 |
| R12-F4 | S2 | OPEN | M52 | T52.4 |
| R12-F5 | S3 | OPEN | M55 | T55.4 |
| R12-F6 | S4 | OPEN | M54 | T54.8 |
| R12-F7 | S4 | OPEN | M43 | T43.2 |
| R13-F1 | S1 | OPEN | M48 | T48.2 |
| R13-F2 | S2 | OPEN | M48 | T48.3 |
| R13-F3 | S2 | OPEN | M48 | T48.1 |
| R13-F4 | S2 | OPEN | M48 | T48.6 |
| R13-F5 | S2 | OPEN | M48 | T48.7 |
| R13-F6 | S2 | OPEN | M48 | T48.10 |
| R13-F7 | S2 | OPEN | M43 | T43.6 |
| R13-F8 | S2 | OPEN | M48 | T48.4 |
| R13-F9 | S3 | OPEN | M48 | T48.13 |
| R13-F10 | S3 | OPEN | M48 | T48.11 |
| R13-F11 | S4 | WORSE | M48 | T48.14 |
| R14-F1 | S0 | OPEN | M39 | T39.1 |
| R14-F2 | S2 | OPEN | M39 | T39.2 |
| R14-F3 | S2 | OPEN | M39 | T39.3 |
| R14-F4 | S2 | OPEN | M48 | T48.8 |
| R14-F5 | S3 | PARTIAL | M39 | T39.5 |
| R14-F6 | S3 | OPEN | M39 | T39.6 |
| R14-F7 | S3 | OPEN | M39 | T39.4 |
| R14-F8 | S3 | OPEN | M48 | T48.12 |
| R15-F1 | S2 | OPEN | M54 | T54.2 |
| R15-F2 | S2 | OPEN | M54 | T54.4 |
| R15-F3 | S2 | OPEN | M54 | T54.3 |
| R15-F4 | S2 | OPEN | M44 | T44.4 |
| R15-F5 | S2 | OPEN | M54 | T54.7 |
| R15-F6 | S3 | OPEN | M47 | T47.7 |
| R15-F7 | S3 | OPEN | M44 | T44.8 |
| R15-F8 | S3 | OPEN | M54 | T54.9 |
| R15-F9 | S4 | OPEN | M54 | T54.9 |
| R15-F10 | S3 | OPEN | M48 | T48.5 |
| R16-F1 | S2 | OPEN | M55 | T55.6 |
| R16-F2 | S3 | OPEN | M55 | T55.6 |
| R16-F3 | S3 | OPEN | M55 | T55.1 |
| R16-F4 | S3 | OPEN | M55 | T55.2 |
| R16-F5 | S3 | OPEN | M55 | T55.5 |
| R16-F6 | S3 | OPEN | M55 | T55.5 |
| R16-F7 | S2 | OPEN | M45 | T45.10 |
| R16-F8 | S2 | OPEN | M54 | T54.5 |
| R16-F9 | S3 | OPEN | M54 | T54.6 |
| R16-F10 | S3 | WORSE | M55 | T55.3 |
| R16-F11 | S4 | OPEN | M55 | T55.7 |
| R17-F1 | S2 | WORSE | M43 | T43.9 |
| R17-F2 | S3 | OPEN | M43 | T43.1 |
| R17-F3 | S3 | OPEN | M43 | T43.2 |
| R17-F4 | S3 | OPEN | M43 | T43.3 |
| R17-F5 | S3 | OPEN | M43 | T43.3 |
| R17-F6 | S2 | OPEN | M43 | T43.10 |
| R17-F7 | S3 | OPEN | M43 | T43.11 |
| R17-F8 | S2 | WORSE | M43 | T43.13 |
| R17-F9 | S3 | OPEN | M43 | T43.4 |
| R17-F10 | S3 | OPEN | M43 | T43.14 |
| R17-F11 | S3 | WORSE | M43 | T43.14 |
| R17-F12 | S3 | OPEN | M43 | T43.12 |
| R17-F13 | S3 | OPEN | M43 | T43.4 |
| R17-F14 | S4 | OPEN | M43 | T43.14 |
| R17-F15 | S3 | WORSE | M43 | T43.2 |
| R17-F16 | S4 | OPEN | M43 | T43.2 |
| R18-F1 | S2 | OPEN | M42 | T42.8 |
| R18-F2 | S2 | OPEN | M42 | T42.5 |
| R18-F3 | S3 | OPEN | M42 | T42.6 |
| R18-F4 | S2 | OPEN | M50 | T50.4 |
| R18-F5 | S3 | OPEN | M40 | T40.0 |
| R18-F6 | S2 | OPEN | M41 | T41.0 |
| R18-F7 | S3 | OPEN | M44 | T44.0 |
| R18-F8 | S3 | OPEN | M41 | T41.0 |
| R18-F9 | S2 | OPEN | M42 | T42.3 |
| R18-F10 | S3 | OPEN | M50 | T50.5 |
| R18-F11 | S4 | OPEN | M42 | T42.7 |
| I-NEW-1 | S2 | OPEN | M41 | T41.2 |
| I-NEW-2 | S3 | OPEN | M41 | T41.0 |
| I-NEW-3 | S2 | OPEN | M42 | T42.4 |
| I-NEW-4 | S3 | OPEN | M43 | T43.9 |
| F-NEW-1 | S3 | OPEN | M52 | T52.3 |
| F-NEW-2 | S3 | OPEN | M52 | T52.5 |
| F-NEW-3 | S3 | OPEN | M52 | T52.7 |
| R4-F1 | — | FIXED by M37 | — | none |
| R5-F5 | — | FIXED by M32 | — | none |
