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
| `lumen-static-text` | its own crate, reached by the shell's `shaping` feature | cluster-correct segmentation, Arabic joining, Indic reordering, bidi runs — **measured and painted** by a `FontStack` in `Shaping::Complex`, which a program chooses with `Program::shaping` |

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

## Text at an angle

A glyph bitmap sits on the pixel grid, so a `Label` cannot turn. A
`VectorText` asks the measurer for its run's **outlines** instead
(`TextMeasure::outline`) and paints them as one path whose transform is
its rotation — on the software canvas and the GPU alike. Its box is the
turned run, so its ink stays inside it at every angle, and hit testing
stays on that box. The showcase turns one run three ways:

```text
{{#include ../../examples/showcase/src/catalog.rs:vectortext}}
```

The outlines come from the measurer, so any measurer can supply them.
This stand-in draws each character as a 10 × 20 px box, and the run
turned on its side is laid out in the turned box:

```rust
# extern crate lumen_static;
# extern crate lumen_static_widgets;
use lumen_static::cx::{PathSink, TextMeasure};
use lumen_static::display::PathEl;
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, LayoutCx, NodeState, Point, Size};
use lumen_static_widgets::VectorText;

/// A measurer whose glyphs are boxes, 10 px a character and 20 px tall.
struct Boxes;

impl TextMeasure for Boxes {
    fn measure(&mut self, text: &str, _size: f32, _max_w: f32) -> Size {
        Size { w: 10.0 * text.chars().count() as f32, h: 20.0 }
    }
    // The run's frame: its top-left at the origin, y down.
    fn outline(&mut self, text: &str, _size: f32, _weight: f32, sink: &mut dyn PathSink) -> bool {
        for k in 0..text.chars().count() {
            let x = 10.0 * k as f32;
            sink.el(PathEl::Move(Point { x: x + 1.0, y: 2.0 }));
            sink.el(PathEl::Line(Point { x: x + 9.0, y: 2.0 }));
            sink.el(PathEl::Line(Point { x: x + 9.0, y: 18.0 }));
            sink.el(PathEl::Close);
        }
        true
    }
}

fn main() {
    let mut run = VectorText::new("Vertical", 90.0);
    let style = [<VectorText as Widget>::type_defaults()];
    let mut st = [NodeState::default()];
    let limits = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 400.0 };
    let mut boxes = Boxes;
    let size = Node::layout(&mut run, &mut LayoutCx::new(&mut boxes), limits, &style, &mut st, 0);
    // Eight characters, 80 x 20, on their side.
    assert_eq!((size.w, size.h), (20.0, 80.0));
    assert!(run.is_vector());
}
```

A measurer with no outlines — the block stand-in, a baked face — answers
`false`, and the widget paints its string level in a box its own size:
visible beats faithful. The window's `FontMetrics` answers from the
font's own outlines only with the window crate's `vector-text` feature,
off by default because the font stack then keeps its faces' bytes; what
that costs, and what an uncached frame of turned text costs on the
software canvas, is in `docs/results-paint-vocabulary.md` (M82/T82.7,
M82/T82.8).

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

**It measures and paints, when a program asks.** Until M81 the shaper
decided only the box and the carets, and the canvas drew the original
string with its own glyph lookup — Arabic measured joined and painted
unjoined. Since M81 a `FontStack` in `Shaping::Complex` shapes in the
one function both renderers place glyphs through, and
`Program::shaping(Shaping::Complex)` chooses it for the whole program:
its window, its measurer, the agent's audit and every headless shot take
the one mode, so a box and its glyphs cannot disagree
(`docs/plan-shaped-paint.md`; `docs/text-render-adoption-decision.md`
says why it is not a glyph command). `Basic`, the default, is what every
program had before.

**Carets and selections follow the shaped run** (M81/T81.5). In
right-to-left text a letter's caret is at its left, and where a line changes
direction one offset has two places on screen — the end of the run that
stops there and the start of what follows — so the shaper reports both,
and a field puts its caret at the second. A press lands on the nearest
leading edge, so a press at the left end of a right-to-left line puts
the caret at the line's logical end. The highlight is the part a
reader would expect, never the letters between two selected parts:

```text
{{#include ../../crates/lumen-static-widgets/src/caret.rs:tint_rule}}
```

Left and Right still step through the text in reading order, not across
the screen (`docs/plan-shaped-paint.md`, "Not in M81"); the gates and
their numbers are in `docs/results-text.md` (M81/T81.5).

**A shaped glyph lands on a third of a pixel** (M81/T81.8). A shaper's
advances are fractional, and rounding each glyph to a whole pixel makes
the gaps in a word uneven, so a `Complex` run's glyphs are drawn at the
nearest third: three bitmaps per glyph at most, derived from the
rasteriser's own 3× horizontal raster, keyed apart in the glyph cache.
Both renderers choose the column and the third in one function:

```text
{{#include ../../crates/lumen-static-shell/src/text.rs:pen_rule}}
```

A `Basic` glyph has no phase, so nothing a program drew before M81 moves.
What the thirds cost a real screen, and how close each is to the glyph
really moved a third, are in `docs/results-text.md` (M81/T81.7, M81/T81.8).

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
