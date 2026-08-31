# Decisions: text storage, single-pass flex, style storage, §6 hatches

*2026-09-01. Four questions raised against `docs/plan-lumen-parity.md`'s
"architectural decisions that must be made first". Two of them change the
design; one closes a deviation; one is a build order.*

---

## 1. Text: generic over `AsRef<str>` — **yes, and it is better than either fixed choice**

`Label<S: AsRef<str> = &'static str>`. The pattern is already proven in
this codebase: `Button<S>` and `Repeat<T>` are generic widgets whose
`Widget` impl carries concrete associated consts at the use site, and
whose ops are monomorphized per instantiation.

What it buys, per profile:

| profile | choice | why |
|---|---|---|
| embedded / `no_std` | `&'static str` or `ArrayString<N>` | no allocator, bounded |
| desktop | `Cow<'static, str>` | literals cost nothing, computed values allocate only when they must |
| simple | `String` | obvious |

Three consequences worth stating:

- **It fixes a real wart.** `Label::restore` currently does
  `Box::leak(String)` because the field type cannot hold an owned value.
  With `Label<String>` (or `Cow`) the reload path serializes and restores
  naturally, and the deliberate dev-only leak disappears.
- **Kind count multiplies by instantiation, not position.** `Label<String>`
  and `Label<&'static str>` are distinct kinds with distinct ops. That is
  per *type*, so §4's trap stays shut; most apps use one.
- **The measure cache arrives with it.** Once text is real, `up` must not
  re-shape every frame. That means a cached `Size` plus a dirty signal —
  which is exactly §3's original sketch (`text: Dirty<String>`,
  `cache: Cell<Size>`). Generic storage and the dirty/cache machinery are
  one change, not two.

Default the parameter to `&'static str` so the embedded profile is the
path of least resistance.

---

## 2. Flex in a single pass — **yes, at four flat passes instead of three**

**Taffy is rejected** (it builds a second node tree beside ours, which is
the duplication the flat table exists to avoid, and Lumen's own CP5
measured what maintaining it costs).

### Why textbook flexbox is not single-pass

Two things break it: resolving flexible lengths is *iterative* (an item
that hits its min/max is frozen and the remaining free space is
redistributed — CSS §9.7), and cross-axis content sizing is *circular*
(a label's height depends on its final width, which depends on
distribution).

### What is actually needed

Flutter solves this locally: a flex container measures its inflexible
children first, then distributes the remainder to the flexible ones —
two child-layout rounds inside one container, no global reflow. That is
compatible with §2's "constraints down, sizes up, parent positions".

Mapped onto flat passes, it becomes **four loops over the same table**:

| pass | direction | what it computes |
|---|---|---|
| 1. intrinsic | bottom-up | each node's natural size under unbounded constraints |
| 2. distribute | top-down | each container knows its own final size and its children's intrinsic sizes → free space → grow/shrink → writes children's *final* constraints |
| 3. final | bottom-up | each node's real size under those constraints — **text rewraps correctly here, because its final width is known** |
| 4. place | top-down | positions |

Still flat, still stack-free, still O(n), still no iteration. Rigid
subtrees skip pass 1 entirely (their intrinsic size is constant), so the
existing rigid/flexible partition keeps paying.

Cost: one extra traversal, ~33% more layout work than today's three
passes — against a measured baseline where layout is roughly 60% of the
frame in our benchmark and 5–15% in a real one (§11).

### The deviation, stated honestly

**Distribution is single-round.** If an item hits its min or max during
pass 3, the residual is not redistributed to its siblings the way CSS
would. Consequences are small (a few px of slack in a container whose
children are all bounded) and the alternative — a freeze-and-redistribute
loop — is exactly the iteration this design is built to avoid.

§1 lists "being a research project on layout algorithms" as an explicit
non-goal and calls the layout model "settled and boring on purpose". A
documented single-round flex is in that spirit; unbounded iteration is
not.

The second known limit: a container whose *cross* size is content-derived
hands down pass-2 constraints based on intrinsic estimates. Its own final
size is still correct (pass 3 is bottom-up), but a pathological
width→height→width chain will not converge in one shot. Same trade every
non-iterative engine makes.

### Note for the intrinsic pass

The audit's two-probe trick (`crates/lumen-static-agent/src/audit.rs`)
already measures intrinsic size using the existing `up` op under generous
constraints. Pass 1 is that idea promoted from a dev-surface check to a
first-class layout phase — which also means the audit stops needing its
own probe and reads pass 1's output instead.

---

## 3. Style values: **a parallel array, not widget fields**

Recommended: `[Style; NODE_COUNT]`, keyed by node index, exactly like
`NodeState`.

```rust
pass::cascade(tree, &SELECTORS, &mut styles);   // one more flat pass
pass::layout(tree, &styles, &mut states, root_c);
```

Why this shape and not fields on each widget:

- **It mirrors the machinery that already works.** `NodeState` is a
  parallel const-sized array; `Style` is the same thing with different
  contents. The cascade is one more flat pass, selectors matching against
  static per-node data.
- **Widgets stay small.** Style on every widget means every `Label`
  carries padding, background, border, font — `size_of::<Label>()`
  explodes and the §3 "values in the fields" property degrades into
  "everything in the fields".
- **Zero allocation survives**: `[Style; NODE_COUNT]` in `.bss`.
- **It is diffable for free.** `ui.lastChange` already diffs `NodeState`;
  pointing the same journal at the style array extends "what changed" to
  styling with no new mechanism.
- **Reload-correct by construction**: computed style is derived, so it is
  `#[reload(reset)]` — recomputed after a swap, never migrated.

Selector inputs (id, classes) are *static per node* and belong in the
const table beside `wid`, promoted out of the `observe` gate.

This also closes an existing deviation: `#[widget(spacing = 4.0)]` bakes
spacing into a generated `fn` today (`docs/step2-design-notes.md` §8).
With a style array it becomes an initializer or a stylesheet rule, and
the attribute constant goes away.

**Known limit to design around:** the array is keyed by *table* node, and
repeater instances are not in the table — so all rows of a repeater share
their template's computed style. That is usually what you want; per-row
overrides (a selected row, a zebra stripe) need either a small per-row
style delta or state flags the cascade can key on. Decide when repeater
styling is actually built, not before.

---

## 4. §6's escape hatches: build order

All four are marked **DECIDED** in the handoff and none exist. They are
prerequisites for any non-trivial UI, and they are our own spec debt.
Ordered by dependency:

### 4a. Visibility — the shared mechanism (build first)

`Option<W>` and `Either<A, B>` both reduce to "these table entries exist
but this subtree is inactive this frame". That needs one runtime bit per
node — a flag on `NodeState` (not the const table; it is per-frame
state). Inactive subtrees take zero size and are skipped by place, paint
and hit testing, reusing the skip machinery the passes already have for
containers.

### 4b. `Option<W>` and `Either<A, B>`

`NODE_COUNT` = `W::NODE_COUNT` (Option) or `A + B` (Either); both hold
memory for every branch always, which §6 correctly calls a *win* —
toggling is free and the hidden branch keeps its state. With 4a in place
these are small.

### 4c. `Vec<W>` — mostly done

`Repeat<T>` with `item_extent` virtualization already is this, minus the
non-virtualized case. A `Vec<W>` hatch is `Repeat` with every instance
materialized, sharing the same side-arena machinery.

### 4d. `enum` variants — **and a finding**

Enum widgets are where the two dispatch paths genuinely diverge.

The unsafe path addresses a widget as `base + const_offset`. An enum's
variants **overlap in memory**, so no single const offset identifies "the
label inside variant B" — the offset is only valid when the discriminant
says so. Supporting enums on the offset path needs per-variant offset
tables selected at runtime, i.e. the const table stops being wholly
const.

The safe accessor path (`docs/results-safe-prototype.md`) has no such
problem: `node(i)` is a *match*, so it can match the discriminant first
and return the right branch. Enum variants fall out naturally.

This is an argument for the safe path that has nothing to do with
verification, and it should be weighed when that decision is reopened.

### 4e. `dyn` islands

A `Box<dyn Widget>` island is one table entry owning a runtime sub-table
and its own state slice — which is precisely what `Repeat` already does
(and what §8's `Assembled` does at startup). This is a generalization of
existing machinery, not new machinery.

---

## Ordering these against the parity plan

1, 3 and 4a–4c are prerequisites for **Phase A/B** of
`docs/plan-lumen-parity.md` and should land before the widget library
grows past its current four. 2 (four-pass flex) gates every widget, so it
comes first of all. 4d is blocked on the safe-path decision and can wait.
