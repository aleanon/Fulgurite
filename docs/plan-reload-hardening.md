# M49 — a live swap keeps what the user did, and refuses what it cannot read

> **Written 2026-09-24, not started.** Closes R4-F2, R4-F3, R4-F5,
> R4-F6, R4-F7, R4-F8, R5-F1, R5-F2, R5-F3, R5-F4, R5-F7, R5-F8, R5-F9
> and R5-F10 from `docs/results-deep-review.md`, as re-triaged against
> `main` at `1717808` on 2026-09-24. The review was written before M37;
> the re-triage re-ran its probes after it, and where the two differ this
> plan follows the re-triage.
>
> **Phase 3** of the fixed order: after M41 and M42. It also needs M43's
> local CI runner (T43.13) for T49.3, M41's gate on `Host::adopt` and
> `restore` marking (R5-F6, which is M41's: T41.0's `reload_oracle.rs`,
> whose `restore` arm must be green, or fixed in M41 if it is red) before
> T49.5's live gate means anything, and M47/T47.3 before T49.10.
>
> **M37 moved every one of these findings from a headless fixture into
> a live window, and two of them got worse on the way.** R5-F2: stateful
> widgets still have no `Reloadable` impl, and the taught pattern
> (`#[tree] #[reload(fresh)]`) now discards what a user typed on every
> save. R4-F8: the reload crate's `unsafe` grew from 11 lines to 36, and
> the miri job still reaches none of it. **One finding recurs in its
> replacement**: T37.0's ABI stamp compares sizes, and a library built
> after two `Cmd` variants were reordered is "agreed" and then misread
> command by command (R4-F5). And **M31's defect is back in a new
> shape**: the window constructs through `program()`, `Host::reload`
> through `Application::build`, and nothing checks they agree (R5-F10).

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and prints the measured quantity beside its bound.*

---

## 0. The rules for this milestone

1. **A swap either keeps a value or says it did not.** Every place a
   value can be lost — a widget with no impl, a save that failed, a
   nested field, a task in flight — ends in the value kept or in a line
   in `Reloader::warning`. Silence is the defect in six of the fourteen
   findings.
2. **One construction path.** M31's rule, restated for M37's window: a
   swap constructs through the new build's `program()`, and so does
   everything else that rebuilds an application.
3. **What crosses the boundary is checked against its source, not its
   size.** A stamp that two differently laid-out builds can agree on is
   the defect the stamp exists to prevent.
4. **Every probe the re-triage ran becomes a committed test.** The
   probes were scratch files (`r5_widget_state.rs`, `r5_code_data.rs`,
   `r5_schema.rs`, the reordered-`Cmd` library, the boot-panic arm). Each
   becomes a gate in T49.0.

## 1. What is wrong, at 1717808

### 1.1 The stamp cannot see a layout change (R4-F5)

`abi::stamp` (`crates/lumen-static-reload/src/lib.rs:240-262`) lists the
compiler, target, panic strategy, debug assertions and features, the
shell's hash of `session.rs` (`abi_fields`,
`crates/lumen-static-shell/src/session.rs:554-580`), and the size and
alignment of six core types (`build_fields`,
`crates/lumen-static/src/reload.rs:179-205`). A reordering changes no
size. **The re-triage built the `reload-host` loader, moved `Cmd::Rect`
after `Cmd::Text` in `crates/lumen-static/src/display.rs`, rebuilt only
the library, and the loader printed "stamp … agreed", then read no Text
commands at all** and exited on its own assertion. It is reachable in
ordinary use: the hot builder compiles the library from the workspace's
current source, so any framework edit made while a hot window runs
produces exactly this.

Two halves of R4-F5 are **fixed** for the window path, and the
re-triage checked them: `Reloader::swap` (`lib.rs:679-706`) opens,
checks and boots the new build before `*current = next`, and
`Build::open` (`:460-475`) resolves every symbol eagerly. Its boot-panic
probe kept the session and a click still counted. **No committed gate
covers that**: "panicked while booting" appears once, at `lib.rs:702`.
The legacy loader, `fixtures/hot-host/src/main.rs`, still drops v1
(`:230`) before it loads v2 (`:253`), and looks each symbol up lazily
(from `:59`).

### 1.2 Unguarded shims, and a miri job that reaches none of them (R4-F6, R4-F8)

`export_reloadable_app!`'s `hot_restore` calls `from_raw_parts(ptr, len)`
unguarded (`crates/lumen-static-reload/src/lib.rs:105`), so `(null, 0)`
is undefined behaviour, while `SAVE` and the stamp accept it. The
window's replacement, `export_program!`'s `lumen_boot`, checks for null
(`:839-843`), so the unguarded shim now serves only `fixtures/hot-app`
and no caller passes null: S4. The same shape remains in `hot_drop` and
`lumen_drop` (`Box::from_raw` with no null check, `:64`, `:856-857`) and
`hot_save`'s dereference (`:79`).

**WORSE (R4-F8):** `unsafe` lines in `crates/lumen-static-reload/src/lib.rs`
went from 11 at the review's commit to 36 at `1717808` — `panic::take`
and `abi::read` (two-call buffer protocols), `export_abi_stamp!`,
`export_program!`'s boot and drop, and `hot::Build`/`Hot`'s round trip of
a `Box<dyn Session>`. The CI miri job (`.github/workflows/ci.yml:186-195`)
runs only `fixtures/miri-smoke`, which depends on `lumen-static` and
`lumen-static-widgets` with no features. The in-process pieces are
checkable by miri; `dlopen` is not.

### 1.3 Two construction paths again (R5-F10, R5-F9)

The style half of R5-F10 was fixed by `8625828`: `Host::adopt` re-asks
the program's sheet. **The build half recurs.** `Application::build` is
still required (`crates/lumen-static/src/event.rs:641`). The
`Reloadable` derive's `fresh()` calls it for every struct with a
`#[tree]` field (`crates/lumen-static-macros/src/lib.rs:1271-1275`), and
M37's `hot` feature requires `A: Reloadable`
(`crates/lumen-static-winit/src/lib.rs:825`). So every hot application
implements `Application::build`, which the window swap never calls:
`hot::boot` (`crates/lumen-static-reload/src/lib.rs:399-415`) boots the
library's `program()`. `Host::reload` (`crates/lumen-static-shell/src/lib.rs:488-495`)
calls `A::fresh()`. **The re-triage replaced `impl Application for
Counter` in `fixtures/hot-window` with a free `boot` passed to
`application`, and `cargo check --features hot` failed with E0277 from
the derive.** So M32's front door cannot be hot-reloaded, and
`docs/plan-program.md`'s "the one place two constructors remain … until
M37 replaces that path" is still true after M37.

R5-F9 is the prose that follows from it: `event.rs:625-626` says any
application deriving `Default` beside `App` is a compile error (the
guard is emitted only by the `Reloadable` derive, for `#[tree]`
structs), and `docs/step4-hot-reload.md`'s honesty list (`:112-125`) is
stale on all three items — there is a stamp now, reload is cfg-gated,
and `fresh()` is `Application::build`, not `Default`.

### 1.4 What a swap loses (R5-F1, R5-F2, R5-F3, R5-F4)

**WORSE (R5-F2).** The only `Reloadable` impls are `Label`, `Button`,
`Toggle`, `Fixed`, `Spacer` (`crates/lumen-static-widgets/src/reload.rs:20-110`)
and `Repeat`, `Vec`, `Option` (`crates/lumen-static/src/reload.rs:79-150`).
The derive refuses enums (`crates/lumen-static-macros/src/lib.rs:1200-1206`),
so a `StaticTree` enum cannot be a reload field at all. The re-triage
re-ran the review's probe unchanged: **7 of 9 user-state values lost**
(text and caret, slider value, scroll, popup and collapsible open, tab
selection) with **0 log lines**. Before M37 this was a headless fixture;
now every live window loses it on every save. The taught pattern,
`#[tree] #[reload(fresh)]` (`fixtures/hot-window/src/lib.rs:32-33`),
discards even the state the existing impls could keep. `TextInput` has
no controlled mode, so an application cannot move typed text into
`#[state]` to keep it.

**R5-F1, downgraded to S3.** In the other direction, a kept subtree
keeps what the *code* wrote: the re-triage's probe restored v1's
`Button` label, v1's static `Label` text, and v1's `Repeat` count and
item extent over v2's build, with 0 log lines. The taught pattern avoids
this by discarding the tree; an unannotated `#[tree]` (the default mode
is keep, `lib.rs:1177`) or a `Repeat` inside a kept subtree still hits
it. **These two findings are one question — which half of a widget is
state — answered in §2.3.**

**R5-F3.** A kept `Option` field whose save fails, or whose value is
`Some(NaN)`, restores `None` over the fresh value: the derive inserts
`Value::Null` on a save error (`lib.rs:1218-1226`), and serde maps NaN
to null. The re-triage's rows logged 0 lines each. T37.3's warning bar is
fed from that log, so a live window shows nothing either.

**R5-F4.** A kept struct that gains a field restores as a whole or not
at all: the re-triage's `Settings {a, b} → {a, b, c}` row came back as
all defaults with one log line. The taught pattern keeps all application
state in one `#[state] #[reload(keep)]` struct
(`fixtures/hot-window/src/lib.rs:35-38`), so adding one field to it
resets every kept value.

### 1.5 M28 work does not cross a window swap, and the docs say it does (R4-F3)

`docs/plan-window-reload.md` (`:292-297`) records that the window's
`Session` has no way to hand pending work over, so a chain started
before a reload does not complete after it — and in the same paragraph
says `fixtures/hot-host` "proves descriptors can cross as data".
`fixtures/hot-app/src/lib.rs:383-387`, `docs/plan-tasks.md:362-363` and
`docs/results-tasks.md:555-558` still credit T28.7 with a framework
property. T28.7's inbox, epoch and descriptor all live in the fixture's
loader; no framework path carries them, and nothing tells the user an
in-flight chain was dropped.

### 1.6 Identity across a shape change (R4-F7)

A node's identity is its offset in the table. `book/src/agent.md:94-97`,
`docs/00-handoff-static-tree.md:660-664` and `book/src/component-boundary.md`
say the identity survives a swap. It survives only an edit that keeps
the tree's shape. M37 made this reachable: an agent holding a node index
across a live swap addresses a different node after any field is added
or reordered.

### 1.7 Developer experience (R5-F7, R5-F8)

The derive's errors land on the derive, not the field, and never name
`#[reload(keep|fresh)]`: `expand_reloadable` uses plain `quote!`, and
`Reloadable` (`crates/lumen-static/src/reload.rs:42`) has no
`diagnostic::on_unimplemented`. M37 put these errors in front of every
application that turns on hot reload (R5-F7). And with `reload` off, an
application that derives `Reloadable` does not build, because
`lumen_static::reload` is `cfg(feature = "reload")`
(`crates/lumen-static/src/lib.rs:141-142`) and the derive names it
unconditionally. `fixtures/hot-window/Cargo.toml` sidesteps this by
enabling `reload` unconditionally, so the only shipped hot crate links
serde_json into its release binary (R5-F8).

### 1.8 Latency measured on a small crate (R4-F2)

`docs/step4-hot-reload.md:108-110` still cites a synthetic crate's
rebuild, and `book/src/component-boundary.md` still says "sub-second".
T37.4 added a real windowed measurement, on `fixtures/hot-window`, a
64-line crate; the re-triage re-ran it at 0.64, 0.44 and 0.45 s. No real
application exports a program. The library's profile follows the
window's (`Crate::at(…, !cfg!(debug_assertions), …)`,
`crates/lumen-static-winit/src/lib.rs:964`), and the stamp requires the
two to agree on debug assertions, so a release window rebuilds its
library in release. The review measured that on a showcase-sized crate
and missed 2 s (R4-F2; `docs/results-deep-review.md`); the re-triage did
not re-measure it.

## 2. Decisions

### 2.1 The stamp hashes the source of the crates whose types cross

- **(a) Size and alignment**, today. Blind to any reordering (§1.1).
- **(b) A layout hash per type**, generated by a derive: the type's name,
  each field's offset, each variant's discriminant. On the pinned
  toolchain (1.94.0), `offset_of!` into an enum variant is still
  unstable — checked while writing this plan, E0658 — and `Cmd` and
  `Event` are enums. Probing a variant's tag at run time means reading
  the bytes of a constructed value, padding included, which is not
  sound without care. And the derive would be needed on every crossing
  type and, transitively, every type in their fields. It is the most
  work and still incomplete.
- **(c) A source hash.** `lumen-static-reload`'s build script, which
  already exists for the compiler version, also hashes the `src/` trees
  of `lumen-static`, `lumen-static-shell` and `lumen-static-reload`, the
  workspace's `Cargo.lock`, and the encoded rustflags, and the stamp
  carries one line per crate. A reordered variant, a changed field type
  or a dependency bump then changes the stamp. The cost: at `1717808`
  that is about 0.84 MB of source (554 KB in 31 files, 253 KB in 8, 36 KB
  in 1; counted for this plan), hashed only when one of those crates
  changes, which is when they recompile anyway. No run-time cost beyond
  a longer stamp string. It subsumes today's `session.rs` hash.

**Recommended: (c), keeping (a)'s lines as a second, cheaper check.**
What it breaks: **any** edit to those three crates during a hot session
— a comment included — refuses every later build until the window
restarts. That is the right answer, because the loader is still running
the old framework, and the refusal says so: "the framework was edited
since this window started; restart it". `lumen-static-widgets` is **not**
hashed, because no widget type crosses the boundary; editing a widget
while a hot window runs keeps working. That exclusion is itself a claim,
and T49.1 tests it (§5).

Rejected as well: hashing only the files that define the crossing types.
It would let more edits through, but nothing can list the files that
define their *fields'* types, so the list would drift silently.

### 2.2 One construction path: a reload boots the program

- **(a) Keep `Application::build`**, and assert at boot that the
  program's boot function is `Application::build`. Not expressible for
  a closure, which `application` accepts.
- **(b) Take `fresh()` out of what an application needs.** It moves to a
  second trait, which the derive emits only for structs with no
  `#[tree]` field (widgets, which need it: `Repeat` and `Vec` build rows
  with it). The host keeps the program's boot, as it already keeps the
  sheet source, and `Host::reload` rebuilds through it. In-process, the
  boot is the current code, which is what `fresh()` was for.
  `export_reloadable_app!` and `fixtures/hot-app` take a boot function.
  The E0119 guard against `Default` stays keyed on `#[tree]`.

**Recommended: (b).** Then nothing requires `Application::build`, and it
is retired as `docs/plan-program.md` §2.3 intended, once a mechanical
`grep` shows no caller is left. What it breaks: a hand-written
`Reloadable` impl for an application stops compiling until it drops
`fresh()`, and `Host::reload` on a host that was not built from a
program has nothing to rebuild from — it reports that rather than
guessing.

### 2.3 A widget saves its user's half, and only that

The question under R5-F1 and R5-F2 is which half of a widget is state.

- **(a) Discard the tree** (the taught pattern). Loses everything the
  user did.
- **(b) Keep the tree** (the default). Keeps what the code wrote, so an
  edit to a label or a row height never appears.
- **(c) Each widget saves what a user can change, and nothing the code
  set.** Text and caret for `TextInput`, `TextArea` and `SearchField`;
  value for `Slider`, `RangeSlider` and `NumberInput`; scroll for
  `ScrollView`, `Repeat` and `DataGrid`; open for `Popup`, `Veil` and
  `Collapsible`; selection for `Strip`, `Chooser` and `DataGrid`; the
  expanded set for `Tree`; the ratio for `SplitPane`. `Button` and a
  static `Label` save nothing: their text is code, and a text that
  changes at run time belongs in `#[bind]`. `Repeat` keeps its scroll;
  its count and item extent come from the build, and the next update's
  binding sets the count from data. The enum derive emits `Reloadable`:
  the active variant's index and its nested save, falling back with a
  log line when the variant set changed.

**Recommended: (c), with the `#[tree]` default staying keep.** The
taught `#[reload(fresh)]` on the tree becomes unnecessary and comes out
of the fixtures and the book. What it breaks: an **unsplit** application
whose handler writes a label's text directly gets the build's text back
after a swap. A split application (M33) cannot write the tree from a
handler, so it loses nothing. The book says so.

### 2.4 Work in flight is re-issued to the new build

- **(a) Drop it and say so.** Count what was in flight and put "N tasks
  in flight were dropped by the swap" in the warning; correct the T28.7
  docs.
- **(b) Re-issue it.** The host keeps a ledger of descriptors spawned
  and not yet applied: a descriptor goes in when it is spawned and comes
  out when its result is applied. `Session::save` carries the ledger
  under a reserved key, and `hot::boot` re-spawns each entry under the
  new build, so the new build's step function answers it. Messages
  sitting in the old inbox are dropped; their descriptors are still in
  the ledger and are run again.

**Recommended: (b).** M28's design, a continuation as a step index
rather than a closure, exists for exactly this, and T28.7 was credited
with it. What it breaks: delivery becomes **at least once** across a
swap. A step whose job has side effects may run twice, once on the old
build's worker and once on the new. That goes in the book, next to the
chains. The host has no such ledger today — the host's `running` list holds
subscriptions only — so (b) is the larger change, and §5 has its kill
rule.

## 3. Tasks

### T49.0 — the gates first

Each check is committed **red** at `1717808`, with the red output in the
commit body. The re-triage's scratch probes become committed tests.

| finding | check | red at 1717808 |
|---|---|---|
| R4-F5 (stamp) | `fixtures/stamp-host` (`just abi-stamp`) gains a case: a library built after a framework source file changed must be refused, naming the field and printing both values. The case builds the reordered-`Cmd` library the re-triage built | "agreed", then no Text commands |
| R4-F5 (boot) | `fixtures/reload-host` (`just window-reload-demo`): `fixtures/reload-app`'s boot panics under a switch; assert the swap did not happen, a click still counts, and the error contains "panicked while booting" and `lib.rs:`, printed beside the assertion | green (the window path is fixed); committed as the missing gate, with its injection recorded |
| R4-F6 | a `#[cfg(test)]` module in `lumen-static-reload` that exports a tiny `Reloadable` in-process and calls restore with `(null, 0)` and drop with null, in a debug build | aborts on the precondition check |
| R4-F8 | `cargo +nightly miri test -p lumen-static-reload --lib` over that module; prints the number of shim calls exercised against the expected count | not run by anything; the job is added to M43's runner, where a red run fails the record like any other step (M43 §2.1), and to the dormant ci.yml, non-blocking as today |
| R5-F10 | a trybuild pass case: a `#[tree]` application booted by `application(boot)` with **no** `Application` impl derives `Reloadable` and compiles with `export_program!` | E0277 from the derive |
| R5-F2 | `crates/lumen-static-shell/tests/reload_widget_state.rs` (the re-triage's probe, promoted): asserts, per widget, "kept, or a log line"; prints the before/after table | 7 of 9 lost, 0 lines |
| R5-F1 | `crates/lumen-static-shell/tests/reload_code_data.rs`: v1 and v2 differ only in `build()`; after restore, `Button`'s label, a static `Label` and `Repeat`'s item extent equal v2's, and a `#[state]`-bound value keeps v1's. Prints "v2 says X, after reload Y" per row | four rows keep v1 |
| R5-F3, R5-F4 | `crates/lumen-static-shell/tests/reload_schema.rs`: every row must end at the fresh value or log; `Settings {a,b} → {a,b,c}` must keep `a` and `b`. Prints change, before, after and log count | the `Option` rows and the nested row |
| R5-F7 | a trybuild fail case with an untagged `u32` field and an untagged `TextInput` field; its `.stderr` must put the span on the field line and show the note | span on the derive line, no note |
| R5-F8 | a `just` recipe builds `fixtures/hot-window` with and without `hot` and prints the count of serde_json symbols in each (`nm -C`); without `hot` it must be 0 and the build must succeed | does not build without `reload` |
| R4-F3 | `fixtures/reload-host`: v1 starts a two-step chain whose first step is in flight at the swap; after the swap and a drain, the frame must show v2's step result. Prints the ledger's length before and after | the chain never completes |
| R4-F7 | `fixtures/reload-host` gains a v4 that adds a field before `#value`: the reload answer must report a shape change, and `#value` resolved by id must read the right label. Prints old and new index | no shape report exists |
| R4-F2 | `scripts/hot_window_drive.py` runs against a real-app fixture (T49.12) and prints `edit->swap X s / 2.0 s` per reload with the profile and the library's size; fails if the median exceeds 2.0 s | not measured on a real application |

R5-F9 is prose and has no check (T49.13).

> **What building T49.0 found (2026-09-26, at `5f6b4f2`).** Each finding
> was re-checked at HEAD before its gate was written; none had moved.
>
> - **R4-F5, stamp.** `just abi-stamp` rsyncs `crates/` into a workspace
>   of its own under `target/abi-stamp/ws` and builds `stamp-lib` there.
>   An unchanged copy is accepted (the stamp must describe source, not
>   where it lives); `Cmd::Rect` moved after `Cmd::Text` is **accepted**
>   at HEAD, "all 12 fields agree" — known red, printed, until T49.1; and
>   an edit to `lumen-static-widgets` alone is accepted. `stamp-lib` now
>   names a widget type, so that crate is in its graph at all.
> - **R4-F5, boot.** Green, as the re-triage found. Two injections:
>   dropping the library's report from `Reloader::swap`'s error fires
>   "the error must say the boot panicked and where"; removing
>   `Build::boot`'s null check makes the loader take a null session, and
>   the gate dies with SIGSEGV (139).
> - **R4-F8: miri cannot build this crate here.** Every crate with a
>   build script fails under `cargo +nightly miri` (nightly 2026-02-11)
>   with "contains outdated or invalid JSON" — reproduced on a fresh
>   crate whose only dependency is serde_json, so it is not this
>   workspace. A dependency-free crate runs under miri and reports UB.
>   T49.3 therefore takes §5's fallback, and the committed R4-F6 module
>   runs in debug `cargo test`, where the standard library's precondition
>   checks abort on all three null calls.
> - **R5-F2's table is 23 rows**, not the re-triage's 9: every widget
>   with user state, including four §2.3(c) did not list (`PaneGrid`'s
>   dividers, `Pagination`'s page, `Picker`'s value, `RichTextEditor`'s
>   text). All 23 are lost with 0 lines under the taught
>   `#[reload(fresh)]`. `DataGrid` has **no selection** to save — §2.3(c)
>   lists one — so its row is its scroll.
> - **R5-F3 and R5-F4** are two tests, so T49.6 can go green before
>   T49.7.
> - **R5-F7's case has no `TextInput` field**: T49.5 gives `TextInput` an
>   impl, after which an untagged one compiles. It uses an application's
>   own widget with no impl instead, beside the `u32`.
> - **R5-F8: the planned count could not see the finding.** `nm` found 0
>   serde_json symbols in the release binary *without* `hot`, while
>   `reload` was on unconditionally: the linker drops the derive's unused
>   `save`/`restore`. `just reload-gating` counts serde_json in the
>   dependency graph instead (2 without `hot` at HEAD, bound 0).
> - **R4-F7 has no gate yet**: "the reload answer reports a shape change"
>   has nothing to be written against at HEAD — there is no answer to
>   read. T49.11 adds the fixture and the gate together.
> - **Waiting, not faked:** R4-F2's latency row needs M42's drive
>   identity and T49.12; T49.5's live step needs M42. R4-F3's chain case
>   is not written, because T49.10 is not built (see its entry).

**Other direction, recorded in the same commit:** an unchanged rebuild is
accepted by the stamp case; the schema test's `u32 → u64` row keeps its
value with 0 lines; an application that derives `Default` still fails
with exactly `fail_reloadable_app_derives_default.stderr` (E0119, not
E0277 — the review found the M31 gate passing for the wrong reason).

**Effort:** M. **Depends on:** M42, for the drive identity the live
checks print.

### T49.1 — the stamp hashes the source

**Closes:** R4-F5.

**Change:** §2.1(c) in `crates/lumen-static-reload/build.rs` and
`abi::stamp`; `build_fields` and `abi_fields` keep their size lines, and
the `session.rs` hash is replaced by the crate hashes. The `Mismatch`
message names the edited crate and says to restart. `fixtures/hot-host`'s
loader boots v2 and resolves every symbol before it drops v1, since it
stays as `just hot-demo`.

**Gate:** T49.0's stamp case and boot case.

**Injection:** remove the source-hash lines from the stamp: the
reordered-`Cmd` library is agreed again and the stamp case fires "a
library built from different source was accepted". In `Reloader::swap`,
assign `*current = next` before checking the boot's result: the boot
case fires "a panicking boot lost the session".

**Accepts good input:** an unchanged rebuild is accepted, and
`just window-reload-demo`'s v1 → v2 swap stays green. **An edit to
`lumen-static-widgets` alone is accepted**, which is §2.1's exclusion
checked rather than assumed.

**Docs in the same commit:** `docs/step4-hot-reload.md` (the stamp
section, and what it does and does not cover); `docs/plan-window-reload.md`
§2.3; the rustdoc on `lumen_static_reload::abi`; the `fixtures/hot-host`
header.

**Effort:** M. **Depends on:** T49.0.

### T49.2 — the shims accept `(null, 0)`

**Closes:** R4-F6.

**Change:** in `export_reloadable_app!`, restore reads an empty slice
for a null pointer; drop returns early on null, there and in
`export_program!`; `hot_save` checks its pointer. The same guard goes
into `fixtures/hot-app`'s hand-written title setter.

**Gate:** T49.0's in-process module, in debug.

**Injection:** remove the null guard: the debug build aborts on
`slice::from_raw_parts`'s precondition.

**Accepts good input:** a real `(ptr, len)` restore returns 0.

**Docs in the same commit:** the Safety sections on the restore and drop
shims; the `symbols` constants' comments in `crates/lumen-static/src/reload.rs`
("`(null, 0)` is accepted").

**Effort:** S. **Depends on:** T49.0.

### T49.3 — miri reaches the reload crate

**Closes:** R4-F8.

**Change:** the in-process module from T49.0 drives every shim that can
run without `dlopen`: new, save to size, save, restore, restore with
null, drop, `panic::take` twice, and `abi::read` against the in-process
stamp export; `export_program!`'s boot and drop against a trivial
program where the `window` feature builds under miri. `just miri-reload`
runs it, and M43's runner and the CI miri job run it beside
`miri-smoke`. If a proc-macro or build-script dependency blocks miri,
the fallback is the review's dependency-free transcription of the shims,
with a check that it matches the macro's expansion.

**Gate:** `just miri-reload`, printing shims exercised against the
expected count.

**Injection:** remove T49.2's null guard, or make `panic::take` copy `n`
bytes when the capacity is smaller: miri reports undefined behaviour and
the job goes red.

**Accepts good input:** the correct shims pass clean.

**Docs in the same commit:** the comment above the miri job in
`.github/workflows/ci.yml`; `docs/verification-notes.md`, miri.

**Effort:** M. **Depends on:** T49.2; M43/T43.13 (the runner).

> **Built 2026-09-26 on the fallback, and better than the fallback
> planned.** miri cannot build this crate (T49.0's note), so no
> transcription was written: T49.2 moved every pointer operation into
> `src/raw.rs` (core/alloc only) and T49.3 moved the panic record into
> `src/panic.rs` (std only), and `fixtures/miri-reload` includes **the
> same two files** by path. There is nothing to keep in step with the
> macro, because the macro's shims call these functions. What miri cannot
> reach is the `libloading` calls themselves. **Pending: the M43 runner.**
> `just miri-reload` exists and the dormant CI job runs it; putting it in
> `just ci` waits for T43.13.

### T49.4 — one construction path

**Closes:** R5-F10.

**Change:** §2.2(b). `fresh()` moves out of `Reloadable` into a second
trait the derive emits for non-`#[tree]` structs; `Repeat`, `Vec` and
`Option` use it. The host keeps the program's boot beside its sheet
source, and `Host::reload` rebuilds through it. `export_reloadable_app!`
and `fixtures/hot-app` take a boot function. `Application::build` is
retired if nothing calls it.

**Gate:** T49.0's trybuild pass case. The existing E0119 fail case must
still match its `.stderr`. And in `fixtures/reload-host`, an in-process
`Host::reload` and a window swap of the same build produce byte-identical
frames, printing both command counts.

**Injection:** put the `Application` bound back in the derive: the pass
case fails with E0277. Make `Host::reload` construct through `Default`:
the frame comparison differs, because handlers become placeholders.

**Accepts good input:** the E0119 case still fails with exactly its
`.stderr`.

**Docs in the same commit:** `docs/plan-program.md` §2.3 (close "the one
place two constructors remain"); the `Application` rustdoc in
`crates/lumen-static/src/event.rs`; the M31 comment block in the derive;
`book/src/first-tree.md`; `examples/counter-window/src/app.rs`.

**Effort:** M. **Depends on:** T49.0.

> **What building found (2026-09-26).** The "second trait the derive
> emits for non-`#[tree]` structs" is `Default`: every row a container
> builds already derives it, and a row's `fresh()` was only ever
> `Default::default()`. So there is no new trait — `Vec`, `Option` and
> `Repeat` take `W: Default`. `Host::reload` now returns whether it had a
> boot to rebuild from. The plan's second injection ("make `Host::reload`
> construct through `Default`") cannot be written: nothing gives `A` a
> `Default` bound, and a reloadable application cannot implement it
> (E0119). The injection used instead drops the boot from
> `Program::host`, and the gate fires by name. `Application` is retired.

### T49.5 — a widget saves its user's half

**Closes:** R5-F1, R5-F2.

**Change:** §2.3(c). `Reloadable` impls in
`crates/lumen-static-widgets/src/reload.rs` for every widget with
user-mutable state; `Button` and `Label` save nothing; `Repeat` saves
its scroll only. The derive accepts enums. `fixtures/hot-window`,
`fixtures/reload-app` and the book drop `#[reload(fresh)]` from the tree.

**Gate:** T49.0's `reload_widget_state.rs` and `reload_code_data.rs`.
A live step in `scripts/hot_window_drive.py`: type into a `TextInput`,
edit the source, and assert the text survives the swap.

**Injection:** make any one widget's restore do nothing (the review's
`Toggle` control): that row reports LOST, by name. Make `Button` save
its label again: `Button.label kept v1 "Save"`.

**Accepts good input:** every row kept, and the `#[state]` count still
survives.

**Docs in the same commit:** `book/src/hot-reload.md` (what survives a
swap, per widget; the unsplit-label consequence); `docs/step4-hot-reload.md`,
keep and fresh; the `Reloadable` impl rustdocs; the derive's rustdoc on
enums; `docs/plan-window-reload.md` §2.4.

**Effort:** L. **Depends on:** T49.4 (the trait split); M41/T41.0's
R5-F6 gate (a restore must mark, or the live step reads last frame's
text under `Boundaries`; M41 fixes `adopt` there if its arm is red).

> **Built 2026-09-26.** 24 rows, all kept: the 23 of T49.0's table plus a
> widget enum, which the derive now accepts (saved **by variant name**,
> not index, so reordering variants in an edit keeps the user's place).
> `Fixed` saves nothing as well: its size is code. Containers save their
> children under their own keys. `Repeat`'s count and row height come
> from the build, so F1.4's hostile-extent tests became "a saved extent
> or count is not read at all". Two fixtures kept what had become code:
> `fixtures/hot-app`'s user-renamed title is now a `TextInput`, and its
> row count kept data the update binds to the list;
> `tests/reload_oracle.rs`'s restore arm (M41's) restored a `Label` to
> change pixels, and now restores a `TextInput` — its injection (no
> mark in `reasked`) still fires, 14,688 px. **The live step waits for
> M42** (drive identity); `just hot-window-drive` was run as a
> regression check and passed.

### T49.6 — a failed save keeps the fresh value

**Closes:** R5-F3.

**Change:** in the derive's keep arm, a save error logs and omits the
key, so restore logs "missing from saved state" and keeps the fresh
value. A field whose saved value is null while its fresh value is not is
logged as "serialized as null" and omitted too; that covers NaN without
a per-type walk.

**Gate:** T49.0's schema rows for `Option<f32>` and the
`Option<HashMap>`.

**Injection:** insert `Value::Null` again: the `Option<HashMap>` row
comes back `None` with 0 lines.

**Accepts good input:** `u32 → u64` keeps its value with 0 lines; the
other rows log exactly one line each.

**Docs in the same commit:** `book/src/hot-reload.md` (per-field
fallback, with the NaN note); `docs/step4-hot-reload.md`; the derive
rustdoc.

**Effort:** S. **Depends on:** T49.0.

> **Built 2026-09-26, and the plan's injection alone was covered twice.**
> Both rules live in the core (`reload::save_keep`, `reload::restore_keep`),
> and the derive calls them. Putting `Value::Null` back on a failed save
> does turn the `Option<HashMap>` row red (`None`, 0 lines) — but only
> because that row's *fresh* value cannot serialize either, so the null
> check cannot see it is non-null. For a type that can, the null check
> alone would have caught the injection. Removing the null check turns
> the NaN row red.

### T49.7 — a kept struct restores field by field

**Closes:** R5-F4.

**Change:** the keep arm restores through a merge: the fresh value is
serialized, the saved object's keys are laid over it recursively
(objects only), the result is deserialized, and each key missing or
dropped is logged. The alternative — document "per top-level field" and
recommend `serde(default)` — leaves the trap in the taught pattern, so
it is not taken.

**Gate:** T49.0's `Settings` row: `a` 7, `b` "kept?", `c` false, one
line naming `c`. A live row in `just window-reload-demo`: a
`fixtures/reload-app` version that adds a field to its kept struct keeps
the count, printed.

**Injection:** restore with a plain deserialize of the saved value: the
row returns `a` 0.

**Accepts good input:** an unchanged struct restores with 0 lines.

**Docs in the same commit:** `docs/step4-hot-reload.md`;
`book/src/hot-reload.md`; `docs/plan-window-reload.md` §2.4.

**Effort:** S. **Depends on:** T49.6 (the same arm).

### T49.8 — derive errors point at the field and name the fix

**Closes:** R5-F7.

**Change:** `expand_reloadable` emits each field's save and restore with
`quote_spanned!` on the field's type. `Reloadable` gains
`diagnostic::on_unimplemented` with a note naming `#[reload(keep)]` for
plain data and `#[reload(fresh)]` for code. Stateless widgets such as
`Divider` get impls the way `Spacer` has one.

**Gate:** T49.0's trybuild fail case.

**Injection:** plain `quote!` again: the span moves to the derive and
the `.stderr` differs. Remove the attribute: the note disappears.

**Accepts good input:** `pass_reloadable_app_without_default.rs` still
passes.

**Docs in the same commit:** `book/src/hot-reload.md` (quote the note);
the derive rustdoc.

**Effort:** S. **Depends on:** T49.4 (the `Application` requirement is
gone, so its error needs no message).

### T49.9 — reload off compiles the derive to nothing

**Closes:** R5-F8.

**Change:** the derive and its helper attribute stay registered, and
expand through a `macro_rules!` shim the core defines: the real impl
with `reload` on, nothing with it off. `export_program!` is gated on the
`hot` feature inside the macro. `fixtures/hot-window/Cargo.toml` and the
book forward `hot` to the `reload` features instead of enabling them
unconditionally.

**Gate:** T49.0's `just` recipe: both builds succeed; serde_json symbols
0 without `hot` and more than 0 with it, both printed.

**Injection:** make the shim expand the real impl unconditionally: the
build without `hot` fails, or its symbol count is above 0.

**Accepts good input:** the build with `hot` still passes
`just window-reload-demo`.

**Docs in the same commit:** `docs/step4-hot-reload.md` (the honesty
list's gating item); `book/src/hot-reload.md` (features); the comment in
`fixtures/hot-window/Cargo.toml`.

**Effort:** M. **Depends on:** T49.0.

### T49.10 — work in flight is re-issued to the new build

**Closes:** R4-F3.

**Change:** §2.4(b). The host keeps the ledger of spawned, unapplied
descriptors; `Session::save` carries it under a reserved key;
`hot::boot` re-spawns it under the new build. The warning names how
many were re-issued. The ledger is also the in-flight count M48/T48.2
added for `ui.waitSettled`: its pending-task count becomes the ledger's length,
and T48.2's separate counter is removed in this commit, so the two
cannot disagree. If §5's kill rule sends this task to §2.4(a), T48.2's
counter stays.

**Gate:** T49.0's chain case in `fixtures/reload-host`.

**Injection:** drop the ledger in `hot::boot` (today's behaviour): "chain
did not complete after swap". Hold the continuation as the old build's
function pointer inside the descriptor, which is T28.7's own injection:
it answers with v1's value.

**Accepts good input:** the same chain with no swap completes
identically, and the ledger is empty after it.

**Docs in the same commit:** `docs/plan-tasks.md:362-363` and
`docs/results-tasks.md` T28.7 (what the fixture showed and what the
framework now does); `fixtures/hot-app/src/lib.rs`'s comment;
`docs/plan-window-reload.md:292-297`; `docs/events-decision.md` rule 3's
citation; `AGENT.md`'s M28 entry; the book's chain chapter (at least
once across a swap).

**Effort:** L. **Depends on:** T49.0; M47/T47.3 (the spawner the ledger
hooks into is rewritten there).

### T49.11 — a swap says when the shape changed

**Closes:** R4-F7.

**Change:** the session reports a shape hash (node count and the
table's kinds). The window's `app.reloads` answer carries whether the
last swap changed it, so an agent knows to resolve ids again. The docs
say identity survives edits that keep the shape, and that across a
shape change the stable name is the author id or a `#a > #b` path.

**Gate:** T49.0's v4 case.

**Injection:** make the shape hash constant: no change is reported
across v3 → v4, and the case fires.

**Accepts good input:** v1 → v2, a format edit only, reports no change.

**Docs in the same commit:** `book/src/agent.md`;
`book/src/component-boundary.md`; `docs/00-handoff-static-tree.md`
(`:660-664`); the agent method docs for `app.reloads`.

**Effort:** S. **Depends on:** T49.0.

### T49.12 — reload latency on a real application

**Closes:** R4-F2.

**Change:** `examples/showcase` gains a library target that exports a
program through a zero-argument wrapper around its `program(entry,
open)`. `scripts/hot_window_drive.py` runs its latency steps against it
as well as `fixtures/hot-window`. Then §5's kill rule decides between
documenting "run a hot window in debug" and a `[profile.hot]` used by
both the window and `Crate::build`.

**Gate:** T49.0's latency step: median edit-to-swap against 2.0 s,
printed with the profile and the library's size, eight reloads.

**Injection:** build the hot library with one codegen unit and fat LTO:
the median exceeds 2.0 s and the step fails by name.

**Accepts good input:** the chosen profile passes on the same crate.

**Docs in the same commit:** `docs/step4-hot-reload.md:108-110` (the
measured figure, naming the fixture and profile);
`book/src/component-boundary.md` ("sub-second" qualified, and "fast
because the rebuild is one crate"); `docs/plan-window-reload.md` rule 4.

**Effort:** M. **Depends on:** T49.5 and T49.8 (showcase's tree must
derive `Reloadable`); T49.4 (showcase's boot takes arguments).

### T49.13 — the prose that follows

**Closes:** R5-F9.

**Change:** `crates/lumen-static/src/event.rs:615-632`: a *reloadable*
application that implements `Default` is a compile error, and the window
reload boots the program. `docs/step4-hot-reload.md`'s honesty list
(`:112-125`): item 1 becomes the stamp and what it misses (T49.1); item
2, gated (T49.9); item 3, one construction path (T49.4).

**Gate:** none beyond `just static`; this is prose.

**Injection:** none; prose.

**Docs in the same commit:** the two places named.

**Effort:** S. **Depends on:** T49.1, T49.4, T49.9.

## 4. Boundaries of this plan

- **Not R5-F6.** `Host::adopt`'s whole-tree mark belongs to M41, which
  owns Boundaries completeness and gates it in T41.0. T49.5's live step
  depends on that gate.
- **Not unloading.** `docs/plan-window-reload.md` §2.5's three
  conditions still hold, and T49.10's ledger adds none.
- **Not state migration.** A changed type still gets its boot value, now
  with a log line wherever one was missing.
- **No won't-fix items.**
- **A finding that may belong elsewhere:** R4-F8's CI half runs in M43's
  runner. This plan writes the harness and the recipe; M43 decides where
  the job lives.

## 5. Kill rules and risks

**T49.12 is the one performance task. Kill rule, written before the
build:** measure eight reloads of the real-app fixture in the window's
release profile. If the median is under 2.0 s, the task is prose only:
the measured figure replaces the synthetic one. If it is over, add
`[profile.hot]` (release, many codegen units, incremental, debug
assertions matched) and measure eight more. If that median is under
2.0 s, ship the profile. If not, ship "run a hot window in debug" as the
documented setup, with both medians recorded, and correct the 2 s
criterion's wording in `docs/plan-window-reload.md` rule 4.

**T49.10 has a cost kill rule too.** The ledger adds an insert per spawn
and a removal per applied result. Measure the per-result arm that
`docs/results-tasks.md` cites for the inbox, eight runs a side. If the
ledger costs more than that arm's spread, fall back to §2.4(a): count
and warn, correct the docs, and record the carry-over as measured and
not built.

Risks, and what would reveal each:

- **The source hash refuses too much.** A window that refuses every
  build after a framework edit is correct, but if `lumen-static-widgets`
  turns out to define a crossing type after all, its edits would be let
  through wrongly. Revealed by T49.1's accepting case, which must pass
  **and** by a check in the same test that no type in `build_fields` is
  defined outside the hashed crates (`core::any::type_name`'s path).
- **At-least-once surprises someone.** A job with a side effect runs
  twice across a swap. Revealed only by use; the book states it, and the
  warning names the count.
- **The user-state split is wrong for some widget.** Revealed by
  `reload_widget_state.rs`'s table, which has a row per widget and is
  extended whenever a widget is added — the same inventory discipline
  M27 used.
- **miri cannot build the crate at all.** Revealed on the first run of
  `just miri-reload`; the fallback in T49.3 is then taken, and the
  commit says which.

## Appendix: finding → task

| ID | sev now | status | task |
|---|---|---|---|
| R4-F2 | S2 | PARTIAL | T49.12 |
| R4-F3 | S2 | OPEN | T49.10 |
| R4-F5 | S3 | PARTIAL | T49.1 |
| R4-F6 | S4 | PARTIAL | T49.2 |
| R4-F7 | S3 | OPEN | T49.11 |
| R4-F8 | S3 | WORSE | T49.3 |
| R5-F1 | S3 | OPEN | T49.5 |
| R5-F2 | S2 | WORSE | T49.5 |
| R5-F3 | S3 | OPEN | T49.6 |
| R5-F4 | S3 | OPEN | T49.7 |
| R5-F7 | S3 | OPEN | T49.8 |
| R5-F8 | S3 | OPEN | T49.9 |
| R5-F9 | S4 | OPEN | T49.13 |
| R5-F10 | S3 | PARTIAL | T49.4 |
