# M38 — A deep review of the four goals, with every finding refuted before it is kept

> **Written 2026-09-23, not started.** This plan was synthesised from seven
> lens surveys (frame-path performance; embedded and configurability; core
> correctness; hot reload; the agent; public API and developer experience;
> verification infrastructure) and a completeness critic. It **reviews**;
> it does not build. Its output is a findings report, plus a table of
> changes to the queued milestones (M29/T29.6, M32–M37).
>
> The owner states four goals:
> (1) absolute peak performance and resource efficiency, best in class on
> embedded, desktop and mobile; (2) configurability through feature flags
> and generics, so that only what is used is compiled in; (3) a built-in
> agent that gives an LLM observability into the program under
> development, and control of it; (4) hot reload that cuts iteration time
> compared with a full Rust recompile.
>
> **The review's central hypothesis is taken from this repository's own
> history, not guessed.** AGENT.md lists twenty tests and gates that
> passed while proving nothing. Every one had one of two shapes: *the
> setup silently did not happen*, or *the bound was computed from the
> wrong quantity*. The surveys have already found candidates of both
> shapes in areas no one had looked at (the truetype raster path outside
> the zero-allocation gate; `app.perf` possibly reporting zeros that pass
> every budget; a `.text` ceiling 2.4× over its measurement; a drive
> harness that labels its strategy but never asks the binary for it).
> So the unit of work here is not "read and opine". It is **a claim, the
> thing that would falsify it, and the result.**
>
> **Read against `c8f4c5c`.** The lens surveys read the queued plans
> (M29, M32–M36) *before* that commit revised them after their own
> seven-lens review and added M37 (`docs/plan-window-reload.md`, in-window
> hot reload). Reviewers read the current text. A question premised on a
> superseded passage — for example R5's "T32.0's fn-pointer reload", which
> M32 now names rule 3's defect and has removed — is dropped under §0 rule
> 2, not answered against the old draft. R4 and R5 review M37's plan as
> the queued answer to their questions.

*Each task is one reviewer agent. Every finding is then attacked by at
least one verifier. Nothing in this milestone is committed to `main`.
Fixes are follow-up milestones, and the synthesis proposes them.*

---

## 0. The rules for this milestone

1. **A finding is a falsified claim or a demonstrated defect, never an
   opinion.** Each one quotes the claim (a doc, a comment, or a gate's
   name), gives the command that tests it, and gives the output. "X
   looks risky" is not a finding. At most it is a question left open.
2. **Premises first.** Each task begins by confirming the premises its
   questions rest on (for example: "winit passes `FrameTiming::default()`
   to `frame_done`"; "`rust-toolchain.toml` installs no thumb target";
   "winit/agent does not imply widgets/observe"). A false premise is
   reported as such, and the questions built on it are dropped.
3. **Injections are mandatory for any claim that a gate is sound or
   vacuous.** AGENT.md's three rules apply unchanged. The injection must
   break the mechanism. An injection that leaves a test green is a
   finding about the test. A gate that refuses must also be shown to
   accept.
4. **Print the quantity beside the bound.** Every measurement, and every
   bound a finding comments on, appears as `measured / bound` in the
   evidence.
5. **Eight runs a side** for any performance comparison. This is the
   M26 Phase D lesson: four runs could not separate a real effect twice.
   Report min, median and spread, and record the machine load at the
   time.
6. **Capability checks before hardware-dependent questions.** GPU
   adapter, display, HiDPI, IME, Android NDK, qemu: each task first
   checks whether the capability exists on this machine. When it does
   not, the question is recorded as **NOT EXECUTABLE**, never as passed.
   A review that skips silently is the V2 shape all over again.
7. **Ask what the fixture contains** (the M29 lesson), not only whether
   its assertions can fail: how many rows it materialised, which passes
   ran, who wrote the value.
8. **The main worktree is read-only.** Injections and scratch tests go in
   a per-task `git worktree add` under the session scratchpad, and are
   never committed to `main`. A reviewer may commit inside its own
   scratch worktree so its diff can be referenced by SHA.

## 1. Running cargo while other agents are running it

Up to 18 reviewers share one machine. The rules:

- **Targeted builds only.** Use `-p <crate>`, `--test <name>` and
  `--bin <name>`. Do not run `cargo build/test --workspace` except in
  R17's clean-container step. Never run `cargo clean`.
- **The main worktree's `target/`** is shared by read-only runs of
  existing tests. Cargo's lock serialises them. Expect waits, and do not
  kill another agent's cargo process.
- **Scratch worktrees** each have their own target dir (the default,
  `<worktree>/target`). This isolates injected code from everyone else.
  Budget one scratch worktree per reviewer, and reuse it across
  injections by reverting, not by re-cloning.
- **Separate target dirs for the matrix and cross-target builds:**
  R11 uses `target/review-R11`; R12 uses `target/review-R12` and
  `--no-dev-deps`. This stops feature churn from invalidating shared
  artifacts.
- **Benches only in R6, R8, R9, R2 (hash cost), R3 (hit test), R14
  (frame_done) and R16 (widget scaling).** Every timed run takes an
  exclusive lock, `flock <scratchpad>/bench.lock <cmd>`, and records
  `uptime` load averages before and after. A timing taken while another
  agent was compiling is marked **LOADED** and cannot support a finding on
  its own.
- **R18's load test** (repeated `just test` under stress) runs last, or
  holds the bench lock for its whole window.
- **Live windows:** each task uses its own agent port
  (`76{task number}`, e.g. R13 → 7613) and its own log path under the
  scratchpad. Both `LUMEN_AGENT` and `LUMEN_AGENT_ADDR` are set, because
  the launchers and clients disagree (R13 checks why). Before driving,
  check that the port answers **your** process (compare the pid from
  `ss -ltnp`).
- **Time budget:** 20 minutes of wall clock per cargo-hack crate
  invocation, and 30 minutes per cargo-mutants function set. An
  unfinished run is reported with its partial result.

### Tooling on this machine (checked 2026-09-23)

- **Not installed:** `cargo-hack`, `cargo-mutants`, `sccache`. Reviewers do
  not install tools; a feature powerset is a shell loop over
  `--no-default-features --features …`, and a mutant is a hand edit in the
  scratch worktree. Present: `cargo-deny`, `cargo-audit`,
  `qemu-system-arm`, and the `thumbv7em-none-eabihf`, Android and wasm32
  targets.
- **Seeding a scratch worktree:** `git worktree add --detach <path> HEAD`,
  then `cp -a <main>/target <path>/target` before the first build. Registry
  dependencies (wgpu, winit, fontdue …) are reused from the copy; only the
  workspace crates recompile.
- **Timings under load:** prefer load-insensitive quantities (allocation
  counts, call counts, bytes, `perf stat -e instructions` where available)
  over wall clock whenever the claim allows it.

## 2. Finding format

```
ID:        R<task>-F<n>
Title:     one line, stating the claim that failed
Severity:  S0 | S1 | S2 | S3 | S4        Category: correctness | perf-claim |
           vacuous-gate | security | safety | doc-drift | config | dx | a11y
Goal:      (1)…(4) affected
Claim:     the sentence tested, quoted, with file:line or doc §
Premise:   verified? (yes / false — then this is a doc-drift finding)
Evidence:  commands run; output; quantity beside bound; configuration
           (features, strategy, backend, profile, target, load)
Injection: what was broken, and what went red or stayed green (required for
           any gate/test claim)
Repro:     minimal steps from a clean checkout at <SHA>
Scope:     who is affected (turnkey window / embedded / dev only / tests)
Fix sketch and owning milestone: e.g. "M34 prerequisite", "new M39"
Confidence: high / medium / low, and why
```

### Severity scale

| | meaning | examples of the shape |
|---|---|---|
| **S0** | Memory-safety UB, a security exposure reachable in a default or documented configuration, or silently wrong pixels or state in the **default** shipped configuration | `from_raw_parts(null,0)` on a documented path; an agent reachable from a browser in a documented setup |
| **S1** | A goal-level claim is false; a gate is vacuous for the property it names; a correctness defect in a configuration a queued milestone will make the default | Boundaries skipping a tick-driven repaint before M34; "raster allocates nothing" false on the truetype path; no windowed hot reload while the handoff says DONE |
| **S2** | A defect or overstated claim with a workaround or limited reach | a lint that fires on the wrong tree; a stale doc number |
| **S3** | A robustness or DX gap with no current consequence | an unhelpful derive error |
| **S4** | A note for the record | a stale comment with no behavioural effect |

## 3. Verification protocol

Every finding goes through an **adversarial verifier**: a fresh agent
that did not write it, whose job is to **refute** it. The verifier:

1. Re-runs the repro from a clean scratch worktree at the stated SHA.
2. Checks the finding against both vacuous shapes: did the setup
   actually happen, and is the bound or quantity the right one?
3. Looks for a doc, comment or decision record that makes the behaviour
   intended. If one exists, the finding is downgraded to doc-drift or
   refuted.
4. For injection-based findings: re-applies the injection and confirms
   the reported red or green result, **and** confirms that the injection
   broke the mechanism rather than changing a value (AGENT.md rule 1).
5. Returns **CONFIRMED**, **PLAUSIBLE** (reproduced, with the severity or
   attribution disputed) or **REFUTED**, with its own evidence.

**S0 and S1 findings get two verifiers with different lenses:**

- **Reproducer:** re-derives the result from scratch, using a different
  method where possible (for example measurement where the author used
  reading, or a different fixture).
- **Attribution skeptic:** accepts the number and attacks the story.
  Is it the nearest plausible mechanism? Split the quantity (the T29.5
  lesson: the 12.2 µs was real, but 10.8 µs of it was the fixture's
  `format!`).

A finding survives if at least one verifier CONFIRMs and none REFUTEs.
When the two verifiers disagree, a third verifier is assigned to that
point alone. REFUTED findings are kept in an appendix with the reason,
because an argument that nearly held is worth recording (the plan-liquid
§12 precedent).

Performance findings also need eight runs a side under the bench lock,
repeated by the verifier. **A claim that two arms disagree is first
treated as a fact about the arms** (Phase D, finding 9: code alignment).

## 4. The tasks

| id | task | merges | priority | cargo |
|---|---|---|---|---|
| R1 | Boundaries completeness and the cross-axis shadow oracle | C1, X2, DX-9 (palette), X5 (press tick) | high | yes |
| R2 | Table structure, identity by hash, memo keys | C2, C3, C9 (keys) | high | yes |
| R3 | Input, focus, keyboard, overlays, row identity, queue overflow | C4, DX-2, P10, AG9, C6 (probe/paint) | high | yes |
| R4 | Hot reload reachability, iteration time, dylib seam safety | HR1, HR2, HR3, HR7, C5, V7 | high | yes |
| R5 | Reload state semantics, `Host::adopt`, construction/stylesheet paths | HR4, HR5, HR6, HR8, DX-4, HR9 | high | yes |
| R6 | Frame-path performance truth | P1, P3, P4, P8, X10 (competitors) | high | yes |
| R7 | Idle/wake/power, tasks/timers, threads, window lifecycle | P2, C8, X11, X7 | high | yes |
| R8 | Software raster, text raster, memory | P5, P7, P9, P10 (latency) | high | yes |
| R9 | GPU, cross-renderer fidelity, Glass | P6, X4, X5 | medium | yes |
| R10 | Text pipeline and text entry | X1, DX-7, DX-3 (IME/clipboard) | high | yes |
| R11 | Embedded budgets, stack, panics, const-eval, end-to-end loop | EMB-1, EMB-2, X8, CT-1, C6 (stack) | high | yes |
| R12 | Configurability matrix | CFG-1, CFG-2, V10, TIER-1, PAY-1, MONO-1, MOB-1 (build) | high | yes |
| R13 | Agent truthfulness | AG1, AG2, AG5, AG7, AG8, AG9 (fill) | high | yes |
| R14 | Agent security, transport, cost | AG3, AG4, AG6 | high | yes |
| R15 | Platform, a11y, mobile lifecycle, capability modules, images | DX-1, DX-3, MOB-1, X6, X12 | medium | yes |
| R16 | Widgets, lints, derives, numeric hygiene, scaling, book | DX-5, DX-6, DX-8, C7, C9, X9, DX-9 | medium | yes |
| R17 | Gate reality, silent skips, drift-check self-tests, deps, number currency | V1, V2, V3, V8, V9 | high | yes |
| R18 | The next vacuous tests, drive harness, mutants, flakiness | V4, V5, V6 | high | yes |

X3 (the queued designs) is not a reviewer task. It is **Phase 3** of the
synthesis, because it can only be done once the findings exist.

The full questions, methods and evidence requirements for each task are
in **Appendix A**, verbatim from the task records this plan was
generated with; each reviewer is handed its record as written. Each
task's key questions are summarised below.

### R1 — Boundaries completeness
**Premise to confirm:** `lumen-static-winit` ignores `host.tick`'s
result, and `Host::tick` never marks. Then build the table of every Host
method and frame-loop call against its mark calls. Build the shadow
oracle that M34/T34.1 promises, *ahead of M34*: one scripted sequence
through `Boundaries<256>+Incremental` and `Everything+Rebuild`, single
and banded, byte-compared on every frame. The script includes tick with
dt>0, compose, a timer, an inbox apply, row hover, wheel, `set_palette`,
`set_scale` and reload. Every step asserts that it landed. Axis-pair
matrix: strategy × {overlay, glass backdrop, repeater, bands, GPU
regions, scale, palette, reload, agent read}. Injections: `mark` as a
no-op; the PointerFocus block as a no-op; drop Glass `observe_dep`; skip
the scale memo invalidation.

### R2 — Table and hash identity
Does `subtree_range(t,i) == i..i+size(i)` hold for every derived table
in the workspace? Construct a table that passes `table_is_well_formed`
but has a wrong `subtree_range`. FoldedText with `"x"`/`"\0x"`, then a
same-length solver-derived `fold` collision within `Cmd::Text` limits,
through both the memo and retention. List every input missing from
`MeasureKey`. Price a finalised hash at eight runs a side.

### R3 — Input and focus
Press row k, remove row k−1 through the inbox, then release. Capture on
a node that becomes hidden. A `{TextInput, Button}` row with the Left
arrow. Keyboard reachability of every arena row. A Modal focus trap.
Radio groups larger than `Small`. InputQueue overflow accounting, and
paste's hard-coded `CAP`. Hit-test cost at 10 000 rows. Probe versus
paint at count=50M. The event sequence of an agent click versus an
xdotool click. Where `input.fill` sends its keystrokes when focus fails.

### R4 — Hot reload and the dylib seam
Is there a windowed reload path at all? Quote every doc that implies
one. Edit-to-.so and edit-to-pixel times, five runs each, printed
against the 2 s criterion (hot-app, and showcase compiled as a cdylib).
For each hot-host assertion: the framework-side injection that fires it,
or "none". A v3 shape change, and 100 alternating swaps (RSS,
`/proc/self/maps`). The null/0 contract of `hot_restore`; panics in v2;
a missing symbol. The statics/TLS classification, with a demonstration
that `take_log` returns empty on the host. A port collision between two
images. The `check_no_unsafe` bypass, then fixed in both directions.
Miri retried on the installed nightlies.

### R5 — Reload state and construction paths
A Reloadable coverage table for every stateful widget. Showcase with
`#[derive(Reloadable)]`: the errors. A schema-change table (added
subfield, rename, narrowing, NaN, a Null from a failed save). A table of
Host fields against `adopt`, and the adopt oracle under both strategies.
Program-without-`.style()` against `Application::style`, by pixel colour.
E0119 against a hand-written `Default`. Whether T32.0's fn-pointer
reload is sound across a dylib. Can the reload surface be compiled out
with `cfg_attr`? Serde_json symbol counts with reload on and off.

### R6 — Frame-path performance truth
A feature table per example and arm, from `cargo metadata`. A step-diff
table of each published figure's timed closure against `pump_with`.
Full-pump cost at 1k/3k/10k for the idle, move, hover and bound-label
frames, split per stage. Showcase Toggle click and Select open, with and
without observe, under both strategies. `cascade_arenas_deep` call counts
per click. `gpu.wait()` placement. A unique-string raster rerun. The
per-frame cost of `thread::scope`. Competitor configurations.
Injections: slow `place`/`finish` by 10×; force `arenas_dirty`.

### R7 — Idle, wake, lifecycle
60 s idle / focused-input / mouse-move windows, sampled from /proc,
counting frames. A timer due at 0 forever, and a NaN deadline. tick(1e9),
tick(NaN). Outbox overflow accounting. A worker panic mid-chain. Whether
the agent thread can hang. wake-gate with the waker install removed.
0×0 minimise, surface loss, close with tasks running, resize storms.

### R8 — Software raster and memory
Alloc gate on the truetype path (predicted red). Shaping calls per text
run under banding. Multi-region walk cost; caret-sized damage against
the full present. SIMD coverage by Cmd kind. `size_of` NodeState, Style
and Cmd, plus a gate with its injection. VmHWM and heap peak at 1k, 3k
and 10k rows and at 4K. Allocations during a resize drag. DisplayList
capacity after a large paste. Allocations per input event.

### R9 — GPU, fidelity, Glass
Adapter check first. Allocations per GPU frame; bytes uploaded for caret
versus full damage; how long present blocks; atlas resets. A Cmd-variant
× renderer-pair parity matrix with max diffs printed beside tolerances.
sRGB versus linear blending. Fractional-scale snapping. Re-derive
glass_cost's +18% and 1.44 ms. The panel count at which Damage::All
kicks in.

### R10 — Text pipeline and entry
ComplexText measurement against the software painter's ink, for Arabic
and mixed bidi (predicted mismatch). Can a turnkey window use
ComplexText? Callers of `Direction`. Ink containment per
measurer/painter pair. Compose and grapheme deletion per text widget.
Word motion and multi-click. Ctrl+C swallowed with no selection. A
100 kB paste. Where the IME candidate window appears.

### R11 — Embedded budgets
Does `f32::clamp` get past `check_text.sh`? Classify every threshold as
gated or prose. Qemu stack painting against `stack_bytes(DEPTH)`, in
debug and release, including `#[bind]`, TextInput and nested `Dyn`. An
interactive firmware variant. Panic bytes, and the profile matrix. The
smallest no_std loop and its compile errors. const-budget's const-eval
share, and a quadratic injection.

### R12 — Configurability
Observe leaking into release-shape (the rustc `-v` flags, and a
`compile_error!` injection). A depth-2 powerset per crate with
`--no-dev-deps`. The widgets alloc tier on thumb. The
winit/agent-without-widgets/observe getTree. Non-additive `boundaries`
and `rdyn_opaque`. Core and widgets tests on the alloc and none tiers,
and char-boundary truncation. Vtable-linked methods in the firmware, and
fuse. Marginal .text per capability. Linked-binary monomorphization.
`cargo check` for Android and iOS.

### R13 — Agent truthfulness
Confirm the `FrameTiming::default()` and `settled:true` premises. The two
frame-number domains over a click → lastDamage race. Real values from
`app.perf`. waitSettled during an animation or timer. Whether `handled`
is faithful. A per-widget role/value table. Whether the live_nodes
residue case can be constructed. `input.fill` into a repeater row. A
method set diff across endpoint, counter-window, skill and plan. The env
var matrix. Whether replay asserts per step, and whether the generated
Rust compiles.

### R14 — Agent security and transport
Threat model: local development only. Pass criteria: a non-loopback bind
is refused or warned about, a release build cannot enable the endpoint
without an explicit opt-in, and a browser-originated request cannot
execute a method. Checks: a `nc`/`curl` HTTP-preamble probe; a 0.0.0.0
bind; one idle client blocking another; a newline-free flood; an invalid
escape in the error reply; a panic in dispatch; non-finite coordinates;
array params and batch requests. `frame_done` cost live, and the idle
cost with no client. A symbol/size gate for non-agent release builds.

### R15 — Platform and a11y
The a11y reality (a grep plus a per-widget table) against the handoff's
claim. Multi-window reachability. A ScaleFactorChanged resize. resumed
called twice; touch-only drive. Gesture, drag, dnd and dialog assembly;
dialog stalls. `Drag<()>` size. ImageSource reachability, and the
missing `image` feature.

### R16 — Widgets, lints, derives, book
A positive/negative matrix for every W10xx lint. Ten beginner mistakes,
each with its compiler or runtime message verbatim. Hygiene of the
macro's locals. Silently accepted attribute misuse. Compile-fail tests
checked against their own reason. Contrast and target size per widget.
A planted appearance defect against widget-drive. Driving each const
KIND. Style NaN/inf fuzzing. Numeric range handling. Growth ratios for
TextArea, Markdown, Chart, Tree and DataGrid. Selector grammar parity.
Book absence claims and coverage.

### R17 — Gate reality
Has CI ever run? The first failing step in a clean container. A
recipe → automatic/manual table. Zero-test binaries per build shape. A
green-while-skipping count with no font, adapter or display. One planted
violation per drift script. `cargo deny`/`audit`; duplicates per
shipping profile; the thumb closure. A **fixed list** of numeric claims,
re-derived mechanically.

### R18 — The next vacuous tests
compile_fail doctests with their mechanism deleted. Thread-blind
allocation counters under `threads`. The shell suite with debug
assertions on. Oracles that never assert their action landed.
Strategy-blind widget-drive (identical binaries; a leftover process on
the port). Inventory coverage against exports. ui_check headroom.
live_gate's `|| true`. Bounded cargo-mutants on six named functions.
Flake rates under load, and nextest against libtest.

## 5. Phases

1. **Phase 1 (parallel):** R1–R17. Throughput is capped by the cargo lock
   and the bench lock, not by agent count. Measurement-heavy tasks (R6,
   R8, R9) queue behind the bench lock. Reading and injection work goes
   ahead meanwhile.
2. **Phase 1b:** R18's load-test window, holding the bench lock
   exclusively.
3. **Phase 2 (verification):** one verifier per finding; two for S0/S1.
   Assignment is cross-lens: a finding from R6 is attacked by an agent
   that ran R8 or R17, never by R6's own author.
4. **Phase 3 (synthesis, including X3):** a synthesis agent reads the
   surviving findings and produces the queued-work table: for each task
   in M32–M37 and M29/T29.6, which finding invalidates its premise,
   changes its order or adds a prerequisite. It rebases
   `proto/state-split` (d861f4f, a1ded79) onto `main` in scratch,
   lists the conflicts, and checks that its `#[bind]` source path keeps
   compare-before-write (R16) and the reload derive (R5). It checks
   whether T34.0's inventory covers R1's mark-path table, whether
   T32.0's fn-pointer reload survives R4's dylib findings, and whether
   M36's per-image panic hook survives R4's duplicated-statics finding.
   It also checks every kill rule and injection in those plans for
   whether it *can* fire (M29's plan once specified an injection it
   predicted would stay green).

## 6. What the final report must contain

`docs/results-deep-review.md`:

1. **A verdict per goal (1)–(4) and per platform (embedded, desktop,
   mobile):** met / partly met / not met, each backed by the finding IDs
   that support it, and not by prose.
2. **The surviving findings**, ranked by severity, in the §2 format,
   with the verifier verdict(s).
3. **A claims ledger:** every doc or comment claim tested, marked
   *held* / *false* / *overstated* / *not executable here*. The
   handoff's DECIDED markers and results-doc numbers that no longer hold
   get the finding ID that overturned them. Under the doc-currency rule,
   the follow-up commits must fix those docs.
4. **The vacuous-test table**, extended in AGENT.md's format (what / why
   it proved nothing / how it was caught), with the injection that
   exposed each entry.
5. **A gate reality table:** each gate, where it runs, whether it can
   skip, and whether it has now been shown to fail.
6. **Not executable here:** every question that could not run on this
   machine, and what it needs.
7. **The queued-work impact table** from Phase 3.
8. **A proposed follow-up milestone list (M39+)**, ordered by dependency.
   Each item names the finding IDs it closes, and the injection its gate
   will be verified with.
9. **The refuted appendix:** findings that did not survive, and why.

This plan's header and AGENT.md's queue entry move to DONE in the same
commit as the results doc. Findings that change public behaviour are
fixed in their own milestones, not in this one.

---

## Appendix A — the task records

### R1 — Boundaries completeness: every path that changes pixels without marking, and the cross-axis shadow oracle
*Priority high; cargo yes. Goals: (1) invalidation wins without stale frames once M34 makes Boundaries<256> the default; (3) the agent's view of a skipped subtree*

**Rationale.** Merges C1, X2, the palette-under-Boundaries half of DX-9, and the Glass press-tick question from X5. Four of M26 Phase A's five defects and T25.0 were invisible under Everything. The lens surveys found candidate unmarked paths: Host::tick discards App::tick's result, Host::compose sets only a widget-private dirty flag, and hovered/pressed are main-table indices, so a move between two rows of one Repeat compares equal. All of this has to be settled before M34 flips the default.

**Prior claims to test.** invalidate.rs: 'reporting a dirty subtree clean costs correctness' and that pointer/focus is marked by the host (T25.0). plan-boundaries-default.md T34.0: widget-drive/agent-drive/tasks-drive already run under both strategies and invalidation_oracle covers hover, press and enum. event.rs App::tick rustdoc says nothing about invalidating. retain.rs: backdrop readers depend on a wider region.

**Scope.**
- crates/lumen-static-shell/src/lib.rs: pump_with, tick, compose, paste, set_scale, set_palette, adopt, reload, mark_node, PointerFocus block
- crates/lumen-static-macros/src/lib.rs: the layout skip (~925-990: can_skip && is_clean && constraints==limits), enum hide ranges (~1495)
- crates/lumen-static/src/invalidate.rs (Boundaries<N>, overflow to mark_all, Changed::can_skip)
- crates/lumen-static/src/retain.rs (observe_dep, backdrop cap -> Damage::All)
- crates/lumen-static-winit/src/lib.rs frame loop (~420-468: host.tick result ignored), IME (~640)
- crates/lumen-static-widgets/src/input.rs compose, glass.rs tick/press, textarea.rs
- crates/lumen-static/src/gesture.rs, anim.rs, drag.rs, i18n.rs
- crates/lumen-static-shell/tests/invalidation_oracle.rs, damage_oracle.rs, glass_oracle.rs, banded.rs, enum_relayout.rs, palette_host.rs
- docs/plan-boundaries-default.md (T34.0 inventory, T34.1 shadow oracle)

**Questions.**
1. Table: for every public Host method and every call the winit frame loop makes, can it change a painted value or a layout input, and does it call mark/mark_all? Premise check first: confirm that winit lib.rs ignores host.tick's return and that Host::tick never marks.
2. Glass press deformation or any Animated<T> field: on frames after a press with no new input, is the render byte-identical under Boundaries<256>+Incremental and Everything+Rebuild? Print the differing node index and the pixel count.
3. IME Preedit("abc")/Commit/Cancel into a TextInput inside a clean subtree of a larger tree: under Boundaries, are the size and caret the same as under Everything?
4. Pointer moves from row 3 to row 4 of a Repeat, List and Chooser: what marks the change? For a row widget that does not handle Event::Hover, is the old highlight stale under Boundaries?
5. Does any existing correctness gate run a Boundaries frame that is not a mark_all frame? handled -> mark_all makes most click frames full; list which gate frames exercise a narrowed set (PointerFocus-only marks).
6. Axis-pair matrix across strategy x {overlay open, glass backdrop, repeater scrolled, banded threads, GPU regions, scale 1.0->1.5, set_palette dark->light, reload/adopt, agent getTree}: which pairs does any existing oracle drive? Drive every undriven pair once.
7. Does Boundaries<256> overflow to mark_all at N marks, and how many marks does a typical showcase hover+bind frame produce (print is_all rate)?

**Methods.**
- read: every mutation entry point checked against its mark calls, output as a table
- build (scratch worktree): a pairwise shadow-oracle test that runs one scripted event sequence through Host<_,Incremental,Boundaries<256>> and Host<_,Rebuild,Everything>, single and banded, and byte-compares each frame, printing the differing-pixel count beside a bound of 0. The script must include tick with dt>0, compose, a timer, an inbox apply, row hover, wheel, set_palette, set_scale and reload.
- every oracle step asserts that its action landed (handled==true, or the hit node index printed) before comparing frames
- fault injection (mandatory): (a) make Boundaries::mark a no-op; (b) make the PointerFocus block a no-op; (c) drop observe_dep for Glass; (d) skip memo invalidation on scale. Each must turn a named case red; record frames and pixels differing
- live drive: `just widget-drive` under the boundaries feature on a unique port, adding an IME preedit and an animated press

**Evidence required.** Each finding names the entry point (file:line), the scripted sequence, the differing pixel count or stale value printed beside the Everything value, and the injection that proves the new check can fail. A claim that a path 'does not mark' must include the frame where the stale output is visible, not only the missing call.

### R2 — The const node table and identity-by-hash: subtree_range soundness, fold collisions, and incomplete memo keys
*Priority high; cargo yes. Goals: (1) and (2): addressing, scoped cascade, marks and wid_index all rest on the table; retention and the text memo are only sound if equal keys mean equal inputs*

**Rationale.** Merges C2, C3 and the key-related half of C9. table_is_well_formed does not check next_sibling == k + subtree_size(k), which is the invariant subtree_range depends on. display::fold is an unfinalised polynomial hash mod 2^64, FoldedText does not fold in the length (so '\0x' collides with 'x'), and retain.rs's '10^-13 per frame' assumes a random function. MeasureKey has already missed two fields (size/weight before T26.3, scale before T26.17 #12).

**Prior claims to test.** macros header: 'the spliced table is well-formed' is asserted for every expansion. verification-notes Tier 0: structural facts 'discharged by const-eval'. retain.rs: collision probability '~10^-13 per frame'. text.rs: 'The display scale is in the key'; key 'Required, not defaulted'.

**Scope.**
- crates/lumen-static/src/widget.rs: splice_child, splice_seeds, table_is_well_formed
- crates/lumen-static/src/style.rs: subtree_range (~1144), index_of_wid, count_wid, cascade_subtree
- crates/lumen-static/src/desc.rs: NodeDesc, NO_PARENT
- crates/lumen-static-macros/src/lib.rs: SUB_TABLE emission, expand_enum STARTS, child_count `as u16`
- crates/lumen-static/src/hatch.rs (Either/Option tables), assemble.rs concat_table
- crates/lumen-static/src/display.rs: fold, Cmd::hash_into, hash_from
- crates/lumen-static/src/text.rs: MeasureKey (~110), Measured, FoldedText
- crates/lumen-static/src/retain.rs Recording/Incremental; crates/lumen-static-agent/src/journal.rs; spawn.rs, crates/lumen-static-widgets/src/choose.rs (other fold users)
- tests: table_invariants.rs, wid_index.rs, measure_key.rs, measure_scale.rs, text_memo.rs, damage_oracle.rs

**Questions.**
1. For every type deriving StaticTree in the workspace (grep derive(.*StaticTree), including tests, examples, fixtures and benches), does subtree_range(t,i) == i..i+size(i) at every index, with size computed independently from parent links, and is every node visited exactly once by the sibling chain?
2. Can a table be constructed (by hand, or by mutating one line of emitted code) that passes table_is_well_formed but gives a wrong subtree_range? If so, write the missing clause and show both directions: the corrupted table is rejected and every real table is still accepted.
3. Do enum STARTS and hide ranges agree with subtree_range for the first, middle and last variants, and for an empty variant? Is table_is_well_formed ever called on an Assembled (runtime) table?
4. FoldedText: does a Button, Badge or Chip showing "x" then "\0x" keep the first measured width? Then a same-length solver-derived fold collision that fits Cmd::Text limits: does Incremental report no damage while Rebuild repaints?
5. What changes a measured size but is not in MeasureKey: font family or stack, fallback face set, direction or locale, letter spacing, which TextMeasure implementation is in use, a font loaded after first layout, NaN limits (f32 PartialEq)? Demonstrate each with a memo hit that returns a stale box.
6. What does folding in the length plus a finaliser cost on `--bin bindings` and on the retention path, eight runs a side, against bindings-decision.md's 4.8 ns?
7. What do wid_index!, Host::invalidate and restyle_subtree do with an id that appears in two instances of a composite, or only in a Repeat template?

**Methods.**
- build: a test that walks every SUB_TABLE and checks the subtree_range and visited-once properties
- fault injection: in the macro, emit next_sibling+1 for a middle child, and separately swap two siblings' links; record whether the const assert, table_invariants.rs or any runtime gate goes red
- adversarial input: NUL-prefix pairs, plus a same-length collision derived by solving h*K+v mod 2^64 (not Thue-Morse, which needs length 2^11); the test must fail today, and a proposed fix must turn it green
- measurement: eight runs a side, the spread printed beside each delta

**Evidence required.** A counterexample table or string pair checked into scratch, the observed stale width or missed damage printed next to the correct value, and the injection record. A performance claim about a stronger hash needs eight runs a side with the spread.

### R3 — Input, focus and overlays: stale targets, row identity, keyboard reachability, and queue overflow
*Priority high; cargo yes. Goals: (1) input correctness and latency; (3) deterministic agent driving; the desktop usability claim*

**Rationale.** Merges C4, DX-2, P10 and the event-semantics part of AG9, plus the probe/paint agreement check from C6. InputState stores focus, capture, hovered and pressed as main-table u32 values. Handlers take their row positionally. Arrow keys move focus among siblings whenever a parent has two focusable children, which may steal Left/Right from a TextInput beside a Button. Tab walks only the main table.

**Prior claims to test.** pass.rs C5: hidden focus stops receiving keys. event.rs: `row` in EventCtx 'replaces move |s| s.remove(idx)'. T15.6: Shift+Tab, arrow groups and focus-visible are done, yet pass.rs:419 says Shift+Tab is not implemented. T27.10: Hover synthesis. M27: 94 actions driven, and it is unknown how many of them are keyboard actions.

**Scope.**
- crates/lumen-static/src/pass.rs: dispatch, dispatch_at, dispatch_overlaid, hit_test_overlaid, focusable, focusable_siblings, next/prev_focusable_in, overlay Tab path (~230), arrow groups (~419-470)
- crates/lumen-static/src/event.rs: InputState, InputQueue (capacity, overflow), EventCtx, NodeSel, Hover synthesis
- crates/lumen-static/src/overlay.rs, hit.rs, repeat.rs (hit, handler ~553-655, visible_range, MAX_ADDRESSABLE_EXTENT), hatch.rs List/Dyn hit
- crates/lumen-static-shell/src/lib.rs: paste (InputQueue::<64>::CAP), copy, compose
- crates/lumen-static-widgets/src: field.rs, input.rs, choose.rs, pick.rs, select.rs, data.rs, veil.rs
- crates/lumen-static-agent/src/lib.rs: input.click/fill/key/hover/scrollIntoView
- tests: events.rs, focus.rs, overlay.rs, hover.rs, select.rs, agent tests/input.rs

**Questions.**
1. Press row k of a Repeat, apply an inbox message that removes row k-1, then release. Does the handler see the item now at k, or refuse? Is 'press and release on the same logical item' specified anywhere?
2. Start a Slider drag (capture set), then switch the enclosing Either or enum so the slider is hidden, then move and release. Is the hidden node still addressed, and does capture clear?
3. In a row {TextInput, Button}: type 'abc', press Left twice. Assert caret==1 and focus unchanged. Repeat for SearchField and NumberInput.
4. Can every Select option, Chooser (PickList/Combobox/Menu) item, DatePicker cell, DataGrid row and Tree row be reached and activated by keyboard alone? List each that cannot. With a Modal open, does Tab stay inside it, and does Escape return focus to the trigger?
5. A radio group with more members than heapless_vec::Small holds: are members silently dropped from arrow navigation?
6. InputQueue: what is its capacity, is overflow counted anywhere, and is paste's hard-coded InputQueue::<64>::CAP correct for other queue sizes? Post 10 000 events between pumps.
7. Is hit-testing O(depth) or O(nodes)? Time a PointerMove at 10 000 rows (eight runs). Are PointerMoves coalesced before dispatch?
8. With count=50M and extent 20, scrolled to the end: does probe(x,y) on each painted row return the row whose text is painted there? Print the largest disagreement in px.
9. Does input.click (down+up, no prior move) produce the same widget-visible event sequence and final hover shade as a real xdotool click? Does input.fill send its backspaces into field A when a click on B fails to take focus?

**Methods.**
- read: every `[i]` or `[*f as usize]` indexing on InputState-derived values
- headless tests through Host/AgentHost::pump returning (frame, handled), asserting handled together with the exact node or row, under both strategies
- fault injection: delete the `group.len() > 1` guard; remove the C5 hidden-focus clear; remove capture release. Each new test must go red by name
- measurement: hit-test timing, eight runs a side, the quantity printed beside any bound
- live drive (display check first): showcase driven by keyboard only via the agent on a unique port; record which entries are reachable

**Evidence required.** The exact event script, the handled flag and node or row index observed versus expected, and an injection record for each new test. Keyboard-reachability findings list the widget and the key sequence that fails.

### R4 — Hot reload: can an ordinary windowed app reload at all, what does an edit cost, and is the dylib seam safe
*Priority high; cargo yes. Goals: (4) hot reload that cuts iteration time; memory safety of the only unsafe code*

**Rationale.** Merges HR1, HR2, HR3, HR7, C5 and V7, as the critic asked: one reviewer covering reachability and iteration time, then loader assertion validity, then FFI/ABI/duplicated statics and panics. The handoff marks Step 4 DONE, while plan-panic-recovery §1 says there is no windowed dylib reload. libloading appears only in fixtures/hot-host, the reload crate has 11 unsafe sites and no tests, and the T28.7 chain assertion calls hot_step, which the test itself wrote.

**Prior claims to test.** Handoff Step 4 'DONE 2026-08-30'; §7 DECIDED. step4: 'swap itself is milliseconds; ... ~90-250 ms, comfortably inside the 2 s criterion'. plan-tasks: T28.7 'demonstrated rather than argued'. check_no_unsafe.sh: 'cannot hide a real unsafe token outside a string literal'. verification-notes: miri unusable. M36 §1: any panic in v2 aborts.

**Scope.**
- crates/lumen-static-reload/src/lib.rs (export_reloadable_app!, 4 shims)
- fixtures/hot-host/src/main.rs, fixtures/hot-app/src/lib.rs, scripts/hot_reload_demo.sh, justfile hot-demo
- crates/lumen-static/src/reload.rs (LOG thread_local, take_log, symbols)
- crates/lumen-static/src/spawn.rs (JobFn, Spawner), inbox.rs (Sender, Waker, epoch)
- crates/lumen-static-agent server thread / port binding
- scripts/check_no_unsafe.sh, fixtures/miri-smoke
- docs/00-handoff-static-tree.md §7 and Step 4, docs/step4-hot-reload.md, docs/results-step1-firewall.md, docs/plan-tasks.md T28.7 and §5, docs/plan-panic-recovery.md, book/src/hot-reload.md

**Questions.**
1. Is there any path from `cargo run -p <example>` or a just recipe to a running window that picks up a source edit without restarting? List every missing piece. Quote each sentence in the book or handoff that implies one exists.
2. Measured, five runs each with the spread: time from a one-line edit of FOOTER to a new .so for fixtures/hot-app, debug and release, split into frontend, codegen and link; the same for examples/showcase with only its crate-type changed to cdylib; dlopen + new + restore + first frame. Print each against the 2 s criterion and the firewall doc's 165 ms tail.
3. For each expect/assert in hot-host: which single-line fault in framework code (not fixture code) turns it red? Does T28.7 touch lumen_static::spawn, the inbox or Host::pump at all?
4. Injections: the derive's restore skips Nested fields; Repeat::restore ignores scroll; hot_restore returns 0 without restoring. Which assertions fire? Could any expectation be met by fresh defaults?
5. Add a v3 that changes the Ui shape (a new widget field, reordered fields), and swap v1->v2->v3->v1 100 times using distinct paths. Report RSS growth per swap, the /proc/self/maps image count, and whether the fallback log is complete.
6. hot_restore with (null,0): slice::from_raw_parts(null,0) is UB; confirm the contract. A panic in restore, build or hot_frame: record the exit status and signal, and whether the saved bytes survived. A v2 missing hot_step: does the loader fail before or after dropping v1?
7. List every static, thread_local!, OnceLock, LazyLock, Once and set_hook in crates/*/src, and classify what duplication per image does to each. Demonstrate the host-side take_log returning empty. With the agent feature in the dylib, does v2 fail to bind v1's port?
8. Does check_no_unsafe.sh miss `let s="a//b"; unsafe {..}` on one line, a #[path] module or build.rs output? Does `cargo +nightly miri` run anything today, and can the shims be driven in-process under miri?
9. nm -D --defined-only on both .so files: are any non-hot_* symbols exported that could interpose? Is there any ABI fingerprint check? Inject a changed symbols::SAVE signature.

**Methods.**
- read: every unsafe block, with provenance, lifetime and allocator noted for each
- measure: cargo build --timings / -Z time-passes; five runs a side with the spread; Instant instrumentation in a scratch copy of hot-host
- fault injection in a scratch worktree: each derive and fixture injection above, run through hot_reload_demo.sh, recording the named assertion that fires and its printed value
- build: a looped hot-host variant, and a v3 hot-app feature
- miri: retry on the installed nightlies and record exactly how it breaks; if it works, add a shim harness
- gate test: plant the one-line bypass in a scratch copy, show the gate misses it, fix it, and show the fix still accepts the current tree

**Evidence required.** Measured times with the spread and the configuration; for each assertion, the framework-side injection that fires it, or a statement that none exists; exit codes and signals for panic cases; nm/readelf output for symbol claims; a reproducer for any UB claim.

### R5 — Reload state semantics, Host::adopt, and the one-construction/one-stylesheet claims
*Priority high; cargo yes. Goals: (4) a reloaded app equals a restarted app and keeps the state being iterated on; (2) the reload surface compiles out*

**Rationale.** Merges HR4, HR5, HR6, HR8, DX-4 and HR9. Only Label, Button, Toggle, Fixed, Spacer, Repeat, Vec and Option implement Reloadable. Per-field fallback applies only to top-level fields. adopt resets marks but keeps states, input, overlays, retention and memo. Program::style is optional, so Config::stylesheet and Application::style can still disagree. This task also covers the M32 prototype review.

**Prior claims to test.** step4: 'a field added in the new build is simply missing from old bytes -> fresh default + log, never a hard error'. M31: 'Application::build is the one path now ... Both directions checked'. adopt rustdoc: 'A new application is a new tree'. Shell Cargo.toml: an app that never swaps should not link serde_json.

**Scope.**
- crates/lumen-static-widgets/src/reload.rs; crates/lumen-static/src/reload.rs (Vec/Option/Repeat impls, log_fallback)
- crates/lumen-static-macros/src/lib.rs: expand_reloadable (save/restore stmts), is_application, the no_default blanket impl, reload_mode
- crates/lumen-static-shell/src/lib.rs: Host fields, launch, adopt (~214), reload, new
- crates/lumen-static-winit/src/lib.rs: Config, run, application, Program (~700-760)
- crates/lumen-static/src/event.rs App/Application
- crates/lumen-static-macros/tests/ui/*reloadable*; crates/lumen-static/tests/reload.rs, reload_hostile.rs; crates/lumen-static-shell/tests/reload_sheet.rs
- examples/*/src/main.rs; docs/plan-application.md, plan-program.md (prototype 29c1238), step4-hot-reload.md honesty list

**Questions.**
1. Table: every widget with user-mutable state (value, text, cursor, scroll, selection, open, active variant). Is it Reloadable? If not, does the app silently lose that state under #[reload(fresh)], or fail to compile? Can a StaticTree enum be a reload field at all?
2. Add #[derive(Reloadable)] to showcase's app and Ui in scratch. Record the compile errors and judge whether they are actionable.
3. Schema-change table: a kept Settings{a,b} becomes {a,b,c}; a rename count->clicks; u32->u64, u32->i8 with value 300, f32 NaN (which serializes as null), and enum variant shape changes. Which changes silently lose data or silently truncate? Does a failed save that wrote Null make an Option restore produce None with no second log line?
4. Where does a fallback log line go in a windowed session? Can the agent see it?
5. Host field table: does adopt reset each field, and is keeping it correct when the new tree differs (a) in values only and (b) in structure? With hover, press and an open popup under Boundaries<256>, does reload() then render equal a fresh launch plus restore, pixel for pixel and in damage?
6. After reload, does pointer capture route a PointerUp to an old node index? Are inbox results queued before the reload applied to the new app, and is there a Host-level test for that?
7. Enumerate every construction and styling site per example. Start an app via Program without .style() whose Application::style sets a colour: which sheet is on screen, and which one would reload install?
8. Does the E0119 trick also refuse a hand-written impl Default? Does the is_application heuristic misfire for a non-app struct with a #[tree] field? Can T32.0's fn-pointer reload be sound across a dylib swap?
9. With reload off, can an app that uses #[derive(Reloadable)] still build? What is the minimum cfg_attr boilerplate? Is serde_json absent from a reload-off example binary (nm count, with the reload-on count printed as the positive control)?

**Methods.**
- read: Host fields against adopt; construction sites per example
- headless tests (scratch worktree): round-trip per stateful widget; the schema-change table; the adopt oracle under both strategies
- fault injection: remove mark_all from adopt and watch the oracle fire; drop one field from a widget's restore and watch the round-trip fire by name; remove the no_default blanket and run the trybuild tests in both directions
- build: reload on and off for examples/minimal with nm, printing both counts

**Evidence required.** For state-loss findings, the before and after values printed. For adopt findings, the pixel diff count or the misrouted node index. For the stylesheet divergence, the pixel colour from each path. Every new test carries its injection record.

### R6 — Frame-path performance truth: the turnkey configuration versus the benchmarked one, bench validity, and clean-frame cost
*Priority high; cargo yes. Goals: (1) peak performance, and the credibility of every published figure; (2) whether the optimisations reach users*

**Rationale.** Merges P1, P3, P4, P8 and the competitor-configuration part of X10. The winit crate ships Inval=Everything, with shell simd and threads off. The cross-arm frame bench calls pass::layout_with/paint directly and skips place, the cascade, retention hashing and Incremental::finish. The '0.01 µs still frame' times layout only, yet §5b presents it as the idle frame. Every clean frame still runs an O(nodes) place, a full repaint and a full hash.

**Prior claims to test.** results-competitive: 'No number here comes from a single run', 'summed stages and an independently timed whole frame agree to under 1%', '9x iced', 'Under Boundaries that frame is the 0.01 µs row'. plan-damage-precision §2.5 concedes click frames save zero. workers_for rustdoc: thresholds knowingly off by 1.8-2.4x.

**Scope.**
- crates/lumen-static-winit/Cargo.toml, src/lib.rs (Inval/Ret aliases, run, application)
- crates/lumen-static-shell/Cargo.toml features, src/lib.rs pump_with (726-960), timings3
- crates/lumen-static/src/pass.rs (paint_overlaid, place), retain.rs (Recording, Incremental::finish/align, Regions::push), display.rs hash_from, style.rs cascade_arenas_deep
- benches-competitive/arms/{frame,stages,raster,lumen-static-app,lumen-static-lean}, competitors.sh, run.sh, bench5.awk
- benches/src/bin/{invalidation,grid,cascade,repeater,paint_sink}.rs
- docs/results-competitive.md §3 §5b §7, results-gpu.md §1, results-simd.md, results-threading.md, competitive-feature-gaps.md

**Questions.**
1. Table from cargo metadata (no build): for every example and for lumen-static-app, which of boundaries, simd, threads, gpu and observe are enabled? Which published figure describes a configuration that no example ships?
2. For each figure in results-competitive §5b/§7 and results-gpu §1: what does the timed closure call, and which pump_with steps (dispatch, subscriptions, outbox, bindings update, cascade, place, Scaled, Recording/finish) does it omit? Does the stages arm still mirror pump_with after M28, and is its sum-versus-pump gap printed beside its bound?
3. Full pump_with cost under Boundaries<256> at 1k, 3k and 10k nodes for (a) nothing changed, (b) the pointer moved with no hover change, (c) a hover change, (d) one bound label changed, split into dispatch, layout, place, paint, hash and finish, eight runs a side.
4. showcase: what do layout, place and cascade cost for a Toggle click and for a Select open, with and without observe, under both strategies? Does every click re-run cascade_arenas_deep even when no arena was touched (count the calls)?
5. Does the raster arm call gpu.wait() inside the timed closure? Rerun with a unique string per card to separate the effect of the glyph atlas.
6. Is threads safe as the desktop default? What is the per-frame thread::scope spawn+join cost at 1080p? Is results-competitive's competitor set built at each competitor's best configuration (release, LTO, their GPU path)?
7. Construct a 10-deep nested Shrink column: does layout grow superlinearly?

**Methods.**
- read: cargo metadata feature graph; trace each timed closure
- fault injection: spin Host::place or Incremental::finish 10x slower, and confirm which arms move (the frame arm should not, the stages pump row should); force arenas_dirty every frame and confirm the bench sees it
- measurement under the bench lock, eight runs a side, the spread printed, machine load recorded
- build: turnkey lumen-static-app with and without simd,threads at 1080p full redraw

**Evidence required.** A step-diff table per figure. Every measurement carries eight runs, the spread, the configuration, the load at run time, and the quantity printed beside any claim it is compared to. An injection record shows the arm can see the step it claims to cover.

### R7 — Idle, wake and power; tasks, timers and cross-thread handoffs; window lifecycle
*Priority high; cargo yes. Goals: (1) resource efficiency (battery), and no stall, spin or lost work; desktop/mobile robustness*

**Rationale.** Merges P2, C8, X11 and X7. Every CursorMoved requests a redraw that does a full pump and a full-surface present. WaitUntil built from a deadline <= 0 may spin. NaN dt leaves animations unsettled forever. Host::tick ignores Outbox::push refusals. The agent thread blocks on rx.recv with no timeout. Nobody has checked 0x0 resize, surface loss or close with tasks in flight.

**Prior claims to test.** winit lib.rs: 'A still UI idles'; 'keeps the idle frame numbers in docs/results-frame-cost.md true'. M28/T28.11-12: WaitUntil fixes timers on idle windows. anim.rs: huge dt 'Guarded rather than trusted'. inbox.rs: a sender 'cannot send and forget to ask for a frame'.

**Scope.**
- crates/lumen-static-winit/src/lib.rs: redraw, about_to_wait, user_event, window_event_impl (Resized, CloseRequested, CursorMoved), answer_agent
- crates/lumen-static-winit/src/surface.rs present/sync_size; gpu.rs; backend.rs; dialog.rs
- crates/lumen-static-wgpu/src/lib.rs present, surface error handling, PresentMode
- crates/lumen-static-shell/src/lib.rs: tick, drain_outbox, diff_subscriptions, next_deadline_ms, more_deferred, spawner drop
- crates/lumen-static/src/spawn.rs (Outbox::push, Timers::advance, Threads), inbox.rs, anim.rs, gesture.rs, num.rs clamp
- crates/lumen-static-shell/src/software.rs Canvas::resize, workers_for at 0 px
- examples/wake-gate, scripts/wake_gate.sh, scripts/tasks_drive.sh, tests/inbox_wake.rs, docs/results-tasks.md, results-frame-cost.md

**Questions.**
1. Showcase with the window idle and the pointer still, sampled over 60 s via /proc/<pid>/stat and Host frame_number: how many redraws? Repeat with a focused TextInput (caret blink period?) and with a periodic subscription. Target 0 for the first case.
2. Pointer moving continuously over non-interactive content: frames/s, CPU%, and pixels presented per frame on software and GPU. Does a hover that changes nothing still cost a full pump and a full present?
3. A timer whose earliest() is always Some(0.0), and next_deadline_ms returning NaN: does the loop busy-spin? Print CPU% beside a bound.
4. tick(1e9) with a 16 ms timer: how many firings and how much work? tick(NaN) and tick(-inf): does any Animated or gesture state stay unsettled for good?
5. Fire more timers than Outbox::CAPACITY in one tick: are the extra ones counted, reported to the agent, or silently lost? A worker panics holding a Sender: does the chain resolve or hang?
6. Can the agent thread hang forever (loop exited, or blocked in an rfd dialog)? Is any Mutex held across a send, recv or present? Does wake-gate go red when the waker install is removed?
7. 0x0 minimise and 1x1 resize on software and GPU; SurfaceError::Lost/Outdated; close with a running thread task: panic, spin, leak, or prompt exit? Does a resize storm's final frame match the final size?
8. Does results-frame-cost.md contain any idle figure, as the winit comments claim?

**Methods.**
- live drive (check the display first; otherwise mark the questions not executable): /proc sampling (perf_event_paranoid=4), xdotool mousemove and resize loops, agent frame counters on a unique port
- headless tests: adversarial dt values, timer floods, a worker panic, each asserting a counter or settled() state
- fault injection: make Outbox::push always refuse; remove the proxy wake in Sender; each must turn a named gate red (wake_gate, the new tests)
- read: every channel, lock and blocking call on the event-loop thread

**Evidence required.** Frame counts and CPU samples with the window duration and configuration. Exit status and stderr for lifecycle cases. For a lost-work claim, the counter value against the number of submitted items. Injection records for new tests.

### R8 — Software raster, the text raster path and memory footprint
*Priority high; cargo yes. Goals: (1) raster efficiency and allocation discipline on desktop and mobile; per-node and per-window memory*

**Rationale.** Merges P5, P7, P9 and the latency part of P10. The alloc gate's raster test uses Canvas::new with BlockMetrics, so the truetype path (no glyph cache, a Vec<Glyph> with a Vec<u8> per glyph every frame) is outside the 'raster allocates nothing' claim. Each damage region re-walks the whole list. Banding re-shapes runs that straddle strips. No size_of assert pins NodeState, Style or Cmd.

**Prior claims to test.** alloc_gate.rs 'rasterising a frame allocates nothing' (M26/T26.13, 'to zero'). surface.rs: render p95 0.089 ms vs present p95 0.044 ms. results-competitive §3: 24.7 MB idle RSS. results-appearance: no per-node desktop budget.

**Scope.**
- crates/lumen-static-shell/src/text.rs (FontStack::prepare, place_run, glyphs, rasterize, measure_run)
- crates/lumen-static-shell/src/software.rs (Surface::render text arm, clipped, blur_plan, render_banded (~1561), workers_for, strip_rows_for, blit_0rgb_within, Canvas::resize)
- crates/lumen-static-winit/src/surface.rs present, sync_size
- crates/lumen-static/src/retain.rs Regions/MAX_REGIONS, Incremental slots/prev; desc.rs NodeState; style.rs Style; display.rs Cmd; DisplayList.text capacity
- crates/lumen-static-shell/tests/alloc_gate.rs (~343), banded.rs, simd_shadow.rs, damage_oracle.rs
- benches-competitive/resources.sh; docs/results-raster-scaling.md, results-simd.md, results-threading.md, results-allocation.md

**Questions.**
1. Allocations and frees per rasterised frame with Canvas::with_font(FontStack::system()) + FontMetrics, for PlainApp and for a 3 000-row list with one changed row. Print the count beside the claimed zero.
2. With threads on, how many times is each text run shaped per frame at 1080p full redraw (glyphs() calls against the number of text commands)? What fraction of damaged-frame raster time is re-rasterising glyphs identical to last frame's?
3. MAX_REGIONS small regions over a 3 000-command list: what does the command-rejection walk cost against the pixel work? At a 24x24 caret, is buf.present() now the dominant cost?
4. Which Cmd kinds have a SIMD path, and what share of a showcase frame falls into scalar-only kinds? Is shadow/blur work bounded by the damage rect or by the shadow's full extent?
5. size_of NodeState, Style, Cmd and NodeDesc today: is any of them asserted? RSS/VmHWM and heap peak (counting allocator with a peak field) at 1k, 3k and 10k rows, and at a 4K full redraw on software and GPU.
6. Does a live resize drag allocate per frame (Canvas::resize, softbuffer, retain reset)? Does shrinking free memory? Does DisplayList.text capacity stay pinned after a one-off 1 MB paste?
7. Allocations per event: a PointerMove, a key repeat in TextInput, a paste.

**Methods.**
- build: a copy of alloc_gate's raster test using the truetype path (expected red today; that failure is the finding)
- measurement under the bench lock, eight runs a side: the raster arm with a many-small-regions scene and a caret scene; present_ns from surface::Frame
- fault injection: make clipped() return the unclipped rect for Shadow, and check damage_oracle/banded go red; make Canvas::resize always reallocate, and check whether any gate notices; add a field to NodeState and confirm a new size_of gate fires
- read: enumerate cfg(feature="simd") sites against Cmd variants

**Evidence required.** Allocation counts and memory figures printed beside their claimed bounds, with configuration and the tool used (a counting allocator with peak, /proc VmHWM). Timings: eight runs with the spread. Injection records for any proposed gate.

### R9 — GPU backend efficiency, cross-renderer fidelity across the whole Cmd vocabulary, and the Glass material
*Priority medium; cargo yes. Goals: (1) desktop/mobile GPU performance and power; output quality, not only speed; the flagship visual feature's cost claims*

**Rationale.** Merges P6, X4 and X5. draw_regions allocates four Vecs per frame, and nothing gates it because `just test` excludes wgpu. present() clears and blits the full target every frame. Parity has been vacuous before (T15.12's 6% bound). No matrix records which Cmd variant appears in which parity corpus. The Glass figures (+18%, 1.44 ms) need re-deriving.

**Prior claims to test.** results-gpu: 17.6x at 4K, crossover at 3-5% damage; M26 T26.12: GPU handed 5,000 px2 instead of 452,400. svg.rs: 'exercises the whole command vocabulary'. results-glass: +18%, 148 ms -> 1.44 ms.

**Scope.**
- crates/lumen-static-wgpu/src/lib.rs (for_window, draw, draw_regions ~643-662, walk, draw_run, present, wait, read_rgba), atlas.rs, shader.wgsl, blit.wgsl
- crates/lumen-static-winit/src/gpu.rs, backend.rs
- crates/lumen-static/src/display.rs Cmd variants
- crates/lumen-static-shell/src/software.rs, svg.rs, golden.rs; tests: border_pixels.rs, clip_depth.rs, simd_shadow.rs, glass_*.rs
- crates/lumen-static-wgpu/tests/parity.rs, glass_parity.rs, gpu_regions.rs
- crates/lumen-static-widgets/src/glass.rs; benches/src/bin/glass_cost.rs
- docs/results-gpu.md, results-glass.md §0b-§0h, plan-glass.md, plan-liquid.md §12

**Questions.**
1. First, does this machine have a GPU adapter? If not, mark the GPU questions not executable (do not count a skip as a pass).
2. Steady-state allocations per GPU frame (a counting allocator in a headless-adapter test). Instance bytes uploaded per frame for a caret-sized damage on a 3 000-row list versus a full frame. Does the per-region clip cull on the CPU or only in the shader?
3. How long does present() block under AutoVsync inside redraw()? Event-to-present latency p50/p99 on GPU versus software.
4. Do glyph atlas resets happen in showcase or at 4K with many sizes? Shrink GLYPH_ATLAS to force resets: does parity.rs still pass, and does anything report the reset?
5. Matrix: for each Cmd variant, which parity tests (scalar vs simd, single vs banded, software vs GPU, software vs golden, svg) contain it? Every empty cell is untested. What tolerance does golden.rs use, and what is the measured max diff (printed together)?
6. Is blending done in sRGB or linear space, and is it the same on software and GPU (a 50% white-on-black edge)? At scale 1.25 and 1.5, do 1-px borders and StrokedRect snap the same way? Do nested rounded clips agree?
7. Does glass_cost reproduce +18% and 1.44 ms within their spreads? How many panels trigger the Damage::All fallback, and what does one frame past the limit cost at 1080p? What does Glass do in a none-tier build?
8. What part of the 197.8 MB GPU RSS is framework-owned (atlases, intermediate target)?

**Methods.**
- capability check first; record 'not executable' explicitly
- fault injection: flip one variant's AA or gamma path in software.rs and see which parity or golden test goes red; remove the backdrop-cap fallback and check glass_oracle
- measurement under the bench lock, eight runs a side: glass_cost, bytes passed to queue.write_buffer, present blocking time
- read: list the variants and grep each corpus

**Evidence required.** The coverage matrix, max diff per variant per renderer pair printed beside each bound, allocation counts, timings with spread. Every GPU finding states the adapter and backend it ran on.

### R10 — The text pipeline end to end: measurer/painter agreement, shaping, bidi and i18n, and text entry
*Priority high; cargo yes. Goals: (1) best-in-class desktop and mobile text; (2) lumen-static-text must work when an application names it*

**Rationale.** Merges X1, DX-7 and the IME/clipboard half of DX-3. ComplexText shapes for measurement, but the canvas repaints Cmd::Text through fontdue with no shaping and no bidi. Direction and the mirror_* helpers have no caller outside tests. Nothing calls set_ime_cursor_area. Ctrl+C is consumed even with nothing selected. Multi-codepoint key text is dropped.

**Prior claims to test.** plan-capabilities T15.8/T15.9 shipped. i18n.rs: 'an application can have a mirrored Arabic UI before it has an Arabic shaper'. lumen-static-text: 'changes where the glyphs go, not how they are drawn'. T14.8: CJK entry works.

**Scope.**
- crates/lumen-static-text/src/lib.rs, tests/shaping.rs
- crates/lumen-static/src/i18n.rs, cx.rs (TextMeasure, GlyphSink, Cluster), font.rs (BakedFace), text.rs (TextBuffer, ClusterBuf)
- crates/lumen-static-shell/src/text.rs, software.rs text arm; crates/lumen-static-wgpu draw_run
- crates/lumen-static-widgets/src/input.rs, textarea.rs, field.rs, rich.rs (RichTextEditor)
- crates/lumen-static-shell/src/lib.rs compose, copy, paste/type_text
- crates/lumen-static-winit/src/lib.rs clipboard_shortcut (~378), IME handling; translate.rs keyboard() (~67 multi-char drop), ClickCounter
- docs/text-render-adoption-decision.md, plan-capabilities.md T15.8/T15.9, T14.8, text-and-scrolling-decisions.md

**Questions.**
1. With ComplexText as the measurer and Canvas::with_font as the painter, render 'مرحبا' and 'abc אבג def'. Are the glyphs joined and bidi-ordered, and does the painted ink width equal the measured width (print both)?
2. Can a turnkey window (run/Program) use ComplexText at all? Is Direction consulted in layout, hit-testing, caret Left/Right or Tab order anywhere?
3. For each shipped measurer/painter pair (BlockMetrics+Canvas::new, FontMetrics+with_font, BakedFace+embedded painter, FontMetrics+wgpu), does the measured box contain the ink over a corpus of labels, sizes and weights? When a face lacks a glyph (emoji, CJK), what do measurement and painting each do?
4. Which text-taking widgets implement compose(Preedit/Commit/Cancel)? Is preedit drawn and kept out of the bound value until commit? Does Backspace after 'e\u{301}' or a ZWJ family emoji remove exactly one visible character in each widget?
5. Are Home/End, Ctrl+Left/Right, Shift-selection, and double- and triple-click selection implemented? Is ClickCounter's count consumed anywhere?
6. Does Ctrl+C with no selection reach App::update? Is Ctrl treated the same as Meta on every OS? How long does a 100 kB paste into TextArea take, and how many allocations does it make? Can text in a Repeat row or popup be copied?
7. Where does the IME candidate window appear for a TextInput away from the origin (code first; then live with ibus/fcitx, if installed)?
8. BakedFace at a non-baked size: how far is the scaled advance from fontdue's?

**Methods.**
- build: a headless test pairing ComplexText with the software painter, comparing measured size with the ink bbox (predicted red)
- read: every TextMeasure implementor and every place a measurer is constructed
- fault injection: delete the bidi reorder in lumen-static-text and see whether any test that goes through a painter turns red; make compose a no-op in field.rs
- headless Host tests per widget for grapheme deletion and compose sequences; a winit translate test for Ctrl+C
- live drive only if an IME is installed; otherwise record it as not executable

**Evidence required.** Measured versus inked widths printed together; per-widget pass/fail tables for compose and grapheme deletion; screenshots or pixel hashes for shaping claims; injection records.

### R11 — Embedded budgets: gates that price a frame no device runs, the stack model, panic cost, const-eval, and the missing end-to-end loop
*Priority high; cargo yes. Goals: (1) best in class on embedded (flash, RAM, stack); (2) pay only for what is used*

**Rationale.** Merges EMB-1, EMB-2, X8, CT-1 and the stack/Dyn half of C6. The firmware runs one layout+paint and never dispatches, invalidates, cascades, binds or rasterises. stack_bytes is depth*272, a constant from T13.0. The flt2dec regression would pass under the 64 KB ceiling. Dyn can get past the compile-time depth bound. const-budget times a whole debug build against 5 s when the const-eval itself takes about 0.2 s. Host, retention, damage and the rasteriser are std-only.

**Prior claims to test.** results-embedded: every threshold met; stack 'asserted at compile time since 2026-09-07' (816 B); 'f64 in the frame path: zero'; .text 27,024 B. desc.rs: 'a tree that could overflow the stack does not compile'. results-const-eval: 3,711 nodes in 0.13-0.21 s. mobile-decision: 'The core is genuinely portable'.

**Scope.**
- fixtures/embedded/src/bin/firmware.rs, src/lib.rs (frame, type_char, Checksum)
- crates/lumen-static/src/desc.rs (MAX_DEPTH, STACK_BYTES_PER_LEVEL, stack_bytes), bind.rs (SCRATCH_CAP, format_into), num.rs, hatch.rs Dyn (~776 runtime assert)
- scripts/check_text.sh, check_bss.sh, bss.allow; justfile thumb, firmware-size, const-budget; root Cargo.toml profiles
- fixtures/const-budget; crates/lumen-static/src/widget.rs (splice_seeds), style.rs (wid_index!, count_wid)
- crates/lumen-static/src/retain.rs, invalidate.rs; crates/lumen-static-shell/src/lib.rs, software.rs (tier classification)
- docs/results-embedded.md, results-const-eval.md, plan-embedded.md, results-monomorphization.md

**Questions.**
1. Reintroduce f32::clamp at Constraints::clamp: does `just thumb` / check_text.sh go red? Does a symbol gate on flt2dec|__aeabi_d|__adddf3|fmt::float fire? Print .text beside 65,536 before and after.
2. Row by row through the results-embedded threshold table: is each threshold script-gated, compile-time-gated or prose-only?
3. Real worst-case stack of the firmware frame on thumbv7em, debug and release, measured by qemu stack painting (the fallback if -Zemit-stack-sizes/cargo-call-stack fails on &dyn Node), against stack_bytes(Ui::DEPTH)=816 B. Do a #[bind] (256 B scratch), a TextInput layout, or Dyn<Dyn<..>> / a Dyn at depth 60 exceed the model?
4. A firmware-interactive variant that adds dispatch+hit, a Boundaries<16> mark/clear cycle, a cascade and one #[bind]: what are .text/.rodata and the headroom used?
5. How many .text/.rodata bytes are panic machinery and location strings? What do -Zlocation-detail=none, panic=abort, opt-level=z, lto=fat and codegen-units=1 change? State which profile the docs' numbers apply to.
6. Classify each Host/retain/software item by the tier it needs. Write the smallest no_std loop (queue -> dispatch -> Boundaries -> layout -> paint of damaged rows into a band Painter) for thumb, and record what fails to compile.
7. const-budget: what share of the 5 s is const-eval (-Z self-profile)? Inject an O(n^2) into splice_seeds or count_wid at 3 711 nodes: does the gate fire?

**Methods.**
- fault injection: f32::clamp; a quadratic const loop; lower the Dyn depth check or remove it (each records whether its gate fires)
- build: firmware variants in a separate target dir (target/review-R11); size -A and nm --size-sort diffs
- measurement: qemu stack painting; -Z self-profile; each quantity printed beside its bound
- read: the threshold table and the tier of every std/alloc cfg

**Evidence required.** Byte counts and stack figures beside the budgets and the model, with the profile stated. Injection records showing each gate firing or not. For the no_std loop, the compile errors verbatim.

### R12 — Configurability: the feature matrix, release-shape leakage, no_std tier behaviour, per-method vtable cost, monomorphization and mobile builds
*Priority high; cargo yes. Goals: (2) flags and generics compile in only what is used, and every advertised configuration builds and behaves; (1) flash/binary size; mobile build readiness*

**Rationale.** Merges CFG-1, CFG-2, V10, TIER-1, PAY-1, MONO-1 and the build half of MOB-1. Core's dev-dependency on widgets with observe probably unifies observe into `just release-shape`'s test build. The widgets alloc tier is never built for thumb. The winit/agent -> widgets/observe seam from M30 is still manual. The boundaries feature swaps a public type alias. Every widget reaches node(i) as &dyn Node, so its vtable links on_event, hit and compose into a paint-only firmware.

**Prior claims to test.** justfile: release-shape is 'the ONLY build that compiles what verification-notes scopes its argument to'. results-widget-crate: the gate is '#[cfg(feature)] and the linker, and both drop what a tree does not name'. step5: observe has zero release cost. plan-embedded T13.4: strict-tier cargo check is 'the strong proof'. results-monomorphization: 'No gate runs this'.

**Scope.**
- crates/*/Cargo.toml [features] and [dev-dependencies]; justfile release-shape, dev-shape, thumb, check-extra
- crates/lumen-static-winit/src/lib.rs Inval alias (~254-257); crates/lumen-static-shell/src/lib.rs cfg(agent) sites; crates/lumen-static-agent/Cargo.toml, audit.rs
- crates/lumen-static/src/widget.rs (trait Node), bind.rs, text.rs (~274), style.rs (cfg all(debug_assertions, alloc)), invalidate.rs
- crates/lumen-static-macros/src/lib.rs node/node_mut expansion, fuse, rdyn_opaque
- fixtures/embedded, fixtures/binsize; docs/results-widget-crate.md, results-monomorphization.md, plan-observe-cost.md, mobile-decision.md, step5-observability.md

**Questions.**
1. Does rustc for the lumen-static lib get --cfg feature="observe" in `cargo test -p lumen-static -v --no-run`? Does the hatches test (required-features observe) run under release-shape? Which checks build core with default features and no dev edges?
2. cargo hack check --feature-powerset --depth 2 --no-dev-deps, one crate per invocation, over std, alloc, observe, reload, truetype, simd, threads, agent, boundaries, gpu, dnd, dialogs, clipboard, with a time budget per crate: which combinations fail, and which build but lose behaviour?
3. Does `cargo check -p lumen-static-widgets --no-default-features --features alloc --target thumbv7em-none-eabihf` pass, and does clippy -D warnings pass on it?
4. An app with lumen-static-winit/agent but without widgets/observe: what does ui.getTree report for a Label, and does anything warn? If shell/reload is on and widgets/reload off (or the reverse), is that a loud error or silent state loss? If crate A names Host<_,_,Everything> and B enables winit/boundaries, what happens to A?
5. Run core and widgets unit tests with --no-default-features --features alloc (and with none) on the host. Which tests cannot compile? Is format_into's 256-byte truncation char-boundary safe (255 ASCII + a 4-byte char), and is it documented at #[bind]?
6. In the linked thumb firmware, which on_event/hit/compose/selection/dismiss symbols exist, and how many bytes do they total? Does #[widget(fuse)] remove them? What is the marginal .text of naming each capability (Repeat, Popup, Either, gesture, anim, cascade, Boundaries, bind)?
7. In linked x86 release+LTO and thumb builds: the per-type and per-field cost; Button<AppA>+Button<AppB>; Host<Everything>+Host<Boundaries<256>>; Boundaries<16> versus <256>; and Boundaries<256>::mark at full occupancy.
8. cargo check --target aarch64-linux-android and aarch64-apple-ios for core, widgets, shell and winit: what breaks?

**Methods.**
- build matrix in a separate target dir (target/review-R12), cargo hack with --no-dev-deps, one crate per call
- fault injection: add `#[cfg(feature="observe")] compile_error!` to a core test and run release-shape (it must go red if observe leaks, and go green once fixed); remove lumen-static/observe from widgets' observe list and see which gate notices; give one widget's on_event a large panic-with-format body and check whether firmware .text grows
- measurement: symbol attribution with nm -C --size-sort -S; binsize/firmware variants; the mark micro-bench, eight runs

**Evidence required.** The rustc -v flag line for the leakage claim, the matrix of failing combinations with their errors, symbol byte totals, and a marginal-cost table. Each claim gives the exact command and target.

### R13 — Agent truthfulness: frame-number domains, waitSettled and perf, tree fidelity, protocol drift and session replay
*Priority high; cargo yes. Goals: (3) an agent that tells the LLM the truth about the program*

**Rationale.** Merges AG1, AG2, AG5, AG7, AG8 and the fill/key semantics of AG9. Input replies carry host.frame_number(), while lastChange, lastDamage and waitSettled use Agent::frame, which only advances in redraw. waitSettled hard-codes settled:true. winit passes FrameTiming::default(), so app.perf may report zeros that pass every budget. The skill lists app.quit, which the turnkey endpoint lacks, and the env var names differ between the launchers and the clients.

**Prior claims to test.** SKILL.md: 'a layout right after a click already sees the result'; the app.perf budget table; app.quit listed. lib.rs: 'No animation scheduler exists'. T27.10: (frame, handled) is 'the only direct answer'. session.rs: mismatch is a hard error. Crate doc: 'in-process dispatch only'.

**Scope.**
- crates/lumen-static-agent/src/lib.rs (handle, handle_input, frame_done, last_change, last_damage, since_param, perf, live_nodes, Index::build, resolve_one, get_tree, input.fill, module doc)
- crates/lumen-static-agent/src/journal.rs, session.rs, trace.rs, harness.rs
- crates/lumen-static-winit/src/agent.rs (Live::pump), lib.rs (answer_agent, the frame_done call ~455-468, waitSettled path)
- examples/counter-window/src/agent_socket.rs, window.rs; examples/showcase, tasks-demo (agent_addr)
- crates/lumen-static/src/observe.rs; widgets observe impls
- crates/lumen-static-agent/tests/*; .claude/skills/verifying-apps/SKILL.md; scripts/agent_client.py, drive/*
- docs/plan-agent-parity.md, results-agent-cost.md, results-harness.md

**Questions.**
1. Premise check first: confirm the FrameTiming::default() call and the hard-coded settled:true in the current code.
2. After input.click returns frame F in the live window, what does an immediate ui.lastDamage{since:F-1} return? Can two back-to-back RPCs run with no RedrawRequested between them? Print both frame numbers.
3. Does app.perf report non-zero stage timings in showcase with the agent feature? Does any script assert a perf budget that zeros would pass? During a Glass press animation or with an M28 timer pending, what does waitSettled say?
4. Is `handled` exactly pump_with's value? Check a click on a Label and a click on a Button. For a handler that spawns a task, does anything report that its consequence is still pending?
5. For every exported widget: does getTree report the right role, value, label and bounds (a mechanical check over showcase)? Can the residue case in live_nodes (a same-kind wrapper at offset 0) be built from Glass or Veil? Can input.fill target a TextInput in a repeater row? Under Boundaries, does getTree give current values for a skipped subtree?
6. Set-difference of method names: winit dispatch, counter-window socket, the skill, and plan-agent-parity. Which environment variable does each launcher and each client read? Which module-doc sentences are stale?
7. Does replay assert handled per step? If one recorded click is changed to miss, does the replay go red? Does the emitted Rust in session.rs compile today? Does a viewport mismatch hard-error?
8. Does input.fill's End+Backspaces land in field A when the click on B does not take focus?

**Methods.**
- live drive on a unique port (display check first) with agent_client.py
- fault injection: remove the request_redraw after answer_agent; make waitSettled return false; drop the kind comparison in live_nodes; revert T26.2's seeds read; corrupt one recorded session step; rename one method after adding a doc-versus-dispatch check. Record which tests notice (none noticing is a finding)
- read and grep: method-name extraction and set diff
- build: showcase with winit/agent alone versus with widgets/observe; diff the getTree kinds

**Evidence required.** Raw RPC transcripts (request and reply) with frame numbers; the method set diff; per-widget role/value table; injection records. A claim that the agent 'lies' must show the reply beside the ground truth (screenshot, host state).

### R14 — Agent endpoint security, transport robustness and cost
*Priority high; cargo yes. Goals: (3) a safe and reliable agent; (1) and (2): the agent costs nothing when it is not wanted*

**Rationale.** Merges AG3, AG4 and AG6. serve() binds any address with no auth, serves one client at a time with no line cap, splices serde error text into raw JSON, and dies silently if recv fails. showcase enables the endpoint whenever the feature is on, with no debug_assertions guard. frame_done runs every frame even with no client connected (46-54 µs measured headless at 901 nodes).

**Prior claims to test.** winit Cargo.toml: 'Default-OFF ... a release build does not'. Config::agent: 'without the feature the field is inert'. hostile.rs: 'An agent is untrusted input'. results-agent-cost: 45.7/53.7 µs, 0 allocations steady-state.

**Scope.**
- crates/lumen-static-winit/src/agent.rs serve(); lib.rs Config::agent and where serve is spawned; Cargo.toml agent feature
- examples/counter-window/src/agent_socket.rs; examples/showcase agent_addr; examples/tasks-demo
- crates/lumen-static-agent/src/lib.rs dispatch, point_param, selector_param, since_param, MAX_TYPE_CHARS; audit.rs, journal.rs
- crates/lumen-static/src/desc.rs NodeDesc size; benches/src/bin/agent.rs; docs/results-agent-cost.md, step5-observability.md
- crates/lumen-static-agent/tests/hostile.rs, protocol.rs

**Questions.**
1. Threat model: the endpoint is for local development only. Pass criteria: a non-loopback bind is refused or warned about; a release build cannot enable it without an explicit opt-in; a browser-originated request cannot execute a method. Test each criterion.
2. LUMEN_AGENT=0.0.0.0:7654: does it bind silently? Does `printf 'GET / HTTP/1.1\r\n\r\n{...ui.getTree}\n' | nc` return the tree after the HTTP lines are rejected? Does a curl POST with a JSON-line body execute input.click?
3. Can `cargo build --release -p showcase --features agent` ship the endpoint enabled? Are TextInput values that are marked as secrets emitted verbatim?
4. Does an idle first client block a second one? What is the socket thread's RSS after 1 GB is streamed with no newline? Is a reply to `{"a":"\q"}` valid JSON? After a panic in dispatch, does the endpoint survive for the next client?
5. What do input.click{x:1e300}, input.scroll{dy:1e39}, params given as an array, a batch array and a notification without an id return? Is anything rejected with the correct JSON-RPC error code?
6. frame_done cost on the live showcase with a real face, as a ratio to the frame. Idle CPU and wakeups with the feature on and no client connected. Is there a gate asserting that a non-agent release binary links zero agent, accesskit or serde_json symbols and that NodeDesc is 48 B?

**Methods.**
- live drive (display check first) on a unique port: a Python fuzz harness based on agent_client.py; RSS sampling
- fault injection: extend hostile.rs to the transport, and confirm the new tests go red while the length cap or the json! fix is absent; force observe on in the embedded fixture and confirm a size/symbol gate fires
- measurement: `--bin agent` and a temporary Instant around frame_done in a scratch worktree, eight runs a side
- build: nm/cargo tree for showcase with and without the agent feature

**Evidence required.** Transcripts of each transport case with the process state afterwards; RSS figures; symbol counts for the positive control and the release build; frame_done timings with the spread. Security findings state the attacker position required.

### R15 — Platform integration: accessibility, multi-window, DPI, mobile lifecycle, capability modules and images
*Priority medium; cargo yes. Goals: desktop and mobile best in class (a11y, platform behaviour); honest capability claims; (2) 'a tree that holds none carries nothing'*

**Rationale.** Merges DX-1, the non-IME half of DX-3, the lifecycle half of MOB-1, X6 and X12. No crate depends on accesskit_winit, so a11y is agent-only. Pane/Panes are re-exported but never constructed. ScaleFactorChanged does not resize the surface. resumed creates a window on every call and there is no suspended handler. Gestures, in-app drag, dnd and dialogs have never been assembled live. The documented image-decoder feature does not exist.

**Prior claims to test.** Handoff: the AccessKit tree DONE 2026-08-30, serving 'screen readers and agents'. T15.10: multi-window. T14.6: cursor. drag.rs: 'Drag<()> is one byte'. gesture.rs: 'a tree that holds none carries nothing'. image.rs: a decoder 'lives behind its default-off image feature'.

**Scope.**
- crates/lumen-static/src/observe.rs, Cargo.toml observe; crates/lumen-static-widgets/src/*.rs observe impls/set_role
- crates/lumen-static-winit/src/lib.rs (resumed/resumed_impl, window_event_impl ScaleFactorChanged ~621), windows.rs (Pane/Panes), translate.rs (Touch, Dropped), cursor.rs, dialog.rs, clipboard.rs
- crates/lumen-static/src/gesture.rs, drag.rs, anim.rs
- crates/lumen-static-widgets/src/image.rs; ImageSource plumbing; examples/glasswall
- docs/00-handoff-static-tree.md observability (~620-710), step5-observability.md:95, plan-capabilities.md T15.5-T15.10, plan-desktop.md T14.6-T14.8, mobile-decision.md, book/src/platform.md

**Questions.**
1. Does any path in a non-observe build produce an accesskit TreeUpdate or answer a platform a11y request? Does the handoff's 'serves both screen readers and agents' overstate what exists? Per widget: role, name, value, toggled, min/max, selected, modal and expanded, as a table flagging GenericContainer or unnamed nodes. Does TreeUpdate.focus follow Tab? Are arena-row NodeIds stable across scrolls?
2. Can any public entry point open a second window? Is T15.10 marked done anywhere?
3. After ScaleFactorChanged with no following Resized, does the next frame render at the new physical size (canvas dims versus inner_size)? Does the cursor shape update for arena rows and overlays?
4. What does a second resumed call do: a second window, a replaced window, or a panic? Is state preserved across suspend/resume? With Touch events only (Host::post), how many of the widget-drive actions land (handled)?
5. Does any example use a gesture recogniser, an in-app drag or a file drop? Dropping 3 files: one event or three, and are Hovering/Cancelled balanced? While an rfd dialog is open, do redraws, timers and agent RPCs stall? size_of Drag<()>, and the .text delta of naming a LongPress in the firmware.
6. Can a run/Program app supply an ImageSource at all? Does the shell `image` feature named in image.rs exist? What does the GPU image atlas do with an image larger than the atlas?

**Methods.**
- read: grep for accesskit_winit, Adapter, Panes::new, set_ime_cursor_area and ImageSource impls
- live drive (capability checks first: display, HiDPI via WINIT_X11_SCALE_FACTOR, Orca/accerciser if present): scale change, xdotool file drop, dialog-open polling
- headless: touch-only replay of inventories; a resumed-twice test
- fault injection: delete set_role in toggle.rs and see what goes red; remove drag capture routing
- build: a firmware variant with gesture/drag and a size diff

**Evidence required.** The per-widget a11y table; greps with zero hits shown verbatim; the canvas size against inner_size; counts of landed touch actions; any stall with its duration. Claims about missing capabilities cite the doc sentence that claims them.

### R16 — Widget set, lints, derive robustness, numeric hygiene, data-heavy scaling and book coverage
*Priority medium; cargo yes. Goals: a best-in-class widget set; developer experience; (2) derives that refuse misuse; (1) performance on real content*

**Rationale.** Merges DX-5, DX-6, DX-8, C7, C9, X9 and the selector-grammar part of DX-9. M27's harness found render-only defects only because a person looked at the render; nothing checks contrast or target size. W1003 is unreachable. There may be no compile-fail tests for common mistakes. Generated code puts locals (changed, st, f) in scope next to user expressions. A NaN in a style silently disables every skip and memo hit. The book's gaps.md denies menus and rich text, which shipped in M27.

**Prior claims to test.** M27: 'thirty-three widgets, checked name by name', 94 actions green both ways, §5's adversary 'passes its own gate'. T26.14: a widget_id typo is a build error. T31.2 E0119 is 'checked both directions'. AGENT.md: the book done 2026-09-10, with prose moving in the same commit.

**Scope.**
- crates/lumen-static-widgets/src/*.rs (esp. rich.rs, textarea.rs, data.rs, chart.rs, slider.rs, field.rs, split.rs)
- crates/lumen-static-agent/src/audit.rs (W1001-W1011); crates/lumen-static/src/style.rs (Rule, cascade, lint_sheet_deep, selector matcher, Palette)
- crates/lumen-static-macros/src/lib.rs (parse_wid, parse_classes, expand_app binds/after_update/tick, attributes list), tests/ui/*
- crates/lumen-static/src/ops.rs, crates/lumen-static-widgets/src/ops.rs, num.rs; tests paint_containment.rs, sheet_lint.rs
- examples/showcase/src/catalog.rs; scripts/drive/widgets/*.py
- book/src/*.md (gaps.md, leaves.md, input-widgets.md, containers.md, overlays.md, attributes.md, styling.md); scripts/check_attr_table.sh; docs/plan-widget-parity.md §3b §5

**Questions.**
1. For each W10xx: the minimal tree that should trigger it and the nearest tree that should not. Does it fire exactly in the first case? Which lints have both a positive and a negative test? Is W1003 dead?
2. Ten likely beginner mistakes (bind to a missing field, a bind type mismatch, a wrong handler signature, a forgotten #[tree], a forgotten axis, a widget_id typo, a duplicate id across instances, a selector matching nothing, a hidden node taking focus, an allocating Repeat bind): is each a compile error, a lint, a panic, or silent? Paste each message. Do spans point at the user's field?
3. A #[bind] expression or after_update that names changed, f or st: does it fail, compile wrongly, or work? Which attribute misuses are silently accepted (an unknown key, two #[tree] fields, both #[widget(row)] and #[widget(column)])? Does each compile_fail test fail for its stated reason when its check is deleted?
4. Per widget, with the default palette: text/background contrast and hit-box size, listing everything below 4.5:1 or 24x24 px. Plant a Tabs selected-label colour equal to its fill: does widget-drive or the audit go red? Is each const KIND of Strip, Veil, Chooser and Picker driven separately? Do empty data, very long labels and narrow windows panic or paint outside the box?
5. A sheet with spacing=NaN, font_size=-1, padding=inf and radius=NaN: any panic, any non-finite rect, a silent loss of every skip? Does negative padding let a child escape a non-clipping parent? What do Slider, RangeSlider, NumberInput and SplitPane do with min>max, NaN or a zero step?
6. Scaling: TextArea keystroke time at 1 KB, 100 KB and 1 MB; Markdown with 10k unclosed '**'; Chart with NaN, inf, empty and 1e6 points; Tree and DataGrid at 100k rows. Print the growth ratio.
7. Is the agent's selector grammar the same as the stylesheet matcher's (~=, >, %k)? Which widgets hard-code colours that bypass the palette?
8. Book: list every claim of absence that is now false; list exported widgets, traits and derive attributes never mentioned; diff the macro's parsed attributes against both attribute tables.

**Methods.**
- trybuild candidates in a scratch copy, reading the .stderr
- fault injection: disable each lint's condition, and check that at least one test fails by its code; delete each macro syn::Error return; the planted colour defect
- proptest/fuzz over Style values plus an event stream in a headless Host, asserting finite rects and printing the skip-hit rate
- measurement: per-widget micro-benches at three sizes, eight runs a side
- mechanical scripts: exported names versus book mentions; Color literal grep; lint positive/negative matrix

**Evidence required.** The lint positive/negative matrix; compiler messages verbatim; contrast and size tables computed from the laid-out tree; growth ratios with the spread; the book-coverage list; injection records.

### R17 — Gate reality: CI, silent skips, self-tests of the drift checks, dependency hygiene and number currency
*Priority high; cargo yes. Goals: process integrity: every goal's claims rest on gates that actually run and can fail*

**Rationale.** Merges V1, V2, V3, V8 and V9. There is no git remote, so ci.yml has probably never run, and its comments are stale. Live gates exit 0 with no display. GPU and font tests return early with a captured eprintln. Test files compile to zero tests without a feature. check_no_unsafe misses // inside string literals. check_attr_table compares two tables, never the parser. There is no deny.toml and no --locked. The numeric claims in AGENT.md are never re-derived.

**Prior claims to test.** ci.yml: 'There was none before this file'; 'the dependency tree is six crates'. check_no_unsafe header. AGENT.md: 'Six checks in just static mechanise the part a human reliably misses'; 'Numbers are stated once'. justfile: `just test` is 'deterministic, no numbers'.

**Scope.**
- .github/workflows/ci.yml; justfile (all recipes); rust-toolchain.toml
- scripts/check_no_unsafe.sh, check_derive_attrs.sh, check_attr_table.sh, check_doc_paths.sh (+allow), check_doc_identifiers.sh (+allow), check_book_includes.sh, check_text.sh, check_bss.sh (+allow)
- scripts/widget_drive.sh, agent_drive.sh, tasks_drive.sh, wake_gate.sh, gpu_parity.sh, live_gate.sh
- crates/lumen-static-wgpu/tests/parity.rs, glass_parity.rs; crates/lumen-static-shell/tests/shaping.rs, glyph_placement.rs; crates/lumen-static-text/tests/shaping.rs; crates/lumen-static/Cargo.toml [[test]] required-features
- Cargo.toml, Cargo.lock, benches-competitive/Cargo.lock
- AGENT.md, docs/results-*.md, docs/00-handoff-static-tree.md DECIDED markers

**Questions.**
1. Has ci.yml run anywhere? Which of its comments are false today? In a clean container (or `act`), at which step does check-extra first fail?
2. Table: every recipe and script that asserts something, marked as in `test`, in `check`, in CI, or manual-only. Which milestone-defining gates are manual-only? Does anything build benches-competitive?
3. Per-binary `cargo test -- --list` counts for release-shape, dev-shape, simd, threads and gpu: which binaries report 0 tests where the docs imply they run? On a machine with no font, no adapter and no display, how many tests and gates go green while doing nothing?
4. One planted violation per drift-check script (8 scripts): does each fire by name? Specifically: unsafe after a "//" literal; a derive attribute missing from both tables; `--bin serde` and `-p nonexistent` cited in a doc; stale allowlist entries.
5. cargo deny check advisories/licenses/bans/sources and cargo audit on both lockfiles; cargo tree -d per shipping profile; the exact normal-dependency closure of lumen-static --no-default-features on thumb. Can it be pinned?
6. Numeric claims, from a fixed list chosen in advance: test binary count 151; widgets 33 (and 34 exported); drivable entries 39 and actions 94; App impls 19; firmware .text 27,024 B; Boundaries<256>; bss.allow and doc-identifiers.allow sizes; Cargo.lock package count; the 'six crates' in ci.yml; lint 4.04x and 5.9 ns/node (whose arm still runs); 15 DECIDED markers; 20 cross-file rustdoc paragraphs. How many are stale?

**Methods.**
- read and grep: map each recipe to what it runs
- build matrix: the ci.yml steps verbatim in a clean rustup home
- fault injection: `unset DISPLAY; just widget-drive; echo $?`; an empty fontconfig dir; an unavailable WGPU_BACKEND, recording the green counts; one planted violation per script in a scratch worktree
- measurement: re-derivation commands for each numeric claim, printed claim-versus-actual

**Evidence required.** The recipe/gate reality table; skip counts per environment; a fired/not-fired record per planted violation; deny/audit output; a claim-versus-actual table with each re-derivation command.

### R18 — The next vacuous tests: compile_fail doctests, thread-blind allocation counters, disabled debug assertions, strategy-blind drive harness, mutants and flakiness
*Priority high; cargo yes. Goals: all goals; the two vacuous-test shapes in AGENT.md*

**Rationale.** Merges V4, V5 and V6. Four compile_fail doctests have no error code and pass on any error. The allocation counters are thread-local, so they cannot see the threads rasteriser or task workers. lumen-static-shell runs its tests with debug-assertions=false. widget_drive's strategy is only a label, and nothing asks the binary what it was built with. Ports and /tmp logs are shared across worktrees. The const-budget wall clock and sleep-polling tests flake under load.

**Prior claims to test.** style.rs: 'A misspelled id fails the build'. text.rs: misuse is 'a compile error at the binding site'. results-allocation: 'Every path asserts zero'. M27: 'green both ways'; 'coverage as an assertion'. justfile: `just test` deterministic.

**Scope.**
- crates/lumen-static/src/text.rs:339, style.rs:314/1250/1265 (compile_fail doctests); crates/lumen-static-macros/tests/ui.rs
- crates/lumen-static-shell/tests/alloc_gate.rs; crates/lumen-static/tests/enum_alloc.rs; every #[global_allocator] in tests, benches and fixtures
- Cargo.toml [profile.dev.package.lumen-static-shell]
- crates/*/tests/*oracle*, *damage*, *invalidation*; fixtures/miri-smoke
- scripts/widget_drive.sh, drive/run.py, harness.py, widgets/*.py, ui_check.py, live_gate.sh
- crates/lumen-static-shell/tests/failure.rs, spawner.rs, batch.rs; justfile const-budget

**Questions.**
1. For each compile_fail doctest: does the fixed sibling compile? Does the failing version fail with the intended error? Delete the mechanism (the TextBuffer bound, the wid_index! duplicate refusal): does the doctest go red? Add error codes where one exists.
2. Under --features threads, make the band workers allocate: does any allocation gate see it? Is there a process-wide counter used anywhere?
3. With debug-assertions and overflow-checks on for lumen-static-shell, does its suite still pass? Each failure is a latent defect.
4. Every test that compares two strategies, backends or builds: does it independently assert that its scenario happened (handled, a changed count, the hit node)? List those that do not.
5. Can the running binary report its strategy and features over the agent? Build the second widget-drive binary identical to the first (drop boundaries): does the drive stay green? Run with a leftover process on 7656: does anything notice? Delete one inventory and one action row: which gate fires? Is there any check that each exported widget has an inventory?
6. ui_check bounds: print each measured value beside its bound and flag headroom above 5x. Does live_gate fail if the app panics after ui_check? Is the T27.10 planted weakness recorded so it can be re-run?
7. cargo-mutants on named functions only (Boundaries::mark, subtree_range, Incremental::finish, Regions::push, dispatch_overlaid, table_is_well_formed) with a time budget: list the surviving mutants.
8. `just test` 20 times back to back and 10 times under load: which steps fail? The const-budget ms distribution against 5 000. nextest versus libtest differences. Shared statics or env::set_var in tests.

**Methods.**
- fault injection: targeted deletions per doctest, alloc gate, oracle and harness case
- build: a scratch profile with debug assertions for shell
- measurement: repeated runs under stress-ng or a parallel build (only in the load-test window, announced to the other reviewers via the bench lock)
- live drive on unique ports, display check first

**Evidence required.** For each test examined: the injection, and whether it stayed green (a vacuous test) or went red. The surviving-mutant list with locations. Flake rates with the load conditions. The strategy-blindness demonstration transcript.
