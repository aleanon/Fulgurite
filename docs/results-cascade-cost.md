# Results: what a stylesheet swap costs, and what scoping it saves

*2026-09-04, plan M8c/T8c.4. Machine: i9-13900KF, governor `powersave`.
`cargo run --release -p benches --bin cascade`, 20k iterations in
blocks, median of blocks. Fixture: `benches::Ten10`, 901 nodes.*

## The measurement that reordered the milestone

M8c was planned as a layout optimisation: dirty bits, a bubble to a
relayout boundary, containers skipping clean children, with the scoped
cascade as a rider that "comes free". Measuring first inverted it.

| operation, 901 nodes | median |
|---|---:|
| whole-tree layout + paint (`crossover`, 1k nodes) | 9.3 µs |
| **`cascade_deep` — a full stylesheet swap** | **37.8 µs** |
| `cascade_subtree`, 90 of 901 nodes | **2.28 µs** |

A stylesheet swap costs **four times a whole layout and paint**, and it
is the operation an app performs at runtime — a class toggle, a theme
change, the tally's sign flip. Scoping it to the subtree that changed is
**16.6× cheaper** and scales with the subtree rather than the tree.

Why it is expensive: `cascade` resolves each node by asking the node for
its `defaults()`, then walking the whole sheet four times (once per
specificity level) testing every rule against it. That is
O(nodes × rules × 4) with a virtual call per node — where layout is one
arithmetic pass with a memo in front of the only expensive part.

## Why scoping is sound, and not merely faster

Every `Selector` is node-local: `Id`, `Class`, `Kind` and `Any` read the
node's own descriptor and its kind name. None reads an ancestor, a
sibling or a position. So a node's resolved style is a function of that
node and the sheet, and re-resolving a contiguous range of the table
cannot disagree with re-resolving all of it. The table is DFS pre-order,
so a subtree *is* a contiguous range.

Adding a descendant combinator (`Card Label`) would end that property.
`cascade_subtree`'s doc says so, and
`crates/lumen-static/tests/scoped_cascade.rs` fails if it is added
without revisiting the function.

The caller's half of the contract — the new sheet may differ only in
rules matching inside the subtree — is the part that cannot be checked
cheaply, so debug builds check it *expensively*: a full cascade is run
and every style outside the range compared. It caught a genuine misuse
the first time it ran, in this repository's own test.

## What was deferred, and why — T8c.1, T8c.2, T8c.3

**Not built. The measurement does not support it and soundness argues
against it.**

- **The win is small.** Whole-tree layout + paint is 9.3 µs at 1k nodes
  — 0.06% of a 16 ms frame, and ~6% of the tally's 79 µs frame after
  M8b. The expensive half of layout is text measurement, and that is
  *already* memoized per label, keyed on (dirty, limits): a memo hit is
  4.8 ns against a 2.5–16.7 µs miss (`docs/bindings-decision.md` T7.3).
  Container-level skipping would save arithmetic, not shaping.
- **It is not sound while widget state is public.** T8c.3 skips a
  child's `layout` when its subtree is clean. Cleanliness comes from
  dirty bits, and a dirty bit is only correct if *every* mutation sets
  it. Widget fields are public by design (handoff §3: "topology in the
  type, values in the fields") and the repository mutates them directly
  in several places — `app.badge.w = …` in the agent tests,
  `s.ui.list.count = rows` in the hot-reload fixture, `repeat.scroll`.
  Every one of those would produce a stale frame. `Label` avoids this
  because its text is behind `Dirty<T>`; `Fixed`, `Repeat` and `Button`
  have no such guard.
- **So the prerequisite is encapsulation, not invalidation.** Making
  layout-affecting fields private with marking setters is an M0-class
  change across every widget and every app, and it should be justified
  by a screen that needs it. Nothing measured so far does.
- **T8c.1 and T8c.2 have no consumer without T8c.3.** Building them
  anyway would be the "finished-but-unreachable" failure
  `crates/lumen-static-agent/src/audit.rs` was written to catch.

Recorded rather than silently dropped: if a future screen makes
whole-tree layout matter — many thousands of on-table nodes, or a
memo-defeating pattern like B10's `Repeat::bind` — this is the file that
says what would have to be true first.
