# M55 — the derive refuses what it ignores, the lints see clips, and the docs count what exists

> **Written 2026-09-24, not started.** Closes R12-F5, R16-F1, R16-F2,
> R16-F3, R16-F4, R16-F5, R16-F6, R16-F10, R16-F11, R8-F9, R9-F11 and
> R9-F12 from M38's deep review (`docs/results-deep-review.md`), as
> re-triaged against `main` at 1717808 on 2026-09-24. Where the review
> and the re-triage differ, the re-triage is authoritative.
>
> **Phase 5** of the fix queue. It comes after M43, which reworks the
> static scripts this plan extends (`scripts/check_attr_table.sh`,
> R17-F4) and provides the local CI runner, and after M42, whose harness
> reports which invalidation strategy it drove — a report T55.4 changes
> the source of. §3 says which tasks may land earlier, and why.
>
> None of these findings is a wrong frame on a shipped path. They are
> the places where the framework **accepts something and does something
> other than what was written**: an attribute argument dropped, a user's
> identifier captured by a derive-internal local, a non-finite length
> turned into a blank window, a lint that fires on healthy scroll
> content, a feature that changes another crate's types, and a book that
> says menus are not built. Each is cheap. Together they are most of what
> an author meets before anything else in this repository.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and prints the measured quantity beside its bound.*

---

## 0. The rules for this milestone

1. **A refusal ships with its acceptance.** Every new derive error has a
   `fail_` case pinning its sentence, and a `pass_` case that is one
   token away from it and compiles (AGENT.md rule 3).
2. **An error path with no test is visible.** A count of the derive's
   `syn::Error` sites against the `.stderr` files that pin them is
   printed on every run (T55.2), so the next untested path is a number,
   not a review finding.
3. **Author input is sanitised once, where it enters.** The cascade is
   the choke point for style lengths. Nothing is sanitised per frame in
   layout, and nothing is repaired silently: every repair is reported.
4. **A count in prose is computed or removed.** "Seven, across three
   derives" became wrong twice (R16-F10). Where a number describes code,
   a check computes it or the sentence stops stating it.

## 1. What is wrong, at 1717808

### 1.1 The `App` derive's `changed` local is visible to the user's tokens (R16-F3)

M33/T33.1 and M29 rewrote `#[bind]`'s expansion, and the hazard
survived. `update_marked` in `crates/lumen-static-macros/src/lib.rs`
declares `let mut changed = false;` at call-site span (`:2089-2090`),
then splices the bindings. A format binding writes the user's extra
arguments into `format_args!(#fmt, &#src #(, #extra)*)` (`:1937`),
so an extra argument spelled `changed` resolves to the derive's `bool`
and prints `false` instead of the field the user meant. A `with =`
binding expands to `let f: fn(&mut _, &_) = #path;` (`:1961`), so a
user function named `changed` is shadowed and rustc reports a
fn-pointer mismatch at the user's token. The locals M29 added are
`__ln_`-prefixed (`__ln_all`, `__ln_site`, `__ln_p`, `__ln_i`,
`__ln_mark`); `changed` is the one ordinary word still exposed. The
evidence is the review's `examples/m08.rs` and `m13.rs` (R16-F3;
`docs/results-deep-review.md`).

### 1.2 The derives accept arguments and repeats they then ignore (R16-F4)

- **Axis keywords overwrite** (`lib.rs:197-213`): `#[widget(row,
  column)]` lays out as a column, and so do two separate `#[widget]`
  attributes. The comment at `:229-231` calls accepting-and-ignoring
  "the shape the `stack`/`spacing` check below exists to refuse", about
  `columns` only.
- **`parse_wid` returns the first `#[widget_id]`** (`:143-156`) and
  accepts an empty string; one on the struct silently becomes the root's
  empty id.
- **`#[tree]` is detected by path alone** (`:1868`), so `#[tree(junk =
  5)]` compiles. **New since the review:** `#[state]` is detected the
  same way (`:2024`, T33.0), so `#[state(junk)]` is accepted silently —
  a second instance of the same shape, added in the milestone that
  introduced the attribute.
- **`#[app]` keys overwrite on repeat** (`:1850-1859`): `tick = t1,
  tick = t2` runs `t2`.
- **Five error sentences have no `.stderr`**: "a second `#[tree]`
  field" (`:1870`), "an enum widget needs at least one variant"
  (`:1367`), the unknown `#[app]` key, the unknown `#[widget]` key, and
  "expected `with`" (`:1738`). "A grid needs at least one column"
  (`:239-241`) is also untested. The review disabled the second-tree and
  zero-column checks together and the `ui` suite stayed green (R16-F4).

### 1.3 A non-finite or negative length is accepted and breaks the frame silently (R16-F5, R16-F6)

`Patch::apply_to` (`crates/lumen-static/src/style.rs:781`) copies
lengths into a `Style` unchecked, and nothing in `style.rs` or
`pass.rs` tests finiteness. One NaN spacing, padding or font size makes
most of the tree's `NodeState` non-finite, and every `contains` against
a NaN rect is false, so the window is blank and unclickable with no
panic and no diagnostic (R16-F5; `docs/results-deep-review.md`).
`slider.rs:90-95` names exactly this hazard. The re-triage lowered it to
S3: the only source is an author-written length in Rust, and no
stylesheet in the repository can produce one.

Padding is also copied unclamped (`crates/lumen-static/src/ops.rs:348`),
and `inner_limits` (`:161-168`) clamps only the *result* of subtracting
it, so a negative padding widens the content box and `place` offsets
children outside their parent. They are painted and cannot be clicked,
because hit testing assumes children lie inside parents (R16-F6).

### 1.4 Two lints do not know about clipping (R16-F1, R16-F2)

W1002 (`OFFSCREEN`, `crates/lumen-static-agent/src/audit.rs:99-100`,
emitted at `:711`) compares each node against the **root's** bounds, with
no clip or scroll ancestor considered, so it fires on the rows of an
unscrolled `ScrollView` whose content runs past the window. It has no
test in either direction: the review filtered W1002 out of the findings
entirely and every existing test stayed green (R16-F1), against a
`tests/lint.rs` whose header promises "a firing case *and* a quiet case"
for each lint.

W1003 is reserved (`audit.rs:101-113`) on the claim that a child is
never allotted more space than its slot, and says it "ships with the
first construct that can exceed its constraints: a scrollable content
region". `ScrollView` is that construct and has shipped. The tripwire
meant to announce it,
`overflow_is_impossible_here_so_squeezing_surfaces_as_truncation`
(`crates/lumen-static-agent/tests/lint.rs:207`), builds only the healthy
fixture, with no scroller and no negative padding, so it cannot fire —
the "setup silently did not happen" shape (R16-F2).

### 1.5 A diagnostic feature changes another crate's types (R12-F5)

`lumen-static-shell`'s `Inval` is `Boundaries<256>` unless the
`everything` feature is on, in which case it is `Everything`
(`crates/lumen-static-shell/src/session.rs:48-51`). `Session::host`
(`:171`), `Live::host` (`:457`) and `Pane::host`
(`crates/lumen-static-winit/src/windows.rs:37`) are public fields of type
`Host<A, Ret, Inval>`. Cargo unifies features across a build graph, so
one crate turning `everything` on changes those fields' types for every
other crate. **The shape inverted with M34** (re-triage, at 1717808):
`Boundaries<256>` is now also `Host`'s default type parameter
(`crates/lumen-static-shell/src/lib.rs:61`), so the most natural
spelling, plain `Host<A>`, is exactly the one that stops compiling when
anything in the graph turns the diagnostic on. The re-triage re-ran the
review's probe adapted to that spelling: green without the feature,
E0308 with it. The failure is loud and at compile time, which is why it
stays S3.

### 1.6 The book and the tables describe the code as it was (R16-F10) — WORSE

`book/src/gaps.md` still lists menus as "not built" (`:31-34`;
`Menu` is `Chooser<MENU>` since T27.9), rich text as having "no spans
within a run" (`:40-41`; `RichText` and `Markdown` since T27.12), and
the drawing vocabulary as "nine commands" (`:44-47`; `Cmd` has eleven
variants in `crates/lumen-static/src/display.rs`). The attribute tables
(`crates/lumen-static/src/lib.rs:40-57`, `book/src/attributes.md:3-20`)
omit `#[widget(wrap)]`, `#[widget(grid, columns = N)]` and
`#[app(tick = f)]`. **It is WORSE than the review found**: T33.0 and
T33.1 added the `#[state]` and `#[bind]` rows correctly, and left both
tables opening with "Seven, across three derives" — while they now list
eight attribute names (`widget`, `widget_id`, `widget_class`, `reload`,
`tree`, `state`, `bind`, `app`). The count became wrong a second way in
the same pass that updated the rows. `scripts/check_attr_table.sh`
compares the two tables with each other and nothing else, so it cannot
see either kind of drift. `crates/lumen-static-widgets/src/textarea.rs:27`
still says "No selection and no IME" about a widget with both.

### 1.7 Numeric edges in the range widgets (R16-F11)

`clamp_finite` (`crates/lumen-static-widgets/src/field.rs:131-136`)
returns `lo` for a non-finite value, while its own rustdoc (`:125-130`)
says "the last good value is the honest answer". With `min > max` it
clamps to `hi.max(lo)`, which is `lo`, above the stated maximum.
`SplitPane::set_split` (`split.rs:70-75`) clamps through
`lumen_static::num::clamp` (`crates/lumen-static/src/num.rs:109-117`),
which passes a NaN straight through, so `split()` can report NaN
(R16-F11; `docs/results-deep-review.md`).

### 1.8 Three stated numbers that drifted (R8-F9, R9-F11, R9-F12)

- `crates/lumen-static-winit/src/surface.rs:157` cites "present p95
  0.044 ms" to `docs/results-frame-cost.md`, which says 0.039 ms
  (`:138`). `docs/results-appearance.md:64` and `:73` price a node with
  `NodeState` and `Style` sizes that `crates/lumen-static/tests/cursor.rs:33`
  no longer asserts (R8-F9).
- `benches/src/bin/glass_cost.rs:221` labels its comparison "so only the
  bevel differs", and compares `FROSTED` with the full `LIQUID`
  material. `docs/results-glass.md` §5b's "the bevel" column is
  therefore the rest of the liquid material, and did not reproduce
  (R9-F11).
- `docs/results-gpu.md` §7 (`:371`) says the crossover "is bounded but
  not located"; §5b of the same doc locates it (R9-F12).

## 2. Decisions

### 2.1 R12-F5: the strategy is a default type parameter, chosen in code, not a feature

The three options:

- **(a) A type alias per strategy.** A `WindowHost<A>` alias for
  `Host<A, Ret, Inval>`, used in every public signature, so the easy
  spelling is the one that follows the feature. It steers. It does not
  remove the trap: plain `Host<A>` still compiles until some other crate
  turns the feature on.
- **(b) A default type parameter that is not a feature.
  Recommended.** `Program`, the shell's `Local` session and `Pane` gain
  an invalidation parameter defaulting to `Boundaries<256>`, and an
  application that wants the diagnostic says so in its own code
  (`application(boot).invalidation::<Everything>()`, spelled however
  T55.4 settles it). **The shell already has the shape**:
  `Program::host` is generic over the strategy
  (`crates/lumen-static-shell/src/program.rs:271`), so only the window
  side hard-codes `Inval`. The window runs a `Box<dyn Session>`, so the
  parameter never reaches `run`'s signature. The applications that
  build both strategies — showcase and tasks-demo — already have their
  own `everything` feature (`examples/showcase/Cargo.toml:25`); it stops
  forwarding to `lumen-static-winit/everything` and instead selects the
  type in the application's `main`. That is additive, because an
  application's own feature changes only that application's code.
- **(c) Make `everything` additive** by turning the strategy into a
  run-time choice, or by implementing the diagnostic as `Boundaries`
  plus `mark_all` every frame. The first costs what M34 chose a type
  parameter to avoid. The second keeps `Boundaries`' own bookkeeping on
  the diagnostic path, so a defect in that bookkeeping could survive the
  very build meant to expose it.

**What (b) breaks.** `lumen-static-winit/everything` and
`lumen-static-shell/everything` go away. `scripts/panic_drive.sh`
names the winit feature directly and moves to the application's own.
Two places read the feature today and must stop: the ABI stamp's
feature list at `session.rs:561`, which reads
`cfg!(feature = "everything")` and loses that entry; and, once M42 has
landed, M42/T42.1's process report, which takes the strategy from the
name of the `Inval` type and lists `everything` among its features. That
report must read the session's type parameter instead — and M42's
harness identity is built on it, so T55.4 keeps M42's gate green or it
does not land. `docs/plan-boundaries-default.md`
rule 2 (the escape hatch "named for its purpose") keeps its purpose and
loses its spelling.

### 2.2 R16-F5 and R16-F6: sanitise in the cascade, report through a new lint

Non-finite lengths and negative padding are one decision: neither is a
feature here. There is no negative-margin model, and hit testing assumes
children lie inside their parents. So one function, called where a
`Patch` is applied to a `Style`, maps a non-finite length to the field's
type default and a negative padding to zero, and counts each repair
with its node, field and value. The agent reports the repairs as
W1012 (NON_FINITE_STYLE is the re-triage's name; T55.5 may widen it,
since negative padding is not non-finite). A `debug_assert!` that every
laid-out rect is finite sits at the end of `pass::layout`, as a net for
lengths that do not come through the cascade.

Rejected: **keeping negative padding legal and shipping W1003 to report
the escape.** It leaves a painted, unclickable control possible and only
tells the author afterwards.

**Numbering.** W1012 is claimed here. M54 does not claim W1013 (it
defers its contrast lint), so no other plan in this series takes a code.

### 2.3 R16-F1 and R16-F2: one clip-ancestor walk, used by both lints

W1002 compares a node against its nearest clipping ancestor, not the
root: a node outside that ancestor's rect is scrolled-out content and is
not W1002. W1003 is a child escaping its parent where no ancestor
between them clips; `ScrollView` and clip parents are exempt, and
`stack` children are W1004's.

**The audit cannot currently ask which node clips.** At 1717808 no
`Node` query answers it: `ScrollView` and `Repeat` clip by pushing a
clip in `paint`, which the audit does not see. So T55.6 adds one beside
`overlaps_children` and `virtualizes` — provided, default `false`,
answered by `ScrollView` and `Repeat`, and not behind `observe`, for the
reason `overlaps_children`'s rustdoc gives (a lint must agree with a
release build about what the tree is). **Provided is right here for the
same reason it is for `overlaps_children`**: it is asked of each node by
its own table index, so a wrapper such as `Glass` is asked about itself
and its scroller content is asked about itself. Neither answers for the
other.

**Whether W1003 ships is decided by
probing after T55.5**, because T55.5 removes negative padding, which is
one of the two constructs known to escape. If an oversized `Fixed`, a
`stack` child or a minimum larger than the parent's maximum still
escapes a non-clipping parent, W1003 ships. If none does, W1003 stays
reserved, and its rustdoc and tripwire are corrected to say the
invariant holds *for non-clipping containers* and to include a
`ScrollView` in the tripwire's fixture, so the tripwire can fail.

## 3. Tasks

**May land early.** T55.1, T55.2, T55.7 and T55.8 touch nothing any
earlier milestone touches (the derive's parser and expansion, two widget
clamps, and prose), and each is small. T55.3 waits for M43's R17-F4,
which rewrites the same script. T55.4 waits for M42. T55.5 and T55.6
wait for M41, which is changing how the frame marks and what the
shadow oracle inspects.

The re-triage's guidance put R16-F11 with the doc-only items. It is
code with a behaviour test, so it is its own task (T55.7); the doc
items are T55.8.

### T55.0 — the gates first

Each is committed red at 1717808, with the red state in the commit body.

1. `crates/lumen-static-macros/tests/ui/pass_bind_names_like_locals.rs`:
   an application with a field `changed: u32` bound as an extra format
   argument, and a free function named `changed` used as `with =
   changed`. **Red today**: it fails to compile on the `with` case. A
   runtime twin in `crates/lumen-static/tests/derive_app.rs` asserts the
   label reads `3 42`; with the `with` case removed it compiles and
   prints `3 false`.
2. The thirteen `fail_` cases of T55.2. **Red today** for the eight new
   refusals (each compiles); the five for untested messages are green
   today and go red only under the review's `11d3b34`, which disabled the
   second-tree and zero-column checks — that red run is recorded. The
   coverage script prints `syn::Error sites: n, pinned by a .stderr: m`
   and fails while `m < n`.
3. `crates/lumen-static-shell/tests/non_finite_style.rs` (the review's
   `r16_nan` cases): NaN spacing, infinite and NaN padding, NaN font
   size, NaN radius, and padding −50, under both strategies. **Red
   today** on non-finite state counts, on child escapes, and on the
   click that should land.
4. `tests/lint.rs`: `w1002_quiet_on_scrolled_out_content` (the review's
   40 px window with a `ScrollView` of four labels, unscrolled and
   scrolled) **red today** at 2 and 1 findings against 0.
5. `crates/lumen-static-widgets/tests/numeric_edges.rs` (the review's
   `r16_num`): **red today** on the NaN-step and `min > max` rows and on
   `set_split(NaN)`.
6. A two-invocation compile test for R12-F5: a function taking a pane
   and returning `&mut Host<A>`, built without and with
   `lumen-static-winit/everything`. **Red today** in the second build
   with E0308.
7. `scripts/check_attr_table.sh`'s key check (T55.3). **Red today**,
   naming `wrap`, `grid`, `columns` and `tick`. This item is committed
   after M43/T43.3, which rewrites the same script, as the first half
   of T55.3's work; the other six items do not wait for it.

**Effort:** M. **Depends on:** nothing, except item 7 (M43/T43.3).

### T55.1 — hygienic derive locals

**Closes:** R16-F3.

**Change.** In `update_marked`, `changed` becomes `__ln_changed`, and
its `let` and every use of it — in `locate`'s closure (`:1909`), the
`with =` arm (`:1963`), the `after_update` arm (`:1989`) and the final
`Wrote::of` — are emitted at `Span::mixed_site()`, so tokens at the
user's span cannot name it. The same for `__ln_all`. User tokens
(`#extra`, `#path`, `#src`, `self.#ident`) keep their own spans, so
errors still point at the user's code. `__ln_site` is a `static` item
in the non-generic arm, and item names are not hidden by
`mixed_site`; the prefix is its only protection, and the rustdoc says
so rather than implying otherwise.

**Gate.** T55.0's `pass_bind_names_like_locals` compiles, and the
runtime twin prints `LABEL=[3 42]` against the expected value.

**Injection.** Put only the `let` of the renamed local back at call-site
span while leaving its uses mixed-site: the build fails at the uses,
which shows the spans are what separate them. Rename it back to
`changed` at call-site span everywhere: the runtime twin prints
`[3 false]`. *Good input:* every existing `pass_` case and
`derive_app.rs` stay green, so user-facing `self.field` paths still
resolve.

**Docs in the same commit.** The `#[bind]` rustdoc's "further arguments
are written out in full" says they are evaluated in the user's scope
only.

**Effort:** S. **Depends on:** T55.0.

### T55.2 — refuse repeats and arguments; pin every error sentence

**Closes:** R16-F4.

**Change.** An axis given twice is an error naming the first; a second
`#[widget_id]` on a field, an empty one, and one on the struct are
errors; `#[tree]` and `#[state]` refuse arguments
(`require_path_only`); a repeated `#[app]` key is an error. A small
script, `scripts/check_derive_errors.sh` in `just static`, extracts the
sentence of every `syn::Error` in `crates/lumen-static-macros/src/lib.rs`
and requires each to appear in some `tests/ui/*.stderr`.

**Gate.** `fail_axis_twice`, `fail_widget_id_twice`,
`fail_widget_id_empty`, `fail_widget_id_on_struct`, `fail_tree_with_args`,
`fail_state_with_args`, `fail_app_key_twice`, `fail_tree_twice`,
`fail_enum_no_variants`, `fail_app_unknown_key`,
`fail_widget_unknown_key`, `fail_bind_expected_with` and
`fail_grid_zero_columns`, each with its `.stderr`. The script prints
`syn::Error sites: n, pinned: m (bound m == n)`.

**Injection.** The review's `11d3b34` (disable the second-tree and
zero-column checks): `fail_tree_twice` and `fail_grid_zero_columns` go
red with "expected to fail to compile, but it succeeded". Delete the
new axis-twice check: `fail_axis_twice` goes red. Add an error with a
new sentence and no case: the script names it. *Good input:* a `pass_`
case using `#[widget(grid, columns = 3)]` with one axis, and one using
a single `#[state]` and a single `#[tree]`, still compile — so the
refusals do not refuse legal input.

**Docs in the same commit.** The comments at `lib.rs:143` and
`:229-231`; both attribute tables gain a line saying repeats are errors.

**Effort:** S. **Depends on:** T55.0.

### T55.3 — the attribute tables and the book, and a check that reads the parser

**Closes:** R16-F10.

**Change.** `book/src/gaps.md`: menus and rich text leave "genuinely
absent", or are restated precisely (no menu bar; Markdown has no
italic, as T27.12 records); the drawing vocabulary cites the `Cmd` enum
instead of a count. Both attribute tables gain rows for
`#[widget(wrap)]`, `#[widget(grid, columns = N)]` and
`#[app(tick = f)]`, and lose "Seven, across three derives" (rule 4).
`textarea.rs:27` describes the selection and preedit API it has.
`scripts/check_attr_table.sh`, after M43's R17-F4 has taught it to read
each derive's `attributes(...)` list, also extracts every key the
parser accepts — each `is_ident("…")` inside the `#[widget]`, `#[app]`,
`#[bind]` and `#[reload]` parsing — and requires each to appear in both
tables. If `gaps.md` names a command count, it is compared with the
variant count of `enum Cmd`.

**Gate.** The check prints `parser keys: n, in rustdoc table: a, in book
table: b, missing: <list>` and fails on a non-empty list, and prints
`Cmd variants: 11, gaps.md says: k` when a count is present.

**Injection.** Delete the `wrap` row from `attributes.md`: the check
names `wrap` as missing from the book. Add an `is_ident("foo")` key to
the parser with no row: the check names `foo`. *Good input:* the updated
tables pass with `missing: (none)` and a key count above eight, which
shows the extractor saw the axis keys and not only the attribute names.

**Docs in the same commit.** Everything above is docs; the commit is the
doc-currency rule applied to the check that enforces it.

**Effort:** S. **Depends on:** M43/T43.3 (R17-F4).

### T55.4 — the invalidation strategy is a type the application chooses

**Closes:** R12-F5.

**Change.** §2.1 (b). `Program`, the shell's `Local` and `Pane` gain the
strategy parameter with its default. `Local` already has a second
parameter by then, M52/T52.7's recovery policy, so this one comes after
it and M52's marker types stay as they are. `Inval` and both
`everything` features are removed; showcase's and tasks-demo's own
`everything` features select the type in `main`; M42/T42.1's process
report takes the strategy from the session's type and drops
`everything` from its feature list; the ABI stamp's feature list drops
it too; `scripts/panic_drive.sh` builds with the application's
feature.

**Gate.** `crates/lumen-static-winit/tests/inval_param.rs`: a function
from `&mut Pane<A>` to `&mut Host<A>` compiles, and so does one from
`&mut Pane<A, Everything>` to `&mut Host<A, Incremental, Everything>`.
`just dev-shape` builds the workspace with showcase's `everything`
feature on and runs this test, so the feature is proven not to reach it.
M42's harness identity reports `Everything` and `Boundaries<256>` for
the two builds of `just widget-drive`, both printed.

**Injection.** Put back a `cfg(feature)` that changes `Pane`'s default:
the `dev-shape` build of the test fails with E0308. Report the strategy
from a constant instead of the session's type: M42's identity check
reports `Boundaries<256>` for the `everything` build and fails. *Good
input:* both builds of the test compile in the same session.

**Docs in the same commit.** The handoff's invalidation section; the
`Inval` rustdoc's replacement on the new parameter;
`docs/plan-boundaries-default.md` (T34.2's feature, and rule 2's
spelling); both manifests' feature comments; `AGENT.md`'s description
of how the drive runs "both strategies" if it names the feature; the
book, wherever `lumen-static-winit/everything` appears (grep first).

**Effort:** M. **Depends on:** M42 (T42.1's process report and T42.2's
launcher); M52/T52.7, which changes `Local`'s parameters first.

### T55.5 — sanitised lengths, reported

**Closes:** R16-F5, R16-F6.

**Change.** §2.2. One sanitising function called from `Patch::apply_to`
for every length field (padding, spacing, font size, radius, border
width, fixed lengths), negative padding clamped to zero, each repair
counted with node, field and value. The agent's audit reports W1012 per
repair. A `debug_assert!` on finite rects at the end of `pass::layout`.

**Gate.** T55.0's `non_finite_style.rs` goes green: per row it prints
`non-finite states: n / 6 (bound 0)`, the count of display commands
carrying NaN (bound 0), `escapes: n (bound 0)` for the padding row, and
the click on `#inc` moving the count from 7 to 8 — under both
strategies. A lint row asserts one W1012 per poisoned field, count
printed.

**Injection.** Remove the sanitising call: non-finite states return and
the click misses. Sanitise every field but `spacing`: only the spacing
row fails, which shows the rows are independent. Remove the negative
clamp: `escapes: 2` and the click misses. *Good input:* the finite
control row and rows with padding 0 and 8 pass with zero W1012
findings, so the sanitiser does not rewrite legal values.

**Docs in the same commit.** `style.rs`'s rustdoc on `Patch` and on
padding (non-finite and negative values are repaired and reported);
`slider.rs:90-95` points at the cascade guard; the audit's code table
for W1012; the handoff's lint list; `audit.rs`'s W1003 text loses
negative padding as a way to escape.

**Effort:** M. **Depends on:** M41 (the frame and oracle this touches).

### T55.6 — W1002 against the nearest clip, and W1003 decided

**Closes:** R16-F1, R16-F2.

**Change.** §2.3. The clip query on `Node`, answered by `ScrollView` and
`Repeat`; the clip-ancestor walk in `audit.rs`; W1002 compares against
it; the W1003 probe runs over the three candidate constructs,
and W1003 either ships with its exemptions or stays reserved with its
rustdoc and tripwire corrected.

**Gate.** `tests/lint.rs` gains `w1002_fires_on_a_node_placed_off_root`
(a node outside the window with no clipping ancestor; one finding, its
rect and the root's printed) and T55.0's quiet case goes green. For
W1003, either `w1003_fires_on_escape_without_clip` and
`w1003_quiet_inside_scrollview`, each printing its count against its
expected value, or the rewritten tripwire whose fixture now contains a
`ScrollView` and asserts the invariant only for non-clipping parents.

**Injection.** Filter W1002 out of the findings (the review's
retain-filter): the firing case fails at 0 against 1. Compare against
the root again: the quiet case fails at 2 against 0. Remove
`ScrollView`'s answer to the clip query: the quiet case fails the same
way, which shows the walk reads the query and not the widget's type. If
W1003 ships,
remove its clip exemption: the scroller case fails at 1 against 0. If it
stays reserved, drop the clip check from the tripwire's invariant: the
tripwire fails on the scroller's content. *Good input:* both directions
of each lint run in one session.

**Docs in the same commit.** `audit.rs`'s W1002 text ("outside its
nearest clip") and W1003 text; the comment at `lint.rs:192-200`; the
W10xx table wherever it is documented (grep the book for W1002).

**Effort:** S. **Depends on:** T55.5.

### T55.7 — the range widgets keep their last good value

**Closes:** R16-F11.

**Change.** `clamp_finite` takes the current value and returns it for a
non-finite input; `NumberInput` and `RangeSlider` order their bounds
once, at construction; a non-finite step is treated as no step;
`set_split` refuses a non-finite fraction and returns `false`. The
rustdoc of `num::clamp` states that it passes a NaN through, so a caller
knows to guard.

**Gate.** T55.0's `numeric_edges.rs` goes green: a NaN step then Up
keeps 5 (printed `after Up: 5 (last good 5)`); a range with NaN step
keeps (0.3, 0.7); `min 10, max 0` yields a value inside [0, 10]
(printed); `set_split(NaN)` returns `false` and `split()` is unchanged.

**Injection.** Return `lo` again from `clamp_finite`: the NaN-step rows
fail at 0 against 5. Remove the guard in `set_split`: `split()` prints
NaN and fails. *Good input:* step 1 moves 5 to 6, and step 0 stays a
no-op.

**Docs in the same commit.** `field.rs:125-130` (now true); `split.rs`'s
`set_split`; `num.rs`'s `clamp`.

**Effort:** S. **Depends on:** T55.0.

### T55.8 — three numbers, stated once

**Closes:** R8-F9, R9-F11, R9-F12.

**Change.** `surface.rs:157` drops its number and cites the section of
`docs/results-frame-cost.md`. `docs/results-appearance.md:64` and `:73`
are annotated as the 2026-09-15 sizes, with today's sizes cited to
`crates/lumen-static/tests/cursor.rs` rather than restated.
`glass_cost.rs` compares `FROSTED` with the bevel-only material its own
M21 arm already builds, or its row is relabelled "the rest of
`LIQUID`"; either way the row is re-measured, eight runs a side under
the bench lock, and `docs/results-glass.md` §5b is corrected with the
new figures stated there once. `docs/results-gpu.md` §7's crossover
bullet becomes a pointer to §5b.

**Gate.** None for the prose; the existing cited-path check covers the
citations. The bench row's label names the material it compares.

**Injection.** Not applicable to a doc fix. The re-measure records its
spread beside its median, so a reader can see whether the correction is
larger than the noise.

**Docs in the same commit.** The four files above.

**Effort:** S. **Depends on:** nothing. The bench lock is the review's
convention (`docs/plan-deep-review.md` §3); M43 does not build one.

## 4. Boundaries of this plan

- **Book coverage of the widget set** (the review's 69 of 91 widget
  types never named in the book) is not closed here. It is a book
  milestone's scope, and a list of names is not a chapter.
- **A numbers-stated-once static rule** (flagging a number in rustdoc
  followed by a `docs/` citation) was proposed by the re-triage for
  R8-F9 as optional. It is not built here: it belongs with M43's other
  static rules, and this plan's T55.8 removes the three instances it
  would have caught.
- **`with =` and `after_update` still report `true` unconditionally.**
  That is M29's narrowing question, not hygiene.
- **The derive's `__ln_site` static** is hidden only by its prefix
  (T55.1). A user who names an item `__ln_site` in the same module gets
  a collision; that is accepted and documented.

## 5. Kill rules and risks

There is no performance work here, so there is no performance kill
rule. What would stop or change the milestone:

- **T55.4 cannot keep M42's strategy report honest.** If the harness
  identity cannot read the strategy from the session's type, T55.4
  falls back to §2.1 (a) — the alias — and records why in §2.1. *Revealed
  by* T55.4's second injection.
- **`mixed_site` hides something a user legitimately reaches.** If an
  existing application reads a derive-generated local on purpose, the
  hygiene change breaks it. *Revealed by* the full workspace build in
  T55.1; none is expected, since no documented API names one.
- **The sanitiser costs a measurable share of a cascade.** It runs once
  per applied patch, not per frame, but `cascade_deep` is a bench arm.
  *Revealed by* running that arm eight runs a side before and after
  T55.5; if the delta is outside the arm's spread, the sanitiser moves to
  where a `Patch` is built instead of where it is applied.
- **W1003 turns out noisy.** If the shipped W1003 fires on any showcase
  entry, it is wrong or the entry is. *Revealed by* `just widget-drive`
  reading `ui.lint` after each action, which it already does.

## 6. What building found (2026-09-27)

Recorded here rather than by rewriting the tasks above.

- **T55.0 item 1.** The runtime twin "prints `3 false` with the `with`
  case removed" needs a bare `changed` in a format argument, which after
  T55.1 is (rightly) a resolution error. It became a fail case,
  `fail_bind_extra_names_derive_local.rs`, pinning rustc's own E0425 at
  the user's token; the runtime twin names a function `changed` instead.
- **T55.0 item 2.** Nine sentences had no `.stderr`, not five, and the
  first version of the coverage check counted a tenth as pinned only
  because it was a prefix of another sentence; the check matches the
  whole `error:` line. The refusals are seven of the plan's plus two it
  did not list — a repeated `#[reload]` mode (last one won) and
  `StaticTree` attributes on an enum or its variants (read by nothing).
- **T55.0 item 3.** A NaN font size or radius on the *root* is read by
  nothing and passes today; the rows put each length where it is read
  (labels, buttons).
- **T55.0 item 5.** "`min 10, max 0` yields a value inside [0, 10]"
  could not fail: the old answer, 10, is inside it. The row asserts the
  value survives and the bounds are ordered.
- **T55.3.** `textarea.rs`'s "No selection and no IME" was fixed by
  M45/T45.6 before M55 reached it.
- **T55.5, §2.2 and §5.** "Called from `Patch::apply_to`" measured
  +1.8% on `cascade_deep`, outside the spread, so the kill rule moved
  the repair to a per-sheet question and a cold per-node pass. After
  that the clock swung ±10% between builds in both directions, including
  a build with strictly less work measuring slower
  (`docs/results-derive-dx.md` T55.5); the verdict rests on construction.
  A non-finite length takes its field's *type* default,
  `Style::default()`'s (the plan's words): the kind's own defaults cost
  a `fn` call per node and 84 more bytes of firmware. W1012 is reported
  per rule and field, like W1010, rather than per node — the value is
  the rule's.

## Appendix: finding → task

| ID | sev now | status | task |
|---|---|---|---|
| R12-F5 | S3 | OPEN | T55.4 |
| R16-F1 | S2 | OPEN | T55.6 |
| R16-F2 | S3 | OPEN | T55.6 |
| R16-F3 | S3 | OPEN | T55.1 |
| R16-F4 | S3 | OPEN | T55.2 |
| R16-F5 | S3 | OPEN | T55.5 |
| R16-F6 | S3 | OPEN | T55.5 |
| R16-F10 | S3 | WORSE | T55.3 |
| R16-F11 | S4 | OPEN | T55.7 |
| R8-F9 | S4 | OPEN | T55.8 |
| R9-F11 | S4 | OPEN | T55.8 |
| R9-F12 | S4 | OPEN | T55.8 |
