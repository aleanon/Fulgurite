# The const-eval ceiling — measured

*2026-09-02, plan `docs/plan-review-fixes.md` F5. Harness:
`just const-budget` (`fixtures/const-budget`, 3,711 nodes, release
feature shape). Machine: i9-13900KF, governor `powersave`, turbo on,
rustc 1.94.0.*

## Why this has a results doc at all

Everything else measured in this project is a runtime cost: something is
slower than it should be, and the fix is an optimization. This one was a
**ceiling on what the framework could express**. `table_is_well_formed`
is `const`-asserted at every `#[derive(StaticTree)]` expansion — that
assertion is the whole of "a malformed table does not compile", one of
the design's stated guarantees — and it rescanned the entire table once
per node to count children. O(n²), paid by `rustc`.

| nodes | before (O(n²)) | after (O(n)) |
|---|---|---|
| 3,200 | ~20 s | — |
| 3,711 | **does not compile** | **0.13–0.21 s** |
| 10,000 | **does not compile** | — |

"Does not compile" is not hyperbole: rustc's `long_running_const_eval`
lint is deny-by-default and fires, so the crate is rejected. Reproduced
here by restoring the old body and rebuilding the fixture:

```
error: constant evaluation is taking a long time
   --> crates/lumen-static/src/widget.rs:192:19
    |
192 |         while k < t.len() {
```

Handoff §8 names "forty component sub-tables" as the large-desktop
target. That was not reachable, and nothing said so — the failure only
appears once someone writes a tree that big, and every fixture in the
repo was under 100 nodes. It was found by a review agent building one.

## The fix

The table is DFS pre-order with sibling links, so the property verifies
in one pass over each node-*as-child* rather than a rescan per node:

1. every node but the root has a parent that precedes it;
2. the declared child counts sum to `n - 1`;
3. each parent's `first_child` is `n + 1`, and walking its sibling chain
   `child_count` times yields nodes that all claim it.

(2) and (3) together are equivalent to the old count-matching scan: if a
node claimed a parent it was not chained to, that parent's chain would
come up short and (3) would reject, and if a chain were over-long the
totals in (2) could not balance. Each non-root node is visited exactly
once as somebody's child, so the whole check is linear.

## What was also wrong, and is the more interesting half

The old implementation had **no test that could fail**. Its only
exercise was one well-formed table asserted `true`, so every `return
false` in it was unreached. A mutation making the function return `true`
unconditionally passed the entire suite — silently disabling
proof-carrying tables while every gate stayed green.

`crates/lumen-static/tests/table_invariants.rs` now pins twelve cases:
empty, root-with-parent, child-before-parent, over- and under-stated
child counts, misplaced `first_child`, a sibling link leaving the table,
a backwards link (a cycle the linear walk must not spin on), an orphan,
a child claimed by the wrong parent, and a 200-deep chain accepted.
**Eleven of the twelve fail against the always-`true` mutation.**

That the check got faster matters less than that it can now be shown to
work. A guarantee nothing can falsify is not a guarantee.

## Threshold

`just const-budget` builds 3,711 nodes and fails past **5 s**. A budget,
not a comparison: the failure mode is a hard compiler limit, so a wide
absolute threshold is the right instrument and machine variance does not
matter at this margin. Current: 0.13–0.21 s, i.e. ~25× of headroom.

## Not fixed here

`Node::node(i)` is still O(fanout × depth) — a linear scan accumulating
an offset — and five callers loop over it. That is a *runtime* cost, it
is not on the release frame path after F2.2, and it is F5.4.
