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

## Complex scripts are a crate, not a feature

`lumen-static-text` is separate on purpose. That is the M15 gating
policy's *strongest* mechanism: nothing enters the dependency graph
unless it is named in a manifest. Cargo features are additive and unified
across a build, so a feature would let one dependent's choice become
everyone's — including the embedded targets that cannot afford a shaper.

`docs/text-render-adoption-decision.md` records a different stack being
measured against this one and declined, with reasons.

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
