# State, bindings and handlers

An application is two structs. [Your first tree](./first-tree.md) showed
them side by side; this chapter is about the second one — the state, and
the three declarations that connect it to the screen.

The shape to keep in mind: **the state struct owns the tree, not the
other way round.** A widget field holds what is on screen; a sibling
field holds what is true. `#[bind]` is the rule that keeps them in
agreement, and the frame loop applies it between input and layout so it
cannot be forgotten.

## All four forms, in one struct

This is `crates/fulgurite/tests/derive_app.rs`, which exists to pin
exactly this behaviour down — so every line below is compiled *and run*
by `just test`, and by this page.

```rust
# extern crate fulgurite;
# extern crate fulgurite_widgets;
use fulgurite_widgets::ops::{Button, Label, Spacer};
use fulgurite::{App as AppDerive, EventCtx, StaticTree};

{{#include ../../crates/fulgurite/tests/derive_app.rs:tree}}

{{#include ../../crates/fulgurite/tests/derive_app.rs:projection}}

{{#include ../../crates/fulgurite/tests/derive_app.rs:hook}}

{{#include ../../crates/fulgurite/tests/derive_app.rs:state}}

{{#include ../../crates/fulgurite/tests/derive_app.rs:handler}}

fn main() {
{{#include ../../crates/fulgurite/tests/derive_app.rs:assertions}}

    // A handler is a bare fn over the state type, and wiring one is
    // assignment — no registration, no dispatch table.
    c.ui.go.on_click = bump;
}
```

Four declarations, four jobs:

| written | does |
|---|---|
| `#[tree] ui: Ui` | names the field that is the widget tree. Exactly one. |
| `#[bind(ui.value, "count: {}")]` | formats this field into that label, every frame |
| `#[bind(ui.ratio, "{} of {}", self.total)]` | same, with extra arguments — **the annotated field is always the first** |
| `#[bind(ui.shout, with = shout)]` | hands it to a `fn(&mut Target, &Source)` instead of formatting |
| `#[app(after_update = recompute)]` | a `fn(&mut Self) -> bool` run once after all the bindings |

A field may carry more than one `#[bind]`. The tally in
`examples/counter-window/src/app.rs` puts two on its single `count`: one
formats the hero number, the other projects the *sign* into a word
through `with =`, so `POSITIVE` and `AT ZERO` are derived rather than
stored. One piece of state, two things on screen, no update method.

## `with =` is for when a format string cannot say it

The projection takes the target widget and a reference to the source:

```rust
# extern crate fulgurite;
# extern crate fulgurite_widgets;
# use fulgurite_widgets::ops::Label;
{{#include ../../crates/fulgurite/tests/derive_app.rs:projection}}
# fn main() { let _: fn(&mut Label<String>, &str) = shout; }
```

`set_if_changed` is the named spelling for "write this only if it
differs". That matters more than it looks: a `Label` owns its text so it
can cache its measurement, and writing an identical string would throw
the measurement away for nothing. `docs/bindings-decision.md` has why
the comparison sits here rather than at the call site, and what it costs.

## Two things called `App`

There is a trait `App` and a derive `App`, and you will meet both:

* `fulgurite::event::App` — the trait. The frame loop calls
  `update()` on it.
* `fulgurite::App` — the derive that *writes* that impl.

The prelude re-exports the derive as `DeriveApp` to keep them apart, and
the file above spells its own alias `AppDerive`. Both are the same
macro; only the local name differs.

## `#[app(after_update)]`, and when you need it

A binding is a per-field rule. When a value depends on several fields at
once, or on something a format string cannot reach,
`#[app(after_update = recompute)]` names a `fn(&mut Self) -> bool` that runs
after every binding has been applied.

The order is the point. Bindings first, then the hook, then layout — so
the hook sees a tree that already agrees with the state, and nothing
downstream can see the intermediate frame. *What a frame does* — the
next part of the book — is the whole sequence.

## Handlers are plain functions

`bump`, in the snippet above, is the whole shape:
`fn(&mut State, &EventCtx)`. No closure capture, no message enum, no
reducer. That is why `Button<Counter>` is generic in the state type: the
button's signature says what it is allowed to mutate, and the compiler
checks it.

A non-capturing closure coerces to the same type, so the short form
works wherever the long one does — `examples/counter-window/src/app.rs`
wires four step buttons as `|t, _| t.count -= 10` and so on. Use whichever
reads better; they compile to the same thing.

The case for `fn` over a `Msg` enum — and what it gives up — is
`docs/events-decision.md`.

## What you have not had to do

No `update()` method to write and keep in step with the fields. No
subscription, no observer registration, no dirty flag to set by hand. The
declarations are next to the values they describe, and the frame loop is
what runs them.
