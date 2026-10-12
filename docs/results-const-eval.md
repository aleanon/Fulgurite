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
   --> crates/fulgurite/src/widget.rs:192:19
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

`crates/fulgurite/tests/table_invariants.rs` now pins twelve cases:
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
matter at this margin.

**Current (M43/T43.5, 2026-09-25): 0.46–0.94 s** over five runs under
the bench lock (load average 3–7 while other builds ran, so LOADED), i.e.
~5× of headroom at the worst run. **What it measures changed**: a clean,
non-incremental build of the fixture crate alone (`cargo clean -p
const-budget`, `CARGO_INCREMENTAL=0`, its own target directory), and
the fixture now calls `wid_index!` three times, so the build also
evaluates `count_wid` over the whole table.

The previous figure, 0.13–0.21 s, timed an *incremental* rebuild after
touching `main.rs`. Incremental compilation reuses the const-eval of the
previous build, so a quadratic in the table builder (M38's R11-F5 plant,
with the `long_running_const_eval` lint allowed) was red on its first run
— 51.7 s — and green on every rerun. Under the new recipe the same plant
is red on the second and third runs (46.0 s, 46.1 s), and a quadratic
`count_wid` — which the old fixture never evaluated — is red on both
reruns too (68.2 s, 69.5 s): `scripts/selftest/plants/slow/`.

## F5.4: the accessor stays a linear scan — the planned fix was slower

`Node::node(i)` walks the fields accumulating an offset, so it is
O(fanout × depth), and five loops call it per index: the cascade, the
default styles, the AccessKit emitter, the agent's index and the
journal. The review measured it quadratic *when one container is wide* —
at 2,001 nodes in a flat-wide tree, doubling the node count cost 4.3× the
time — and the plan's F5.4 said to emit a const `CHILD_STARTS` array and
binary-search it.

**Built, measured, reverted.** Same fixture, same harness, one A/B:

| accessor | full scan over 901 nodes | `frame_done` |
|---|---|---|
| linear scan (kept) | **5.0 µs** | **44.8 µs** |
| binary search over a const offset table | 18.0 µs | 72.4 µs |

3.6× *slower*, and it made the agent's frame 60% more expensive. The
reason is visible in what each shape compiles to. At the fanouts real
containers have — 5 to 10 fields — the scan becomes a chain of compares
against constants, and each `Node::node(&self.field, ..)` is a direct
call on a known type that inlines. Binary search replaces that with a
genuine loop over an array and then a `match` on a runtime index, i.e. a
jump table and an indirect branch, in exchange for turning O(10) into
O(log 10).

Two things worth keeping from this:

- **The quadratic case is real but is not the case we have.** It needs a
  single container with many dozens of fields. Every container in the
  workspace has ten or fewer, and a struct with forty fields is a design
  problem before it is a performance one. If one appears, the crossover
  is worth re-measuring — not assumed in either direction.
- **A `const` array is materialized at each use.** The first attempt had
  `binary_search` copying the whole table to the stack per call; promoting
  it to a `&'static` reference in rodata changed nothing measurable, so
  that was not the cause — but it is a real footgun and the reason the
  first number was even worse.

An optimization that a plan asserted and measurement rejected. The plan
said so as a recommendation, not a finding; this is the finding.
