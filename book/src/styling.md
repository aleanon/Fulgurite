# Styling

Styling is a cascade, in Rust rather than in a stylesheet language. You
build a `Vec<Rule>`, hand it to the host, and every node's `Style` is
resolved once — **not per frame**.

## Rule, Selector, Patch

A `Rule` is a `Selector` and a `Patch`:

```rust
# extern crate lumen_static;
use lumen_static::prelude::*;

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

Since M25/T25.2 the host lints every sheet it is given. A rule is
reported when its selector matches no node — `Kind("Label")` above, or
an id nothing carries — or when a field it sets is read by none of the
nodes it matches: a checkbox paints no text, so a `font_weight` for one
changes nothing. `Host::sheet_findings` returns them by rule index, and
the agent's `ui.lint` reports each as `W1010`.

Which fields a kind reads is declared beside its kind name and **measured
rather than trusted**: `crates/lumen-static-widgets/tests/style_reads.rs`
renders every kind with and without each field and fails if a declared
mask disagrees with what the output did (`docs/results-appearance.md`).

## A rule may name a palette role instead of a colour

A `style::Palette` is the colours a theme is made of, named by role —
`page`, `surface`, `control`, `text`, `muted`, `accent`, `on_accent`,
`focus` — and the host holds one. A rule themes by naming a `style::Role`
where it would have named a colour:

```rust
# extern crate lumen_static;
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

The cascade resolves a role into the node's `Style` — `color_role` into
`color`, `background_role` into `background` — so nothing on the frame
path knows roles exist. `Host::set_palette` re-runs the cascade over the
sheet the host kept, and marks the tree changed so a skipped subtree is
laid out again; a rule that named a value keeps its value. The lint
counts a role as the field it sets, so a `background_role` on a kind
that paints no background is reported like a `background` would be.

`examples/showcase` is written this way: `LUMEN_PALETTE=dark just
showcase` starts it dark, and its `every_role_rule_follows_the_palette`
test checks every field a role decides against both palettes. What a role
cannot reach is a colour a widget keeps as its own field — a toggle's
`mark`, a text field's `caret_color` — and a pressed look that is more
than a shade; `docs/plan-appearance.md` T25.10 and T25.11 record why
those were left there.

## Ids and classes come from the tree

They are attributes on the field, not values set at runtime:

* `#[widget_id("save")]` — one stable author id per field. It is also
  the agent protocol's selector and the change journal's key, so it is
  worth spelling deliberately.
* `#[widget_class("row", "striped")]` — any number of classes.

Both are **static, fixed in the type**. There is no `add_class` at
runtime, which is a real constraint and has a real workaround: swap the
sheet. `examples/counter-window/src/app.rs` recolours its number by sign
that way — `stylesheet_for(sign)` returns a different `Vec<Rule>` and the
host re-cascades. The mechanism underneath is the same one a class toggle
would use; the difference is where the branch lives.

## A worked stylesheet

This is the whole sheet from `examples/counter`, unedited:

```rust
# extern crate lumen_static;
# use lumen_static::prelude::*;
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
