# Results: M30 — one crate owns the widgets

*M30. Owns every figure the move cites. Machine: i9-13900KF. Flash rows
target `thumbv7em-none-eabihf`.*

## What moved, and what did not

Ten modules left `lumen-static` for `lumen-static-widgets`:

| module | types |
|---|---|
| `ops` (the widget half) | `Label`, `Button`, `Fixed`, `Spacer` |
| `toggle` | `Toggle`, `CheckBox`, `Radio`, `Switch` |
| `slider` | `Slider` |
| `input` | `TextInput` |
| `textarea` | `TextArea` |
| `image` | `Image`, `Fit` |
| `scroll` | `ScrollView` |
| `select` | `Select` |
| `popup` | `Popup`, `Anchor` |
| `glass` | `Glass` |

**What stayed is not widgets.** `Either`, `List`, `Dyn` and `Repeat` are
containers a user never places by hand — the derive emits them — and
`lumen_static::ops` keeps the layout helpers derive-generated code calls.
A widget is something you put in a tree; those are how a tree is built.

## The number the old split was argued from

`lumen-static-widgets`' own manifest argued the split from the embedded
budget: *"an embedded profile that pulls a pie chart it cannot draw has
paid `.text` for nothing."* That is a real concern and it is **not what
the crate boundary was protecting**.

| | `.text` | of the 64 KB ceiling | Toggle symbols |
|---|---:|---:|---:|
| widgets in the core (before) | 27,036 B | 41.3% | 3 |
| widgets in this crate (after) | **27,024 B** | **41.2%** | 3 |

**Twelve bytes smaller**, on the leaf `no_std` profile that links a
`Button` and three toggles. The gate was never the crate: it is
`#[cfg(feature)]` and the linker, and both drop what a tree does not
name whichever crate it lives in. The optional tier still pays for
itself the same way — `alloc`-needing widgets stay behind `alloc`, and a
leaf profile takes neither.

## The precondition: the crate could not build `no_std` at all

Found while sizing the move, and fixed before it
(`fix(widgets): the crate could never build no_std`). The crate declared
`#![cfg_attr(not(feature = "std"), no_std)]` and offered
`alloc = ["lumen-static/alloc"]`, and neither could work: the dependency
line omitted `default-features = false`, so every build pulled
`lumen-static`'s `default = ["std"]`.

```
cargo tree -p lumen-static-widgets --no-default-features --features alloc
  lumen-static-widgets
  └── lumen-static feature "default"     <- always
```

The failure surfaced as *"std is required by lumen_static because it
does not declare `#![no_std]`"* — an error **about core**, which builds
`no_std` cleanly on its own. That misdirection is why an advertised tier
went unexercised: nothing had ever built this crate for a `no_std`
target.

## Two couplings, and they were the only two

Core referenced the moved types in ~90 places and **two were real code**.
The rest were rustdoc, `Cmd::Image`/`Cmd::Glass` (display commands, not
the widgets) and `Length::Fixed` (a length variant, not the widget).

| coupling | resolution |
|---|---|
| `bind::format_into(&mut Label<S>, …)` | both functions only called `set_str`, so the dependency was on one method: `bind::TextTarget` |
| `Dyn::<S>::default()` built an `ops::Spacer` | a private `hatch::Vacant` — zero-sized, paints nothing, takes no hits |

**The derive needed no change at all.** Its `::lumen_static::ops::`
emissions are all to the layout helpers — `inner_limits`, `loose_child`,
`resolve_self`, `align_offset`, `hide_subtree`, `record_painted` — which
stayed in core. A proc macro that had named a concrete widget type would
have made this move impossible; it never did.

`Vacant` is `IS_RIGID` and zero-sized where `Spacer` filled its parent.
An empty island should occupy nothing; the old default silently claimed
the whole box until something was put in it.

## `ops` was two modules wearing one name, and F6.1 said so

`ops.rs` held the widget vocabulary *and* the machinery the derive
calls. Its own rustdoc named the fix and deferred it:

> Splitting the two into a widgets module and a hidden private one is
> the honest end state (`docs/plan-review-fixes.md` F6.1); hiding the
> machinery is most of the benefit for a fraction of the churn.

The crate boundary made it compulsory rather than merely honest, so
**F6.1 is closed**: `lumen_static::ops` is the helpers,
`lumen_static_widgets::ops` is the vocabulary. The name is kept on both
sides rather than improved — `ops` is where a reader who knows this
codebase looks for `Label`, and a rename would cost every `use` line in
the workspace a second edit.

## Cost of the move

| | |
|---|---|
| files whose imports changed | **158** (106 path rewrites, 31 prelude additions, 21 split import lists) |
| manifests gaining the dependency | 17 — four of them as a **dev**-dependency |
| test binaries green | **194** |
| `.text`, embedded leaf profile | 27,036 → **27,024 B** |

Four crates take it as a dev-dependency only, because their `src` never
referenced a widget — `lumen-static` itself, `lumen-static-macros`,
`lumen-static-shell` and `lumen-static-agent`. That core's own tests
need the widgets is a dev-cycle, which Cargo permits and which is the
honest shape: the dependency is real and it is only the tests'.

## One feature seam the move exposed

A widget's `observe` impl is gated on **this crate's** `observe`, not on
`lumen-static`'s. `lumen-static-agent` forces core's on non-optionally,
so before the move a `Label` published its role and value for free;
after it, an application building the agent must also enable
`lumen-static-widgets/observe` or `ui.getTree` reports a label with
`role: GenericContainer` and no value.

`examples/showcase` already had the right shape —
`agent = ["lumen-static-winit/agent", "lumen-static-widgets/observe"]` —
and it is now what every agent-building consumer does. Found by
`counter-window --drive2` failing with exactly that symptom, which is
the live gate earning its place: no unit test can see a feature seam.

**The agent's own tests had the same seam, hidden by unification**
(M38's R13-F7, fixed in M43/T43.6): its dev-dependency on the widgets
took no `observe`, and `cargo test --workspace` supplied it through
core's dev edge, so `cargo test -p lumen-static-agent` alone failed 19
tests while every gate was green. A unit test *can* see a feature seam,
if the crate is built alone — which is what `just test-crates` does for
every library under `crates/`.
