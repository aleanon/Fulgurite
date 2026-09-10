# The component boundary

Spec §9 opens with a sentence the rest of the framework rests on: *"This
is the decision to anchor the framework on."* Not because component
boundaries are unusual — everybody has them — but because here **four
different mechanisms are deliberately made to coincide at the same
seam.**

## Four things, one boundary

A component boundary is simultaneously:

**An incremental-compilation firewall.** Its const sub-table is opaque to
the root, so editing a leaf recompiles one small crate rather than the
whole interface. This is the direct fix for the compile-time cost that
[What it costs](./costs.md) lists — and without it, a very large
application is unbuildable regardless of how fast it runs.

**The hot-reload unit.** One dylib, one save/fresh/restore cycle, and it
is fast because the rebuild is one crate rather than the world.

**The erasure boundary.** Inside a component, `#[widget(fuse)]`
monomorphizes freely — static calls, no `&dyn`. At the seam, dispatch is
by kind. That makes the overhead dial **local**, which is precisely what
a cargo feature cannot be: a feature is unified across the whole build,
so one dependent's choice becomes everyone's.

**The observability namespace.** A stable path that survives a reload,
so an agent, a screen reader and a change journal all name the same node
the same way.

The argument for making them coincide is short: *if these four boundaries
do not coincide, you end up with four mechanisms that disagree with each
other.*

## What that buys in practice

You choose a boundary once, and you get compile isolation, reload
granularity, dispatch cost and observability naming from the same
decision. Splitting a growing interface into components is therefore not
a stylistic tidy-up — it is the operation that keeps compile times
bounded and reload times sub-second as the tree grows.

## Retention lives at the same granularity

`Retention` is a type parameter on `Host`, not a cargo feature, and for
the same reason: `Rebuild` must stay alive as the **oracle**
`Incremental` is tested against. "Same pixels for the same input" is not
a test you can write if the thing you are comparing against has been
compiled out.

`Rebuild` costs nothing when unused because its `observe` body is empty —
a call the optimizer removes, not a branch taken per command.

## And so does provenance

`NodeDesc` carries a `SrcLoc` in `observe` builds: the file and line
where the node was declared. It turns an agent's report from "a button
has zero hit area" into a file and a line — and it composes with hot
reload, because the node is identified by an offset in a table rather
than by a generated key, so patching that line and swapping the dylib
leaves the identity intact.

The whole development surface is behind one cfg that compiles to nothing
in release. The const tables exist either way; only *exposure* is gated,
which is what makes the release cost genuinely zero rather than small.
