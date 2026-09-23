# M35 — `observe` should cost nothing until something observes

> **Written 2026-09-23, not started. Revised the same day after a
> seven-lens review** (§5). The review confirmed nine findings against
> this plan, six of them major, and the plan keeps its corrections
> visible rather than replacing itself. `docs/results-competitive.md` §7,
> under "Attributed, 2026-09-23", owns every figure this plan relies on.
>
> Compiling the `observe` feature in slows the frame arm's layout and
> paint, even with no agent attached and no journal running. The
> handoff blamed `probe` calling `layout`. **Measured, the cause is a
> vtable.** `List::observe` walks each item as a `&dyn Node`. That
> coercion builds the row widget's vtable. The vtable takes the
> addresses of `layout` and `paint`, and LLVM then stops inlining them
> into the per-row loop. Removing that one call site restored the plain
> build's frame and its inlining.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and prints the measured quantity beside its bound.*

---

## 0. The rules for this milestone

1. **The fix keeps the feature's capability intact.** The agent must
   still see every node it sees today, with the same role, bounds and
   hidden flag. That includes the entries for a `None` and for an
   inactive enum variant. The fix changes how the walk dispatches,
   never what it reports. T35.1's equivalence gate proves this.
2. **Measure on the frame arm, then on an application.** The frame arm
   is where the cost was found, and it is the worst case. The milestone
   does not describe the saving until the grid fixture has been measured
   too (T35.2).
3. **The symbol table is the gate, and the clock is the record.** Only
   a symbol check can say *why* a frame is slow, on any machine, `perf`
   or not, and it answers yes or no, so it cannot flake. The clock
   figure is recorded in §7 but gates nothing (§5, finding 5).

## 1. What exists today

- `observe::emit_table(table, tree: &dyn Node, …)` emits **one entry per
  table index**. It marks a zero-area node hidden, then calls
  `tree.node(i).observe(…)`. `emit_subtree<W>` forwards to it.
- **Where rows become `&dyn Node`.** `List::observe` and
  `Repeat::observe` call `emit_subtree` once per item. `Dyn` calls
  `emit_table` too, but its content is already a `dyn Node` in every
  build, so it is out of scope.
- **Why derived-table widgets are unaffected.** A non-`fuse` derived
  container already coerces every child to `&dyn Node` in its layout,
  place, paint and hit bodies (the derive's `call_child`). A leaf's
  `node(0)` returns `self` as `&dyn Node`, and dispatch reaches `node()`
  in every build. So those vtables exist whether or not `observe` is
  on. `fuse` containers call their children statically, and whether
  *their* leaves lose inlining through dispatch alone is unmeasured and
  out of scope (§4).
- **The agent does not go through `emit_subtree`.** Its `get_tree` and
  its `Index::build` call `emit_table(tree.table, tree.root, …)`
  directly, over the main table.
- **`Node` impls come from three sources.** The derive; about 33
  `leaf_common!` uses; and about 30 hand-written impls with their own
  `node()`. The hand-written ones include `Toggle`, `Slider`,
  `TextInput`, `TextArea`, `Image`, `Glass<W>`, `ScrollView`, `Popup`,
  `Veil`, `Select`, `Option<W>`, `Either<A, B>`, `List`, `Repeat` and
  `Dyn`. The derive refuses generic types, so no generic wrapper can
  ever get a derived walk. T35.1 recounts them mechanically and puts the
  command in its commit.
- **The `benches` crate enables `observe` unconditionally**, both
  through `lumen-static-widgets` and through its dependency on the agent
  crate. So `just grid` cannot currently be built without it.

## 2. The decision: one table-driven walk, statically dispatched where a widget exists

`Node` gains an observe-only method that emits a widget's own subtree.
**It is required, with no default.** A provided default can either skip
a wrapper's children or coerce `self` and bring the vtable back.
`AGENT.md` already names this shape: a provided method is right for a
leaf and wrong for a wrapper, always. Being required, it makes the
compiler list every impl that needs one.

The walk is **table-driven for emission and static for dispatch**:

- It emits one entry per table index, in the same order and with the
  same hidden rule `emit_table` uses today.
- Where a live child owns an index range, it calls that child's method
  directly. The derive generates this per field, and hand-written
  wrappers write it: `Glass<W>` calls its content, `Option<W>` calls its
  `Some`, and so on.
- Where no live widget owns an index range — a `None`, an inactive
  variant — it emits those entries from the container's own `observe`,
  exactly as `node(i)`'s "resolve to the container" does today.

**The agent's walk moves onto it as well**, so one table has one walk.
`emit_table` over `&dyn Node` stays, but only as the reference in a
test.

Rejected, with reasons:

- **`#[inline(always)]` on `layout` and `paint`.** This treats the
  symptom in one widget. Every widget written afterwards would need it
  too, and nothing would notice when one did not.
- **A separate `Observe` trait with its own vtable.** It would work.
  But getting from a `&dyn Node` to a `&dyn Observe` needs another
  method on every widget anyway, so it is the same number of impls with
  a second vtable added.

## T35.0 — the gates and the baselines, before the fix

1. **Make `observe` and the agent optional features of `benches`**, and
   mark with `required-features` the bins that need them (`agent`, and
   any others). Build `just grid` both ways, with
   `lumen-static-widgets/observe` only, to match the frame arm. **Record
   the grid's cost before T35.1 here**, because T35.2 cannot take a
   baseline after the fix. **Write the prediction down first.** The
   grid's `BodyRow` is a non-`fuse` derived row whose labels are
   coerced by `call_child` in every build, so the mechanism predicts
   that `observe` adds only the rows' own vtable, and a small cost.
2. **The symbol gate**, which is the committed gate. Build unstripped,
   plain and with `lumen-static-widgets/observe`, for **two arms**: the
   frame arm (`List<Label<String>>`) and a new `Repeat<Label<String>>`
   arm. Assert that no row widget's `layout` or `paint` appears as a
   standalone symbol in either build, and print each symbol's size when
   one does.
3. **The clock, as a record.** Eight interleaved runs a side. The
   statistic is **the delta between the fast-cluster minima**, printed
   beside the plain build's fast-cluster width. The fast cluster is
   chosen by a mechanical rule written into the script (drop runs more
   than 5% above the minimum) instead of by eye. Run the same rule over
   two plain builds and print the result, so the statistic's noise
   floor is on record.

**Today the symbol gate is red, and that is its first injection.** It
fails on both arms, with the sizes printed. Commit the gate marked as
expected to fail until T35.1. **Other direction:** run it over two
plain builds and watch it pass.

## T35.1 — the walk

§2's mechanism: the required method, the derive's generated walk, every
hand-written impl, and the agent moved onto it.

**Gates:**

- **The symbol gate goes green on both arms.** *Injection:* put the
  `&dyn Node` coercion back in `Repeat::observe` only. The `Repeat` arm
  fails and the `List` arm does not, which shows each arm tests its own
  container.
- **Equivalence**, which is new and is rule 1's proof. On one tree that
  contains `Option::None` and `Some`, `Either`, an enum with an inactive
  variant, a `List`, a `Repeat`, a `Glass<W>` and a hand-written leaf,
  the new walk's node list must equal
  `emit_table(W::SUB_TABLE, &tree as &dyn Node, …)`: same ids, roles,
  bounds, hidden flags and children, compared element by element, with
  the first difference printed. *Injections:* have `Glass<W>`'s walk skip
  its content, have the derive's walk skip one field, and have the
  `None` case emit nothing. Each must fire and name the missing node.
- `tests/observe.rs` gains a `List` case, because it covers `Repeat`
  and has never covered `List`. `just widget-drive` stays green under
  both strategies.

## T35.2 — what an application pays, measured

Measure the grid after T35.1 against T35.0's baseline, using T35.0's
statistic, and record it in `docs/results-competitive.md` §7 beside the
frame arm.

**This is a scoping rule, not a kill condition.** T35.1 ships whatever
this shows, because the mechanism is real for any list of leaves. What
the number decides is the prose. If the grid's delta is inside the
noise floor T35.0 recorded, §7 says the cost belongs to lists of
leaves, and nothing claims the fix saves an application a quarter of
its frame.

## 3. What this is not

- **Not the journal's cost.** A running journal is a separate and much
  larger figure (§7's third row). That is the price of observing, not
  of being observable.
- **Not a change to what the agent sees.** See rule 1.

## 4. Deferred

- **`fuse` containers.** A fused leaf is called statically in layout,
  but dispatch coerces it in every build. Whether that already costs
  inlining in plain builds has not been measured. Reopen if a `fuse`
  tree's frame is profiled.

## 5. What the review found (2026-09-23)

A seven-lens review (one lens per plan, plus a cross-plan lens), with
an adversarial verifier for every finding, confirmed nine findings
against the first draft of this plan. What changed:

| # | finding | what changed |
|---|---|---|
| 1 | the new method was neither required nor provided, and about 30 hand-written impls were not counted | §2: required. §1: impls counted |
| 2 | a `None` or an inactive variant has no widget for a field-by-field walk to recurse into | §2: emission is table-driven |
| 3 | the agent walks the main table itself, so the fix left two walks and nothing checking they agree | §2: the agent moves onto the new walk. T35.1: equivalence gate |
| 4 | the gates covered only `List`, so leaving `Repeat` coerced would have stayed green | T35.0: a `Repeat` arm. T35.1: an injection per container |
| 5 | "inside the plain build's spread" would have **refused the known-good fix** on layout (13.8–14.7 against 13.5–14.3), and a plain-against-plain check would pass only about 23% of the time | rule 3: the clock records and does not gate. T35.0: the statistic is defined, and its noise floor is measured |
| 6 | no grid build without `observe` could exist, because `benches` forces it on | T35.0 step 1 |
| 7 | the "kill condition" killed nothing, and its baseline had to be taken before the fix, which T35.2 comes after | T35.0 takes the baseline. T35.2 is a scoping rule |
| 8 | §1 named the wrong coercion site (the derive's `node()` is a static call; the coercion is in `call_child` and a leaf's `node(0)`) | §1 corrected, and results §7 with it. `fuse` deferred |
| 9 | the first draft restated §7's percentage while claiming only to cite it, and the frame arm's manifest carried a second, different figure | percentages removed. The manifest now cites §7 |
