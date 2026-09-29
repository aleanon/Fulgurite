# M61 — `.lss` stylesheets, parsed at compile time

> **T61.0–T61.7 DONE 2026-09-28; `just ci-verify 0793dd5` green** (28
> steps, 0 failed, 0 skipped). **All three kill rules hold**
> (`docs/results-lss.md` owns the numbers): **K1** zero bytes in all four
> firmwares; **K2** +9 ms (tally) and +44 ms (showcase) of warm build, the
> latter after a fix — the first cut's tree checks spent 88 of the 100 ms;
> **K3** 0 `lumen_static_lss` symbols in a release window, 63 with
> `reload`. Per task:
> T61.0 — the gates committed red, against a stub macro;
> T61.1 — `lumen-static-lss`: Lumen's lexer and parser, trimmed, spans
> kept, 21 refusals by name;
> T61.2 — the lowering and property table (21 setters, held to `Patch` by
> `scripts/check_lss_setters.sh`); `PartialEq` on `Rule` and `Patch`;
> T61.3 — `include_lss!`, errors at `file:line:col`, tracked by cargo;
> T61.4 — ids and classes asserted against the tree (one table pass since
> K2); kinds stay `W1010`'s, pinned;
> T61.5 — `lss::load`, `Program::style_lss`/`style_lss_by`/`live_lss`,
> parity over every `.lss` (`scripts/check_lss_coverage.sh`);
> T61.6 — the tally's sheet is `app.lss`, `LUMEN_SHOT` byte-identical at
> −3, 0 and 7 on both paths; K3 gated;
> T61.7 — the handoff, the book's styling chapter and gaps, the gaps doc,
> this header. **Ten corrections and findings** are recorded below,
> under "What building found".
>
> *Written 2026-09-28.* A stylesheet can be written as a
> `.lss` file and cost nothing at run time that a hand-written
> `&[Rule]` does not already cost. `include_lss!` parses the file while
> the crate compiles and emits the same `const` rules a person writes
> today. A mistake — an unknown property, a bad value, an `#id` the tree
> does not have — is a **compile error at the file's line and column**,
> not a `W1010` finding at run time. The same parser, linked only into
> debug and `reload` builds, reads the file at run time so a colour
> change needs no rebuild.
>
> This is `docs/plan-lumen-parity.md` Phase C's first half, which has
> been unscheduled since `docs/plan-architecture-v2.md`. The second half
> — a cascade at compile time — is **deferred** (§4).
>
> **Depends on** M60 (`docs/plan-tally-slim.md`), whose
> `Program::style_by` is how a state-keyed sheet reaches the frame; the
> pilot (T61.6) uses it.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and shows that it accepts good input.*

## 0. The claim, and what would falsify it

**The claim:** authoring a sheet as `.lss` adds **zero** bytes and
**zero** instructions to a release binary compared with the same sheet
written as `Rule::new(..)` calls, and the run-time cost of styling is
unchanged — the cascade, not the sheet, is what runs
(`docs/results-cascade-cost.md`: a full re-cascade is 15.4 µs at 901
nodes, and it runs only when the sheet or `style_by`'s key changes).

**What would falsify it:** the firmware's `.text` or `.rodata` differing
between the two authorings (kill rule K1, §5), any `lss` symbol in a
release binary (K3), or a build that no longer notices an edited `.lss`
(T61.3's tracking gate).

## 1. Where things stand

- **A stylesheet is a slice of rules.** `pub type Stylesheet<'a> =
  &'a [Rule]` (`style.rs`, "a `.lss` parser (parity Phase C) would
  produce one"). `Rule::new`, `Patch::new` and every `Patch` setter are
  `const fn`; `Selector` is `Id`, `Class`, `Kind` or `Any`, each a
  `&'static str`. So everything a parser would produce can be a
  `static`.
- **What our cascade can apply** is one simple selector per rule and
  the `Patch` setters: `width height padding spacing align visible
  cursor focus_ring color background color_role background_role
  border_width border_color border_color_role radius font_size
  font_weight shadow pressed_dim hover_dim`. Classes are static per type
  (`#[widget_class]`); there is no run-time class toggling.
- **Lumen's `lumen-style`** (`/mnt/Utvikling-linux/Lumen/crates/lumen-style`,
  4,322 lines) is a lexer (337), parser (789) and AST (244) plus a
  resolver. Its grammar is much larger than our cascade: compound
  selectors (`.value.pos`), combinators (descendant, `>`), `:state`,
  `@tokens`, `@theme`, `@keyframes`, `@media`, and run-time classes.
  Its lexer and parser are adoptable; its resolver is Lumen's
  architecture and is not.
- **Mistakes are found late today.** A selector that matches nothing is
  `W1010` from the agent's sheet lint, at run time
  (`crates/lumen-static-agent/src/audit.rs`); a typo in a `Rule::new`
  id compiles.
- **`Rule` and `Patch` do not implement `PartialEq`**, which the parity
  gate (T61.5) needs. Deriving it costs nothing at run time.
- **The tally's sheet is Lumen's `examples/counter/app.lss`, selector
  for selector** (`examples/counter-window/src/app.rs`), which makes it
  the natural pilot and a direct comparison.

## 2. Decisions

### 2.1 v1's grammar is what our cascade can apply — the rest is a named error

Accepted in v1:

- Selectors: `#id`, `.class`, a bare kind word, `*` — one simple part.
- Declarations: one per `Patch` setter, CSS-shaped names
  (`background`, `color`, `padding`, `border-radius`, `border-width`,
  `border-color`, `font-size`, `font-weight`, `width`, `height`,
  `spacing`, `align`, `visible`, `disabled` (M57/A16, added at the M56
  merge), `cursor`, `focus-ring`,
  `box-shadow`, `hover-dim`, `pressed-dim`), values as the setter takes
  them (`px`, `#rrggbb[aa]`, keywords for `align`/`cursor`, `fixed(n)` /
  `fill` / `shrink` for lengths).
- `@tokens { name: value; }` — compile-time constants, `$name` to use.
- **Palette roles as tokens:** `$page`, `$surface`, `$control`, `$text`,
  `$muted`, `$accent`, `$on-accent`, `$focus` (and, since the M56 merge,
  M57/A8's `$raised`, `$border`, `$track`, `$placeholder`, `$scrim`,
  `$danger`, `$on-danger`, which `scripts/check_lss_setters.sh` refused
  the merge without) lower to the `*_role`
  setters, so a `.lss` colour follows `app.setPalette` exactly as
  `Patch::color_role` does (M25/T25.11). A `@tokens` name may not
  shadow a role name — that is an error, because it would silently stop
  following the theme.
- `@sheet name { rules }` — a named block, emitted as its own `const`.
  This is how a `.lss` feeds `Program::style_by` (M60): the key picks a
  block. It is not CSS; it is the one addition v1 makes, and only
  because run-time classes (Lumen's `.value.pos`) do not exist here.

Refused, each with its own message naming this plan's §4: compound
selectors, combinators, `:state`, `@media`, `@keyframes`, `@theme`,
unknown properties, and a declaration whose value the setter cannot
take. **Refused, not ignored**: a stylesheet that silently drops a rule
is the vacuous-gate shape in a different medium.

### 2.2 The macro emits `const` data and tracks the file

`include_lss!(Tree, "app.lss")` expands to a `&'static [Rule]` built
from `Rule::new` / `Patch::new().setter(..)` calls — the same tokens a
person writes. The path is relative to the invoking crate's
`CARGO_MANIFEST_DIR`.

**Rebuild tracking:** a proc macro that reads a file is invisible to
cargo's dependency tracking. The expansion therefore also contains
`const _: &[u8] = include_bytes!("<absolute path>");`, which makes rustc
record the file as an input, so an edited `.lss` rebuilds the crate
(and a hot reload, M37, picks it up). T61.3 gates this, because the
failure — a build that keeps the old sheet — is silent.

### 2.3 Ids and classes are checked against the tree type; kinds are not, in v1

The macro takes the tree type so it can emit, per `#id`:

```text
const _: () = assert!(lumen_static::style::count_wid(<Tree as Widget>::SUB_TABLE, "card") > 0,
                      "app.lss:7:1: no #[widget_id(\"card\")] in Page");
```

`> 0`, not `wid_index!`'s `== 1`: styling several nodes with one id is
allowed (`wid_index!`'s rustdoc says so); only *addressing* them is not.
Classes get the same check, because `NodeDesc` carries them
(`classes: &'static [&'static str]`) and a const fn can scan the table.

**Kinds cannot be checked this way.** `NodeDesc` has `wid` and
`classes` but no kind; a node's kind is `Node::kind_name`, a run-time
method. That is a pity, because the `kind_name` spelling (lowercase
leaves, struct idents for containers) is exactly the trap
`Selector::Kind`'s rustdoc warns about. v1 leaves a kind selector to
`W1010` at run time and says so in the book. Checking kinds at compile
time would need a `const KIND: &str` on `Widget`, emitted by the derive
and by every hand-written impl. That is a separate change with its own
forwarding hazard (a wrapper that forwards it wrongly), so it is listed
in §4, not smuggled into this plan.

An `#id` or `.class` that matches nothing is then a build error, and
`W1010` stays as the check for kinds and for sheets built any other
way.

### 2.4 Run-time reading only in debug and `reload` builds

`lumen_static_lss::load(path)` parses at run time with the same parser
and lowering, for a `Program::style_lss(path)` that re-reads the file
when it changes. Its selectors need `&'static str`; a dev-only loader
may leak them (bounded by edits made in a session), which is recorded,
not hidden. It is behind `debug_assertions` or the `reload` feature, so
a release build without `reload` links none of it (K3).

## 3. Tasks

### T61.0 — the gates first

Committed ignored (`red until T61.n`), with their red output:

1. **Byte parity (K1).** A firmware variant whose sheet is
   `include_lss!` beside the existing hand-written one; `check_text.sh`
   / `firmware-size` must report identical `.text` and `.rodata`.
2. **Errors land on the file.** A `trybuild` (or `compile_fail` doctest)
   per refusal in §2.1 and per bad value, each asserting the message
   names `file:line:col` and the reason.
3. **Id and class checks (§2.3).** A typo'd `#id` and an unknown
   class each fail to compile, by name. A wrong-case kind still
   compiles, and the test asserts `W1010` names it at run time, so the
   documented gap is pinned rather than assumed.
4. **Tracking (§2.2).** A test that edits a `.lss` in a scratch crate,
   rebuilds, and asserts the new rule is in the binary.
5. **Parity of the two paths (T61.5).** For every `.lss` in the repo,
   `include_lss!` and `load()` give equal `&[Rule]`.

### T61.1 — `lumen-static-lss`: the lexer and parser

A new std crate, used by the proc macro at build time and by `load` in
dev builds. The lexer and parser are adopted from Lumen's
`lumen-style` (`lexer.rs`, `parser.rs`, `ast.rs`), trimmed to §2.1's
grammar, keeping spans. Lumen's resolver and animation modules are not
brought over.

**Gate:** the parser round-trips every accepted construct and refuses
every §2.1 refusal with its message. **Injection:** make the parser
accept a compound selector silently — gate 2's compound case goes green
when it must be red, so the gate is shown to watch the refusal, not the
parse.

### T61.2 — lowering: AST to rules

The property table (declaration name → `Patch` setter, value grammar
per setter), `@tokens` substitution, role tokens to `*_role` setters,
and `@sheet` blocks. Derive `PartialEq` for `Rule`, `Patch`, `Selector`
(and what they contain) in the same commit.

**Gate:** every `Patch` setter has a declaration, checked mechanically
against the setter list (a setter added later without a `.lss` name
fails `just static`). **Injection:** drop one entry from the table.

### T61.3 — `include_lss!`

The proc macro, emitting `const` rules plus the `include_bytes!`
tracking line (§2.2), with errors as `compile_error!` spanned to the
macro call and carrying `file:line:col`.

**Gates:** T61.0 items 1, 2 and 4. **Injections:** emit rules through a
`Vec` built at run time instead of `const` data — K1's `.text` equality
fails; delete the `include_bytes!` line — the tracking gate keeps the
old rule.

### T61.4 — validation against the tree type

§2.3's per-id and per-class assertions. Kinds stay with `W1010`.

**Gate:** T61.0 item 3, both ways — the tally's and the showcase's real
sheets still compile. **Injection:** make the id assertion `>= 0`.

### T61.5 — reading at run time, dev builds only

`load(path)` and `Program::style_lss(path)` (re-read on change), behind
`debug_assertions` or `reload`.

**Gates:** T61.0 item 5 (parity of the two paths over every `.lss` in
the repo); K3 — `nm` on a release build without `reload` finds no
`lumen_static_lss` symbol. **Injection:** lower a colour differently in
`load` — the parity gate names the rule.

### T61.6 — the pilot: the tally's sheet as `app.lss`

`examples/counter-window/app.lss`, beside Lumen's
`examples/counter/app.lss`: the base rules, and `@sheet pos`, `@sheet
neg`, `@sheet zero` for the sign, fed through `Program::style_by`.

**Gates:** `LUMEN_SHOT` frames at −3, 0 and 7 byte-identical to the
hand-written sheet's; every recipe that runs the tally stays green
(`just tally`, `tally-drive`, `live-gate`, the plants). **Injection:**
swap two colours in `app.lss` — the shot comparison fails by pixel
count.

### T61.7 — docs

The handoff's styling section (OPEN → DECIDED, pointing here), the book's
styling chapter (an included `.lss` by anchor, per the book's rules),
a new results doc, `results-lss.md` (promised; written by T61.7), with K1–K3's numbers, and
`docs/competitive-feature-gaps.md`'s "a style language" entry.

## 4. Boundaries of this plan

- **Not a cascade at compile time.** Selectors are static per node and
  the tree's shape is a type, so (sheet, tree) determines the resolved
  styles and could be a `const` table. It is deferred: palette roles
  re-resolve at run time, `style_by` sheets are chosen at run time, and
  the saving is a ~15 µs re-cascade that runs only on a change
  (`docs/results-cascade-cost.md`). If it is attempted, it is its own
  milestone with a kill rule written first.
- **Not Lumen's full grammar.** Compound selectors, combinators,
  `:state`, `@media`, `@keyframes`, `@theme` and run-time classes each
  need cascade machinery this framework does not have; each is a named
  compile error until it does (§2.1).
- **Not compile-time kind checks.** They need a `const KIND` on
  `Widget` (§2.3); if wanted, that is its own task with a forwarding
  gate.
- **Not `hover`/`pressed` as pseudo-classes.** They are `hover-dim` and
  `pressed-dim` declarations, because that is what the cascade applies.
- **Not a migration of every example.** The tally is the pilot; the
  showcase's sheets move only if T61.6 shows the path is clean.

## 5. Kill rules and risks

Written before anything is built.

- **K1 — zero bytes.** The firmware's `.text` and `.rodata` with the
  sheet authored by `include_lss!` must equal the hand-written sheet's,
  byte for byte. Any difference is investigated before T61.3 lands; a
  difference that cannot be removed stops the milestone at T61.2.
- **K2 — build time.** `include_lss!` on the tally's and the showcase's
  sheets must add under 100 ms to a warm incremental build of the
  invoking crate, measured under `just bench-lock`, eight runs a side.
- **K3 — nothing in release.** A release build without `reload` links
  no `lumen_static_lss` symbol (`nm`).

Risks:

- **The tracking line is not enough on every toolchain.** *Revealed by*
  T61.0 item 4, which fails if an edit does not rebuild.
- **The `kind_name` spelling trap moves into `.lss`,** and v1 only
  catches it at run time. *Revealed by* T61.0 item 3's wrong-case kind
  case, which asserts `W1010` names it.
- **`@sheet` is non-standard and may confuse readers used to CSS.**
  Recorded in the book; revisited if run-time classes ever exist.

## Appendix: task → gate

| task | gate | injection |
|---|---|---|
| T61.1 | parser accepts §2.1, refuses the rest by name | accept a compound selector silently |
| T61.2 | every `Patch` setter has a declaration | drop one table entry |
| T61.3 | K1 byte parity; errors at `file:line:col`; edit rebuilds | run-time `Vec`; delete `include_bytes!` |
| T61.4 | bad id/class fail to compile; real sheets compile; bad kind is W1010 | id assertion `>= 0` |
| T61.5 | both paths equal on every `.lss`; K3 no symbol in release | lower a colour differently in `load` |
| T61.6 | tally shots byte-identical at −3, 0, 7 | swap two colours in `app.lss` |

## What building found

- **2026-09-28, T61.0 — K1 cannot compare a cargo feature against its
  absence.** The first K1 script built the `include_lss!` side with a
  fixture feature, and the two *paint-only* firmwares — which never read
  `SHEET` — came out 780 `.text` bytes apart at equal sizes. A feature
  changes the crate's `-C metadata`, so every mangled name changes, and
  the linker lays same-sized functions out in a different order. The
  choice is now a cfg the fixture's `build.rs` sets from `LUMEN_SHEET=lss`,
  which leaves the metadata alone: the two builds of `firmware` and
  `firmware_bind` are then byte-identical, and the stub's empty sheet
  shows as the only difference (−396 B `.rodata` in the two firmwares
  that cascade).
- **2026-09-28, T61.0 — item 5 is written with T61.5**, because `load` —
  one of the two sides it compares — is T61.5's API; a gate over a
  function that does not exist cannot be committed ignored, only
  uncompiled.
- **2026-09-28, T61.3 — the path is relative to the invoking file, not
  to `CARGO_MANIFEST_DIR`** as §2.2 said. A trybuild case compiles inside
  a generated crate under `target/tests/trybuild/`, so a manifest-relative
  path could not name the case's own `.lss`, and gate 2 could not have
  been written as trybuild cases at all. `include_str!` and
  `include_bytes!` — which the tracking line already is — resolve against
  the invoking file, so `include_lss!` now does too
  (`proc_macro::Span::local_file`, stable since 1.88), falling back to the
  manifest directory only when the compiler does not know the file.
- **2026-09-28, T61.3 — the planned K1 injection cannot reach K1.** "Emit
  the rules through a `Vec` built at run time" does not compile in the
  leaf firmware — it has no allocator, and a `const` cannot hold one — so
  the build fails before any byte is compared. The injection used instead
  breaks the same mechanism (the expansion must be the rules a person
  writes, in order): emit the rules **reversed**. Sizes stay equal and
  `.rodata` differs in both firmwares that cascade, which is also the case
  for comparing bytes rather than sizes.
- **2026-09-28, T61.4 — "the showcase's real sheet still compiles" needed
  a showcase `.lss`, which §4 said not to migrate.** It is written as a
  **mirror**, not a migration: `examples/showcase/showcase.lss` is 49 rules
  checked against `Screen` at compile time, and a test holds it equal,
  rule by rule, to the `stylesheet()` the showcase actually runs. That is
  the accept direction of the tree checks on a sheet nobody wrote for
  them — including three part ids a composite stamps (`#select-trigger`,
  `#select-menu`, `#tooltip-content`), which are in `SUB_TABLE` and so
  pass — and it is K2's larger arm.
- **2026-09-28, T61.5 — "behind `debug_assertions`" cannot be written in
  the shell.** The workspace's dev profile builds `lumen-static-shell` with
  `debug-assertions = false` (for the rasteriser's speed), so a
  `cfg(debug_assertions)` there is false in every debug build — the first
  cut of `lss` compiled out of exactly the builds it was for, and the
  parity test could not import it. M36 met the same trap
  (`recover.rs`'s module note). The decision is the window's now:
  `Program::style_lss`/`style_lss_by` keep the compiled sheet *and* the
  path, and `lumen-static-winit`'s `run` calls `Program::live_lss` under
  its own `cfg(any(debug_assertions, feature = "reload"))`. The reader is
  compiled into the shell unconditionally and reached only from there, so
  a release window without `reload` drops it at link time — which is what
  K3 counts.
- **2026-09-28, T61.5 — `style_lss` takes the compiled sheet too.** §2.4
  named `Program::style_lss(path)`; a program must build in release, where
  there is no reader, so the release answer has to be in the call:
  `style_lss(include_lss!(..), path)`, and `style_lss_by(key, compiled,
  path, block)` for a keyed sheet. A file that does not lower draws with
  the compiled sheet and says why on stderr.
- **2026-09-28, T61.5 — K3's gate lands with T61.6**, because the tally is
  the first program that calls `style_lss_by`: a symbol count over a
  binary that never names the reader is 0 whatever the gating does, and
  the gate's accept direction (above 0 with `reload`) needs a caller.
- **2026-09-28, K2 — §2.3's `count_wid(..) > 0` per id nearly spent the
  build budget.** Measured first with the checks as §2.3 wrote them: the
  showcase's 42 names cost **+88 ms** of const evaluation, +94 ms in all
  against K2's 100 ms. Each assertion walked the whole table. The
  expansion now collects the tree's ids and classes once
  (`style::wids_of`, `style::classes_of`) and each assertion reads that
  list: 88 → 31 ms, +44 ms in all (`docs/results-lss.md`). The rule held
  both times; the margin is why it was fixed rather than recorded.
- **2026-09-28, T61.7 — there was no OPEN marker to flip.** The handoff
  had no section on a style file format, so M61's decision is a new
  **DECIDED** paragraph beside M60's `style_by`, pointing here.
