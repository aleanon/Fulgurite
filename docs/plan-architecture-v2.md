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

Drop `kind` and `offset` (24 B → 16 B). `splice_child` loses two
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

**Outcome, with two deviations recorded rather than papered over.**

1. **`Either<A, B>` is one table node, not `A + B`.** Concatenating two
   sub-tables into a merged const table needs
   `generic_const_exprs` — the stable-Rust wall the whole design avoids.
   `Either` instead holds both branches in off-table arenas behind a
   single table node, which is the mechanism `Repeat` already proved.
   Both branches retain their state across a toggle, which was the point
   of the hatch; what is lost is per-branch table addressing, so agent
   ids inside an `Either` are branch-relative.
2. **The enum derive cannot paint variant content.** Enum variants keep
   stable indices and `node(i)` matches the discriminant as planned, but
   the scratch arena a variant would need for painting is not retained
   across the call. `Either` is the answer where painted content matters;
   the enum derive is for structural switching. Documented at the derive.

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
render/text crates. Both are Phase A follow-ons, not blockers for M6.
(The `lumen-text` adoption also settles M6's open measurement — see
`docs/bindings-decision.md`.)

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

**Outcome, including the part that does not flatter the design.** The
mechanism works and the ergonomics land: the counter's click loop halved
and its `update` method is gone. But the measurement says the
output-compare **currently costs ~4–13 ns per binding to save ~2 ns** —
a net loss, because `BlockMetrics` is arithmetic and the memo it guards
is nearly empty. The design is a bet on real shaping arriving with
`lumen-text`, and `docs/bindings-decision.md` states the threshold
(~13 ns) so the bet can be *settled* rather than assumed when it does. In
absolute terms none of this matters yet: 1000 bindings cost 18 µs a
frame, 0.1% of a 16.6 ms budget.

---

## M7 — Scrolling and text input — **PLANNED, not started**

Design settled in `docs/text-and-scrolling-decisions.md` (twelve
decisions, 2026-09-02). Two features, one decision, because both need a
child larger than the box it is shown in. Two of A1's consequences are
already fixed (`Repeat` clips; the software renderer honours the clip
stack); the rest is listed there under "Consequences still to handle" and
needs a task breakdown before it is started.

Note for whoever writes that breakdown: A4 is the trap. `W1005` fires on
any `Shrink` node given less than it asked for, which inside a scroller
is the normal case — it must learn about clip scopes in the same commit
that introduces one, or it will scream on every scrolling view.

---

## M8+ — parity phases B–E

Widget vocabulary (Phase B), `.lss` (Phase C), platform/content (D), test
harness (E) as laid out in `docs/plan-lumen-parity.md`. Not expanded
here; each gets its own plan when reached, after M5 has shown the
architecture end to end.

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
M0 ──► M1 ──► M2 ──► M3 ──► M4 ──► M5 ──► M6 ──► M7 ──► M8+
        (style array first: Length must live somewhere)
```

M0 is the large one and is pure refactor — no new behavior, every test
green before and after. M1–M4 are each small. M5 is integration.

**Status 2026-09-01: M0–M6 complete** (M5 less the winit binding, above).
Gate at the close of M5: 32 default suites + 8 gated all green, clippy
clean in both feature configs, zero `unsafe` in the release core, hot
reload demo PASS, counter renders. What M2's model changed and what M4
could not deliver as specified are recorded in their sections rather than
in this line, so the next reader finds the reason next to the claim.
