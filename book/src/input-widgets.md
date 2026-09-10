# Taking input

Four widget families take input directly, and they divide on one
question: **where does the value originate?**

| widget | value originates | so the application |
|---|---|---|
| `Toggle` (`CheckBox`, `Radio`, `Switch`) | in application state | binds *into* it |
| `Slider` | at the widget, from a gesture | reads *out of* it |
| `TextInput`, `TextArea` | at the widget, from typing | reads *out of* it |

That is not a stylistic split. A `Toggle`'s `on` is a *copy* of state, so
something must keep the copy in agreement — a `#[bind]` with a `with =`
projection, exactly as `examples/counter-window/src/form.rs` does. A
slider's value has no earlier home; the gesture created it.

## `Toggle` — one type, three presentations

```text
Toggle<S, const SHAPE: u8>
CheckBox<S> = Toggle<S, CHECKBOX>
Radio<S>    = Toggle<S, RADIO>
Switch<S>   = Toggle<S, SWITCH>
```

The shape is a **const parameter, not a field**, and the reason is
mechanical. `kind_name` feeds the cascade and the agent's selectors, and
`defaults()` is snapshotted once when a stylesheet is set. A shape held
in a field would make both state-dependent — the same hazard that had to
be fixed once already (`docs/plan-review-fixes.md` F3.2). As const
parameters, `Toggle<S, CHECKBOX>` and `Toggle<S, SWITCH>` are different
types that answer `kind_name` differently, so `Selector::Kind("radio")`
means something.

## `Slider`

Owns its `value`, and its `step` is public because **a slider that can
only be dragged with a mouse is not an accessible slider** — arrow keys
have to move it, and how far is the application's business.

It also caches `track_w` from the last layout, which looks like
redundancy and is not: `on_event` receives the node's local point and
nothing else — no style array, no state array — so the width a fraction
is computed against has to be remembered from a pass that had it.
`TextInput::pad_left` and `ScrollView::measured_overflow` exist for the
same reason. If you write an interactive widget, expect to need this
pattern.

## `TextInput` and `TextArea`

`TextInput` is one line and owns a great deal:

* `text` — the buffer, generic over `TextBuffer` so `no_std` has a
  choice.
* `caret` — a byte offset, always on a `char` boundary; every path that
  moves it steps by clusters, not bytes.
* `anchor` — the fixed end of a selection, `None` when the caret is a
  point.
* `preedit` — the range an input method is composing. The provisional
  text **is** in the buffer, so it measures, positions and scrolls like
  any other text; what makes it provisional is that the next composition
  event replaces exactly that range.
* `scroll` — horizontal offset so the caret stays visible in a field
  narrower than its content.

`TextArea` is a separate type rather than a `wrap` flag on `TextInput`,
and that reversed an earlier decision deliberately: the plan had reserved
a field for it, and **an unused field is a claim the code has not
earned**. The reasoning is in `docs/text-and-scrolling-decisions.md` B7
and the module's own header.

## What you get for free, and what you do not

Focus, the focus ring, tab order and keyboard activation are the
framework's, not each widget's — the ring is a `Style` property so a
stylesheet sets it once and every focusable widget gets one, including
custom widgets that implement nothing.

Selection, IME composition and clipboard are `TextInput`'s and
`TextArea`'s. If you write your own text-bearing widget you inherit none
of it, which is the honest reason [What it costs](./costs.md) says the
years in a GUI project go to text.
