# M62 — `.lss` catches up: cheap syntax, and selectors the cascade can match

> **Written 2026-09-29, not started.** The first two of four steps
> towards Lumen's `.lss` feature set, after M61 (`docs/plan-lss.md`)
> made `.lss` a compile-time format with v1's grammar. This milestone
> does the parts that are **syntax and cascade only**. It adds no new
> layout, paint or text capability; that is M63
> (`docs/plan-style-capabilities.md`). The large and design-level items
> are **deferred to M64 and not started until M62 and M63 are done**
> (M63 §6 lists them and says why).
>
> Part A (T62.1–T62.4) is `.lss` syntax and lowering over capabilities
> that already exist. Part B (T62.5–T62.7) extends the cascade:
> compound selectors, combinators, and `:state` for the states the
> framework already tracks.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and shows that it accepts good input.*

## 0. What must not get worse

M61's claims still hold for every sheet that does not use a new
feature:

- **K1 (M61) — zero bytes:** a sheet written without M62's features
  compiles to the same firmware `.text` and `.rodata` as before.
  `scripts/check_lss_parity.sh` keeps running in `just firmware`.
- **The cascade does not slow down for sheets that do not use
  compound selectors, combinators or states** (kill rule K62.1, §5).

## 1. Where things stand, at `7d58aad`

- `.lss` accepts 18 properties (`crates/lumen-static-lss/src/lower.rs`,
  `PROPERTIES`): `width height padding spacing align visible cursor
  focus-ring color background border-width border-color border-radius
  font-size font-weight box-shadow pressed-dim hover-dim`. Values are
  hex colours, `px`, keywords, `$tokens` and palette roles.
- `Patch.padding` is `Option<Edges>`: all four sides at once, so a
  longhand such as `padding-top` has nowhere to go without a core
  change.
- `Selector` is `Id | Class | Kind | Any`, one simple part each.
  Matching is `style.rs`'s `matches(sel, d, kind)`, one line per
  variant. Specificity is `Selector::specificity() -> u8`, 0–3, and the
  cascade applies rules in ascending specificity, then declaration
  order.
- **States the framework already has:** `InputState::hovered`,
  `capture` (the pressed node), `focus` and `focus_visible`
  (`crates/lumen-static/src/event.rs`). Hover and press reach paint as
  `hover_dim` and `pressed_dim`, which are paint-time dims, not rules.
- **Widget states** (`toggled`, `expanded` and so on) are published
  only through `observe`, for the agent and a11y (M48, M54). A default
  build has no core-level way to ask a node whether it is checked.
- Lumen's grammar, for reference: `/mnt/Utvikling-linux/Lumen/crates/lumen-style`
  (compound selectors, combinators, 14 `:state` names, `@theme`,
  colour functions, the CSS_ALIASES table).

## 2. Decisions

### 2.1 A compound selector stays `const`

`Selector` gains `All(&'static [Simple])`, where `Simple` is today's
four parts. `#card.wide`, `.value.pos` and `button.primary` lower to
it. Specificity becomes a tuple `(ids, classes + states, kinds)`,
compared lexicographically, as in CSS and Lumen. The existing four
variants keep their meaning and their `u8` ordering maps into the
tuple, so every existing sheet orders exactly as before.

### 2.2 A combinator walks the parent chain

`Selector::Chain(&'static [(Combinator, &'static [Simple])])` for
`a b` (descendant) and `a > b` (child), matched right to left against
`NodeDesc::parent`, as a browser does. It is O(depth) per candidate
node; K62.2 bounds the cost.

### 2.3 `:state` re-cascades the node that changed, not the tree

A state rule (`#save:hovered`, `.row:selected`) is resolved by the
cascade, not at paint. When a tracked state changes on node *i*, the
host re-cascades node *i*'s subtree only, with `cascade_subtree`
(the scoped cascade `Host::restyle_subtree` already uses), and marks it. It does not re-run the whole sheet.

This is weighed against today's `hover-dim`, which costs nothing
extra per hover. So K62.3 compares a hover frame with a `:hovered` rule
against the same hover with `hover-dim`. If the rule path loses, state
rules whose only effect is a colour dim lower to `hover_dim` and
`pressed_dim` instead, and only the rest re-cascade.

**Which states**, in v1 of this milestone:

- From `InputState`: `:hovered`, `:pressed`, `:focused`,
  `:focus-visible`.
- From the widgets: `:checked`, `:unchecked`, `:mixed`, `:selected`,
  `:expanded`, `:collapsed`. These need a core query that does not
  depend on `observe` (§2.4).
- **Not** `:disabled`, `:readonly`, `:required`, `:invalid` or
  `:busy`: no widget models them yet. That is widget work, deferred to
  M64 (M63 §6), and until then each is a named compile error.

### 2.4 Widget states through a node query, not through `observe`

A provided `Node::style_state(&self) -> StateSet` (a `u16` bitset),
default empty, overridden by the toggles, `Collapsible`, `Tree`,
`Select` rows and the choosers. The host asks it **by table index**
(`node_mut(i)`), as M54's `Event::Action` does. A wrapper is never
the receiver, so AGENT.md's rule that a provided method is wrong for a
wrapper does not bite here. T62.7's gate checks that claim with a
wrapped toggle.

The existing `observe` state strings are then derived from
`style_state()`, so the agent, a11y and the cascade read one source.
M48's serialiser gets a gate that they agree.

## 3. Tasks

### T62.0 — the gates first

Committed ignored (`red until T62.n`), with their red output. One per
task below, plus K62.1–K62.3's arms in `benches --bin cascade`, with
baselines recorded at `7d58aad`.

### Part A — syntax over what exists

#### T62.1 — padding longhands

`padding-top`, `-right`, `-bottom` and `-left`. `Patch.padding`
becomes four `Option<f32>`s (setters stay `const`; `padding(Edges)`
sets all four), so a longhand composes with a shorthand in declaration
order, as in CSS.

**Gate:** `padding: 8px; padding-left: 20px;` resolves to (8, 8, 8, 20),
and the reverse order resolves to (8, 8, 8, 8). **Injection:** let the
shorthand ignore a longhand already set, and the second case fails.
**K1:** the firmware is byte-identical for sheets without longhands.

#### T62.2 — colour functions

`rgb()`, `rgba()`, `oklch()` and `color()`, evaluated at compile time
to a `Color`. Out-of-gamut `oklch` is clamped, and the clamp is
reported as a warning in the macro's output, not silently applied.
`%` is accepted for alpha and for channels only. Layout-dependent `%`
and `calc()` are M64.

**Gate:** each function against known conversions (Lumen's own test
values), and a malformed call as an error at `file:line:col`.
**Injection:** drop `oklch`'s gamut clamp, and the out-of-gamut case
fails.

#### T62.3 — `@theme`

`@theme light | dark | high-contrast { role: value; }` sets palette
role colours. The block lowers to a `Palette` const per theme, which
`Program::palette` and `app.setPalette` select by name. Core gains
`Palette::HIGH_CONTRAST`, with its role pairs measured at ≥ 7:1 by
M54's contrast helper.

**Gate:** a `.lss` theme's roles equal the palette the program shows
after `app.setPalette { name: "dark" }`, pixel-checked through
`LUMEN_SHOT`. **Injection:** swap two roles in the lowering.

#### T62.4 — did-you-mean and CSS aliases for properties

Adopt Lumen's CSS_ALIASES table (`box-shadow`, `left` →
`inset-left`, …) and extend did-you-mean from tokens to property
names. An alias is **an error with a suggestion, never accepted**:
this is Lumen's reasoning, recorded in its `properties.rs`, and it
keeps one spelling per property.

**Gate:** `background-colour` suggests `background`, and
`border-radius` still parses. **Injection:** return no suggestion.

### Part B — the cascade

#### T62.5 — compound selectors

§2.1. **Gate:** `.value.pos` matches only a node with both classes;
`#card.wide` wins over `#card` by specificity; every existing sheet in
the repo cascades to identical `NodeState`s, compared before and after.
**Injection:** match a compound on any one of its parts, and the
both-classes case fails. **K62.1** is measured here.

#### T62.6 — combinators

§2.2. **Gate:** `#card > .row` does not match a `.row` nested two deep,
and `#card .row` does. **Injection:** treat `>` as descendant.
**K62.2** is measured here.

#### T62.7 — `:state`

§2.3 and §2.4:

- `StateSet`;
- `Node::style_state` on the widgets that have state;
- the scoped re-cascade on a state change;
- `observe`'s strings derived from `style_state`.

**Gates:**

- A `:hovered` rule repaints on the hover frame and reverts on leave,
  with both strategies compared (`Everything` against `Boundaries`,
  M41's oracle judging the Boundaries leg).
- `:checked` follows a click.
- A toggle inside a wrapper widget reports its state.
- The agent's `toggled` and the cascade agree on every drivable
  widget.

**Injection:** skip the mark after a state re-cascade, and the
Boundaries leg paints the old colour. The oracle must name the node.
**K62.3** is measured here.

## 4. Boundaries of this plan

- No new style properties or layout, paint or text capabilities: that
  is M63.
- No `:disabled`, `:readonly`, `:required`, `:invalid` or `:busy`
  (§2.3), no run-time classes, no `@media`, no transitions, no nested
  rules, no `!important`, no `calc()`. All of those are deferred to
  M64, and none is started before M62 and M63 are done (M63 §6).
- No compile-time cascade (M61 §4).

## 5. Kill rules

Written before anything is built. Timings go under `just bench-lock`,
eight runs a side.

- **K62.1:** `cascade_deep` with a sheet that uses no compound
  selector, combinator or state stays within its own run-to-run spread
  of the `7d58aad` baseline. If it does not, `matches` keeps a fast path
  for simple selectors, and that path is measured.
- **K62.2:** `cascade_deep` with a showcase-sized sheet using
  descendant combinators costs no more than 2× the same sheet with
  simple selectors. If it costs more, combinators are restricted to
  `>` (O(1) per candidate) and descendant stays deferred.
- **K62.3:** a hover frame with a `:hovered` colour rule costs no more
  than 1.5× the same hover with `hover-dim`, on the showcase, as whole
  pump time. If it costs more, colour-only state rules lower to the
  paint-time dims (§2.3).

## Appendix: task → gate → injection

| task | gate | injection |
|---|---|---|
| T62.1 | shorthand/longhand order resolves as in CSS | shorthand ignores a set longhand |
| T62.2 | functions against known values; errors at file:line:col | drop oklch's gamut clamp |
| T62.3 | `.lss` theme equals the shown palette (LUMEN_SHOT) | swap two roles |
| T62.4 | a misspelling suggests the real name; aliases refused | return no suggestion |
| T62.5 | compound needs all parts; specificity tuple; old sheets identical | match on any one part |
| T62.6 | `>` is one level, space is any depth | treat `>` as descendant |
| T62.7 | `:hovered`/`:checked` repaint under both strategies; wrapper; agent agrees | skip the mark after a state re-cascade |
