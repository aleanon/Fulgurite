# Your first tree

An application here is two structs and a function. The struct with
`#[derive(StaticTree)]` is the *tree* — its fields are the widgets, in order,
and its nesting is the tree's nesting. The struct with `#[derive(App)]` (the
prelude calls it `DeriveApp`) is the *state*, and it holds the tree as one of
its fields rather than the other way round.

This is `examples/counter`, taken apart.

## The tree

```rust
# extern crate lumen_static;
# extern crate lumen_static_widgets;
# use lumen_static::prelude::*;
# use lumen_static_widgets::prelude::*;
# #[derive(DeriveApp, Default)] struct Counter { #[tree] ui: Ui, #[bind(ui.value, "count: {}")] count: i64 }
# fn main() {}
{{#include ../../examples/counter/src/main.rs:tree}}
```

Two things are worth slowing down for.

`Controls` is a field of `Ui`, and that is the whole nesting mechanism: a
widget-typed field is a child, and a `StaticTree`-typed field is a subtree.
There is no builder, no `children: Vec<_>`, and no place to put a node the
type does not already account for.

`#[widget_id("inc")]` names a node for anything that needs to find it later —
a stylesheet selector, a test, or the agent protocol. Fields without one
(`gap`) are still nodes; they just have no author-facing name.

## The state, and the binding

```rust
# extern crate lumen_static;
# extern crate lumen_static_widgets;
# use lumen_static::prelude::*;
# use lumen_static_widgets::prelude::*;
# #[derive(StaticTree, Default)] #[widget(column)] struct Ui { value: Label<String> }
# fn main() {}
{{#include ../../examples/counter/src/main.rs:state}}
```

`#[tree]` says which field is the tree. `#[bind(ui.value, "count: {}")]` says
how `count` reaches the screen, and the derive turns it into the frame loop's
update step: the label is refreshed from `count` between input and layout, on
every frame, so a handler cannot leave a stale value on the screen. The
formatting is declared once, next to the value it formats, instead of in a
method that has to be remembered at every call site.

Note where the truth lives: `count: i64` on the state is the number, and
`Label<String>` on the tree holds the *presentation copy*. Owning that copy is
what lets the label cache its measurement
(`docs/layout-and-storage-decisions.md` §1).

## Handlers

```rust
# extern crate lumen_static;
# extern crate lumen_static_widgets;
# use lumen_static::prelude::*;
# use lumen_static_widgets::prelude::*;
# #[derive(StaticTree, Default)] #[widget(column)] struct Ui { value: Label<String> }
# #[derive(DeriveApp, Default)] struct Counter { #[tree] ui: Ui, #[bind(ui.value, "count: {}")] count: i64 }
# fn main() {}
{{#include ../../examples/counter/src/main.rs:handlers}}
```

A handler is a plain `fn(&mut State, &EventCtx)` — no closure capture, no
message enum, no dispatch table. It is attached by assignment
(`c.ui.controls.inc.on_click = on_inc;`), which is why `Button<Counter>` is
generic in the state type: the button knows what it is allowed to mutate.

## Styling

```rust
# extern crate lumen_static;
# extern crate lumen_static_widgets;
# use lumen_static::prelude::*;
# use lumen_static_widgets::prelude::*;
# fn main() {}
{{#include ../../examples/counter/src/main.rs:style}}
```

`Selector::Kind("Ui")` matches by the type's name, so a rule written once
applies to every instance of that subtree. `Length::FILL` on the control row
is what lets the `Spacer` between the buttons push them apart — the job
flexbox would need `justify-content` for.

## Running it

The counter example runs headless: it builds a `Host`, pumps frames, clicks
its own buttons through the agent protocol, and writes a PPM. The window
version (`examples/counter-window`) adds exactly two things to that — window
events into `Host::post`, and `Canvas` pixels blitted to the surface.

```console
$ cargo run -p counter
counter: count=2 nodes=8 depth=3 cmds=7 ink=9.6% -> counter.ppm
```

Those are not decorative: `just fixtures` runs this example in CI, and its
assertions are in `main`, so a change that breaks the click path fails the
build rather than the next person to read the file.
