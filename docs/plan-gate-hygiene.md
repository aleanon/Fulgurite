# M43 — gates that run, refuse what they claim to, and say when they did not run

> **T43.0–T43.15 DONE 2026-09-26.** T43.0–T43.12 and T43.14 on branch
> `impl/m43`; T43.13 and T43.15 on `impl/m43b`, after M42/T42.2. One line
> per task:
>
> - **T43.13** `just live` (8 gates, 8 passed; no display → eight
>   `skip(77)`, exit 77), `just ci`/`ci-verify` (steps derived from
>   `check`'s closure at the commit; record `target/ci/<sha>.json`; a
>   dirty tree fails), `install-hooks`, `bench-lock`, `scripts/probe.sh`
>   (thumb, qemu, miri); the firmware gates moved into `test` (§2.10).
> - **T43.15** `selftest` in `test`, `selftest-build` in `check`, `slow`
>   and `live` in `ci`; a check replaced by `exit 0` fails `just test`
>   (`stayed green`); `docs/results-deep-review.md` §4 addendum and §5.
>   The full `just ci` record is at the end of "What building T43.13
>   found".
>
> - **T43.0** `scripts/selftest/`: 37 plants (21 static, 14 build, 2 slow; 38 with T43.12's);
>   at b213ae0 one of the 37 went red by name. All 38 are red now.
> - **T43.1** `forbid(unsafe_code)` on 8/8 crates (all but reload);
>   the scan is literal-aware (`scripts/lib/rustsrc.py`).
> - **T43.2** comments lexed (trailing, block), AGENT.md and manifests
>   read, the allowlist checked (six live entries removed), `--bin`/`-p`
>   checked against parsed metadata, any-fence `ignore` ban.
> - **T43.3** one parser of the derives' own lists; declared = rustdoc =
>   book = 8.
> - **T43.4** firmware path from cargo's JSON; `nm` failure is a
>   failure; the headline re-measured (27,024 B) and held to 1%.
> - **T43.5** const-budget: clean non-incremental build, `wid_index!`
>   ×3; both quadratics red on runs 2 and 3.
> - **T43.6** the agent's dev edge takes `observe`; `just test-crates`
>   (908 tests, nine crates alone, 22 s warm).
> - **T43.7** core's dev edge featureless; `fixtures/observe-tests`;
>   the `lumen_release_shape` tripwire; 416 core tests in the shipped
>   shape.
> - **T43.8** `fixtures/embedded/tests/bind_nostd.rs`: 256 of 1,000
>   stored, under `LUMEN_REQUIRE_NOSTD=1` in `thumb`.
> - **T43.9** goldens run with the shell alone; `Recorded` fails in
>   both files; `check_test_counts.sh` (1 zero-test binary, allowed and
>   verified).
> - **T43.10** `LUMEN_REQUIRE_GPU=1` in `just gpu`; `the_gpu_half_ran`.
> - **T43.11** DejaVu Sans committed (759,720 B); only
>   `font_discovery.rs` needs the system; `just fonts`.
> - **T43.12** the competitive lock regenerated; `check_locks.sh`;
>   `--locked` everywhere there; `deny.toml` with two accepted
>   advisories.
> - **T43.14** the thumb target declared and checked; ci.yml dormant;
>   no bare counts (`check_ci_claims.sh`).
>
> **What building found (2026-09-26)** is at the end of §3.
>
> **Written 2026-09-24.** This plan closes 27 findings from
> M38's deep review (`docs/results-deep-review.md` §2.3, §2.4, §4 and
> §5), re-triaged against `main` at 1717808 on 2026-09-24:
>
> - **Static drift checks:** R17-F2, R4-F4, R17-F3, R17-F4, R17-F5,
>   R17-F9, R17-F15, R17-F16, R12-F7.
> - **Gates that never run:** R17-F1, R9-F4, I-NEW-4, R12-F1, R12-F2,
>   R13-F7, R3-F8.
> - **Skips and the runner:** R17-F6, R17-F7, R17-F8, R17-F10, R17-F11.
> - **Firmware and compile-time gates:** R11-F5, R11-F6.
> - **Locks and number currency:** R17-F12, R6-F6, R17-F13, R17-F14.
>
> Where the review and the re-triage differ, the re-triage wins. §1
> notes where this plan's own reading of 1717808 differs from both.
>
> **Phase 2.** M43 runs alongside M41 and M42. Its local runner
> (`just ci`, T43.13) is the gate that every later milestone's gates run
> in. Phase 4 (M50–M53) waits on it and on the fixed firmware scripts
> (T43.4). T43.13 also depends on M42/T42.2, because the live scripts
> gain their skip exit code there. The two milestones interleave rather
> than wait on each other: T43.0 → M42/T42.0 (its live plants use this
> harness) → M42/T42.2 → T43.13, and T43.15 last, after M42's plants are
> green. No task depends on a task that depends on it.
>
> Three facts at 1717808 shape the plan:
>
> - **There is no git remote.** `.github/workflows/ci.yml` has never
>   run, and a pre-push hook would never fire.
> - **Cargo's feature unification** decides which tests exist in a given
>   build.
> - **Every drift script in `just static` was written to refuse
>   something**, and the review found each one accepting some form of
>   what it claims to refuse.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that turns it red,
prints the measured quantity beside its bound, and shows that it
accepts good input in the same session.*

---

## 0. The rules for this milestone

1. **Every refusal gets a planted violation that lives in the
   repository.** A check in `just static` or `just check` is not done
   until `scripts/selftest/` plants the thing it refuses and watches it
   go red by name. The review showed most of these checks failing once,
   in scratch commits that nothing re-runs. §4 of the results doc is
   the list of what that cost.
2. **A plant must prove that it applied.** An edit that matched nothing
   leaves the check green for the wrong reason. The review's `.bss`
   plant was dropped by the linker, and the awk locale probe could
   never fire. The harness therefore asserts the edit took effect
   before it reads the verdict.
3. **A skip is not a pass.** A gate that did not run says so with an
   exit status that the aggregate can refuse (§2.2).
4. **A test binary that lists zero tests is a finding**, unless an allow
   file says why. Feature unification made `golden.rs` exactly that, in
   every automated build.
5. **The gate set runs from a clean checkout, and leaves it clean.**
   Otherwise it proves the working tree and not the commit. At 1717808
   `just test` rewrites a committed file (§1.5).

## 1. What is wrong, at 1717808

### 1.1 Feature unification decides which tests exist

**The golden gates run nowhere.** R17-F1 and R9-F4 are both WORSE, and
I-NEW-4 is new.

- `crates/lumen-static-shell/tests/golden.rs:15` is
  `#![cfg(not(feature = "truetype"))]`.
- Every `--workspace` build turns `truetype` on, so the file compiles
  to zero tests in `dev-shape` (justfile:62) and in ci.yml.
- The only `-p lumen-static-shell` lines are justfile:64 (`shadow`),
  `simd` (:132-134) and `threads` (:146). None of them names `golden`.

The findings got **worse** because M36/T36.2 (5641723) added a second
golden gate with the same cfg:
`failure_screen.rs:81` (`the_screen_matches_its_golden_image`). It also
matches `v => println!` for every verdict except `Differs` (`:94-95`).
So a deleted golden is re-recorded and the test passes. That is
I-NEW-4, and it contradicts `Verdict::Recorded`'s own doc: "A first run
is not a pass". The re-triage corrupted `panic_screen_480x320.ppm`. The
dev-shape selection stayed green, and the shell built alone went red.

**Core's release-shape tests compile with `observe` on (R12-F1).**

- justfile:30-31 calls `release-shape` "the ONLY build" of the shipped
  shape.
- `crates/lumen-static/Cargo.toml:52` gives the core a dev-dependency
  on `lumen-static-widgets` **with `observe`**. Widgets' `observe`
  turns core's on, so `cargo test -p lumen-static` (justfile:48) and
  the clippy line (:49) both build core in the dev shape.
- The re-triage re-ran the `520ed2e` tripwire. `--tests` went red, and
  the lib alone stayed green.

**No gate executes core's no_std-only branch (R12-F2).**
`crates/lumen-static/src/bind.rs:89-90` defines `SCRATCH_CAP`, and
`:164-165` is the only place that uses it. The branch is not reachable
from core's own tests, for R12-F1's reason. Neither `fixtures/embedded`'s
source nor its one test (`baked_face.rs`) contains a `#[bind]`.

**The agent's tests pass only through unification.** R13-F7 and R3-F8
are one defect found twice.

- `crates/lumen-static-agent/Cargo.toml:34` takes `lumen-static-widgets`
  without `observe`.
- `cargo test -p lumen-static-agent` fails 19 tests.
- `--workspace` passes them, because core's dev edge (above) turns
  widgets' `observe` on.

**A consequence the triage did not state:** fixing R12-F1 by removing
core's `observe` edge takes away the only thing that keeps the agent's
tests green. The two fixes are ordered (T43.6, then T43.7).

### 1.2 A skip looks exactly like a pass

- **GPU parity, R17-F6.** With no adapter, `parity.rs` and
  `glass_parity.rs` return early. The comment at `parity.rs:65` says
  the skip "Prints why, so a skipped test does not look like a passing
  one". libtest captures that print on a pass.
- **Fonts, R17-F7.** Several shaping tests return early when no system
  face exists, and some print nothing when they do. The sites are
  `lumen-static-shell/tests/shaping.rs:9` and its `else { return }`
  sites, `glyph_placement.rs:32`, `lumen-static-text/tests/shaping.rs:34`
  and `fixtures/embedded/tests/baked_face.rs:31`. The counts are in the
  review (`docs/results-deep-review.md` §2.4, R17-F7).
- **The live gates, R17-F8, WORSE.** They exit 0 on an empty display:
  - `widget_drive.sh:21-23`
  - `agent_drive.sh:23-25`
  - `tasks_drive.sh:16-18`
  - `wake_gate.sh:15-17`
  - `gpu_parity.sh:20-26`, which also skips with no `xwd` and with no
    adapter (`:45`)
  - M36's `panic_drive.sh:9-11`
  - M37's `hot_window_drive.sh:9-11`

  The last two are why the finding is **worse**. `check` (justfile:97-99)
  runs only `live_gate.sh`, and only when DISPLAY or WAYLAND_DISPLAY
  is set. All seven scripts test only DISPLAY, and `live_gate.sh`
  itself tests neither. None of the seven is in `test` or `check`.
- **Nothing runs the gate set without a person.** There is no remote,
  no hooks directory and no `core.hooksPath`. `xvfb-run` is not
  installed. **Checked for this plan:** neither is `jq`, so the
  triage's `jq`-based fixes are written below in Python instead.

### 1.3 The drift scripts accept what they claim to refuse

- **No `unsafe` (R17-F2, R4-F4).** `scripts/check_no_unsafe.sh:29`
  strips from the first `//` on a line, including a `//` inside a
  string literal. It also scans only `crates/*/src` as text, so a
  `#[path]` module outside `src` and a build script's `include!` both
  pass. `#![forbid(unsafe_code)]` is in one crate,
  `lumen-static-widgets/src/lib.rs:56`.
  - The re-triage re-applied `dea0bb6`. All three bypasses passed, and
    `cargo build --release -p lumen-static` succeeded.
  - The re-triage also found that no crate other than
    `lumen-static-reload` contains `no_mangle`, `export_name`,
    `link_section` or `extern "C"`. So a `forbid` in the other seven
    breaks nothing.
- **Identifiers (R17-F15, WORSE).** Three gaps in
  `check_doc_identifiers.sh`:
  - its candidates come only from lines that *start* with `//` (`:53`),
    so trailing and block comments are never read;
  - its markdown list (`:60`) omits AGENT.md;
  - the allowlist is subtracted and never itself checked.

  **Six allowlist entries are live code tokens** today: `AtomicU64`,
  `MARK`, `offset`, `PanicReport`, `ROWS` and `ToggleLook`.
  `PanicReport`'s own entry (`doc-identifiers.allow:113`) says it
  "comes off when it exists". It exists, in
  `crates/lumen-static-shell/src/recover.rs`.
  **Correction to the re-triage:** it says the allowlist grew from 76 to
  83 entries. It did not. Counted the way the script counts, it holds
  76 names at both ac91d51 and 1717808, and the file has no commits in
  between. The finding is **worse** only because two more of its
  entries have gone live since the review.
- **Doc paths (R17-F3).** `check_doc_paths.sh` builds its target set
  from every `"name"` string in `cargo metadata` (`:63-65`), so a
  dependency's name passes as a `--bin`. It never extracts `-p`, and it
  reads `--bin` only from `docs/*.md` and the justfile (`:69`).
  `docs/plan-deep-review.md` quotes the review's own plants verbatim
  (its §-list item 4). Once the check is tightened, that line needs an
  allow entry (T43.2).
- **Book fences (R17-F16).** The ban on `rust,ignore`
  (`check_book_includes.sh:87`) matches only a backtick fence at
  column 0. A tilde fence or an indented fence passes.
- **Attribute tables (R17-F4, R17-F5).**
  - `check_attr_table.sh` compares two hand-written tables and never
    reads the derives' `attributes(...)` lists. It skips with exit 0
    when the book is missing (`:22`).
  - `check_derive_attrs.sh:17` parses those lists one line at a time,
    so a list split across lines disappears. It also counts a comment
    as a use.
  - Its header (`:9`) still says `widget_class` has zero uses, which is
    stale.
  - The App derive (`lumen-static-macros/src/lib.rs:1825`) gained
    `state` in M33.
- **A stripped comment in a manifest (R12-F7).**
  `crates/lumen-static-widgets/Cargo.toml:28-30` has lost its
  backticked names ("`#  implies , the tiering  uses`"). No check reads
  TOML comments.

### 1.4 The firmware and compile-time gates measure something else

- **Target directory (R11-F6).** `check_text.sh:21` and
  `check_bss.sh:19` read a hard-coded `target/…/firmware`. With
  CARGO_TARGET_DIR set, both report on a stale binary and exit 0.
  (The re-triage cites `:20` and `:17`. The lines are `:21` and `:19`.)
- **A failing `nm` (R17-F9).** `check_bss.sh:28` passes an empty
  symbol list when `nm` fails.
- **Stale headline (R17-F13).** The headline of
  `docs/results-embedded.md` (:38-41) still reads the 2026-09-05
  figures, with "now …" notes a milestone behind. AGENT.md carries the
  current `.text`. Numbers are stated once, and here the owner is the
  stale copy.
- **const-budget (R11-F5).** `just const-budget` (justfile:343-354)
  touches `main.rs` and times an incremental rebuild. With the
  quadratic (`b634311`) and the lint allowed, the gate goes red once
  and is then green on every rerun. **The re-triage's numbers: 51,723
  ms on the first run, 159 ms and 155 ms after.** Separately,
  `fixtures/const-budget/src/main.rs` contains no `wid_index!` and no
  `#[widget_id]`, so the fixture never evaluates `count_wid`.

### 1.5 The runner, its specification, and the locks

- **Toolchain targets (R17-F10).** `rust-toolchain.toml` declares no
  `targets`. `thumb` needs `thumbv7em-none-eabihf` (justfile:166 admits
  it), so a clean runner fails at `check-extra`. Severity is S3 now,
  because ci.yml runs nowhere.
- **ci.yml's comments (R17-F11, WORSE).** They are false, and more so
  than at the review:
  - "six crates" (`:60`) lists seven names;
  - "zero … `std::fs`, `std::process`" (`:40`) is contradicted by four
    and two files respectively;
  - "only `crossover` and `bindings`" is contradicted by
    `benches/Cargo.toml`, which declares eight `[[bin]]`s;
  - the portability job's "excluded" fixture (`:201`) is not excluded
    (`:214`).

  **The re-triage's number: `Cargo.lock` holds 418 packages.**
- **Dependency counts in the justfile (R17-F14).** justfile:53 and
  :429 state dependency counts that no longer hold. The re-triage
  counts wgpu at 60 unique crates.
- **The competitive lock (R17-F12, R6-F6).** `benches-competitive/Cargo.lock`
  has not matched its manifests since M30.
  `cargo metadata --locked --offline` there exits 101, and `--locked`
  appears nowhere in the justfile, ci.yml or the scripts.

  **Checked for this plan** (`git archive` of HEAD into scratch):
  `cargo metadata --offline` in `benches-competitive` rewrote the lock,
  adding `lumen-static-widgets`. `just test` runs `observe-inlining`,
  which builds in that workspace (`scripts/check_observe_inlining.sh:16-23`).
  So since T35.0, **the every-commit gate rewrites a committed file**.
  Rule 5 exists because of this.

## 2. Decisions

### 2.1 What runs the gate set, with no remote

Four options were considered:

- **(a) A self-hosted GitHub runner.** It needs a remote, and the
  project has none.
- **(b) Git hooks.** A pre-push hook never fires without a remote. A
  pre-commit hook fires, but it can only afford sub-second work.
- **(c) A local `just ci`** in a clean worktree, which records its
  result.
- **(d) Nothing new**, with a discipline written into AGENT.md.

**Recommended: (c), plus (b)'s pre-commit half as an opt-in
convenience.**

`just ci` keeps one persistent worktree per checkout at `target/ci/wt`,
so its `target/` stays warm between runs. On each run it does this:

1. Detaches the worktree at the SHA, and cleans everything except
   `target/`.
2. Runs `check` (which includes `test`), `live`, the self-test tiers and
   the per-crate test loop (§2.3), with the live scripts on free ports
   (M42 §2.3).
3. Writes `target/ci/<sha>.json` in the invoking checkout. The record
   holds each step's name, exit status, skip flag and duration, plus
   `git status --porcelain` of the worktree taken **after** the run.
4. Fails if that status is not empty (rule 5).

`just ci-verify [sha]` checks that a record exists and is green.
**AGENT.md does not call a milestone done until `just ci-verify`
passes for the commit that closed it,** and the queue entry cites that
SHA.

`just install-hooks` sets `core.hooksPath` to `scripts/hooks`, whose
pre-commit runs `just static`. It is a convenience that `--no-verify`
bypasses. The gate is `just ci`.

ci.yml stays as a **dormant specification** (§2.7). It is not the
runner.

What (c) breaks:

- `just ci` opens windows on the developer's display. That is how the
  live gates run today anyway.
- It takes machine time. §5 gives it a budget.

### 2.2 What a skip is

For a script, the options were:

- **(a) Exit 0 with a loud notice.**
- **(b) A distinct "not run" exit status**, which the aggregate refuses
  unless told otherwise.

**Recommended: (b).** A notice is read by a person, and an exit status
is read by the aggregate, so only the status actually gates anything.
The concrete rules:

- **Every live script exits 77 when a capability it needs is absent**
  (a display, `xwd`, an adapter). 77 is automake's convention. The
  display check itself lives in M42's launcher.
- **`just live` runs all seven live scripts plus `live_gate.sh`.** It
  prints `name: pass | FAIL | skip(77) — reason` per script and a total.
  It fails on any FAIL. It also fails on any skip, unless
  LUMEN_ALLOW_SKIP=1 is set, in which case it prints `N skipped
  (allowed)`.
- **`check` calls `live`** in place of its ad hoc display test (justfile:97-99).
- **Skips inside a test process** (GPU adapter, system fonts) follow
  the same policy through requirement variables. The recipe that names
  the capability sets them, unless LUMEN_ALLOW_SKIP=1:
  - `gpu` sets LUMEN_REQUIRE_GPU=1;
  - `dev-shape` sets LUMEN_REQUIRE_FONTS=1 for the tests that remain
    about system discovery (T43.11).

  A plain `cargo test` outside `just` keeps today's skip-and-return,
  because a developer's partial run is not a gate.

What (b) breaks: `just check` on a headless machine. It now fails
unless the user sets LUMEN_ALLOW_SKIP=1. That is the point. The record
`just ci` writes then carries the skip flag, so a green record never
hides a skip.

### 2.3 Feature unification: which per-package steps

The options:

- **(a)** Loop `cargo test -p` over every workspace member.
- **(b)** Loop over the nine `crates/*` libraries only.
- **(c)** Add targeted per-package lines, plus a zero-test check.

**Recommended: (c) in `just test`, and (b) in `just ci`.**

(c) is cheap:

- the two golden files run with `lumen-static-shell` alone;
- the zero-test check (`scripts/check_test_counts.sh`) catches the
  *next* file that a cfg compiles out, which is what a list of lines
  cannot do.

(b) catches manifests that lean on unification, which is R13-F7's
shape. It is limited to `crates/*`, because those are the published
libraries whose manifests someone else will build alone. Examples and
fixtures are only ever built by this workspace.

(a) was rejected on cost. Each member alone re-resolves features, and
twenty-odd extra feature graphs is minutes of compile for crates whose
manifests are not API.

### 2.4 One self-test harness for every refusal

**Recommended: `scripts/selftest/`.** A plant is a small shell file that:

1. applies an edit in a scratch worktree (created with `git worktree
   add --detach`, and reset between plants);
2. asserts the edit took effect (rule 2);
3. names the check to run, and the string its red output must contain.

The harness first runs every check on the unmodified copy, and they
must all be green. This is the accepting direction, and it catches a
tightened check that now refuses the tree. Then it runs each plant, and
it reports two failure cases by name:

- `plant X: stayed green — expected red`;
- `plant X: edit did not apply`.

The plants come in four tiers:

| tier | recipe | where it runs | cost |
|---|---|---|---|
| static | `just selftest` | `test` | no compile; budget in §5 |
| build | `just selftest-build` | `check` | cargo check/build, firmware links |
| slow | `just selftest-slow` | `ci` | const-budget's 50 s plant |
| live | `just selftest-live` | `ci` (via `live`) | M42's plants; 77 without a display |

**Rejected: a plant as a committed `.patch` file.** Patches rot silently
against moving code, and a patch that fails to apply looks like any
other error. A scripted edit with an asserted effect fails by name.
The one exception is M42/T42.7's adversary plant, which is a patch
because it reproduces a recorded one.

### 2.5 `unsafe`: the compiler is the gate

**Recommended:**

- Add `#![forbid(unsafe_code)]` to `lumen-static`, `-agent`, `-macros`,
  `-shell`, `-text`, `-winit` and `-wgpu`. With `-widgets`, which has it
  already, that is every crate except `lumen-static-reload`.
- Keep the script as a fast check before compiling. Make it
  literal-aware, and add a presence check that every `crates/*/src/lib.rs`
  except reload carries the attribute (`forbid: 8/8 crates`).

`forbid` cannot be overridden by an inner `allow`. It covers `#[path]`
modules and `include!`d code, because both expand inside the crate.

**One limit, stated in the script's header:** `forbid` in
`lumen-static-macros` covers the macro crate's own code, not the tokens
its derives emit. Emitted code is checked by `forbid` in whichever crate
uses the derive, which for the core's own tests is the core.

### 2.6 `release-shape`: what to move out of core's dev edge

The re-triage recommends moving "the widget-using core tests" into a
new crate. It says the alternative, a separate `--lib` step, cannot
work, because dev-dependencies unify anyway. **The move is much smaller
than that sounds.** 63 of core's 70 test files use
`lumen-static-widgets`, but only four need `observe`:

- `tests/observe.rs` and `tests/hatches.rs`, both `required-features =
  ["observe"]`;
- the `cfg(feature = "observe")` sections of `tests/events.rs` and
  `tests/scroll_containment.rs`.

**Recommended:**

- Drop `observe` from core's dev edge, and keep the edge itself.
- Move those two files and two sections into a new workspace package
  under `fixtures/` that depends on core and widgets with `observe`.
- Add the permanent tripwire: a `compile_error!` under
  `cfg(all(feature = "observe", lumen_release_shape))`, with
  `release-shape` setting that cfg in its own target directory.

What this breaks: the agent's workspace tests, until T43.6 gives the
agent's own manifest the feature. Hence the order.

### 2.7 ci.yml

The options:

- **(a)** Delete it.
- **(b)** Keep it runnable and in sync.
- **(c)** Keep it as a dormant specification.

**Recommended: (c).**

- Its header says it has never run, and that `just ci` is the executed
  gate.
- Its comments are cut to what the file itself shows, plus citations of
  `docs/results-*.md`.
- It keeps the job split, because the split is what a future runner
  needs, and each job's steps are `just` recipes.

(b) was rejected because nothing would ever check it. (a) loses the job
split and the portability reasoning.

### 2.8 AGENT.md's vacuous-test table

The M38 results doc's §4 extends AGENT.md's table with the review's
rows. Since then the re-triage has added I-NEW-1..4, and R7-F2 was
re-confirmed at 1717808 after M37 moved the code it names. The options:

- **(a)** Copy every row into AGENT.md.
- **(b)** Keep AGENT.md's rows, which are the history its prose argues
  from, and add **one summary row** that cites
  `docs/results-deep-review.md` §4 for the review's rows and their
  count.

**Recommended: (b).** The five post-review entries become a dated
addendum at the end of that §4. The results doc then owns every row and
the "what caught them" tally.

AGENT.md gains a third shape in its prose: *the gate never runs* —
compiled out by unification, skipped as a pass, or run against a
process it did not launch (M42). It cites the table for the evidence.
(a) would state every number twice, which is exactly what the "numbers
are stated once" rule forbids, and AGENT.md is loaded into every
session.

### 2.9 A number in a doc and the number a gate measures

For R17-F13, `docs/results-embedded.md` owns the `.text`, `.rodata` and
flash figures. `check_text.sh` reads the headline row and fails if the
measurement differs from it by more than 1%. That catches a doc that is
a milestone behind, and lets a toolchain bump through.

For R17-F14 and R17-F11, the justfile and ci.yml state no dependency
counts. They cite the results doc that does. A new check,
`scripts/check_ci_claims.sh`, refuses a bare "N crates" or "N
`[[bin]]`" in either file.

### 2.10 The firmware gates run in `test` (added 2026-09-26, T43.13)

**The finding.** `check_text.sh` and `check_bss.sh` ran only under
`thumb`, which is in `check-extra`, not in `test`. Two branches each grew
the firmware's `.text` against the same 27,024 B headline without
touching it: T45.12 by 308 B and T46.1 by 400 B. The early batch merged
T45.12 and left the headline 1.14% stale with every every-commit gate
green; rebasing M46 onto it made the gap 2.62%. It was caught only
because `check_text.sh` was run by hand (c0c2cfd). §2.9's comparison
was right; it ran in the wrong tier.

The options were:

- **(a)** leave them in `thumb`, and rely on `just ci`, which runs
  `check`'s closure;
- **(b)** move them into `test`, as a `firmware` recipe.

**Decided: (b).** The cost is one thumb release build of core, widgets
and the fixture. Measured at this branch's base (826c7c3), LOADED (load
average 2.3–11.9 on 32 cores, other agents compiling): **1.35 s** for
`check_text.sh` against a firmware built earlier in the session, **0.84 s**
after a one-line change to `crates/lumen-static/src/lib.rs` (cargo
recompiled core, widgets and the fixture), **0.12 s** with nothing
changed, and 0.10 s for `check_bss.sh` after it. That is noise beside
`dev-shape`. (a) would keep the gap open for exactly the case that
produced the finding: a merge checked with `just test` and nothing else.

`thumb` keeps the cross `check`/`clippy` tiers and the no_std bind test.
Both recipes start with `scripts/probe.sh thumb`, so a machine without
the target skips with 77, rather than failing with a linker error that
names something else.

## 3. Tasks

### T43.0 — the gates first: the self-test harness, and every plant red

**Closes:** nothing by itself. It commits the red state of every finding
below.

**Change:** build `scripts/selftest/` (§2.4) with the static, build and
slow tiers, and one plant per finding.

| plant | check | expected once fixed | at 1717808 |
|---|---|---|---|
| string-slash | `check_no_unsafe.sh` | names the file:line of an `unsafe` after a `"http://…"` literal | green |
| path-module, OUT_DIR include | `cargo check -p lumen-static` | `usage of an unsafe block` | builds |
| forbid-removed | the presence check | names the crate | no check |
| trailing, block, AGENT.md | `check_doc_identifiers.sh` | names the dead identifier | green |
| stale-allow (no edit) | `check_doc_identifiers.sh` | names the six live entries | green |
| toml-stripped | the identifiers check over manifests | names the Cargo.toml line | no check |
| bin-is-dependency, bin-is-package, unknown-package, bin-in-book | `check_doc_paths.sh` | names each | green |
| tilde fence, indented fence | `check_book_includes.sh` | names each | green |
| ghost attribute, missing book | `check_attr_table.sh` | `declared, not documented`; `no book` as a failure | green; exit 0 |
| multi-line list, comment use | `check_derive_attrs.sh` | names the unused attribute | green |
| nm-fails (fake `nm` first on PATH) | `check_bss.sh` | `nm failed` | green, 0 symbols |
| stale target dir | `check_text.sh`, `check_bss.sh` | report on the fresh binary, and name the planted static | green on the stale one |
| stale headline | `check_text.sh` | `headline … delta` | no check |
| quadratic, rerun | `just const-budget` | red on run 2 | green on run 2 |
| agent-alone | `cargo test -p lumen-static-agent` | green | 19 failed |
| observe-tripwire | `just release-shape` | the tripwire's message | no tripwire |
| nostd-bind (`fa64952`) | the embedded bind test | `stored 0 of 1000` | no test |
| golden-corrupt, golden-deleted | dev-shape's golden lines | `differs`; `a first run is not a pass` | green |
| zero-tests | `check_test_counts.sh` | names `golden` | no check |
| lock-stale (no edit at 1717808) | `check_locks.sh` | names `benches-competitive` | no check |
| target-undeclared | `check_toolchain_targets.sh` | names `thumbv7em-none-eabihf` | no check |
| ci-claim | `check_ci_claims.sh` | names the line | no check |

The GPU and font plants need an environment rather than an edit (the
adapter hidden, the fonts bind-mounted empty). They are harness *modes*
in the build tier, recorded red the same way.

**Gate:** `just selftest` and `just selftest-build` each print one line
per plant, and a total. At 1717808 both exit non-zero, and the commit
body records the full output. Neither recipe is wired into `test` or
`check` yet. T43.15 wires them in, once every row is green.

**Injection** (for the harness itself):

- A plant whose edit matches nothing must report `edit did not apply`.
- A check replaced with `exit 0` must make its plants report `stayed
  green`.
- **Accepts good input:** the baseline pass over the unmodified copy
  prints `baseline: N checks green`.

**Docs in the same commit:** the header of `scripts/selftest/run.sh`,
covering plant format, tiers and the two failure reports. That covers
AGENT.md's rules 1 and 2.

**Effort:** L. **Depends on:** nothing. M42/T42.0 builds its live
plants on this harness.

### T43.1 — `forbid(unsafe_code)`, and a script that cannot be fooled by a string

**Closes:** R17-F2, R4-F4.

**Change:**

- Add the attribute to the seven crates (§2.5).
- `check_no_unsafe.sh` strips string, raw-string and char literals
  before it strips comments.
- It gains the presence check.
- Its header's claim that "it cannot hide a real `unsafe` token" is
  removed. The header says instead that the compiler is the gate and
  the script is the early warning.
- Re-check that the tree builds under `forbid` with winit's `hot` and
  `gpu,agent` features, and the shell's `truetype,agent,reload`.

**Gate:**

- `cargo check --workspace` (in `dev-shape`), with `forbid`.
- The script prints `no-unsafe: clean; forbid: 8/8 crates`.

**Injection:** the string-slash, path-module and OUT_DIR plants.
`cargo check -p lumen-static` must fail three times with `usage of an
unsafe block`. The forbid-removed plant makes the presence check name
the crate.

**Accepts good input:** the unmodified tree builds, and the script
prints 8/8.

**Docs in the same commit:**

- the script's header;
- AGENT.md's doc-currency paragraph ("the core must contain no
  `unsafe`") says how it is enforced;
- `docs/00-handoff-static-tree.md`, wherever it states the mechanism.

**Effort:** S. **Depends on:** T43.0.

### T43.2 — the doc drift checks read what they claim to

**Closes:** R17-F15, R17-F3, R17-F16, R12-F7.

**Change:**

- **`check_doc_identifiers.sh`:**
  - comment text is extracted by a literal-aware pass that emits
    trailing `//` text and `/* … */` spans;
  - AGENT.md is added to the markdown set;
  - Cargo.toml comments are scanned for the stripped-span shape (two
    spaces before punctuation), which catches R12-F7;
  - a stale-allowlist pass: an entry that is a live code token, or that
    is cited nowhere, fails;
  - the six stale entries are removed, and the Cargo.toml comment is
    restored, in the same commit.
- **`check_doc_paths.sh`:**
  - the bin set is `kind == bin` targets, and the package set is
    package names, both parsed from `cargo metadata` with Python;
  - `-p` and `--package` are extracted as well as `--bin`;
  - AGENT.md, `book/src` and the non-check scripts are scanned;
  - `docs/plan-deep-review.md`'s verbatim quotation of the review's
    plants gets an allow entry, with the reason "a plan quoting the
    plants it asked for". Rewording a historical plan is the worse
    choice.
- **`check_book_includes.sh`:** the fence pattern accepts up to three
  spaces of indent and a tilde or backtick fence of any length.

**Gate:** the three scripts in `just static`. They print, respectively:

- `candidates N (line L, trailing T, block B, toml K), allowlist A, stale 0`;
- `bins cited B live / B, packages cited P live / P`;
- `fences: N rust, M no_run, 0 ignore`.

**Injection:** the plants from T43.0's rows for these scripts. The
stale-allow plant is the unmodified tree before this commit, which is
red at 1717808.

**Accepts good input:** the tree after this commit passes all three.

**Docs in the same commit:**

- each script's header;
- `check_doc_paths.sh`'s "Two are dangling today" is removed;
- AGENT.md's list of the checks in `just static` stops stating a count,
  and cites the justfile.

**Effort:** M. **Depends on:** T43.0.

### T43.3 — the attribute checks read the parser

**Closes:** R17-F5, R17-F4.

**Change:**

- Add one shared parser, a small Python function in
  `scripts/selftest/`'s support module or beside the two scripts. It
  reads every `#[proc_macro_derive(…, attributes(…))]` across lines and
  asserts that the number of lists equals the number of
  `#[proc_macro_derive`.
- `check_derive_attrs.sh` counts only uses outside comments.
- `check_attr_table.sh` requires the declared set, the rustdoc table
  and the book table to be equal. A missing book is a failure.

**Gate:** both scripts in `just static`. They print `derives 3,
attributes 8`, then per attribute its use count, and then `declared 8,
rustdoc 8, book 8`.

**Injection:** the multi-line-list, comment-use, ghost-attribute and
missing-book plants.

**Accepts good input:** the unmodified tree passes at 8/8/8.

**Docs in the same commit:** both headers. `check_derive_attrs.sh:9`'s
stale `widget_class` claim is removed.

**Effort:** S. **Depends on:** T43.0.

### T43.4 — the firmware scripts measure the binary cargo built, and fail when they cannot

**Closes:** R11-F6, R17-F9, R17-F13.

**Change:**

- Add `scripts/lib/firmware.sh`. It resolves the firmware path from
  `cargo build --message-format=json`, parsed with Python, so it follows
  CARGO_TARGET_DIR. Both scripts use it and print the path they read.
- `check_bss.sh` runs `nm` into a variable and checks its status. It
  requires at least one symbol, because the fixture's own exempted
  static is always present.
- Rewrite `docs/results-embedded.md`'s headline table with today's
  measurement, and keep the history below it, dated.
- `check_text.sh` reads the `.text` headline from that table and fails
  on a delta above 1% (§2.9).

**Gate:** `just thumb` prints:

- `bss: N symbol(s) (nm: <path>), all N exempted`;
- `text: M B measured, H B in the results doc's headline (delta d%,
  tolerance 1%)`.

**Injection:** the nm-fails, stale-target-dir and stale-headline plants.
The stale-target-dir plant is a static planted in `pass::paint`, built
with CARGO_TARGET_DIR elsewhere and a clean binary left in `./target`.
It must go red, naming the static.

**Accepts good input:** the unmodified build passes under both the
default and an overridden target directory.

**Docs in the same commit:**

- both scripts' headers;
- `docs/results-embedded.md`'s headline;
- AGENT.md's M30 entry cites the headline instead of carrying the
  `.text` figure.

**Effort:** S. **Depends on:** T43.0. M52's embedded tasks (R11-F1/F2/F4)
build on `scripts/lib/firmware.sh`.

### T43.5 — const-budget times const-eval, not a no-op

**Closes:** R11-F5.

**Change:**

- The recipe runs `cargo clean -p const-budget` and builds with
  CARGO_INCREMENTAL=0 in its own target directory, so every run pays
  for const-eval.
- Give the fixture several `#[widget_id]` fields and one `wid_index!`,
  so the fixture evaluates `count_wid`.
- The recipe asserts that the fixture uses `wid_index!` at least once
  (by grep), so a later edit cannot silently drop it.
- Re-state `docs/results-const-eval.md`'s "Current" figure as the
  non-incremental build, and say what it measures.

**Gate:** `just const-budget` prints `const-budget: N ms for 3711 nodes
(non-incremental, bound 5000; wid_index uses: k)`.

**Injection:** the quadratic plant, with the lint allowed, is red on
**run 2 and run 3**, not only on run 1. The deciding check is that it
goes red on a rerun. A quadratic in `count_wid` must also go red, which
proves the fixture reaches it.

**Accepts good input:** reverted, the gate is green, and the doc's new
figure is printed beside the bound.

**Docs in the same commit:**

- `docs/results-const-eval.md`;
- the fixture's rustdoc ("a build that takes minutes");
- the justfile comment.

**Effort:** S. **Depends on:** T43.0.

### T43.6 — the agent's tests pass alone, and every library is tested alone

**Closes:** R13-F7, R3-F8.

**Change:**

- `crates/lumen-static-agent/Cargo.toml:34` takes
  `lumen-static-widgets` with `observe`, with the comment that core's
  own dev edge carries.
- Add `just test-crates`, which loops `cargo test -p` over the nine
  `crates/*` libraries (§2.3). It prints `crate: P passed, F failed`
  per crate and a total, and it is in `just ci`.

**Gate:** `just test-crates`.

**Injection:** the agent-alone plant, which removes the feature. The
recipe reports `lumen-static-agent: 19 failed`, while `cargo test
--workspace` stays green. That contrast is the point of the gate.

**Accepts good input:** with the feature, the recipe reports 0 failures
across all nine crates.

**Docs in the same commit:**

- the agent's Cargo.toml comment;
- `docs/results-widget-crate.md`'s M30 feature-seam note cites the
  recipe;
- the justfile comment.

**Effort:** S. **Depends on:** T43.0. The loop's cost is measured here
(§5).

### T43.7 — `release-shape` really is the release shape

**Closes:** R12-F1.

**Change:** §2.6.

- Drop `observe` from `crates/lumen-static/Cargo.toml:52`.
- Move `tests/observe.rs`, `tests/hatches.rs` and the two cfg'd sections
  into the new package.
- Add the `lumen_release_shape` tripwire in `crates/lumen-static/src/lib.rs`.
- `release-shape` sets the cfg in its own target directory, and prints
  the resolved feature set of `lumen-static` from `cargo tree -e
  features` together with the core test count.

**Gate:** `just release-shape`. The tripwire fires if any edge turns
`observe` or `reload` on. The printed test count must be non-zero, so
an emptied test set is visible.

**Injection:** the observe-tripwire plant, which re-adds `features =
["observe"]` to a core dev-dependency. The recipe fails with the
tripwire's message.

**Accepts good input:** without the plant, the recipe is green, and it
prints the core test count and a feature set without `observe`.

**Docs in the same commit:**

- justfile:30-37;
- `docs/verification-notes.md`'s scope line;
- the dev-dependency comment in core's Cargo.toml;
- the moved tests' headers say why they live apart.

**Effort:** M. **Depends on:** T43.6. Without it, dropping the edge
turns the agent's workspace tests red.

### T43.8 — the no_std bind branch runs

**Closes:** R12-F2.

**Change:**

- Add `fixtures/embedded/tests/bind_nostd.rs`. It is an application
  with a `#[bind]` label, defined **in the test**, so the firmware's
  `.data` is not changed (M52's F-NEW-1 concern does not arise). The
  test asserts:
  - the label's text after `update`;
  - a 1000-byte format stores exactly 256 bytes;
  - truncation at a char boundary;
  - `update_marked` reports the bound node.
- `fixtures/embedded` builds core with `default-features = false` and
  has no dev edge that turns std on, so this is the no_std branch.
- The stored-byte count is the proof: under std, the same label grows
  past 256.
- Add `cargo test -p embedded` to `thumb`.
- Document the 256-byte cap in `#[bind]`'s rustdoc.

**Gate:** the new test prints `stored N of 1000, cap 256`.

**Injection:** the nostd-bind plant (`fa64952`: `format_into` clears its
buffer). The test goes red with `stored 0 of 1000`, while `cargo test -p
lumen-static` stays green.

**Accepts good input:** reverted, the test stores 256.

**Docs in the same commit:**

- the rustdoc in `lumen-static-macros/src/lib.rs` for `#[bind]`;
- `bind.rs:74-89`;
- `docs/results-embedded.md` notes that bind is exercised.

**Effort:** S. **Depends on:** T43.0.

### T43.9 — the golden gates run, and a first run is not a pass

**Closes:** R17-F1, R9-F4, I-NEW-4.

**Change:**

- `dev-shape` gains `cargo test -p lumen-static-shell --test golden
  --test failure_screen` next to justfile:64.
- `failure_screen.rs` treats `Verdict::Recorded` as a failure ("commit
  it and re-run"), and accepts `Updated` only under LUMEN_UPDATE_GOLDENS=1.
- Add `scripts/check_test_counts.sh` at the end of `dev-shape`. It runs
  the workspace test binaries with `--list` and fails on any binary
  listing zero tests that is not named in `scripts/zero-tests.allow`
  with a reason.

The heavier fix was considered: making `golden::compare` work under
`truetype` so that the cfg can go. It was rejected here. It changes
what a golden means, and the zero-test check covers the hazard for
every future file, not only these two.

**Gate:**

- The new recipe line prints `running 2 tests` for `golden` and
  `running 4 tests` for `failure_screen`. The failure screen prints
  `480x320: Match` and `260x420: Match`.
- `check_test_counts.sh` prints `<binary>: N tests` for every binary,
  and `zero-test binaries: K (allowed A)`.

**Injection:**

- Golden-corrupt makes the recipe fail at `failure_screen.rs:94`.
- Golden-deleted makes it fail with `a first run is not a pass`.
- The zero-tests plant, which removes the recipe line and keeps the
  check, names `golden`, because that is where the cfg compiles it out.

**Accepts good input:** both restored, the check prints 0 unallowed
binaries.

**Docs in the same commit:**

- the justfile's `dev-shape` comment;
- `docs/results-harness.md` §T12.2 says where the gate runs;
- `docs/plan-panic-recovery.md`'s T36.2 gate row;
- AGENT.md's M12 entry.

**Effort:** S. **Depends on:** T43.0.

### T43.10 — the skip policy, and a GPU gate that cannot pass without a GPU

**Closes:** R17-F6.

**Change:**

- Implement §2.2's in-process half. `parity.rs`'s adapter helper panics
  when LUMEN_REQUIRE_GPU=1, instead of returning `None`.
- Add a test, `the_gpu_half_ran`, that asserts the adapter exists under
  the requirement and prints its name and backend.
- `gpu` sets the variable unless LUMEN_ALLOW_SKIP=1.
- Record the skip policy in `docs/results-gpu.md`.

**Gate:** `just gpu` prints `adapter: <name> (<backend>)`.

**Injection:** the GPU plant (the adapter hidden by environment). It
fails with `LUMEN_REQUIRE_GPU=1 but no adapter`. Remove the variable
from the recipe while hiding the adapter, and the recipe goes green
again. That shows the mechanism is the requirement and not a flake.

**Accepts good input:** the normal environment passes and prints
llvmpipe.

**Docs in the same commit:**

- `parity.rs:65`'s comment, which is now true;
- the justfile's `gpu` comment;
- `docs/results-gpu.md`.

**Effort:** S. **Depends on:** T43.0.

### T43.11 — text tests shape a committed face

**Closes:** R17-F7.

**Change:**

- Commit a test face under `crates/lumen-static-shell/tests/fonts/`.
  DejaVu Sans is on the reference machine, and its licence permits
  redistribution. `fontTools` is not installed, so the face is
  committed whole rather than subset, and its size is recorded in the
  commit.
- The shaping tests load it through `FontStack::from_bytes`.
- Only tests *about* system discovery keep `FontStack::system`. They
  panic under LUMEN_REQUIRE_FONTS=1, which `dev-shape` sets (§2.2).
- `baked_face.rs` does the same with its source path.

**Gate:** each affected test asserts a non-zero glyph count and prints
`face: <name>, glyphs: N`.

**Injection:** the font plant (fonts hidden with `unshare -rm` and an
empty bind mount).

- The pinned-face tests still pass and print their glyph counts.
- The discovery tests fail under the requirement.
- A wrong path for the pinned face fails with `pinned face not found`,
  never a silent return.

**Accepts good input:** a normal environment passes all of them.

**Docs in the same commit:**

- the headers of the affected test files;
- `docs/results-harness.md`, on host-font independence.

**Effort:** M. **Depends on:** T43.10, for the policy variable.

### T43.12 — locks that match, and a gate set that leaves the tree clean

**Closes:** R17-F12, R6-F6.

**Change:**

- Regenerate and commit `benches-competitive/Cargo.lock`.
- Add `scripts/check_locks.sh` to `just static`. It runs `cargo metadata
  --locked --offline` for both workspaces.
- Add `--locked` to `check_observe_inlining.sh`'s builds and to the
  competitive recipes (`raster-sweep`, `vello`, `stages`).
- Add a `deny.toml` that records the two unmaintained advisories
  (rustybuzz, ttf-parser) as accepted, with a reason, so that a *new*
  one fails. `cargo deny --offline check advisories` runs in `just ci`.
  `cargo-deny` is installed on the reference machine.

**Gate:** `check_locks.sh` prints `lock ok: . ; lock ok:
benches-competitive`. The deny step prints the advisory count against
the accepted list.

**Injection:**

- The lock-stale plant is the unmodified 1717808 tree, which is red
  today.
- Add a dependency edge in a competitive manifest without updating the
  lock. The check names `benches-competitive`.
- Remove one accepted advisory from `deny.toml`. The deny step names it.

**Accepts good input:** the regenerated lock passes.

**Docs in the same commit:**

- the justfile comments for the competitive recipes;
- `docs/results-competitive.md`'s reproduction line (`--locked`).

**Effort:** S. **Depends on:** T43.0.

### T43.13 — `just live` and `just ci`

**Closes:** R17-F8.

**Change:**

- Add `just live` (§2.2).
- `check` depends on `live` in place of justfile:97-99's display test.
- Add `just ci` and `just ci-verify` (§2.1), including the dirty-tree
  assertion.
- Add `just install-hooks` and `scripts/hooks/pre-commit`.

**Gate:**

- `just live` prints one line per script and a total.
- `just ci` writes its record, and prints a one-line summary for the
  closing commit: `ci <sha>: S steps, 0 failed, 0 skipped, tree clean`.
- `just ci-verify` checks the record for the given SHA.

**Injection:**

- `env -u DISPLAY -u WAYLAND_DISPLAY just live` exits non-zero with
  eight `skip(77)` lines. With LUMEN_ALLOW_SKIP=1 it exits 0 and says
  `8 skipped (allowed)`.
- `just ci-verify` on a fresh commit fails with `no CI record for <sha>`.
- Revert T43.12's lock inside the CI worktree. `just ci` must fail with
  `tree changed during the run: benches-competitive/Cargo.lock`, which
  is rule 5's mechanism.

**Accepts good input:** with `DISPLAY=:0`, all eight pass, and `just ci`
on HEAD writes a green record.

**Docs in the same commit:**

- the justfile header (the "manual" tier and `check`);
- AGENT.md: the "Tests must be shown to fail" end-to-end paragraph names
  `just ci`, and the work-queue preamble states the `ci-verify` rule
  for calling a milestone done.

**Also, assigned by the index (2026-09-25).** The consistency review
found two pieces every later plan assumes and none provides:

- **A bench lock recipe.** M44, M45, M46 and M55 cite "the bench lock".
  `just bench-lock <cmd>` runs `<cmd>` under `flock` on one lock file
  under `target/`, records `uptime` before and after, and marks the run
  **LOADED** when the one-minute load average exceeds a stated fraction
  of the core count. M38's reviewers ran this protocol by hand
  (`docs/plan-deep-review.md` §1).
- **A capability probe for qemu and the thumb target.** `just ci`
  checks for `qemu-system-arm` and `thumbv7em-none-eabihf` before
  running the recipes that need them. If either is missing, those
  recipes exit 77, which counts as a failure unless LUMEN_ALLOW_SKIP=1.
  Both are present on the reference machine.

**Effort:** L. **Depends on:** M42/T42.2 (the live scripts' 77 and free
ports), T43.4 (the firmware scripts inside the CI worktree), T43.10 and
T43.12.

### T43.14 — ci.yml as a dormant specification, and numbers that are cited

**Closes:** R17-F10, R17-F11, R17-F14.

**Change:**

- `rust-toolchain.toml` gains `targets = ["thumbv7em-none-eabihf"]`.
- Add `scripts/check_toolchain_targets.sh` to `just static`. Every
  `--target` triple in the justfile and scripts must be declared. The
  local runner shares the rustup home, so it would never notice a
  missing target on its own.
- ci.yml gets its dormant header (§2.7). Its false comments are
  replaced by citations, and its portability job stops claiming an
  exclusion it does not make.
- The justfile's dependency counts (:53, :429) become citations of
  `docs/results-gpu.md` and the a11y results.
- Add `scripts/check_ci_claims.sh` (§2.9) to `just static`, and add
  ci.yml to the identifier and doc-path scans.

**Gate:** the two new checks print `targets used 1, declared 1` and
`ci claims: 0 bare counts`.

**Injection:**

- The target-undeclared plant names the triple and the justfile line
  that uses it.
- The ci-claim plant restores "the dependency tree is six crates". The
  check names the line.
- Restoring "wgpu is 37 crates" makes the check name justfile:53.

**Accepts good input:** the rewritten files pass.

**Docs in the same commit:**

- ci.yml;
- the justfile's `thumb` comment (the manual `rustup target add` is no
  longer needed);
- `docs/results-gpu.md` states the counting command beside the count it
  owns.

**Effort:** S. **Depends on:** T43.2 (the scan extension), T43.13
(ci.yml's header points at `just ci`).

### T43.15 — wire the self-tests in, and bring AGENT.md's table up to date

**Closes:** nothing new. It finishes the milestone: T43.0's tiers join
the gates, and the vacuous-test record follows §2.8.

**Change:**

- `just selftest` joins `test`. `just selftest-build` joins `check`.
  `selftest-slow` and `selftest-live` join `ci`.
- Add the dated addendum to `docs/results-deep-review.md` §4:
  - I-NEW-1 and I-NEW-2 (M41);
  - I-NEW-3 (M42);
  - I-NEW-4 (here);
  - R7-F2's re-run at 1717808 (M47).

  Each row has what, why, how it was caught, and the injection.
- AGENT.md's table gains one summary row that cites §4, including the
  addendum. Its prose names the third shape (§2.8) and cites the table
  for the tally.
- Each row that this milestone closed gets its "shown to fail: yes"
  updated in §5 of the results doc, naming the plant.

**Gate:** `just selftest` and `just selftest-build` are green, with
every plant reported red under its plant. The commit body carries the
count of plants, and the count of baseline checks, from the run.

**Injection:** replace one check in the copy with `exit 0`. Its plants
must report `stayed green`, and `just test` must fail. This is T43.0's
harness injection, now inside the every-commit gate.

**Accepts good input:** the unmodified tree passes `just test`.

**Docs in the same commit:**

- AGENT.md, "Tests must be shown to fail" (the table and the paragraph
  on shapes);
- `docs/results-deep-review.md` §4 and §5.

**Effort:** S. **Depends on:** T43.1–T43.14; and M42/T42.2–T42.7,
because `selftest-live` runs M42's plants, and a plant whose fix has
not landed would hold `just ci` red.

### What building found (2026-09-26)

Recorded here rather than folded into the tasks above, which keep their
original wording.

- **§2.4's scratch copy is a copy of the working tree, not `git worktree
  add --detach` of HEAD.** A gate run before a commit must see the
  uncommitted change; a worktree at HEAD would test the previous commit.
  The copy is its own throwaway git repository under
  `target/selftest/wt`, reset between plants.
- **The harness found a defect in itself** (T43.5): syncing with `rsync
  -a` preserved the source's old mtimes, so cargo called a previous
  plant's build fresh and a baseline ran against it. The sync now writes
  only changed files, with the current time.
- **Plants for fixes that do not exist yet are written post-fix**, so
  at T43.0 four of them honestly reported `edit did not apply`
  (agent-alone, observe-tripwire, zero-tests, targets-undeclared).
- **`golden.rs` had I-NEW-4's shape too**, which §1.1 did not list: its
  `Recorded` branch re-ran the comparison and passed. T43.9 fixed both.
- **T43.6's "19 failed" needs `--no-fail-fast`**: a bare `cargo test -p`
  stops at the first red binary and reports 3.
- **T43.7's moved tests would not have failed in `release-shape` had they
  stayed**: `required-features = ["observe"]` would have skipped them.
  They moved anyway, so their feature needs are stated rather than
  borrowed from unification.
- **T43.8's test accepts the std answer in a unified build** (1,000
  stored under `--workspace`, where other members turn `std` on) and
  only `LUMEN_REQUIRE_NOSTD=1` in `thumb` insists on 256 — a no_std test
  cannot be a gate inside `cargo test --workspace`.
- **The shaping tests were shaping Noto, not DejaVu**, on the reference
  machine (`FontStack::system`'s first candidate). T43.11 pins DejaVu.
- **T43.14 ran before T43.13**, so ci.yml's header names `just ci` as not
  yet built; T43.13's commit updates it.
- **AGENT.md edits the plan assigns** (T43.1's doc-currency sentence,
  T43.2's check count, T43.4's M30 `.text` figure, T43.9's M12 entry)
  were left to the coordinator, who owns AGENT.md during parallel work.

### What T43.13 and T43.15 will need

- **From M42/T42.2:** every live script exiting 77 on a missing
  capability, and free agent ports, so `just live` can aggregate them.
- **Already here for them:** `just test-crates` and `just deny` (T43.13
  runs both); `scripts/selftest/run.sh` tiers `static`, `build`, `slow`
  (T43.15 wires them into `test`, `check` and `ci`; the static tier is
  22 plants in 19 s, inside §5's 30 s budget; build 14 plants in ~50 s
  warm; slow 2 plants in ~6 min); the requirement variables
  (`LUMEN_REQUIRE_GPU`, `LUMEN_REQUIRE_FONTS`, `LUMEN_REQUIRE_NOSTD`)
  and `LUMEN_ALLOW_SKIP`, which `just ci` must record as skip flags.
- **To build in T43.13:** `just live`, `just ci`/`ci-verify` (the
  dirty-tree assertion can reuse the harness's sync), `just
  install-hooks`, the bench-lock recipe (§3 T43.13) and the
  qemu/thumb probe; ci.yml's header should then point at `just ci`.
- **A `live` selftest tier** does not exist yet; M42/T42.0 adds its
  plants under `scripts/selftest/plants/live/`.

### What building T43.13 found (2026-09-26)

- **`just ci`'s steps are derived, not listed.** §2.1 says `ci` runs
  "`check`, `live`, the self-test tiers and the per-crate loop". A list
  of those in `scripts/ci.sh` would drift from the justfile the way
  ci.yml did. So the steps are the recipes in `check`'s dependency
  closure, read from the justfile *at the commit under test* (`just
  --dump --dump-format json`), each run with `just --no-deps` so the
  record names the stage. Only the ci-only steps are listed: `test-crates`,
  `deny`, `selftest-slow`, `selftest-live`, `adversary-plant` (M42/T42.7,
  which also needs a window and had no runner) and `miri-reload` (M49's
  pending wiring, docs/plan-reload-hardening.md T49.3).
- **The first `LUMEN_ALLOW_SKIP=1` run recorded a skip as a pass.** With
  no display, `live` printed `8 skipped (allowed)` and exited 0, and the
  record said 0 skipped — the green-record-hides-a-skip case §2.2 exists
  to prevent. Under LUMEN_ALLOW_SKIP=1 a step that exits 0 and prints an
  accepted-skip marker (`skipped (allowed)`, or `gpu`/`fonts`'s
  `LUMEN_ALLOW_SKIP=1 --`) is now recorded as a skip, and `ci-verify`
  refuses any skip unless it is given `--allow-skip` too.
- **An `--only` run writes `<sha>.only.json`**, which `ci-verify` never
  reads, so a partial run cannot replace or pass for a full record.
- **The bench lock is not under `target/`.** One file per worktree's
  `target/` serialises nothing across worktrees, and that is where the
  parallel work is. It lives at `<git common dir>/lumen-bench.lock`,
  which every worktree of the repository shares; LUMEN_BENCH_LOCK
  overrides it (this session's agents were briefed to use a path outside
  the repository, which the variable reaches). LOADED means the
  one-minute load average, before or after, exceeded 0.25 of the online
  cores (LUMEN_BENCH_LOAD_FRACTION).
- **No recipe uses qemu yet.** `grep` finds none; the capability exists
  on the reference machine and nothing runs it. `scripts/probe.sh qemu`
  is ready for M52's first qemu recipe to call on its first line, and
  `just ci` records every capability the probe finds in the record. The
  probe also covers `miri` (`miri-reload` starts with it). The thumb probe
  asks the toolchain the build will use (`rustc --print sysroot` under
  rust-toolchain.toml), not rustup's default.
- **`gpu_parity.sh` still exited 0 on a missing adapter or `xwd`.** §1.2
  listed it; M42/T42.2 moved only the display check to 77. Both now exit
  77, so `just live` refuses them unless LUMEN_ALLOW_SKIP=1.
- **The `stranger` live plant held a fixed port (7890)**, outside any
  range a caller gives. `scripts/selftest/live.sh` now gives the stranger
  the top port of LUMEN_LIVE_PORT_RANGE and the launcher the rest.
- **`just install-hooks` writes `core.hooksPath` to the config every
  worktree shares.** It was not run in this session, so as not to put a
  hook on other agents' worktrees; it was exercised in a scratch clone.
- **`just ci` registers `target/ci/wt` as a git worktree**, in the
  repository's shared worktree list. It is persistent by design
  (§2.1), and `git worktree prune` drops it once `target/` is removed.
- **The firmware gates moved into `test`** (§2.10).

### What T43.15 leaves to the coordinator (2026-09-26)

AGENT.md is the coordinator's during parallel work, so the edits T43.13
and T43.15 assign to it are proposed here, not made:

- **Work-queue preamble:** "A milestone is not done until `just
  ci-verify` passes for the commit that closed it, and its queue entry
  cites that SHA (`docs/plan-m38-fixes.md` §5)."
- **"Tests must be shown to fail", the end-to-end paragraph:** after
  "`just gpu-parity` and `just agent-drive` exist because of this", add
  "and `just ci` runs every one of them, with `just live`, against the
  commit in a clean worktree."
- **The table:** one summary row, `| M38's review and its re-triage |
  79 rows (74 plus a dated addendum of 5) | see docs/results-deep-review.md
  §4 |`, citing the tally there rather than restating it.
- **The prose:** a third shape beside the two, *the gate never runs* —
  compiled out by feature unification, skipped as a pass, or run
  against a process it did not launch — citing §4 for the evidence.

## 4. Boundaries of this plan

- **Not the live scripts' bodies or the launcher.** That is M42.
  T43.13 aggregates them, and M42/T42.2 gives them exit 77 and free
  ports.
- **Not the embedded gates' blind spots**, meaning a firmware that never
  dispatches, `f64` symbols, and the `flt2dec` ceiling (R11-F1, R11-F2,
  R11-F4, F-NEW-1). Those are M52. T43.4 gives M52 a firmware path that
  follows cargo, and T43.8 keeps its test out of the firmware so that
  F-NEW-1 does not arise here.
- **Not the winit `agent` feature forwarding `lumen-static-widgets/observe`**
  (R12-F3, M48). T43.6 fixes the agent crate's *test* manifest only. A
  downstream build is R12-F3's problem.
- **Not `cargo fmt`.** The justfile records why it is off (justfile:103-107).
  Nothing in the review reopened that.
- **Not a license policy in `deny.toml`.** The review noted there is
  none (§5 of the results doc). T43.12 adds advisories only, because
  that is the part with a current finding. Licences are a decision
  nobody has asked for.
- **Findings the triage labelled for other milestones and assign.json
  keeps here:**
  - R11-F6, which the triage tagged M39: it is a firmware-script defect,
    and `just ci` needs it fixed.
  - R17-F13 and R17-F14, which it tagged M52: they are number currency,
    and T43.4 and T43.14 are the scripts that own the comparison.
  - R17-F8, R17-F10 and R17-F11, which it tagged M42.

  They stay here, and none of them is moved.

No finding is won't-fix.

## 5. Kill rules and risks

There is no performance claim in this milestone, but three budgets are
written down before anything is built. A plan that makes gates slower
until nobody runs them has failed its own purpose.

- **`just selftest` inside `test`: a warm budget of 30 s.** If it is
  over, the static tier moves to `check` and `test` keeps only the
  baseline pass. *Revealed by:* the time the harness prints at the end
  of every run.
- **`just test-crates`: a warm budget of 10 minutes on the reference
  machine.** If it is over, the loop covers only the crates whose
  manifests carry a featured dev-dependency, which is the shape R13-F7
  had, and the rest are recorded as not looped. *Revealed by:* T43.6's
  first timed run, recorded in its commit.
- **`just ci`, warm: 60 minutes.** If it is over, it splits into `ci`
  (no live gates) and `ci-live`, and `ci-verify` requires both.
  *Revealed by:* the durations in the record.

Other risks:

- **`forbid(unsafe_code)` refuses code that exists.** A derive in the
  core's own test crates emitting `unsafe`, or `#[no_mangle]` somewhere
  new, would trip it. *Revealed by:* T43.1's build matrix. The answer is
  an exemption line in the presence check with a reason, not a quiet
  `allow`.
- **Tightened checks refuse the current tree**, as with the plan-deep-review
  quotation and the six stale allowlist entries. *Revealed by:* the
  harness's baseline pass, which runs before any plant. Each such case
  is fixed or allowed, with a reason, in the same commit that tightens
  the check.
- **The CI worktree drifts from a clean checkout.** An ignored directory
  that a recipe depends on, such as `benches-competitive/build/`,
  survives `git clean -e target`. *Revealed by:* running `just ci` once
  from a fresh `git worktree add`, before its first record is trusted.
  T43.13's commit records both runs.
- **The committed font changes shaping results that a golden depends
  on.** No golden uses system text today (T43.9). *Revealed by:* the
  pinned-face tests' printed glyph counts, compared against the
  system-face run in the same session.

## Appendix: finding → task

| ID | sev now | status | task |
|---|---|---|---|
| I-NEW-4 | S3 | OPEN | T43.9 |
| R11-F5 | S2 | OPEN | T43.5 |
| R11-F6 | S3 | OPEN | T43.4 |
| R12-F1 | S2 | OPEN | T43.7 |
| R12-F2 | S2 | OPEN | T43.8 |
| R12-F7 | S4 | OPEN | T43.2 |
| R13-F7 | S2 | OPEN | T43.6 |
| R17-F1 | S2 | WORSE | T43.9 |
| R17-F2 | S3 | OPEN | T43.1 |
| R17-F3 | S3 | OPEN | T43.2 |
| R17-F4 | S3 | OPEN | T43.3 |
| R17-F5 | S3 | OPEN | T43.3 |
| R17-F6 | S2 | OPEN | T43.10 |
| R17-F7 | S3 | OPEN | T43.11 |
| R17-F8 | S2 | WORSE | T43.13 |
| R17-F9 | S3 | OPEN | T43.4 |
| R17-F10 | S3 | OPEN | T43.14 |
| R17-F11 | S3 | WORSE | T43.14 |
| R17-F12 | S3 | OPEN | T43.12 |
| R17-F13 | S3 | OPEN | T43.4 |
| R17-F14 | S4 | OPEN | T43.14 |
| R17-F15 | S3 | WORSE | T43.2 |
| R17-F16 | S4 | OPEN | T43.2 |
| R3-F8 | S3 | OPEN | T43.6 |
| R4-F4 | S2 | OPEN | T43.1 |
| R6-F6 | S4 | OPEN | T43.12 |
| R9-F4 | S2 | WORSE | T43.9 |
