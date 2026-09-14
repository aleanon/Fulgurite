# M25 — appearance on the widget: a look per kind, a palette as data

> **Not started. Written 2026-09-14.** The ask: *"find out if there
> could be any benefit in this framework to have style attached to each
> widget, similar to how iced does it — so the style struct only carries
> what it needs for the given widget and styling becomes a function on
> the widget."*
>
> The answer the investigation reached is **yes for appearance, no for
> layout**, and this plan is shaped by that split. `Style` today is one
> struct resolved for every node, and six places in the tree have
> hard-coded or derived a colour *because* of that (§0). iced's
> per-widget style carries appearance only — width, height and padding
> are set on the widget, not in its style — and that is the half worth
> taking.
>
> Two objections raised while investigating turned out to be wrong, and
> are recorded in §2 so they are not raised again: that a style function
> cannot be hot-reloaded, and that iced's theming is code rather than
> data.

*Each task is one commit; each ends with `just test` green. Task ids go
in commit subjects. Every task names the injection its tests must go red
under.*

---

## 0. What one shared struct has cost

`Style` is 72 B and `NodeState` 76 B, both `const`-asserted, and every
field is paid on every node of every tree
(`docs/results-embedded.md`). That budget is right to defend — and
defending it is why appearance that only one widget needs has nowhere to
live. Each row below says so in its own doc comment:

| where | what it does instead of styling | the reason it gives |
|---|---|---|
| `toggle.rs` `MARK` | tick, dot and knob are always white | "a third colour on every node in the tree, for one part of one widget" |
| `input.rs` `selection_tint` | selection is the text colour at alpha 64 | "`NodeState` is 76 B and asserted at that size" |
| `input.rs` `CARET` | caret colour is a constant | the same budget |
| `scroll.rs` `paint` | track and thumb are both derived from `fg` | the same budget, via `selection_tint` |
| `chart.rs` `WEDGES` | pie series colours are hard-coded | — no stylesheet can reach them |
| `style.rs` `pressed_dim` / `hover_dim` | a pressed or hovered control may only darken its background | "doubling its size to express the rare case would cost every node" |

And one thing that does not exist at all: **a disabled look.**
`Cursor::NotAllowed` is in the vocabulary; nothing can make a control
*look* unavailable.

### How the cascade is actually used

Counted across `examples/` on 2026-09-14:

| example | rules | `Id` | `Kind` | `Class` |
|---|---:|---:|---:|---:|
| `showcase` | 30 | 29 | 2 | 0 |
| `glass` | 20 | 17 | 3 | 0 |
| `glasswall` | 26 | 15 | 11 | 0 |
| `counter-window` (`app.rs`) | 11 | 10 | 1 | 0 |
| `counter-window` (`form.rs`) | 8 | 2 | 6 | 0 |
| `counter` | 2 | 0 | 2 | 0 |

**No example uses a class.** The dominant use of the stylesheet is
per-instance styling by name — which is what a style function on the
instance is, with the compiler checking the name.

Two silent no-ops the current shape allows, both of the kind the
repository keeps finding:

- `Rule::new(Selector::Kind("checkbox"), Patch::new().font_weight(700.0))`
  compiles and changes nothing. A field that has no meaning for a kind
  is still writable for it.
- `Selector::Kind("Label")` matches nothing (`book/src/styling.md`,
  "kind names are not spelled alike").

## 1. What iced does, and what this takes from it

Verified against iced 0.14:

- Each widget module has its own `Style` struct holding **appearance
  only**, and its own `Status` enum (`Active`, `Hovered`, `Pressed`,
  `Disabled` for a button; a scrollable's variants carry per-rail
  flags).
- A widget takes `.style(impl Fn(&Theme, Status) -> Style)`.
- `Theme` is built from a `Palette` — background, primary, secondary,
  success, warning, danger, and an `is_dark` flag — which is **data**,
  changeable at runtime.
- Layout is not in the style: `button(..).padding(15)`.

| iced | this plan | why the difference |
|---|---|---|
| per-widget appearance struct | **taken** — one look struct per kind | §0 |
| `Status` per widget | **taken** — one shared `Status`, widgets read what they need | four states cover every widget in the tree today; a per-widget enum is added when one needs a fifth |
| `Palette` as data | **taken** | §2.2 |
| `Box<dyn Fn>` style closure | **not taken** — a plain `fn` pointer | `no_std` without `alloc` has no `Box` (M13/T13.4), and every handler here is already a `fn` pointer (`docs/events-decision.md`) |
| widgets generic over a `Theme` with a `Catalog` trait | **not taken** — one concrete palette type | a type parameter on every widget is a second axis of monomorphization, and `docs/results-monomorphization.md` says code size scales per *type* |
| layout on the widget | **not taken** — layout stays in the cascade | §2.1 |

## 2. The decisions

| | decided |
|---|---|
| **What leaves `Style`** | Appearance: `color`, `background`, `radius`, `shadow`, `pressed_dim`, `hover_dim`. |
| **What stays** | Layout and cross-cutting properties: `width`, `height`, `padding`, `spacing`, `align`, `visible`, `cursor`, `focus_ring` — and `font_size`, `font_weight`, which are inputs to text measurement. |
| **Where a look is chosen** | A `look: fn(&Palette, Status) -> …Look` field on each leaf widget, defaulting to the kind's own function. |
| **Where it is resolved** | In `layout`, which already has `&mut self` and already resolves pressed/hovered (`record_painted`). Stored on the widget; `paint` reads it. `paint`'s signature does not change. |
| **How theming works** | One `Palette` held by the host, reached through `LayoutCx`. `Host::set_palette` swaps it. |
| **The name** | `Style` keeps its name through this milestone. A rename to say "this is layout now" touches every stylesheet in the tree and the book, and is a separate decision after T25.6. |

### 2.1 Why layout stays in the cascade

Every node has a width. Rules like "every focusable control gets a
2 px ring" or "every row fills" are what a cascade is for, and
`cascade_subtree`'s soundness argument (`docs/results-cascade-cost.md`)
does not care which fields a `Patch` has. The half of `Style` that is
genuinely per-node stays per-node; only the half that is per-*kind*
moves.

### 2.2 The two objections that were wrong

**"A style function is code, so it cannot be hot-reloaded."** It can,
and the reload design already handles exactly this shape.
`reload.rs` on `Button`: *"`on_click` is code: `fresh()` supplies the
new dylib's pointer and `restore` never touches it."* The old library is
never `dlclose`d, so the old pointer cannot dangle
(`docs/step4-hot-reload.md`, "Dev-mode leaks are policy"). And a `fn`
field marked `#[reload(keep)]` **fails to compile**, because `fn` types
are not `Serialize` — so a look cannot be migrated by mistake. A `look`
field is one more handler as far as reload is concerned. T25.2 proves it
end to end rather than trusting this paragraph.

**"iced's theming is code, not data."** Its `Palette` is data. The
design here matches it: the palette is data a host, an agent or a
config file can swap; the mapping from palette to a widget's look is
code, and code hot-reloads.

**What genuinely differs, stated plainly.** The agent endpoint is a
separate process: it can send a palette, and it cannot send a function.
Today it can restyle one node's colour with a sheet rule; after T25.6 it
restyles colours by palette only. If tooling needs per-node colour
overrides, a registry of named looks the agent selects by string is the
shape — deferred until something asks for it.

### 2.3 Derived containers — **OPEN, settled by T25.3**

A derive cannot add a field to the user's struct, so a `Card` cannot
carry a `look` field the way a `Button` can. Containers paint a
background today through `Style`, and `examples/counter-window`'s card
is styled exactly that way. Three shapes, in order of preference:

1. **A type-level look:** `#[widget(column, look = card)]` emits a
   `const` function on the type. No per-instance choice — the same
   property `KIND` and `SEEDS` already have (`docs/plan-select.md`
   T24.0) — and a new attribute, so `lib.rs`'s table and
   `book/src/attributes.md` move together (`scripts/check_attr_table.sh`).
2. **A wrapper widget**, `Surface<W>`, shaped like `Glass<W>`: a
   container that wants a fill says so in the tree.
3. **Container appearance stays in `Style`.** Cheapest, and it keeps
   `background`/`radius`/`shadow` per node, which gives back most of the
   bytes.

T25.3 counts how many derived containers in `examples/` and `fixtures/`
paint a background, and decides.

### 2.4 Byte arithmetic — an estimate, not a measurement

From field sizes alone: removing appearance takes `Style` from 72 to
about 48 B, and `NodeState` from 76 to about 60 B once `fg`, `bg`,
`radius` and `shadow` no longer need stamping. **That arithmetic is not
a result** — alignment, the `Length` discriminants and whatever §2.3
decides all move it, and the look structs move bytes onto widget
instances. T25.0 measures it, and the number is stated in
`docs/results-appearance.md` and cited from here.

---

## T25.0 — measure before building

No API. Three probes, applied and reverted, the way
`docs/results-embedded.md` measured a larger `Style` on 2026-09-10.

**Thresholds, recorded before measuring:**

| quantity | threshold | kill if |
|---|---|---|
| RAM per node, a tree of labels and containers only | must not grow | it grows |
| embedded `.text` (`just firmware-size`) | ≤ +1 KB for the pilot widget | it exceeds that |
| frame time at 10 000 nodes | distributions overlap the baseline | they separate |
| full cascade, 901 nodes (`benches --bin cascade`) | reported, not gated | — a smaller `Patch` should make it faster; if it is slower, find out why before T25.1 |

**The precondition.** Resolving a look in `layout` is only correct if a
change of `hovered` or `pressed` reaches that node's `layout` before the
frame that shows it. It must already — `hover_dim` works, and it is
resolved in `record_painted`, which only `layout` calls — but *must
already* is a reading, and reading is what wrote the six vacuous tests
in `AGENT.md`. T25.0 finds the path and pins it with a test: hover a
button, assert its painted background changed with no sheet swap and no
explicit relayout call.

**Injection:** break the path found (stop marking the node when
`hovered` flips). The test must fire by name, and print the painted
colour beside the resting one.

Creates `docs/results-appearance.md`.

## T25.1 — `Status` and `Palette` in the core

`style::Status { Active, Hovered, Pressed, Disabled }`, derived from
`NodeState` — `Disabled` is a variant nothing produces yet (§4).
`Palette` as a `Copy` struct of named colours, a light default and a
dark one. `LayoutCx` carries `&Palette`; `Host::set_palette` swaps it
and damages the whole frame. **No widget reads either.**

**Gates:**

- A test leaf reads `cx.palette` in `layout`; the host's palette
  reaches it. *Injection:* the host passes the default palette instead
  of its own — fires, printing both.
- `set_palette` produces a frame whose pixels differ, *and* a leaf's
  `layout` ran (assert the action landed, not only the result).
  *Injection:* `set_palette` stores the palette without damaging —
  fires.
- The other direction: `set_palette` with an equal palette does not
  force a full redraw.

**The agent surface** gains `set_palette` in the same commit, because a
host capability the agent cannot reach is the Config-gap shape
`docs/plan-widgets-gallery.md` §3d names. `scripts/agent_drive.sh`
drives it.

## T25.2 — the pilot: `Toggle`

`Toggle` gains `look: fn(&Palette, Status) -> ToggleLook` with
`ToggleLook { body, on_body, mark, radius }`. `MARK` is deleted. The
widget stops reading `color`/`background`/`radius` from `Style` —
which still has them; nothing else migrates yet.

`Toggle` first because it has the most workarounds: `MARK`, the
inverted-colour history in its own doc comment, and a switch knob whose
colour has been wrong twice (`tests/switch_knob.rs`).

**Gates:**

- **What was impossible now renders:** a look with a coloured tick. A
  pixel under the tick is that colour. *Injection:* paint reads the old
  `MARK` constant — fires.
- **A pressed look differs by more than a shade:** a look whose pressed
  `body` is a different hue. *Injection:* `Status` is computed as
  `Active` always — fires, naming the state.
- **The silent no-op becomes loud.** A `Kind("checkbox")` rule setting
  `background` now changes nothing, which is exactly the §0 failure
  shape. The cascade emits a lint for an appearance field matched onto
  a migrated kind. *Injection:* delete the emission — the lint test
  fires. *Other direction:* a `width` rule on the same kind emits
  nothing.
- **Hot reload, end to end.** `fixtures/hot-app` builds a toggle with
  one look, the host swaps to a build with another, and after the swap
  the tick is the new colour *and* the toggle's `on` survived.
  *Injection:* the fixture's second build keeps the first look function
  — the colour assertion fires while the state assertion stays green,
  which is the test telling the two halves apart.

**Measure** the pilot against T25.0's thresholds and append to
`docs/results-appearance.md`. **If `.text` or frame time trips a
threshold, stop here** and record the decision; the milestone is a
single widget's worth of evidence that it should not continue.

## T25.3 — decide containers (§2.3)

Count derived containers in `examples/` and `fixtures/` that set
`background`, `radius` or `shadow`. Pick one of §2.3's three shapes,
build it on one example's card, flip §2.3 to **DECIDED** with the count.

**Gate** for option 1 or 2: the card paints its look's background under
a palette swap. *Injection:* the derive (or wrapper) ignores the look —
fires. For option 3 there is no new mechanism, and the decision record
is the deliverable.

## T25.4 — the core's leaves

`Button`, `Label`, `TextInput`, `TextArea`, `Slider`, `ScrollView`,
`Select`, `Image` (its `tint`). Deletes `CARET`; `selection_tint` and
the scrollbar's derived track become look fields. One commit per widget
or per closely related pair, each carrying its own doc-comment updates —
**`AGENT.md`'s measured split applies directly here**: milestone-sized
commits are the ones that skip docs.

`Glass<W>` is not migrated: `Material` is already per-widget appearance,
which is §1's shape arrived at independently
(`docs/plan-glass.md` §1.1). Whether its tint should read the palette is
M22's question.

**Gate per widget:** its default look under the dark palette passes
`scripts/ui_check.py` contrast. *Injection:* one widget's look ignores
the palette and returns its light colours — the contrast check fires on
that widget by name.

## T25.5 — the widgets crate

`Rule`, `Badge`, `Chip`, `Avatar`, `Icon`, the three charts (`WEDGES`
becomes a series list on the palette), `ProgressBar`, `Spinner`,
`Skeleton`. Same gate as T25.4.

## T25.6 — remove appearance from the per-node structs

Delete the §2 fields from `Style`, `Patch` and `NodeState`, and their
stamping from `record_painted`. Update, **in the same commit**:

- the size assertions in `tests/cursor.rs`, `tests/focus.rs` and
  `fixtures/embedded`, to the measured sizes;
- `docs/results-embedded.md`'s per-node table;
- `docs/layout-and-storage-decisions.md` §3 — **partly superseded**:
  its argument was against *every* widget carrying *every* field, and
  it still holds for layout;
- the agent's `style` method, which serializes `background`,
  `font_weight` and the painted colour today, and gains the resolved
  look;
- `book/src/styling.md`, whose "What a `Style` costs" section is the
  argument this milestone answers;
- `docs/00-handoff-static-tree.md` where it names `Style` fields.

**Gate:** the new sizes are `const`-asserted. *Injection:* re-add one
`f32` of appearance to `Style` — the assertion fires, printing both
sizes.

## T25.7 — use the thing

`AGENT.md`, 2026-09-14: four defects in M24 were found by *assembling*
passes that were each tested directly. This milestone is the same risk
— a look resolved in `layout`, a palette reaching `LayoutCx`, damage on
a swap, a reload of a function pointer — so it is not done until it is
used end to end.

`examples/showcase` gains a light/dark switch that calls `set_palette`.
Then: `just agent-drive` flips the palette and screenshots every demo;
`just gpu-parity` holds under both palettes; `scripts/ui_check.py`
contrast and target checks pass under both. **Every widget in the
catalogue must visibly change** between the two screenshots — a widget
that looks identical under both palettes did not migrate.

---

## 3. What this is not

**Not the end of the cascade.** Layout, cursor, focus ring and
visibility stay rules. `cascade_subtree`'s soundness is unchanged.

**Not per-instance layout.** A `Repeat`'s rows still share their
template's style row. Their *looks*, resolved per instance in `layout`,
may differ per row — which would be the first per-row styling the tree
has had (`docs/layout-and-storage-decisions.md` §3's known limit). That
is a consequence to test in T25.4, not a promise this plan makes.

**Not a stylesheet language.** Nothing here parses.

**Not iced's `Catalog`.** One palette type, no theme parameter on
widgets (§1).

## 4. Deferred, with conditions

- **Producing `Disabled`.** A disabled flag needs a byte of per-node
  state, and `NodeState` has none spare until T25.6 frees some. After
  T25.6, if a widget needs it.
- **Named looks the agent can select.** When a tool needs per-node
  colour overrides that a palette cannot express (§2.2).
- **Renaming `Style`.** After T25.6, as its own decision.
