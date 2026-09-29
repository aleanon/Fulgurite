# M63 — style capabilities the framework does not have yet

> **Written 2026-09-29, not started. After M62**
> (`docs/plan-lss-selectors.md`). This is the third of four steps
> towards Lumen's `.lss` feature set. It adds the **framework**
> capabilities with the most value per unit of work: size limits and
> aspect ratio, margin, text alignment and ellipsis, opacity, gradient
> backgrounds, and a richer shadow. Each arrives as a `Style` field, a
> `Patch` setter and a `.lss` property together.
>
> **The fourth step, the large and design-level items, is deferred to
> M64 and is not started until M62 and this milestone are done.** §6
> lists every item, where it came from and why it waits.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and shows that it accepts good input.*

## 0. What each new field costs

Every `Style` field is paid per node, in `NodeState` or `Style` RAM,
on every target, including the embedded one. That is the milestone's
main risk, so it is priced before it is built:

- **K63.1 — RAM.** The embedded `Ui`'s size assert
  (`fixtures/embedded`) and `docs/results-embedded.md`'s RAM row are
  re-measured per task. The total growth of `Style` over the milestone
  is stated beforehand as ≤ 24 B per node. If a task would exceed its
  share, its field goes behind a style tier (a `rich-style` feature,
  off for the embedded profile), and the plan records it.
- **K63.2 — no cost when unused.** For a tree whose sheet uses none of
  M63's properties, `crossover` and the `grid` pump stay within their
  run-to-run spread, and the firmware's `.text` stays within
  `check_text.sh`'s 1% of its headline.

## 1. Where things stand, at `7d58aad`

- `Style`: `width height padding spacing color background radius
  font_size font_weight shadow pressed_dim hover_dim align focus_ring
  cursor border_width border_color visible`.
- `Length` is `Fixed | Shrink | Fill | FillPortion`. There is no
  minimum, maximum or aspect ratio.
- **No margin model.** `style.rs`'s rustdoc says so. Hit testing
  assumes a child lies inside its parent, which is why a negative
  margin was never allowed.
- **Text** is `Cmd::Text { rect, text, size, weight, color }`. It has
  no alignment inside its box and no ellipsis; the only typographic
  axis is weight (`Painter`).
- **The display list** has `Gradient { rect, stops, angle }` and
  `Shadow { rect, color, radius, spread }`, but `Style` exposes neither
  a gradient nor a shadow's colour, spread or offset.
- **No opacity anywhere:** colours carry alpha, but there is no group
  opacity.

## 2. Decisions

### 2.1 Opacity multiplies colours; it is not a layer

`opacity` on a node multiplies the alpha of every command its subtree
paints, through a `Painter` wrapper like `Scaled`. It does **not**
render to an offscreen layer. The difference shows only where a
subtree's own commands overlap each other: then they blend with each
other at the reduced alpha, where a layer would blend once. That is
recorded as a known difference, as CSS's `opacity` on overlapping
children would render differently.

A true layer needs an offscreen target in both renderers and is M64.
The wrapper must forward every `Painter` method; AGENT.md's `Deferring`
lesson applies, and T63.4's gate checks every command kind.

### 2.2 Margin is non-negative and belongs to the parent's layout

`margin` (and the four longhands) is space the **parent** leaves
around a child, added in the container's layout, never a negative
offset. So a child still lies inside its parent, and hit testing's
assumption holds. A negative margin is a compile error in `.lss`, and
is repaired to 0 and reported (`W1012`) when it comes from Rust.

### 2.3 Min/max and aspect ratio clamp in `resolve_self`

`min-width`, `max-width`, `min-height`, `max-height` and
`aspect-ratio` clamp the node's own resolved box, in the one place
every node's box is decided. Constraints passed to children follow
from the clamped box, as `Fixed` does today.

## 3. Tasks

### T63.0 — the gates first

One red gate per task below, plus K63.1's RAM rows and K63.2's
benchmark arms, with baselines at `7d58aad`.

### T63.1 — size limits and aspect ratio

§2.3, and `.lss`: `min-width max-width min-height max-height
aspect-ratio`.

**Gates:**

- `min-width` wins over a `Shrink` box, and `max-width` wins over
  `Fill`.
- `aspect-ratio` holds under both axes' constraints.
- A repeater row is clamped too.

**Injection:** apply the clamp after children are laid out; the
children's constraints are then wrong, and the gate names them.

### T63.2 — margin

§2.2, and `.lss`: `margin` and `margin-top/-right/-bottom/-left`.

**Gate:** row, column, wrap and grid containers each leave the margin,
and a click in the margin reaches the parent, not the child.
**Injection:** add the margin to the child's position without the
parent reserving it; the child then overlaps its sibling, and the gate
fails by pixel count.

### T63.3 — text alignment and ellipsis

`text-align: start | center | end` inside the text's box, and
`text-overflow: ellipsis` for a single line that does not fit. Both
live in the label and text-painting path, and both are measured with
the real face.

**Gates:**

- Centred text's ink is centred within ±0.5 px, measured by the gate
  itself. `ui_check.py centring` checks buttons at ±2 px, which is too
  loose to catch an alignment off by one pixel: the bound would come from
  the wrong quantity.
- An overflowing label ends in "…" and its ink stays inside its box.
- M45's memo keys include the new fields.

**Injection:** leave alignment out of the text memo key; the second
layout serves the old position.

### T63.4 — opacity

§2.1, and `.lss`: `opacity`.

**Gates:**

- Every `Cmd` kind painted under `opacity: 0.5` has its alpha halved,
  checked through the software renderer and the GPU parity harness.
- The wrapper forwards every `Painter` method; a new method added
  without forwarding fails to compile, or fails this gate.

**Injection:** make one `Painter` method provided rather than
forwarded; its command keeps full alpha, and the gate names the kind.

### T63.5 — gradient background

`background: linear-gradient(<angle>, <stop>, …)` through
`Cmd::Gradient`. The stops are evaluated at compile time, as M62's
colour functions are.

**Gates:** software and GPU parity within the existing tolerance; a
gradient under a rounded corner is clipped.

**Injection:** drop the radius clip; the corner pixels fail.

### T63.6 — a richer shadow

`box-shadow: <x> <y> <blur> <spread> <color>`. `Cmd::Shadow` already
takes a colour and a spread; the offset is new.

**Gates:** the shadow's ink is offset by (x, y) within ±0.5 px, and
damage includes the offset shadow, which M40's damage oracle checks.

**Injection:** leave the offset out of the damage rect; a moved card
leaves a stale shadow, and the oracle names it.

### T63.7 — docs

- The book's styling chapter (each property by anchor).
- The handoff's styling section.
- A results doc, `results-style-capabilities.md` (promised; written
  by this task), with K63.1's RAM figures and K63.2's timings.
- `docs/competitive-feature-gaps.md`'s style-language entry.

## 4. Boundaries of this plan

Everything not in §3 is §6's, deferred to M64.

## 5. Kill rules

- **K63.1** and **K63.2** (§0).
- **Per task:** a task that moves a hot path measured by `crossover`,
  `grid` or `cascade` beyond its spread, for a tree that does not use
  the feature, is fixed before the next task starts, or reverted.

## 6. Deferred to M64 — come back to these when M62 and M63 are done

**Decision (2026-09-29, the owner's): these wait.** Nothing here is
started until M62 and M63 are both closed with a green `just
ci-verify`. **When they are, M64 is planned from this list**, re-checked
against the code as it is then, like every other milestone's T.0.

### 6.1 Large or design-level

These are what the owner asked to wait:

- **Transitions, `animation`, `@keyframes`.** `Animated<T>` exists
  (`anim.rs`), but no style field is interpolated over time. This needs
  a design for where interpolation lives (cascade, tick or paint) and
  how it interacts with M41's invalidation.
- **`@media` and container queries.** A re-cascade on resize or scale
  is cascade work. Container queries make style depend on layout, which
  is a two-pass design question.
- **Run-time classes** (Lumen's `.value.pos` toggled at run time).
  Classes are `&'static` per type in `NodeDesc`. This needs a dynamic
  class set per node, and is why M61 has `@sheet` and `style_by`.
- **`transform`, `transform-origin`, `z-index`, `filter`, `blend-mode`,
  `backdrop-filter` as style.** The display list has no transform or
  z-order. `backdrop-filter` exists only as the `Glass<W>` widget. True
  group opacity (§2.1's layer) belongs here too.
- **`font-family` per node and italic.** `FontStack` has fallbacks but
  no per-node choice. `Painter`'s only typographic axis is weight.
- **Layout axes as style** (`display`, `flex-direction`, `flex-wrap`,
  `grid-template-*`, `grid-row/-column`). These are type-level derive
  axes by design, because the tree's shape is a type. Making them
  styleable is a question about the static tree itself, and it must be
  decided before anything is built.

### 6.2 Framework items not chosen for M63

These came from the same gap analysis and were not picked for M63.
They are also deferred, so none is lost:

- `justify-content`, `flex-grow`/`flex-shrink`/`flex-basis` (today
  `FillPortion` ≈ `flex-grow`), `align-items` vs `align-self`,
  `row-gap`/`column-gap`.
- `position`/`inset` (absolute positioning; today only the `stack`
  axis and overlays).
- `overflow` as style (today clipping is `clips_children`, per widget
  type).
- Borders per side (`border-top` and so on).
- `line-height`, `letter-spacing`, `text-decoration`, `text-wrap`,
  `font-features`/`font-variation`, `selection-color`.
- Widget states with no widget model yet: `:disabled`, `:readonly`,
  `:required`, `:invalid`, `:busy` (M62 §2.3).
- `calc()`, layout-dependent `%`, `ms`/`s`/`deg` units, nested rules
  (`&`), `!important`.

## Appendix: task → gate → injection

| task | gate | injection |
|---|---|---|
| T63.1 | min/max beat Shrink/Fill; aspect holds; repeater rows clamped | clamp after the children's layout |
| T63.2 | margin reserved in every container; margin clicks reach the parent | offset the child without reserving |
| T63.3 | centred ink ±0.5 px; ellipsis inside the box; memo keys | alignment left out of the memo key |
| T63.4 | every Cmd kind at half alpha, on both renderers | one Painter method provided, not forwarded |
| T63.5 | gradient parity; clipped at the corner | drop the radius clip |
| T63.6 | shadow offset ±0.5 px; damage includes it | offset left out of damage |
