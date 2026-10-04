# Text

Text is where GUI projects spend their years — [What it
costs](./costs.md) quotes the spec on that — so this chapter is mostly
about which layer owns what, and why the seams are where they are.

## Storage: a widget field, generic over the profile

Text lives in a *widget field*, not in a sibling with a getter. The
reason is the one that keeps coming up: **only owned storage can know
when to invalidate.** Mutation goes through the widget, so the widget can
mark its measurement stale. A getter cannot observe that application
state changed, and would have to re-measure every frame.

Storage is generic so each profile can choose:

* `&'static str` — no allocator at all.
* An inline buffer — `no_std` with a bounded maximum.
* `Cow<'static, str>` — desktop, so a literal costs nothing and an owned
  string is available when needed.

## Measurement: a trait, so the stack is swappable

`TextMeasure` is the seam. Everything that lays out text takes one, and
three implementations exist at different points on the cost curve:

| implementation | needs | gives |
|---|---|---|
| `BakedFace` | nothing — a `&'static` table in flash | measures and draws without an allocator |
| `FontMetrics` (`fontdue`) | `alloc`, `std` | real outlines, real metrics |
| `lumen-static-text` | its own crate | **measurement only**: cluster-correct segmentation, Arabic joining, Indic reordering, bidi runs — for the box and the caret; the painter still draws the string unshaped |

`BakedFace` exists because of a gap worth remembering: `FontMetrics` is a
`std` crate living in the shell, so until the baked face was written the
embedded device had **no measurer at all** — and a fixture was laying out
a `Label` whose glyphs nothing could draw.

## Line height: a style field, CSS's rule

`line_height` is a `Style` field, so a stylesheet sets it — `.line_height(1.5)`
on a `Patch`, `line-height: 1.5` in a `.lss`. It is a **ratio** of the font
size, kept as sixteenths in one byte, and `0` (the default) is the face's own
spacing, which is what every tree had before the field existed. The rule is
CSS's and iced's: *n* lines are `n × pitch` tall, a single line included, and
each line's glyphs sit at half-leading inside its pitch. Below the face's own
spacing the box grows by the difference rather than let ascenders leave it,
because nothing clips a run. `Label`, `TextArea` and `RichText` read it; the
sheet lint reports a rule that sets it on anything else. The showcase's
`Label` demo restyles its paragraph from a switch, by making the program's
sheet a function of the switch:

```text
{{#include ../../examples/showcase/src/main.rs:line_height}}
```

What it measures and costs is in `docs/results-text.md` (M80/T80.2).

## Overflow: one line, cut where a reader's character ends

`overflow` is the other text-flow field: `TextOverflow::Wrap` (the default),
`Clip` or `Ellipsis` — `text-overflow: ellipsis` in a `.lss`. `Clip` and
`Ellipsis` make the text **one line, cut in layout**: the widget keeps the
longest prefix whose width, with the `…`'s, fits the width it occupies, and
paints only that. The prefix ends on a grapheme boundary, so a ZWJ family, a
flag or a skin tone is kept whole or dropped whole, and a hard line break ends
the visible line. `Clip` is the same cut with no mark, not a run clipped
mid-glyph: a run that reached past its node would also be damaged past it.
`Label` and `AppBar` read it (a bar is one line, so it reads `Wrap` as
`Ellipsis`); the screen reader still gets the whole text. Without an
allocator a `Label` reads every mode as `Wrap`, because the cut did not fit the
firmware's budget. The same demo's second switch:

```text
{{#include ../../examples/showcase/src/main.rs:text_overflow}}
```

What the cut costs, in shaping and in flash, is in `docs/results-text.md`
and `docs/results-embedded.md` (M80/T80.3).

## Selecting text you cannot edit

A `Label` paints text; a `Selectable` is a label a reader can select and
copy. It takes focus, a press puts the caret, a drag extends the
selection, a double press takes a word and a triple press (or Ctrl+A)
the whole text, and losing focus clears it. The copy is the window's
ordinary Ctrl+C, because the widget answers the same question a text
field does — "what is selected?" — at the one node it shares with its
label. Ctrl+X copies too, and deletes nothing: the text is not the
widget's to change. The showcase's demo opens with its paragraph
selected, and its readout counts what is:

```text
{{#include ../../examples/showcase/src/catalog.rs:selectable}}
```

It selects a `Label`'s one run, so it does not select `RichText` or
`Markdown`, which paint word by word. It needs an allocator (its
cluster list is a `Vec`), so the leaf firmware does not link it; what it
costs, and why its highlight is lighter than a field's, is in
`docs/results-text.md` (M80/T80.4).

## Complex scripts are a crate, not a feature

`lumen-static-text` is separate on purpose. That is the M15 gating
policy's *strongest* mechanism: nothing enters the dependency graph
unless it is named in a manifest. Cargo features are additive and unified
across a build, so a feature would let one dependent's choice become
everyone's — including the embedded targets that cannot afford a shaper.

`docs/text-render-adoption-decision.md` records a different stack being
measured against this one and declined, with reasons.

Since M81 the shell, which paints, reaches the shaper through its own
`shaping` feature, off by default: a build that does not name it compiles
no shaper, and a window turns it on by forwarding
`lumen-static-winit/shaping` (`docs/plan-shaped-paint.md`).

**It measures; it does not paint.** The shaper decides the box a run is
laid out in and where its carets are, and the canvas then draws the
original string with its own glyph lookup — so Arabic is measured joined
and right to left and painted unjoined and left to right. No window can
install it yet either. `docs/plan-capabilities.md` T15.8 records the
correction, and `docs/plan-text-correctness.md` §2.1 proposes shaped
paint as its own milestone.

## Localisation

A `Catalogue` is a sorted `&'static [(&str, &str)]` — the same shape a
baked font uses, and for the same reason. On an embedded target it lives
in flash and costs no RAM; on a desktop it is just as good.

A *loadable* catalogue is a desktop concern and belongs above this layer,
behind `alloc`. Text direction travels with the catalogue, because a
translation and the direction it reads in are not independent choices.

## What you inherit and what you do not

`TextInput` and `TextArea` own selection, IME composition and clipboard
integration — see [Taking input](./input-widgets.md). A custom
text-bearing widget inherits none of it.

That is the honest shape of the cost: the framework gives you
measurement, drawing, storage and a working field. It does not give you a
second working field for free.
