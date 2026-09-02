# Decisions: text storage, single-pass flex, style storage, §6 hatches

*2026-09-01. Four questions raised against `docs/plan-lumen-parity.md`'s
"architectural decisions that must be made first". Two of them change the
design; one closes a deviation; one is a build order.*

---

## 0. The safe accessor path is now the target architecture

**DECIDED 2026-09-01.** `docs/results-safe-prototype.md` measured it at
+8% traversal on desktop-scale trees, *faster* below ~100 nodes, half the
per-node static data, and no `unsafe` in a release build. Adopting it now
rather than later avoids porting the widget library twice, and it is what
makes §4d (enum variants) possible at all.

Consequences to plan around:

- `NodeDesc` loses `kind` and `offset` (24 B → 16 B — *later 48 B, once M1's `wid` and `classes` fat pointers landed; measured 2026-09-02*); `KIND_LIST` and the
  derive's kind-rebasing disappear.
- `Repeat`, `Button`/event dispatch and the `observe` ops must be ported
  — they are unsafe-path only today. `Repeat`'s `RefCell` should go with
  it, since a `node_mut` accessor lets layout mutate the window directly.
- **Traversal is recursive** (`docs/results-traversal.md`, decided
  after this section was written): the accessor `node(i)` survives for
  *index addressing* only; layout/place/paint recurse through `&dyn`
  children and read no table.
- **Split the traits so the app-state type stays out of the hot path.**
  `Node`/`Tree` (layout, paint) carry no `S`; a separate `Handlers<S>`
  accessor returns `Option<fn(&mut S, &EventCtx)>` for dispatch. Layout
  and paint — the verification targets and the frame-time cost — stay
  monomorphic and `S`-free, and `S` appears only on the event path where
  it is unavoidable.
- Keep the `safe_no_inline` dial: embedded wants accessor code
  O(component types), desktop wants it inlined.

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

### Why the text lives in the *widget*, not beside it

The alternative considered: leave the `String` in application state (a
sibling field) and give the widget a getter — `Label<S> { get: fn(&S) ->
&str }`. It is attractive: no duplicated storage, one source of truth,
capture-free and therefore reload-safe.

**Rejected, because of caching.** Shaping text is the expensive part of a
real frame (§11: "the years go to text"), so `up` must not re-shape every
frame — it must cache and invalidate. Owned storage can know when to
invalidate, because mutation goes through the widget and can set a dirty
bit. A getter cannot: the widget has no way to observe that app state
changed, so it would have to re-shape or re-hash the string every frame,
which is exactly the cost the cache exists to avoid.

> **Corrected 2026-09-01 (`bindings-decision.md`).** The last clause was
> too broad. Re-*hashing* or comparing is nanoseconds; re-*shaping* is
> what the cache exists to avoid, and the two differ by orders of
> magnitude. The conclusion stands and is strengthened: `#[bind]` is
> precisely the getter's "compare instead of re-shape" variant, and it
> works only because the storage is owned — a getter has nowhere to keep
> the previous value to compare against.

So the model is the ordinary retained one, and the same one Flutter and
Qt use: **application state is the source of truth in sibling fields;
widget fields hold the presentation copy, and owning that copy is what
makes caching possible.** An explicit update step copies one into the
other — which is the same `update` function §7's `Msg` discipline and
`docs/events-decision.md` already imply, and the piece the hot-reload
fixture visibly lacks (its `counter` never reaches the screen).

> **Refined 2026-09-01 (M6, `bindings-decision.md`).** The update step
> is no longer hand-written or hand-called. `#[bind(target, "fmt {}")]`
> declares it at the field and `#[derive(App)]` generates the body;
> `Host::pump` calls it between input and layout. The hot-reload fixture
> now derives its `status` label this way, closing the gap named above.

Storage is therefore `Dirty<S>`: a wrapper whose `DerefMut`/setter marks
the node dirty, wrapping whichever `AsRef<str>` the profile chose —
literally §3's `text: Dirty<String>, cache: Cell<Size>` sketch.

---

## 2. Not flex — iced's `Length` model, in four flat passes

> **SUPERSEDED IN PART 2026-09-01.** The `Length` model stands. The
> *four flat passes* do not: `docs/results-traversal.md` measured
> recursive descent 34–39% faster than flat loops, and with recursion
> `Length` needs **one visit per node** — the parent gets each child's
> final size back from the call, so there is no intrinsic-estimate pass
> and no correction pass. The passes table below is kept as the record
> of why four were needed *without* recursion. Implementation:
> `docs/plan-architecture-v2.md` M2.

**Taffy is rejected** (it builds a second node tree beside ours, which is
the duplication the flat table exists to avoid, and Lumen's own CP5
measured what maintaining it costs).

**CSS flexbox is also rejected, and it turns out we do not want it.**
iced ships no flexbox and loses nothing that real UIs need; it has four
length modes per axis:

| `Length` | meaning |
|---|---|
| `Fixed(n)` | exactly `n` |
| `Shrink` | content size (intrinsic) |
| `Fill` | share the remainder equally with other `Fill`s |
| `FillPortion(k)` | share the remainder in ratio `k` (`Fill` == `FillPortion(1)`) |

That covers what flexbox is actually used for — toolbars, sidebars, split
panes, forms, lists — while dropping precisely the parts that force
iteration:

- no `flex-basis` (a base size is just `Fixed` or `Shrink`);
- no `flex-shrink`, so nothing is squeezed below its content and there is
  nothing to redistribute;
- no min/max clamping interacting with distribution, therefore **no
  freeze-and-redistribute loop at all**.

**This deletes the deviation.** The four-pass scheme below was going to
have to document "distribution is single-round, unlike CSS". With
`Length` there is no second round to omit: the model *is* single-round by
construction, and the result is not an approximation of anything.

Other things worth taking from iced: its `Limits` (a min/max box) is our
`Constraints` under another name, and its `layout::Node` tree — sizes and
positions kept *separate from the widgets* — is our `NodeState` array,
which is mild confirmation that the split is the natural one. What we
keep that iced does not have: iced re-lays-out the whole tree every
frame, where the rigid partition and dirty tracking let us skip.

### The four passes

> **Note 2026-09-02 (F7.5).** This heading was followed by the Taffy
> paragraph from earlier in the section, duplicated verbatim — a
> copy-paste artifact in the most-cited document in `docs/`. The four
> passes it introduces are the table further down. The whole scheme is
> superseded in any case: recursion gives single-pass layout for free
> (`docs/results-traversal.md`), and only the `Length` model survives.

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

Still flat, still stack-free, still O(n), still no iteration.

Two partitions make pass 1 cheap, and both are compile-time facts the
derive can emit: rigid subtrees have constant intrinsic size and are
skipped outright, and **only `Shrink` subtrees need measuring at all** —
`Fixed` is known and `Fill` comes from distribution. In a typical UI most
containers are `Fill` or `Fixed`, so pass 1 touches a small fraction of
the tree. That is the same shape as the existing rigid/flexible split,
which measured −58%/−32% on layout when rigidity was dense.

Cost: one extra traversal, ~33% more layout work than today's three
passes — against a measured baseline where layout is roughly 60% of the
frame in our benchmark and 5–15% in a real one (§11).

### The one remaining limit

A container whose *cross* size is content-derived hands down pass-2
constraints based on intrinsic estimates. Its own final size is still
correct (pass 3 is bottom-up), but a pathological width→height→width
chain will not converge in one shot. Every non-iterative engine makes
this trade, iced included.

§1 lists "being a research project on layout algorithms" as an explicit
non-goal and calls the layout model "settled and boring on purpose".
`Length` is exactly that: it is what a real toolkit shipped, it needs no
iteration, and it is less machinery than flexbox rather than more.

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

> **Closed 2026-09-02 (F4.5).** `style::cascade_deep` recurses into
> off-table arenas, so a rule reaches a repeater's template row, a
> `List`'s items, both `Either` branches and a `Dyn` island's content.
> `Host::set_stylesheet` uses it. Recursion is by *template* — an arena's
> instances share one style row — so a rule matching inside a template
> applies to every instance of it; per-instance styling would need
> per-instance rows and a selector that can name a row, and neither
> exists.
>
> Worth recording that this limit was documented here for the repeater
> and inherited *silently* by `Either`, `List`, `Dyn` and the enum
> derive, which is the difference between a decision and an accident. It
> also made `#[widget_id]` inside a template dead for agent selectors as
> well as for styling.
>
> One case remains: an arena nested inside another arena's template (a
> `List<Repeat<_>>`) styles the outer but not the inner. The borrow
> checker will not let the walk descend while the template is borrowed
> shared and its style row exclusively; the shape that fixes it yields
> owned handles instead, which is F4.4's territory.
> `crates/lumen-static/tests/cascade_deep.rs`.
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

> **The three prerequisites in short:** **4a** one runtime visibility bit
> per node; **4b** `Option<W>` and `Either<A, B>` built on it; **4c**
> `Vec<W>`, which is `Repeat` with everything materialized.

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

### 4d. `enum` variants — **unblocked by §0**

Enum widgets are where the two dispatch paths genuinely diverge, and the
§0 decision settles it in their favour.

The unsafe path addresses a widget as `base + const_offset`. An enum's
variants **overlap in memory**, so no single const offset identifies "the
label inside variant B" — the offset is only valid when the discriminant
says so. Supporting enums on the offset path needs per-variant offset
tables selected at runtime, i.e. the const table stops being wholly
const.

The safe accessor path (`docs/results-safe-prototype.md`) has no such
problem: `node(i)` is a *match*, so it can match the discriminant first
and return the right branch. Enum variants fall out naturally.

This was an argument for the safe path unrelated to verification, and it
contributed to adopting it in §0. With the accessor path, enum widgets
are ordinary.

### 4e. `dyn` islands

A `Box<dyn Widget>` island is one table entry owning a runtime sub-table
and its own state slice — which is precisely what `Repeat` already does
(and what §8's `Assembled` does at startup). This is a generalization of
existing machinery, not new machinery.

---

## Ordering these against the parity plan

§0 (port to the safe path) comes first — doing it before the widget
library grows avoids porting every widget twice. Then §2 (`Length` + four
passes), which gates every widget because each one encodes the model.
Then §1, §3 and §4a–4c, all prerequisites for **Phase A/B** of
`docs/plan-lumen-parity.md`. §4d and §4e follow once §0 has landed.
