# Writing your own widget

Two traits, and the split between them is the first thing to understand.

## `Widget` and `Node` are two halves of one thing

**`Widget`** carries the compile-time facts — `NODE_COUNT`, `DEPTH`,
`SUB_TABLE` — which is exactly what a trait object cannot have.

**`Node`** carries the methods and is object-safe, so a container can
reach a child as `&mut dyn Node`.

Every widget implements both. The practical consequence, and the first
thing most people hit: calling `.node(i)`, `.layout(..)` or `.defaults()`
needs `Node` in scope. The prelude brings it in.

## The four methods you must write

```text
fn layout(&mut self, cx, limits, style: &[Style], st: &mut [NodeState], i) -> Size
fn place (&mut self, at: Point, style: &[Style], st: &mut [NodeState], i)
fn paint (&self, st: &[NodeState], i, out: &mut dyn Painter)
fn hit   (&self, st: &[NodeState], i, x, y) -> Option<Hit>
```

Three details in those signatures are decisions, not accidents:

**`&mut self` in `layout` and `place`.** Each node is visited exactly
once per pass, so exclusive access is free — and it lets a stateful
widget (a repeater's window, a measurement cache) mutate without interior
mutability.

**`paint` takes `&mut dyn Painter`, not `&mut DisplayList`.** A widget
describes *what* to draw and never names where it goes. A desktop host
collects into a display list; a host with no allocator implements
`Painter` directly on a rasteriser and no list is ever built.

**`paint` and `hit` do not get `&[Style]`.** Only `layout` and `place`
do. Resolved paint attributes are stamped into `NodeState` during layout
instead — see [Where state lives](./state-lives.md).

## `defaults()` must depend on the type alone

```text
fn defaults(&self) -> Style
```

**Never on `self`'s current state.** The cascade calls this when a
stylesheet is set, not per frame, and the arena containers snapshot it
once per template — so a state-dependent answer is frozen at whatever
the state happened to be and never revisited.

This has already gone wrong once. `Option::defaults` returned
`Style::default()` while empty, so an `Option` that was empty at
construction had its slot styled `Shrink` permanently: setting it to
`Some(Spacer)` later laid the spacer out at 0×0 instead of filling, with
nothing on screen to indicate why (`docs/plan-review-fixes.md` F3.2).

Runtime state belongs in `layout`, which runs every frame.

## The wrapper rule

This is the most important paragraph in the chapter, and it is a rule
about *provided* trait methods.

`Painter` has eleven methods and four of them are provided: `line`,
`gradient`, `image` and `glass` have default bodies that degrade to a
simpler command, and `text` forwards to `text_weighted`. Those defaults
are written for a **leaf** implementor — a rasteriser that only knows how
to fill rectangles.

A **wrapper** — anything that holds another painter and forwards to it —
inherits those bodies too, and a provided body **acts on `self`**. So an
inherited `image` does not forward the image; it rewrites it into
whatever the default degrades to, on the wrapper, and the real painter
downstream never sees it.

> **A provided method is right for a leaf and wrong for a wrapper,
> always.**

This is not hypothetical. `Deferring` overrode seven of `Painter`'s
eleven methods and inherited four, so **every `Line` and `Image` painted
under an overlay pass was silently dropped and every gradient
flattened** — four commands in, two out — until it was found by a review
reading the plan for a different feature.

The general form is worse than the instance: **when a trait gains a
method, every wrapper of it is a call site the compiler will not show
you.** If you add a `Painter` method, the compiler is happy and the
wrappers are wrong. Search for the wrappers by hand.

## Off-table content

If your widget owns instances the const table cannot describe, it also
implements:

```text
fn arena_count(&self) -> usize
fn arena_mut(&mut self, a: usize) -> Option<ArenaMut<'_>>
```

Zero for everything in the table. `Repeat` owns one, `Either` owns two,
`List` and `Dyn` own one each. The cross-cutting walks — the deep
cascade, the audit — need this to reach content the table does not
describe.

## And the three type-level items

```text
const KIND: &'static str;
fn type_defaults() -> Style;
const SEEDS: &'static [Seed];
```

These are on `Widget`, not `Node`, because none of them may depend on an
instance — and a *hidden* subtree has no instance at all, which is what
made the old instance-method spelling wrong rather than merely untidy
(see [Styling](./styling.md#when-the-cascade-runs)).

`KIND` is what `Selector::Kind` matches and what the agent's selectors
report. Follow the convention: a **leaf** gets a lowercase name, matching
`label`, `button`, `fixed`, `spacer`. See [Styling](./styling.md) for why
the convention is not uniform. Two widgets must not share one, which
`lumen-static/tests/seeds.rs` checks, because a copied `const KIND` is
how a `Kind` rule ends up styling a widget it was never written for.

`SEEDS` has one entry per node of the subtree, in `SUB_TABLE` order. For
a leaf that is a one-element array:

```text
const SEEDS: &'static [Seed] = &[Seed::new(Self::KIND, Self::type_defaults)];
```

A container splices its children's, which `#[derive(StaticTree)]` does
for you. There is deliberately **no default**: one that is right for a
leaf is silently wrong for a container, so the compiler asks every
implementation instead.

`leaf_common!` already forwards `Node::kind_name` and `Node::defaults`
to these, so a leaf that uses it writes nothing further.
