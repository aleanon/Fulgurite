# Step 2 design notes — node table + traversal

*2026-08-30. What was built, where it deviates from the handoff §5 sketch,
and why. Companion measurement record: `docs/results-step2-crossover.md`.*

## Crate layout

- `crates/lumen-static` — `NodeDesc`/`WidgetOps`/state types, const splice
  helpers, the iterative passes, built-in leaves (`Label`, `Fixed`,
  `Spacer`), row/column math, runtime assembly (`Assembled<C>`).
- `crates/lumen-static-macros` — `#[derive(StaticTree)]`.
- `benches` — dyn baseline, shapes, the `crossover` harness binary.

## Decisions and deviations from the §5 sketch

### 1. `NodeDesc` gained `parent` and `next_sibling` (24 bytes total)

The handoff's sketch has `offset/kind/first_child/child_count/flags`. The
stack-free passes need two more index fields:

- `parent` — the reverse (size-up) loop and the partitioned pass relate a
  node to its parent without a traversal stack;
- `next_sibling` — in DFS order a child's subtree intervenes, so
  `first_child + k` does not address child `k`; sibling links let a
  container op walk exactly its children. `0` means "last child" (slot 0 is
  always the root, never a sibling).

### 2. Dispatch is per *container* per phase, not per node-visit

> **SUPERSEDED 2026-09-01** (`docs/results-traversal.md`). This rule was
> the best flat-loop shape, and it beat naive dyn recursion; recursive
> descent over `&dyn` children then beat *it* by 34–39%, because the flat
> path's `desc → kind → vtable` indirection costs more than the dispatch
> granularity saves. Kept as the record of the intermediate result.

The first cut dispatched per child (each child pulled constraints through
its parent's ops; each placement advanced a cursor through dispatch) and
**lost to the naive dyn tree at every size** — ~3 indirect calls per node
against dyn recursion's 2 virtual calls, plus redundant recomputation of
the same per-container constraint split once per child. Reworked so a
container op is dispatched once per phase and loops its own children
inline (dispatch-free inner loop), leaves dispatch only in the up phase.
That flipped the benchmark; both variants' numbers are in the results doc.
This is the load-bearing implementation rule for anyone touching the
passes: **the unit of dispatch is the container, not the node-visit.**

### 3. Layout is three flat loops (down / up / place)

Constraints down (forward over containers), sizes up (reverse over all
nodes), positions down (forward over containers). DFS pre-order makes each
loop's data dependency already satisfied when a node is visited. No
recursion anywhere; §2's "no node reads its own final position during
layout" holds by construction.

### 4. Composition is concrete structs only (for now)

Every container is a `#[derive(StaticTree)]` struct; all `Widget` impls
are on concrete types, so every associated const (`NODE_COUNT`,
`SUB_TABLE`, …) is concrete and **stable Rust suffices** — the
`[NodeDesc; Self::NODE_COUNT]`-in-a-trait form the handoff sketches needs
`generic_const_exprs`. Tuple-container sugar (`Row<(A, B)>`) is the DSL
layer §11 already prices in; deferred.

### 5. `NodeState` is 32 bytes and streamed

`constraints + size + pos`. The first cut also carried an accumulator and
a cursor (60 bytes); per-container child walks made both redundant. The
passes are bandwidth-bound at 100k nodes, so state width is performance.

### 6. Values live in the fields, reached via `base + offset`

> **SUPERSEDED 2026-09-01.** Values still live in the fields (§3 holds);
> they are reached by *recursion* now, not by a byte offset and a cast.
> The release core has no `unsafe` at all.

Ops fns cast the tree base pointer plus `desc.offset` to their concrete
widget type (§3 made real). Safety contract documented on `WidgetOps`:
ops may only be dispatched against tables produced by the same
derive/assembly. The paint test mutates a field and asserts the pass sees
it — no copies of values exist anywhere.

### 7. `KIND_LIST` concatenates with duplicates

> **SUPERSEDED 2026-09-01.** `KIND_LIST` is gone: with recursion the
> vtable arrives on the `&dyn` reference, so rustc's own per-type
> deduplication provides §5's "one body per kind" guarantee. `NodeDesc`
> lost `kind` and `offset` with it (24 → 16 bytes).

A derived struct's kind list is its own ops followed by its fields' lists,
un-deduplicated; spliced child tables get their kind indices rebased by a
running offset. Duplicates cost `8 bytes × repetition` of table, never
correctness. Deduplication (or the §12.1 enum hybrid) is a later size
optimization; runtime assembly already shares one kind list across all
instances of a component, which removes the only place the duplication
would have scaled.

### 8. Spacing is a `#[widget(...)]` attribute constant

Baked into the derive's wrapper fns (fn pointers can't capture). Reading
it from a field instead is the right end state once containers have
styling values; it changes the wrappers, not the architecture.

### 9. Rigid/flexible partition: const arrays, subtree-granular

> **SUPERSEDED 2026-09-01.** The partition was a flat-loop optimization
> (index arrays feeding a partitioned pass). With recursion the
> equivalent is "do not descend into a rigid or clean subtree" — one
> early return — which M2 picks up along with `Length`.

`count_partition`/`collect_partition` are const fns; at a concrete root
the arrays are compile-time (`crossover.rs` proves it with a
`const`-context assertion). Container rigidity is the AND of its
children's, so a flexible node's ancestors are all flexible and the
partitioned pass never re-enters a rigid subtree. Measured: the partition
pays off only when the rigid *fraction* is high — at 18% rigid leaves the
index indirection costs more than the skipped dispatches save. Numbers in
the results doc. Note what this is not: it skips provably-fixed-size
nodes, not unchanged-this-frame nodes — invalidation (§11's emphasis) is
separate, later work.
