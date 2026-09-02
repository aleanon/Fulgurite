# Zero allocation in steady state — measured, and now gated

*2026-09-02, plan `docs/plan-review-fixes.md` F6.9. Harness:
`crates/lumen-static-shell/tests/alloc_gate.rs`, a counting
`#[global_allocator]` in its own test binary. Machine-independent: the
numbers here are integers.*

## The claim

`docs/00-handoff-static-tree.md` §10: *"Zero allocation for the entire UI
— not 'few', zero."* It is the property that makes the no-allocator
profile possible at all, and it is stated as a design constraint
throughout the handoff.

**Nothing checked it.** A counting allocator was reached for three times
by hand during the 2026-09-02 review, and each time it found something:

| found | where | fixed |
|---|---|---|
| 2 allocations per enum node per `layout` | the derive's scratch arena | F4.3 |
| 3 collections per frame + a `String`/`Value` per finding | the agent's journal and audit | F2.2, F2.3 |
| 1002 allocations, 9,906 bytes leaked per 1000 bound updates | `Label`'s default storage under `#[bind]` | T6.2 fix, F1.x era |

All three had shipped past a green gate.

## Results

Allocations per `Host::pump`, after five warmup frames, averaged over
100:

| path | per frame |
|---|---|
| plain tree (leaves + derived containers), idle | **0** |
| plain tree, a state change through `#[bind]` every frame | **0** |
| `Repeat` + `List` + `Either` + `Dyn` + `Option`, idle | **0** |
| `Repeat` over 10,000 rows, scrolled one row per frame | **0** |
| two enum-widget nodes, idle | **0** |

And from `benches --bin agent`, for the dev surface:

| path | per frame |
|---|---|
| `Agent::frame_done`, few findings | **0** |
| `Agent::frame_done`, 132 findings | **0** |

## The number is zero, not a budget

Every path measures zero, so a budget would only encode a bug. That is
the whole value of asserting an integer: it cannot drift quietly the way
a microsecond figure does, it needs no reference machine, and a
regression is a failed test rather than a number someone has to notice
got worse.

## Two things that make this test correct rather than flaky

**Its own binary.** `#[global_allocator]` is per-binary, so folding this
into another test file would tax every test in it.

**A thread-local counter, not a global one.** `cargo test` runs tests in
parallel threads within one process, so a global counter measures every
*other* concurrently-running test's allocations too. An earlier version
of this file used an `AtomicUsize` and reported 0.16 allocations per
frame from code that allocates none — and reported *more* the longer it
warmed up, because more of the suite was running alongside. The
allocator hook uses `try_with`, since during thread teardown the
thread-local is gone and a panic in the allocator is not recoverable.

## Verified to have teeth

Planting one `Vec::with_capacity(8)` in `Host::pump` fails all five
tests. A gate that cannot fail is not a gate — the same lesson as
`table_is_well_formed` (`docs/results-const-eval.md`).

## What is *not* covered

Construction. `Host::new` allocates the two parallel arrays, `Either`
allocates its branch arenas, `List` its item arena; those are one-time
and proportional to the tree. The constraint is about steady state, and
the §10 claim's own framing — a microcontroller with no allocator —
would need a construction story too (an arena or a `const` initializer).
That is not in this plan and is not claimed here.
