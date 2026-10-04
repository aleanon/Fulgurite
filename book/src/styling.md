# Styling

Styling is a cascade. You build a `Vec<Rule>` — in Rust, or in a `.lss`
file that becomes the same rules while your crate compiles (below) —
hand it to the host, and every node's `Style` is resolved once — **not
per frame**.

## Rule, Selector, Patch

A `Rule` is a `Selector` and a `Patch`:

```rust
# extern crate lumen_static;
# extern crate lumen_static_widgets;
use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;

fn main() {
    let sheet = vec![
        Rule::new(
            Selector::Kind("Card"),
            Patch::new().padding(Edges::all(16.0)).width(Length::FILL),
        ),
        Rule::new(Selector::Class("muted"), Patch::new().font_size(13.0)),
        Rule::new(Selector::Id("title"), Patch::new().font_weight(700.0)),
    ];
    assert_eq!(sheet.len(), 3);
}
```

A `Patch` is every `Style` field, each optional. **A `None` field leaves
the cascaded value alone**, so rules compose instead of overwriting: a
kind rule can set the padding and an id rule can set the weight, and both
survive.

Specificity decides who wins, highest first:

| selector | writes as | specificity |
|---|---|---|
| `Selector::Id("save")` | `#save` | 3 |
| `Selector::Class("row")` | `.row` | 2 |
| `Selector::Kind("label")` | `label` | 1 |
| `Selector::Any` | `*` | 0 |

Ties break on declaration order — later wins.

## Borders

A border is a `Patch` field like any other, and it cascades like any
other:

```text
Rule::new(
    Selector::Kind("textinput"),
    Patch::new().border_width(1).border_color_role(Role::Line),
)
```

Two things about it are worth knowing because they are choices, not
accidents.

**It is drawn *inside* the node's box.** A bordered control and an
unbordered one in the same row line up; adding a border never changes a
layout. That is the opposite of the focus ring, which is deliberately
drawn *outside* — a ring means "this has focus" and has to be visible
past the control's own edge.

**The width is a `u8`, in logical pixels.** A border wider than 255 px is
not a border, and `Style` is resolved per node — a byte here is a byte
times every node of every tree. What the border cost the embedded
profile is in `docs/results-embedded.md`, measured rather than estimated.

`border_color_role` follows the palette, so `Host::set_palette` re-resolves
it like any other role.

## A gradient behind a node: the fill table

A gradient is fifty-odd bytes and a `Style` is paid per node, so a rule
does not carry one: it names one, by index. The program hands the host a
table of `style::Fill`s, and `Patch::background_fill(n)` paints entry
`n - 1` under the node's background (`0`, the default, is none). An
entry's stops may name palette roles, so a fill themes the way a
`background_role` does — `Host::set_palette` re-resolves the table:

```rust
# extern crate lumen_static;
use lumen_static::display::{Color, GradShape, Stops};
use lumen_static::style::{Fields, Fill, Palette, Patch, Role};

const FILLS: [Fill; 1] = [Fill::new(
    Stops::two(Color(255, 255, 255, 255), Color(0, 0, 0, 255)),
    GradShape::Radial { center: [0.5, 0.5], radius: 1.0 },
)
.role(0, Role::Raised)];

# fn main() {
let rule = Patch::new().background(Color(0, 0, 0, 0)).background_fill(1);
assert!(rule.fields().contains(Fields::BACKGROUND_FILL));
// The role-named stop is whatever the palette calls `raised`.
assert_eq!(FILLS[0].resolve(&Palette::DARK).stops.color[0], Palette::DARK.raised);
assert_eq!(FILLS[0].resolve(&Palette::DARK).stops.color[1], Color(0, 0, 0, 255));
# }
```

The table goes on the program — `application(boot).fills(&FILLS)` — and
the showcase's Card demo is drawn this way:

```text
{{#include ../../examples/showcase/src/main.rs:card_fill}}
```

**It paints under the background**, so a node with an opaque background
hides its fill; the card above sets its background transparent. Hover
and press shading act on the background, so a fill alone shows neither.
Every kind that draws a border paints a fill (its seed declares
`Fields::BACKGROUND_FILL`); a `background_fill` rule on any other kind is
a `W1010` finding, and `scripts/check_fill_paint.sh` keeps the two calls
paired. In `.lss` it is `background-fill: 1` — a bare number, not a
length. What it cost per node is in `docs/results-embedded.md`
(M82/T82.3).

## The one thing that will trip you: kind names are not spelled alike

`Selector::Kind` matches `Node::kind_name`, and it has **two
conventions**:

* **Built-in leaves are lowercase** — `label`, `button`, `fixed`,
  `spacer`.
* **Derived containers use the struct's own ident** — `Ui`, `Controls`,
  `Card`.

So `Selector::Kind("label")` and `Selector::Kind("Ui")` are both correct,
and `Selector::Kind("Label")` silently matches nothing. Nothing in the
type system tells you which you need. This was documented nowhere until
2026-09-02 (`docs/plan-review-fixes.md` F7.5).

## A rule that does nothing is reported

A host built with the `agent` feature lints every sheet it is given. A
rule is reported when its selector matches no node — `Kind("Label")`
above, or an id nothing carries — or when a field it sets is read by none
of the nodes it matches: a checkbox paints no text, so a `font_weight`
for one changes nothing. `Host::sheet_findings` returns them by rule
index, and the agent's `ui.lint` reports each as `W1010`.

**Behind a feature since M26/T26.18, and it used to be unconditional.**
The walk is O(rules × nodes) and re-walks every arena once per rule —
183 µs on a 27-rule sheet over 901 nodes, against the 66 µs the cascade
costs over the same sheet (`docs/results-frame-and-types.md` T26.0) —
and its only consumer is the agent. A build with no agent was paying it
on every sheet swap for findings nothing could read. Turn it on with
`lumen-static-shell`'s `agent` feature, which `lumen-static-winit`'s
`agent` feature already implies.

Which fields a kind reads is declared beside its kind name and **measured
rather than trusted**: `crates/lumen-static-widgets/tests/style_reads.rs`
renders every kind with and without each field and fails if a declared
mask disagrees with what the output did (`docs/results-appearance.md`).

## A rule may name a palette role instead of a colour

A `style::Palette` is the colours a theme is made of, named by role —
`page`, `surface`, `control`, `text`, `muted`, `accent`, `on_accent`,
`focus`, and since M57 `raised`, `border`, `track`, `placeholder`,
`scrim`, `danger` and `on_danger` — and the host holds one. A rule themes by naming a `style::Role`
where it would have named a colour:

```rust
# extern crate lumen_static;
# extern crate lumen_static_widgets;
use lumen_static::style::{Palette, Patch, Role, Rule, Selector};
use lumen_static::Color;

fn sheet() -> [Rule; 2] {
    [
        // Follows the palette: whatever it calls text.
        Rule::new(Selector::Kind("label"), Patch::new().color_role(Role::Text)),
        // Does not: a hairline is this colour under every palette.
        Rule::new(Selector::Id("hr"), Patch::new().background(Color(0xdf, 0xe4, 0xed, 0xff))),
    ]
}
# fn main() {
#     let s = sheet();
#     assert_eq!(s[0].patch.color_role, Some(Role::Text));
#     assert_eq!(Palette::DARK.get(Role::Text), Palette::DARK.text);
# }
```

**A widget's own colours are roles too** (M57/A8). A kind declares
them on its `Seed` with `with_roles`, and the cascade resolves them
against the host's palette before any rule applies. So a label is the
palette's `text`, a card its `raised`, a badge its `danger`, with no rule
in the sheet, and a rule that names a colour or a role still wins because
rules apply after. Until M57 the palette reached a widget only through a
rule, and every widget's defaults were light-palette literals. An
application had to restyle every kind for dark mode, and the showcase did
it for four (`docs/widget-audit.md` X6).
`lumen-static-widgets/tests/palette_defaults.rs` checks every kind in both
palettes: each colour it paints must be a role, and each pair it shows
must meet its bound. Colours that are *data*, such as a chart's series or
a colour picker's swatches, stay literal.

The cascade resolves a role into the node's `Style` — `color_role` into
`color`, `background_role` into `background` — so nothing on the frame
path knows roles exist. `Host::set_palette` re-runs the cascade over the
sheet the host kept, and marks the tree changed so a skipped subtree is
laid out again; a rule that named a value keeps its value. The lint
counts a role as the field it sets, so a `background_role` on a kind
that paints no background is reported like a `background` would be.

`examples/showcase` is written this way: `LUMEN_PALETTE=dark just
showcase` starts it dark, and its `every_role_rule_follows_the_palette`
test checks every field a role decides against both palettes;
`chrome_text_meets_contrast` checks that every text colour its chrome
paints reads on its ground at WCAG AA in both
(`docs/plan-showcase-design.md` T56.3). What a role
cannot reach is a colour a widget keeps as its own field — a toggle's
`mark`, a text field's `caret_color` — and a pressed look that is more
than a shade; `docs/plan-appearance.md` T25.10 and T25.11 record why
those were left there.

**A palette is checked where it is painted.** The showcase's tests
measure every text run of the catalogue against the fill under it, from
the display list, under its light palette and the core's — 4.5:1 for
text, 3:1 in the tests that also cover the dark ones — and the values
that failed were changed rather than a role added
(`docs/results-a11y-platform.md` has them). The dark palettes do not yet
pass: the widgets' own defaults are light-theme values, which a
stylesheet has to re-colour.

## Ids and classes come from the tree

They are attributes on the field, not values set at runtime:

* `#[widget_id("save")]` — one stable author id per field. It is also
  the agent protocol's selector and the change journal's key, so it is
  worth spelling deliberately.
* `#[widget_class("row", "striped")]` — any number of classes.

Both are **static, fixed in the type**. There is no `add_class` at
runtime, which is a real constraint and has a real workaround: swap the
sheet. `examples/counter-window` recolours its number by sign that way —
`stylesheet_for(sign)` returns a different sheet for each sign, and
`Program::style_by` (or `Program::style_lss_by`, for a `.lss` sheet,
below) has the host re-apply it on the frame the sign changes. The
mechanism underneath is the same one a class toggle would use; the
difference is where the branch lives.

## A stylesheet as a file: `.lss`

A sheet can be written as a `.lss` file — CSS-shaped, and Lumen's format
cut down to what this cascade can apply — and **cost nothing at run time
that the same sheet written in Rust does not**. `include_lss!` reads the
file while your crate compiles and expands to the `Rule::new(..)` /
`Patch::new().setter(..)` a person would write, as a `const`: a firmware
linked with its sheet authored either way is byte-identical
(`docs/results-lss.md`, K1).

This is the tally's, `examples/counter-window/app.lss`:

```text
{{#include ../../examples/counter-window/app.lss:base}}
```

and the part that depends on the count's sign:

```text
{{#include ../../examples/counter-window/app.lss:sheet}}
```

On the Rust side, one `include_lss!` per sign — the tree type first, so
every `#id` and `.class` in the file is checked against it — and the
program keyed on the sign:

```text
{{#include ../../examples/counter-window/src/app.rs:lss}}
```

```text
{{#include ../../examples/counter-window/src/app.rs:program}}
```

What v1 accepts:

* **Selectors**: `#id`, `.class`, a kind word (spelled as above —
  `label`, `Page`), `*`, and comma lists of them. One simple selector per
  rule.
* **Declarations**, one per `Patch` setter: `width`, `height` (`12px`,
  `fixed(12px)`, `fill`, `fill(2)`, `shrink`), `padding` (one to four
  lengths, in CSS order), `spacing`, `align`, `visible`, `disabled`, `cursor`,
  `focus-ring`, `color`, `background`, `border-width`, `border-color`,
  `border-radius`, `font-size`, `font-weight`, `box-shadow`, `hover-dim`,
  `pressed-dim`, `line-height` (a bare ratio, `1.5`), `text-overflow`
  (`wrap`, `clip` or `ellipsis`), `background-fill` (a fill table index,
  a bare number, `1`). A setter added to `Patch` without a declaration here
  fails `just static`.
* **`@tokens { name: value; }`**, used as `$name`.
* **Palette roles as tokens**: `$page`, `$surface`, `$control`, `$text`,
  `$muted`, `$accent`, `$on-accent`, `$focus`, `$raised`, `$border`,
  `$track`, `$placeholder`, `$scrim`, `$danger`, `$on-danger` — every
  `Role`, held to the enum by `scripts/check_lss_setters.sh` — lower to the `*_role`
  setters, so the colour follows the palette as `Patch::color_role` does.
  A `@tokens` name may not shadow one — it would silently stop following
  the palette — which is why the tally's `page` token is `ground`.
* **`@sheet name { rules }`** — not CSS. A named block, chosen by
  `include_lss!(Tree, "app.lss", name)`: the top-level rules, then the
  block's. It stands in for the run-time classes this framework does not
  have, and it will be revisited if those ever exist.

**A mistake is a build error at the file's line and column**, not a
finding at run time:

```text
error: app.lss:3:5: unknown property `colour`; did you mean `color`?
error: app.lss:1:1: compound selector `.value.pos`: v1 takes one simple selector per rule ...
error[E0080]: evaluation panicked: app.lss:2:1: `#titel` names no node -- there is no `#[widget_id("titel")]` in `Ui` ...
```

Everything Lumen's grammar has beyond v1 — compound selectors,
combinators, `:state`, `@media`, `@keyframes`, `@theme` — is **refused
with its own message, not ignored**: a sheet that silently dropped a rule
would be worse than no sheet. Hover and press are the `hover-dim` and
`pressed-dim` declarations, because that is what the cascade applies.

**Kinds are not checked at compile time.** A node's kind is a run-time
method, not a table entry, so `Label { .. }` — the wrong case, as above —
compiles, and the `W1010` lint names it when the sheet is applied.

**In a debug window the file is live.** `Program::style_lss` and
`Program::style_lss_by` take the compiled sheet *and* the file's path; a
debug build (or one with `lumen-static-winit/reload`) reads the file
instead, and again whenever it is saved, so a colour change needs no
rebuild. A file that does not lower draws with the compiled sheet and
prints why. A release window without `reload` links none of the reader
(`docs/results-lss.md`, K3), and an edited `.lss` rebuilds the crate that
includes it.

## A worked stylesheet

This is the whole sheet from `examples/counter`, unedited:

```rust
# extern crate lumen_static;
# extern crate lumen_static_widgets;
# use lumen_static::prelude::*;
# use lumen_static_widgets::prelude::*;
{{#include ../../examples/counter/src/main.rs:style}}
# fn main() { assert_eq!(stylesheet().len(), 2); }
```

Two rules, matching two container *kinds* by their struct names. The
second is doing real work: `Length::FILL` on the control row is what
gives the `Spacer` inside it something to divide, which is how the two
buttons get pushed apart. Styling and layout are not separate systems —
`width` is a `Style` field.

## When the cascade runs

Once, when the sheet or the tree's shape changes — `Host::new` and
`Host::set_stylesheet` — and not on the frame path. That is only sound
because a node's kind and its defaults depend on the *type* alone and
never on instance state, so a resolved `Style` cannot go stale while the
sheet and the table are unchanged.

Since M24 that is enforced rather than asked for. Both facts live in a
`style::Seed`, one per node, in a const `Widget::SEEDS` array spliced
the way `SUB_TABLE` is; `Node::kind_name` and `Node::defaults` forward
to the type-level halves. The rule used to be prose on
`Node::defaults`, and prose could not reach the case that mattered — a
*hidden* subtree has no instance to ask, so an `Option` that was `None`
or an inactive enum variant cascaded from its container's kind and
defaults instead of its own (`docs/plan-select.md` §1).

Off-table constructs — `Repeat`, `List`, `Either`, `Dyn` — keep their
instances in an `Arena`, and the sheet reaches inside them by *template*
(`cascade_deep`). One consequence to plan around: **per-instance styling
does not exist.** Row 7 of a `Repeat` cannot be styled differently from
row 6 by selector; if rows must differ, the difference belongs in what
they contain.

## What a `Style` costs

Every field on `Style` is paid per node, in every tree, and the M15
gating policy treats that as a budget rather than an afterthought:
`focus_ring` is a `u8` because as an `f32` it pushed the struct over a
size boundary, and `cursor` is free only because it fits in the tail
padding. `crates/lumen-static/tests/cursor.rs` is where that budget is
asserted. If you are adding a property, that test is the gate you have to
argue with.
