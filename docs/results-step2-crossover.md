# Step 2 — static table vs `Box<dyn Widget>`: measured crossover

> **SUPERSEDED 2026-09-02.** The numbers below describe the *flat-table*
> implementation that `docs/results-traversal.md` replaced with recursive
> descent. Re-measured on the shipped code with the batched harness
> (`bench` rewritten 2026-09-02; machine: i9-13900KF, `powersave`, turbo
> on, `tsc`), two runs:
>
> | nodes | static | dyn | |
> |---|---|---|---|
> | 10 | 103–125 ns | 113 ns | **indistinguishable** |
> | 1k | 8.8 µs | 12.9 µs | static −31% |
> | 100k | 0.90 ms | 1.31 ms | static −31% |
>
> **There is no crossover.** The 10-node result this document reports as
> "dyn −16%" is now a toss-up within run-to-run noise, and static wins at
> every larger size. Four later docs cite this one as their machine and
> profile anchor; they inherit a baseline that no longer describes the
> code, and the absolute numbers here are 30–60% lower than the same
> benchmark produces today — consistent with an unrecorded difference in
> machine state. The *lesson* below (per-child dispatch inverted the
> result; dispatch per container) stands and is what made recursive
> descent possible. The table does not.
>
> Also from review: the two arms are not like-for-like. The static arm
> builds a real `DisplayList` while the dyn oracle folds into a `u64`
> checksum. Decomposed at 100k the asymmetry runs *against* static (it
> does more paint work and still wins by 31%), so the direction is safe;
> the percentages should not be quoted as a measured ratio of equal
> traversals.

*2026-08-30. The §10 Step 2 benchmark: full layout + paint per iteration,
static node-table passes against a naive recursive `Box<dyn Widget>` tree,
identical shapes and identical layout math (equality verified by test).
Reference machine as in `docs/results-step1-firewall.md`; release profile;
medians over 200k/20k/300 iterations at 10/1k/100k nodes.
Harness: `cargo run --release -p benches --bin crossover`.*

## Headline: the architecture won only after a rework

The first cut of the passes dispatched **per child** (constraints pulled
through the parent's ops per child; cursor advanced through dispatch per
placement) and lost to naive dyn recursion at *every* size:

| nodes | static (first cut) | dyn | verdict |
|---|---|---|---|
| 10 | 115 ns | 68 ns | dyn −41% |
| 1k | 11.9 µs | 8.1 µs | dyn −32% |
| 100k | 1,111 µs | 813 µs | dyn −27% |

Cause: ~3 indirect calls per node vs dyn's 2 virtual calls, the same
container split recomputed once per child, and a 60-byte `NodeState`
streamed three times. After reworking dispatch to once-per-container with
dispatch-free child walks (see `docs/step2-design-notes.md` §2, §5):

| nodes | static (reworked) | dyn | verdict |
|---|---|---|---|
| 10 | 76 ns | 64 ns | dyn −16% |
| 1k | 6.5 µs | 8.2 µs | **static −21%** |
| 100k | 686 µs | 815 µs | **static −16%** |

**The crossover sits between 10 and 1,000 nodes.** Below it, fixed
per-pass overhead (three loop preambles vs one recursive call) outweighs
the flat-iteration advantage; a 10-node tree is also exactly where §5's
`#[fuse]` (Step 6) would erase the seams entirely. Above it, contiguous
streaming and predictable dispatch win and the margin is stable.

This is consistent with the handoff's own calibration (§4, §11): the
static table is a modest constant-factor traversal win, not a
transformative one — ~7 ns/node end-to-end at 100k.

## Rigid/flexible partition

At the list-shape's natural 18% rigid (all leaves), the partition is a net
*loss* — the `&[u32]` index indirection defeats streaming and the skipped
leaf dispatches are trivial:

| variant | 1k | 100k |
|---|---|---|
| full pass | 6.5 µs | 686 µs |
| partitioned (18% rigid) | 6.6 µs | 716 µs |

At 90% rigid (whole `RigidItem` subtrees drop out of down/up), it pays,
and the decomposition shows where:

| variant | 1k-rigid | 100k-rigid |
|---|---|---|
| layout, full | 3.94 µs | 454 µs |
| layout, partitioned | **1.65 µs (−58%)** | **308 µs (−32%)** |
| paint (always full) | 2.71 µs | 273 µs |

The 100k saving is smaller than 1k's because at that size the pass is
memory-bound: the place phase still walks every container and the state
array is streamed regardless. Conclusion: the compile-time partition is
real but conditional — worth keeping because it is nearly free to emit,
worth *using* only where rigidity is dense. The order-of-magnitude wins
the handoff cares about (§11) must come from invalidation (skip
*unchanged*, not merely *fixed-size*), which is orthogonal and not yet
built.

## What this does not show (honesty list)

1. **Leaf work is trivial here** (a multiply for text measure). Real
   widgets do real measure work, which grows both arms equally and shrinks
   every ratio above. The traversal-cost *difference* is the signal, not
   the percentages.
2. **The dyn baseline is naive on purpose** (fresh child-constraint
   computation per recursion, `Vec<Box<dyn>>` pointer chasing) — that is
   the §10 control. A tuned dyn arena would land between the two arms.
3. **Dispatch mechanism** (§12.1, vtable array vs enum hybrid) is still
   unmeasured — this benchmark fixes one mechanism (`&'static WidgetOps`
   per kind, two loads) for both partition variants.
4. **No repeaters yet**: the 100k tree materializes 100k nodes. Step 3
   exists precisely so that never happens in a real list.

## Verdict

Architecture validated with a correction: flat-table traversal beats naive
dyn recursion above ~1k nodes *only* with per-container dispatch
granularity — the first-cut per-child form inverted the result, and that
lesson is now load-bearing in the design notes. Fixed-cost disadvantage
below ~100 nodes is accepted (that regime belongs to `#[fuse]`, Step 6).
Proceed to Step 3 (repeaters).
