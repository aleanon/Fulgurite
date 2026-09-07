# Static-Tree Rust GUI Framework — Design Handoff

**Status:** design handoff; §10 build order **complete** (2026-08-30);
§5's traversal decision **superseded by measurement** (2026-09-01) — the
current work queue is `docs/plan-architecture-v2.md`
**Audience:** an engineering agent picking up implementation
**Purpose:** capture decisions, rationale, and open questions so they don't get relitigated

---

## 0. Read this first

This document records a design exploration, not a finished spec. Sections marked
**DECIDED** are settled and should be treated as constraints. Sections marked
**OPEN** need resolution before or during implementation. Sections marked
**REJECTED** record ideas that were considered and ruled out, with reasons — do
not re-propose them without addressing the stated objection.

The single most important thing in this document is [§10, the validation
experiment](#10-build-order). Run it before building anything else. If it fails,
most of the rest of this design is void.

> **Status 2026-09-01.** §10 ran and passed, and every **OPEN** section in this
> document is now closed — by measurement where one was called for, by
> deliberate deferral where it was not. Two sections no longer describe the
> implementation and say so at the top: §5's traversal half is **SUPERSEDED**
> (recursive descent, `docs/results-traversal.md`) and §7 is **REFINED** (`fn`
> pointers, not `Msg` — `docs/events-decision.md`). The live work queue is
> `docs/plan-architecture-v2.md`; this document is the design record behind it.

---

## 1. Goal

A Rust GUI framework that:

- is maximally efficient across a wide size range, from microcontroller-class
  apps to very large desktop applications
- supports hot reload during development
- exposes an observability surface that a coding agent can read and drive during
  development
- is a general framework for third-party use, **not** application-specific

Non-goals (explicit):

- server-driven UI or fully data-defined layouts as a first-class path
  (supported only through the `dyn` island escape hatch, §6)
- being a research project on layout algorithms — the layout model is settled
  and boring on purpose (§2)

---

## 2. Baseline: retained-mode pipeline

**DECIDED.** The runtime architecture is the standard high-performance retained
pipeline, as converged on by Flutter, browser compositors, Slint, and GPUI.
Nothing here is novel; it is the floor.

```
UI thread
  1. Retained node tree        persistent identity + state
  2. Invalidation              dirty flags, relayout boundaries
  3. Layout                    constraints down, sizes up
  4. Paint to display lists    cached per unchanged subtree
                 │
Raster thread    ▼
  5. Raster and composite      GPU layers, damage rects only
```

The four properties that produce the performance, in rough order of importance:

1. **Nothing rebuilds by default.** A mutation sets a dirty flag that propagates
   up only until it reaches a *relayout boundary* — a node whose own size
   provably cannot change (fixed size, or tight constraints from its parent).
   A text change deep in a list costs O(depth-to-boundary), not O(tree).

   **PLANNED 2026-09-04, not built** — `docs/plan-architecture-v2.md` M8b/M8c.
   The table carries the parents, the `RIGID` bits and contiguous subtree
   ranges the property needs; no pass consumes them yet, and every frame
   lays out, paints and rasterises the whole tree. **DECIDED there:** retention
   is a type parameter on `Host` (`Rebuild` | `Incremental`), never a cargo
   feature, because `Rebuild` is the oracle the incremental path is tested
   against, and rigidity is per axis (`RIGID_W`/`RIGID_H`; a card fixed in width and shrink in height bubbles height changes only).

2. **Layout is single-pass.** Constraints flow down, sizes flow back up, parent
   positions the child. No node reads its own final position during layout.
   This is what makes it linear rather than iterative reflow.

3. **Stages skip independently.** Dirty-for-paint-but-not-layout skips layout.
   A node that only moved skips both and gets a new transform on its cached
   layer. A transform-only animation frame reaches the GPU with no structural
   work on the UI thread.

4. **The GPU keeps its own retained copy.** The layer tree is persistent
   textures. An unchanged frame is a few quads plus a damage rect.

   **HALF-BUILT 2026-09-07** — `lumen-static-wgpu`, `docs/results-gpu.md`.
   There *is* a GPU backend now, and it does keep a persistent target
   texture that damage is scissored into, so a partial frame is a few
   quads over a retained image. There is **no layer tree**: no per-subtree
   texture, no transform-only frame, and raster is on the UI thread. The
   half that was built is worth 20× at 4K (22.87 ms → 1.10 ms); the half
   that was not is what item 3 above needs, and nothing depends on it yet.

Plus two implementation details that matter as much as the architecture: flat
contiguous node storage addressed by `u32` handles rather than pointer chasing,
and pipelining the two threads so frame N builds while frame N−1 rasterizes
(roughly halves perceived latency at no algorithmic cost).

**AMENDED 2026-09-01:** flat contiguous *state* storage (`[NodeState; N]`)
stands; flat *traversal* does not — see §5's supersession note. Recursive
descent over the static struct, writing into the flat state array, measured
34–39% faster than iterating a flat table at every size tested.

**Caveat to keep in mind:** this design pays real costs in memory and in
structural-change work. For a UI where everything changes every frame, immediate
mode can win. That is not our target.

---

## 3. The core idea: topology in the type, values in the fields

**DECIDED.** The widget tree is a Rust struct whose *type* encodes the topology
and whose *fields* hold the dynamic values.

```rust
struct App {
    header: Row<(Label, Spacer, Button)>,
    rows:   Column<Vec<TodoRow>>,   // only the dynamic part allocates
    footer: Either<Label, Spinner>,
}

struct Label {
    text:  Dirty<String>,   // runtime value + dirty bit
    cache: Cell<Size>,      // memoized measure
}
```

Prior art: SwiftUI (`VStack<TupleView<(Text, Button<Label>)>>`), Slint (its
compiler generates exactly this — a struct with one field per element and
properties as cells), Xilem (view tree as nested generic types), Druid
(`WidgetPod<T, W>`).

### What this buys

- **No allocation for the tree itself.** No `Box<dyn Widget>`, no `Vec` for
  fixed children. One struct, children inline, parent and children adjacent in
  memory.
- **Free identity.** Every node has a fixed offset in one allocation. Focus,
  animation state, and scroll position survive frames with no diffing and no
  reconciliation keys, because the node *is* the same memory.
- **Const-computable facts about the tree** (see below).

### Const lowering

This is where the compile-time structure actually pays, and it is the part worth
protecting:

```rust
trait Widget {
    const NODE_COUNT: usize;
    const IS_RIGID:   bool;   // size provably independent of content

    fn layout(&mut self, c: Constraints, state: &mut [NodeState]) -> Size;
}

impl<A: Widget, B: Widget> Widget for Row<(A, B)> {
    const NODE_COUNT: usize = 1 + A::NODE_COUNT + B::NODE_COUNT;
    const IS_RIGID:   bool  = A::IS_RIGID && B::IS_RIGID;
}
```

`IS_RIGID` is the relayout boundary from §2, except the compiler knows it.
`NODE_COUNT` gives `[NodeState; App::NODE_COUNT]` — the entire layout cache for
the UI as one fixed-size array, stack- or static-allocatable, no allocator
required.

---

## 4. The monomorphization trap

**Read this before writing any traversal code.** It is the reason for the
central design decision in §5.

If layout and paint are written as recursive generic descent, the compiler emits
a separate specialized `layout` and `paint` **per distinct subtree type** — that
is, per tree *position*, not per widget kind. On a large app that is tens of
thousands of instantiations.

Consequences:

| Symptom | Effect |
|---|---|
| Instruction-cache pressure | A boring `dyn` loop over a flat arena can be *faster* past a certain size. A dyn call with a stable target is predicted nearly perfectly. |
| Compile times | Change one leaf → root type changes → every monomorphization above it invalidates → whole UI crate recompiles. |
| Binary size | Bloat, ironic given the embedded niche. |
| Error messages | 400-line type names; rust-analyzer degrades badly. |
| Refactoring | Moving a widget changes types up the whole chain. |

Rust also has no variadic generics, so `Row<(A, B, C, ...)>` needs
macro-generated tuple impls and gets unpleasant past roughly a dozen children.
SwiftUI's "expression too complex to be solved in reasonable time" is this exact
failure mode.

**The crossover point where dyn wins is lower than intuition suggests.**

---

## 5. Central decision: reify the tree as a flat const table

> **SUPERSEDED 2026-09-01 — traversal half only** (`docs/results-traversal.md`).
> Storage as a static struct stands. Const `NODE_COUNT` stands. A const
> table stands, slimmed, for *index addressing* (the agent surface,
> journal and audit). What is overturned is *traversal*: iterating the
> table through `desc → kind → vtable` costs three dependent loads per
> node per phase, and recursive descent over `&dyn` children — the vtable
> carried in the reference, no table read at all — measured 34–39% faster
> at 10, 1k and 100k nodes, with devirtualization ruled out. §4's trap is
> avoided the same way this section intended: children are called through
> `&dyn`, so code is per *kind*, not per position; `#[fuse]` is the same
> body with static calls. The i-cache/prefetch premise below was measured
> false for this workload. Stack depth is a compile-time `DEPTH` cap. The
> §12.1 dispatch question is closed by the same measurement.
> Implementation: `docs/plan-architecture-v2.md` M0.

**DECIDED.** Storage stays a static struct. Traversal does **not** use recursive
generic descent. A derive macro emits a flat const table alongside the struct,
and the runtime passes are iterative loops over that table.

```rust
struct NodeDesc {
    offset:      u32,   // byte offset from tree root
    kind:        u16,   // index into VTABLES
    first_child: u32,
    child_count: u16,
    flags:       u8,    // rigid, opaque, clips, ...
    // dev-only, see §9:
    // src_file: u16, src_line: u32,
}

static NODE_TABLE: [NodeDesc; App::NODE_COUNT] = /* generated */;
static VTABLES:    [WidgetOps; KIND_COUNT]     = /* one per widget kind */;
```

The load-bearing number is `KIND_COUNT`. There is one `layout` body per widget
*kind* — `Label`, `Button`, `Row`, maybe forty in a full toolkit — not one per
tree *position*. The code-size blowup from §4 disappears.

### What survives from §3

- Zero allocation — offsets index into the same single struct.
- Stable identity — a node's offset *is* its ID.
- `NODE_COUNT` still const, so `[NodeState; N]` still fits a fixed budget.
- Rigidity still computed at compile time; it is a flag byte now rather than a
  const-folded branch. **Better:** because the table is const, the macro can
  partition it into `RIGID_NODES` and `FLEXIBLE_NODES` index arrays so the
  layout pass only iterates the flexible ones. A compile-time partition beats a
  runtime branch outright.

### What is gained

The pass becomes an iterative loop over a contiguous array in parent-before-child
order: no call-stack depth, no stack overflow on deep trees, siblings adjacent
in memory, prefetcher can see what is coming.

### What is lost, and the mitigation

Cross-node inlining. Fully monomorphized, `Row::layout` inlines `Label::measure`,
constant-folds a fixed width, and collapses a subtree into straight-line code
with no calls. The table version pays one indirect call per node. For a small
hot subtree — a table cell rendered ten thousand times — monomorphization
genuinely wins.

Mitigation is per-subtree fusion, choosing erasure granularity locally instead of
globally:

```rust
#[fuse]  // one kind, one specialized layout fn, one NodeDesc
struct Cell { icon: Icon, label: Label }
```

Same trade as kernel fusion in an ML compiler: fuse the hot inner region, keep
the outer schedule dynamic.

### DECIDED (2026-09-01): vtable array vs enum dispatch — the question dissolved

*Resolved by M0, not answered: `docs/results-traversal.md`.*

This asked which dispatch mechanism should drive a flat table walk. Recursive
descent removed the walk, and with it `WidgetOps`, `KIND_LIST` and
`NodeDesc.kind`. A container calls its children through monomorphized code the
compiler can see through; there is no per-node dispatch left for an enum to
beat. Measured 34–39% faster than the table walk it replaced.

The hybrid escape this section proposed — built-in kinds as variants,
`Kind::External` for the rest — is subsumed by §6: `Dyn` is the one island
where indirection is paid, and it is opt-in per node rather than a property of
the dispatch mechanism.

---

## 6. Dynamism escape hatches

**DECIDED**, in order of preference:

| Need | Mechanism | Cost |
|---|---|---|
| Homogeneous list | `Vec<W>` / `SmallVec<[W; N]>` | one alloc, static dispatch |
| Known variant set | `enum Cell { Text(Label), Check(Checkbox) }` | `size_of` = max variant, no alloc |
| Show/hide, either/or | `Option<W>`, `Either<A, B>` | memory for both branches always held |
| Genuinely unknown | `Box<dyn Widget>` island | the one place indirection is paid |

`Either` holding both branches' memory is usually a *win* — toggling is free and
the hidden branch keeps its state.

> ~~**Implementation deviation (M4/T4.2, 2026-09-01)**~~ — **RESOLVED
> 2026-09-04 (T8.0): §6's shape ships.** `Either<A, B>` has
> `NODE_COUNT = 1 + A + B` with both branches spliced into a merged
> const table, so the deviation below no longer exists. Kept because the
> reason recorded for it was false and that is worth remembering.
> `Either<A, B>` shipped as *one* table node with both branches in off-table
> arenas, on the stated ground that a merged `A + B` table "requires
> `generic_const_exprs`". Review showed that is false: `SUB_TABLE` is a
> slice, so a padded `[NodeDesc; MAX]` built in const-eval and `split_at(N)`
> gives the merged table on stable, with the bound a compile error. Both
> branches retain state either way; what the shipped shape costs is
> tree-absolute agent ids and four heap `Vec`s per `Either`. See
> `docs/either-reopened.md`.

### Repeaters

**DECIDED.** A million-row table cannot have a million `NodeDesc` entries.
Instead, one table entry marks a **template range plus a runtime instance
count**, with instance state in a growable side arena and only the visible
window materialized. Slint's `for` construct works this way.

The resulting scaling property is the important one:

> `NODE_COUNT` grows with how much UI you **wrote**, not how much data you
> **display**.

---

## 7. Hot reload

**DECIDED.** Hot reload is a hard requirement and it is achievable. Prior art:
`dexterous_developer` does serde-based state migration across dylib reload for
Bevy using `rmp_serde`, explicitly supporting schema evolution.

### The model

Throw away the tree, rebuild from the new code, graft old *values* back on.
Structure and closures come from the fresh dylib; only data crosses the
boundary. Same model as Flutter and React Fast Refresh.

Make the split a type-level fact, not an attribute you must remember:

```rust
#[derive(Reloadable)]
struct TodoRow {
    #[reload(fresh)] on_toggle: Msg,                // new dylib supplies it
    #[reload(keep)]  label:     String,             // migrated
    #[reload(keep)]  done:      bool,
    #[reload(reset)] glyph_run: Option<ShapedText>, // dropped, recomputed
}
```

The derive emits a parallel mirror type containing only the `keep` fields:

```rust
#[derive(Serialize, Deserialize)]
struct TodoRowSaved { label: String, done: bool }

impl Reloadable for TodoRow {
    type Saved = TodoRowSaved;
    fn save(&self) -> Self::Saved;
    fn fresh() -> Self;                 // constructor from the new dylib
    fn restore(&mut self, s: Self::Saved);
}
```

This is strictly better than `#[serde(skip)]`: `Saved` **cannot** contain a
vtable or function pointer, because the type does not have those fields. The
use-after-free stops being a discipline and becomes structurally impossible.

### Container impls — saved data drives arity

```rust
impl<W: Reloadable> Reloadable for Vec<W> {
    type Saved = Vec<W::Saved>;
    fn restore(&mut self, saved: Vec<W::Saved>) {
        self.clear();
        self.extend(saved.into_iter().map(|s| {
            let mut w = W::fresh();   // code from the new lib
            w.restore(s);             // data from the old run
            w
        }));
    }
}
```

`App::fresh()` gives a zero-row list; the restore gives back all 47. This rule
applies everywhere collections appear.

### Events must be data, not closures

**DECIDED, and it constrains the whole architecture.**

```rust
// Not reloadable — field is a pointer into the old dylib.
struct Button { on_click: Box<dyn FnMut(&mut State)> }

// Reloadable — Msg is a plain serializable enum; the match arm
// that interprets it lives in the fresh dylib.
struct Button { on_click: Msg }
```

This is not merely a workaround for unserializable closures. A closure that
captured meaningful state — `move |s| s.items.remove(idx)` — gets whatever `idx`
the new `fresh()` hands it, which for a dynamically built list is wrong.
`Msg::Remove(idx)` survives because `idx` is data in the `keep` half. **Elm-style
message dispatch is the precondition for hot reload here.**

**REFINED 2026-08-31** (`docs/events-decision.md`): the constraint stands —
event *payloads* must be data, never captures — but the static tree removes
the reason handlers capture (identity is positional; repeater rows re-bind
from data every frame and arrive as dispatch arguments), so tree-internal
handlers are plain `fn` pointer fields (capture-proof by type, re-supplied
by `fresh()`), already validated live by `Repeat.bind` in the Step 4 demo.
`Msg`-shaped data survives at exactly one seam: work in flight across a
frame/thread/reload boundary, which `fresh()` cannot reach into.
Implementation plan: `docs/plan-events.md`.

### Sequence — not negotiable

The old tree's `Serialize` impls, `Drop` impls, and vtables all live in the old
dylib:

1. `save()` **while the old lib is still loaded** → pointer-free bytes
2. Drop the entire old tree (old `Drop` impls run), still loaded
3. Unload (or don't — see below)
4. Load new lib, `App::fresh()`, then `restore(bytes)`

The classic bug is keeping the old tree alive across the swap as a safety net,
which guarantees destructors run against unmapped code.

### ABI details that bite

- The byte buffer must be owned by the **host binary**, not passed as a `Vec`
  from old lib to new. Each dylib may have its own allocator instance; freeing
  in the new lib memory allocated by the old one is UB.
- Rust has no stable ABI. Keep the cross-boundary surface to a couple of
  `extern "C"` functions taking raw pointers and lengths.
- `TypeId` is not stable across compilations — any `Any` downcasting needs a
  hand-assigned stable ID.
- `repr(Rust)` field ordering is not guaranteed stable across compilations
  either, so there is no shortcut around full serialization even for
  POD-looking structs.
- **Pragmatic shortcut:** `dlopen` without ever calling `dlclose`. Leaks a few
  MB of address space per reload, nobody notices in a dev session, and the
  entire "old code unmapped while still referenced" class disappears.

---

## 8. Configuration: how the size dial actually works

### REJECTED: feature flags for framework overhead

Cargo features are **additive and unified across the whole dependency graph**.
If a third-party chart widget depends on the framework with `features =
["small"]` and a table widget uses `features = ["large"]`, Cargo enables both.
Mutually exclusive features cannot exist in a library that third parties build
on. Worse, a widget crate author fundamentally cannot know whether the final app
is tiny or huge, so they could not choose correctly even if unification were not
a problem.

For a leaf application, feature flags are fine — you control your own graph. For
an ecosystem framework they are the one mechanism that structurally cannot work.

### DECIDED: one representation, sliding const-ness

Do not build a "small mode" and a "large mode". The `NODE_TABLE` format is
byte-identical across the whole range; what changes is how much of it the
compiler knew. The root table is assembled from three kinds of source:

- **Const sub-tables** — one per component crate
- **Repeater ranges** — template + runtime count (§6)
- **Dyn islands** — plugins spliced in at runtime

All three feed one uniform kind-dispatched traversal loop.

A tiny embedded app is 100% const sub-tables: root table is a single `static`,
`NODE_COUNT` is const, arena is `[NodeState; N]` in `.bss`, `no_std` works
because nothing allocates. A large desktop app is *mostly* const sub-tables too
— it concatenates forty component sub-tables into a `Vec` at startup and splices
in plugins. Same descriptors, same loop, no mode switch, and **a widget crate
compiles identically for both** because it only emits its own const slice.

### Type-level axes

Any knob that changes semantics must live in a type parameter threaded from the
app root, or be resolved automatically. **Keep type-level axes to one** —
realistically the arena/allocator. Each one is viral through every signature in
the framework.

---

## 9. The component boundary is the unit of everything

**DECIDED. This is the decision to anchor the framework on.**

A component boundary is simultaneously:

1. **Incremental compilation firewall** — its const sub-table is opaque to the
   root, so editing a leaf recompiles one small crate rather than the whole UI.
   This is the direct fix for the §4 compile-time cost. Without it, "extremely
   large" is unbuildable regardless of runtime performance.
2. **Hot-reload unit** — one dylib, one `save`/`fresh`/`restore` cycle,
   sub-second because the rebuild is one crate.
3. **Erasure boundary** — inside, `#[fuse]` monomorphizes freely; at the seam,
   kind dispatch. This is the overhead dial, and it is *local* rather than
   global, which is exactly what a feature flag cannot be (§8).
4. **Observability namespace** — a stable path like `app::sidebar::filter_row`
   that survives reloads.

If these four boundaries do not coincide, you end up with four mechanisms that
disagree with each other.

### Agent observability

**DECIDED: target AccessKit; do not build a bespoke agent channel.**

An agent needs a serializable tree with roles, labels, geometry, and state, plus
the ability to synthesize events and observe what changed. That is precisely the
accessibility tree. One implementation serves both screen readers and agents,
and the accessibility work — which is required anyway — stops being a tax.

**One addition beyond AccessKit:** put source provenance in `NodeDesc`. The
macro knows the file and line where each node was declared, so bake
`src_file: u16, src_line: u32` behind a dev-only cfg. An agent's report becomes
"the button at `src/sidebar.rs:42` has zero hit area" — directly actionable, and
it composes with hot reload, because the agent identifies the node, you patch
that line, the dylib swaps, and the node ID is still valid (it is an offset, not
a generated key).

Gate the entire dev surface behind one `cfg` that compiles to nothing in
release. The const tables already exist; only *exposure* is gated, so the
release cost is genuinely zero rather than "small".

---

## 10. Build order

**Do these in order. Step 1 is a kill criterion.**

**All six steps DONE (2026-08-30).** Subsequent work — agent parity
P0–P4, the event system E0–E4, and the measurements that overturned §5 —
is recorded in `docs/plan-agent-parity.md`, `docs/plan-events.md`,
`docs/results-*.md`, and the v2 work queue `docs/plan-architecture-v2.md`.

### Step 1 — Validate the compile-time firewall (do this first)

Build a deliberately ugly test app with ~200 components and measure incremental
rebuild time for a single-leaf edit. If component-level incrementality does not
hold, the entire design collapses at scale, and that is far cheaper to discover
in week two than in year two.

Success criterion to fix before starting: pick a target (suggested: leaf edit →
usable reload in under 2s on the reference machine) and hold the design to it.

### Step 2 — Node table + traversal

Derive macro emitting `NodeDesc` tables, `KIND_COUNT` vtables, iterative
layout/paint loops, rigid/flexible partitioning. No hot reload, no observability
yet. Benchmark against a naive `Box<dyn Widget>` tree at 10, 1k, and 100k nodes
— the crossover behaviour from §4 needs to be measured, not assumed.

**DONE 2026-08-30.** Built in `crates/lumen-static{,-macros}`; measured
~~crossover between 10 and 1k nodes (static wins ~16–21% above it, loses ~16%
at 10 nodes)~~ — *superseded 2026-09-02: on the recursive implementation
there is no crossover; 10 nodes is a toss-up and static wins ~31% at 1k
and 100k (`docs/results-step2-crossover.md`, header note).* Two deviations from the §5 sketch proved necessary —
`parent`/`next_sibling` fields in `NodeDesc` and once-per-*container*
dispatch (per-child dispatch loses to naive dyn outright). See
`docs/step2-design-notes.md` and `docs/results-step2-crossover.md`.

### Step 3 — Repeaters

Virtualized list with template + runtime count. Validates that `NODE_COUNT`
scales with source size rather than data size.

**DONE 2026-08-30.** `Repeat<T>` contributes one table entry; the template
is `T::SUB_TABLE` itself, so no skip logic exists anywhere. Measured flat
1.26 µs/frame from 1k to 10M rows (35-row window) vs 69 ms materialized at
1M rows. `NODE_COUNT` data-independence is a compile-time assertion. See
`docs/step3-repeaters.md`.

### Step 4 — Hot reload

`Reloadable` derive, save/fresh/restore, dylib swap, `Msg`-based events. Depends
on Step 1 having succeeded.

**DONE 2026-08-30.** `Reloadable` + derive with keep/fresh/reset field
classes, nested widget recursion, Vec-arity-from-saved-data, and per-field
fallback with logging (which resolves §12.2 as the general mechanism, not
an enum special case). Live dylib swap demo passes the full §7 sequence:
`scripts/hot_reload_demo.sh`. See `docs/step4-hot-reload.md`.

### Step 5 — Observability

AccessKit integration plus source provenance in `NodeDesc`.

**DONE 2026-08-30.** `observe::snapshot` → `accesskit::TreeUpdate` (one
tree for screen readers and agents; the JSON agent dump is derived from
it, so they cannot drift). `NodeDesc.src` provenance stamped const by the
derive; ids positional for table nodes, row-stable for repeater
instances. Gated behind additive `observe`/`reload` features (Step 4's
gating debt paid); zero release cost verified by re-running the Step 2
benchmark unchanged. Deferred: action synthesis (needs events),
incremental TreeUpdates. See `docs/step5-observability.md`.

### Step 6 — `#[fuse]`

The optimization dial. Deliberately last: it is a refinement, and its value can
only be judged against Step 2's measurements.

**DONE 2026-08-30 — and with it the build order is complete.** Spelled
`#[widget(axis, spacing, fuse)]`: one table entry, one kind, subtree
layout/paint as straight-line `Inline` code, geometry exactly equal to
the unfused twin (tested). Measured against Step 2's arms: 43 ns at 10
logical nodes (closing the dyn gap, −36%), −61%/−64% vs the table at
1k/100k. Per-*type* monomorphization keeps §4's per-position blowup
shut. Costs recorded: fused internals are opaque to Step 5's
observability and hold no per-node state. See
`docs/results-step6-fuse.md`.

---

## 11. Honest risk assessment

### Performance expectations — calibrate these

In a mature retained GUI, **the tree walk is rarely the bottleneck.** Frame time
goes mostly to text shaping and rasterization, GPU command submission, and
damage computation. Layout and paint traversal are often in the 5–15% range.

So if the static tree makes traversal 3× faster than a dyn-based one, that buys
a few percent of frame time. Meanwhile **invalidation architecture — which is
completely orthogonal to whether the types are static — decides whether you
touch 3 nodes or 3000.** A `Box<dyn Widget>` tree with correct relayout
boundaries beats a static tree with sloppy invalidation by an order of
magnitude, and no amount of monomorphization closes that gap.

The two advantages that are real and that nothing else gives:

- **Zero allocation for the entire UI** — not "few", zero. This is why Slint
  went this way: microcontrollers with no allocator and a fixed memory budget
  known before shipping.
- **Const-folded rigidity** — a dynamic tree checks the flag at runtime per node
  per frame. The static version does not have the check at all.

**Implication for prioritization:** invalidation granularity, text caching, and
damage tracking deserve at least as much attention as the static tree. Do not
let the interesting part crowd out the effective part.

**DECIDED 2026-09-03 — confirmed by the first profile of a live frame**
(`docs/results-frame-cost.md`): layout and paint were 1 µs of a 190 ms frame.
Everything else was rasterisation — an 18-layer shadow and an unoptimised
dev build. The static tree's own cost is invisible; the calibration above
was right. `app.perf` now reports every stage plus input → presented
latency, and `scripts/ui_check.py latency` judges it against 16 ms / 100 ms.

### What actually kills GUI framework projects

Layout and traversal are the tractable, fun part, and roughly 15% of the work.
The years go to:

- **Text**: shaping, bidi, complex scripts, font fallback, IME composition,
  selection across all of it
- **Platform integration**: multi-window, HiDPI, per-monitor scaling, clipboard,
  drag and drop

None of these get easier because the tree is static. **Strong recommendation:**
compose rather than build. Take an existing shaping and rasterization stack and
make the contribution the node-table architecture, which is the genuinely novel
part.

### Residual costs of this design

- Binary size (monomorphization bloat, reduced but not eliminated by §5)
- Error messages and rust-analyzer degradation at deep generic nesting
- Refactoring friction — moving a widget changes types up the chain
- Authoring ergonomics: nobody hand-writes `Row<(Label, Spacer, Button<Msg>)>`
  at scale. Every project down this road built a DSL layer (Slint invented a
  language and compiler; SwiftUI needed result builders plus compiler
  special-casing). **The real price is that you must build and maintain a DSL,
  then debug through it** — reading generated code, owning your macro's error
  spans.

### Note on the authoring cost given LLM authorship

If code is primarily machine-authored, the calculus shifts but not uniformly:

- **Largely neutralized:** verbosity, refactoring cascade (mechanical
  propagation is reliable), long type errors (a parsing problem).
- **Actively improved:** rich static types make the compiler a verifier in the
  loop. A generation mistake in a stringly-typed dynamic tree surfaces at
  runtime in a specific UI state, possibly untested; the same mistake in the
  static tree is a compile error. The type system's value goes *up* for
  machine-authored code.
- **Relatively worse:** compile time. Iteration speed is the main thing an
  agentic loop buys, and a full UI-crate rebuild per leaf edit throttles exactly
  that. A human absorbs a 90-second build by context-switching; a loop just runs
  90 seconds slower per step, compounding across every iteration.

Net: authoring difficulty largely stops being the reason to avoid this design.
**Compile time is still the reason, and it is now a larger share of what
remains** — which is why §10 Step 1 is the kill criterion.

---

## 12. Open questions for the agent

1. **Dispatch mechanism** (§5) — vtable array vs enum-with-external-escape.
   Requires measurement at realistic `KIND_COUNT` and node counts.
   **CLOSED 2026-09-01.** Neither: recursive descent over `&dyn` children
   (`docs/results-traversal.md`) beats both a vtable array and an indexed
   accessor, and needs no `KIND_LIST` at all.
   Earlier note — **MEASURED 2026-08-31, a third option:** a derive-generated
   `node(i) -> &dyn Node` accessor, where the vtable rides on the
   reference and `KIND_LIST` disappears entirely. Costs +8% traversal at
   100k nodes with a cached index (under 1% of a realistic frame), is
   *faster* than the vtable array below ~100 nodes, halves per-node
   static data (32 B → 16 B *(per-node saving; both figures predate M1's `wid`/`classes`, which add 32 B to `NodeDesc` in either path — see `docs/results-safe-prototype.md` F1.9)*), and removes every `unsafe` in the release
   build. **Prototype retained, not adopted** — `Repeat` and event
   dispatch are not ported, which is where the estimate could move. The
   deciding factor is the long-term formal-verification goal, not
   performance. See `docs/results-safe-prototype.md`,
   `docs/verification-notes.md`.
2. **Reload sequence for enum/`Either` migration** — when the saved value names
   a variant the new code no longer has, or `Either` saved `A` and the new code
   defaults to `B`. Rule: take the saved variant when it still exists, fall back
   to `fresh()` when it does not, **and log it** — silent fallback is
   indistinguishable from a bug in the new code.
   **RESOLVED 2026-08-30:** implemented as per-field fallback-with-log for
   *every* keep field (missing/type-changed/unknown-variant alike), not an
   enum special case. Demonstrated live in the Step 4 demo. See
   `docs/step4-hot-reload.md`.
3. **Arena as the one type-level axis** (§8) — confirm a single type parameter
   at the app root is genuinely sufficient, or find the second one early.
4. **Repeater state lifecycle** — what happens to instance state for rows
   scrolled out of the materialized window, and how it interacts with hot reload.
   **DECIDED (partially) 2026-08-30:** window instances are disposable — 
   rebound from data every frame, dropped across reload; only
   count/scroll/extent are kept. Persistent per-row UI state, when it
   arrives, must live in the data model or a keyed side store, never in
   window instances. See `docs/step3-repeaters.md`, `docs/step4-hot-reload.md`.
5. **Fusion granularity heuristics** (§5) — whether `#[fuse]` stays manual or
   the macro can infer candidates from subtree size and instantiation count.
   **RESOLVED 2026-08-30: stays manual.** Per-type (not per-position)
   monomorphization bounds the cost of a wrong choice, and fusion's real
   trade-offs (observability opacity, no per-internal-node state) are
   semantic judgments. Revisit only on evidence from real apps. See
   `docs/results-step6-fuse.md`.
6. **Parallel layout** — independent subtrees are trivially parallelizable given
   the flat table, but this was not explored. Interacts with the two-thread
   pipelining in §2.
   **NOTE 2026-09-01:** with recursive traversal the natural unit is a
   subtree call, which parallelizes as fork/join rather than as index
   ranges. Still unexplored.
7. **Reference target for §10 Step 1** — pick the machine and the rebuild-time
   budget before writing the test app, so the kill criterion is falsifiable.
   **RESOLVED 2026-08-30:** reference machine is the i9-13900KF/32-thread dev
   box (`rustc 1.94.0`); budget is **leaf edit → usable rebuild < 2s**. See
   `docs/results-step1-firewall.md`.
