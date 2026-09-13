# M24 — the cascade's seed, and the `Select` it unblocks

> **Started 2026-09-13.** The ask, from a reading of
> `examples/showcase`: *"the dropdown — is it correct that it's a
> manually wired list of buttons? Shouldn't it reference a field with an
> enum, each selectable a variant, and what renders depend on the
> variant?"*
>
> Yes on both counts. **The enum switch the question describes has been
> in the derive since T4.4** (2026-09-01;
> `crates/lumen-static-macros/src/lib.rs`,
> `expand_enum`), and the showcase could not use it — because of a
> cascade defect that makes a hidden subtree resolve its style from the
> wrong type. That defect is this milestone's first task, and the rest
> falls out of it.

## 0. What the showcase hand-rolled, and what it bypassed

| showcase code | lines | what already existed |
|---|---:|---|
| `Panel` — 18 `Row` fields + `ROWS` | 42 | — (needs `Select`, T24.2) |
| `pick0`..`pick17` + `PICKERS` | 77 | — (needs `Select`, T24.2) |
| `sync_open`'s `[&mut Row; ROWS]` rebind | 37 | — (needs `Select`, T24.2) |
| `PanelRow` / `PanelLayer` spacer scaffold | 32 | **`Popup<W>`** — `popup.rs`, out-of-flow, `Anchor::Below`, overlay-deferred |
| `Demos` — 18 `Option`s + `fill_all`/`shown`/`clear` | 111 | **the enum switch** — `#[derive(StaticTree)]` on an enum |

**~300 of the showcase's 1657 lines**, and three quarters of them
reimplement something already shipped. That is not a criticism of the
example; it is a measurement of a framework seam, and the example is the
only thing that could have taken it.

## 1. The defect, measured

`cascade_range` (`style.rs:497`) matches a rule against the **table**
`NodeDesc` for `Id` and `Class`, but pulls `defaults(i)` and
`kind_of(i)` through `Node::node(i)` on the **live tree**. A hidden
subtree has no live node: `Option::<W>::node` returns the `Option` when
`None`, and the enum's `node` returns the enum itself for an inactive
variant's indices (documented there, deliberately, so the agent can walk
them).

Probed on a two-variant enum, `First(A)` active:

```
node 0  wid=""   kind_seen=Pick
node 3  wid=""   kind_seen=Pick     <- inactive variant's root, should be B
node 4  wid="b"  kind_seen=Pick     <- inactive variant's CheckBox

Id rule   -> width = Fixed(77.0)    works: Id reads the table
Kind rule -> width = Shrink         never applied
defaults  -> the CheckBox's own default width is Fixed(24.0);
             the enum reports Shrink
```

`Fixed(24.0) → Shrink` on a checkbox is a control with no size of its
own, which on screen is **a widget that looks like a label**. That is
the exact symptom reported against the showcase on 2026-09-12, and
`Demos::fill_all` is the workaround: build the tree with every demo
`Some`, cascade, then empty the stage.

**`fill_all` cannot be written for an enum.** An enum holds one variant
by definition, so there is no moment at which all variants are live.
The better primitive was already there and silently did not work, which
is why eighteen `Option`s got written instead.

### Why `Id` kept working, and what that tells you

`wid` is a compile-time property of the node that lives in the const
`NodeDesc`. `kind_name` and `defaults` are *equally* compile-time
properties — `Node::defaults`' own rustdoc has said so since F3.2:
**"Must depend only on the type, never on `self`'s current state"** —
and they are read through `&dyn Node`, which requires an instance.

A fact that cannot vary per instance, reachable only through an
instance, is a fact stored in the wrong place. The contract was written
down and then left to prose; this milestone moves it into the type.

## T24.0 — `Widget::SEEDS`: the type-level style inputs

Add to `style.rs`:

```rust
pub struct Seed { pub kind: &'static str, pub defaults: fn() -> Style }
```

and to `Widget`:

```rust
/// One seed per node of this subtree, in `SUB_TABLE` order.
const SEEDS: &'static [Seed];
```

`fn() -> Style` and not a `const Style`, and that is forced rather than
chosen: every `defaults()` body in the tree ends `..Style::default()`,
and `Default::default` is not a `const fn`. A thunk costs one pointer
per node of `.rodata` and keeps the bodies exactly as they read now.

Each widget moves its two bodies to the type: `const KIND` and
`fn type_defaults()` on `Widget`, with `Node::kind_name` and
`Node::defaults` forwarding so the agent surface and every existing
caller are untouched. The derive splices children's `SEEDS` the way it
already splices `SUB_TABLE`.

**Gate:** `SEEDS[0]` must agree with the instance methods, for every
widget in both crates. That is the drift check — the two spellings of
one fact are exactly the thing that rots.

**Injections, and what one of them found.** Six were run
(`crates/lumen-static/tests/seeds.rs`,
`crates/lumen-static-widgets/tests/widgets.rs`):

| # | injection | result |
|---|---|---|
| A | `Toggle::kind_name` stops forwarding, returns the literal `"checkbox"` | fires — names `Switch<()>`, `"switch"` vs `"checkbox"` |
| B | the enum derive stops splicing its variants' seeds | fires — the hidden `CheckBox` seeds `"widget"` |
| C | `splice_seeds` writes nothing | fires — 3 tests, `Pair node 1: seed says "widget", the live node says "label"` |
| D | a widget's `const KIND` is changed | **green — a finding about the test** |
| D′ | `Chip` copy-pastes `Badge`'s `KIND` | fires — `two widgets both call themselves "badge"` |
| E | a widget's `KIND` is left as the trait default | fires — `kind 7 is "widget": never written` |
| F | `Popup` copy-pastes `ScrollView`'s kind | fires — 2 tests |

**D is the one worth keeping.** Every leaf reaches `Node::kind_name`
through `leaf_common!`, which forwards it to `Widget::KIND` — so the two
sides of the agreement assertion are *one expression*, and changing the
`KIND` changed both. The agreement test cannot fail for a forwarding
leaf, and it read as though it could.

The test was not weakened to suit the injection; the missing property
was found instead. What a kind actually has to be is **distinct**, and
the realistic way it stops being distinct is that a new widget is
written by copying the nearest one and `const KIND` rides along in the
copied block. Both widgets compile, both paint, and a `Kind` rule meant
for one silently styles the other. `no_two_kinds_collide` and
`no_two_widgets_share_a_kind` assert that, D′ and F fire on it, and the
agreement test now says in its own doc comment which half of it is
tautological.

## T24.1 — the cascade reads `SEEDS`

`cascade` takes `&[Seed]` instead of two `&dyn Fn` closures;
`cascade_deep`, `cascade_subtree` and `cascade_arenas` pass `W::SEEDS`.
The live tree stops being consulted for style at all, which is also
what makes a scoped re-cascade cheaper: no virtual call per node.

**Gate:** the probe above, as a committed test — a `Kind` rule reaches
an inactive enum variant, and a `None` `Option`'s subtree resolves its
own defaults.

**Injection:** restore the closure that reads the live tree and watch
both halves fail; then check the *other* direction, that an active
variant still cascades correctly, so the fix is not "everything gets
the leaf's defaults now".

## T24.2 — `Select<E>` over `Popup`

The dropdown as a widget: a trigger, a `Popup` panel anchored `Below`,
rows from a `&'static [&'static str]`, and a selection reported as an
index. Retires `Panel`, `PICKERS`, `PanelRow`, `PanelLayer` and
`sync_open`.

This is **T11.5**, deferred 2026-09-04 for want of the `stack` axis,
which shipped 2026-09-10. `docs/plan-widgets.md` §T11.5 lists what is
still open there — dismissal on click-outside is an event-routing
question, not a layout one — and that list is the spec.

## T24.3 — the showcase's stage becomes an enum

`Demos` → `#[derive(StaticTree)] enum Demo { Label(DLabel), … }`.
`shown()` and `clear()` cease to exist: one-of-N by construction. The
comment in `stage.rs` that says a field missing from `clear()` is *"the
one bug this shape makes possible and a `Dyn` did not"* is retired with
the shape that made it possible.

**Blocked on T24.1.** Attempting it before is the 2026-09-12 symptom
again, with no `fill_all` available to hide it.

## What this is not

**Not a return of `NodeDesc::kind`.** `docs/results-traversal.md`
removed `kind` and `offset` from the table when dispatch stopped going
through it; nothing here dispatches. `SEEDS` is a *parallel* const
array, consulted once per stylesheet, so the descriptor the hot paths
read stays 48 bytes.

**Not per-instance styling.** `lib.rs` §"the cascade is static" still
holds: a seed is a property of the type, and two instances of a widget
still cannot differ before a rule applies.
