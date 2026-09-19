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

## T24.0 — `Widget::SEEDS`: the type-level style inputs — **DONE 2026-09-13**

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

## T24.1 — the cascade reads `SEEDS` — **DONE 2026-09-13**

`cascade` takes `&[Seed]` instead of two `&dyn Fn` closures;
`cascade_deep`, `cascade_subtree` and `cascade_arenas` pass `W::SEEDS`.
The live tree stops being consulted for style at all, which is also
what makes a scoped re-cascade cheaper: no virtual call per node.

**Gate:** the probe above, as a committed test — a `Kind` rule reaches
an inactive enum variant, and a `None` `Option`'s subtree resolves its
own defaults.

**Shown to fail.** Injection H puts `cascade_deep` — the function
`Host::set_stylesheet` calls, which is why the gate goes through it
rather than through `cascade` — back on the live tree. Three of the four
tests go red (`a Kind("checkbox") rule did not reach the hidden
CheckBox: it resolved Shrink`, `hidden CheckBox resolved width Shrink;
its own default is Fixed(24.0)`), and the fourth,
`the_showing_variant_is_unaffected`, stays green — correctly: the
showing variant was never broken, and that assertion is the other
direction, that the fix does not hand every node the leaf's defaults.

**And it is 2.4× faster**, which was not the point and is worth stating
plainly for that reason. The virtual call per node was most of what a
stylesheet swap cost: 37.45 µs → 15.42 µs on the 901-node fixture, one
`git stash` apart on the same machine. `docs/results-cascade-cost.md`
owns the figures and records that its 2026-09-04 conclusion — a swap is
the expensive runtime operation, scope it — is unchanged.

**One case still reads an instance**, deliberately: an arena under a
`Dyn` island. Type erasure is what an island is *for*, so the boxed
content's kinds cannot reach a const array. `ArenaMut` gains
`seeds: Option<&'static [Seed]>` — `Some` for `Repeat` and `List`, whose
content types are known, `None` for `Dyn` — and the `None` arm calls
`cascade_from_instance`. That fallback is sound where the main table's
was not: an arena template is a live instance by construction, whereas
the node the main cascade needed might be hidden.

### What it removed from the showcase, and how that was checked

`Demos::fill_all()` — thirty-four lines that made every demo `Some`
before the cascade and let the first pump empty the stage again. With
`Option` seeding transparently it is dead, and deleting it leaves the
showcase's nine tests green.

**Green is not the evidence; red is.** Deleted *and* with `cascade_deep`
put back on the live tree, `every_entry_paints_a_demo` fires:

```
Badge drew only text on the stage — it looks like a label
```

which is the sentence the defect was reported in, from a test that
already existed and had never seen it — because `fill_all` was hiding
it.

## T24.2 — `Select<S>` over `Popup` — **DONE 2026-09-14**

The dropdown as a widget: a trigger, a `Popup` panel anchored `Below`,
and rows from a `Repeat`. Four nodes, whatever the option count.

This is **T11.5**, deferred 2026-09-04 as "a layout milestone with three
widgets on the end of it". Two of its three prerequisites landed in M15
and the third in T11.6, so what was left was widget code — and the
widget itself took an afternoon. **Seven framework defects underneath it
took the rest of the day**, which is the part worth recording: a
capability nothing had exercised end to end was not one capability but
seven, each individually invisible — and one of them a release-build
panic that every gate in the repository was green for.

### What it owns

`Select` owns **opening**, in `Node::on_event` — the hook a widget has
for its own state, the one `Slider` uses. It cannot be an `on_click`,
because that is `fn(&mut S, ..)` and from `&mut S` a widget is reachable
only as `&mut dyn Node`.

It does **not** own closing, and that is not an omission. A press
outside every overlay is dismissed and consumed by `dispatch_overlaid`
before any widget sees it, so pressing the trigger while the menu is
open closes it and does *not* reopen it — the press never arrives. The
classic dropdown bug, avoided by not writing code, because M15 made
dismissal the pass's job. `on_event` writes `open = true` rather than
`!open` so the bug cannot return even if the press did.

### The parts are addressable

A composite is more than one node and the derive names only a *field*,
so `select::PART_TRIGGER`, `PART_POPUP` and `PART_MENU` are stamped on
the internal nodes. Without them nothing can reach inside a `Select` —
not a stylesheet, not the agent's `#id` selectors, not
`scripts/agent_drive.sh`. Namespaced, because a bare `trigger` would
collide with an application's own `#[widget_id("trigger")]`.

### T24.4 — what a part id still cannot reach: a *row* — **DONE 2026-09-19**

Recorded 2026-09-14, unbuilt. `PART_TRIGGER`/`PART_POPUP`/`PART_MENU`
reach the `Select`'s own three nodes. They do not reach **row *k***,
because an arena row is a template instantiated per index and carries no
`#[widget_id]` of its own. The only selector that reaches one is `Kind`,
and that works in the showcase solely because every *other* `Button` in
its sheet is addressed by an id, and an id beats a kind. Add one unnamed
button and the menu's rows restyle with it.

Two consequences, and the second is the load-bearing one:

- a stylesheet cannot say "the selected row", which is what
  `docs/plan-appearance.md` calls *not per-row status*;
- **nothing can click row *k* by name.** `scripts/agent_drive.sh`
  computes `box[1] + ROW_H * 8 + ROW_H / 2` instead, which is arithmetic
  over a resolved box and one renamed row away from clicking the wrong
  widget and passing — the "setup silently did not happen" shape this
  repository has been caught by twice.

Promoted to a prerequisite by `docs/plan-widget-parity.md` T27.0, where
six of the thirty-three widgets are repeaters whose rows must be
addressable to be tested at all.

**Built 2026-09-19**, as `%k` plus a fix to the descendant step.

`#menu > %3` is the fourth row of `#menu`; `%3` alone is every
repeater's fourth row at once, which `ui.resolve` reports as ambiguous
rather than guessing. The row index rides in the id
`lumen_static::observe::instance_id` already built — bits 12..=43, with
the owner in 44..=62 — so nothing new is stored and the decoder sits
next to the only two things that read it, where it cannot drift from the
encoder the way four copies of *that* did (F4.4).

**The descendant step could not reach a row at all**, which is the half
that was silently costing something. It filtered on the table's parent
chain and an arena row has no table index, so `#grid > ~file-000.rs`
matched nothing however visible the row was — and `M27/T27.10`'s drive
worked around it with a bare `~text`, safe only while the text happens
to be unique in the window. That is a property of a fixture, not of a
widget. `entry_is_inside` answers for both kinds of entry now.

Five injections (DT–DW). **One left every test green**: "every row is
inside every owner" cannot be wrong in a tree with *one* repeater, and
the fixture had one. A second repeater is the smallest thing that makes
"inside" mean anything, and with `#left > %0` and `#right > %0` naming
different rows it reddens.

**What is still open**, and it is the other half of this section: a
stylesheet still cannot say *the selected row*. Rows share one template
style row by construction — that is what an arena is — so per-row status
needs a style row per instance, which is `docs/plan-appearance.md`'s
question and not a selector's. The addressing half is what
`docs/plan-widget-parity.md` T27.0 promoted this for, and it is done.

### The five things underneath

Each was found by the widget failing to work — four of them by *looking
at a rendered frame*, one by running the window. **Five of the seven
left the whole suite green when injected back**, which makes them
findings about the tests; every one was answered by strengthening the
test rather than weakening the injection.

| # | defect | how it surfaced | gate now |
|---|---|---|---|
| 1 | `dispatch_overlaid` threw away its overlay hit and fell through to `dispatch`, which hit-tests plainly — so **no handler inside an open popup could ever run** | the menu opened and clicking a row did nothing | `tests/overlay.rs::overlay_activation`, `tests/select.rs` |
| 2 | `Host` painted with **no overlay sink and no `dispatch_overlaid`** — so `Popup` had never worked from the shell every example uses | the menu painted *inline*, under the backgrounds of its later siblings | the showcase's own tests, and `just agent-drive` |
| 3 | `Repeat` laid its rows out against the limits it was **given**, not its resolved box | 18 rows 932 px wide in a 300 px panel | `tests/repeat_box.rs` |
| 4 | `Repeat` painted no background and never called `record_painted`, so `style.background` reached no `NodeState` | the panel was transparent | `tests/repeat_box.rs` |
| 5 | the enum derive emitted **no `defaults`**, inheriting `Node`'s `Shrink`/`Shrink` where every other container is `FILL`/`Shrink` | the stage stopped being centred | `tests/hatches.rs` |
| 6 | the journal and the auditor painted and probed **stand-in nodes** — `node(i)` answers with the enclosing container for an index inside a branch that is not laid out, and using it *at `i`* runs off the end of `states` | a **release-build panic** the moment the menu opened; `just check` was green and 134 test binaries were green | `lumen-static-agent/tests/hidden_subtree.rs` |
| 7 | `pass::default_styles` seeded from the live tree, so a hidden subtree got the container's defaults | an inactive variant measured 400 px instead of 40 | `tests/hatches.rs` |

Six and seven are M24's own shape once more, in the two places T24.1
did not reach. Seven is literally the cascade defect in a second
function. Six is subtler and worth stating on its own:

> `Node::node(i)` **cannot** answer for an index inside a branch that is
> not laid out, and answers with the enclosing container instead so the
> agent can still walk and report those nodes. Anything that then uses
> that value *at `i`* — paints it, probes it, lays it out — is using a
> node that does not know where it is.

`crate::live_nodes` is the predicate, and what it is *not* is the part
worth reading: **not "has area"**. That was tried first and it turned
off `W1001`, the invisible-content lint, because a leaf with content and
no room is zeroed exactly like a hidden one. The test is object
identity — `node(i)` is a stand-in iff it is the same object as
`node(parent(i))`, address *and* kind. The kind is not defensive: a
zero-sized first field shares its parent's address, and dropping it from
the comparison fails `changes.rs`.

Number 2 is the **fourth** instance of the `Config`-gap shape §3d of
`plan-widgets-gallery.md` named: a capability shipped into the framework
with nothing in the front door wired to it. M15 built the overlay phase,
`Painter::defer`, `paint_overlaid`, `dispatch_overlaid` and dismissal —
and `Host`, which every example and every window goes through, called
none of them. `examples/glass` and `tests/overlay.rs` drove the passes
directly, which is why it looked covered.

Number 5 is the shape this milestone started from, one level up: a fact
that is a property of the *kind* of container, left to a default that
happened to be wrong, and invisible until something was placed where the
difference showed.

## T24.3 — the showcase's stage becomes an enum — **DONE 2026-09-14**

`Demos` → `#[derive(StaticTree)] enum Demo { Label(DLabel), … }`.
`shown()` and `clear()` cease to exist: one-of-N by construction. The
comment in `stage.rs` that says a field missing from `clear()` is *"the
one bug this shape makes possible and a `Dyn` did not"* is retired with
the shape that made it possible.

**Was blocked on T24.1**, which landed 2026-09-13. Attempting it before
would have been the 2026-09-12 symptom again with no `fill_all`
available to hide it — the workaround an enum cannot have.

## What this is not

**Not a return of `NodeDesc::kind`.** `docs/results-traversal.md`
removed `kind` and `offset` from the table when dispatch stopped going
through it; nothing here dispatches. `SEEDS` is a *parallel* const
array, consulted once per stylesheet, so the descriptor the hot paths
read stays 48 bytes.

**Not per-instance styling.** `lib.rs` §"the cascade is static" still
holds: a seed is a property of the type, and two instances of a widget
still cannot differ before a rule applies.
