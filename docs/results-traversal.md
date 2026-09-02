# Flat loops vs recursive descent — measured

> **Machine.** i9-13900KF, governor `powersave`, turbo on, `tsc`
> clocksource, rustc 1.94.0. Absolute figures are not portable across
> machines or governors; ratios taken inside one run are. Recorded from
> 2026-09-02, when it turned out no results doc had said.

*2026-09-01. Prompted by a correction: iced's layout is single-pass, and
the four-pass flat scheme in `docs/layout-and-storage-decisions.md` §2
was buying back exactness that recursion gets for free. This measures the
traversal strategy itself. Harness: a `benches` binary named `safety`, **since deleted** — see the
reproducibility note below; machine and profile as in
`docs/results-step2-crossover.md`.*

> **Reproducibility note, 2026-09-02.** The `safety` binary this document
> cites (and `memory`, below where cited) no longer exists in
> `benches/src/bin/`; only `crossover`, `fusion`, `repeater` and
> `bindings` do. The numbers here therefore cannot be re-run from this
> tree. This is the document that overturns handoff §5. Under the project's own rule that a gate must be able to
> say no, a gate whose harness has been deleted cannot say anything.
> The result is retained as historical, with its reasoning; the
> comparison it records was between two implementations of which only one
> survives in the tree, so a faithful re-run would require reconstructing
> the other. Recorded rather than papered over. The per-iteration harness
> the numbers were taken with has since been replaced by a batched one
> (`bench`, 2026-09-02); its sub-100 ns rows in particular should be read
> as indicative — the 69 vs 75 ns small-size figure in the safe-prototype
> doc is inside that harness's own floor and should not be relied on.

## The correction that started it

iced lays out **each node exactly once**. Its flex `resolve` iterates the
child *list* twice — separating `Fill` children from the rest — but calls
each child's `layout()` a single time. It can do that because it
**recurses**: the parent gets the child's true final size back from the
call, so a squeezed label's re-wrapped height is folded into the parent's
size with no estimate and no correction pass.

Flat loops cannot do that. A top-down loop reaching a container has no
way to ask "how big is this child?" without descending, so the answer has
to come from a separate bottom-up pass — which is the *only* reason the
plan had grown to four passes.

## Numbers

Identical shapes, identical layout math, geometry equivalence asserted
across all three arms at startup.

| | 10 nodes | 1k | 100k |
|---|---:|---:|---:|
| flat, unsafe table (shipped) | 74 ns | 6.47 µs | 690 µs |
| flat, safe accessor + cached index | — | 7.15 µs | 808 µs |
| **recursive `&dyn`** | **53 ns** | **3.99 µs** | **423 µs** |
| recursive `&dyn`, devirtualization defeated | 59 ns | 4.30 µs | 456 µs |
| `#[fuse]` (monomorphized recursion, from Step 6) | 43 ns | 2.46 µs | 249 µs |

**Recursive descent beats the flat table by 34–39% at every size**, and
it is `unsafe`-free.

### The devirtualization check

A `&dyn` coercion from a concrete field is devirtualizable — LLVM can see
the type and inline through it, which would have made this fusion in
disguise. Two checks say it did not:

- **Code size is O(kinds), not O(nodes)**: 9 symbols, 4,230 B — one per
  (type × method). `SafeCard::layout` is 562 B while `SafeItem::layout`
  exists separately at 579 B, so container→container calls are genuinely
  indirect. (Leaf calls are partly inlined; that is ordinary.)
- **Forcing opacity barely moves it.** `black_box` on every child
  reference (feature `rdyn_opaque`) defeats devirtualization completely,
  and the gap only goes 39% → 34% at 100k. Both figures are stable across
  runs (423.0/423.3/423.5 µs and 456.3/457.6 µs).

The knob is resolved when the **macro crate** is compiled, so emitted
code carries no `cfg` at all. The first attempt put `#[cfg(feature =
...)]` in the derive's output and a helper fn at the call site — the
first tripped Step 5's rule that a cfg in derive output resolves against
the *destination* crate, and the second turned into an optimization
barrier that contaminated the default arm (423 → 511 µs). Both are worth
remembering: derive output must be cfg-free, and a measurement harness
must not perturb the thing it measures.

## Why recursion wins

Not the pass count — the *indirection*. Per node per phase, the flat arm
loads `table[i]` (24 B), then `d.kind`, then `kinds[kind]`, then the ops
pointer: three dependent loads before the call. Recursion carries the
vtable **in the reference it already holds** and reads no `NodeDesc` at
all — the tree structure is in the call graph, and child state offsets
are consts.

So the flat table's central mechanism turns out to be pure overhead for
traversal. It remains useful for the *agent surface*, which is
index-addressed by design, but the passes do not need it.

## What this overturns

Handoff §5 — the central DECIDED section — chose flat iterative loops
over recursive descent, on the strength of i-cache locality, prefetching
and no stack depth. **That premise is now measured false for this
workload**, at every size tested, and Step 6 already contained the clue:
`#[fuse]` is recursion with static calls, and it beat the flat table 2.8×
without anyone reading it as a verdict on traversal.

## What it makes better, beyond speed

- **Single-pass layout.** iced's exactness, with no intrinsic-estimate
  pass and no correction pass. `Length` distribution needs the two cheap
  child-list scans inside a container, not extra walks of the tree.
- **`#[fuse]` stops being a special case.** Fused = static calls,
  unfused = `&dyn` calls, the same generated code shape. The dial becomes
  "monomorphize this subtree" rather than a separate lowering.
- **Invalidation gets easier**, which matters most (§11): "do not descend
  into a clean subtree" is one early return, where the flat scheme needs
  precomputed index partitions. The optimization the handoff says is
  *the* effective one is more natural here.
- **Verification gets easier**, correcting `docs/verification-notes.md`:
  bounded structural recursion gives proofs induction over the tree,
  which is friendlier to Kani and Creusot than reasoning about flat-loop
  index invariants. That note listed "no recursion" as favourable; it had
  it backwards.

## What it costs

- ~~**Stack depth proportional to tree depth**~~ — **closed 2026-09-01**:
  `Widget::DEPTH` is a const the derive computes and asserts against
  `MAX_DEPTH`, so a tree deep enough to be a hazard fails to compile.
- **The rigid/flexible partition does not port directly.** It was a
  flat-loop optimization (index arrays). Its replacement — skip a clean
  or rigid subtree by not recursing — is strictly simpler, but it is
  different code.
- ~~Fusion is still 1.7× faster~~ — **re-measured after M0**: on the real
  recursive path fusion is 429 µs against 435 µs for `&dyn` calls at
  100k, i.e. within noise. The 1.7× was an artifact of comparing fused
  code against the *flat table*. `#[fuse]`'s remaining value is code size
  and embedded control, not speed; re-price it before investing further.

## Verdict

**Adopt recursive descent as the traversal strategy**, together with the
safe accessor path already decided. It is faster at every size, removes
the last reason to keep the unsafe base+offset mechanism in the hot path,
makes layout genuinely single-pass, and simplifies both invalidation and
verification.

`docs/layout-and-storage-decisions.md` §2's four-pass scheme is
**superseded**: with recursion, `Length` needs one visit per node.

Not yet ported: `Repeat`, event dispatch, `observe`. The flat table stays
for the agent surface. The `rdyn_opaque` feature is retained as a
measurement tool.
