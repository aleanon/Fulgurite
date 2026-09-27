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

## What M24/T24.1 changed — 2026-09-13

**The virtual call per node is gone**, and it was most of the cost.
`cascade` now reads a const `Widget::SEEDS` array — one
`{kind, defaults: fn() -> Style}` per node — instead of calling
`Node::defaults` and `Node::kind_name` through `&dyn Node` behind two
`&dyn Fn` closures. That was done for correctness, not speed: a *hidden*
subtree has no live node to ask, so `Option::None` and an inactive enum
variant were resolving their container's kind and defaults
(`docs/plan-select.md` §1). The speed was a side effect, and it is
larger than the correctness fix's author expected.

Same machine, same session, one `git stash` apart — so the two rows are
the same fixture, the same build flags and the same thermal state:

| operation, 901 nodes | before (T24.0) | after (T24.1) | |
|---|---:|---:|---:|
| `cascade_deep` — a full stylesheet swap | 37.45 µs | **15.42 µs** | **2.43× faster** |
| `cascade_subtree`, 90 of 901 nodes | 2.19 µs | **1.43 µs** | 1.53× faster |

The `before` row reproduces the 37.8 µs measured on 2026-09-04 to within
1%, which is what makes the comparison worth reading.

**The conclusion the table above draws is unchanged.** A swap is still
the expensive runtime operation — 15.4 µs against a 9.3 µs whole-tree
layout and paint — and scoping is still what an `#[widget_id]` toggle
should do: 10.8× cheaper here, against 16.6× before. The ratio narrowed
because the per-node constant fell on both sides, not because scoping
matters less.

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

## M26/T26.6 — skipping the specificity passes a sheet has none of

Measured 2026-09-19, `cargo run --release -p benches --bin cascade`,
**eight runs before and eight after**, classified into the fast and slow
clusters M25/T25.12 found this machine's arm family to be bimodal over.

The cascade runs four passes per node so that a higher-specificity rule
always lands last. Three of those four are usually empty: every sheet in
this workspace uses `Id` and `Kind` and neither `Class` nor `Any`, so
half the passes scanned every rule, per node, to match nothing. The
levels a sheet has are a property of the *sheet*, so they are computed
once before the node loop.

| arm | before (fast cluster) | after | Δ |
|---|---:|---:|---:|
| `cascade_deep`, 901 nodes, 4-rule sheet | 14,823 ns | 13,094 ns | **−11.7%** |
| `cascade_deep`, showcase-shaped 27-rule sheet | 64,249 ns | 41,230 ns | **−35.8% (1.56×)** |

Fast-cluster spread: 204 ns (1.4%) before and 561 ns (4.3%) after on the
small arm; 771 ns (1.2%) and 598 ns (1.5%) on the showcase-shaped one.
Both deltas are an order of magnitude outside it. Four of eight runs
landed in each cluster before and after, on both arms.

**The number the review could not settle.** It reported this twice with
different figures — 2.2× in one agent's run and 54,399 → 40,739 ns
(1.33×) in another. Neither was citable and neither is right: it is
**1.56×** on the showcase-shaped sheet and 1.12× on the small one, which
is the spread you would expect from a saving proportional to the rule
count.

**Not a frame win.** The cascade runs on a sheet, palette or shape
change, which `style.rs` states as its contract; a still frame does not
pay this at all. What it shortens is a palette swap and the showcase's
own demo switch.

The gate is `crates/lumen-static/tests/cascade_levels.rs`, and it
asserts the **ordering** rather than differencing against a naive copy
of the loop — a differential test would pass under an injection that
broke both sides, since both sides would be the same code. Three
injections (DX–DZ): marking a present level absent, running the passes
in reverse, and skipping a level unconditionally. All three redden, and
the first two are caught by two tests each.

## M60/T60.2 — the tally's own sign flip, full against scoped

*2026-09-27, `docs/plan-tally-slim.md` T60.2. Same machine, under the
bench lock (load 0.77 → 1.27). `cargo test --release` on
`Host<Tally>` (13 nodes, the 13-rule `stylesheet_for`), alternating the
positive and negative sheets, 2,000 swaps per block, median of 41 blocks.
Both arms run the sheet lint, which the agent feature compiles in.*

| arm, 13 nodes | median |
|---|---:|
| `set_stylesheet` — the whole sheet | 829 ns |
| `restyle_subtree(.., "card")` | 752 ns |
| whole sheet, then the pump that lays it out | 1,391 ns |
| scoped, then the pump | 1,360 ns |
| the pump alone after `invalidate_all` | 521 ns |

**Scoping saves 31 ns of a 1.4 µs frame (2%) at the tally's size**,
where it saved 10.8× at 901 nodes: the card *is* the tree here (every
node but the page and its two spacers). So `Program::style_by` cascades
the whole sheet, which asks nothing of its caller — `restyle_subtree`'s
promise that a new sheet differs only inside the subtree is one a
program-level API would have to extract from every application, for a
saving that only exists when the keyed rules sit in a small corner of a
large tree. Such an application can still call `restyle_subtree` itself.
