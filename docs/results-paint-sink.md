# Results: what the `Painter` sink costs

*2026-09-04, plan M10a/T10a.2. Machine: i9-13900KF, governor
`powersave`. `cargo run --release -p benches --bin paint_sink`, 20k
iterations in blocks, median of blocks.*

## Why this was measured rather than assumed

`Node::paint` wrote into a concrete `&mut DisplayList` through inherent,
inlinable methods. It now writes through `&mut dyn Painter`.

The recursion into children was **already** indirect for ordinary trees
— the derive's `call_child_ref` emits `&dyn Node` — so the recursion is
not what changed. What changed is the **leaf builder calls**, and in a
`#[fuse]`d tree that matters most: paint there was 100% static, so the
sink introduces the only indirection in it. `#[fuse]` exists to
devirtualize (`docs/results-traversal.md`), and cross-cutting rule 1
requires a dispatch change to be re-measured.

## The number

| commands | concrete `&mut DisplayList` | `&mut dyn Painter` | added |
|---:|---:|---:|---:|
| 34 (the tally's frame) | 96.6 ns | 124.1 ns | **+27.5 ns** |
| 1 000 | 2 460 ns | 2 868 ns | **+408 ns** |
| 2 000 (a ~900-node tree) | 5 592 ns | 6 329 ns | **+738 ns** |

**≈0.4–0.8 ns per command**, which is one indirect call.

## The verdict

Against `docs/results-frame-cost.md`'s measured render p95 of
**0.089 ms**:

- the tally's 34 commands add **27 ns — 0.03% of a frame's render**;
- a 2 000-command frame adds **0.74 µs — 0.8%**.

**Accepted.** The sink buys an embedded host that builds no display list
at all, and costs under one percent of the raster it feeds.

`crossover` and `fusion` were re-run across the change and are within
run-to-run noise; `fusion` still shows its ~30% margin over naive `dyn`
at 1k and 100k nodes, so the sink did not cost `#[fuse]` its purpose.

## What this does not measure

Painting is not the frame. The full picture is in
`docs/results-frame-cost.md`: layout 1 µs, paint 3 µs, render 89 µs,
present 39 µs at p95. A per-command cost only becomes interesting in a
frame whose command count is large *and* whose renderer is cheap — an
embedded immediate-mode `Painter` is exactly that case, and it is the
one that cannot be measured here because the hardware has not been
named (`docs/plan-embedded.md` T13.0).
