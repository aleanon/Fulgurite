# What it costs, and who should not use this

[Topology in the type](./topology.md) made the case. This chapter is the
other side of it, and it is the chapter to read before committing a
project to this framework.

No figures here either — the measurements live in `docs/` and are named
where they apply.

## The monomorphization trap

Write layout and paint as recursive generic descent and the compiler
emits a separate specialised `layout` and `paint` **per distinct subtree
type** — which is per tree *position*, not per widget kind. Two buttons
in different places are two instantiations. On a large application that
is tens of thousands of them.

What that costs, from spec §4:

| symptom | effect |
|---|---|
| instruction-cache pressure | past a certain size a boring `dyn` loop over a flat arena is *faster*, because a dyn call with a stable target predicts nearly perfectly |
| compile times | change one leaf, the root type changes, every monomorphization above it invalidates, the whole UI crate rebuilds |
| binary size | bloat — ironic, given the embedded target |
| error messages | type names hundreds of lines long, and rust-analyzer degrading badly |
| refactoring | moving a widget changes types all the way up the chain |

Rust also has no variadic generics, so the obvious encoding —
`Row<(A, B, C, ...)>` — needs macro-generated tuple impls and becomes
unpleasant past roughly a dozen children. SwiftUI's "expression too
complex to be solved in reasonable time" is this exact failure mode.

**The crossover point where `dyn` wins is lower than intuition
suggests.** That sentence is in the spec, and the escape it motivates is
§5: reify the tree as a flat `const` table, so the *shape* is const data
rather than const code. The struct is still the tree; the traversal is
not generated per position.

## The escape hatches are a consequence, not a feature

`Either`, `Dyn`, `List` and `Repeat` exist because the type cannot know
everything: a list whose length comes from a file, a panel that swaps
layouts. They are deliberately explicit — you name the hatch, so the cost
is visible at the point you accept it. That is the trade the whole design
makes, applied to dynamism: nothing is paid for by an application that
does not ask.

## Calibrate the benefit honestly

Spec §11 is unusually candid and worth quoting in substance:

> In a mature retained GUI, **the tree walk is rarely the bottleneck.**
> Frame time goes mostly to text shaping and rasterization, GPU command
> submission, and damage computation.

So making traversal several times faster buys a few percent of a frame,
while **invalidation architecture — completely orthogonal to whether the
types are static — decides whether you touch three nodes or three
thousand.** A `Box<dyn Widget>` tree with correct relayout boundaries
beats a static tree with sloppy invalidation by an order of magnitude,
and no amount of monomorphization closes that gap.

The first profile of a live frame here confirmed the calibration exactly:
layout and paint were a rounding error, and everything else was
rasterisation. `docs/results-frame-cost.md` has the stage table and what
was done about it.

**The two advantages that are real, and that nothing else gives:**

* **Zero allocation for the entire UI** — not "few", zero, gated by a
  counting allocator in the test suite (`docs/results-allocation.md`).
  This is why Slint went the same way: microcontrollers with no allocator
  and a memory budget that must be known before shipping.
* **Const-folded rigidity** — a dynamic tree checks a flag at runtime,
  per node, per frame. The static version does not have the check.

If neither of those is worth anything to your project, the honest answer
is that the static tree is not buying you much.

## What actually kills GUI framework projects

Also §11, also worth taking seriously: layout and traversal are the
tractable, enjoyable part and roughly a sixth of the work. The years go
to **text** — shaping, bidi, complex scripts, font fallback, IME
composition, selection across all of it — and **platform integration** —
multi-window, HiDPI, per-monitor scaling, clipboard, drag and drop.

None of that gets easier because the tree is static. The spec's own
recommendation is to compose rather than build: take an existing shaping
and rasterisation stack and make the contribution the node-table
architecture. `docs/text-render-adoption-decision.md` is where that
recommendation was tested against a specific candidate and declined, with
reasons.

## Residual costs you will actually feel

* **Binary size.** Reduced by the const table, not eliminated.
* **Error messages and rust-analyzer** at deep generic nesting.
* **Refactoring friction** — moving a widget changes types up the chain.
* **Authoring ergonomics.** Nobody hand-writes deeply nested generic
  trees at scale. Every project down this road built a DSL layer: Slint
  invented a language and a compiler, SwiftUI needed result builders plus
  compiler special-casing. Here the DSL is `#[derive(StaticTree)]`, and
  the real price is the same one — you own a macro, its generated code,
  and its error spans.

## So: who should not use this

* **Anything that redraws everything every frame** with no stable
  identity between frames. Immediate mode wins; the spec says so.
* **Anything whose interface shape is data**, defined at runtime from a
  server response or a user-editable document. You can express it — that
  is what `Dyn` and `List` are — but you would be paying for a static
  tree and then not using it.
* **Anything that needs a mature platform story today.** Multi-window,
  dialogs, clipboard and complex-script text exist here
  (`docs/plan-capabilities.md`), but "exists" and "has survived a decade
  of other people's edge cases" are different claims, and
  `docs/competitive-feature-gaps.md` is an honest per-framework list of
  the difference.

And who should: an application with a **known, fixed interface shape**,
a **memory budget that has to be provable**, or a target with **no
allocator at all**. That is the case the whole design is built around,
and it is the case where nothing else in Rust does quite this.
