# Plan: architecture v2 — recursive, safe, single-pass

*2026-09-01. The work queue. Consolidates every decision taken since the
§10 build order completed, into ordered milestones with acceptance
gates. Each task is one commit (AGENT.md); each milestone ends with the
full gate (`just test`) green and the benchmarks re-run.*

**Decisions this plan implements** (each has its own record):

| decision | record |
|---|---|
| safe accessor path, no `unsafe` in release | `docs/results-safe-prototype.md`, `docs/layout-and-storage-decisions.md` §0 |
| recursive descent traversal, `&dyn` children | `docs/results-traversal.md` |
| iced `Length` model, single pass, no flexbox, no taffy | `docs/layout-and-storage-decisions.md` §2 |
| style in a parallel `[Style; N]` array | `docs/layout-and-storage-decisions.md` §3 |
| text as a widget field, `Label<S: AsRef<str>>` + `Dirty` | `docs/layout-and-storage-decisions.md` §1 |
| §6 escape hatches, build order 4a–4e | `docs/layout-and-storage-decisions.md` §4 |
| `fn`-pointer handlers, data at the async seam | `docs/events-decision.md` |
| parity phases A–E | `docs/plan-lumen-parity.md` |

**What stays.** The const table (slimmed) for index addressing — the
agent surface, journal and audit are index-keyed by design; `NODE_COUNT`
and the compile-time firewall (Step 1); the repeater's scaling property;
hot reload (`Reloadable`, the swap sequence, the fixture); the agent
protocol P0–P4; `#[fuse]` as a dial; every benchmark as a regression
gate.

**What goes.** `WidgetOps`, `KIND_LIST`, `splice_kinds`, `NodeDesc.kind`
and `.offset`, the three flat passes and their partitioned variant,
`base + offset` casts (all 13), `Repeat`'s `RefCell`, the `Inline` trait
(fusion becomes a call-style flag), the `safe` prototype module (merged
into the core), the `Acc`-era leftovers.

---

## M0 — Foundation pivot: recursive safe traversal — **DONE 2026-09-01**

*Landed as one commit (a trait-level refactor has no individually
compiling pieces). Gates met: 25 suites, clippy clean both configs, hot
reload PASS, `just no-unsafe` finds zero. Measured: crossover 50 ns /
4.10 µs / 438 µs at 10 / 1k / 100k, against 74 ns / 6.47 µs / 690 µs on
the flat table, and 47% faster than the naive-dyn control (was 16%).*

*Two deltas worth carrying forward:*
- *`Widget` had to split into `Node` (object-safe methods) and
  `Widget: Node` (consts), since associated consts are not
  dyn-compatible.*
- ***Fusion is now nearly free either way** — 429 µs vs 435 µs at 100k,
  where it was 2.8× on the flat table. With recursion the only thing
  `#[fuse]` removes is the indirect call, which the compiler often
  devirtualizes anyway. `#[fuse]`'s purpose narrows to code-size and
  embedded control; re-price it before investing further (§6 of this
  plan).*

*Replaces the flat table passes with recursive descent over trait
objects. Everything else depends on this. Port the four leaves, the
repeater, assembly, fusion, events and the agent surface, then delete the
old path. No behavior change is intended: every existing test keeps
passing on the new path before the old one is removed.*

**Gates:** all 28 suites green; `crossover`, `safety`, `repeater`,
`fusion`, `memory` re-run and recorded; `miri-smoke` runs clean; zero
`unsafe` in `crates/lumen-static/src` with default features (`grep`
gate added to `just test`); hot reload demo PASS.

### T0.1 — the `Widget` trait, unified

One trait replaces `Widget` (consts) + `WidgetOps` (fn pointers) +
prototype `Node`/`Tree`/`RNode`:

```rust
pub trait Widget {
    const NODE_COUNT: usize;
    const DEPTH: usize;                 // T0.4
    const IS_RIGID: bool;
    const SUB_TABLE: &'static [NodeDesc];   // slim, agent-facing only

    fn layout(&mut self, limits: Constraints, st: &mut [NodeState], i: usize) -> Size;
    fn place(&self, at: Point, st: &mut [NodeState], i: usize);
    fn paint(&self, st: &[NodeState], i: usize, out: &mut u64);
    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit>;
    fn node(&self, i: usize) -> &dyn Widget;      // index accessor (agent)
    #[cfg(feature = "observe")]
    fn observe(&self, st: &[NodeState], i: usize, id: u64, node: &mut accesskit::Node,
               extra: &mut Vec<(NodeId, accesskit::Node)>) {}
    #[cfg(feature = "observe")]
    fn stats(&self, out: &mut NodeStats) {}
}
```

Decisions inside the task:
- `layout` takes `&mut self` — each node is visited exactly once, so
  exclusive access is natural, and it is what lets `Repeat` mutate its
  window without `RefCell` (T0.6). `place`/`paint`/`hit` take `&self`.
- `observe`/`stats` are default methods gated *in the trait definition*
  (inside `lumen-static`), so derive output stays cfg-free — the rule
  from `docs/step5-observability.md`, re-learned in
  `docs/results-traversal.md`.
- `NodeState` unchanged (32 B). `NodeDesc` slims in T0.3.
- Children are called through `&dyn Widget` (the `&dyn` coercion the
  prototype used); `#[widget(fuse)]` emits the same body with static
  calls (T0.8).

### T0.2 — derive: recursive impls + slim table + accessor

`StaticTree` emits, per container: `layout`/`place`/`paint`/`hit`
looping its fields (the prototype's `RNode` output), `node(i)` as the
range-delegating match, `SUB_TABLE` via `splice_child` **without** byte
offsets or kind rebasing, and provenance/`widget_id` stamping unchanged.
`SafeTree` is deleted (its output is now the only output).

### T0.3 — `NodeDesc` slim

Drop `kind` and `offset` (24 B → 16 B *(before M1's `wid`/`classes`; 56 B → 48 B today, same 16 B saving — `docs/results-safe-prototype.md` F1.9)*). `splice_child` loses two
parameters. `concat_table` keeps its signature minus `stride`. The
`memory` benchmark's `SlimDesc` projection becomes the measurement.

### T0.4 — depth cap

`const DEPTH: usize = 1 + max(children's DEPTH)`, computed by the
derive; `const _: () = assert!(DEPTH <= lumen_static::MAX_DEPTH)` emitted
with every table. `MAX_DEPTH = 64` initially. This is §5's stack-depth
fear turned into a compile-time fact — a tree that could overflow does
not compile. Recorded in `docs/results-traversal.md`'s cost list as
closed.

### T0.5 — leaves ported

`Label`, `Fixed`, `Spacer`, `Button<S>` implement the unified trait
(from the prototype's `RNode` impls). No `unsafe` remains in `ops.rs`.

### T0.6 — `Repeat<T>` ported, `RefCell` removed

`layout(&mut self)` materializes and binds the window directly.
`hit`/`paint`/`observe` walk `self.window`. `instance_state`/`window_len`
keep their signatures (tests depend on them). The row-stable synthetic
id scheme is unchanged. `Reloadable for Repeat` unchanged.

### T0.7 — runtime assembly

`Assembled<C>` becomes the prototype's `RSlab<C>`: `Vec<C>` under a
`VirtualColumn`, `concat_table` for the agent index. `base()` and the
stride arithmetic go.

### T0.8 — fusion as a call-style flag

`#[widget(..., fuse)]` emits the identical container body with
`self.field.layout(...)` (static) instead of the `&dyn` coercion. The
`Inline` trait and `fuse.rs` are deleted; `fused_geometry_matches_unfused`
keeps passing by construction. The `fusion` benchmark keeps its arms.

### T0.9 — events on the new path

- `trait App { type Tree: Widget + Handlers<Self>; fn tree(&self) -> &Self::Tree;
  fn tree_mut(&mut self) -> &mut Self::Tree; }` — the root contract that
  replaces the `(state: *mut u8, base: *const u8)` pair.
- `trait Handlers<S> { fn handler(&self, i: usize) -> Option<fn(&mut S, &EventCtx)>; }`
  The derive emits `impl<S> Handlers<S> for #name where #(#field_ty:
  Handlers<S>),*`, delegating by range like `node(i)`. Leaves are
  `Handlers<S>` for all `S` (returning `None`); `Button<A>` is
  `Handlers<A>` only. A `Button<Other>` inside an `App`'s tree is a
  **type error** — the state-type contract becomes a compile-time check
  instead of a safety comment.
- `pass::dispatch` → `event::dispatch<S: App>(state: &mut S, focus, ev)`:
  hit-test through `tree().hit(...)`, copy the handler out via
  `tree().handler(i)` (the borrow ends), call it with `&mut S`. The
  copy-before-`&mut` sequence is now enforced by the borrow checker
  rather than documented.
- `Repeat` routes to template handlers with `ctx.row` as before.

### T0.10 — agent surface on the new path

`TreeRef` becomes `&dyn Widget` + `&[NodeState]` + the slim table.
`hit_test` calls `tree.hit`. `observe::snapshot` calls
`tree.node(i).observe(...)`. Journal per-node checksums call
`tree.node(i).paint(...)`. Audit's intrinsic probe calls
`tree.node(i).layout(...)` under the two probe constraints — noting
`layout` now takes `&mut self`, so the probe uses `node_mut(i)` (add to
the trait) and a scratch copy, or the probe is replaced by the `Shrink`
intrinsic pass once M2 lands (preferred; record which).

### T0.11 — delete the old path

`pass.rs` flat loops, `layout_partitioned`, `WidgetOps`, `KIND_LIST`,
`splice_kinds`, `desc::Inner`, the `safe` module (merged), `binsize`'s
unsafe arm. `safety` benchmark keeps `rdyn_opaque` and a fusion arm;
its "unsafe" arm is retired with its last number recorded. Update
`miri-smoke` and the `justfile` `unsafe` grep gate.

### T0.12 — doc currency for M0

Handoff §5 SUPERSEDED pointer; `step2-design-notes.md` §2 (dispatch
rule) superseded; `verification-notes.md` "present state" updated (no
`unsafe`, recursion favourable); `results-traversal.md` cost list closed.

---

## M1 — Style array — **DONE 2026-09-01**

*Before `Length`, because `Length` has to live somewhere.*

**Gates:** `ui.lastChange` reports style diffs; all suites; `crossover`
unchanged within noise (style reads add one array index per node).

- **T1.1** `Style { width: Length, height: Length, padding: Edges,
  spacing: f32, align: Align, … }`, `Default`, 32–48 B. `StyleState =
  [Style; NODE_COUNT]`, a parallel array like `NodeState`; passes take
  `&[Style]`.
- **T1.2** Static selector inputs in `NodeDesc`: `wid` promoted out of
  the `observe` gate; `classes: &'static [&'static str]` added via
  `#[widget_class("a", "b")]`. Slim table grows to 24 B again — recorded,
  accepted (it is what selectors match on).
- **T1.3** `Rule { selector, style-delta }` and a programmatic
  `Stylesheet`. `cascade(tree, sheet, &mut styles)`: one walk, specificity
  = (id, class, kind). `.lss` parsing is Phase C (`plan-lumen-parity.md`).
- **T1.4** Derive: `#[widget(spacing = 4.0)]` becomes a default rule
  emitted into `DEFAULT_RULES`; the attribute is deprecated and removed at
  the end of M2. Closes `step2-design-notes.md` §8's deviation.
- **T1.5** Journal diffs `StyleState` alongside `NodeState`; findings
  classify `style` as a third change kind.
- **T1.6** Reload: `StyleState` is derived state — recomputed after a
  swap, never migrated. Assert in the reload demo.

**Outcome.** All six landed as written. `Style` is 40 B; the parallel
array is built by `pass::default_styles` and re-derived after a reload
rather than migrated. `crossover` moved 122 → 129 ns at N=1 and stayed
inside noise at N=100k, which is the one-index-per-node cost the gate
predicted. `#[widget(spacing = …)]` survived M2 after all: it lowers to a
`DEFAULT_RULES` entry, so the attribute is sugar for a rule rather than a
second layout input, and removing it would only cost authors ergonomics.

---

## M2 — `Length` layout, single pass — **DONE 2026-09-01**

**Gates:** the dyn oracle in `benches` is rewritten to the same model and
equivalence holds; `crossover` re-run; the audit's `W1005` test (text
squeezed) still fires via `Shrink` under a tight parent; a new toolbar
test pins the worked example from the decision record.

- **T2.1** `Length { Fixed(f32), Shrink, Fill, FillPortion(u16) }` per
  axis, read from `Style`.
- **T2.2** Container `layout`: iced's resolve — scan 1 lays out non-`Fill`
  children (`Fixed` with tight limits, `Shrink` with the parent's loose
  limits), accumulating used space; scan 2 divides the remainder among
  `Fill`/`FillPortion` children and lays them out. Cross axis: `Fill`
  stretches, `Shrink` takes content. Padding and spacing applied. Still
  one `layout` call per child.
- **T2.3** Leaves: `Label` defaults `Shrink`×`Shrink`, `Spacer`
  `Fill`×`Fill`, `Fixed` `Fixed`×`Fixed`, `Button` `Shrink`.
- **T2.4** Alignment (`Start`/`Center`/`End`) in `place`.
- **T2.5** Audit: retire the two-probe intrinsic trick in favour of
  reading `Shrink` results; `W1005` becomes "a `Shrink` node whose parent
  gave it less than it asked for". Update
  `docs/plan-agent-parity.md` P3 notes.
- **T2.6** `hit.rs`'s last-containing-node argument still holds
  (children never escape parents under `Length` either — `Fill` gets
  exactly the remainder, `Shrink` is clamped by loose limits). Re-pin the
  invariant test with `Length` shapes.

**Outcome, with two corrections to the plan as written.**

1. `Fill` is not a separate variant. `Length` is `Fixed | Shrink |
   FillPortion(u16)` and `Length::FILL` is `FillPortion(1)` — the same
   three-way decision at each child with one fewer case to match, since
   `Fill` was only ever `FillPortion(1)`.
2. **Cross-axis `Fill` conforms, it does not define.** T2.2 said "cross
   axis: `Fill` stretches" without saying against *what*. Stretching a
   cross-axis `Fill` child against its own measured size makes it define
   the container it is supposed to fill: a `Fill` spacer in a `Shrink`
   row inflated the row to the viewport. The rule the implementation
   settles on is that cross-fill children are excluded from the cross
   accumulator during layout and stretched to the resolved cross extent
   at `place` time. Recorded in `docs/layout-and-storage-decisions.md`
   §2; the toolbar test pins it.

The dyn oracle in `benches` was rewritten to the same model, so
equivalence stayed a real check rather than a tautology; several test
expectations that encoded the old equal-division model were updated with
comments naming the new semantics. `W1005` re-derives from `Shrink`
results as planned. Crossover after M2: 129 ns / 9.0 µs / 882 µs static
vs 117 ns / 12.4 µs dyn.

---

## M3 — Text storage — **DONE 2026-09-01**

**Gates:** `Label<String>` round-trips through hot reload with no
`Box::leak`; measure cache hit/miss counted in a test; `no_std`-shaped
`Label<&'static str>` still the default.

- **T3.1** `Dirty<T>`: `Deref`, a setter that marks dirty, `take_dirty()`.
- **T3.2** `Label<S: AsRef<str> = &'static str> { text: Dirty<S>, cache: Cell<Size> }`;
  `layout` re-measures only when dirty or when limits changed (store the
  limits the cache was computed under).
- **T3.3** `Reloadable for Label<S>` where `S: From<String> + AsRef<str>`
  — `String` and `Cow` restore without leaking; `&'static str` keeps the
  documented dev-only leak.
- **T3.4** Fixture `hot-app`: `Session.counter` reaches the screen through
  an explicit `update` step, closing the gap named in
  `docs/plan-agent-parity.md`'s appendix.

**Outcome.** Landed as written, with the generic named `TextStorage`
rather than a bare `AsRef<str>` bound: measurement caching needs
`from_string` to restore after a reload, and a bare `AsRef<str>` cannot
provide it. `String` and `Cow` own their text; `&'static str` keeps the
documented dev-only leak, so the default stays `no_std`-shaped. The cache
is keyed on (dirty flag, limits), which is why the text lives in the
widget rather than behind a getter — a getter has nothing stable to key
on (`docs/layout-and-storage-decisions.md` §1).

---

## M4 — §6 escape hatches — **DONE 2026-09-01**

**Gates:** each hatch has a firing and a quiet test; `NODE_COUNT` stays
a compile-time constant in every case (asserted); agent ids for inactive
subtrees are stable across toggles.

- **T4.1 (4a)** `NodeState.flags: HIDDEN`. Hidden subtrees: zero size in
  `layout`, skipped by `place`/`paint`/`hit`; `observe` marks them
  `hidden`.
- **T4.2 (4b)** `impl<W: Widget> Widget for Option<W>` — `NODE_COUNT =
  W::NODE_COUNT`, `None` hides. `Either<A, B>` — `A + B`, inactive branch
  hidden, both retain state.
- **T4.3 (4c)** `List<W>`: `Vec<W>` fully materialized, sharing `Repeat`'s
  arena code; `Repeat` keeps virtualization.
- **T4.4 (4d)** `#[derive(StaticTree)]` on enums: `NODE_COUNT = 1 +
  sum(variants)` so every variant's nodes keep stable indices; `node(i)`
  matches the discriminant; inactive variants hidden. This is the task
  the accessor path unblocked.
- **T4.5 (4e)** `Dyn`: one node owning a runtime-built sub-table and
  state slice, a generalization of `Repeat` with count 1 and a boxed
  template.

**Outcome, with three deviations recorded rather than papered over.**

0. **T4.1 put hiding in `Style.visible`, not `NodeState.flags: HIDDEN`.**
   M1's style array had landed by then and visibility is styleable. Not
   recorded at the time; found during the 2026-09-02 review. It turned
   out to matter: it is the precedent A2 of the scrolling decisions first
   cited and then had to distinguish itself from, because hiding has a
   geometric proxy (zero size) and clipping does not.
1. ~~**`Either<A, B>` is one table node, not `A + B`**~~ — **CLOSED
   2026-09-04 (T8.0): the merged padded table shipped.** Kept for the
   story, which is that the recorded reason was false. The reason recorded here was that a merged const table
   "needs `generic_const_exprs`". Review disproved it with a compiling
   counter-example on stable: `SUB_TABLE` is a slice, so the length is a
   value, and a padded const array plus `split_at` builds the merged
   table with a compile-time bound. The shipped shape stands until
   re-decided; it costs tree-absolute agent ids and four heap `Vec`s per
   `Either`. Options, evidence and a recommendation in
   `docs/either-reopened.md`; the re-decision is task T8.0 so it lands
   before any scroll region is built on top of the current shape.
2. ~~**The enum derive cannot paint variant content.**~~ **Closed
   2026-09-02 (F4.3).** The limit was real and its stated cause was the
   fix: a variant's state lived in a scratch arena allocated per `layout`
   call, so there was no retained geometry to paint against — and two
   heap allocations per enum node per frame, in framework-generated code,
   against the zero-allocation constraint.
   `NODE_COUNT` is now `1 + sum(variants)`, which is what handoff §6
   specified in the first place: every variant has its own range in the
   main table, so there is no arena at all. Paint, hit and `Handlers` are
   real, inactive variants are hidden rather than stale, and a node's
   index means the same thing whichever variant shows. The test that
   pinned the limit as intended behaviour now pins the capability.

Every hatch has a firing and a quiet test, and `NODE_COUNT` is asserted
constant in each.

---

## M5 — Phase A: make it real — **DONE 2026-09-01** (except the winit binding)

*`docs/plan-lumen-parity.md` Phase A, unchanged except that layout is
now single-pass and the seams are the trait methods above.*

- **T5.1** Adopt `lumen-render`: `paint(..., out: &mut DisplayList)`
  replaces the `u64` checksum; journal/audit checksums hash the list.
- **T5.2** Adopt `lumen-text`: `LayoutCx` (concrete struct, `dyn` inside)
  threaded through `layout`; `Label::layout` shapes for real, cache from
  T3.2 keyed by (text dirty, limits, font).
- **T5.3** winit shell: window → `InputQueue` → `event::dispatch` →
  layout → paint → present via `lumen-render`'s CPU backend first. Implements
  the agent `Host` trait so P0–P4 work against the live window.
- **T5.4** `counter` example on screen. This is the milestone the project
  has not had yet.

**Outcome — done to the OS seam, and stopped there deliberately.**

T5.1 and T5.2 landed as their own `DisplayList` and `LayoutCx` rather
than by adopting Lumen's `lumen-render`/`lumen-text` crates. The seam is
what the plan actually needed: `paint` writes commands instead of a
checksum, and `LayoutCx` carries a `dyn TextMeasure` inside a concrete
struct so no type parameter goes viral through every widget signature.
Adopting the real crates is now a swap behind those two traits, with no
widget signature affected — which is the property that made the seam
worth building first.

T5.3 stops one step short of a window. `crates/lumen-static-shell` is
everything between an application and the OS *except* the OS binding:
`Host` owns the tree, both parallel arrays, the input queue, focus and
the frame's display list; `pump()` drains input, lays out, paints, times
both phases; `render()` hands the list to a `&mut dyn Renderer`. A winit
front end attaches at exactly two points — window events into
`Host::post`, `Canvas::pixels` blitted to the surface. It is not written,
because a window cannot be gated headlessly and every other subsystem
here earned its acceptance test. Seven shell tests pin the loop instead.

T5.4 runs: `examples/counter` clicks its own buttons *by author id*
through the agent's `ui.getLayout` — the same path an agent driving the
app takes, so the example doubles as an agent-surface test — and renders
to a PPM at 7.6% ink. `just counter` writes a viewable frame.

**Remaining in M5:** the winit binding, and adopting Lumen's actual
render/text crates. Both are Phase A follow-ons, not blockers for M6 —
**now scheduled as M7**, since M8's text input depends on the text stack.

---

## M6 — Derived presentation (`#[bind]`) — **DONE 2026-09-01**

*Not in the original plan. Added when the counter example made the cost
of a hand-written update step concrete: `docs/bindings-decision.md`.*

**Gates:** the counter drives one pump per click with no `update` call;
the hot-reload demo still asserts `status='count 3'` after the swap, now
from a binding; the binding's per-frame cost is measured, not assumed.

- **T6.1** `App::update` (empty default) called by `pump` between input
  drain and layout — the loop owns *when* derived state refreshes.
- **T6.2** `bind::format_into` / `set_if_changed`, and
  `TextStorage::set_from_str` so accepting a value reuses the storage's
  allocation. Change detection compares the formatted *output*: no bit to
  forget, no setter to bypass, no stored state.
- **T6.3** `#[derive(App)]` — `#[tree]`, `#[bind(t, "fmt {}")]`,
  `#[bind(t, with = f)]`, `#[app(after_update = f)]`. Emits the whole
  `App` impl so nothing about the update step is hand-written.
- **T6.4** Counter and `hot-app` converted; `benches --bin bindings`.

**Outcome, corrected 2026-09-02.** The mechanism works and the
ergonomics land: the counter's click loop halved and its `update` method
is gone. The first version of this paragraph then reported that the
output-compare "costs ~4–13 ns per binding to save ~2 ns — a net loss".
Review showed the harness could not resolve that question (its own floor
was 10–27 ns), the range was one quantity sampled five times, and the
cost belonged to the thread-local scratch buffer rather than the
comparison. Re-measured batched with the format actually running: the
comparison is a wash (±0.5 ns), the scratch is 3–14 ns, and 1000
bindings cost 25 µs. The ~13 ns threshold is withdrawn.
`docs/bindings-decision.md` keeps the full account, including how the
wrong number was produced. Two defects found by the same review — a leak
on the default `Label` storage under `#[bind]`, and an un-amortized
reentrant path — are recorded there; the leak is fixed (compile error),
the reentrancy is noted.

---

> **Inserted 2026-09-02:** the seven-agent review of that date produced
> `docs/plan-review-fixes.md` — milestones **F0–F7**, which run *before*
> M7. F4 (`ArenaHost`) changes what M8/M9 build on; F2 makes the agent
> usable during M7's measurement work. M7–M9 below stand as written and
> wait.

## M7 — Phase A completion: a window, real glyphs, real pixels

*The remainder of `plan-lumen-parity.md` Phase A, promoted from a
four-step sketch to a milestone because it is next and because M8 depends
on it. `docs/plan-lumen-parity.md` Phase A steps 2–4.*

**Gates, corrected 2026-09-02.** The first version required "the counter
runs in a window and accepts real keyboard and pointer input", which
cannot be met: keyboard routing does not exist and is M9's work (Cluster
C of `docs/text-and-scrolling-decisions.md`). That made M7 depend on the
milestone that depends on it. Split as review recommended:

- **M7a (T7.1–T7.3), headlessly gated:** the software `Canvas` remains
  the oracle and every existing pixel test passes against the adopted
  renderer; `benches --bin bindings` runs against the real measurer and
  T7.3's four numbers are recorded.
- **M7b (T7.4–T7.5), manually gated:** the counter runs in a window and
  responds to *pointer* input. Keyboard is not a gate here.

**M7a status 2026-09-04: DONE.** T7.1 and T7.2 were re-decided rather
than executed — adopting Lumen's crates means adopting `lumen-core`,
measured at 74 and 84 transitive crates against this project's 6
(`docs/text-render-adoption-decision.md`). The interface is adopted, not
the crates: the fontdue stack and the software `Canvas` sit behind
`dyn TextMeasure` and `Renderer`, and T7.2b extended the former with the
shaping method M8/M9 need. T7.3 ran and its four numbers are in
`docs/bindings-decision.md`.

- **T7.1 — SUPERSEDED 2026-09-04.** Adopt `lumen-render` behind the existing `Renderer` trait. The
  seam is already the right shape, so no widget signature changes. **The
  adopted backend must honour `PushClip`/`PopClip`** — the software
  renderer ignored them for three milestones and only a pixel-level test
  caught it (`docs/text-and-scrolling-decisions.md`).
- **T7.2 — SUPERSEDED 2026-09-04.** Adopt `lumen-text` behind `dyn TextMeasure`. Unblocks M8's
  `Shaped` cache, which cannot be designed against `BlockMetrics`. (The
  unblocking happened; the crate did not. See T7.2b.)
- **T7.3 — DONE 2026-09-04.** Four numbers in `docs/bindings-decision.md`:
  hit 4.8 ns, miss 2.46 µs short / 16.7 µs wrapped. The memo stays by
  three orders of magnitude. Measure what the memo guards, against the real shaper. The
  first version of this task was a threshold gate on `format_into`'s
  comparison; review showed the comparison has no cost to gate
  (`docs/bindings-decision.md`), so the gate was aimed at the wrong
  quantity. What is actually unmeasured is the memo's value. Specified
  so it can be executed rather than argued: with `lumen-text` installed
  and its own cache warmed by ≥100 prior calls, batched ≥10k iterations,
  measure `Label::layout` on a memo hit and a memo miss for (a)
  `"count: 41"` unwrapped and (b) a 60-char label wrapped at 200 px.
  Record all four numbers in `docs/bindings-decision.md`. Expected: miss
  − hit in the hundreds of nanoseconds (shaper cache hit) to microseconds
  (cold shape). If it is under ~1 ns the memo is pointless and
  `Measured` should go; nothing about `format_into` changes either way.
  The bench must be taught to install the real measurer — re-running it
  as-is measures `BlockMetrics` again.

  **One trap to name before running it.** A shaper with its own
  content-keyed cache — cosmic-text's `ShapeRunCache`, for instance —
  makes a memo *miss* cost a hash lookup rather than a shape. Measure
  against a cold shaper and the answer is microseconds; measure against
  its warm path and it is hundreds of nanoseconds. Both are true and
  they differ by ~10x, so the number recorded must say which. It also
  means `layout-and-storage-decisions.md` §1's "re-shaping is what the
  cache exists to avoid" stops being the whole story once a real shaper
  lands: the framework memo will no longer be the only thing between a
  label and a shape.
- **T7.2b — DONE 2026-09-04.** `TextMeasure::shape(text, size, weight,
  max_w, &mut dyn GlyphSink) -> Size` plus `line_height`. Reports
  `Cluster { byte, x, line }` — one per character plus the trailing
  insertion point, byte offsets into the *caller's* string even when a
  glyph was substituted. A sink rather than a returned buffer because
  B9 put the buffer in the storage and because `shape` must not
  allocate; `NoSink` and `Vec<Cluster>` ship. Six tests in
  `shell/tests/shaping.rs`. Original text:
  `TextMeasure` gains a shaping method. `dyn TextMeasure` today
  returns a bare `Size`; M8's `Shaped` cache needs per-glyph advances,
  so the seam T7.2 adopts behind is too narrow for T7.2's own stated
  purpose. The signature is an interface decision and gets written down
  before `lumen-text` is wired, not discovered during it.
- **T7.4 — DONE 2026-09-02.** `examples/counter-window`: winit events
  into `Host::post`, `Canvas::pixels` blitted to a softbuffer surface.
  The claim M5 stopped on held — the binding is two functions,
  `window_event` and `redraw`, and nothing above the seam changed to
  accommodate it. The example ships three modes so the parts that *can*
  be gated are: `--headless <path> [count]` writes a frame, `--drive`
  runs it through the agent surface and asserts, and no argument opens
  the window.

  Two things came with it that are not the binding and are worth naming.
  `Style` gained `color`, `background`, `radius` and `font_size`, and
  those resolve into `NodeState` during layout — the A2-revised
  mechanism, since `paint` has no `&[Style]`. And the software renderer
  grew a real text path behind a `truetype` feature (`fontdue`), because
  `BlockMetrics` draws one filled square per glyph: correct for proving
  the pipeline, and the reason a window first showed a row of black
  boxes rather than a counter. The stand-in remains the default, so
  every test still runs on a deterministic measurer.

  **Live agent endpoint — DONE 2026-09-03.** The window serves the
  agent protocol on `LUMEN_AGENT_ADDR` (default `127.0.0.1:9230`),
  newline-delimited JSON-RPC over TCP, Lumen's transport. Requests
  cross to the event-loop thread through an `EventLoopProxy` and are
  answered after `pump`, so a reply already reflects the input it
  carried. `ui.screenshot` (parity plan P5) came with it — the frame
  the window blits, PNG-encoded in the agent crate. What this closed:
  the first live window was verified by a desktop screenshot tool,
  which needs an unlocked session and cannot be automated; now
  `scripts/agent_client.py` drives and looks at the window from a shell,
  and `.claude/skills/verifying-apps` records the ladder. The example
  also gained the rest of Lumen's counter look — `font_weight` and
  `shadow` in `Style`, a bold face in the `FontStack`, cross-face glyph
  fallback so U+2212 is a minus rather than tofu, and advance-based
  measurement (ink extent truncated "POSITIVE" to "POSITIV").
- **T7.5 — DONE 2026-09-04.** `examples/counter-window --features a11y`
  creates an `accesskit_winit::Adapter` before the window is shown and
  pushes `observe::snapshot_focused` from `redraw`, beside
  `frame_done` — so the screen reader's tree, the agent's journal and
  the pixels are always the same frame. An `ActionRequested` becomes a
  click at the node's centre rather than a direct handler call, so
  assistive input takes the one input path the whole design rests on
  (`docs/events-decision.md`) and shows up in the journal like any
  other click.

  Three things came with it. `accesskit` 0.24 → 0.25 (what the adapter
  requires; `Tree` is now `TreeInfo`). `observe::snapshot` reported
  `focus: NodeId(1)` unconditionally — the root, which a reader
  interprets as "nothing focused" — so `snapshot_focused` takes the
  host's focus and `index_of_node` maps an adapter's id back to a table
  index, rejecting 0, out-of-range and overflow (four tests in
  `tests/observe.rs`; a stale focus after a hot reload must not name a
  node that is not in the tree).

  **Default-off**, and that is the interesting part: on Linux the
  adapter pulls `accesskit_unix` and a D-Bus/async stack, measured
  **95 → 157 crates** for the example. `just test` builds this example
  on every run. The window itself, and every gate, still builds from
  95. `just tally-a11y` runs it; verified against `scripts/ui_check.py
  all` with the feature on.
---

## M8 — Scrolling — **DONE 2026-09-04**

`ScrollView<W>`: one node plus its content's subtree, both on a merged
const table (the technique T8.0 introduced). Content is measured against
an unbounded main axis so it can exceed the viewport, placed at a
clamped offset, and clipped when painted and when hit. Ten tests in
`crates/lumen-static/tests/scroll_view.rs`, including the A5 NaN repro
and a scroller inside an `Either` branch — the composition T8.0 was
sequenced ahead of this milestone for.

What the tasks became:

- **A5** needed two numbers, not one. The first attempt passed the
  viewport unchanged, reasoning that `Shrink` returns content size; but
  `resolve_self` ends with `limits.clamp(...)`, so a finite maximum
  squeezes the content and nothing scrolls. The `Fill` reference went
  into `LayoutCx` (`with_fill_span`/`fill_extent`) rather than into
  `Constraints`, which 69 struct literals construct by hand.
- **A4's feared false positive did not materialize**, and structurally
  rather than by a special case: because the scroller loosens the main
  axis, nothing inside is squeezed, so `W1005` has nothing to report.
  Pinned by a test that would fail if the axis were bounded again.
- **A7 became a lint, not a panic** (`W1008`), and so did the f32 scroll
  ceiling (`W1009`) — see below. `Node::virtualizes()` carries the
  signal ungated.
- **The f32 ceiling has a number**, after two deferrals:
  `Repeat::MAX_ADDRESSABLE_EXTENT = 2^24`, which is **699,050 rows at
  24 px**. This repository's own repeater bench used 1,000,000 at 24 px
  — past the ceiling, where some rows have no scroll offset that
  selects them.

Original plan:



Design in `docs/text-and-scrolling-decisions.md` Cluster A, all of it
decided (A2 revised after review; A5–A7 settled 2026-09-04): `Fill`
inside a scroller means the viewport, never the content, with a
NaN-guard test; the offset is clamped in `layout` after measuring; a
`Repeat` inside a `ScrollView` is forbidden and linted — a `Repeat` *is*
a scroller.

Depends on nothing in M7 — true again under the *revised* A2, which
stamps a `NodeState` flag from `place` instead of changing `paint`/`hit`
signatures. The first version of this note made the same claim on a
false premise.

**T8.0 — DONE 2026-09-04.** `Either` rebuilt on the merged padded table
(`docs/either-reopened.md`, Option A): `NODE_COUNT = 1 + A + B`, table
built by `EitherTable::ARRAY` in const-eval, four `Vec`s per `Either`
gone, branch internals addressed by tree-absolute index. The agent crate
needed no change, which is the clearest evidence its `Either` special
case was a workaround. It also exposed a hole open since M4:
`observe::emit_table` announced absent subtrees — an empty `Option`'s
nodes, now also a hidden branch's — to the screen reader; zero-area
nodes are marked `set_hidden()` now.

Two notes for whoever writes the breakdown:

- **A4 is the trap.** `W1005` fires on any `Shrink` node given less than
  it asked for, which inside a scroller is the normal case — it must
  learn about clip scopes in the same commit that introduces one, or it
  will scream on every scrolling view.
- **Fold in the f32 scroll ceiling** from `docs/step3-repeaters.md`, and
  give it a number: `repeat.rs:165` puts `count * item_extent` into
  `size.h` and `repeat.rs:85` divides in f32 to pick `first`; at 10M ×
  24px one ulp is 16px and rows are skipped before the offset quantizes.
  It has been deferred twice; a third deferral needs a stated reason.

---

## M8b — Incremental raster — **DONE 2026-09-04**

All four tasks landed; both T8b.3 gates pass with margin (frame p95
0.154 ms against 0.2, input → presented p95 1.38 ms against 3). Numbers
and what the profile found in `docs/results-frame-cost.md`. Two things
the plan below did not anticipate:

- **The damage source is the display list, not the node table.** The
  plan said "move the geometry+checksum diff out of the agent crate".
  That diff re-paints every leaf into a scratch list to sign each node,
  because the agent's question is *which node* changed. The renderer's
  question is *which pixels*, and comparing this frame's commands
  against last frame's answers it with no per-node bookkeeping, no
  second paint, and no change to `Node::paint`. The journal keeps its
  own mechanism; the cheaper one is on the release path.
- **Damage is banked per present, not per pump.** The surface is
  multi-buffered: what softbuffer hands back may be zeroed or hold the
  frame from two presents ago, and it says which through the buffer's
  age. Drawing one frame's difference onto a two-frame-old base is the
  classic double-buffered smear. So `Retention::take_damage_for_age`
  keeps a short ring, and damage *accumulates* across the several pumps
  one frame may take — the tally re-cascades and re-lays-out when the
  sign flips, and both pumps land on one screen.

Original plan:



The frame is 1.05 ms and 0.9 ms of it is rasterising pixels that did
not change: every redraw clears the canvas, walks the whole display
list (shadow included) and blits the whole surface
(`docs/results-frame-cost.md`). Layout and paint are 1 µs. This
milestone is the handoff's stage 5 — "damage rects only" — and it comes
before dirty-scoped layout (M8c) because it is where the time is.

The damage rect already exists. The agent's journal diffs per-node
geometry and paint checksums every frame and answers `ui.lastDamage`
from them; the renderer just never hears about it. M8b is mostly
plumbing that answer to where it is useful.

**T8b.0 — decide how retention is switched off.** See "Retention is a
type parameter" below. Settled here so T8b.1 does not grow a `cfg`.

**T8b.1 — a damage rect from the frame diff, in the core.** Move the
geometry+checksum diff out of the agent crate into `lumen-static`
(`retain::Frame`: previous `Geom` and paint hash per node, 24 B/node)
so a release build has it; the journal keeps its ring of coalesced
changes on top. The diff's cost is already measured: part of the 46 µs
audit at 901 nodes, zero allocations (`docs/results-agent-cost.md`).
Gate: `ui.lastDamage` answers identically before and after.

**T8b.2 — render only what the rect touches.** `Canvas::render_within
(list, damage)`: push the damage rect as the outermost clip, skip any
command whose rect (a shadow's, expanded by `spread`) misses it. Every
command that *does* intersect is repainted in order, clipped — so the
card's shadow is redrawn where it lies under a changed label, and only
there. That is ordinary compositing, and it is why a per-node retained
display list is not needed for correctness. Gate: the frame after a
click is pixel-identical to a full re-render (`tests/`: render both
ways for the tally at every count from −12 to 12, compare buffers).

**T8b.3 — present only the rect.** `softbuffer::Buffer::
present_with_damage` (0.4.8 has it). Blit only the rect's rows.
Gate: `just live-gate` — the tally's click frame under **0.2 ms** p95
(today 2.3 ms), and `input → presented` p95 under 3 ms.

**T8b.4 — no damage, no *raster*. CORRECTED 2026-09-04.** The task as
written said "presents nothing", and that is wrong: `RedrawRequested` is
also X11's expose, which means the display server lost the content —
something the application's damage cannot know. Implemented literally,
it put another window's pixels on screen. The frame is always presented;
only the raster is skipped. Gate: `app.perf` shows `render_ns` 0 for a
no-op frame, **and `ui_check.py presented` passes**
(`docs/results-frame-cost.md`).

### Retention is a type parameter, not a feature flag (T8b.0)

The question was whether the retained machinery — dirty bits, the
previous frame's geometry and checksums, damage — should be switchable
off for very small apps that would rather rebuild every frame.

Yes, and the reason is not memory. The numbers: `NodeState` is 72 B,
`NodeDesc` 48 B, `Style` 64 B per node today; retention adds ~24 B per
node plus a dirty bitset. For the tally (14 nodes) that is 336 bytes
next to a 915 KB pixel buffer; for a 1,000-node list it is 24 KB next
to 184 KB. Nobody's RAM is decided by it. What retention costs a small
app is **correctness surface**: incremental layout and partial raster
are exactly the code that produces stale pixels and off-by-one damage
rects, and "rebuild everything" is trivially correct. An embedded
target that renders a dozen nodes should not have to trust it.

So: `Host<A, R: Retention = Incremental>`, with two implementations —

- `Rebuild`: zero-sized state, `mark_dirty` is a no-op, `damage()` is
  the viewport, every frame lays out and rasterises everything. This
  is today's behaviour and it stays exactly as fast as it is.
- `Incremental`: the dirty bitset and `retain::Frame`; scoped layout
  (M8c) and partial raster (M8b).

Not a cargo feature, for two reasons. Features unify across a
workspace: a dependency that wants `Incremental` would switch it on for
an app that chose `Rebuild`, and there would be no way to run both in
one process. And the two must run side by side, because **`Rebuild` is
the oracle**: the gate for every task in M8b and M8c is "`Incremental`
produces the same `NodeState` array and the same pixels as `Rebuild`
for the same input sequence". A feature flag would make that test
impossible to write. The type parameter is one generic on `Host`,
monomorphised at most twice, and the derive output never mentions it
(cross-cutting rule 2).

---

## M8c — Dirty-scoped layout and style — **T8c.4 DONE, T8c.1–3 DEFERRED 2026-09-04**

Measuring first inverted this milestone (`docs/results-cascade-cost.md`).
It was planned as a layout optimisation with the scoped cascade as a
rider that "comes free". In fact a **full stylesheet swap costs 37.8 µs
at 901 nodes — four times a whole layout and paint (9.3 µs)** — and it
is the operation an app performs at runtime. Scoping it to the changed
subtree is 16.6× cheaper and provably equivalent, because every
`Selector` is node-local and a subtree is a contiguous range of a
pre-order table. `Host::restyle_subtree(sheet, "card")` ships;
`cascade_subtree` checks the caller's half of the contract against a
full cascade in debug builds, and caught a real misuse on its first run.

**T8c.1–T8c.3 are deferred, with the reason recorded.** The layout win
is 9.3 µs at 1k nodes (~6% of the tally's post-M8b frame) because the
expensive half of layout — text measurement — is already memoized per
label. And skipping a clean subtree is *unsound* while widget fields are
public: a dirty bit is only correct if every mutation sets it, and this
repository mutates `badge.w`, `list.count` and `repeat.scroll` directly
in fixtures and tests. The prerequisite is encapsulation, not
invalidation, and that is an M0-class change nothing measured yet
justifies. T8c.1/T8c.2 would have no consumer without T8c.3.

Original plan:



The handoff's property 1: "a mutation sets a dirty flag that propagates
up only until it reaches a relayout boundary". The table already holds
everything the property needs — parent indices in `SUB_TABLE`, a
`RIGID` flag stamped from `IS_RIGID`, and subtrees as contiguous index
ranges — and nothing consumes any of it. Whole-tree layout runs every
frame: 1 µs at 14 nodes, 7.3 µs at 901. This milestone pays off at
list scale, which is why it follows M8b rather than leading it.

**T8c.1 — the bound target's index, from the derive.** `#[bind(ui.card.
value, ..)]` resolves a field path; each derive already emits per-field
`STARTS`, so the target's table index is a `const` sum the `App` derive
can write into the generated `update()`: when `set_str` reports a
change, `R::mark_dirty(index)`. This is the only piece that needs the
macro, and it is one line of generated code per binding. Widgets
mutated directly (`Repeat::scroll`, a handler writing `label.set`)
mark themselves through `Dirty<T>`, which T3.1 built for this.

**T8c.2 — bubble to the boundary.** `while i != NO_PARENT && !table[i]
.is_rigid() { dirty.set(i); i = table[i].parent }`. Five lines over the
table; no macro. **DECIDED 2026-09-04: rigidity is per axis.** The
tally's card is `Fixed(360)` wide and `Shrink` tall, so a taller value
label must bubble to the page while a wider one must not. `IS_RIGID`
becomes `RIGID_W`/`RIGID_H`, the derive ANDs each separately, and
`NodeDesc` carries two bits; a dirty bit bubbles past a node on the
axis it is not rigid in. Gate: `tests/`: a `Dirty` deep in a `Ten10` fixture
marks O(depth) nodes, never O(N); the count is asserted.

**T8c.3 — containers skip clean children.** In the derive's generated
`layout`, a child whose subtree is clean and whose constraints are
unchanged keeps its `NodeState` (size and stamped attributes); the
parent still runs its own placement arithmetic over all children's
sizes, which is O(children), not O(subtree). `Repeat`/`List`/`Dyn`
treat a dirty window as a dirty subtree. Gate: `Incremental` and
`Rebuild` produce identical `NodeState` arrays for the M8b click
corpus and for a scrolled `Ten10`; the 901-node click frame's layout
under **200 ns** p95 (today 7.3 µs).

**T8c.4 — scoped cascade.** A class toggle re-cascades one subtree —
the index range `[start, start + NODE_COUNT)` — instead of the table.
This is the tally's sign flip, and it is the one style operation an
app does at runtime; a full `set_stylesheet` stays whole-table. Styles
resolve into `NodeState` during layout, so a scoped cascade dirties the
range and T8c.3 does the rest: there is no separate "style pass" to
scope. Gate: the range is asserted equal to the subtree's
`NODE_COUNT` for every hatch kind (`Either`'s reopened shape,
`docs/either-reopened.md`, decides what the range means for a
variant).

---

## M9 — Input routing and text input — **C1–C4, C6 DONE 2026-09-04**

The routing layer both halves depend on exists now.

- **C1/C2 — every event routes.** `dispatch` returned for anything but
  `PointerUp`, so `input.type` and `input.scroll` were enqueued, drained
  and dropped. It now runs two passes over the ancestor chain:
  widget-level (`Node::on_event`), then application activation.
  `EventCtx::event` carries what happened.
- **C3 — `Node::on_event(&mut self, ev, cx) -> bool`.** The seam a
  widget edits *itself* through. `Handlers` carries the application's
  `fn(&mut S, &EventCtx)` and cannot reach widget state, because from
  `&mut S` a widget is only reachable as `&mut dyn Node` — which is why
  a scroller could not consume a wheel however well events were routed.
- **C4 — chaining falls out of C3's `bool`.** A scroller at the end of
  its travel declines and `dispatch` keeps walking up, which is the
  whole of "nested scrollers, at my end take it". No separate mechanism.
- **C6 — pressed and hovered.** `NodeState` carries both; `dispatch`
  sets them; `record_painted` resolves `Style::pressed_dim`/`hover_dim`
  into the painted background, the same A2-revised mechanism the colours
  use. `PointerUp` clears the bit on *every* node, so a press dragged
  off and released does not stay lit. `scripts/ui_check.py press` checks
  it live, which closes the one GUI-theory rule that was previously
  unverifiable (`docs/results-frame-cost.md`).

**Two design points worth keeping.** Activation stayed pointer-up (plus
**Enter on the focused node**, which is what makes a `Button` reachable
without a pointer): every handler written so far is `|c, _| c.count +=
1` and ignores the ctx, so routing arbitrary keys to `Handlers` would
have made every keystroke press every button. And `PointerMove` is
deliberately not routed — it fires continuously and its only consumer is
the hover bit, so routing it would walk the chain on every mouse motion
looking for something that is almost never there.

**Cluster B, 2026-09-04.** `TextInput<S: TextBuffer>` ships: the widget
owns the buffer (B1), edits go through methods (B3), the caret is an
ordinary `Cmd::Rect` drawn only when `NodeState::focused` (B4),
caret-into-view happens in `layout` after shaping (B5), the app reads
`field.as_str()` (B8). No IME (B6) and no multi-line (B7) — and **no
reserved fields for them**, because an unused field is a claim the code
has not earned. B9 became `TextBuffer::Clusters`, an associated type, so
a fixed-capacity storage pairs with a fixed-capacity cluster array; B11
emptied `TextStorage` down to `AsRef<str> + Default` and moved
`from_string` to a `reload`-gated `TextRestore`. B10 was **already
closed** by F4.6's ring plus the comparing setter.

**No selection, and that is a routing fact, not an omission.** A
selection needs modifier state on `Event::Key` (Shift+Arrow) or pointer
drag (`PointerMove` is deliberately unrouted, C6). Both are named
changes to the event vocabulary; neither is worth making for a field
nobody has asked to select in. The anchor is *not* stored.

**C5 is part-built.** Hidden focus is dropped and Tab walks the
focusables in declaration order, wrapping. Focus as a `NodeSel` is not
built: T8.0 removed its main motivation — `Either` branches are ordinary
table nodes now — leaving only `Repeat` rows, where no focusable widget
currently lives.

Original plan:



Cluster C plus Cluster B of the decisions doc, every question decided
(C1–C5 and B9–B11 settled 2026-09-04 on their recommendations). Split
from M8 because the two share the containment decision and nothing else
— different subsystems, different gates, different risk — and because
Cluster C is larger than a task: `dispatch` drops every event that is
not `PointerUp`, `EventCtx` carries no payload, and a handler reaches
widget state through `Node::on_event(&mut self, ev, cx) -> bool` (C3,
option ii). That change is M0's class and gets its own task with its
own gate before any text field is attempted.

Depends on M7a for the text half only: `TextMeasure` needs a shaping
method (T7.2b) before B2's `Shaped` cache has anything to hold. B10
(`Repeat::bind` defeating the memo) must land before B2 regardless.

Order within: C1–C2 (routing and payload, small) → C3 (the trait change)
→ C5 (focus) → B10 → B11 → B3/B2/B9 → the widget → C4 (chaining, once
two scrollers exist to chain).

**Added 2026-09-03 — C6, pressed and hovered state.** `NodeState` has no
pressed/hovered bit and `dispatch` tracks neither, so a button gives no
feedback while held: the one GUI-theory check `scripts/ui_check.py`
cannot run (`docs/results-frame-cost.md`, "not fixed"). Rides with C1–C2:
the routing pass that learns `PointerDown`/`PointerMove` sets the bits,
the cascade gets `:pressed`/`:hover`-class selectors, and the check
becomes "the frame after `PointerDown` differs at the target and the
frame after `PointerUp` restores it".

---

## M10 — The embedded profile — **PLANNED 2026-09-04, not started**

*Replaces "parity phases B–E", which was written on 2026-09-01 against
numbers that no longer hold. See "Why not parity" below. **Task
breakdown: `docs/plan-embedded.md`.***

The architecture's stated reason for existing is a profile Lumen cannot
serve: "zero allocation for the entire UI — not few, zero … no allocator
required" (handoff §"Performance expectations"). That claim appears in
three documents and is **demonstrated by nothing**.
`shell/tests/alloc_gate.rs` proves no allocation *in steady state on a
desktop with an allocator present*, which is a different and much weaker
statement. M10 makes the claim true, gated, and measured — or finds out
where it is false.

It comes before more widgets for the reason `docs/plan-lumen-parity.md`
gives for its own gating decisions: **every widget encodes the answer.**
`TextBuffer::Clusters` is already an associated type precisely so a
fixed-capacity storage can pair with a fixed-capacity cluster array
(B9). Writing eight more widgets and *then* discovering they all assume
`Vec` is the same mistake one level up.

### What is already true, measured 2026-09-04

- The core is **9 `std::` references** from compiling without `std`:
  `RefCell` (bind), `VecDeque` (event), `Cow` (text), a `thread_local!`
  (reload, already feature-gated) and `mpsc` (inbox, **not** gated).
- **The alloc-free path already exists and is used by 233 tests.**
  `pass::layout`/`paint`/`hit_test` take *slices*; the tests own
  `[NodeState; Ui::NODE_COUNT]` and `[Style; Ui::NODE_COUNT]` directly,
  because `NODE_COUNT` is a const at the concrete type. `Host` is the
  convenience wrapper that allocates, not the architecture.
- What genuinely needs an allocator divides cleanly: the **escape
  hatches** (`List`, `Dyn`, `Assembled`, and the arenas behind them) and
  the **display list**. The purely static path — `Label`, `Button`,
  `Fixed`, `Spacer`, `Repeat`, `ScrollView`, `TextInput` — has none.

That last point is the shape of the answer: *the static core is
allocation-free; the escape hatches are what cost you an allocator.*
M10's job is to make that sentence checkable rather than plausible.

### T10.0 — how paint reaches pixels without a list — **DECIDED**

The blocker is `DisplayList { cmds: Vec<Cmd>, text: String }`, which
`Node::paint` writes into. Two shapes were considered:

- **A fixed-capacity list**, `DisplayList<const N: usize>`. The const is
  viral: it reaches every `paint` signature, every hatch and the derive.
- **Paint into a sink.** `Node::paint(&self, st, i, out: &mut dyn
  Painter)`, where `Painter` is `DisplayList`'s existing builder surface
  — `rect`, `rounded_rect`, `text_weighted`, `shadow`, `push_clip`,
  `pop_clip`. `DisplayList` implements it by appending and interning
  text; an embedded host implements it by rasterising immediately.

**Decided: the sink.** It is the shape this project has now chosen three
times for the same reason — `dyn TextMeasure` (T5.2), `dyn GlyphSink`
(T7.2b), `dyn Renderer` — namely that the *caller* should own the
storage. It removes the whole list on embedded rather than just its
allocation, it is one parameter type across **16 `paint` sites** rather
than a viral const, and it composes with M8b: `Rebuild` retention needs
no retained list, so embedded is sink + `Rebuild` + no damage ring + no
allocation, while desktop keeps `Incremental` and its list unchanged.

The one thing the sink cannot carry is `Cmd::Text`'s `(offset, len)`
range into `DisplayList::text`; the trait therefore takes `&str`
directly, and `DisplayList`'s impl does the interning it already does.

**T10.1 — `no_std + alloc`, and the feature polarity.** Point the nine
references at `core`/`alloc`, gate `inbox` (it is `mpsc`, and deferred
work off the frame loop is not an embedded concern), and declare
`#![cfg_attr(not(feature = "std"), no_std)]` with `extern crate alloc`.

**DECIDED: the features are `std` and `alloc`, both default-on. There is
no `no_std` feature.** The obvious spelling — a `no_std` feature and
`#[cfg(not(feature = "no_std"))]` on everything else — inverts the one
rule Cargo features have: **they are additive, and unified across a
build.** A `no_std` feature *subtracts*, so one dependent enabling it
would silently strip `std` from every other crate in the graph that
never asked. This project has already been bitten by exactly that
unification — the justfile records that `--workspace` cannot produce the
shipped shape "because lumen-static-agent, examples/counter and
fixtures/hot-app turn on `observe`/`reload` and cargo unifies features
across the selection". A subtractive feature turns that annoyance into a
build that compiles and is missing things.

So: `default = ["std"]`, `std = ["alloc"]`, `alloc = []`. Adding a
feature only ever adds. An embedded consumer writes
`default-features = false`, or `features = ["alloc"]` for the middle
tier — the same three-tier shape `serde` and `hashbrown` use, and for
the same reason.

Gate: `cargo check -p lumen-static --no-default-features --target
thumbv7em-none-eabihf`. The target is available through rustup and
nothing else in the workspace needs to build for it.

**T10.2 — the sink (T10.0), and an alloc-free static path.** After the
sink lands, an app that uses only the static widgets, owns its own state
and style arrays, and paints into an immediate rasteriser should touch
no allocator at all. Gate: a fixture that builds for `thumbv7em` with
`#[global_allocator]` set to a stub that **panics on any allocation**,
lays out and paints a small tree, and returns a checksum. An allocation
is then a link-time-visible abort, not a number to interpret.

**T10.3 — name the price of the hatches.** `List`, `Dyn`, `Assembled`
and `Arena` behind the default-on `alloc` feature from T10.1. Not a demotion:
the point is to make "what does dynamism cost me" answerable in the
type system rather than in prose. `Either`, `Option` and `Repeat` stay
in the alloc-free core — `Either` because T8.0 removed its arenas, and
`Repeat` because its window is a fixed capacity the caller chooses.
Open: whether `Repeat`'s arena can be caller-supplied, which is the
same question `TextBuffer::Clusters` already answered for text.

**T10.4 — the number.** `docs/results-embedded.md`: `.text` and `.bss`
for the fixture at `thumbv7em-none-eabihf`, and the peak RAM its state
and style arrays occupy for a stated node count, against a stated
threshold. A profile with no measured footprint is a claim, and this
file is what turns M10's premise into a result the gate can refuse.

---

## M11 — Widget vocabulary — **PLANNED 2026-09-04, not started**

*Was Phase B of `docs/plan-lumen-parity.md`. Follows M10 because every
widget encodes the allocation answer, and three of these items reopen
decisions M9 closed deliberately. **Task breakdown:
`docs/plan-widgets.md`.***

A third of Phase B's list already exists — text input, scrollable, and
the escape hatches as first-class widgets. What is left, and what each
actually costs:

- **checkbox / radio / switch.** One toggle core, three presentations.
  Cheap: they are `Button` with a bool and a different `paint`. The
  first widgets that will exercise `NodeState::pressed` for something
  other than a shade.
- **slider.** Needs **pointer drag**, which M9 deliberately does not
  route: `PointerMove` is unrouted because it fires continuously and its
  only consumer is the hover bit (C6). A slider is the first thing that
  makes routing it worth the cost, and it must come with the measurement
  that says so.
- **text area.** Reopens **B7** (multi-line was decided out of the first
  cut). The shaping seam already reports `Cluster::line`, so the
  groundwork is there; the caret arithmetic and `line_height` scrolling
  are not.
- **tabs.** Mostly `Either` plus a header strip. T8.0 made this cheap by
  putting branch nodes on the table.
- **dropdown / tooltip / modal.** These need **overlay and z-order**,
  which the layout model has no concept of — containers place children
  sequentially by measured size, which is also why `W1004` (sibling
  overlap) is a *reserved* lint code that "ships with the first layout
  mode that can produce it". That reserved code is the marker for this
  work; whoever builds overlays makes it real.

**Selection and IME** belong here too, and both are blocked on the event
vocabulary rather than on widget code: selection needs modifier state on
`Event::Key` or pointer drag; IME needs a preedit event and a
composition seam. Named in `docs/text-and-scrolling-decisions.md` B6 and
the `input.rs` module doc, not silently deferred.

---

## M12 — The test harness — **PLANNED 2026-09-04, not started**

*Was Phase E. Last not because it is least valuable, but because it is
the only one whose substrate is already finished — so it stays cheap
however long it waits. **Task breakdown: `docs/plan-harness.md`.***

What exists: one input queue, sessions that replay by construction, 233
tests, a pixel-exact oracle (`shell/tests/damage_oracle.rs`), a live
gate with seven checks, and the whole agent protocol. What is missing is
**ergonomics and artifacts**, not mechanism.

- **A locator/expect API** over the existing selectors, so a test reads
  `app.locator("#save").click()` and `expect(...).to_have_text("Saved")`
  rather than assembling JSON-RPC by hand. Lumen's `lumen-test` is 931
  lines and this is the part worth copying — it is architecture-neutral,
  which the crates that were declined were not.
- **Golden images with a diff artifact.** The comparison exists
  (`damage_oracle.rs` compares whole pixel buffers); what is missing is
  recording, updating and a red-over-dimmed diff to `Read` on failure.
- **Session recording as a test artifact.** The invariant already holds
  — every synthesized input goes through the one queue — but nothing
  writes the recording out, so "an agent session replays as a test" is
  true and unexercised. This is the task that makes it exercised, and it
  is the reason `app.setValue` was refused.

---

### Why not parity — **DECIDED 2026-09-04**

`docs/plan-lumen-parity.md` called itself "a survey, not a commitment",
and two of its load-bearing figures have since gone:

- **"About 10k is adoptable"** is false. `lumen-render` and `lumen-text`
  were measured and declined (`docs/text-render-adoption-decision.md`);
  `lumen-style` was checked on 2026-09-04 and depends on `lumen-core`,
  `lumen-layout` *and* `lumen-text`, so it is in the same position.
  Parity now means writing ~35–40k lines of someone else's library
  ourselves, with nothing inherited.
- **"~4,300 lines, 4 widgets"** is stale by 2.5×: 11,101 lines and
  eleven widget types, with a third of Phase B's list already built.

So parity stops being a destination. Widgets get built when an
application here needs one — which is how `ScrollView` and `TextInput`
arrived — and `plan-lumen-parity.md` stays as what it always was: a map
of someone else's territory, useful for orientation and not for
navigation.

---

## Cross-cutting rules for every milestone

1. **Measured, not assumed.** Any traversal, dispatch or memory change is
   re-run through `crossover`/`safety`/`memory`/`repeater`/`fusion` and
   the numbers go in the commit and the results doc.
2. **No `cfg` in derive output.** Bodies gated in `lumen-static`, never
   the call site (bitten twice; see `results-traversal.md`).
3. **Zero `unsafe` in the release core** is a gate from M0 on.
4. **Doc currency** per AGENT.md: the handoff's DECIDED/SUPERSEDED markers
   and the decision records move in the same commit as the code.
5. **One commit per task.** Task ids (`T0.6`) in commit subjects.

## Order and dependencies

```
M0 ──► M1 ──► M2 ──► M3 ──► M4 ──► M5 ──► M6 ──► M7a ──► M8 ──► M9 ──► M10 ──► M11 ──► M12
                                                    └─► M7b ──► M8b ──► M8c    (embed) (widgets) (harness)
                                                       (manual)  (raster) (layout)

Actually run, 2026-09-04: M7a → M7b → M8b → M8c → M8 → M9 → agent P6.
M8b/M8c came before M8 because a window existed to measure against and
scrolling needed nothing from them; M8's T8.0 had to precede a
`ScrollView` inside an `Either`, which it now is (tested).
        (style array first: Length must live somewhere)
```

M0 is the large one and is pure refactor — no new behavior, every test
green before and after. M1–M4 are each small. M5 is integration. M8b
and M8c hang off M7b, not M8: they need a window to measure against and
nothing from scrolling; M8b before M8c because the frame's time is in
pixels, not layout (`docs/results-frame-cost.md`).

**Status 2026-09-04: M0–M9 complete, plus M8b/M8c and agent parity
P0–P6.** What is *not* built is recorded where it was decided rather
than dropped: M8c's T8c.1–T8c.3 (measured; unsound while widget fields
are public), focus as a `NodeSel` (T8.0 removed its motivation), IME and
multi-line text (B6/B7), selection (needs modifier state or pointer-drag
routing), and `app.setValue` (would bypass the one input queue, and `input.fill`
serves the need instead). **M10 is the embedded profile**, then M11
(widget vocabulary) and M12 (test harness) — which replaced "parity
phases B–E" on 2026-09-04. Parity is no longer a destination.

**Status 2026-09-01: M0–M6 complete** (M5 less the winit binding, above).
Gate at the close of M5: 32 default suites + 8 gated all green, clippy
clean in both feature configs, zero `unsafe` in the release core, hot
reload demo PASS, counter renders. What M2's model changed and what M4
could not deliver as specified are recorded in their sections rather than
in this line, so the next reader finds the reason next to the claim.
