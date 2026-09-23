# M35 — `observe` should cost nothing until something observes

> **Written 2026-09-23, not started.** The cause is already measured.
> `docs/results-competitive.md` §7, under "Attributed, 2026-09-23", owns
> every figure below, and this plan cites them rather than restating
> them.
>
> Compiling the `observe` feature in costs the frame arm about a quarter
> of its layout and paint time, with no agent attached and no journal
> running. Every application built so an agent *can* attach pays that on
> every frame. The handoff that raised this blamed `probe` calling
> `layout`. **Measured, the cause is a vtable.** `List::observe` walks
> each item as a `&dyn Node`. The coercion builds the row widget's
> vtable, the vtable takes the addresses of `layout` and `paint`, and
> LLVM stops inlining them into the per-row loop. Removing that one call
> site restored the plain build's frame and its inlining.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and prints the measured quantity beside its bound.*

---

## 0. The rules for this milestone

1. **The fix keeps the feature's capability intact.** The agent must
   still see every row of a `List` and a `Repeat`, with the roles and
   bounds it sees today. The fix changes how the walk dispatches,
   never what it reports. `tests/observe.rs` and `just widget-drive` are
   the proof.
2. **Measure on the frame arm, then on an application.** The frame arm
   is where the cost was found. It is also the worst case: one widget
   type, three thousand rows, one container. The milestone does not
   describe the saving until the grid fixture has measured it as well
   (T35.2).
3. **Guard the symbol table as well as the clock.** A clock shows that a
   frame is slower. Only the symbol table shows *why*, and it can show
   it on any machine, `perf` or not.

## 1. What exists today

- `observe::emit_subtree<W: Widget>` forwards to `emit_table(table,
  tree: &dyn Node, …)`, which calls `tree.node(i).observe(…)` for every
  node of the subtree.
- `List::observe` and `Repeat::observe` each call it once per item.
  Those are the only places a statically typed *row* widget becomes a
  `&dyn Node`. `Dyn` calls `emit_table` too, but its content is already
  a `dyn Node`, so it has a vtable in every build and is out of scope.
- A widget on the derived table is already coerced by the derive's
  `node(i)`, so its vtable exists in every build. This milestone does
  not change that path.
- The `Node` trait's observe-only methods are `probe`, `observe` and
  `stats`, all under `#[cfg(feature = "observe")]`.

## 2. The decision: dispatch the walk statically

`emit_table` needs *something* dynamic, because a subtree's nodes are
different types. The fix keeps the walk and changes its entry point.
**`Node` gains an observe-only method that walks the widget's own
subtree by static dispatch.** The `StaticTree` derive generates it
alongside `node(i)`: for each field it calls the field's own method
directly, never through `&dyn Node`. A leaf's implementation, supplied
by `leaf_common!`, just calls its own `observe`. `List` and `Repeat`
call the new method on each item, so no item is ever coerced to
`&dyn Node`, and the row widget's `layout` and `paint` keep one call
site each.

Two alternatives were considered and rejected:

- **`#[inline(always)]` on `layout` and `paint`.** It treats the symptom
  in one widget. Every widget written after it would need it too, and
  nothing would notice when one did not.
- **A separate `Observe` trait with its own vtable.** It would work,
  because the observe vtable would not hold `layout`. But `emit_table`
  would still need a way to get from a `&dyn Node` to a
  `&dyn Observe`, which is another method on every widget. The derived
  walk is the same amount of generated code with no second vtable.

## T35.0 — the gate, before the fix

Add a check that builds the frame arm twice, plain and with
`lumen-static-widgets/observe`. It must **not strip either build**,
because the benchmark profile strips by default. It then reports two
things:

1. Whether `Label`'s `layout` and `paint` appear as standalone symbols
   in each build. They must be absent from both.
2. The observe build's layout and `+paint` against the plain build's,
   eight interleaved runs each. The observe build must fall inside the
   plain build's fast-cluster spread.

**Today the gate is red, and that is its first injection.** Record the
failure as it stands, with the symbol sizes and the spread, and commit
the gate marked as expected to fail until T35.1. Show the other
direction in the same session as well: point the gate at two plain
builds and watch it pass. A gate that refuses both builds tests
nothing.

## T35.1 — the static walk

§2's mechanism. `emit_subtree` keeps its signature for every existing
caller, so the agent crate does not change.

**Gate:** T35.0 goes green. `tests/observe.rs` and `just widget-drive`
still report every row, with its role and bounds. *Injection:* put the
`&dyn Node` coercion back in `List::observe` only. The symbol check
fires, naming `Label::layout`, and the frame check fires with its
numbers printed.

**And a second injection, for the derived walk:** have the generated
walk skip one field. `tests/observe.rs` must fail, naming the missing
node. That shows the new walk is what produces the snapshot, not
something that merely sits beside it.

## T35.2 — what an application pays

Measure `just grid`, which has a `Repeat` body and about two thousand
table nodes, with and without `observe`, before and after T35.1. Record
the result in `docs/results-competitive.md` §7, beside the frame arm.

The kill condition is written here, before the number exists. If the
grid shows no cost before T35.1, the frame arm's 24% belongs to that
arm's shape. §7 then says so, and the milestone still ships T35.1,
because the mechanism is real for any list of leaves, but it stops
describing the win as universal.

**Measure:** frame arm and grid, eight runs a side, split into clusters
as `docs/results-frame-and-types.md` describes.

## 3. What this is not

- **Not the journal's cost.** A running journal is a separate and much
  larger figure (§7's third row). That is the price of observing, not of
  being observable. It needs its own milestone if it matters.
- **Not a change to what the agent sees.** Rule 1.
