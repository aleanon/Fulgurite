# M25 — appearance on the widget, and the invalidation gap under it

> **Not started. Written 2026-09-14; revised the same day after a
> six-lens review** (facts, injections, design, cost, migration,
> alternatives), each lens followed by an adversarial verifier that
> re-opened the code: 47 findings, 43 confirmed, 4 plausible, none
> refuted. §5 records what the review found and what it changed. Read
> it before §2 — the first draft's central premise was false, and the
> plan keeps the correction rather than quietly replacing itself.
>
> **The first task is a shipped defect, not a feature.** Hover and press
> shading go stale under `Boundaries` invalidation today (T25.0). The
> review found it by checking the premise this milestone was built on —
> "hover already reaches layout" — and it does not, except under the
> default `Everything`. Everything after T25.0 depends on it.
>
> The ask that started this: *"find out if there could be any benefit
> in this framework to have style attached to each widget, similar to
> how iced does it — so the style struct only carries what it needs for
> the given widget and styling becomes a function on the widget."* The
> answer is **yes for appearance, no for layout**, with a stop point at
> the pilot where a much smaller change (§2.6) is the fallback.

*Each task is one commit unless it says otherwise; each ends with
`just test` green. Task ids go in commit subjects. Every gate names the
injection it must go red under, and what it prints beside its bound.*

---

## 0. What one shared struct has cost

`Style` is 72 B and `NodeState` 76 B, both `const`-asserted, and every
field is paid on every node of every tree
(`docs/results-embedded.md`). Defending that budget is right, and it is
why appearance that one widget needs has been hard-coded or derived:

| where | what it does instead of styling | what its doc comment says |
|---|---|---|
| `toggle.rs` `MARK` | tick, dot and knob are always white | "a third colour on every node in the tree, for one part of one widget" |
| `input.rs` `selection_tint` | selection is the text colour at alpha 64 | "`NodeState` is 76 B and asserted at that size" |
| `input.rs` `CARET` | caret colour is a constant | nothing — the budget reason is `selection_tint`'s |
| `scroll.rs` `paint` | track and thumb both derive from `fg`, via `selection_tint` | nothing about budget; it explains clip placement |
| `chart.rs` `WEDGES` | pie series colours are hard-coded | nothing |
| `style.rs` `pressed_dim` / `hover_dim` | a pressed or hovered control may only darken its background | "doubling its size to express the rare case would cost every node" |

**Not every row needs this milestone.** `Image` already carries
`pub tint` and `Glass<W>` a `Material`: a per-widget colour field is a
shape the tree already uses, and MARK's own objection is to a `Style`
field, not a widget field. §2.6 compares that smaller path honestly.
What only this milestone gives is in §2.6 too.

### How the stylesheets are used

Counted across `examples/` on 2026-09-14 (selector *occurrences*, which
can exceed constructors):

| example | rules | `Id` | `Kind` | `Class` |
|---|---:|---:|---:|---:|
| `showcase` | 26 | 24 | 3 | 0 |
| `glass` | 20 | 17 | 3 | 0 |
| `glasswall` | 26 | 15 | 11 | 0 |
| `counter-window` (`app.rs`) | 11 | 10 | 1 | 0 |
| `counter-window` (`form.rs`) | 8 | 2 | 6 | 0 |
| `counter` | 2 | 0 | 2 | 0 |

**What this shows is that the examples mostly name instances — not that
appearance should leave the cascade.** Many `Id` rules are layout-only
and stay rules; many mix a colour with `font_size` on the same label,
which this plan splits across two mechanisms. And the case the design
must still serve is the one real theming pattern here: `form.rs` themes
every checkbox, switch, slider, button and label with **one `Kind` rule
each**, and in the showcase a `Kind` rule is the only selector that
reaches a `Repeat`'s rows (§2.4).

Two silent no-ops exist, and **neither is fixed by moving appearance**
— `font_weight` stays in `Style`, and nothing checks kind spelling. They
get their own task, T25.2, which is independent of looks:

- `Rule::new(Selector::Kind("checkbox"), Patch::new().font_weight(700.0))`
  compiles and changes nothing.
- `Selector::Kind("Label")` matches nothing (`book/src/styling.md`).

## 1. What iced does, and what this takes from it

Verified against iced 0.14:

- Each widget module has its own `Style` struct holding appearance only,
  and its own `Status` enum (`Active`, `Hovered`, `Pressed`, `Disabled`
  for a button; a scrollable's variants carry per-rail flags).
- A widget takes `.style(impl Fn(&Theme, Status) -> Style)`.
- `Theme` is built from a `Palette` — background, text, primary,
  success, warning, danger — which is **data**, changeable at runtime.
  An extended palette (adding secondary and a dark flag) is generated
  from it.
- Layout is not in the style: `button(..).padding(15)`.

| iced | this plan | why |
|---|---|---|
| per-widget appearance struct | **taken** | §0 |
| `Status` per widget | **taken** — one shared `Status`; focus stays a paint-time bit (§2.1) | the four states are what hover, press and a future disabled flag need |
| `Palette` as data | **taken**, with a text colour | §2.2 |
| `Box<dyn Fn>` style closure | **not taken** — a plain `fn` pointer | `no_std` without `alloc` has no `Box` (M13/T13.4); handlers are already `fn` pointers |
| widgets generic over a `Theme` with a `Catalog` trait | **not taken** — one concrete palette type | a type parameter on every widget is a second monomorphization axis (`docs/results-monomorphization.md`) |
| layout on the widget | **not taken** — layout stays cascaded | §2 |

## 2. The decisions

| | decided |
|---|---|
| **What leaves `Style`** | Per-widget appearance: `color`, and `pressed_dim` / `hover_dim`. Whether box fills (`background`, `radius`, `shadow`) leave too is **OPEN** — §2.3, settled by T25.4. |
| **What stays** | `width`, `height`, `padding`, `spacing`, `align`, `visible`, `cursor`, `focus_ring`, `font_size`, `font_weight` (the last two feed text measurement). |
| **Where a look is chosen** | A `look` field on each leaf widget, `fn(&Palette, Status) -> …`, defaulting to the kind's own function — or to the kind-wide choice of §2.4. |
| **Where it is resolved** | In `layout`, stored on the widget; `paint` reads it and its signature does not change. **Sound only after T25.0** — see §2.1. |
| **How theming works** | One `Palette` held by the host, reached through `LayoutCx`. `Host::set_palette` stores it and **invalidates** — `mark_all`, as `set_stylesheet` does — when it differs. Damage then follows from retention's command hashes; it is not something the setter produces. |
| **The name** | `Style` keeps its name through this milestone. |

### 2.1 Why resolve in layout, and what that depends on

A look that depends on `Status` must be re-resolved when a node's
hovered or pressed bit flips. **Today nothing reports that flip** (T25.0).
Under the default `Everything` every node is laid out every frame, so it
does not matter; under `Boundaries` a clean subtree returns before
`record_painted` runs, and the shade goes stale. Resolving looks in
layout inherits exactly that, so T25.0 comes first.

The alternative the review raised — resolve the status-dependent part in
`paint`, which already has `st[i].hovered` — needs either the palette in
`paint` (the `Node` signature change `docs/text-and-scrolling-decisions.md`
A2 rejected) or every status variant of the look stored on the widget,
which multiplies the per-instance bytes of §2.5 by four. Neither is
cheaper than fixing the report, which is needed anyway.

**Focus is not a `Status`.** `TextInput` and `TextArea` already branch
on `st.focused` in `paint`, and the host rewrites `focus_ring` *after*
layout for pointer focus, so a look resolved in layout cannot see
focus-visible. Focus appearance stays a paint-time read; where the ring
takes its colour and radius from once `fg`/`radius` leave `NodeState`
is part of T25.4.

### 2.2 The two objections that were wrong, and what they missed

**"A style function cannot be hot-reloaded."** It can: `reload.rs` on
`Button` — *"`on_click` is code: `fresh()` supplies the new dylib's
pointer and `restore` never touches it"*; the old library is never
`dlclose`d; and a `fn` field marked `#[reload(keep)]` fails to compile.
**What that argument missed:** only Label, Button, Fixed, Spacer,
Repeat, `Vec` and `Option` implement `Reloadable`, so a widget gaining a
`look` still needs its own hand-written impl before a reloadable tree
can hold it. T25.5 writes Toggle's.

**"iced's theming is code, not data."** Its palette is data. Here too:
the palette is data a host, agent or config swaps; the mapping from
palette to look is code, and code hot-reloads.

**What genuinely differs.** The agent endpoint can send a palette, not a
function, so per-node colour overrides by sheet rule are lost when
colours leave the cascade. Deferred until a tool needs it (§4).

### 2.3 Box painters — **OPEN, settled by T25.4**

Every node that paints its own box reads `st.bg`, `st.radius` and
`st.shadow`, and none of them is a leaf with room for a `look`:

| painter | reads | note |
|---|---|---|
| the derive's container `paint` | `bg`, `radius`, `shadow` | `crates/lumen-static-macros/src/lib.rs` |
| the derive's container `hit` | `bg` alpha, `shadow` | **input routing, not only paint**: a container that paints nothing does not claim the hit, which is what stops a transparent stack layer swallowing `examples/glass`'s input |
| `Repeat::paint` | `bg`, `radius`, `shadow` | how `Select`'s menu panel gets its fill (M24/T24.2) |
| `Glass::paint` | `radius`, `shadow` | `tests/glass_widget.rs` pins radius-from-style |
| `ScrollView`, `Fixed`, `Image`, `Chart` | `bg` / `fg` | `Fixed` paints `fg`; `Image` and `Chart` paint a background behind content |
| `ops::focus_ring` | `fg`, `radius` | used by five widgets |

Three shapes, **now ordered by cost**:

1. **Box fills stay per node.** `background`, `radius`, `shadow` remain
   in `Style` and `NodeState`; only per-widget appearance leaves. No new
   mechanism, hit semantics unchanged, and most of §2.5's bytes come
   back. The cheapest, and the default if T25.4 finds no reason against.
2. **A wrapper widget**, shaped like `Glass<W>`, holding a fill look as a
   field. Consistent with "stored on the widget"; the author says so in
   the tree.
3. **A type-level look** via a new container attribute. **Not cheaper
   than 1**, which the first draft claimed: `paint` has no palette, and
   a derived container has no field, so the resolved fill has nowhere to
   live except the `NodeState` bytes shape 1 keeps.

### 2.4 Kind-wide appearance and composites — **OPEN, settled by T25.4**

**Kind-wide.** "Every button in this app has radius 6 and the accent
colour" is one rule today. Under per-instance looks it is a look on every
button — and a `#[derive(StaticTree, Default)]` field has nowhere to put
one, and a `Repeat` row is reachable only by `Kind`. Candidates: an
app-level table of per-kind look functions on the host beside the
palette, consulted when an instance's `look` is unset (iced's `Catalog`
default class, made concrete); or layout-plus-shape tokens (`radius`)
on the palette itself. T25.4 picks one against `form.rs` and the
showcase's menu rows.

**Composites.** `Select` is a trigger `Button`, a `Popup` and a
`Repeat` of `Button` rows — not a leaf. Its look is a struct of part
looks handed to its children in layout, and `Button` and `Select`
migrate in one commit so the showcase's trigger and rows do not change
appearance between them.

**Repeat rows never receive hovered or pressed.** `set_hovered` writes
only the table's `st`; `Repeat::hit` reports the repeater's index with
the row in `inner`. So a menu row's `Status` is always `Active`. That is
true today and this milestone does not change it (§4).

### 2.5 Bytes — probed by the review, owned by T25.1

A review probe using the real component types: `Style` 72 → 48 B and
`NodeState` 76 → 60 B with all six appearance fields removed, on both
x86-64 and thumbv7em. Widgets grow by a `fn` pointer and a stored look:
on thumbv7em `Toggle` 8 → 28, `Button` 64 → 88, `Label` 40 → 48 B.

**Two consequences the first draft missed.** A table tree still shrinks
per node once the struct is counted (a thumb `Label`: −40 + 8). An arena
row does not: a `List` or `Repeat` shares one style row, so a row saves
only `NodeState`'s 16 B and gains the look — roughly neutral for a
label, a net gain for a button. And under §2.3 shape 1 the per-node
saving is smaller, because `background`, `radius` and `shadow` stay.
These are probe figures, not results: T25.1 re-measures, and
`docs/results-appearance.md` owns the numbers.

### 2.6 The smaller path, and why it is the fallback rather than the plan

Three changes, none of which removes anything from `Style`:

- **(a) Per-widget colour fields**, the `Image::tint` shape: a mark
  colour on `Toggle`, caret and selection on `TextInput`, track on
  `ScrollView`, a series slice on `Chart`. Four or five of §0's six rows,
  in one or two commits.
- **(b) Palette-indexed colours in `Patch`**, resolved by the existing
  cascade. Theming with no widget changes; `set_palette` is "store,
  re-cascade, `mark_all`", measured at 15.4 µs for 901 nodes
  (`docs/results-cascade-cost.md`); per-node restyle by rule survives.
- **(c) The sheet lint** — which this plan builds anyway, as T25.2.

**What only the full milestone gives:** a pressed or hovered look that is
a different hue rather than a shade; appearance fields that cannot be
written for a kind that ignores them; and the per-node RAM of §2.5.
**What it costs:** ten tasks, and every example stylesheet's colour
rules rewritten.

The plan builds T25.0–T25.3, which (c) and the invalidation fix need
regardless, then the pilot. **T25.5 is the decision point:** if the
pilot trips T25.1's thresholds, or its status-look gate turns out to be
the only thing it buys, the milestone stops there and (a)+(b) ship
instead, recorded in `docs/results-appearance.md`.

---

## T25.0 — hover and press reach layout under every invalidation strategy — **DONE 2026-09-15**

**A shipped defect**, independent of everything below.

> **Landed.** `InputState` keeps `hovered` and `pressed`; `Host::pump_with`
> compares them with `focus` and `focus_visible` across dispatch and marks
> the subtree of every node whose state changed, old and new.
> `crates/lumen-static-shell/tests/pointer_invalidation.rs` holds the four
> gates below, and the oracle's `drive` gained the four pointer steps.
>
> **Red before the fix**, on `Boundaries<16>`: the oracle diverged at step
> 13 (the hover); the hovered button painted (226,232,240) against an
> expected (210,215,223); the pressed one (210,215,223) against
> (185,190,196); the ring was 0 against a styled 2. The still-frame test
> was green before the fix, correctly — nothing marked anything.
>
> **Five injections, each red by name and restored green:** every mark
> deleted (the oracle at step 13 and three of the four tests); the
> `pressed` comparison alone (the press test only — the oracle's own press
> step also moves focus, which marks the node, so the isolated test is the
> one that sees it); the `focus_visible` comparison (the ring, 0 vs 2);
> marking the hovered node every frame (the still-frame test: the unreported
> note re-laid out, 196×48 against a stale 32×16); `mark_all` in place of
> the scoped mark (the hover test's scoping assertion: 196×48 against 40×16).
>
> **One correction to the gate as first written.** "Tab to a button, then
> Tab back" could not fail: Tab returns `true`, so it reaches `mark_all`
> through `handled` whether or not the fix exists. The gate uses an arrow
> key on a button alone in its group, which makes focus visible **without
> being handled**, and asserts that it was not.

`dispatch` writes `hovered`, `pressed` and `focused` into the state
array and returns `false` for an unrouted pointer move
(`crates/lumen-static/src/pass.rs`). `Host::pump_with` marks the dirty
set only when a handler ran or a binding wrote
(`crates/lumen-static-shell/src/lib.rs`). Under `Boundaries<N>` the
derive returns early for a clean subtree with unchanged limits, so
`record_painted` never re-resolves the shade. Probed by the review on a
button with background (200,100,50), `hover_dim` 0.5, `pressed_dim` 0.9:

```
Everything:     hovered=true handled=false painted=Color(100,50,25)  changed=true
Boundaries<16>: hovered=true handled=false painted=Color(200,100,50) changed=false
Boundaries<16>: pressed=true handled=false painted=Color(200,100,50) changed=false
```

`invalidation_oracle.rs`'s `drive` never hovers, which is why the
oracle has not seen it.

**Which flips are unreported.** A Tab or arrow move returns `true` and
reaches `mark_all` through `handled`. Unreported: pointer hover, press,
release (which clears every pressed bit), the C5 drop of a focus that is
no longer on screen, and `focus_visible` turning off on a pointer press
over a node with no handler — which leaves a zeroed `focus_ring` in a
state array a skipped layout never rewrites.

**The fix.** `InputState` gains the hovered and pressed node indices
beside `focus` — per host, not per node, so no `NodeState` byte. `pump`
reads `(hovered, pressed, focus, focus_visible)` before the event loop
and after it, and for each that changed marks `subtree_range` of the old
index and of the new one; a `focus_visible` change marks the focused
node. No allocation (`crates/lumen-static-shell/tests/alloc_gate.rs`).
The direct callers of `pass::dispatch` (`fixtures/embedded`,
`fixtures/miri-smoke`) keep no dirty set and are unaffected.

**Gates:**

- **The oracle sees it.** `drive` gains four steps: a pointer move onto a
  button, a press, a release, and a move off. Each step asserts its
  setup landed — the intended node's bit is set, and `handled` is
  `false`, so the step proves the unrouted path rather than a handler's
  `mark_all`. **Red before the fix**: record which step and the two
  display-list hashes. *Injection after:* delete the new marks — the
  oracle fires at the hover step, printing the painted `bg` under both
  strategies.
- **The mark is precise.** On a `Boundaries<16>` host, a hover marks a
  set that is not `is_all`, and the painted background equals the dimmed
  resting colour, both printed. *Other direction:* a pointer move that
  stays on the same node leaves `changed()` empty. *Injection:* mark on
  every pointer event regardless of change — the still-frame assertion
  fires.
- **Focus-visible.** Tab to a button, then press elsewhere and Tab back
  on a `Boundaries<16>` host: the ring's width is non-zero exactly when
  keyboard focus is visible. *Injection:* drop the `focus_visible`
  comparison — fires, printing the ring width.

`crates/lumen-static/tests/events.rs`'s shade test calls `layout` by
hand after each event and so could never see this; it stays as the
unit-level check.

**Docs in the same commit:** the `invalidate.rs` module doc and the
pump's comments, which list what marks the set; `InputState`'s rustdoc;
`docs/plan-invalidation.md` T16.7, which describes the handler-only
marking as the conservative path; and any book chapter that enumerates
what marks.

## T25.1 — measure before building

No API. Probes applied and reverted, as `docs/results-embedded.md`
measured a larger `Style` on 2026-09-10. **Each probe first proves its
setup exists.**

| quantity | how | threshold | kill if |
|---|---|---|---|
| embedded `.text` | `just firmware-size`, after adding a `CheckBox`, a `Radio` and a `Switch` to `fixtures/embedded` — each `SHAPE` is its own instantiation — and asserting Toggle symbols are in the linked binary before taking the baseline | pilot ≤ +300 B, against M16's +488 and M18+M19's +148 | it exceeds that |
| RAM, table tree | state + style + widget struct, on thumbv7em; the fixture's `UI` size is computed today and not asserted — assert it, and print all three terms | must not grow | it grows |
| RAM, arena row | `NodeState` + one instance, for `List<Label>` and `List<Button>` on x86-64 | reported, not gated — the desktop has no per-node budget | — |
| frame time, per-node work | `benches --bin crossover` (1k table nodes) and a new bench binary with 1 000 toggles on the table, so the pilot's code is in the measured frame | delta smaller than the measured run-to-run spread, printed beside it | the delta exceeds the spread |
| frame time, arena | `just stages 10000 150 20`, the incremental layout and paint stages — not the rebuild, which is 93% text shaping (`docs/results-competitive.md`) | same | same |
| cascade | `benches --bin cascade`, plus the showcase's sheet as a second fixture, since the bench's two `Kind` rules match almost nothing | reported with no predicted direction | — |

**Re-baseline**: the review measured linked `.text` at 21 278 B, against
the 22 606 `docs/results-embedded.md` last recorded; that file is
corrected in this commit.

**Docs:** creates `docs/results-appearance.md`; adds an **OPEN** entry to
`docs/00-handoff-static-tree.md` for appearance per kind, with §2.3 and
§2.4 as sub-questions, pointing here.

## T25.2 — the sheet lint

Independent of looks, and the fix for both §0 no-ops.

Each kind declares, as a type-level fact spliced into `Seed` beside
`KIND`, the mask of `Style` fields it reads. The agent's audit gains a
`W10xx` lint, reported through `ui.lint` like `W1004`–`W1009`: a `Kind`
selector naming no kind present in the tree, and a `Patch` field that no
matched kind reads. When a kind migrates, its mask loses the appearance
bits — so this is also the "appearance field on a migrated kind" warning
the first draft specified without a mechanism.

**Gates:**

- `Kind("Label")` fires; `Kind("label")` does not.
- `Kind("checkbox")` with `font_weight` fires; with `width` does not.
- **The mask cannot lie.** For every kind in both crates, patching each
  field its mask claims changes that node's layout or its display list,
  printed per field. *Injection:* add a field Toggle does not read to
  its mask — fires, naming kind and field. *Injection:* splice every mask
  as full — the `font_weight` case goes silent and its test fires.
- Every example sheet is lint-clean, or the commit fixes it.

## T25.3 — `Status`, `Palette` and `set_palette`

**Two commits.**

**(a) Core types.** `style::Status { Active, Hovered, Pressed, Disabled }`
from the `NodeState` bits — `Disabled` is produced by nothing yet (§4).
`Palette` as a `Copy` struct of named colours including text, page,
surface, accent, on-accent, muted and focus; a light default and a dark
one. `LayoutCx` carries `&Palette`. No widget reads either.

**(b) The host and the agent.** `Host::set_palette` stores the palette
and calls `mark_all` if it differs. The agent gains a method to set it,
driven from `scripts/agent_drive.sh`.

**Gates, all on `Host<_, Incremental, Boundaries<16>>`** — under
`Everything` every layout runs every frame and none of these could fail:

- A test leaf counts its `layout` calls and records the palette it saw.
  The host's palette is asserted to differ from the default first.
  *Injection:* the host passes the default palette — fires, printing both.
- After `set_palette`, the counter advances and the recorded palette is
  the new one. *Injection:* omit `mark_all` — the counter does not
  advance; fires, printing the stale palette.
- *Other direction:* an equal palette leaves `changed()` empty and the
  counter unchanged.

## T25.4 — decide the box painters, kind-wide looks, and the focus ring

A decision task before any widget migrates, because the pilot's example
rules and the removal task both depend on it. Settles:

1. **§2.3**: count every box painter in `crates/`, `examples/` and
   `fixtures/` and pick a shape. Default: shape 1.
2. **§2.4**: the kind-wide mechanism, chosen against `form.rs` and the
   showcase's `Repeat` rows; and composite looks for `Select`.
3. **The focus ring's** colour (the palette's focus colour) and radius
   source, once `fg` and possibly `radius` leave `NodeState`.

Flips the handoff entry from T25.1 to **DECIDED** with the counts.

**Gates, if shape 2 or 3 is built:** a card paints its fill look's
colour after a palette swap, on `Boundaries<16>` — *injection:* ignore
the look, fires. **And hit routing survives:** a click on a stack layer
that paints nothing still reaches what is under it — *injection:* the
container always claims the hit, fires. Under shape 1 nothing new is
built, and the decision record is the deliverable.

## T25.5 — the pilot: `Toggle` — **the decision point**

`Toggle` gains `look: fn(&Palette, Status) -> ToggleLook` with
`ToggleLook { body, on_body, mark, radius }`; `MARK` is deleted. Toggle's
lint mask loses `color` (and the box fields, if T25.4 moved them).

**In the same commit:**

- `examples/counter-window/src/form.rs`'s `Kind("checkbox")` and
  `Kind("switch")` rules move to T25.4's kind-wide mechanism. `just test`
  renders that form; without this the pilot silently recolours it.
- `Reloadable for Toggle`: saves and restores `on`, never touches `look`
  or the resolved look.
- `ui.getAppliedStyles` reports a toggle's resolved look instead of
  `Style` colours it no longer paints.

**Gates:**

- **A coloured tick.** The test's mark colour is asserted to differ from
  white and from the default look's mark, both printed; the check reads
  the tick command's colour from the display list, as
  `tests/switch_knob.rs` does, not a pixel on a 3 px anti-aliased dot.
  *Injection:* paint reads the old `MARK` — fires.
- **A pressed look that is a different hue**, on `Boundaries<16>` — so
  it rides T25.0's mark. *Injection:* `Status` is always `Active` —
  fires, naming the state.
- **Hot reload.** `fixtures/hot-app` gains a toggle with a v1 look and a
  v2 look behind its `v2` feature, and an extern returning the painted
  mark colour. Before the swap the host asserts v1's colour — so the
  probe is shown to read the look — and after it v2's colour and the
  surviving `on`. *Injection* on the mechanism, not the fixture: `restore`
  writes back a saved resolved look — the colour assertion fires while
  the `on` assertion stays green. Run by `just hot-demo`.

**Measure** against T25.1 and append to `docs/results-appearance.md`.
**Stop here** if a threshold trips, and ship §2.6's (a)+(b) instead.

## T25.6 — the core's leaves

**One commit per widget**, except `Button` and `Select` together (§2.4).
`Label`, `TextInput`, `TextArea`, `Slider`, `ScrollView`, `Fixed`,
`Image` (its `tint` and its background). Deletes `CARET`;
`selection_tint` and the scrollbar track become look fields.

**Each commit also:** rewrites the example rules that target that kind;
updates its lint mask; updates `ui.getAppliedStyles` for it; and, for
the firmware-linked `Button`, `Label` and `TextInput`, records the
cumulative `.text` in `docs/results-appearance.md`.

`Glass<W>` keeps `Material`, which is already this shape; its `radius`
and `shadow` follow T25.4.

**Gate per widget — headless, in `just test`.** Lay the kind out and
paint it under the dark palette; from the display-list commands, assert
the look's fill contrasts with the palette's page colour and its text or
mark with its own fill, printing both ratios beside 4.5:1 and the number
of nodes of that kind measured. *Injection:* the look ignores the palette
and returns its light colours — the page-versus-fill assertion fires by
kind name. (`scripts/ui_check.py contrast` cannot be this gate: it
visits only Button and Label, measures inside a widget's own box, and
skips a node whose text it cannot separate. T25.9 extends it.)

## T25.7 — the widgets crate

**One commit per file:** `trim.rs` (`Divider`, `Badge`, `Chip`,
`Avatar`, `Icon`), `indicator.rs` (`ProgressBar`, `Spinner`, `Skeleton`),
`chart.rs` (`Chart` — BAR, LINE, PIE — whose `WEDGES` becomes a palette
series and whose background follows T25.4). Same gate and same
same-commit list as T25.6.

## T25.8 — remove the moved fields

**Three commits**, because this is the milestone-sized change `AGENT.md`
measured skipping its docs.

**(a) The remaining readers.** Every reader of the moved `NodeState`
fields outside `record_painted` goes to its T25.4 home: the derive's
container paint and hit, `Repeat`, `Glass`, `ops::focus_ring`.
**Precondition for (b):** a grep for those field reads returns only the
stamping.

**(b) Tests and examples off the setters.** The tests that read or write
them: `crates/lumen-static/tests/{events,repeat_box,cascade_hidden,toggles,glass_widget}.rs`,
`crates/lumen-static-shell/tests/{damage_oracle,invalidation_oracle}.rs`,
`crates/lumen-static-agent/tests/explain.rs`. `cascade_hidden.rs` is
T24.1's gate; its injection went red on `width`, which stays, so its
colour assertions move without re-deriving the gate. **Precondition for
(c):** no appearance setter left in `examples/`.

**(c) The deletion, sizes and docs.** Delete the moved fields from
`Style`, `Patch` and `NodeState` — the list T25.4 settled, not §2's
first-draft list. Update in the same commit:

- the size assertions in `crates/lumen-static/tests/cursor.rs`,
  `crates/lumen-static/tests/focus.rs` and `fixtures/embedded`. The
  target's `const` asserts cannot format a value, so their messages carry
  the expected number and `just firmware-size` prints the measured sizes
  beside them;
- every rustdoc that cites the 76 B / 72 B sizes or the full-struct
  argument: `invalidate.rs`, `desc.rs`, `event.rs`, `style.rs`,
  `input.rs`, and the shell's focus-ring comment;
- `docs/results-embedded.md`, each place it repeats the sizes;
- `docs/layout-and-storage-decisions.md` §3 — the "Widgets stay small"
  bullet and the "Known limit" paragraph, edited, not bannered;
- `docs/00-handoff-static-tree.md` open question 8, whose "a widget
  rather than a `Style` field" principle this generalises;
- `ui.getAppliedStyles`, which loses `color`, `background`, `radius`,
  `shadow` and `state.painted_background` as T25.4 decided, and
  `scripts/agent_client.py`'s `styles` command;
- `book/src/styling.md` ("What a `Style` costs") and
  `book/src/state-lives.md`, which lists the stamped fields;
- `docs/results-frame-cost.md` and `docs/results-glass.md`, which cite
  `pressed_dim`, and `crates/lumen-static-agent/src/session.rs`.

**Gate:** the new sizes asserted. *Injection:* re-add one `f32` to
`Style` — the host test fires printing both sizes, and the target build
fails with the expected size in its message.

## T25.9 — use the thing

`AGENT.md`, 2026-09-14: M24's defects were found by assembling passes
each tested alone.

- **Parity under both palettes.** `examples/minimal` gains an
  environment switch for the dark palette; `scripts/gpu_parity.sh` loops
  both, and asserts the light and dark captures **differ from each other**
  before comparing backends — otherwise it compares one palette twice.
- **Every widget changes.** The showcase gains a light/dark switch. With
  `phase` pinned, `just agent-drive` flips the palette and compares each
  catalogue node's resolved look through `ui.getAppliedStyles`, printing
  the kinds that did not change. `Glass` is exempt by name.
- **`scripts/ui_check.py contrast`** is extended to the roles of every
  migrated kind, prints measured-versus-present counts per role, and
  fails rather than skips a text-bearing node it cannot measure. Run
  under both palettes.

---

## 3. What this is not

**Not the end of the cascade.** Layout, cursor, focus ring and visibility
stay rules, and so do box fills unless T25.4 says otherwise.

**Not per-row status.** `Repeat` rows share their template's style row
and never receive hovered or pressed (§2.4). Looks resolved per instance
may differ per row in ways that do not depend on `Status`; that is a
consequence to test, not a promise.

**Not a stylesheet language, and not iced's `Catalog`.**

## 4. Deferred, with conditions

- **Producing `Disabled`.** A `disabled` field on the widget needs no
  `NodeState` byte; only a disabled state that cascades to descendants
  would. When a widget needs it.
- **Hover and press on `Repeat` rows.** Needs the repeater to keep the
  hit row from `Hit.inner` and hand it to that slot's layout. When a menu
  needs a row highlight.
- **Named looks the agent can select**, for per-node overrides a palette
  cannot express.
- **Renaming `Style`**, after T25.8.

## 5. What the review found, and what it changed

Six lenses, each verified by a second agent told to refute. The rows are
grouped; every one was re-checked against the code by its verifier.

| found | evidence | changed |
|---|---|---|
| **The premise was false**: hover reaches layout only under `Everything`; under `Boundaries` the shade is stale today, and the first draft's injection ("stop marking the node") named a mark that does not exist | the probe in T25.0; `pump_with` marks only on `handled` or a binding write | **T25.0 added**, first; §2.1 rewritten; every status gate moved to `Boundaries<16>` |
| The contrast gate could not fire: `ui_check.py` visits only Button and Label, inside the widget's own box | `scripts/ui_check.py` `check_contrast` | headless display-list gate in T25.6; `ui_check` extended in T25.9 |
| The `.text` kill switch measured a firmware with no Toggle in it | `nm` on the linked firmware: 0 Toggle symbols | toggles added to the fixture, symbols asserted first (T25.1) |
| "RAM per node must not grow" summed state + style, which can only shrink | `fixtures/embedded` `PER_NODE_RAM` | the struct term added, arena rows reported (T25.1) |
| The 10 000-node frame named no command and is 93% shaping; at the pilot, no toggle was in it | `docs/results-competitive.md`; `just stages` over `List<Label>` | named commands, a toggle bench, spread-relative threshold (T25.1) |
| `set_palette` "damages": damage comes from retention hashes, and the injection stayed green under `Everything` | `retain.rs`; `set_stylesheet` calls `mark_all` | invalidates, gated on `Boundaries` (§2, T25.3) |
| The removal deleted fields that `Repeat`, `Glass`, `Fixed`, `Image`, `Chart`, `focus_ring` and the derive's **hit test** still read | §2.3's table | §2.3 widened and reordered; T25.4 decides before any migration; T25.8 split in three |
| §2.3's preferred container option needed the same `NodeState` bytes as keeping the fields | derive `paint` has no palette | reordered; shape 1 is the default |
| No task migrated the examples; the pilot silently recoloured `form.rs`; kind-wide theming had no replacement | `form.rs` `Kind` rules; `just test` renders it | §2.4 added; example rules move with their widget |
| The migrated-kind lint had no marker and no channel; both §0 no-ops survived the plan | `Seed` is `{ kind, defaults }`; lints live in the agent audit | **T25.2 added**: one field-mask lint covering both |
| The hot-reload gate needed a `Reloadable` impl and a fixture that cannot express a colour; its injection edited the fixture | `reload.rs` impls; `fixtures/hot-app` | named in T25.5; injection moved to `restore` |
| No comparison with a smaller path | `Image::tint`, `Material` already exist | §2.6; T25.5 is a decision point |
| Smaller: iced's palette fields were the extended palette's; the showcase counts did not add up; two §0 reasons were not in their doc comments; `Rule` is not a widget; the agent method is `ui.getAppliedStyles`; ~10 tests, two chapters and several docs were missing from the removal list | recounts and greps | corrected in place |
