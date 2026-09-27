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
                               (a command-count change is aligned, not
                               the whole window — M26/T26.11)
                               (damage in physical pixels: retention
                               observes after Scaled — M40/T40.1)
```

**DECIDED (M40/T40.1): damage is in the space the surface draws in.**
Layout, hit testing, `NodeState` and the agent's journal are logical;
retention's journal, `Damage`, the display list and both rasterisers are
physical, because `Recording` sits inside `Scaled`
(`docs/plan-physical-damage.md` §2.1; the gate is
`crates/lumen-static-shell/tests/damage_scale.rs`). The agent's
`ui.lastDamage` is journal-derived and stays logical.

**DECIDED (M40/T40.3): a plain rect's edges land on device pixels.**
`Scaled::rect` rounds each scaled edge to the nearest pixel (ties to
even), independently, keeping at least one pixel per axis — so a hairline
is crisp, shared edges stay shared, and the two backends agree at every
scale (`docs/results-gpu.md` §2a). Other shapes keep exact geometry.

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
   against, and rigidity is per axis — a card fixed in width and shrink in height should
   bubble height changes only.

   **That last clause is OPEN, not DECIDED, and said DECIDED until
   2026-09-08.** It named two constants, `RIGID_W` and `RIGID_H`, which
   have never existed: `desc.rs` has a single `RIGID` bit and always has.
   The doc-identifier gate reads `.rs` comments and did not read markdown,
   so the spec asserted an API it did not have, in a DECIDED marker, for
   four days. `docs/plan-invalidation.md` T16.6 extended the gate to
   markdown; T16.9 either implements per-axis rigidity — where it earns
   its place, in a layout pass that can skip — or deletes the claim.

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

**CORRECTED 2026-09-08 — the parenthetical is backwards, and the
recommendation is the wrong one of the two.** `docs/results-threading.md`.

Pipelining improves *throughput*, not latency: input→photon latency goes
**up** by one frame, because the frame a click affects is now rasterised
one period after it is built. The live gate measures click→presented at
6.0 ms against a 100 ms budget; a pipeline would add 16.7.

And it buys almost nothing here, because it only pays when the two
halves are balanced. On a real frame they are not: the tally spends
**2 µs above the rasteriser and 482 µs in it**, so hiding the first
behind the second saves 0.4%. Even at 10 000 rows it is 13% on the
software path and 30% on the GPU one.

**The threading that would pay is the other one — splitting the
*rasteriser* across bands**, which this section does not mention.
Measured, sixteen bands cost 1–6% more than one full render, so it
scales close to linearly: ~3 ms for a 4K full redraw against 19.2 on one
core, which is the difference between holding 60 fps at 4K on the CPU
and not. It adds no latency, needs no `unsafe`, and is still not urgent,
because the frames it helps are the ones the GPU backend already wins
and the frames it cannot help are already 18 µs.

**How to turn it on, and why it is off — DECIDED 2026-09-27 (M50/T50.5).**
It shipped as the shell's `threads` feature (T17.4), with `simd` beside
it (T17.1), and until M50 neither was reachable from `lumen-static-winit`
(R6-F5). An application enables `lumen-static-winit/threads` and
`lumen-static-winit/simd` now. **Both stay default-off by decision**
(`docs/plan-raster-allocation.md` §2.3): full redraws on desktop are the
GPU's (`docs/results-gpu.md`), and a banded frame spends a stated budget
of `std`'s spawns, 3 per worker + 1 (`docs/results-allocation.md`, "The
threaded rasteriser's budget"). Reopening the default wants a
measurement, not this paragraph.

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

> **CLARIFIED 2026-09-11 — the gloss is encoding-specific, and reading it
> as a general claim put a false sentence in the book**
> (`docs/results-monomorphization.md`). Monomorphization is per **type**
> and is deduplicated: two fields of the same type share one body. Two
> `Button<Counter>` at different positions are *one* instantiation, not
> two — counted in a shipped binary, where eight nodes over six distinct
> types produce six `layout` symbols.
>
> "Per position" holds for the `Row<(A, B, C, …)>` encoding this section
> is warning against, because there a node's type contains its entire
> subtree, so positions almost never share a type and the distinct-type
> count approaches the node count. It does **not** describe what shipped:
> §5's supersession note already says "code is per *kind*, not per
> position", and `docs/layout-and-storage-decisions.md` already says
> "kind count multiplies by instantiation, not position … so §4's trap
> stays shut". Both predate the book and neither was consulted.
>
> The Binary size row below is directionally right and its magnitude is
> not per node: measured, a node reusing an existing type costs ~20×
> less than one introducing a new struct.

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
  **Gated over the whole frame since 2026-09-19 (M26/T26.13)**, and not
  before: `crates/lumen-static-shell/tests/alloc_gate.rs` measured
  `Host::pump`, which stops at the display list, so the software
  rasteriser was outside every zero in `docs/results-allocation.md`. It
  was allocating 1.00 per frame for its clip stack and 3.00 per glass
  panel for the blur's buffers; both are zero now. **The truetype path
  every window runs is gated too since M50/T50.2** — a changing label, a
  3000-row list with one row changed, a text field per edit, each on
  `Canvas::with_font` with `FontMetrics`, each at zero: the canvas shapes
  into scratch it keeps and blits from a glyph cache, and the measurer
  lays out into a layout it keeps (`docs/results-allocation.md`, "The
  truetype path, covered"). Until T50.2 the gate drew block glyphs only
  and that path allocated 25.80 times a frame (T50.1 said so). A run that
  needs a fallback face still allocates, and says so.
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

**DECIDED 2026-09-25 (M41/T41.3): a data change is reported.** A
repeater's bind runs on every layout that reaches it, and under the
`Boundaries` default an unmarked repeater is not reached. The data's
version is `Repeat::generation`, which a value `#[bind]` writes from the
application's state and which marks exactly that repeater; the other
report is `Host::invalidate`. A marked repeater lays its rows out under
`Changed::ALL`, since a row's indices are its arena's, not the main
table's (`docs/plan-boundaries-repair.md` §2.3, §6).

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

**EXTENDED 2026-09-19 (M27/T27.11): `Event::Hover`.** A `PointerMove`
reaches a widget only under pointer capture, and capture is granted only
to a node that consumed a `PointerDown` — so a widget that paints *rows*
could not learn which row the pointer was over except while a button was
held. `Tree` and `OptionList` both shipped with the code for a hot row
and neither could ever receive the event that sets it; a hovered tree
rendered byte-identically to an un-hovered one. Routing the move itself
was tried first and is wrong — `Slider` reads a move as a drag, so a
hover would have dragged it, and `tests/slider.rs` said so. The two
questions are two events: *you are being dragged* is a `PointerMove`,
*the pointer is over you* is an `Event::Hover`, synthesized by
`dispatch` and offered to the hit node alone. A widget that does not
match it is unaffected, which is the safe default. `Event` is still 16
bytes (`tests/gesture.rs`), so `InputQueue`'s size is unchanged.

**REFINED 2026-08-31** (`docs/events-decision.md`): the constraint stands —
event *payloads* must be data, never captures — but the static tree removes
the reason handlers capture (identity is positional; repeater rows re-bind
from data every frame and arrive as dispatch arguments), so tree-internal
handlers are plain `fn` pointer fields (capture-proof by type, re-supplied
by `fresh()`), already validated live by `Repeat.bind` in the Step 4 demo.
`Msg`-shaped data survives at exactly one seam: work in flight across a
frame/thread/reload boundary, which `fresh()` cannot reach into.
Implementation plan: `docs/plan-events.md`.

**MADE CONCRETE 2026-09-20** (M28/T28.4, `docs/plan-tasks.md` §2.5, and
`docs/results-tasks.md` for the measurements): the seam this paragraph
anticipated is now two fixed-capacity queues and nothing else.
`inbox::Inbox<A::Msg>` carries results *into* the frame;
`spawn::Outbox` carries requests *out* of it, as
`Descriptor { kind, step, payload }` — three `u64`s, no code. A handler
is `fn(&mut S, &EventCtx)` and so cannot be handed a spawner; it pushes
a value, and the host pairs each descriptor with a job `fn` from the
**currently loaded** build at the moment of spawn, which is what keeps
the pointer out of flight. An unrecognised `kind` is dropped and
counted, never guessed at, exactly as the reload model already treats an
enum variant a new build removed.

**CORRECTED 2026-09-25 (M47/T47.1, `docs/plan-timers-wake.md` §2.1):** a
timer on an idle *window* fires. Until T47.1 it did not — the loop woke
at the deadline and drew no frame, and the live gate could not tell
because its every read asked for one (`docs/results-tasks.md` T28.12,
the correction). The window now fixes the deadline as an instant after
each frame and turns a wake at it into a frame; while a panic is shown
the timers are frozen and a retry starts from `dt` 0. **Catch-up is
arithmetic since M47/T47.2**: a stall fires at most a full outbox and
counts the rest (`Host::timer_coalesced`), where it used to hang or lose
them uncounted (`docs/results-tasks.md` T47.2).

**DECIDED 2026-09-26 (M47/T47.3, `docs/results-tasks.md` T47.3):** the
default spawner is a small pool — at most `Threads::cap` long-lived
workers, started lazily — not a thread per job, so a timer tick no
longer costs an OS thread. And the host keeps a **ledger** of every job
spawned and not yet landed (body returned, every result drained); its
length is `Host::in_flight`, the one in-flight count M48's "settled"
answer and M49's re-issue across a swap read.

**DECIDED 2026-09-27 (M49/T49.10, `docs/results-tasks.md` T49.10):** a
window's hot swap **re-issues the work in flight** to the new build. The
old session saves the ledger's unsettled descriptors, less running
subscriptions (`Host::reissuable`), beside its kept state. The new build's
boot spawns each one through its own `App::job_for` after the restore
(`Host::reissue`). Delivery across a swap is at least once: the old
worker cannot be stopped, so a job with a side effect may perform it
twice. Until then a chain in flight at a window swap was dropped silently,
and the docs credited M28/T28.7 with carrying it.

**DECIDED 2026-09-26 (M47/T47.7, `docs/mobile-decision.md`):** a window
keeps its images across suspend and resume — `Backend::take_images` is
required, and `suspended` takes the source back before the backend is
dropped. And a dropped GPU present is asked for again, a bounded number
of times (T47.4); a still window's idle cost is measured at last —
`docs/results-frame-cost.md`, Idle (T47.8).

### Sequence — not negotiable

The old tree's `Serialize` impls, `Drop` impls, and vtables all live in the old
dylib:

1. `save()` **while the old lib is still loaded** → pointer-free bytes
2. Drop the entire old tree (old `Drop` impls run), still loaded
3. Unload (or don't — see below)
4. Load new lib, `App::fresh()`, then `restore(bytes)`

The classic bug is keeping the old tree alive across the swap as a safety net,
which guarantees destructors run against unmapped code.

**DECIDED 2026-09-24 (M32): what `fresh()` is not asked for.** A
stylesheet is code-shaped data and comes from the program's
*description* (`lumen_static_shell::program`), which is the one place the
window, every headless host and a swap all ask: `Host::adopt` re-asks the
description's sheet of the adopted application. `Application::style` and
the window's `Config::stylesheet` are gone — they were two sources, and
the window silently ignored the first (`docs/plan-program.md`).

**DECIDED 2026-09-24 (M36): a panic is caught, located and survived in
development builds.** One guard, `Host::guard`, covers every path that
runs application code; the screen is a display list naming the message,
`file:line:col` and the nearest application frame; state is kept without
rollback and retry is explicit. It is compiled in for debug builds and
release builds with `lumen-static-winit/reload`, and a release build
without it dies. Across a dylib the catch and the hook live **inside the
library**, because a `cdylib` has its own std and its own hook
(`docs/plan-panic-recovery.md`, `docs/step4-hot-reload.md`).

**DECIDED 2026-09-24 (M37): a window reloads itself, with the `Host`
in the library.** The tree holds code, so a loader-side `Host<A>` would
never change. The window drives a type-erased `Session`, and a hot
library exports one through `export_program!`. The boundary is
**borrowed pointers behind an ABI stamp** (compiler, target, profile,
features, layouts, and — since M49/T49.1 — a hash of the source of
every crate whose types cross, because sizes cannot see a reordered
enum), chosen by
measurement over a byte encoding (`docs/step4-hot-reload.md` T37.0). A
swap boots the new build's `program()` and restores per field. A failed
build, a refused library or a panicking boot keeps the running one.
Libraries are never unloaded (`docs/plan-window-reload.md`).

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
2. **Hot-reload unit** — one dylib, one `save`/boot/`restore` cycle,
   sub-second because the rebuild is one crate.
3. **Erasure boundary** — inside, `#[fuse]` monomorphizes freely; at the seam,
   kind dispatch. This is the overhead dial, and it is *local* rather than
   global, which is exactly what a feature flag cannot be (§8).
4. **Observability namespace** — a stable path like `app::sidebar::filter_row`
   that survives reloads. **DECIDED 2026-09-26 (M49/T49.11):** the stable
   path is the author id (`#id`, `#a > #b`). A node's *offset* survives
   only an edit that keeps the tree's shape; a window swap reports a shape
   change (`app.reloads` → `shape_changed`, `Session::shape`), so an agent
   knows to resolve again (`docs/plan-reload-hardening.md` §1.6).

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

**DECIDED (2026-09-25, M39/T39.1): the endpoint is a local line protocol
that refuses HTTP.** Newline-delimited JSON-RPC over TCP, one transport
(`lumen_static_agent::transport`) that every server calls, and a line
that is not a JSON object **ends the connection**. That is framing, not
policy: nothing reads an `Origin` or `Host` header, because every way a
web page reaches a local port begins with an HTTP request line, and no
request line is a JSON object — so a page the developer happens to have
open can no longer drive the application (R14-F1, S0). **Out of scope:**
another process running as the same user, which can already `ptrace` the
application. **A per-session token is deferred, not rejected**: it is the
only option that separates users on a shared host, and the plan's §4
names what reopens it. Per vector, before and after:
`docs/results-agent-transport.md`. **It binds loopback only** (T39.2):
a non-loopback address is refused unless `LUMEN_AGENT_ALLOW_REMOTE=1`,
which binds it and warns that there is no authentication.

**DECIDED 2026-09-26 (M48/T48.7): a stand-in is recognised by
structure.** `Node::node(i)` answers with an enclosing container inside a
branch that is not laid out, and the agent must not paint or probe that
container at `i`. It told them apart by address and kind, which a
same-kind wrapper at offset 0 defeated (`Glass<Glass<Label>>`, R13-F5).
Now `Node::node_count` — required under `observe`, emitted by the derive
and `leaf_common!`, one `node_count!()` line in a hand-written impl —
is compared with the table's subtree size at `i`: an ancestor's is
always larger (`lumen_static_agent::live_nodes_with`).

**DECIDED 2026-09-26 (M48/T48.2): `ui.waitSettled` answers from the
host, and "cannot say" is `null`.** It answered `true` in every state
under a comment saying no animation scheduler existed, long after
`App::tick`, timers and the task inbox made one (R13-F1). A host now
says: settled when nothing is animating, the inbox has no backlog, no
job is in flight or has results undrained (M47/T47.3's ledger, less
running subscriptions) and no timer is due within a frame;
the reply names each term. A host that cannot know answers `settled:
null` — not `false`, which would spin every poll to its timeout.
Polling stays the contract; a blocking wait is not built
(`docs/plan-agent-truth.md` §2.2, §4).

**DECIDED 2026-09-25 (M41/T41.4): observing must not change what is
drawn.** The audit's intrinsic probe measures every leaf through
`Node::probe`, and a widget whose `layout` writes state that paint reads
overrides it (`TextInput`, `TextArea`, `Label`, `Repeat`). Until then
an agent build reset every scrolled field it probed, which the
`Boundaries` default then painted (`docs/plan-boundaries-repair.md`
R1-F5).

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
*Amended 2026-09-26 (M48/T48.5):* "cannot drift" was true of the walk
and not of the JSON — the agent's `ui.getTree` carried a second
hand-written serialiser, and both dropped every state a widget publishes
(`toggled`, numeric value, `expanded`, `modal`, `hidden`, actions). One
function, `observe::node_json`, now serves both, with an equality test.

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
  *Qualified 2026-09-25 (M50/T50.1), and the qualifier closed 2026-09-27
  (M50/T50.2):* it holds for `pump`, for a block-glyph raster and for
  the truetype raster and `FontMetrics` (`docs/results-allocation.md`,
  "The truetype path, covered"). What remains outside it is stated
  there: a run that needs a fallback face, a glyph too large to cache,
  the threaded rasteriser's stated spawn budget (M50/T50.4), and wgpu's
  own per-frame encoding on the GPU path — what `lumen-static-wgpu` adds
  on top of it is gated at zero since M51/T51.4.
- **Const-folded rigidity** — a dynamic tree checks the flag at runtime per node
  per frame. The static version does not have the check at all.

**Implication for prioritization:** invalidation granularity, text caching, and
damage tracking deserve at least as much attention as the static tree. Do not
let the interesting part crowd out the effective part.

**DECIDED 2026-09-24 (M29, M34): the bounded strategy is the default.**
*Evidence re-gathered 2026-09-26 (M41/T41.7, `docs/plan-boundaries-default.md`
T34.2 as amended): 6 083 live frames judged by the redesigned oracle,
beside plain `Boundaries` legs; five probe defects it found are fixed, and
one unreproduced divergence (`OptionList`, `size`) is open in
`docs/plan-boundaries-repair.md` §6.*
`Boundaries<256>` is `Host`'s default and the window's (`everything` is a
diagnostic feature); a binding marks its own node, and a click on a
split application narrows too. The evidence is
`docs/plan-boundaries-default.md` T34.0–T34.2; the figures are
`docs/results-damage-precision.md` T29.3, T29.6 and T34.2.
**M41 is repairing what the flip exposed** (`docs/plan-boundaries-repair.md`):
T34.0's inventory missed `Host::compose`, which marked nothing, so an IME
commit was painted over a stale layout until M41/T41.1 made it mark the
focused subtree; the host's writes are now audited method by method in
`docs/plan-boundaries-default.md` T34.0. The other known unmarked writes
(a `Repeat`'s data, the agent's probe, a paste's measurer) are gated and
fixed by M41's later tasks, and the re-validation found the probe's class
in five more widgets (T41.7).
**DECIDED 2026-09-27 (M53/T53.2–T53.3, `docs/plan-frame-cost.md`): a
frame pays only for what it marked.** The arena cascade asks only the
marked nodes, and a pump with nothing marked, handled, applied or owed
is skipped outright under a strategy that can skip — no place, paint or
`finish`, damage `None` — with the shadow oracle judging skipped frames
too. An unmarked write to the tree, which paint used to hide, now paints
nothing at all. A window's own redraw with no damage presents nothing
(T53.4); a redraw the platform asked for always presents. Figures: `docs/results-damage-precision.md` T53.2 and
T53.3.

**DECIDED 2026-09-03 — confirmed by the first profile of a live frame**
(`docs/results-frame-cost.md`): layout and paint were 1 µs of a 190 ms frame.
Everything else was rasterisation — an 18-layer shadow and an unoptimised
dev build. The static tree's own cost is invisible; the calibration above
was right. `app.perf` now reports every stage plus input → presented
latency, and `scripts/ui_check.py latency` judges it against 16 ms / 100 ms.
*Amended 2026-09-26 (M48/T48.3):* that was true of counter-window only;
every turnkey window reported 0 for every stage until its redraw kept the
frame `present` returns (R13-F2). A stage nobody measured is now `null`,
and `ui_check.py` fails it as NOT MEASURED rather than passing on zeros.

### What actually kills GUI framework projects

Layout and traversal are the tractable, fun part, and roughly 15% of the work.
The years go to:

- **Text**: shaping, bidi, complex scripts, font fallback, IME composition,
  selection across all of it. *Status (M45):* font fallback **DECIDED** —
  a list of fallback faces, loaded the first time a run needs one
  (`docs/plan-text-correctness.md` §2.4, `docs/results-text.md` T45.3).
  Shaped paint **OPEN**: `lumen-static-text` shapes for measurement only,
  and the painter draws the unshaped string (§2.1 there; T45.4).
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
   **Extended 2026-09-19 (M27/T27.10):** of the three that are kept,
   `scroll` is the only one a *person* sets, and **`layout` may not write
   it.** Layout runs speculatively — the agent's audit measures every
   leaf twice per frame under 1e6 and 2e6 px boxes — so a clamp written
   back from a measurement destroys an offset nothing can recompute. The
   offset is clamped at use (`Repeat::offset`), the viewport comes from
   `place` rather than from whatever limits `layout` was last handed,
   and `Repeat` overrides `Node::probe` so a measurement does not pick a
   window. See `docs/plan-widget-parity.md` §T27.10 and
   `crates/lumen-static/tests/repeat_scroll.rs`.
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
8. **How far the paint vocabulary goes** — the spec never bounded `Cmd`, and
   "as capable as CSS" turns out to be two questions, not one.
   **DECIDED 2026-09-10.** CSS's *box* styling is closed-form and belongs in
   `Cmd`: per-corner radii, borders, inset and offset shadows, radial and
   conic gradients, blend modes, rounded clips, and backdrop effects
   including blur, refraction and specular. CSS's *vector* half — rotation
   and skew, `clip-path`, dashes and joins, SVG — is a Bézier pipeline, is
   not in `Cmd`, and plugs in at the `Renderer` seam if it is ever wanted.
   The measurement behind the line is `docs/results-vello.md`; the first
   feature built on the near side of it is `docs/plan-glass.md` (M19),
   extended by `docs/plan-liquid.md` (M22) — which adds Fresnel, an inner
   shadow, saturation, **chromatic dispersion**, a **backdrop-adaptive
   tint**, press deformation and a two-blob **morph** to the backdrop
   effect, and ports the whole of it to the GPU, where a full-screen
   filter costs 1.44 ms against the software canvas's 148
   (`docs/results-glass.md` §0h). Costs in `docs/results-glass.md`,
   and it is a **widget** rather than a `Style` field — parameters that
   only a handful of nodes per frame use do not belong in a struct
   resolved for every node, and paint order is something a position in
   the tree expresses and a style property cannot.
   **Layout is not part of this question** and never was: layout produces
   the rects a renderer consumes, so grid, wrap and min/max constraints are
   work in `ops.rs` that no renderer choice affects.

   **The first of those box features to land is the border, M27/T27.1,
   2026-09-18** — `Cmd::StrokedRect`, an *inset* stroked rounded rect,
   with `border_width`/`border_color` on `Style` and a role variant that
   follows the palette. Inset rather than outset is the load-bearing
   choice: a bordered and an unbordered control in the same row line up,
   where `ops::focus_ring`'s deliberately-outset underlay would widen
   one. `Painter::stroked_rect` is a **required** method, not a provided
   one, so every wrapper is a compile error until it forwards — the
   `Deferring` lesson, and an injection in
   `crates/lumen-static/tests/border.rs` reproduces that bug exactly by
   making it provided. It cost 16 B per node and +676 B of `.text`
   (`docs/results-embedded.md`).
9. **Appearance per kind** — should appearance leave the per-node `Style`
   for a look on each widget, a function of a palette that is data?
   **OPEN 2026-09-15**, planned as M25 in `docs/plan-appearance.md`. It is
   question 8's principle generalised — parameters few nodes use do not
   belong in a struct resolved for every node — from glass to every
   widget's colours, with layout staying cascaded. Two sub-questions are
   settled by that plan's T25.4 rather than here: where box fills live
   (per node, a wrapper widget, or a type-level look; its §2.3), and how an
   application sets one look for every instance of a kind (its §2.4). The
   plan's pilot is a stop point, with a much smaller change as the
   fallback, so this may close as "no".
   **Sub-questions DECIDED 2026-09-15 (M25/T25.4):** box fills stay per
   node — seven example rules fill derived containers and about twenty
   kinds read the fields, so only `color` and the two dims would leave,
   16 B a node rather than 40; and kind-wide colour comes from the
   application's palette, because every kind-wide colour rule in the
   examples is a palette role. Counts in `docs/results-appearance.md`.
   The question itself stays OPEN until the pilot.
   **DECIDED 2026-09-15: no.** The `Toggle` pilot tripped two of the
   thresholds written before it was built — RAM, and 1 000 toggles 27%
   slower — and bought only one thing a smaller change could not: a
   pressed look that changes hue. Appearance stays in `Style`; the colours
   one widget needs become fields on that widget, and a stylesheet can name
   a palette role (`docs/plan-appearance.md` T25.5, T25.10, T25.11).
   **T25.12 measured it again with the look cached, and the "no" stands**
   (2026-09-16). The pilot resolved its look on every layout, where the
   cascade it lost to resolves once. Cached the same way, the look still
   cost about four times the frame threshold, and probes put that cost in
   the cache check and the look-shaped paint, not in the widget's size
   (`docs/results-appearance.md` T25.12).
