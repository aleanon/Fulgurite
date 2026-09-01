# Is the `unsafe` needed? — measured

*2026-08-31. The question from `docs/verification-notes.md`: the release
build's 13 `unsafe` blocks are all one pattern (`base.add(offset) as
*const W`). Is that pattern load-bearing, or a performance choice? Built
both and measured. Harness:
`cargo run --release -p benches --bin safety`; machine and profile as in
`docs/results-step2-crossover.md`.*

> **Reproducibility note, 2026-09-02.** The `safety` binary this document
> cites (and `memory`, below where cited) no longer exists in
> `benches/src/bin/`; only `crossover`, `fusion`, `repeater` and
> `bindings` do. The numbers here therefore cannot be re-run from this
> tree. This is the document that justified removing the unsafe surface. Under the project's own rule that a gate must be able to
> say no, a gate whose harness has been deleted cannot say anything.
> The result is retained as historical, with its reasoning; the
> comparison it records was between two implementations of which only one
> survives in the tree, so a faithful re-run would require reconstructing
> the other. Recorded rather than papered over. The per-iteration harness
> the numbers were taken with has since been replaced by a batched one
> (`bench`, 2026-09-02); its sub-100 ns rows in particular should be read
> as indicative — the 69 vs 75 ns small-size figure in the safe-prototype
> doc is inside that harness's own floor and should not be relied on.

## What was built

`crates/lumen-static/src/safe.rs` plus a `SafeTree` derive: the same
architecture with the *erasure mechanism* swapped.

| | unsafe path (shipped) | safe path (prototype) |
|---|---|---|
| reach a node's data | `base.add(desc.offset) as *const W` | derive-generated `node(i) -> &dyn Node`, a range-delegating match |
| find its behavior | `KIND_LIST[desc.kind]` | the vtable rides on the `&dyn` |
| allocation | none | none — a `&dyn` points into storage that already exists |

Both arms iterate a **byte-identical const table** (the prototype reuses
`<Self as Widget>::SUB_TABLE`), so the benchmark isolates exactly one
variable. Equivalence is asserted at startup: every node's size and
position agrees across both arms, for a 10-node const tree and a
991-node runtime slab.

## Numbers (median)

| nodes | unsafe | safe | safe + index rebuilt per frame | **safe + index cached** |
|---|---|---|---|---|
| 10 | 75 ns | **69 ns** | 89 ns | — |
| 1k | 6.33 µs | 9.80 µs (+55%) | 9.38 µs (+48%) | **7.30 µs (+15%)** |
| 100k | 684 µs | 991 µs (+45%) | 959 µs (+40%) | **741 µs (+8%)** |

Reading:

- **At small sizes the safe path is faster** (69 ns vs 75 ns). The
  accessor is a match over concrete types, so the compiler devirtualizes
  and the `KIND_LIST` indirection disappears.
- **Naively, at scale, it costs 45–55%.** The delegation chain
  (`Slab → Card → Item → leaf`) is walked *four times per frame*, once
  per pass phase (down / up / place / paint).
- **Resolving the tree into `&[&dyn Node]` once and reusing it costs
  8% at 100k nodes, 15% at 1k.** This is the honest configuration for a
  host: a static tree does not move between frames, so the index is built
  at startup and invalidated only by a reload. Rebuilding it *per frame*
  recovers almost nothing (the chain walk dominates either way) — the win
  comes from caching across frames, not from batching within one.

Cost of the index: one fat pointer (16 B) per node, against 24 B/node of
table and 32 B/node of state — roughly +20% node memory. On the embedded
profile that cares most about verification, node counts are small enough
for this to be noise.

## The 8% is an upper bound

The prototype reuses the *unsafe path's* `NodeDesc` so the tables match
byte for byte. A real safe design would drop `kind: u16` and
`offset: u32` — neither is used — shrinking `NodeDesc` from 24 to 18
bytes and improving the safe arm's cache behaviour in the passes that
stream it. That was left on the table deliberately to keep the comparison
honest.

Also worth weighing against §11's own calibration: in a mature retained
GUI the tree walk is 5–15% of frame time. Eight percent of that is well
under 1% of a frame.

## Memory: measured, and it splits by *kind* of memory

Speed was only half the question. Sizes are measured
(`cargo run --release -p benches --bin memory`) and code sizes come from
symbol sums over two isolated binaries that link one path each
(`fixtures/binsize`).

### Per-node data

| | unsafe | safe |
|---|---|---|
| `NodeDesc` | **24 B** | **16 B** — `kind` and `offset` are both dead once the vtable rides on the `&dyn` and addressing is by index. *Now 48 B: M1 added `wid: &'static str` and `classes: &'static [&'static str]` (two fat pointers, 32 B) for the cascade. Measured 2026-09-02.* |
| `KIND_LIST` | **8 B/node** | **0 B** |
| `NodeState` | 32 B | 32 B |
| resolved index | — | +16 B/node, *only if cached* |

The kind list costs a full pointer per node, not per kind: derived kind
lists are not deduplicated (`docs/step2-design-notes.md` §7), and for
every shape in the benchmark `KIND_COUNT == NODE_COUNT` exactly. So the
safe path halves per-node static data, 32 B → 16 B.

### Code size (90-node tree, one path per binary)

| | framework code |
|---|---|
| unsafe | **3,948 B** |
| safe, accessor inlined (default) | 6,355 B (+61%) |
| safe, accessor `#[inline(never)]` | **4,758 B (+20%)** |

The inlined accessor is a single 1,914 B symbol for the 90-node root:
rustc inlines the nested `node()` calls, so accessor code becomes
O(`NODE_COUNT`) per *root* rather than O(fields) per *type*. That is a
faint echo of §4's monomorphization trap, and it has a switch —
`#[inline(never)]` on the generated accessor collapses it to 401 B across
three symbols, one per component type, restoring "code scales with how
much UI you wrote, not how much tree you built".

The switch is a real dial, measured both ways
(`--features safe_no_inline`):

| | 10 nodes | 100k cached |
|---|---|---|
| unsafe | 75 ns | 684 µs |
| safe, inlined | **69 ns** | 741 µs (+8%) |
| safe, out of line | 85 ns | 783 µs (+13%) |

### Putting it together

For a 90-node tree, out-of-line accessor, slim descriptor:

- **flash**: unsafe `3,948 + 2,160 + 720 = 6,828 B`; safe
  `4,758 + 1,440 + 0 = 6,198 B` — safe is ~9% smaller.
- **RAM**: identical at 32 B/node — *unless* the index is cached, which
  adds 16 B/node (+50% RAM).

The break-even is around **51 nodes**: the safe path pays ~810 B more
code (for three component types) and saves 16 B per node of static data.
Below that, unsafe uses less flash; above it, safe does, and the gap
widens with node count.

### The embedded reading

On the profile that most wants verification, this lands well:

- flash is where tables live, and the safe path wins there past ~50 nodes;
- RAM is unchanged, because the index is exactly the thing you *don't*
  cache when RAM is scarce;
- and uncached is the right choice there anyway — at small node counts
  the safe path is *faster* than the unsafe one (69 ns vs 75 ns at 10
  nodes), so the configuration that costs no RAM is also the quick one.

The 8%-at-100k figure is the desktop trade: spend 16 B/node of plentiful
RAM on a cached index. The two profiles want opposite settings of the
same dial, and neither setting requires `unsafe`.

## What the safe path deletes

Not just the `unsafe` keyword:

- **all 13 `unsafe` blocks** in a release build;
- **`KIND_LIST` entirely** — rustc dedups vtables per type, so §5's "one
  body per widget *kind*, not per tree *position*" becomes a language
  guarantee instead of something our `splice_kinds` machinery maintains;
- **`kind` and `offset` from `NodeDesc`** (6 bytes/node), and the
  kind-index rebasing from the derive;
- **the `RefCell` in `Repeat`** — untested here, but that interior
  mutability exists *only* because ops receive a shared base pointer; a
  `node_mut` accessor lets layout mutate a window directly, retiring a
  panic obligation as well.

Everything distinctive survives untouched: const table, const
`NODE_COUNT`, zero allocation, flat stack-free passes, rigid/flexible
partitioning, fusion, repeater virtualization. `fuse.rs` was already
`unsafe`-free — the safe path is that style generalized to every seam.

## Not yet ported

`Repeat`, `Button`/event dispatch, and the `observe` surface are still
unsafe-path only. Dispatch is the interesting one: the borrow must be
sequenced (copy the handler `fn` pointer out, *then* take `&mut S`),
which is exactly what `Button::event_op` already does — so it should
port, but it has not been demonstrated.

A structural limit worth recording: the accessor chain cannot be
flattened into a single jump table, because a derive sees only its own
fields' *types*, not their internals. Delegation depth is therefore
inherent to the per-type macro model — which is precisely why the cached
index matters.

## Verdict

**The `unsafe` is a performance choice, not a necessity, and the price
of removing it is ~8% of traversal at 100k nodes with a cached index —
under 1% of a realistic frame.** For a project whose stated long-term
goal is formally verified applications, deleting the entire unsafe
surface, the kind list, and an interior-mutability panic obligation for
that price looks like a good trade. It also lowers the ceiling on
tooling: Kani over safe code and compile-time table assertions would
suffice, where the current design needs Verus-class pointer-permission
reasoning.

**Status 2026-09-01: adopted, then overtaken.** The safe path was
adopted (`docs/layout-and-storage-decisions.md` §0); the follow-up
traversal measurement (`docs/results-traversal.md`) found recursive
descent over `&dyn` children faster than *both* the unsafe table and this
indexed accessor — so the accessor survives for index addressing (agent
surface, journal, audit) while layout/paint recurse. The port is
`docs/plan-architecture-v2.md` M0.
