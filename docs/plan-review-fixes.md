# Plan: review fixes — F0–F7, before M7

*2026-09-02. The work queue produced by the seven-agent review of that
date (three plan reviews in the morning; the four-phase full review in
the afternoon: code quality, architecture, security, allocation, tests,
documentation, gates). Every task cites the finding that produced it so
the reason survives the fix. Each task is one commit; each milestone
ends with `just test` green — which, from F0 on, means something it did
not mean before.*

**This plan precedes M7.** `docs/plan-architecture-v2.md` M7–M9 stand as
written but wait: F1 closes live defects, F2 makes the agent usable
during M7's measurement work, F4 changes what M8/M9 build on. F6/F7 can
interleave with M7 once F0–F4 are done.

**Already fixed today, not in this plan:** the `Label<&'static str>`
leak under `#[bind]` (`TextBuffer` split), NaN/hang in the software
renderer, unclipped paint in `Repeat`/`List`/`Dyn`/`Assembled`, the
per-iteration bench harness, the inverted benchmark conclusion, six doc
corrections, the `no-unsafe` pipefail inversion. Those are in `git log`
between `f1cbdc0` and `4441e2b`.

**Sizing.** F0 and F5 are hours. F1, F2, F3, F7 are a day each. F6 is
two. F4 is the large one — three to four — and is the only milestone
here that is a refactor rather than a set of fixes.

---

## F0 — The gate becomes true — **DONE 2026-09-02**

*Phase 4B wrote the files; they sit uncommitted in the working tree. The
gate is red on six known items. This milestone commits the gate and
turns it green by fixing what it flags — not by allowlisting.*

**Gates:** `just test` green on every stage (`static`, `release-shape`,
`dev-shape`, `fixtures`); `.github/workflows/ci.yml` runs it; the
release feature shape (`observe`, `reload` both off) is compiled and
tested by name.

- **F0.1** Commit `justfile`, `ci.yml`, `rustfmt.toml`, the four
  `scripts/check_*.sh` and `scripts/doc-identifiers.allow`, and the
  boundary-anchored `expect` in `fixtures/hot-host` (`"window="` →
  `"window=24"`; `contains` matched `window=0`). *Finding: 3A §3, 4B D.*
- **F0.2** Rewrite the `lumen-static-macros` front-page doc
  (`lib.rs:1-14`), which describes `KIND_COUNT`, `offset_of!` and a kind
  list deleted in M0. Fix `WidgetOps` in `widget.rs:4` and `Pump` in
  `agent/tests/input.rs:41`. *Finding: 3B D6, D14; 4B C.*
- **F0.3** The two dangling doc citations in `plan-agent-parity.md:8-9`
  and the two deleted-bench citations in the `results-*` headers — the
  reproducibility notes already say the binaries are gone; reword the
  headers so they do not read as runnable commands. *3B D18; 4B C.*
- **F0.4** `cargo test -p lumen-static` (release shape) is in `just
  release-shape`; verify it runs 30 tests and that `just dev-shape`'s
  `--workspace` line is the only unified build. Delete the redundant
  `--all-features` line. *3A opening correction.*
- **F0.5** Flip "verifiable in CI" from aspiration to fact, with the
  workflow path. *The claim is in `examples/counter/src/main.rs:5`, not
  the handoff §10 this task first named.* `AGENT.md` gains one sentence
  pointing at this plan.

---

## F1 — Live defects — **DONE 2026-09-02**

*Each is independent, small, and ships with the test that would have
caught it. Order within F1 does not matter; F1.1 and F1.2 first because
they are soundness.*

**Gates:** every task's test fails on the parent commit and passes on
its own; `just test` green; the allocation probe from 2B still reports
zero on the plain-tree path.

- **F1.1** `Dyn::DEPTH = MAX_DEPTH`, not `2`. The comment at
  `hatch.rs:599-600` claims it is "charged the maximum"; it is a
  constant, and `widget.rs:99-102`'s "a tree that could overflow the
  stack does not compile" is false for one type. Test: a `Dyn` holding a
  `MAX_DEPTH`-deep tree fails to compile via the existing assert.
  *3B D2.*
- **F1.2** `fixtures/hot-app/src/lib.rs:113` `fn session` → `unsafe fn`.
  A safe fn manufacturing `&'a mut` from a raw pointer; two calls alias
  and the optimizer collapses them (PoC ran). Eleven call sites gain an
  `unsafe` block each, which is the point. *2A F3.*
- **F1.3** `since.saturating_add(1)` at `journal.rs:190` and
  `audit.rs:220`. Test: `tests/hostile.rs` dispatching `since ∈ {u64::MAX,
  u64::MAX−1, "abc", −1, 1.5, absent}` × `{ui.lastChange, ui.lastDamage,
  app.logs}`, asserting a JSON-RPC error and never a panic; second pass
  after pushing 300 findings so `dropped > 0` reaches the audit site.
  *1A #4, 2A F1/F2, 3A test #3.*
- **F1.4** `Repeat::restore` clamps `item_extent` to a floor and `count`
  to a ceiling before either reaches layout; `repeat.rs:87` uses
  `saturating_add`. Test: `tests/reload_hostile.rs` with hand-written
  JSON — `item_extent ∈ {1e-40, 1e-6, 0, −1, NaN, ∞}` × `count ∈ {0, 1,
  u32::MAX, 1e18}` — asserting `window_len() ≤ CAP` and that
  `take_log()` shows the rejection (the three `log_fallback` arms at
  `reload.rs:145-153` are uncovered today). Doc: handoff §12.4.
  *2A F4, 3A test #5.*
- **F1.5 — DONE** (with F0.3). `#[widget_class(...)]` accepts the documented string form and
  errors on anything else; `#[widget_id(...)]` errors on a bare ident.
  Both currently `let _ =` the parse error. Test: `SUB_TABLE[i].classes`
  asserted from an integration test — the first use of `widget_class`
  in the repo. *1A #3, 3B D7, 4B C (`check_derive_attrs.sh` fires today).*
- **F1.6** `Style.visible` honoured by `paint` and `hit`, via the
  `NodeState` flag that F3 makes the single mechanism — **or**, if F3 is
  not next, the derive's `!visible` early-return zeroes the whole
  subtree the way `hatch::hide()` does, so stale child geometry cannot
  be painted. Test: `tests/visible.rs` quoting `style.rs:94-97` and
  asserting each clause: no `Cmd` from the hidden subtree, `hit_test`
  inside it `None`, parent height as if absent — for a hidden container
  *and* a hidden leaf. `style.rs:94-97` rewritten to match whichever
  ships. *1A #1, 1B #1, 3B D3, 3A test #1.*
- **F1.7** `hot-host` `take_log` uses the two-call size probe and does
  not `mem::take` until the copy succeeds. A >64 KB fallback log
  currently panics *after* draining the diagnostic it exists to keep.
  Doc: `step4-hot-reload.md` §7 sequence. *2A F5.*
- **F1.8** Bounds-check `focus` where it enters `dispatch`
  (`pass.rs:144`), not where it is used. Masked today only because
  `pass.rs:154` drops every non-`PointerUp` event; M9 removes that mask.
  Test: `h.focus = Some(9999); h.pump()` does not panic, and the masking
  is asserted explicitly so removing it is visible. *1A #5, 3A test #9.*
- **F1.9** `results-safe-prototype.md:102,133-139` — the prose still
  says "halves… 32 → 16 B" eight lines below the cell corrected to 48 B.
  At 48 B the flash argument inverts; either re-derive it or strike the
  embedded-flash conclusion and say the `no-unsafe` decision rests on
  verification alone. Same miss at handoff `:707` and plan
  `:113,212`. *3B D1, D12.*

---

## F2 — The agent stops costing more than the frame — **DONE 2026-09-02**

*`frame_done` is 117× the frame it observes at 10k nodes and poisons
every `Label` memo every frame. Under a real shaper (M7) this is the
app's whole shaping cost paid three times. Self-contained in
`lumen-static-agent`.*

**Gates:** `tests/audit_is_inert.rs` — `measure_is_warm` holds across
`frame_done`; allocation gate on `frame_done` = **0** in steady state;
`frame_done` at 10k nodes under 2 ms (generous; fails on 13.4 ms without
flaking); `just test` green.

- **F2.1** The intrinsic probe stops mutating the tree. Either
  `Measured` gains `snapshot()`/`restore()` and `Auditor::natural` wraps
  its two `layout` calls, or — as `audit.rs:235-236` already proposes —
  the probe is retired for reading `Shrink` results, which M2 made
  available. Prefer the second. *1B #3a, 2B #2.*
  *Amended 2026-09-25, M41/T41.4:* the probe went through `Node::probe`
  instead, which fixed the memo and left the rest of the mutation: a
  widget whose `layout` writes state paint reads (`TextInput`'s and
  `TextArea`'s scroll) was still reset by the probe, and under the
  `Boundaries` default the reset reached the screen (R1-F5). Both
  override `probe`; `Node::probe`'s rustdoc states the rule, and
  `crates/lumen-static-agent/tests/audit_is_inert.rs` sweeps every leaf
  kind.
  *Amended 2026-09-26, M41/T41.7:* it did not. The sweep left
  `Pagination` and the strips out, left `RichText`, `Markdown` and
  `NumberInput` empty and unfocused, and probed with the layout's own
  measurer, so it could not see a widget that caches measured widths
  or places; the live shadow oracle found `Pagination` and
  `NumberInput` painting the probe's measurements in the showcase. All
  five override `probe` now, and the sweep seeds content and probes
  with a different measurer.
- **F2.2** `Journal` retains `geom`, `sums` and its `DisplayList` across
  frames and swaps rather than reassigns. Paints **once from the root**,
  recording each node's command range as the traversal goes, instead of
  calling `node(i).paint()` per index (2.1–2.8×N commands, each
  re-hashed). Per-node signature becomes O(own commands). *1B #7, 2B #1.*
- **F2.3** `Auditor::record`'s presence diff becomes O(n) (sort or hash
  on `(code, node)`); `Finding` stores numbers and formats in `lint()`,
  not at construction — 189 legitimately-below-the-fold nodes build and
  drop a `String` + `json!` every frame today. *2B #1.*
- **F2.4** `frame_done`'s doc (`lib.rs:112`) and `journal.rs:19-21` state
  the real cost. *3B D4, D5.* `lib.rs:340`'s "Mirrors
  `Repeat::observe_op`" → the function that exists. *3B D9.*
- **F2.5** `lumen-static-agent` stops hard-enabling `observe` on the
  core. It takes its tree view through a trait the core defines without
  `observe`, and gates the accesskit-shaped parts behind its own
  feature. Verify `examples/counter`'s `binsize` is a release number
  afterwards. Doc: `step5-observability.md`'s zero-cost claim gets the
  condition it was missing. *1B #8, 3B D8.*
- **F2.6** `input.type` caps `text.len()` and returns `INVALID_PARAMS`
  past it. *2A F8.*

---

## F3 — One visibility mechanism, constant defaults — **DONE 2026-09-02**

*Two mechanisms hide today — `Style.visible` (which nothing writes and
paint ignores) and `hatch::hide()` (which zeroes `NodeState`). And
`defaults()` is state-dependent for `Option`/`Either`, but the cascade
runs twice per process, so a `None` at construction styles the slot
`Shrink` forever. 1B's diagnosis: the second problem is a consequence of
the first, and both dissolve if visibility is one flag that `Option`/
`Either`/enum **set** rather than a side effect they produce.*

**Gates:** `Option<Spacer>` toggled `None → Some` at runtime fills its
container (measures 0×0 today); `List<Option<Spacer>>` with `items[0] =
None` styles item 1 correctly; `defaults()` returns the same value for a
kind regardless of instance state; `pass::frame` either has callers or
is deleted.

- **F3.1 — superseded by F1.6.** The subtree-zeroing route was taken instead of a `NodeState` flag: it needed no new per-node field (which would have grown `NodeState` 32 B -> 40 B) and no signature change, because a zero-area node cannot paint or be hit. Original text: `NodeState.flags: HIDDEN`, stamped by `place` from
  `style[i].visible` — the A2 mechanism applied to the existing property.
  `paint`/`hit` read the flag from the array they already carry; the
  derive's early-returns go; `hatch::hide()` becomes "set the flag on
  `NODE_COUNT` slots". Records the M4 T4.1 deviation as reversed. *1B
  #1, #2a.*
- **F3.2** `Option`/`Either`/enum express state through the flag, and
  `defaults()` becomes constant per kind. Test #7 from 3A
  (`heterogeneous_hatches.rs`): per-item `Length` and geometry asserted
  individually across `List<Option<_>>`, `List<Either<_,_>>`, and a
  `Repeat` whose `bind` changes `Length`. *1A #2, 1B #2, 3A test #7.*
- **F3.3** `Host::pump` re-cascades when a cheap shape-changed flag says
  so, or `pass::frame` (the only entry point that cascades and lays out
  together, zero callers) is wired into `Host` and every test that
  hand-rolls `default_styles + cascade + layout` uses it. Pick one and
  delete the other path. *1B #2b, 1A/3A on `pass::frame`.*
- **F3.4** `Patch::visible` stays; it now works. `style.rs:94-97` says
  what the code does. *3B D3.*

---

## F4 — `ArenaHost`: one abstraction for six containers — **DONE 2026-09-02** (F4.4 partly; see there)

*Six containers keep off-table state and none share a shape. The cost
is five hand-written special-case families: `Handlers` decoders ×5,
observe-id bit layouts ×3 plus a copy in the agent mirroring a deleted
function, `ensure_style` sites ×4 none cascaded, a journal that
collapses a repeater window to one checksum, and author ids inside
templates dead for both styling and selectors. Plus the enum derive's
two allocations per frame, which exist because its arena is not
retained. This is the refactor; everything in it is one root cause.*

**Gates:** allocation gate **zero on every path including the enum
derive**; a stylesheet rule reaches a `Repeat` template row; `#save`
inside a template resolves via the agent; `ui.lastChange` reports the
specific row that changed, not "the repeat node"; `Repeat::observe`
emits no row outside the viewport; the five decoder/id/ensure families
are deleted, not wrapped; `crossover` and `repeater` benches within
noise of the F3 baseline.

- **F4.1** The trait, in `lumen-static`, storage-only:
  `Arena { node_count, states, style }` with `ensure(items, probe: &dyn
  Node)`, `slice(k)`, `parts()` — keyed on a runtime count so `Dyn` can
  implement it. `ensure` is where F3.2's per-item probing lives once.
  Placement stays per-container: `Repeat` is a window, `List` stacks,
  `Either` picks, `Dyn` has one, `Assembled` rebases into real indices.
  *1A Tier 3, 1B #4.*
- **F4.2** `Repeat`, `List`, `Either`, `Dyn` on the arena. `Assembled`
  becomes a `Node` (it is not one today; its inherent
  `layout`/`paint`/`hit` cannot be swapped into any shape the others
  share) and implements it too. *1A Tier 3 caveat, 1B #4.*
- **F4.3** The enum derive retains its arena. Paint, hit and `Handlers`
  stop being no-ops; `tests/hatches.rs:216-222`, which pins the paint
  hole as intended, flips. The two per-frame allocations go. *1B #6, 2B
  #3.*
- **F4.4 — PARTLY DONE 2026-09-02.** Two of the three landed and the
  third is deferred with a reason.
  - **Done:** the synthetic id layout is defined once, in
    `observe::instance_id`. It had four hand-written copies — three
    containers plus the agent crate, the last under a comment citing a
    `Repeat` method deleted in M0 — and nothing checked they agreed.
    When they disagree, `ui.probe` and `ui.getTree` return different ids
    for the same node, the failure the agent's docs call impossible.
    Pinned by `probe_and_the_snapshot_agree_on_a_repeater_row_id`.
  - **Done:** the false `W1006`. `collect` walks the table and every
    off-table container carries `CONTAINER`, so nothing inside a
    `Repeat`/`Either`/`List`/`Dyn` was examined and a frame full of
    content reported itself blank — the false positive this module's own
    docs call "the first thing an agent ever sees". `List` escaped only
    by accident, via `stats()`. The lint now declines to claim blankness
    when the walk could not see inside something with area, and still
    fires in both directions (`tests/off_table_visibility.rs`).
  - **Deferred: per-row `ui.lastChange` and `ui.lastDamage`.** The
    journal's arrays are table-indexed, so an arena instance has no
    index to key on; reporting per-row needs a second, arena-keyed store
    *and* a change to the wire format (`{node, row}` rather than
    `{node}`). That is a protocol decision, not a refactor, and it
    belongs with the agent-parity work rather than a fix pass. Today a
    change inside a repeater is reported as "the repeat node changed",
    which is true and coarse. Recorded here so it is a known limit
    rather than a surprise.
- **F4.5** `cascade` recurses into arenas with a synthesized `NodeDesc`
  slice. Rules reach templates. Doc: `layout-and-storage-decisions.md
  :263-268`'s "known limit" is closed, not extended to four widgets.
  *1B #5, 3B D11.*
- **F4.6** `Repeat`'s window becomes a ring (`row % window.len()`) so one
  row scrolling in touches one slot, and `bind` is skipped when
  `first`/`count` are unchanged. `Label::set_str` absorbs
  `set_if_changed` — compare-then-write is the only sane default for a
  memoized field. 62 measures per idle frame → 0; 62 per one-row scroll
  → 2. Closes B10 of the scrolling decisions. *2B #4.*
- **F4.7** `Repeat::observe` filters to the viewport. *rust-pro #4, 1B.*
- **F4.8** Containment property test (3A #2): for every `Cmd` after
  paint, its rect lies inside its nearest enclosing `PushClip`, over a
  ~40-tree corpus of `{Repeat, List, Dyn, Either, Assembled, column} ×
  {scroll 0, 12, 0.5, last} × {viewport taller, shorter, exact}`. This
  generalizes `lint.rs:192`'s guard off the table and onto the display
  list, where escaping constructs are actually visible. Update
  `verification-notes.md:47-51`, which still states the superseded
  invariant.

---

## F5 — The compile-time ceiling — **DONE 2026-09-02** (F5.4 measured and rejected)

*`table_is_well_formed` is O(N²) at const-eval: 3.2k nodes → 20 s, 10k →
`long_running_const_eval`, fails to compile. The only finding today that
limits what the framework can express. Small, independent, and worth
doing before anyone builds a large tree.*

**Gates:** a 3.2k-node fixture compiles in under 2 s; a 10k-node fixture
compiles at all; `widget.rs:142-171`'s every `return false` is covered;
`docs/results-const-eval.md` records before/after.

- **F5.1** One forward pass. The table is DFS pre-order with
  `first_child == n+1`; the O(N) form already exists at
  `equivalence.rs:28-34`. *2B bonus.*
- **F5.2** `tests/table_invariants.rs`: hand-built malformed tables
  (empty, root with parent, child before parent, `child_count`
  mismatch, `first_child ≠ n+1`) each assert `false`. A mutation making
  the function return `true` passes the whole suite today. *3A test #8.*
- **F5.3** A compile-budget fixture in `experiments/` promoted into the
  workspace (it is excluded today, which is why the cost went
  unmeasured) with a `just` recipe asserting a wall-clock budget — a
  budget, not a comparison, since the failure mode is a hard compiler
  limit. *4B E.*
- **F5.4** `node(i)` via a derive-emitted const `CHILD_STARTS` array and
  binary search; five loop-over-it callers, one per frame. Do after
  F2.2 has removed the per-frame one. *1B #7, 2B #5.*

---

## F6 — Surface and hygiene — **DONE 2026-09-02** (F6.6 declined; see the commit)

*Nothing here is a defect. Each is a trap for the next reader or the
next user, and each is cheap.*

**Gates:** `cargo doc` renders an entry point a user can start from;
`ops`'s apparent surface is the widget vocabulary; the trybuild suite
exists and pins the 23 properties 1A enumerated; `just test` green.

- **F6.1** `#[doc(hidden)] pub mod __private` for the eleven
  derive-only callees in `ops` (`spaced`, `inner_limits`, `loose_child`,
  `fill_child`, `align_offset`, `record`, …) and `widget::splice_child`
  / `table_is_well_formed`. `pub mod prelude` with the everyday names.
  Delete `VirtualColumn`, `fold`, `fold_pub`, `paint_container`; one
  private mixer in `display.rs` for its two hand-rolled copies. *1A
  #7, #9; 1B #11; 3B §4.*
- **F6.2** Macro hygiene: drop the emitted `use` block and fully
  qualify; rename the handler generic so a widget named `S` compiles.
  Reject generic/lifetime parameters on `StaticTree`/`Reloadable` with
  one error. `leaf_common!` either emits `SUB_TABLE` from `$flags` or
  loses both parameters — as written it is a decoy invariant. *1A #6,
  #8, #13.*
- **F6.3** `Measured` takes `&mut self`; the `Cell` is a vestige of the
  flat passes and costs `Sync` on every tree with a label. *1A #12.*
- **F6.4** `Button` gains the `TextStorage` parameter `Label` has, so
  `Button::restore` has a non-leaking path; until then, document the
  leak the way `text.rs` does. *1A #13, 2A F7.*
- **F6.5** Rename one `Host`. The agent's trait and the shell's struct
  share a name and nothing implements the former outside a test mock;
  the counter reaches into three `pub` fields to rebuild what
  `AgentHost::tree()` exists to return. Either the trait moves to the
  core, or a bridge crate, or a feature-gated impl in the shell. *1B #8.*
- **F6.6** `App::update` leaves `pass::dispatch`'s trait. It is a
  frame-loop hook the core never calls; the "impossible to forget"
  guarantee holds only for shell users, and `hot-app:177` proves it by
  hand-calling it. *1B #8.*
- **F6.7** `lumen_static::reload::export_reloadable_app!($ty)` emits the
  six `extern "C"` shims; symbol names as `pub const`; the loader side
  references them. Today the ABI is duplicated byte-string literals
  across two fixtures, and the two sides already implement the size
  probe differently. *1B #9.*
- **F6.8** `crates/lumen-static-macros/tests/ui.rs` with trybuild: the
  18 `compile_error!` cases the derive already produces, the 5
  type-level ones (`table_is_well_formed`, `DEPTH`, `Button<Other>`,
  `#[reload(keep)]` on a fn pointer, the typed `let f:` bindings), and
  `pass/` cases asserted at runtime. Un-`ignore` the two doctests. *1A
  trybuild §, 3A test #6.*
- **F6.9** `crates/lumen-static-shell/tests/alloc_gate.rs` — its own
  binary, counting allocator, five warmup frames, assert **zero** for
  plain / hatch / scrolling / `frame_done` (after F2) / enum (after
  F4.3). `docs/results-allocation.md` with the table. *3A §6, 4B E.*
- **F6.10** `loop_test.rs` strengthened in place (exact focus node, exact
  resize width, per-band ink); `equivalence.rs` compares the whole
  `Vec<(Size, Point)>` it builds instead of four scalars — its module
  doc claims per-node position equality and the code checks none. *3A
  §2.*

---

## F7 — Documentation — **DONE 2026-09-02**

*Twenty drifts found; 13 in rustdoc, which the doc-currency rule does not
name. Six corrected this morning, four more by F0–F2. This is the rest,
plus the three documents that do not exist.*

**Gates:** `just static` green with an empty allowlist delta;
`docs/README.md` lists all 21 files with class and status; a new reader
can go from clone to a rendered widget using only `README.md` and the
crate doc.

- **F7.1** `results-step6-fuse.md` gets a supersession header in the
  style of `results-step2-crossover.md`'s, citing
  `results-traversal.md:128-132`. The only wholly stale doc with no
  marker. *3B D13.*
- **F7.2** Root `README.md`: two sentences, the counter as the snippet,
  the four `just` recipes, the crate map, two pointers. *3B §5.1.*
- **F7.3** "How to write a widget" as the `lumen-static` crate-level
  doc: `Node` vs `Widget` (and that `use lumen_static::Node` is needed
  to call `.node(i)`), the seven attributes across three derives in one
  list, the two `kind_name` conventions, the hatch cascade limit until
  F4.5 closes it. *3B §5.2, 1A #10.*
- **F7.4** `docs/README.md`: a status table — file, question it answers,
  class (load-bearing / historical / superseded-by), reproducible, last
  verified. Replaces six competing marker conventions with one place to
  look. *3B §3.*
- **F7.5** Remaining drifts: `step3-repeaters.md`'s `RefCell` and
  flat-phase claims; `plan-lumen-parity.md`'s line counts dated;
  `verification-notes.md` 13 vs 17; `layout-and-storage-decisions.md
  :161-165`'s duplicated paragraph; three intra-doc links; `kind_name`
  conventions documented. *3B D15–D20.*
- **F7.6** `AGENT.md` clause 1 extended to "and any doc comment on the
  code you changed" — the difference between a rule that addresses 2 of
  20 findings and one that addresses 15. Plus one sentence: a rule a
  commit establishes lands in a doc or a comment; the body keeps the
  story. Ten pieces of load-bearing rationale exist only in commit
  bodies today. *3B §2, §7.*
- **F7.7** Results docs gain a machine header (CPU, governor, toolchain,
  date). `bindings-decision.md`'s note about a content-keyed shaper
  cache goes into T7.3's specification. *4B E, 2A.*

---

## Deferred, with the reason

- **`Either` merged table (T8.0)** — reopened, not re-decided; it
  reverses a shipped shape and carries a binsize trade. F4's arena makes
  the current shape cheaper either way. Stays scheduled first in M8.
- **`Style` interning to `u16`** (2B #6: 55% of live heap) — real, but a
  layout-data-model change; it belongs with the microcontroller profile
  work, not a fix pass.
- **`ui.explain`, WebSocket transport, authorization** — the security
  review's qualification that every connected agent is root becomes a
  decision the day a socket exists (E5 of `plan-events.md`).
- **`cargo fmt` in the gate** — red at 63 hunks even with the config 4B
  added; turning it on is a decision to accept a reformat, not a CI
  setting.
- **Benchmark numbers in CI** — run for compile coverage and archived,
  never asserted. Runner variance exceeds any regression worth catching.
  The gate lives on the integers (allocations, bytes, compile budget).

---

## Cross-cutting rules, in addition to `plan-architecture-v2.md`'s five

6. **A finding's test lands in the same commit as its fix**, and is
   shown failing on the parent. Coverage of the table is not coverage of
   the tree: any test of a hatch drives it through the display list or
   the pixel band, not `SUB_TABLE`.
7. **Numbers are stated once.** In the results doc; cited everywhere
   else. A doc comment that must carry a number carries the citation in
   the same sentence. Today's D1 — a cell corrected, the sentence eight
   lines below it missed — is what the alternative looks like.
8. **`just static` runs before `cargo` does.** A dead identifier should
   not cost a build to find.

## Order

```
F0 ──► F1 ──► F2 ──► F3 ──► F5 ──► F4 ──► F6 ──► F7
                            (F5 is independent; slot it wherever a
                             short task fits — it must precede any
                             tree over ~1k nodes)
```

F0 first because it makes the rest verifiable. F1 before anything else
because they are live. F2 before M7's measurement work needs a
trustworthy agent. F3 before F4 because F4's `ensure` is where F3's
constant `defaults()` lands. F4 before M8 because scroll regions inside
arenas and focus for off-table nodes (C5) both build on it. F6 and F7
interleave with M7 from then on.
