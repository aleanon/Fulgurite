# M10 — the embedded profile

*Planned 2026-09-04. Replaces "parity phase B" as the next milestone
(`docs/plan-architecture-v2.md`). Status: **not started**.*

## The premise

> "Zero allocation for the entire UI — not 'few', zero. This is why
> Slint went this way: microcontrollers with no allocator and a fixed
> memory budget known before shipping."
> — `docs/00-handoff-static-tree.md`, §"Performance expectations"

That sentence, or a paraphrase of it, appears in the handoff, the README
and `docs/plan-lumen-parity.md`. **Nothing demonstrates it.**
`crates/lumen-static-shell/tests/alloc_gate.rs` proves that a *steady-
state frame* performs no allocation on a desktop with a working
allocator — a real result, and a much weaker claim than the one being
made. No build of this project has ever targeted a machine without an
allocator, so "no allocator required" is an intention.

M10 makes it true, gated and measured — or finds out where it is false.
Either outcome is worth more than the current situation, which is a
claim nobody has tested.

## Why before more widgets

`docs/plan-lumen-parity.md`'s own summary: the gating decisions "should
be settled before any widget beyond the current four is written, because
every widget encodes both." Allocation is that kind of decision.
`TextBuffer::Clusters` is already an associated type precisely so a
fixed-capacity storage can pair with a fixed-capacity cluster array
(B9). Writing eight more widgets and then discovering each assumes `Vec`
is the same mistake one level up.

## The starting position, measured 2026-09-04

Better than expected. This is what M10 is actually up against:

**Nine `std::` references in the core**, and three are already gated:

| file | what | disposition |
|---|---|---|
| `bind.rs` | `std::cell::RefCell` | → `core::cell` |
| `event.rs` | `std::collections::VecDeque` | → `alloc::collections` |
| `text.rs` (×4) | `std::borrow::Cow` | → `alloc::borrow` |
| `reload.rs` | `std::thread_local!` | already behind `reload` |
| `inbox.rs` | `std::sync::mpsc` | gate behind `std` (T10.1) |

**Five float-math calls**, all `std`-only for `f32` in a `no_std` build:
two `.floor()`, two `.ceil()`, one `.abs()`, in `cx.rs`, `repeat.rs` and
`input.rs`. That is a smaller surface than a dependency on `libm` would
be worth (T10.2).

**The allocation-free path already exists and is exercised.**
`pass::layout`, `pass::paint` and `pass::hit_test` take *slices*. The
233 tests in this repository already own their arrays —
`[NodeState; Ui::NODE_COUNT]` and `[Style; Ui::NODE_COUNT]` — because
`NODE_COUNT` is a `const` at the concrete type. `Host` is a convenience
that allocates; it is not the architecture.

**What genuinely needs an allocator divides on one line:**

> **A compile-time node count is allocation-free. A runtime content
> count needs an allocator.**

- Alloc-free: `Label`, `Button`, `Fixed`, `Spacer`, `Either` (T8.0
  removed its arenas), `Option`, `ScrollView`, `TextInput` with a
  fixed-capacity storage, and every derived container.
- Needs `alloc`: `Repeat` (a `Vec<T>` window **and** an `Arena`), `List`,
  `Dyn`, `Assembled`, and `DisplayList` itself.

Proving that sentence is M10's whole job.

---

## T10.0 — how paint reaches pixels without a list — **DECIDED**

`Node::paint(&self, st, i, out: &mut DisplayList)` is the blocker:
`DisplayList` is `Vec<Cmd>` plus `String`. Two shapes were considered.

- **A fixed-capacity list**, `DisplayList<const N: usize>`. The const is
  viral — it reaches every `paint` signature, every hatch, and the
  derive's generated `paint`.
- **Paint into a sink.** `Node::paint(&self, st, i, out: &mut dyn
  Painter)`, where the trait is `DisplayList`'s *existing* builder
  surface: `rect`, `text_weighted`, `rounded_rect`, `shadow`,
  `push_clip`, `pop_clip`.

**Decided: the sink.** Reasons, in order:

1. It is the shape this project has chosen three times already for the
   same reason — `dyn TextMeasure` (T5.2), `dyn GlyphSink` (T7.2b), `dyn
   Renderer` — namely that the **caller owns the storage**.
2. It removes the whole list on embedded rather than just its
   allocation: an immediate-mode rasteriser implements `Painter` and no
   command list is ever built.
3. It is **16 `paint` sites** (measured: `ops` 4, `hatch` 4, `scroll` 1,
   `input` 1, `repeat` 1, `assemble` 1, `widget` 1, `pass` 1, derive 2),
   not a viral const.
4. It composes with M8b. `Incremental` retention needs a retained list
   and keeps one; `Rebuild` needs none. Embedded is sink + `Rebuild` +
   no damage ring + no allocation, and the desktop path is unchanged.

The one thing the sink cannot carry is `Cmd::Text`'s `(offset, len)`
range into `DisplayList::text`. The trait therefore takes `&str`
directly and `DisplayList`'s impl interns it, exactly as it does now.

**Gate:** every existing test passes unchanged; `DisplayList` is the
only `Painter` in the workspace at the end of this task, so the
refactor is provably behaviour-preserving before anything depends on
it.

## T10.1 — `no_std + alloc`, and the feature polarity — **DECIDED**

Point the nine references at `core`/`alloc`, gate `inbox` behind `std`
(it is `mpsc`, and deferred work off the frame loop is not an embedded
concern), and declare `#![cfg_attr(not(feature = "std"), no_std)]` with
`extern crate alloc`.

**The features are `std` and `alloc`, both default-on. There is no
`no_std` feature.**

```toml
default = ["std"]
std     = ["alloc"]
alloc   = []
```

The intuitive spelling — a `no_std` feature with
`#[cfg(not(feature = "no_std"))]` on everything else — inverts the one
rule Cargo features have: **they are additive and unified across a
build**. A `no_std` feature *subtracts*, so one dependent enabling it
would strip `std` from every other crate in the graph that never asked,
in a build that still compiles.

This repository has already been bitten by that unification. The
justfile records why `--workspace` cannot produce the shipped shape:
"lumen-static-agent, examples/counter and fixtures/hot-app turn on
`observe`/`reload` and cargo unifies features across the selection."
Today that is an annoyance about which build is authoritative. With a
subtractive feature it becomes a build that is quietly missing things.

An embedded consumer writes `default-features = false`, or
`features = ["alloc"]` for the middle tier — the three-tier shape
`serde` and `hashbrown` use.

**Gate:** `cargo check -p lumen-static --no-default-features --target
thumbv7em-none-eabihf`, added to `just static` (it is a check, not a
build, so it costs seconds). The target is available through rustup and
nothing else in the workspace needs it.

## T10.2 — float math without `std`

`f32::floor`, `ceil` and `abs` are `std`-only. Five call sites.

**Do not take a `libm` dependency for five calls.** Write them as
`const fn`s in a small `num` module against the ranges that actually
occur: layout coordinates are finite and bounded by the viewport, and
every one of the five is already guarded by a finiteness check or
followed by a clamp. `abs` is a sign test; `floor`/`ceil` on a value
known finite and within `i32` range are casts.

**Gate:** a property test over the finite `f32` range that the
replacements agree with `std`'s to the bit, run in the `std` build where
both exist. That is the only way to know the substitution is honest, and
it costs one test.

## T10.3 — the alloc-free static path

With T10.0 and T10.1 done, an application that uses only the alloc-free
widget set, owns its own state and style arrays, and paints into an
immediate-mode `Painter` should touch no allocator at all.

Two things stand in the way and both are `Repeat`:

- `window: Vec<T>` — the materialized rows.
- `arena: Arena` — their `NodeState` and `Style`.

**Decision to make here, not now:** whether `Repeat` gains
caller-supplied storage (the shape `TextBuffer::Clusters` already uses
for text) or simply moves behind `alloc` with `List`, `Dyn` and
`Assembled`. The second is simpler and defensible — a virtualized
million-row list is not an embedded use case — and it keeps the dividing
line exactly at "runtime content count". The first is more useful and
more work. **Recommendation: move it behind `alloc` in M10, and revisit
if an embedded app actually wants a bounded list.** Recording it so the
choice is visible rather than defaulted.

**Gate:** `fixtures/embedded/` — a crate that builds for
`thumbv7em-none-eabihf` with `#![no_std]`, a `#[global_allocator]` whose
`alloc` is `panic!("allocated")`, a small tree of the alloc-free widgets,
and a `layout`+`paint` into a checksumming `Painter`. An allocation
becomes a link-visible abort rather than a number to interpret. Run
under `just static` as a `cargo check`, and under `just check` as a real
build.

## T10.4 — name the price of the hatches

`List`, `Dyn`, `Assembled`, `Arena` — and `Repeat`, per T10.3 — behind
the default-on `alloc` feature. Not a demotion: the point is that "what
does dynamism cost me" becomes answerable **in the type system** instead
of in prose. A `--no-default-features` build simply does not have them,
and the compiler says which line wanted one.

**Gate:** the `alloc`-less build of the core compiles with the escape
hatches absent, and the doc comment on each names `alloc` as its
requirement.

## T10.5 — the number

`docs/results-embedded.md` currently says, correctly, that no number has
been measured. This task fills it in:

- `.text` and `.bss` for the T10.3 fixture at `thumbv7em-none-eabihf`,
  against a stated threshold.
- Peak RAM its `[NodeState; N]` and `[Style; N]` arrays occupy at a
  stated node count, and the per-node figure that follows — stated once,
  cited elsewhere (AGENT.md).
- Whether the allocator-panics fixture links and runs.

**Gate:** the threshold is written down *before* the measurement, so the
gate can say no. A profile with no measured footprint is a claim, not a
profile.

---

## What M10 is not

- **Not a driver, a display or a board.** The fixture computes a
  checksum; it does not talk to hardware. Proving the framework fits is
  a different task from proving a screen works, and only the first is
  about this architecture.
- **Not `no_std` for the shell, the agent or the macros.** The shell
  owns fonts, a rasteriser and a window; the agent owns JSON. Both are
  desktop by nature and stay `std`. Only `lumen-static` is in scope.
- **Not a promise about `Repeat`.** See T10.3.

## Risks

- **The `Painter` refactor touches every widget.** It is the one M0-class
  change in M10, which is why it is first and why its gate is "every
  existing test passes unchanged".
- **`thumbv7em` has no `std`, so the gate is a `cargo check`,** not a
  test run. What it proves is that the code compiles for a target with
  no allocator and no OS — which is exactly the claim, but it is not the
  same as running. The allocator-panics fixture (T10.3) is what raises
  it from "compiles" to "does not allocate", and it runs on the host.
