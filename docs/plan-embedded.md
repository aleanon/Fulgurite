# M13 — the embedded profile

*Planned 2026-09-04 as M10; **renumbered M13 and moved behind M11 the
same day**, after a five-agent review of this document found its
evidence substantially wrong and its scope roughly seven times what was
written. **Status: done 2026-09-05, T13.0–T13.9.** Corrections are
marked inline rather than quietly applied — the errors are the useful
part, and the review's were: every one of the four claims it overturned
turned out to matter, and the two gaps it did not find (no text stack
below `std`, and `TextBuffer::insert_str` returning `()`) were both
found by building rather than by reading. Results:
`docs/results-embedded.md` and `docs/results-retention.md`.*

> **What the review changed.** Four of this plan's "already true"
> claims were false: `grep "std::"` cannot see `thread_local!`, the
> style array has no non-allocating constructor, `TextInput` has no
> alloc-free storage, and the recorded type sizes were stale. It also
> found a blocker never considered — **stack depth** — and that only
> the `paint` signature (now M10a) actually blocked the widget work
> this milestone was scheduled ahead of.

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

## Why this is no longer first — **REVISED 2026-09-04**

The original argument was that "every widget encodes the allocation
answer", by analogy with the parity survey's rule about gating
decisions. The review took that apart: walk M11's own list and none of
the widgets would be written differently based on this milestone's
outcome. Toggles are a `Button` with a `bool`; tabs are `Either` plus a
header strip; the slider is a *routing* change; overlays are a layout
milestone. The one item with storage in it, `TextArea`, sits on
`TextBuffer::Clusters` — which this document itself cites as *already*
being an associated type for exactly this reason.

What genuinely blocked M11 was the `paint` signature, because every
widget adds a `paint` impl. That is now **M10a** and runs first. This
milestone runs after M11, and before it starts, someone has to name a
device (see below) — there is no user for the profile yet: no commit in
this repository has ever targeted an embedded platform.

## The starting position, measured 2026-09-04

Better than expected. This is what M10 is actually up against:

**Nine `std::` paths (three gated) — and that count is the wrong
metric.** It is arithmetically right and materially misleading, because
a `grep` for `std::` cannot see an unqualified prelude macro:

| file | what | disposition |
|---|---|---|
| `bind.rs` | `std::cell::RefCell` | → `core::cell` |
| `bind.rs` | **`thread_local!`, ungated** | **no `core` equivalent.** It is the runtime of `#[bind]` — the mainline derive path, not a dev feature. Decide: a `&mut` scratch buffer threaded through `format_into`, a critical-section static, or gate `#[bind]`'s formatting behind `std` |
| `event.rs` | `std::collections::VecDeque` | → `alloc::collections` — but see "input" below |
| `text.rs` (×5) | `std::borrow::Cow` | → `alloc::borrow`; two already behind `reload` |
| `reload.rs` | `thread_local!`, `eprintln!` | already behind `reload` |
| `inbox.rs` | `std::sync::mpsc` | gate behind `std` |

Counting every prelude identifier that needs an `alloc::` import under
`#![no_std]` — `Vec`, `String`, `Box`, `vec!`, `format!`, `to_owned`,
`to_string` — gives **~67 across 15 files**. So T13.1 is not a nine-line
diff; it is ~70 lines across most of the crate, plus a design decision
for `bind.rs`.

**Four float-math calls, not five.** `.abs()` is in `core` — verified by
compiling it under `#![no_std]`. The std-only surface is two operations
at four sites: `.floor()`/`.ceil()` in `cx.rs` and `repeat.rs`. Nothing
else is missed: `max`, `min`, `clamp`, `to_bits`, `is_finite`,
`signum` and `copysign` are all core, and float *formatting* works in
`no_std + alloc` because `Display` for floats lives in `core::fmt` —
though its flt2dec path is multiple KB of `.text` and belongs in the
budget.

**The allocation-free path *half* exists.** `pass::layout`,
`pass::paint` and `pass::hit_test` take *slices*, and there are 86
`[NodeState; N]` array literals in the workspace. But there are **zero**
`[Style; N]` literals: all 84 sites call `pass::default_styles`, which
returns a `Vec` and is the only way to build one. An earlier draft
claimed "233 tests already own their arrays"; they own their *state*
arrays. A non-allocating style constructor is work in this milestone,
not a starting position.

**What genuinely needs an allocator divides on one line:**

> **A compile-time node count is allocation-free. A runtime content
> count needs an allocator.**

- Alloc-free: `Label`, `Button`, `Fixed`, `Spacer`, `Either` (T8.0
  removed its arenas), `Option`, `ScrollView`, and every derived
  container.
- Needs `alloc`: `Repeat` (a `Vec<T>` window **and** an `Arena`), `List`,
  `Dyn`, `Assembled`, and `DisplayList` itself.
- **`TextInput` is on neither side yet.** An earlier draft listed it as
  alloc-free "with a fixed-capacity storage". No such storage exists:
  both `TextBuffer` impls (`String`, `Cow`) need `alloc`, and the only
  `ClusterBuf` impl is `Vec<Cluster>`. B9's associated type is the right
  machinery with no inhabitant — so building one is a task here (T13.5),
  and it is the thing that cashes B9 out.
- **Input is not covered by the sentence at all.** `InputQueue` is a
  `VecDeque`, so an app that wants *events* needs an allocator. An event
  backlog is a runtime *event* count, not a runtime *content* count.
  Either it becomes a bounded ring with a stated drop policy, or the
  line is extended to "a compile-time node count **and a bounded event
  count**" and an embedded host calls `pass::dispatch` directly.

Proving that sentence — with those three corrections in it — is this
milestone's whole job.

## The blocker the first draft never considered: stack

Traversal is recursive, and `MAX_DEPTH` is **64**. Measured frame sizes
for the derive-generated `layout` on 32-bit ARM: **256–272 B per level
in release, 568 B in dev**. At the framework's own maximum depth that is
**~17 KB of stack in release, ~36 KB unoptimised**, against typical
Cortex-M budgets of 4–16 KB. There is no overflow trap without MPU
setup: it silently corrupts `.bss`, which is exactly where the
`[NodeState; N]` array lives.

`desc.rs` currently claims `MAX_DEPTH` is "how §5's stack-depth concern
is retired: it becomes a fact about a constant rather than a runtime
hazard." That is true given a desktop stack. On this profile the
constant is a cap on a cost that must be *budgeted*, and `Dyn` is
documented as a hole in the guarantee anyway. **T13.0 derives a maximum
tree depth from a stack budget** — at 264 B/level a 4 KB stack allows
depth 15 — and amends that comment in the same commit.

---

## T13.0 — the device, and the stack budget — **DECIDED 2026-09-05**

Every threshold in this milestone is derived from a device, and "write
the threshold down before the measurement" is not satisfiable without
one. So:

### The target

**Cortex-M4F, `thumbv7em-none-eabihf`, 128 KB RAM, 512 KB flash** — the
STM32F411 class. Cheap, ubiquitous, and well served by Rust HALs. A
hardware single-precision FPU is a *requirement*, not a preference: this
framework is `f32` throughout — layout, clipping, the shadow's signed
distance — and soft-float on a Cortex-M0 would pay for that on every
coordinate.

**Display: 320×240 at 16 bpp over SPI, with its own GRAM** (an ILI9341
and its relatives). **No local framebuffer**, which is the decision the
whole class rests on: one would be **150 KB**, more RAM than the part
has. An immediate-mode [`Painter`] pushes to the bus and never builds a
list — which is exactly what M10a bought, and without it this device
class is not reachable at all.

### The budget

| | |
|---|---:|
| widget tree, 100 nodes @ ~190 B | ~19 KB |
| layout stack (see below) | 8 KB |
| framebuffer | **none** |
| left for the application, drivers and buffers | ~100 KB |
| const table in flash, 100 nodes | ~3 KB |

`~190 B` is `NodeState` (76) + `Style` (72) + the widget struct
(13–40) — the third of which no earlier version of this plan counted.

### The frame interval comes from the bus, not the CPU

A full 320×240×16 bpp frame is **1.23 Mbit**:

| SPI clock | full frame | ceiling |
|---|---:|---:|
| 20 MHz | 61 ms | 16 fps |
| 40 MHz | 31 ms | 33 fps |
| 62 MHz | 20 ms | 50 fps |

Layout and paint are microseconds against that. **The bus is the frame
budget**, which has a consequence large enough to have its own task —
see T13.8.

### The stack, correctly framed

The review measured **256–272 B per level** for the derive-generated
`layout` in release, and concluded ~17 KB at `MAX_DEPTH = 64`. That is
the right measurement and the wrong reading: the cost is *actual* depth,
not the cap. The tally is 4 deep and the settings panel 5, so a real
screen costs **~1.6 KB**.

`MAX_DEPTH = 64` is a compile-time ceiling on how deep a tree may be, so
what it bounds is the worst case a badly-built tree could reach. **The
embedded profile lowers it to 24** — 6.3 KB, comfortably inside the 8 KB
budget, and still four times deeper than anything built here. It is a
const, so exceeding it is a compile error naming the type, which is the
whole reason it exists.

`desc.rs` claims `MAX_DEPTH` "retires the stack-depth concern"; that is
true given a desktop stack and must be amended in the same commit that
lowers it.

### What this device cannot do, stated up front

- **No `alloc`.** So no `Repeat`, `List`, `Dyn`, `Assembled` (T13.4).
- **No `fontdue`.** It needs `alloc` and is a `std` crate, so the text
  stack is a gap with its own task — T13.7.
- **Not a 60 Hz UI.** The bus says 33 fps for a full frame; anything
  smoother comes from drawing less, not from being faster.

## ~~T10.0~~ — moved to M10a — how paint reaches pixels without a list

`Node::paint(&self, st, i, out: &mut DisplayList)` is the blocker:
`DisplayList` is `Vec<Cmd>` plus `String`. Two shapes were considered.

- **A fixed-capacity list**, `DisplayList<const N: usize>`. The const is
  viral — it reaches every `paint` signature, every hatch, and the
  derive's generated `paint`.
- **Paint into a sink.** `Node::paint(&self, st, i, out: &mut dyn
  Painter)`, where the trait is `DisplayList`'s *existing* builder
  surface: `rect`, `text_weighted`, `rounded_rect`, `shadow`,
  `push_clip`, `pop_clip`.

**Decided: the sink**, and **split out as milestone M10a** because it is
the only part of this plan that blocks M11. The decision and its task
breakdown now live in `docs/plan-architecture-v2.md` M10a; the reasoning
is kept here because it was argued here. Reasons, in order:

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

## T13.1 — `no_std + alloc`, and the feature polarity — **DONE 2026-09-05**

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

**Also gate `observe` and `reload` behind `std`.** Neither lists it
today, and both pull `accesskit`/`serde_json`, which are not `no_std`.
Without it, `--no-default-features --features alloc,observe` resolves
cleanly at the Cargo level and then fails deep inside a dependency
instead of at the point of the mistake.

**Gate:** `cargo check -p lumen-static --no-default-features
**--features alloc** --target thumbv7em-none-eabihf`. Not bare
`--no-default-features`: with the tiers above that means *neither*
`std` nor `alloc`, and `Vec`/`String`/`VecDeque` are still in the core
until T13.4 — so the stricter form belongs in T13.4's gate, not this
one. The target needs `rustup target add thumbv7em-none-eabihf`, which
makes it a prerequisite rather than a free check; `just static`
currently runs no cargo at all, so this either changes that recipe's
character or belongs in `just check`.

**Done. `just thumb`, in `just check`** — it compiles clean with zero
warnings. The conversion was smaller than the nine references suggested
and the surprises were all in the same place:

- **Eight explicit `std::` lines; the work was the implicit prelude.**
  `Vec`, `String`, `Box`, `VecDeque`, `Cow` and `vec!` arrive free under
  `std` and must be named from `alloc` without it — 19 files.
- **Six of those imports are `observe`-gated**, because the only `Vec`
  left in `ops`, `input`, `slider`, `textarea`, `toggle` and `widget` is
  the accesskit `extra` argument. Worth knowing: the alloc-free widget
  set T13.3 needs is closer than the raw grep implied.
- **`bind::format_into` is the one place `no_std` is measurably worse.**
  Its reusable scratch buffer is a `thread_local!`, which is std-only,
  and the `static` replacement is not `Sync` — so reuse without `std`
  needs `critical-section` (a dependency) or `unsafe` (an M0 violation).
  `no_std` therefore allocates one buffer per bound label per frame.
  Acceptable because the function only runs for a `Label<S>` whose `S`
  allocates anyway, so the alloc-free path never reaches it; the fix, if
  one is wanted, is a fixed-capacity scratch on T13.5's storage.
- `inbox` is behind `std` as planned (it is `mpsc`).

## T13.2 — float math without `std` — **DONE 2026-09-05**

**Two operations at four call sites**, not five: `.abs()` is in `core`
(verified by compiling it under `#![no_std]`). Only `.floor()` and
`.ceil()` are std-only, in `cx.rs` and `repeat.rs`.

**Do not take a `libm` dependency for four calls** — but do not write
the obvious cast either. A range-restricted `x as i32 as f32` cannot
pass the gate this task sets: measured against `std`, it disagrees for
**every finite `f32` with |x| ≥ 2³¹** (billions of values inside "the
finite range"), for `±inf`, for `NaN`, and for `−0.0`. And the inputs
are reachable: `Repeat::scroll` is a public field and `MIN_ITEM_EXTENT`
is 1/1024, so `scroll / item_extent` can be `+inf` or `NaN` through the
public API. Today's downstream saturation happens to mask it, which is
luck rather than design.

Write the domain-complete form instead — barely longer, and exhaustively
correct:

```rust
const fn floor(x: f32) -> f32 {
    // NaN, ±inf and |x| ≥ 2^23 are already integral.
    if !(x.abs() < 8_388_608.0) { return x; }
    let t = x as i32 as f32;
    if t > x { t - 1.0 } else { t }
}
```

with the one remaining divergence — `floor(-0.0)` and `ceil(-0.5)`
returning `+0.0` — either sign-corrected or documented.

**Gate: exhaustive over all 2³² bit patterns**, not sampled. At two
operations that is seconds in release, and it is a strictly stronger
gate than a property test — which is the reason to prefer the
domain-complete implementation over the cast.

**Built as `crates/lumen-static/src/num.rs`.** All 4,294,967,296 inputs
agree with `f32::floor`/`f32::ceil` *bit for bit*, in 12.9 s release
(`num::tests::exhaustive_agreement_with_std`, `just floats`, in
`just check`). Three notes on what the writing settled:

- The ±0.0 divergence is **sign-corrected, not documented**. Once the
  gate compares bit patterns rather than values it is a real failure,
  and it covers the ~2³⁰ inputs that round to a zero, so the two-line
  `zero_like` was cheaper than the exception it would otherwise need.
- `has_fraction` is bit math (`bits & 0x7fff_ffff < 0x4b00_0000`)
  rather than `x.abs() < 8_388_608.0`. That makes both fns `const`,
  and NaN and ±inf fall out of the *same* comparison — an all-ones
  exponent compares larger than 2²³'s — instead of needing an arm.
- Both are `pub const fn`, which the cast form could not have been.

## T13.3 — the alloc-free static path — **DONE 2026-09-05**

With T10.0 and T10.1 done, an application that uses only the alloc-free
widget set, owns its own state and style arrays, and paints into an
immediate-mode `Painter` should touch no allocator at all.

Three things stand in the way, and only two of them are `Repeat`:

- `window: Vec<T>` — the materialized rows.
- `arena: Arena` — their `NodeState` and `Style`.
- **`pass::default_styles` returns a `Vec` and is the only way to build
  a style array.** Add `default_styles_into(&tree, &mut [Style])` and
  keep the allocating form as the `alloc` convenience. This is the item
  an earlier draft missed by asserting the arrays were already owned.

Also: `style::cascade_subtree` allocates a witness `Vec<Style>` under
`#[cfg(debug_assertions)]` to check the caller's contract. That is gated
on the *profile*, not a feature, so an embedded debug build allocates
inside the cascade. Decide whether the check survives there.

**Decision to make here, not now:** whether `Repeat` gains
caller-supplied storage (the shape `TextBuffer::Clusters` already uses
for text) or simply moves behind `alloc` with `List`, `Dyn` and
`Assembled`. The second is simpler and defensible — a virtualized
million-row list is not an embedded use case — and it keeps the dividing
line exactly at "runtime content count". The first is more useful and
more work. **Recommendation: move it behind `alloc` in M10, and revisit
if an embedded app actually wants a bounded list.** Recording it so the
choice is visible rather than defaulted.

**Taken, as recommended — `Repeat` moves behind `alloc` in T13.4.**
Nothing found while building the fixture argued the other way: a
virtualized list is a runtime-content-count feature, and the dividing
line stays exactly there.

**Gate:** `fixtures/embedded/` — a crate with `#![no_std]`, a small tree
of the alloc-free widgets, and a `layout`+`paint` into a checksumming
`Painter`.

Three corrections to how an earlier draft specified it:

- **The allocator must abort, not `panic!`.** std's panic machinery
  boxes its payload, which allocates, which re-enters the allocator.
  `shell/tests/alloc_gate.rs` already has the right pattern — count over
  `System` with `try_with` — and says why in a comment.
- **The static gate is the strong one, not the dynamic one.** A
  panicking allocator only covers paths that execute; T13.4's
  `--no-default-features` build, where `Vec` cannot be *named*, is what
  actually proves the property. The emphasis was backwards.
- **A real `thumbv7em` binary needs `#[panic_handler]`, `#[entry]`,
  `panic = "abort"` and a linker script with a concrete memory map** —
  i.e. a board, which "What M13 is not" says is out of scope. So the
  fixture is a *library* check for thumb and a *binary* run on the host,
  and this plan should say that rather than imply a flashable artifact.

`scripts/check_no_unsafe.sh` scans `crates/*/src`, so the fixture's
`unsafe impl GlobalAlloc` under `fixtures/` does not violate the M0
rule — worth stating, because a reader will assume it does.

**Done, as `fixtures/embedded/`.** 7 nodes, 9 commands, **zero
allocations over 100 frames**, and the same checksum every frame. The
lib half compiles for `thumbv7em-none-eabihf` (`just thumb`); the bin
half runs in `just fixtures`.

- **`pass::default_styles_into(&tree, &mut [Style])`** is the item that
  unblocked it, exactly as this task predicted: `default_styles`
  returning a `Vec` was the only way to build a style array, so an app
  owning its own `[Style; N]` still had to allocate one to fill it. The
  allocating form is now a two-line wrapper over it, behind `alloc`.
- **The cascade witness is gated on `alloc` as well as the profile.**
  `debug_assertions` is a *profile*, not a feature, so an embedded debug
  build was allocating a whole second style array inside
  `cascade_subtree` — on the one target that can least afford it. It
  survives wherever a `Vec` is nameable, which is every build that has a
  stylesheet to get wrong.
- **The aborting allocator was validated by re-injecting the bug**: one
  `Vec::with_capacity(8)` inside `frame` turns the run into SIGABRT
  (134). A gate nobody has watched fail is a gate nobody has tested.
- The alloc-free widget set turned out to be *the defaults*:
  `Label` and `Button` both default their text storage to
  `&'static str`. The fixture uses the defaults rather than spelling
  them out, so it stays honest about what an embedded app writes.

## T13.4 — name the price of the hatches — **DONE 2026-09-05**

`List`, `Dyn`, `Assembled`, `Arena` — and `Repeat`, per T10.3 — behind
the default-on `alloc` feature. Not a demotion: the point is that "what
does dynamism cost me" becomes answerable **in the type system** instead
of in prose. A `--no-default-features` build simply does not have them,
and the compiler says which line wanted one.

**Gate:** the `alloc`-less build of the core compiles with the escape
hatches absent, and the doc comment on each names `alloc` as its
requirement.

**Two things this task turned up that the plan did not foresee.**

**1. `InputQueue` was a `VecDeque`** — and it is not an escape hatch, it
is the one queue `docs/plan-events.md` E0 exists to unify. Giving
embedded a second queue would fork exactly that. So it is now a
**fixed-capacity ring in every build**, `CAP = 64`: `Event` is `Copy`
and 16 bytes, so the whole queue is 1 KB inline, affordable on the
device (`docs/results-embedded.md`) and one less heap structure per host
on the desktop. One implementation, exercised by the existing suite
rather than by an embedded-only path nothing runs.

`push` returns `bool` and overflow **rejects the newest** event, not the
oldest: either choice breaks gesture pairing, but evicting the oldest
can strand a `PointerUp` whose `PointerDown` is gone — a click that was
never pressed — while rejecting the newest leaves the queued prefix a
coherent history. Drops are counted and readable through
`InputQueue::dropped`, on the same rule as the change journal's
`truncated` flag.

**Done. `just thumb` builds two tiers — `--features alloc` and bare
`--no-default-features` — and clippy-cleans both.** Behind `alloc`:
`Arena`, `Assembled`, `List`, `Dyn`, `Repeat`, and — not foreseen here —
`DisplayList`, `Renderer` and the whole `retain` module. Each names the
requirement in its own doc comment, not only in its `cfg`.

**Three things the bare build turned out to implicate beyond the
hatches.**

- **`DisplayList` and `Renderer`.** A list is one `Painter`, not the
  vocabulary; `Renderer::render` names `&DisplayList` in its signature.
  A host without a list paints straight through the trait, which is
  what M10a's seam was built for — this is the first build that
  *proves* the seam holds, because it is the first where the list
  cannot be named.
- **The whole `retain` module**, because `Retention::record` takes a
  `&DisplayList`. That is precisely T13.8's finding, arrived at from the
  other direction: the trait hard-wires the vocabulary. Until T13.8 a
  no-alloc build has no retention, which is honest — it rebuilds every
  frame by definition, and `Rebuild` is a ZST that does nothing.
- **`Widget::arena_count`/`arena_mut`.** Only `Repeat`, `List` and `Dyn`
  implement them and the derive never emits them, so they gate cleanly.

**2. T13.4's gate depends on T13.5, so T13.5 runs first.**
`TextInput<S: TextBuffer = String>` and `TextArea<S: TextBuffer =
String>` name `String` as a **default type parameter**. A default cannot
be removed by a `cfg` — the definition still has to name *some* type —
so the `alloc`-less build cannot compile until a fixed-capacity storage
exists to be the default instead. That is T13.5's `InlineText`, and it
is why the order in this file is not the order of work.

## T13.5 — an allocation-free text storage — **DONE 2026-09-05**

`TextInput<S: TextBuffer>` cannot be instantiated without an allocator
today: both `TextBuffer` impls need `alloc`, and the only `ClusterBuf`
impl is `Vec<Cluster>`. B9 built the associated type precisely so a
fixed-capacity storage could pair with a fixed-capacity cluster array,
and then nothing ever inhabited it.

`InlineText<const N: usize>` implementing `TextBuffer`, and
`InlineClusters<const N: usize>` implementing `ClusterBuf`, used by the
T13.3 fixture. This is the task that cashes B9 out and the one that
decides whether the associated type was worth having.

**Gate:** the fixture instantiates a `TextInput<InlineText<64>>`, types
into it through `pass::dispatch`, and allocates nothing.

**Done, and it answers this task's own question — the associated type
was worth having, and the gate proved it by failing.**

`crates/lumen-static/src/inline.rs`: `InlineText<N>` (N *bytes*, not
chars) and `InlineClusters<N>`, paired through `TextBuffer::Clusters`.
One cluster per byte is the worst case — every cluster boundary is a
`char` boundary and the shortest `char` is one byte — so `N` clusters
can never be too few for `N` bytes.

**The gate found a live defect on its first run.**
`TextBuffer::insert_str` returned `()`, which is sound only while every
implementation is unbounded: `TextInput::insert` advances its caret by
`s.len()` after inserting, so a storage that *refuses* leaves the caret
past the end of the text. Against `String` this was unobservable.
Against `InlineText` the first keystroke after the buffer filled tripped
its bounds assertion.

`insert_str` now returns `bool` and both `TextInput::insert` and
`TextArea::insert` advance the caret only on success. An unbounded
storage returns `true` always and pays nothing to say so. This is the
answer to "whether the associated type was worth having": a bounded
storage did not merely *fit* the B9 shape, it found a bug in the trait
around it that no `String`-only test could reach.

Two overflow rules, deliberately different:

- `set_from_str` **truncates** at a `char` boundary. It is the binding
  path — a formatted value too long for the field should show its
  prefix, as a narrow column does.
- `insert_str` **refuses whole**. It is the typing path, where a partial
  insert would either split a `char` or land a prefix of what the user
  typed.

`InlineClusters` overrides `GlyphSink::is_full`, which is what that
method was added for: `shape` checks it and stops early, so a run too
long for the buffer costs nothing to shape past the point where it would
be discarded.

## T13.7 — a text stack that works without an allocator — **DONE 2026-09-05**

`FontMetrics` is `fontdue`, which needs `alloc` and is a `std` crate,
and it lives in `lumen-static-shell` — which this milestone explicitly
keeps `std`. So on the target there is **no measurer at all**, and
T13.3's fixture would lay out a `Label` whose glyphs nothing can draw.
No earlier version of this plan named that, which made the alloc-free
widget set look more complete than it is.

`TextMeasure` is a trait, so the shape is settled; what is missing is an
implementation. A **pre-rasterised bitmap face**: glyph advances and
coverage bitmaps in `const` data, indexed by a sparse table, measured
and drawn with no allocation and no shaping. That is what every embedded
GUI does, and it is the honest answer for a device whose font lives in
flash.

What it gives up, said plainly: one size per baked face, no hinting, no
complex-script shaping. `docs/text-render-adoption-decision.md` already
records the last of those as deferred; the first two are new and belong
next to it.

**Gate:** the T13.3 fixture lays out and paints a `Label` and a
`TextInput<InlineText<64>>` with a baked face, allocating nothing, and
the glyph coverage it emits matches a desktop render of the same face
at the same size.

**Done.** `lumen_static::font::{BakedFace, Glyph}` is the machinery;
`fixtures/bake-font` is the generator; `fixtures/embedded/src/face.rs`
is DejaVuSans at 16 px — **95 glyphs, 9,010 bytes of flash**, which is
1.8% of the device's 512 KB. The fixture resolves **108 glyphs per
frame** through it and still allocates nothing.

- **The face lives in the application, not the core.** A GUI framework
  should not ship a font, and the split falls out naturally: `BakedFace`
  is generic `const`-slice machinery, the bytes are the app's.
- **Coverage is one blob with per-glyph offsets**, not a slice per
  glyph — that is how it lands in flash, one contiguous region and a
  table, rather than 95 relocations.
- **`glyph()` returns `Option`, and the fixture counts misses.** A face
  is a fixed set; a caller that silently substitutes a box cannot tell
  "this font lacks the character" from "this character is a box".
- **The fixture's `Painter` folds coverage, not the `&str`.** Folding
  the string would have proved text reached paint but not that anything
  could draw it — and "nothing can draw it" was precisely the hole this
  task exists to close.
- `fixtures/embedded/tests/baked_face.rs` re-rasterises every glyph with
  a live `fontdue` and compares advance, extent, bearings and every
  coverage byte. It **skips with the path printed** where the system
  font is absent, as `just check` skips the live gate without a display.

The two new deferrals — one size per baked face, no hinting — are
recorded in `docs/text-render-adoption-decision.md` beside the
complex-script one, as this task asked.

## T13.8 — damage without a retained list — **DONE 2026-09-05**

**The finding that makes this a task:** damage is worth *more* on this
device than on a desktop, not less. M8b bought 2.33 ms → 0.154 ms on a
desktop where the raster was the cost. Here the **bus** is the cost, and
redrawing a tenth of the screen is a tenth of 31 ms. That is a ~10×
difference in the thing the user actually feels.

But `Incremental` computes damage by keeping the previous
`DisplayList` — a `Vec<Cmd>` and a `String` — which is precisely what
an allocator-free, list-free host does not have. M10a records "embedded
is sink + `Rebuild` + no damage ring" as a *choice*; this task is the
evidence that it is the wrong one.

**What is already right:** `Retention` is a type parameter and `Rebuild`
is zero-sized, so a host that wants no retention carries none.

**What is not:** two things.

**Item 2 below is fixed; item 1 stands as recorded.**

1. `Host` holds `list: DisplayList` as a concrete field whatever `R` is
   — cleared, painted into and never read under `Rebuild` with an
   immediate-mode painter. It costs the embedded profile nothing, only
   because `Host` also holds two `Vec`s and so is not on that profile at
   all; an embedded host calls `pass::layout`/`pass::paint` with its own
   arrays, as all 233 tests already do. Worth recording so the next
   reader does not mistake `Host` for the architecture.
2. `Retention::record(&mut self, list: &DisplayList)` hard-wires the
   vocabulary. Even `Rebuild` is *typed* against a list it does not
   want, and a strategy that computes damage without storing commands
   cannot express itself.

**Proposal: retention becomes a `Painter` wrapper**, the shape this
crate has now chosen for `Scaled` and for every seam before it —

```text
paint → Retention(Scaled(sink))
```

— observing commands as they stream past and keeping per-command
`(hash, rect)` rather than the commands themselves. No stored list, no
text buffer, and it composes with the scaling wrapper already there.

Two reasons beyond embedded:

- **It should be cheaper on the desktop too.** `Incremental::record`
  currently copies the whole frame every frame; at 2 000 commands that
  is ~80 KB of memcpy, roughly **8 µs against an 89 µs render**
  (`docs/results-frame-cost.md`). `(hash, rect)` is 20 bytes per
  command against ~40 plus text.
- It removes the last coupling between `Retention` and `DisplayList`.

**Gate — and this one decides the task.** Streaming hashes every command
instead of comparing stored ones, so it may be a wash; that is a claim,
and this project measures claims. Benchmark both at the tally's 34
commands and at ~2 000, and **keep the copying implementation if
streaming is not faster** — the point is a host that needs no list, and
if that costs desktop time it should be a second `Retention` impl rather
than a replacement. `crates/lumen-static-shell/tests/damage_oracle.rs`
is the correctness gate either way: whatever computes the damage, the
partial redraw must stay pixel-identical to a full one.

**Done, and the gate came back clearly: streaming is faster, so it
replaces the copying form rather than joining it.** Numbers in
`docs/results-retention.md`; `damage_oracle.rs` and all 79 test binaries
pass unchanged.

`Retention` no longer names a display list. It is
`observe(hash, ink)` + `finish() -> Damage`, fed by `Recording`, a
`Painter` wrapper in the shape `Scaled` already uses and composing with
it: `paint → Recording(Scaled(sink))`. Three implementations now —
`Rebuild` (still a ZST, still the oracle), `Incremental` (a growable
buffer, `alloc`), and `Streaming<N>` (a fixed array, no heap).

**The whole `retain` module came back out from behind `alloc`**, which
T13.4 had had to put it behind for exactly the reason this task names.
That is the deliverable: a host with no allocator and no list can have
damage, on the device where damage is worth the most.

Four findings the measurement produced that this plan did not predict:

1. **`Streaming<N>` is 2× faster than `Incremental`** — 1.38 vs 2.71 ns
   per command, same algorithm. A fixed array indexed inline against a
   `Vec` that costs a pointer load and a second bounds check. The
   allocator-free implementation is the fast one, which is not the usual
   direction.
2. **`Recording` is free** — 0.08 ns per command, at the noise floor.
   That is what makes wrapping the painter an affordable seam rather
   than a tax.
3. **Damage with no list is cheaper than a list with no damage**:
   11.4 µs against a 12.8 µs floor at 2 000 commands. The M10a seam
   paying off in a number.
4. **Text comparison got better for free.** `Cmd::Text` holds a *range*
   into a text buffer, so "12" → "13" produced identical commands and
   the copying diff needed a special case to catch it. A stream is
   handed the `&str`; the case does not exist to forget.

The cost, stated once: equality is now a 64-bit hash, so a collision
would leave stale pixels — around 10⁻¹³ per frame at 2 000 commands,
with `damage_oracle.rs` standing over it.

## T13.9 — the number, last — **DONE 2026-09-05**

`docs/results-embedded.md` currently says, correctly, that no number has
been measured. This task fills it in:

- `.text` and `.bss` for the T13.3 fixture at `thumbv7em-none-eabihf`,
  against the threshold T13.0's device implies.
- Peak RAM at a stated node count — **state + style + the widget
  structs**, which no earlier version of this counted, and measured **on
  the target**: `NodeDesc` holds two pointers, so it is ~32 B on 32-bit
  against 48 B on the host.
- The stack figure from T13.0, and the maximum tree depth it implies.
- Whether the allocator fixture links and runs.
- Correct `docs/results-embedded.md`'s sizes in the same commit — it
  said 72/64 where `size_of` says **76/72**, and cited a document
  containing neither figure.

**Keep `f64` out of the frame path.** The only `f64` today is in
`observe`, behind a feature; `thumbv7em-none-eabihf` has a
single-precision FPU, so any `f64` becomes soft-float calls. Worth
stating because `repeat.rs` already notes that raising
`MAX_ADDRESSABLE_EXTENT` "means an `f64` offset".

**Gate:** the threshold is written down *before* the measurement, so the
gate can say no. A profile with no measured footprint is a claim, not a
profile.

**Done, thresholds committed first, in their own commit.** Numbers in
`docs/results-embedded.md`; `just firmware-size` reproduces them from a
**linked** `thumbv7em-none-eabihf` binary, because `cargo check` proves
none of this — it type-checks and stops, with no codegen, no link and no
sections. Every threshold met, `.text` at 31% of budget and `.bss` at
literally zero — *as measured 2026-09-05; `.text` is 34.3% of budget at
2026-09-10 and is a gate now, `docs/results-embedded.md`*.

**And the measurement earned its keep on the first link.** `.text` was
43 KB, a third of it `core::num::flt2dec` — float *formatting*, in a GUI
that formats no floats. `f32::clamp` panics when `lo > hi` or a bound is
NaN, and its panic message formats a float. Replacing it with
`num::clamp` at nine sites — one of them `Constraints::clamp`, which
runs for every node of every frame — took the whole firmware from
60,173 B to 33,677 B: **−26.5 KB, 44%**. Invisible on a desktop; found
in ten minutes by linking once for the target.

Two smaller corrections the numbers forced:

- **`MAX_DEPTH` stays 64.** T13.0 said the profile lowers it to 24; it
  does not, because `MAX_DEPTH` is a compile-time assertion with no
  runtime cost, so lowering it would only reject trees — on desktop
  too. The *budget* is real and is recorded instead.
- **`InlineClusters<64>` is 768 B**, 72% of the fixture's whole widget
  tree. One cluster per byte is the correct worst case and an expensive
  one; an embedded app should size `InlineText<N>` to its field.

---

## What M10 is not

- **Not a driver, a display or a board.** The fixture computes a
  checksum; it does not talk to hardware. Proving the framework fits is
  a different task from proving a screen works, and only the first is
  about this architecture.
- **Not `no_std` for the shell, the agent or the macros.** The shell
  owns fonts, a rasteriser and a window; the agent owns JSON. Both are
  desktop by nature and stay `std`. Only `lumen-static` is in scope —
  which is why the bitmap face of T13.7 cannot live in the shell either.
- **Not a board bring-up.** T13.0 names a device to derive budgets from,
  not a target to flash. The fixture is a library check for thumb and a
  binary run on the host; a real binary would need `#[panic_handler]`,
  `#[entry]` and a linker script with a concrete memory map.
- **Not a promise about `Repeat`.** See T10.3.

## Risks

- **`cargo check` proves less than the earlier draft claimed.** It
  type-checks a library and stops: no codegen, no linking, no missing
  intrinsics, no `#[panic_handler]` requirement (a *lib* never needs
  one), no `.text`/`.bss`, no stack usage — and with `--features alloc`
  it proves nothing about allocation either, since a library may name
  `Vec` freely without a `#[global_allocator]`. **It also never compiles
  derive output**, because the core contains no `#[derive(StaticTree)]`
  outside tests and `cargo check` builds neither tests nor doctests — so
  whether the macro's generated code is `no_std`-clean is untested until
  T13.3's fixture, two tasks later. The macro does emit `format!`,
  behind `reload`.
- **The `Painter` refactor is gone from this milestone** — it is M10a,
  and it is the one M0-class change. What is left here is broad but
  shallow: ~70 import changes, four float calls, three storage
  decisions.
