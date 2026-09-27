> **Mechanism drifted, substance stands (2026-09-02, F7.5).** Three
> details below describe machinery that no longer exists: the `RefCell`
> side arena (removed in T0.6 — the recursive path hands out `&mut`, so
> interior mutability bought nothing), the flat down/place phases (M0),
> and "the table stays 24 bytes" (`NodeDesc` is 48 B since M1 added the
> cascade's selector inputs — `docs/results-safe-prototype.md`). The
> scaling property this document establishes, and the deferred f32 extent
> ceiling it records, are unaffected; the window lives in
> `crate::arena::Arena` now.

# Step 3 — repeaters: design notes and measurements

*2026-08-30. Handoff §6's repeater and the §10 Step 3 validation. Machine
and profile as in `docs/results-step2-crossover.md`. Harness:
`cargo run --release -p benches --bin repeater`.*

## Design: the template never enters the parent's table

`Repeat<T>` contributes **exactly one** `NodeDesc` to whatever table
contains it. The handoff sketches "one table entry marks a template range";
here the template range is simply `T::SUB_TABLE` — it already exists once
per *type* as a const, so the repeater's ops reference it directly and the
main passes need no skip logic, no range bookkeeping, nothing. The §6
scaling property is structural, and tested as a compile-time fact:

```rust
const _: () = assert!(Repeat::<Item>::NODE_COUNT == 1);
const _: () = assert!(ListApp::NODE_COUNT == 3);   // header + list + root
```

Runtime shape: `count` (logical rows), `scroll`, uniform `item_extent`, a
`bind: fn(&mut T, usize)` that configures a recycled instance for a row,
and a `RefCell` side arena holding the window's `Vec<T>` instances plus
their `NodeState`s. The repeater's `up` computes the visible range from
its height constraint (it is its own scrollport), recycles and binds the
window, then runs the *ordinary* `pass::layout` per instance over the
template table — the nested pass is the same code as the outer one.
`place` shifts instance geometry by `row * extent - scroll` from the
repeater's origin; `paint` walks the window.

### Changes this forced elsewhere

- **`NodeDesc::CONTAINER` flag.** The down/place phases used to gate
  dispatch on `child_count > 0`; a repeater has zero *table* children but
  must still be dispatched in place. The gate is now an explicit flag set
  by the derive (fields > 0), runtime assembly, and the repeater.
- `ops::fold` became `pub(crate)` for the repeater's paint.

### Decisions worth recording

- **`bind` is a `fn` pointer, not a closure** — §7's "events are data"
  rule applied early, so Step 4 will not have to unpick captured state
  from the repeater.
- **Windowed state is disposable.** Rows sliding out of the window lose
  instance state (recycled + rebound). The §12.4 lifecycle question
  (persistent per-row state, e.g. an expanded row, and its interaction
  with hot reload) is deliberately still open; nothing here prejudges it.
- **Uniform extent.** `item_extent` is fixed per repeater — the standard
  virtualization simplification. Measured/estimated extents are a later
  feature; they change `visible_range`, not the architecture.
- **f32 extent ceiling.** `count * item_extent` at 10M rows is 2.4e8 px,
  past f32's exact-integer range; deep-scroll positions quantize. Real
  fix (f64 scroll domain or window-relative coordinates) deferred.
  **Since M44/T44.7 the rows past the ceiling are cut** rather than laid
  out on top of each other (`Repeat::reachable_rows`), in every build,
  and the agent's `W1009` names the cut; `Repeat::MAX_ADDRESSABLE_EXTENT`
  has the reasoning and the measured overlap it replaced.

## Measurements

Full layout + paint per iteration, 800px viewport (35-row window),
5-node `Item` template, medians:

| rows | repeat | resident nodes | materialized | resident nodes |
|---|---|---|---|---|
| 1k | 1.26 µs | 176 | 34 µs | 5,001 |
| 100k | 1.26 µs | 176 | 4.66 ms | 500,001 |
| 1M | 1.27 µs | 176 | 69.1 ms | 5,000,001 |
| 10M | 1.27 µs | 176 | — | — |

- The repeat arm is **flat to within noise across four orders of
  magnitude** of data: cost tracks the viewport (35 × 5 nodes + 1), table
  stays 24 bytes, window state ~5.6 KB.
- The materialized arm is linear, as it must be: at 1M rows a single
  frame is 69 ms and the tree holds ~280 MB of table + state. That is the
  world §6 exists to prevent.

**Corrected 2026-09-27: the 100k and 1M materialized rows above measured
a malformed tree.** A node's child count is a `u16`, so `concat_table`
wrapped the root's count: 100,000 instances linked 34,464 children, and
the rest of the table was never reached. That is why "4.66 ms at 100k"
is *cheaper* than the valid 65,535-row table below. M46/T46.3 made
`concat_table` refuse more than 65,535 instances, the benchmark then
panicked, and its materialized arms now stop at the limit. Re-measured
with the same bench (`--bin repeater`, three runs, load 0.46), medians:

| rows | materialized | resident nodes |
|---|---|---|
| 1k | 48.3 µs | 5,001 |
| 10k | 0.554 ms | 50,001 |
| 65,535 | 5.27 ms | 327,676 |

About 80 ns per row, linear. The repeat arm reads ~1.8 µs a frame at
every size on the same machine, against ~2.0 µs for the same bench at
`1717808`, the review's base (8 interleaved runs a side). Both are above
the 1.26 µs this doc first recorded, and that drift predates M39.
- Per-window-node cost in the repeat arm (~7 ns) matches the Step 2
  full-pass figure — the nested pass adds no measurable overhead over the
  outer one.

## Verdict

§6's scaling property holds and is enforced by construction, not by
discipline: there is nowhere to put a million descriptors even by
accident, because `NODE_COUNT` is a type-level constant the data cannot
reach. Proceed to Step 4 (hot reload) — noting the §12.4 repeater-state
question is now concrete: window state is currently disposable, and the
reload design gets to decide whether that stays true.
