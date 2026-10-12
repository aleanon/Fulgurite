# Layout

Layout here is deliberately small. There are three container modes, three
lengths, one alignment axis, and no constraint solver. A container places
its children and that is the whole model — which means you can predict a
frame by reading the struct, and it means some things CSS gives you for
free have to be built out of the pieces.

## Three container modes

`#[widget(...)]` on a `StaticTree` struct picks one:

| mode | children |
|---|---|
| `row` | left to right, main axis is width |
| `column` | top to bottom, main axis is height |
| `stack` | **share the container's rect** |

`row` and `column` are `Axis::Row` and `Axis::Column`. `stack` is *not* a
third `Axis` variant, and the reason is worth internalising: every method
on `Axis` presupposes that one of width and height is the main one, and
a stack has no main axis at all. Its children do not divide the box, they
each get all of it.

## Four lengths

`Length` has exactly four cases:

* `Fixed(px)` — exactly this many pixels.
* `Shrink` — the node's own content size. The default for a leaf.
* `FillPortion(k)` — share what is left, in this ratio. `Length::FILL` is
  `FillPortion(1)`.
* `Flexible(k)` — its content size, but at most its share of what is
  left, in this ratio. It is the next section's subject.

A container is different from a leaf: **containers default to `FILL`
width and `Shrink` height**, whether they are a row, a column or a stack.
That default is why a column laid out in a window is as wide as the
window without being told to be.

## How a row divides its space

The order matters and it is short:

1. The container's own box, minus its `padding`, is the inner box.
2. `spacing` × (children − 1) comes off the main axis.
3. Every non-`Fill` child is measured, and may take up to what is left.
4. What remains after *that* is divided between the `Fill` children by
   portion.

Which is predictable enough to assert:

```rust
# extern crate fulgurite;
# extern crate fulgurite_widgets;
use fulgurite::prelude::*;
use fulgurite_widgets::prelude::*;
use fulgurite::pass;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 10.0)]
struct Bar {
    left: Fixed,
    gap: Spacer,
    right: Fixed,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    bar: Bar,
}

fn main() {
    let mut ui = Ui::default();
    ui.bar.left = Fixed { w: 40.0, h: 20.0 };
    ui.bar.right = Fixed { w: 60.0, h: 20.0 };

    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    let view = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 100.0 };
    pass::layout(&mut ui, &sty, &mut st, view);

    // 0 = Ui, 1 = Bar, 2 = left, 3 = gap, 4 = right.
    // 200 wide, two 10 px gaps, 40 + 60 of fixed children:
    // the spacer gets 200 - 20 - 100 = 80.
    assert_eq!(st[3].size.w, 80.0, "the spacer takes what is left");
    assert_eq!(st[2].pos.x, 0.0);
    assert_eq!(st[3].pos.x, 50.0);
    assert_eq!(st[4].pos.x, 140.0);
}
```

Nothing in that had to be solved iteratively, and nothing about it
depends on what the children contain — which is the property that lets
layout be memoised per subtree.

### `Flexible`: content, up to a share

A `Flexible(k)` child is measured after the others, like a fill, under
its share of what is left as a **maximum**: shares are counted over the
`Flexible` and `Fill` portions together, so it ends at the smaller of its
content and its share. What it does not use goes to the `Fill` children
— and only to them. Two `Flexible` children never hand each other their
leftover, because that would mean laying one out a second time; CSS
does, with a loop, and this layout has none
(`docs/layout-and-storage-decisions.md` §2 states the price, and
`docs/results-layout-input.md` T85.1 has the counts).

```rust
# extern crate fulgurite;
# extern crate fulgurite_widgets;
use fulgurite::prelude::*;
use fulgurite_widgets::prelude::*;
use fulgurite::pass;
use fulgurite::style::Length;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Bar {
    left: Fixed,
    right: Fixed,
}

fn widths(left: f32, right: Length) -> (f32, f32) {
    let mut ui = Bar::default();
    ui.left = Fixed { w: left, h: 20.0 };
    let mut sty = pass::default_styles(&ui);
    // 0 = Bar, 1 = left, 2 = right.
    sty[1].width = Length::Flexible(1);
    sty[2].width = right;
    let mut st = [NodeState::default(); Bar::NODE_COUNT];
    let view = Constraints { min_w: 0.0, max_w: 300.0, min_h: 0.0, max_h: 100.0 };
    pass::layout(&mut ui, &sty, &mut st, view);
    assert_eq!(st[0].size.w, 300.0, "the bar takes the whole 300 px");
    (st[1].size.w, st[2].size.w)
}

fn main() {
    // Each has a 150 px share. 80 px of content stays 80, and the fill
    // gets the 220 it left.
    assert_eq!(widths(80.0, Length::FILL), (80.0, 220.0));
    // 400 px of content is capped at its share.
    assert_eq!(widths(400.0, Length::FILL), (150.0, 150.0));
    // Beside another Flexible, the 70 px it left goes nowhere.
    let mut ui = Bar::default();
    ui.left = Fixed { w: 80.0, h: 20.0 };
    ui.right = Fixed { w: 400.0, h: 20.0 };
    let mut sty = pass::default_styles(&ui);
    sty[1].width = Length::Flexible(1);
    sty[2].width = Length::Flexible(1);
    let mut st = [NodeState::default(); Bar::NODE_COUNT];
    let view = Constraints { min_w: 0.0, max_w: 300.0, min_h: 0.0, max_h: 100.0 };
    pass::layout(&mut ui, &sty, &mut st, view);
    assert_eq!((st[1].size.w, st[2].size.w), (80.0, 150.0));
}
```

## There is no `justify-content`

A container places children sequentially and `Fill` divides the
remainder. That is the whole vocabulary, so main-axis centring is built,
not configured: put a `Fill` spacer on each side and they split the
leftover evenly.

```rust
# extern crate fulgurite;
# extern crate fulgurite_widgets;
use fulgurite::prelude::*;
use fulgurite_widgets::prelude::*;
use fulgurite::pass;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Page {
    top: Spacer,
    card: Fixed,
    bottom: Spacer,
}

fn main() {
    let mut ui = Page::default();
    ui.card = Fixed { w: 80.0, h: 100.0 };

    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Page::NODE_COUNT];
    let view = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 300.0 };
    pass::layout(&mut ui, &sty, &mut st, view);

    // Two equal spacers around a 100 px card in 300 px: 100 each.
    assert_eq!(st[1].size.h, 100.0, "top spacer");
    assert_eq!(st[2].pos.y, 100.0, "the card is centred");
    assert_eq!(st[3].size.h, 100.0, "bottom spacer");
}
```

`examples/counter-window/src/app.rs` centres its tally card exactly this
way, and its comment says so at the field.

## The cross axis is `Align`

Four values — `Start`, `Center`, `End`, `Fill` — and three of them place
the child. `Fill` is the odd one: it sets the child's cross-axis
*minimum* as well as its maximum, so the child stretches to the
container rather than merely being permitted to. On a `stack`, where
there is no cross axis either, `Align::Fill` applies that meaning to
both axes at once.

## Stacks, z-order and hit testing

A stack paints its children **in field order, so the last field is on
top**, and it hit-tests them **backwards**, so the topmost is found
first. Every other container may walk forwards, because its children
cannot overlap.

```rust
# extern crate fulgurite;
# extern crate fulgurite_widgets;
# use fulgurite_widgets::ops::Fixed;
# use fulgurite::{pass, Constraints, NodeState, StaticTree, Widget};
{{#include ../../crates/fulgurite/tests/stack.rs:stack}}
# fn main() {
#     let mut ui = Panel::default();
#     ui.under = Fixed { w: 80.0, h: 40.0 };
#     ui.over = Fixed { w: 80.0, h: 40.0 };
#     let sty = pass::default_styles(&ui);
#     let mut st = [NodeState::default(); Panel::NODE_COUNT];
#     pass::layout(&mut ui, &sty, &mut st,
#         Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 120.0 });
#     // Assert the setup happened first: two zero-sized rects are also
#     // equal, which is the vacuous shape this chapter warns about below.
#     assert!(st[1].size.w > 1.0 && st[1].size.h > 1.0, "the children have no area");
#     assert_eq!(st[1].pos.x, st[2].pos.x, "a stack's children share a rect");
#     assert_eq!(st[1].size.w, st[2].size.w);
# }
```

A stack sizes to the largest child **on each axis independently** — not
to the sum, and not to one child's box. `crates/fulgurite/tests/stack.rs`
asserts each of those separately.

## Two traps

**A `Fixed` child does not shrink to fit.** If you give a child more
pixels than its parent has, it takes them and overflows; nothing clamps
it and nothing warns. This is not hypothetical — a `Fixed(220)` card in a
300 px window put a button at x = 256, outside its own parent, and every
synthetic click in an invalidation test therefore landed on the root
instead of the button. The test compared two frames in which nothing had
happened and passed (`AGENT.md`, M16/T16.7). If you are driving an
application in a test, **assert that the action landed** — a changed
count, a `handled` flag — and not only that two results match.

**`visible = false` is not "skip the paint".** A hidden node resolves to
zero size, is not cross-stretched by its parent, records no drawing
commands and fails every hit test. All four clauses are asserted, for a
hidden container and a hidden leaf, in
`crates/fulgurite/tests/visible.rs` — because until 2026-09-02 none of
them held: no leaf read the flag at all, so hiding a `Label` left it
sized and painting (`docs/plan-review-fixes.md` F1.6).
