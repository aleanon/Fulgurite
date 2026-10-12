# Monomorphization: per type, not per position — measured

> **Machine.** rustc 1.94.0, `--release`, `opt-level = 3`, `lto = false`,
> `panic = "abort"`, x86-64. Absolute byte counts are not portable across
> toolchains; the ratios taken inside one run are what this document
> claims.

*2026-09-11. Written because the book asserted the opposite and a reader
caught it. The question: does a widget type used at two tree positions
produce two copies of its code?*

## The claim under test

The design spec §4 warns that recursive generic descent emits a
specialised `layout` and `paint` "**per distinct subtree type** — that
is, per tree *position*, not per widget kind", and puts binary-size bloat
in its consequences table.

`book/src/costs.md` restated that as **"two buttons in different places
are two instantiations."** That sentence is wrong, and this is the
measurement that settles it.

## Census of a shipped binary

`examples/counter`, debug build, symbols demangled. Its tree is eight
nodes over six distinct types:

| type | fields declaring it | `::layout` symbols |
|---|---:|---:|
| `Button<Counter>` | 2 (`dec`, `inc`) | **1** |
| `Label<&'static str>` | 2 (`title`, `hint`) | *see below* |
| `Label<String>` | 1 (`value`) | *see below* |
| `Label<S>` — both storages together | 3 | **2** |
| `Spacer` | 1 | **1** |
| `Ui`, `Controls` (derived) | 1 each | **1** each |

Eight nodes, six distinct types, **six `layout` instantiations**. Two
`Button<Counter>` at different positions share one body; three `Label`
fields produce two bodies because there are two distinct storage types.

Instantiation count is the **distinct-type count**. It is not the node
count and it is not the position count.

## Does code size grow with nodes, with types, or both?

Three arms of a synthetic tree, same dependency, same profile. A
four-field row struct is the unit. `.text` and `.rodata` of the rlib:

| arm | nodes | distinct structs | `layout` symbols | `.text` | `.rodata` |
|---|---:|---:|---:|---:|---:|
| base — 4 reused | 17 | 2 | 5 | 11 552 | 348 |
| **A** — 24 reused | **97** | **2** | **5** | **27 831** | 668 |
| **B** — 4 reused + 20 distinct | **97** | **22** | **25** | **109 144** | 1 758 |

A and B declare **the same number of nodes**. They differ only in whether
those nodes reuse one struct or introduce twenty.

| quantity | cost |
|---|---:|
| a node of a type already used (A − base, ÷80) | **≈ 203 B** |
| a distinct struct, at equal node count (B − A, ÷20) | **≈ 4 066 B** |
| ratio | **≈ 20×** |

`.rodata` moves by hundreds of bytes across all three, so the growth is
code, not const tables.

## Both mechanisms are real, and they are different

**Per type.** Each distinct struct gets its own `layout`, `place`,
`paint` and `hit`. Twenty new structs cost twenty new sets. This is the
dominant term and it is what §4 is about.

**Per declared field.** The derive unrolls per-field code inside the
*container's* own methods — `node_mut` expands one indexed block per
field, and the other passes are the same shape. So declaring more
children grows the parent's code linearly even when every child is the
same type. That is arm A's 203 B per node.

What does **not** happen is the thing the book claimed: reusing a type at
another position does not re-emit that type's methods.

## Verdict

**§4's warning stands; its gloss does not transfer to this codebase.**

"Per tree position" is accurate for the encoding §4 is warning
against — `Row<(A, B, C, …)>`, where a node's type *contains its whole
subtree*, so two positions almost never share a type and the distinct-type
count approaches the node count. That is where "tens of thousands of
instantiations" comes from, and it is a real trap.

It does not describe what shipped. Two other documents already said so
and were not consulted when the book was written:

* Spec §5's supersession note, 2026-09-01: *"§4's trap is avoided the
  same way this section intended: children are called through `&dyn`, so
  code is per **kind**, not per position."*
* `docs/layout-and-storage-decisions.md`: *"Kind count multiplies by
  instantiation, not position … That is per **type**, so §4's trap stays
  shut."*

Nominal structs are what shuts it. `Controls` is one type wherever it
appears; `Row<(Label, Button)>` and `Row<(Button, Label)>` are two.

## What this does not measure

* Inlining. A single monomorphization may still be inlined at several
  call sites, which duplicates machine code without duplicating symbols.
  That is ordinary optimizer behaviour in any Rust program, bounded by
  heuristics, and it is not the trap §4 names.
* `#[widget(fuse)]`, which emits static calls instead of `&dyn`. Its
  effect on code size is unmeasured here;
  `docs/results-step6-fuse.md` covers what it does to the table and to
  agent visibility, and records that its *speed* claim was superseded.
* Real applications. Three synthetic arms bound the mechanism; they say
  nothing about how many distinct structs a real interface needs.

## Reproducing

No gate runs this — `fixtures/binsize` prints one number for one binary
with no threshold, and nothing in the tree compares code size against
tree size. To redo it, build a crate depending on `fulgurite` with
`default-features = false` and three `lib.rs` variants: a four-field
container of one derived row struct; the same with twenty-four fields of
that struct; and four of that struct plus twenty structurally identical
but separately named ones. Then compare `size -A` on the rlibs and count
`nm -C | grep '::layout$'`.

The binary census is reproducible directly:

```
cargo build -p counter --target-dir target/book
nm -C target/book/debug/counter | grep -E 'Node>::layout$'
```
