# Embedded and `no_std`

This is the target the whole design is built around, and the chapter is
short because most of the work shows up elsewhere: the escape hatches
being explicit, `Style` being budgeted per byte, animation having no
clock, images carrying an id rather than pixels. All of that is this
chapter's constraint, spent early.

## Three tiers, all additive

```text
default = ["std"]
std     = ["alloc"]
alloc   = []
```

An embedded consumer writes `default-features = false`, or
`features = ["alloc"]` for the middle tier.

## There is deliberately no `no_std` feature

This is the part worth understanding, because it looks like an omission
and is a decision.

Cargo features are **additive and unified across a build**. A subtractive
feature — one that *removes* `std` — would let a single dependent strip
`std` from every other crate in the graph that never asked for it, in a
build that still compiles. The failure would surface deep inside someone
else's crate rather than at the point of the mistake.

So the polarity runs the other way: `std` is a feature you have by
default and turn *off* for yourself, never one another crate can take
away from you. `docs/plan-embedded.md` T13.1 has the argument.

The same reasoning appears twice more in the framework: `observe` and
`reload` both *require* `std` explicitly, because they pull crates that
are not `no_std` — and without the requirement,
`--no-default-features --features alloc,observe` would resolve cleanly
and then fail deep inside a dependency.

## What is available at each tier

| tier | you get | you lose |
|---|---|---|
| `core` | the tree, layout, paint, events, `Either`, `BakedFace`, animation, i18n | `List`, `Dyn`, `Repeat`, `String` storage |
| `alloc` | the escape hatches, owned text | — |
| `std` | `observe`, `reload`, the shell, the windowing crate | — |

`Either` being in the bottom tier is not an accident. It is the hatch
that keeps both branches resident, which costs storage and buys freedom
from the allocator — see [Containers and escape
hatches](./containers.md).

## The gate that keeps it true

`just thumb` builds both bare-metal tiers after every task, and it is not
ceremonial: over M15 it caught three module-gate mistakes and one
`std`-only `f32::sqrt` that review did not.

That is the general shape of every `no_std` claim here — a build in CI
rather than a promise in a comment. `docs/results-embedded.md` measures
the footprint, and `just firmware-size` reproduces it.

## What the device cannot do

`docs/plan-embedded.md` T13.0 states the target's limits up front rather
than discovering them later, which is worth reading before planning an
embedded application. The frame interval comes from the bus rather than
the CPU, and the stack budget is a hard constraint the design had to be
shaped around — a tree deep enough to overflow it does not compile, which
is one of the two things [Topology in the type](./topology.md) said the
static shape buys.
