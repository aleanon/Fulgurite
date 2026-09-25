# Formal verification: what this architecture makes easy, and what it does not

*2026-08-31. Exploratory assessment, not a commitment. Scope is the
**release build only** — `observe` and `reload` off, so no serde, no
accesskit, no agent crate, no dylib, no FFI. Those surfaces are dev-only
and out of scope by construction.*

## The release surface is small

With both features off, the entire shipped core is: `desc` (the table
types), `widget` (const splicing), `pass` (three layout loops + paint +
dispatch), `hit`, `ops` (containers + `Label`/`Fixed`/`Spacer`/`Button`),
`fuse`, `repeat`, `assemble`, `event`, `inbox`. That is roughly 1.5k
lines with **17 `unsafe` blocks, all in two files** *(counted here on
2026-08-30; `results-safe-prototype.md` counts 13 the next day, after the
prototype removed four — both are right about their own moment, and both
are now zero)*, and every one of
them is the same pattern:

```rust
let w: &W = unsafe { &*(base.add(d.offset as usize) as *const W) };
```

A verification effort has one central obligation, not a scattered set.

## What is unusually favourable here

- **The node table is a `const`.** Structural facts — parent precedes
  child, sibling chains terminate and cover `child_count`, `kind` is in
  range for `KIND_LIST` — are properties of a compile-time constant, not
  of a runtime heap. They can be discharged by const-eval, today, with
  no external tool (see "Tier 0").
- ~~**No recursion, no allocation, no dynamic tree.**~~ **Corrected
  2026-09-01:** the passes are now recursive descent
  (`docs/results-traversal.md`), and that is *better* for verification,
  not worse — bounded structural recursion gives proofs induction over
  the tree, which Kani and Creusot handle more naturally than flat-loop
  index invariants. Depth is a compile-time `DEPTH` constant with a cap,
  so termination and stack bounds are facts about a constant. No
  allocation and no dynamic tree still hold.
- **First-order dispatch.** Ops are monomorphic `fn` pointers, and after
  `docs/events-decision.md` handlers are `fn` pointers too — no closures,
  therefore no captured environments to model. Verifying dispatch does
  not require higher-order reasoning.
- **`NODE_COUNT` is const**, so `[NodeState; N]` is fixed-size: a proof
  can be parametric in `N` or instantiated at a concrete `N` without
  modelling growth.
- **Determinism.** No clock, no threads, no I/O in the core. `Inbox` is
  the only concurrency and it sits at the seam by design.
- **Two invariants are already load-bearing and already stated**:
  children never escape their parent, siblings never overlap. Hit testing

> **Restated 2026-09-02 (F4.8).** "Children never escape their parents"
> has been false since Step 3 — a repeater's window overruns its viewport
> by construction. The invariant is *scoped*: content may extend beyond a
> clipping ancestor and is clipped there; it may not extend beyond a
> non-clipping one. What is checked, in
> `crates/lumen-static/tests/paint_containment.rs`, is the consequence
> that matters — for every drawing command, its rect intersected with
> every enclosing clip lies inside the root. Over a corpus crossing
> `{Repeat, List, Dyn, Either, plain}` x four scroll positions x three
> viewport shapes x squeezed and free. Verified to fail, naming the
> case, when `Repeat`'s clip is removed.
  *depends* on them (`hit.rs` picks the last containing node in DFS
  order), and the audit reserves `W1003`/`W1004` because of them. Today
  both are pinned by one test on one fixture.

## What is hostile

1. **The `base + offset` cast is the whole design.** Its correctness
   rests on a triple — table, kind list, and the concrete type at each
   offset — agreeing, which the derive establishes and nothing re-checks.
   This is where proof effort should concentrate, and it is not trivial:
   it needs `offset_of!` semantics and a story about what makes a
   `(base, table, kinds)` triple *coherent*.
2. **f32 layout arithmetic.** Decidable in SMT but expensive, and several
   interesting properties (containment, non-overlap) are inequalities
   over floats. A fixed-point or rational abstraction for the verified
   profile would cost less than proving over IEEE-754.
3. **`RefCell` in `Repeat`.** Dynamic borrow checking is a panic
   obligation, and `Vec` growth breaks the no-allocation story. A
   verified profile wants a fixed-capacity window (`[T; N]`), which is
   what the embedded profile wants anyway.
4. **The derive is a proc macro.** Verifying generated code means either
   verifying the generator (hard, low value) or checking each expansion
   (cheap, high value — see Tier 0).
5. **Tooling reality check.** `cargo miri` is currently unusable in this
   environment: it fails intercepting build scripts of any proc-macro
   dependency (`quote`, `proc-macro2`) with "contains outdated or invalid
   JSON", independent of target dir, toolchain pin, or explicit
   `--target`. Making `lumen-static-macros` an optional dependency (so a
   hand-written-table profile has no proc macros at all) would sidestep
   it, and is defensible for the embedded profile regardless.

## A tiered plan, cheapest first

### Tier 0 — proof-carrying tables (no tools, do this first)

`benches/tests/equivalence.rs::tables_are_well_formed` checks table
structure at runtime, for the shapes the tests happen to build. The same
predicates are `const`-evaluable. Move them into a `const fn
check_table(&[NodeDesc]) -> bool` and have **the derive emit
`const { assert!(check_table(&TABLE)) }` alongside every table it
generates**.

That single change converts "trust the macro" into "the compiler
re-checks the macro's output at every expansion, in every downstream
crate, forever" — proof-carrying code, and it costs nothing at runtime.
It fits §5's spirit exactly: facts the compiler already knows should be
facts the compiler checks.

> **Tightened 2026-09-25, M41/T41.6.** The check that shipped as
> `table_is_well_formed` asked that a parent *precede* its child and let
> a last child's `next_sibling` point at "a later node". Both were
> weaker than what `subtree_range` and every child-chain walk assume, so
> it accepted four tables whose ranges were wrong or whose sibling
> chains ran into an uncle (R2-F5). It now requires DFS pre-order (node
> `j`'s parent is `j - 1` or an ancestor of it) and a last child's link
> to be 0. The review's derive mutation I3 — a last child linked to its
> parent's end — compiled everywhere under the old check and now fails
> with 52 const-evaluation errors across 18 targets.
> `crates/lumen-static/tests/table_invariants.rs` holds the four
> counterexamples, judged against descendants computed from parent
> links alone.

### Tier 1 — bounded model checking of the passes (Kani)

Harnesses over small symbolic tables (3–6 nodes) proving, for all inputs:

- no panic (every index in range, no arithmetic overflow);
- **containment and non-overlap** — promoting the two invariants above
  from one fixture to all tables, which is the highest-value item on this
  list because `hit_test`'s correctness and the audit's reserved codes
  both rest on them;
- `hit_test` returns the deepest containing node, against a naive
  reference implementation;
- **`layout_partitioned` ≡ `layout`** when rigid caches are warm. Today
  this is an equivalence tested on one 45-card fixture; it is really a
  theorem, and equivalence of two implementations is exactly what a
  bounded checker is good at;
- **fused geometry ≡ unfused geometry** — same argument, currently tested
  on two shapes;
- dispatch reaches at most one handler, and `focus` is always a valid
  index or `None`.

Kani handles raw pointers and can check for UB, which matters given the
central obligation. Floats are the likely friction point.

### Tier 2 — the coherence contract, made checkable

Name the predicate the `unsafe` blocks rely on ("this `(base, table,
kinds)` triple was produced by one derive/assembly for this type") and
give it a witness: a constructor that is the *only* way to obtain a
`Tree<'_>` handle, produced either by derive-generated code or by one
audited `unsafe` call in `assemble`. The passes then take the witness
instead of three loose arguments, and every op's safety comment reduces
to "the witness guarantees it". This is design work, not proof work, and
it makes any later proof much shorter.

### Tier 3 — deductive verification of layout (Creusot/Verus)

Only worth it after Tiers 0–2 and after the float question is settled.
Verus is the better fit than Creusot/Prusti here because it models raw
pointers with permission tokens, which is what the `base + offset`
pattern needs; the cost is writing the passes in its subset.

### Not worth verifying

The dev surface (`observe`, agent, journal, audit), the reload path
(dylib + FFI is unverifiable at that boundary by construction), and the
benchmarks. Excluding them is not a compromise: they are compiled out of
a release build.

## Why this might be worth doing at all

The niche where this architecture is genuinely differentiated —
microcontroller-class targets with a fixed memory budget, no allocator,
and `NODE_COUNT` known at compile time (§11's "the two advantages that
are real") — is the same niche where formal verification has commercial
pull: avionics, medical, automotive. A statically-allocated UI whose
layout pass is proved panic-free and whose table structure is checked by
the compiler is a defensible product claim in a way that "fast tree
walk" is not.

## Status (2026-09-01)

The central question — is the `unsafe` load-bearing? — is **answered: no**
(`docs/results-safe-prototype.md`). A safe `&dyn`-accessor path exists
behind a benchmark, is equivalence-checked against the shipped path, and
costs +8% traversal at 100k nodes while *saving* static memory and
deleting the entire unsafe surface.

**Update 2026-09-01: adopted, and superseded upward.** The safe
accessor path became the target, and the traversal measurement then
showed recursive descent through `&dyn` children is faster still while
being equally `unsafe`-free. `docs/plan-architecture-v2.md` M0 ports the
core to it and adds a zero-`unsafe` gate on the release build. The
verification *push* itself remains unscheduled; the surface it would
verify is now the intended shape rather than a prototype.

## Present state (updated 2026-09-01)

**The release core now contains zero `unsafe`.** M0 of
`docs/plan-architecture-v2.md` replaced the `base + offset` mechanism
with recursive descent over `&mut dyn Node`, and `just no-unsafe` is a
standing gate. Tier 0 also landed early: the derive emits
`const { assert!(table_is_well_formed(&TABLE)) }` and
`assert!(DEPTH <= MAX_DEPTH)` with every table, so the compiler
re-checks the macro's output at every expansion in every downstream
crate, and a tree deep enough to overflow the stack does not compile.

What remains for a verification push is Tier 1 (Kani over the recursive
passes — now structural induction rather than index invariants) and
Tier 2's coherence contract, which shrank considerably: with no raw
pointers there is no `(base, table, kinds)` triple left to keep
coherent.

## Historical: present state before M0

`fixtures/miri-smoke` is a release-shaped harness (no features, no
dev-dependencies) that exercises every `unsafe` path in one `main`:
layout, partitioned layout, paint, fused `Inline`, hit testing, dispatch
including the aliasing-critical handler sequence, a scrolled repeater,
runtime assembly, and the inbox. It runs clean under ordinary execution
and is the entry point for `cargo miri run -p miri-smoke` once the
tooling blocker above is resolved.
