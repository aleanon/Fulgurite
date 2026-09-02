# Step 6 — `#[fuse]`: the erasure dial, measured

> **Machine.** i9-13900KF, governor `powersave`, turbo on, `tsc`
> clocksource, rustc 1.94.0. Absolute figures are not portable across
> machines or governors; ratios taken inside one run are. Recorded from
> 2026-09-02, when it turned out no results doc had said.

> **SUPERSEDED 2026-09-02 (F7.1).** Every number below was measured
> against the flat table walk that `docs/results-traversal.md` replaced
> with recursive descent. On the shipped code the headline does not hold:
> **fusion is 429 µs against 435 µs for `&dyn` calls at 100k nodes, i.e.
> within noise** — `results-traversal.md` records the re-measurement, and
> `plan-architecture-v2.md`'s M0 outcome repeats it. The −42% / −61% /
> −64% figures and the "wins ~2.8×" conclusion were artifacts of the
> traversal being compared against, not of fusion.
>
> Why it kept a marker for three days after the fact: the 2026-09-02
> correction pass annotated four other results docs and skipped this one,
> which is the same manual-pass failure `AGENT.md`'s doc-currency rule is
> about. It was the only wholly stale document in `docs/` with no marker
> at all.
>
> **What survives** is the mechanism and the trade, which are unchanged:
> `#[fuse]` emits static calls instead of `&dyn`, collapses a subtree to
> one table entry, and therefore makes its internals invisible to the
> agent surface and unable to carry per-node state. That is a real dial
> with a real cost; the *speed* it was sold on is not there on the
> recursive path, where the vtable already arrives with the reference.

*2026-08-30. Handoff §5's per-subtree fusion, judged against Step 2's
measurements as §10 prescribes. Same machine, profile, shapes, and
constraints as `docs/results-step2-crossover.md`; harness
`cargo run --release -p benches --bin fusion`.*

## What was built

`#[widget(row|column, spacing = …, fuse)]` on a `StaticTree` struct
(spelling deviation from the handoff's separate `#[fuse]` attribute —
axis and spacing are needed anyway, so one attribute carries all three).
A fused struct is **one table entry, one kind**: the derive emits an
`Inline` impl (`measure` / `paint_inline`) whose bodies call the fields
*directly* — no fn pointers — so the compiler inlines and const-folds
across the whole subtree, and a `WidgetOps` whose `up`/`paint` enter that
straight-line code with a single dispatch. Internal geometry is
recomputed from the node's constraints during paint rather than stored:
a fused subtree has no per-node `NodeState`.

Composition rules, both tested:
- a fused widget sits anywhere a widget can (the §9 seam: outer schedule
  dynamic, inner kernel fused);
- fusion is inward-only — fused fields must be `Inline` (leaves or other
  fused structs); a table container inside a fused one is a compile
  error by design;
- rigidity composes through fusion (`RigidFused::IS_RIGID` is a const);
- geometry is **exactly** the unfused twin's (asserted, not approximated).

## The numbers

Full layout + paint per iteration; "logical nodes" counts the UI as
authored — the fused arms' tables hold 1, 46, and 4,546 entries:

| logical nodes | static table | **fused** | dyn | fused vs table | fused vs dyn |
|---|---|---|---|---|---|
| 10 | 74 ns | **43 ns** | 67 ns | −42% | −36% |
| 1k | 6.28 µs | **2.46 µs** | 8.18 µs | −61% | −70% |
| 100k | 691 µs | **249 µs** | 826 µs | −64% | −70% |

- **The Step 2 gap is closed.** The one regime where naive dyn beat the
  table (≤ ~100 nodes, by ~16%) now belongs to fusion by a wide margin —
  exactly the job §5 assigned it.
- **The recompute bet pays.** Fused paint re-measures children instead
  of reading stored state, and still wins ~2.8× at 100k logical nodes:
  straight-line arithmetic beats streamed 32-byte state plus two
  dispatches per node. The p10–p90 spread collapses too (249.3–250.9 µs
  vs 682–777 µs) — no memory traffic, no variance.
- ~2.5 ns per logical node at 100k, vs ~7 ns for the table and ~8 ns
  for dyn.

## Why the §4 trap does not reopen

§4's blowup came from *per-position* monomorphization (recursive generic
descent specializes per subtree type = per tree position). `#[fuse]`
monomorphizes per **named type**: `FusedCard`'s straight-line body is
emitted once no matter how many thousands of instances the table holds.
Code size still grows with how much UI you *wrote* — the same scaling
law as `NODE_COUNT` — and the dial stays local to the structs that opt
in. The compile-time cost is likewise confined to the defining crate,
inside the Step 1 firewall.

## Costs, stated plainly

1. **Observability granularity.** A fused subtree is one AccessKit node
   and one provenance entry; internals are invisible to Step 5's
   surface. The repeater's `extra`-nodes mechanism could re-expose them
   (geometry is recomputable, provenance could be tabled) — deferred,
   and the right default may well be "fused = private detail".
2. **No per-internal-node state** means no per-internal-node
   invalidation, focus, or animation handles. Fuse leaf-ish display
   cells, not interactive regions — this matches §5's "table cell
   rendered ten thousand times" motivation.
3. Double-measure of children (layout + paint). Included in every
   number above; it is part of why the win is "only" ~2.8×.

## §12.5 (fusion heuristics): stays manual, and the pressure is lower

The handoff asked whether the macro should infer fusion candidates.
Decision: **manual**, and the measured shape of the trade says the
urgency the handoff feared is largely gone — per-type monomorphization
means the cost of a wrong `fuse` is bounded and local, and the
observability/state costs (1, 2 above) are semantic judgments a
heuristic should not make. Revisit only if profiling real apps shows
authors systematically missing wins.

## Verdict

The dial works and its trade-offs are the designed ones. **§10's build
order is complete**: firewall PASS → table beats dyn above ~1k →
repeaters flat to 10M rows → live dylib swap → AccessKit + provenance at
zero release cost → fusion closing the small-subtree regime.
