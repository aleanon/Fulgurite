# `Either<A, B>`: the table shape, reopened

*2026-09-02. M4/T4.2 shipped `Either` as one table node with off-table
branch arenas, on the ground that a merged `A + B` const table needs
`generic_const_exprs`. Three reviews of the plan disproved that. This
records what is actually true, what the shipped shape costs, and a
recommendation. Status: **DECIDED and BUILT 2026-09-04 — Option A** (padded merged table, bound by `hatch::MAX_EITHER_NODES`; `EitherN` not needed so far). T8.0 is done; what shipped is at the bottom.*

## What was claimed, and what is true

**Claimed:** a generic impl cannot build `[NodeDesc; A::NODE_COUNT +
B::NODE_COUNT]` on stable Rust.

**True:** it cannot build that *type*. But nothing needs it.
`Widget::SUB_TABLE` is `&'static [NodeDesc]` — a slice — so the length is
a value, not a type parameter. A padded const array does the job:

```rust
const MAX: usize = 64;

impl<A: Widget, B: Widget> Merged<A, B> {
    const N: usize = 1 + A::NODE_COUNT + B::NODE_COUNT;
    const PADDED: [NodeDesc; MAX] = {
        assert!(1 + A::NODE_COUNT + B::NODE_COUNT <= MAX, "merged table exceeds MAX");
        let mut t = [NodeDesc::EMPTY; MAX];
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 2, NodeDesc::CONTAINER);
        splice_child(&mut t, 1, A::SUB_TABLE, 0);
        t[1].next_sibling = (1 + A::NODE_COUNT) as u32;
        splice_child(&mut t, 1 + A::NODE_COUNT, B::SUB_TABLE, 0);
        t
    };
}
impl<A: Widget, B: Widget> Widget for Merged<A, B> {
    const NODE_COUNT: usize = Self::N;
    const SUB_TABLE: &'static [NodeDesc] = {
        let (head, _) = Self::PADDED.split_at(Self::N);
        assert!(table_is_well_formed(head));
        head
    };
}
```

Verified independently on stable `rustc 1.94.0`, no feature gates, using
the crate's own `splice_child` and `table_is_well_formed`:

- `Merged<Merged<Merged<F, S>, F>, Merged<S, F>>` — three levels — yields
  a well-formed DFS pre-order table, checked at const-eval and at runtime.
- Spliced into a real `#[derive(StaticTree)] #[widget(column)]` struct:
  `NODE_COUNT = 6`, every node gets a **tree-absolute** index.
- Setting `MAX = 2` is a **compile error** (`E0080: evaluation panicked:
  merged table exceeds MAX`). The bound is proof-carrying.

`&mut [T]` in `const fn` has been stable since 1.83; `split_at` in const
since 1.71. Nothing here is recent.

## What the shipped shape costs

Two things, both because of the mistaken reason:

1. **Agent ids inside an `Either` are branch-relative**, not table
   indices. The handoff's §6 row promised tree-absolute addressing; the
   `docs/plan-architecture-v2.md` M4 outcome recorded giving it up as the
   price of stable Rust. It was not the price of anything.
2. **Four heap `Vec`s per `Either`** — `a_state`, `b_state`, `a_style`,
   `b_style`. A merged table puts both branches' state in the parent's
   parallel arrays, allocated once for the whole tree. Under the
   zero-allocation constraint these four are not free, and they are the
   kind of allocation the no-allocator profile cannot make at all.

Also inherited: `Either::layout` writes the branch's size straight into
`st[i]` without `resolve_self`, so its own width/height style is dead and
it cannot be a clip scope under A2 (`docs/text-and-scrolling-decisions.md`,
consequences §9). A merged-table `Either` is an ordinary container node
and gets both for free.

## Options

| | table | agent ids | heap | rodata | user spells |
|---|---|---|---|---|---|
| **Keep** — one node, off-table arenas | 1 | branch-relative | 4 `Vec` | none | nothing |
| **A — padded `MAX`** | `1 + A + B` | tree-absolute | none | `MAX × 48 B` per distinct `Either<A,B>` | nothing |
| **B — exact `const N`** | `1 + A + B` | tree-absolute | none | exact | `Either<A, B, 7>` |

Rodata for A, measured: `size_of::<NodeDesc>()` is 48 B release (72 with
`observe`), so `MAX = 64` is ~3 KB per instantiation. Raising `MAX` from
64 to 1024 across the probe's instantiations grew `.rodata` by 138 KB.
Real, and a trade rather than a wall.

B has zero waste and an ugly signature: the `N` is checked
(`assert!(N == 1 + A::NODE_COUNT + B::NODE_COUNT)`) so a wrong value is a
compile error, but the author has to count.

## Recommendation — adopted 2026-09-04

**Option A for `Either`, with `MAX` a crate-level constant the embedded
profile may lower; Option B kept available as `EitherN` if anyone hits
the rodata.** Reasons:

- It restores what §6 promised and M4 wrongly gave up, and removes four
  allocations per `Either`.
- 3 KB per distinct `Either<A, B>` is small on desktop, and on embedded
  the *current* shape's four `Vec`s are worse than 3 KB of rodata — they
  are heap, which that profile may not have.
- The same mechanism generalizes: the enum derive's "cannot paint variant
  content" limit (M4 T4.4) exists because variant arenas are not
  retained; a padded merged table would retain them the same way.

**Not recommended:** doing this before T8.0. Nothing built so far depends
on `Either`'s internal shape, but a `ScrollView` inside an `Either` would,
and re-deciding after that means migrating it.

## What re-deciding touches

`hatch.rs` (`Either` impl and its `Reloadable`), `tests/hatch.rs`, the
agent's `TreeRef` (which currently special-cases off-table branches for
`Either` the way it does for `Repeat`), and the three docs that stated
the wrong reason — already corrected to say so.

## What shipped (T8.0, 2026-09-04)

`Either<A, B>` has `NODE_COUNT = 1 + A + B` and a merged `SUB_TABLE`
built by `EitherTable<A, B>::ARRAY` — `[NodeDesc; MAX_EITHER_NODES]`
filled with `splice_child` in const-eval, handed out as
`split_at(N).0`. Exceeding the bound is a compile error naming the type.
The two arena fields are gone: an `Either` now holds **no heap at all**,
where it held four `Vec`s.

Confirmed by the change, not just predicted by it:

- **Branch nodes are ordinary table nodes.** `node`, `node_mut`,
  `handler` and `hit` all delegate by index range, and a `Hit` inside a
  branch names an absolute table index — the `(row, node)` indirection
  that addressed branches like repeater rows is gone. Agent selectors,
  lints, `#[widget_id]` and stylesheet `#id` rules reach branch
  internals with no special case; `crates/lumen-static-agent` needed no
  change at all, which is the clearest evidence the special case was
  only ever a workaround.
- **Each branch node has its own style slot.** `Either::defaults()`
  returned *branch A's* because both branches shared one slot; it now
  returns a plain container's and each node answers for itself.
- **The hidden branch is zeroed every frame**, the way an empty
  `Option`'s subtree is, so nothing downstream reads stale geometry.
  `tests/hatches.rs` probes three points across a 300×300 hidden icon
  and asserts none of them lands in it.

### One thing the option table did not predict

Putting a second branch on the table exposed a hole that had been open
since M4: **`observe::emit_table` announced absent subtrees to the
screen reader.** An empty `Option`'s nodes, and now a hidden branch's,
are in the table with zeroed geometry — and were emitted as ordinary
nodes with zero bounds. A reader would walk into the branch the user is
not looking at and read it out. Zero-area nodes are now marked
`set_hidden()`, which keeps them in the tree (no orphans) and tells the
reader to skip them. `Option` had the bug first; `Either` is what made
it visible.
