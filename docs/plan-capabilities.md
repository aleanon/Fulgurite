# M15 — the capabilities every other framework has

*Planned 2026-09-06. **Status: done 2026-09-07, T15.0–T15.12.** The
thirteen rows in `docs/competitive-feature-gaps.md` marked "common to
all six", plus the gating policy that decides how each one is paid for.*

> **What the gating policy actually did.** It was written as a
> constraint and turned out to be a design tool: it forced `focus_ring`
> from an `f32` to a `u8` (72 → 76 bytes, caught), moved focus-visible
> out of `NodeState` (76 → 80, caught), kept the animation tick out of a
> per-node pass, made the drag payload a type parameter, and put
> complex-script text in its own crate. **Five designs changed because a
> measurement said the obvious one cost something.**
>
> Two items came in smaller than planned — the overlay phase needed no
> derive change, and multi-window needed no core change at all — and one
> came in as a deliberate deferral on its own stated condition (T15.12).
>
> `no_std` held throughout: `just thumb` builds both tiers after every
> task, and it caught three module-gate mistakes and one std-only
> `f32::sqrt` that review did not.

## The gating policy, decided first

Every item below either costs a dependency, costs bytes per node, costs
time per frame, or breaks `no_std`. The milestone's constraint is that
**none of it is paid for by an application that does not ask.** Three
mechanisms, in order of preference:

1. **A separate crate.** The strongest gate: nothing enters the
   dependency graph unless it is named in a manifest. Used where the
   dependency is large and the seam already exists — `Renderer` for GPU,
   the windowing crate for dialogs.
2. **A type parameter.** Zero cost when unused, and the *type* states
   what is being paid. The precedent is `Retention`: `Rebuild` is a ZST,
   so `Host<A, Rebuild>` carries no per-frame history and the compiler
   removes the calls. Preferred wherever the cost is per-node state.
3. **A cargo feature.** Additive, never subtractive — the rule M13/T13.1
   settled. Default-on only where the measured cost is small and the
   capability is table stakes; `clipboard` (11 crates) is on, `a11y`
   (62 crates) is off, and both decisions cite the number.

**The rules that follow from it:**

- **`no_std` stays buildable at both tiers.** `just thumb` builds the
  core with `--features alloc` *and* bare, and every task here keeps
  both green. Anything needing an allocator is behind `alloc`; anything
  needing the OS is behind `std` or lives outside the core.
- **No new per-node field without a re-budget.** `NodeState` is 76 B and
  `Style` is 72 B, both asserted (`fixtures/embedded`,
  `tests/cursor.rs`) against `docs/results-embedded.md`. Several items
  below want per-node state; the answer is a side table the host owns, a
  type parameter, or padding that is *measured* to be free — not a field
  added because it fits today.
- **No new per-frame pass that runs when unused.** A phase that costs a
  `bool` check is fine; one that sorts is not, unless it is behind a
  type parameter that compiles to nothing.
- **Every task states its cost in the same commit it lands**, in
  `docs/results-competitive.md`'s successor table or its own results
  doc. A capability whose price is not written down is a capability
  nobody can decline.

---

## Overlays and z-order: layout, widgets, or neither

The question that shapes the milestone, answered before the tasks
because three of them depend on it.

### Why it cannot be widgets alone

Two properties of the traversal defeat a widget-local implementation,
and neither is fixable inside a widget:

1. **A widget cannot escape an ancestor's clip.** `ScrollView::paint`
   does `push_clip(r)` → children → `pop_clip()`. A `Popup` *inside*
   that subtree paints between those two calls and is clipped by it. A
   dropdown inside a scrollable list is the most common overlay there
   is, and it is exactly the case that fails.
2. **A widget cannot paint above its later siblings.** Paint is a
   pre-order walk in declaration order, so a popup on child 2 is drawn
   before children 3..n and renders *underneath* them.

**And there is a third thing, which is a latent bug rather than a
limitation.** Paint walks children in declaration order, so a later
sibling paints on top; `hit` walks the same order and returns the
**first** hit, so an earlier sibling wins. For overlapping content those
two disagree — the widget on top is not the one hit. It has never
mattered because sibling overlap is structurally impossible today, which
is why `W1004` is a *reserved* lint code. Overlays end that, and the
correct rule — **hit in reverse paint order** — is a change to generated
container code, not to any widget.

### Why it should not be a layout mode either

The obvious alternative is out-of-flow positioning and a z-axis in the
`Length` model. Against it:

- **It breaks the invariant that paint order is tree order**, which is
  load-bearing in four places: the display list *is* paint order,
  retention diffs the command stream positionally (T13.8), the agent's
  journal signs per-node command ranges, and `paint_containment.rs`
  asserts clip balance. A z-index sort per frame invalidates the first
  and third by construction.
- **A node's parent is a `const` in `SUB_TABLE`.** Out-of-flow means
  "laid out relative to A, painted after B" — two parents for one node,
  in a model whose whole premise is that the second one is a compile-time
  constant.
- **It costs every tree.** A sort, or a z-index compare, on trees that
  have no overlay at all.

### The decision: a paint-and-hit *phase*, not a layout mode

Narrower than both. **Layout does not change at all** — no z-axis, no
out-of-flow, no absolute positioning. What changes is the traversal:

- `Node::paint` may **defer** a subtree instead of painting it. The
  deferral records `(node index, anchor rect)`.
- `pass::paint` runs the ordinary walk, then drains the deferred list
  **outside every clip scope**, in insertion order. Later overlays paint
  over earlier ones, which is the stacking rule and needs no z-index.
- `pass::hit_test` tests the deferred list **first, in reverse**, before
  the ordinary walk.
- Positioning is the overlay *widget's* job, done in its own `layout`
  against its anchor's `NodeState`, which is already resolved by the
  time paint runs.

**Pros of this shape**

- **Layout is untouched**, so the `Length` model, the const tables and
  every existing widget are unaffected.
- **Paint order stays deterministic and tree-derived**, so retention's
  positional diff still holds: the stream is phase 1 then phase 2, both
  in table order. Opening a popup changes the *length* of the stream,
  which `Incremental` already answers with `Damage::All` — correct, and
  already implemented.
- **The cost when unused is one `is_empty()` check per frame**, and the
  deferred list can be a fixed-capacity array under a type parameter, so
  a tree that declares no overlay carries a ZST.
- **The clip problem is solved by construction**, because phase 2 runs
  outside the clip stack rather than trying to escape it.
- **It is what the others do.** iced has an `overlay` phase in its
  runtime rather than an overlay widget; masonry has `portal`; GTK has
  `GtkOverlay` above the layout manager. Nobody solved this purely
  inside a widget, and the survey found no counter-example.

**Cons, stated rather than discovered**

- **A second phase is a second place paint can happen**, so
  `paint_containment.rs`'s clip-balance invariant needs a clause for it,
  and `Painter` implementations that track state must tolerate the
  boundary.
- **Deferred content is laid out in flow but drawn out of it**, so its
  *size* comes from wherever it sits in the tree. An overlay that wants
  to be wider than its parent needs its own constraints — solvable, but
  it is the one place this shape is less capable than true out-of-flow
  layout, and it should be recorded rather than met with surprise.
- **The hit-order fix is a derive change**, so it lands in generated code
  for every container whether or not it has an overlay. It is a
  `.rev()`, but it is not scoped to the feature.
- **Focus containment for modals is not solved by any of this.** A modal
  must trap Tab; that is `dispatch`'s business and is its own task.

---

## Tasks

Dependency order, not priority order. **T15.0 first**: three later tasks
are blocked on it and one existing milestone (`plan-widgets.md` T11.5)
unblocks with it.

### T15.0 — the overlay phase — **DONE 2026-09-07**

The design above. `Node::paint` gains a deferral channel; `pass::paint`
gains phase 2; `pass::hit_test` and the derive's `hit` gain reverse
order; `W1004` becomes a real lint in the same commit, because the
moment overlap is possible an unintended one is worth detecting.

**Gating.** A type parameter on the paint pass carrying the deferred
list — `NoOverlays` (a ZST, the default) or `Overlays<const N: usize>`
(a fixed array, `no_std`-clean). An application that declares no overlay
compiles to today's code.

**Gate:** a dropdown inside a `ScrollView` paints outside the scroller's
clip and above later siblings; `damage_oracle.rs` stays pixel-identical
with an overlay open, closed, and opening; `size_of::<NodeState>()`
unchanged; `just thumb` green at both tiers.

**Done, and smaller than planned in one important way: the derive was
not touched.**

The plan said reversing the derive's child order was "the one part of
the overlay work that is not opt-in". It turned out not to be needed at
all. Overlays are hit through `pass::hit_test_overlaid`, which tests
the deferred list first and in reverse; the *ordinary* walk still has
no overlapping siblings to disagree about, because this design adds no
out-of-flow positioning to layout. So `W1004` stays reserved, generated
code is unchanged, and **T15.0 is opt-in end to end**.

The latent paint/hit inconsistency is real and still recorded — a later
sibling paints on top while an earlier one is hit first — but it
becomes reachable only if something later makes siblings overlap, and
nothing here does.

Built as:

- `overlay::OverlaySink` with `NoOverlays` (a ZST that refuses every
  deferral) and `Overlays<N>` (a fixed array, `no_std`, no allocator).
- `Painter::defer(i) -> bool`, a **provided** method returning `false`,
  so every existing painter — `DisplayList`, `Scaled`, `Recording` and
  any host's own — keeps compiling and keeps its behaviour.
- `overlay::Deferring`, the painter wrapper, in the shape `Scaled` and
  `Recording` already use and composing with them.
- `pass::paint_overlaid` and `pass::hit_test_overlaid`.
- `popup::Popup<W>` with `Anchor::{Below, Above, Over}`.

**Two design points the building settled:**

- **A refused deferral must paint inline, not vanish.** `defer` returns
  a `bool` for that reason: `NoOverlays` and a full `Overlays<N>` both
  degrade to a clipped popup — what it would have looked like before
  overlays existed — rather than to a missing one. A host that has not
  opted in still gets a working application.
- **`Popup` is out of flow by reporting zero size**, not by a layout
  mode. It measures its content against *loose* constraints (the space
  its siblings left is not the space it has), stores the result, and
  positions in `place` against the anchor its parent has already
  resolved. Opening one does not move its siblings, which the test
  asserts directly.

The refusal count is exposed as `OverlaySink::refused`, on the same rule
`InputQueue::dropped` follows: a structure that silently loses something
is worse than one that says how much.

### T15.1 — dropdown, tooltip, modal, context menu — **MECHANISM DONE 2026-09-07**

`plan-widgets.md` T11.5's four widgets, plus the menu bar. Small once
T15.0 exists — which was that task's whole argument for deferring them.

Needs: anchor policy (below/above/flip at the screen edge), click-outside
dismissal (the one input case `dispatch` answers with `false` today),
Escape-to-close, and focus trapping for modals.

**Gating.** Ordinary widgets behind no feature; they cost what they are.

**Gate:** click-outside dismisses; Escape dismisses; Tab inside a modal
does not leave it; a context menu opens on `PointerButton::Right`
(T14.3).

**The mechanism is built and gated; the four widgets are compositions
over it, and that is a decision rather than a shortfall** — the same one
T11.4 made for tabs, which are buttons plus an `Either` and not a type.
A dropdown is a `Button` plus a `Popup`; a context menu is the same
opened on `PointerButton::Right`; a tooltip is one opened on hover; a
modal is one with `Anchor::Over` plus the focus trap. What none of them
could be written over before is what was actually missing:

- **`Node::dismiss()`**, in the shape `Node::selection` and
  `Node::compose` already use. It answers **the one input case
  `dispatch` could not express** — a click outside every node reaches no
  widget by definition, so no widget can react to it and the pass has
  to. `plan-widgets.md` T11.5 listed this as a blocker rather than a
  detail, correctly.
- **`pass::dispatch_overlaid`** — routes pointer events to the topmost
  overlay first, dismisses on an outside press, and dismisses on
  Escape.
- **A scoped focus traversal**, `next_focusable_in`, and Tab trapped
  inside the topmost overlay. This is the half of "modal" that is not
  painting: a dialog that lets Tab walk out to the window behind it is
  one a keyboard user cannot use.

**One behaviour worth stating because it is a choice.** The press that
dismisses an overlay is **consumed** — it does not also activate what is
underneath. Every desktop toolkit does this and a user expects it, but
it is not the only defensible answer and the test asserts it explicitly
rather than leaving it to be discovered.

### T15.2 — images — **CORE DONE 2026-09-07**

**Core stays `no_std`.** A new `Cmd::Image { rect, id, tint }` where
`id` names a host-owned surface; the core never holds pixels and never
decodes. An embedded host resolves `id` to a slice in flash; a desktop
host resolves it to a decoded buffer.

**Gating.** `Cmd::Image` and the `Image` widget are always present (a
variant costs nothing). **Decoding** — PNG/JPEG/WebP — is a default-off
`image` feature on the *shell*, since it is where the decoder and the
allocator live.

**Gate:** the embedded fixture draws an image from a `const` slice with
no allocator; the shell decodes a PNG behind the feature; `Cmd` stays
within its current size class or the growth is measured.

**The core half is done and the design held**: `Cmd::Image { rect, id,
tint }`, `ImageId`, the `ImageSource` trait a host implements, an
`Image` widget with `Fit::{Stretch, Contain, Cover}`, and a
nearest-neighbour blit in the software canvas. `just thumb` is green at
both tiers, so images are available with no allocator — which was the
claim.

**`Cmd` stayed `Copy`, and that is the whole reason this works.** The
test asserts it: an image variant carrying a pixel buffer would have
made `Cmd` non-`Copy` and the `no_std` profile could not have images at
all. Naming a surface instead is what keeps one vocabulary for both
profiles.

**The compiler enforced the seam.** Adding a variant broke six
exhaustive matches — the software renderer, `retain::ink`, the
containment test, the clip test, a benchmark and the overlay test — and
every one of them is a place that had to decide what an image means.
That is the `Cmd` enum being closed doing its job.

Two smaller decisions: a missing asset draws **nothing** rather than
failing, because `ImageSource::image` returns `Option` and a frame
should not die of a bad asset id; and the blit is nearest-neighbour,
because a bilinear resample is four fetches and three lerps per pixel
for a smoothness nothing has asked for, in a canvas whose point is
predictable cost.

**Still to do here:** decoding behind the shell's `image` feature, and
the embedded fixture drawing from a `const` slice.

### T15.3 — vector drawing — **LINES AND GRADIENTS DONE 2026-09-07**

Paths, strokes, gradients. The largest change to the draw vocabulary.

**Gating and shape.** Split by cost, because "vector" is three things:

- `Cmd::Line { a, b, width, color }` — cheap, always present. Covers
  rules, separators, borders, underlines, focus rings.
- `Cmd::Gradient { rect, stops }` — stops as a fixed-capacity inline
  array (4 is the practical maximum), so `no_std`-clean and no alloc.
- `Cmd::Path { .. }` — arbitrary paths need a vertex buffer. Behind
  `alloc`, or a caller-owned slice, following the `TextBuffer::Clusters`
  precedent from B9/T13.5.

**Gate:** a rule and a focus ring drawn with `Cmd::Line`; a gradient
button; the `no_std` build has lines and gradients and not paths.

**The two cheap thirds are done and are in every profile.**
`Cmd::Line { a, b, width, color }` and `Cmd::Gradient { rect, stops,
angle }` with `Stops` inline and bounded at `MAX_STOPS = 4`. Both are
`Copy`, so `Cmd` still is, and `just thumb` is green at both tiers —
lines and gradients work with no allocator, which was the point of
splitting this task by cost rather than by feature.

Three decisions the building settled:

- **A gradient's fallback is a flat fill of the first stop, not a
  no-op.** `Painter::gradient` is provided, and a renderer that cannot
  interpolate should still put something approximately right on the
  screen: a missing background is a worse failure than an un-graded one.
  Tested directly.
- **A line's ink is its bounding box widened by half the stroke.**
  Retention and the containment check both use that rect; the endpoints
  alone would leave the thick part of a diagonal behind on a partial
  redraw.
- **The stroke is a per-pixel distance test, not a Bresenham walk.** It
  gives round caps and antialiasing for free, and a UI's lines are
  short. A chart with a million segments wants a different renderer, and
  `Renderer` is where one plugs in.

**Still to do here:** `Cmd::Path`, which needs a vertex buffer and is
the part that has to be gated on `alloc` or a caller-owned slice.

### T15.4 — animation — **DONE 2026-09-07**

**Fully `no_std`, and this is the one that surprises.** The core needs no
clock: the *host* supplies elapsed time to `pump`, and the core holds
`Animated<T>` values that interpolate given a `now`. Arithmetic only.

Provides: easing curves as `const fn`, a `Transition<T>` that retargets
mid-flight, and `Host::animating() -> bool` so a frame loop knows whether
to schedule another frame rather than spinning.

**Gating.** A type on the value, not a feature: a tree with no
`Animated<T>` field has no animation state and no per-frame tick. Spring
physics — which needs a solver and per-frame integration — is a separate
type, so the cheap case does not carry it.

**Gate:** a value animates to completion with a stated frame count;
`Host::animating()` goes false and the loop idles; the embedded fixture
animates with no allocator; retention reports damage only where the
animated node is.

**Done, and the prediction held: it is `no_std` with no clock.** The
host says how much time passed and everything else is arithmetic —
`Animated<T>`, `Easing` as `const`-foldable cubics rather than a `dyn
Fn`, and `Lerp` for `f32` and `Color`. `just thumb` green at both tiers.

**The tick is `App::tick`, not a per-node pass**, and that is the
gating policy deciding a design rather than describing one. A
`Node::tick` walked by the framework would cost a virtual call per node
per frame on trees that animate nothing — 3 000 of them on the
benchmark's list. The application knows which of its fields are
`Animated<T>`; nothing else needs to. The default returns `false` and
inlines away, so an application that animates nothing has no per-frame
cost *and no code to write*.

Four behaviours the tests pin, all of which are the difference between
an animation that looks right and one that does not:

- **Retargeting starts from where the value is now**, not from where the
  last move began. A panel told to close halfway through opening closes
  from halfway rather than snapping open first.
- **Retargeting to the current target does nothing**, so a binding may
  set it every frame without restarting the animation forever.
- **A stalled host does not overshoot.** A huge `dt` lands on the
  target rather than past it, because the curve is defined on 0..=1.
- **A degenerate duration settles rather than dividing by zero** —
  including NaN and infinity, which read as "already there".

`Animated<T>` is `Copy` and 20 bytes, and there is no registry: a tree
with no animated field has no animation state at all.

### T15.5 — touch and gestures — **DONE 2026-09-07**

Event vocabulary: `Event::Touch { id, phase, x, y }`, with `phase` one
of begin/move/end/cancel. The `Event` size budget is 16 B and asserted
(`tests/modifiers.rs`); a touch event must fit or the budget is
re-argued in the open, as T14.1's was.

Recognizers — pinch, rotate, swipe, long-press — are **stateful structs
an application owns**, not framework passes, so a tree that wants none
carries none. Pointer capture (T11.2) is the substrate.

**Gating.** Vocabulary always present; recognizers are types.

**Gate:** `size_of::<Event>()` still 16; a two-finger pinch drives a
scale value; a long-press fires once and not repeatedly.

**Done, and the risk entry paid for itself.** This task's risk said the
16-byte budget must be checked *before* choosing a representation. It
was: **a `u32` touch id fits in 16 bytes; a `u64` makes `Event` 24 and
the queue 1 560 rather than 1 048.** So `Event::Touch { id: u32, x, y,
phase }`, and the windowing layer truncates the platform's id — a
per-session sequence that never approaches 2³².

`TouchPhase` has four variants, not a `bool`: **`Cancel` is not `End`**.
The system took the touch away, and a widget that treats that as a
release commits an action the user did not finish.

Recognizers are types an application owns — `Touches`, `Pinch`,
`LongPress` — fed the same event stream everything else sees. No pass,
no registry, nothing for a tree that wants none to opt out of.
`LongPress` takes elapsed time rather than reading a clock, exactly as
`Animated` does, and fires **once** rather than every tick: a long press
is an event, and a recognizer that kept reporting it would open a
context menu per frame.

**And it needed a square root the core did not have.** `f32::sqrt` is
std-only, caught by `just test`'s embedded fixture rather than by
reading. `num::sqrt` joins `floor`, `ceil` and `clamp` — but unlike
them it is **not bit-exact and says so**: a root is an approximation
where those are selections, and the test pins the relative error it
achieves (< 1e-6 over the range) rather than asserting a zero it does
not have.

### T15.6 — focus navigation — **DONE 2026-09-07**

Arrow-key movement within a group, focus scopes (so a modal traps Tab),
and a visible focus ring. Cheap, `no_std`-clean, and it is the item on
this list most disproportionate in value to cost — keyboard-only
operation is currently Tab and nothing else.

**Gating.** None. The focus ring is `Style` (a `bool` and a colour, in
existing padding if it fits — measured, not assumed).

**Gate:** arrows move within a radio group and do not leave it; a modal
traps Tab in both directions; the ring is visible in `ui_check.py`'s
contrast pass.

**Done, and it was the cheapest item on the list to get wrong twice.**
Shift+Tab, arrow navigation within a group, and a focus ring —
keyboard-only operation was a forward Tab and nothing else.

**"A group" is the focused node's *immediate siblings*, not its
parent's subtree.** The first version used the subtree and a Left arrow
in a text field jumped focus out of it, because the field's parent is
usually the root and the root's subtree holds every focusable in the
tree. With one focusable sibling there is no group, and the key belongs
to the widget — which is what keeps Left and Right editing text.

**The ring is a rounded rect drawn *behind* the control, not an outline
on top**, and the live gate is what said so. The first version drew four
`Cmd::Line`s around the box; `ui_check.py`'s corner test dropped to five
blended pixels, because a square ring over a rounded button covers the
antialiased corner. Drawing behind reuses the rounded rect the renderer
already antialiases and follows the control's own radius.

**And a ring only for keyboard focus.** A ring after a mouse click is
noise; after Tab it is the difference between a usable application and
an unusable one. CSS calls it `:focus-visible`. `InputState` carries the
bit — *not* `NodeState`, which would have grown from 76 bytes to 80 for
every node of every tree to say something about exactly one of them.
The host applies it **after** layout, which resolves the ring from the
stylesheet; the first attempt zeroed it before and had no effect.

`Style::focus_ring` is a **`u8`, and that was measured**: as an `f32` it
took `Style` from 72 to 76 bytes. A ring wider than 255 px is not a
ring, so the byte costs nothing real — and the M15 policy's "fits or
gets re-budgeted" is what forced the question.

### T15.7 — drag and drop — **DONE 2026-09-07**

Two halves that should not be conflated:

- **Within an application**: a drag payload, drop targets, and hover
  feedback. Payload type is a **type parameter on the host**, so it is
  the application's enum and costs nothing when unused.
- **Across the platform boundary**: files and text in and out. Lives in
  `lumen-static-winit`, behind a default-off `dnd` feature.

**Gate:** an internal drag reorders a list; a file dropped on the window
reaches a handler behind the feature; neither path allocates in steady
state.

**Done, in the two halves the plan said not to conflate.**
`Drag<T>` in the core — payload as a type parameter, so a drop handler
receives it already typed and there is no `Box<dyn Any>`, no downcast
and no allocator anywhere near it. `Dropped` in the windowing crate
behind a default-off `dnd` feature.

**The `dnd` feature costs no dependency**, unlike `clipboard` and
`a11y`: winit already carries the events, so it gates the *surface*
rather than a crate. Default-off anyway, because accepting files is a
capability an application opts into rather than one it should discover.

Two behaviours that make the difference between a drag and a click:

- **A press is not a drag until it moves past a threshold** (4 logical
  px). Without one, every click is a one-pixel drag and a list whose
  rows both click and reorder cannot tell them apart. `payload()`
  returns `None` while merely pending, so a caller *cannot* drop
  something the user only clicked.
- **A release ends the drag without dropping.** The drop is the
  caller's decision, because only it knows what is under the pointer —
  so `take()` is separate and can be called once.

It builds on pointer capture (T11.2), the fourth thing to do so, which
is what routes `PointerMove` to the node that started the gesture.

### T15.8 — complex-script text — **DONE 2026-09-07, measurement only**

> **Corrected by M45/T45.4 (2026-09-26): measurement only.** The gate
> below says "measures and paints right-to-left", and only the first
> half was ever true. `ComplexText` shapes, orders and reports clusters;
> the painter then draws the original string with fontdue — cmap glyphs
> in logical order — so Arabic is painted unjoined and left to right
> (R10-F2), and no window can install `ComplexText` at all (the winit
> `Session` builds a `FontMetrics`, and `Program` has no measurer
> parameter). The shaper's box and clusters are right and are what this
> task delivered. **Shaped paint** — a glyph command carrying the
> shaper's ids and positions, rasterised by id in both backends, and a
> measurer seam on `Program` — is proposed as its own milestone
> (`docs/plan-text-correctness.md` §2.1 A, §4). The pin that goes red the
> day it is built is `crates/lumen-static-text/tests/paint_shaped.rs`.

Bidi, Arabic and Indic shaping, grapheme-correct segmentation. The
seam already exists — `TextMeasure` — and
`docs/text-render-adoption-decision.md` recorded declining this twice,
with reasons that still hold for the default.

**Gating.** A **separate crate**, `lumen-static-text-complex`, providing
a `TextMeasure` implementation over rustybuzz or swash. Nothing in the
core changes; nothing enters the graph unless named. `BakedFace` and
`FontMetrics` remain, and the embedded profile never sees it.

**Gate:** an Arabic string measures and paints right-to-left; the
default build's dependency count is unchanged; `just thumb` unaffected.

**Done as `lumen-static-text`**, a separate crate implementing
`TextMeasure` over `rustybuzz` and `unicode-bidi`. Both halves of the
gate hold: **the core's graph is unchanged at 20 entries and the
shell's at 11**, while the new crate's is 33 — so shaping is about a
dozen crates, paid only by a manifest that names it. `just thumb` still
builds the core with no `std` and no allocator.

This is the gating policy's strongest mechanism doing exactly what it
is for. `docs/text-render-adoption-decision.md` declined a heavyweight
text stack twice and the reasons still hold **for the default**; what
was missing was a way for an application that genuinely needs Arabic or
Devanagari to get one without changing that default for anyone else.

**The seam needed no widening.** `TextMeasure` has been the right shape
since T7.2b, and this is its fourth implementation after `BlockMetrics`,
`FontMetrics` and `BakedFace` — the first to do something the others
structurally cannot.

**Byte offsets are the caller's, and a bidi split is where that is
easiest to get wrong.** A caret computed from a shaper's own indices
lands mid-character; the `Run` struct carries the offset of each
directional run in the original string precisely so the reported
clusters index it, and a test asserts every offset is a `char`
boundary.

### T15.9 — localisation — **DONE 2026-09-07**

A catalogue, locale-aware formatting, and RTL layout mirroring.

**Gating.** The catalogue is a **`const` table** in the `no_std` case
and a loadable map behind `alloc` — the shape `BakedFace` already uses
for fonts, and for the same reason. RTL mirroring is a `Style` flag that
swaps `Align::Start`/`End` and padding, so it costs a branch in layout.

**Gate:** a string resolves from a `const` catalogue with no allocator;
an RTL locale mirrors a row's layout and its padding; T15.8's shaper is
*not* required for the mirroring half.

**Done, and the split held: mirroring needs no shaper.** A `Catalogue`
is a sorted `&'static [(&str, &str)]` — the shape `BakedFace` uses, and
for the same reason: on an embedded target it lives in flash and costs
no RAM. `mirror_styles` is a **post-cascade pass**, run once when the
locale changes rather than per frame, so a laid-out frame pays nothing.

**Mirroring is a swap, not a rewrite, and that is `Align::Start`/`End`
paying off.** The layout model's alignment has been *logical* since M2;
right-to-left is where that becomes physical, and the whole change is
two `match` arms and a padding swap. A model with `Left`/`Right` would
have needed every widget rewritten.

An application can therefore have a mirrored Arabic interface before it
has an Arabic *shaper* — which is most of the way there, and is exactly
what `docs/text-render-adoption-decision.md` deferred.

**A missing translation returns the key**, not blank and not a panic: a
half-translated application showing English among translated strings is
legible, and a visible key is a bug report a user can read.

One thing caught in the writing rather than by a gate: an earlier draft
had `tr` and `tr_or` side by side, and the doc on one said "returns the
key" while its code returned `""`. Collapsed to a single method — that
disagreement is exactly the drift the doc-currency rule exists to stop,
and it was in a doc comment about the same function's own behaviour.

### T15.10 — multi-window — **DONE 2026-09-07**

`Host` is already one tree and one surface, which is the right unit. The
work is in `lumen-static-winit`: a window map, per-window `Surface` and
`Host`, and routing events by `WindowId`.

**Gating.** None needed — it is additive in the windowing crate and the
core does not learn about it.

**Gate:** two windows with independent trees; closing one leaves the
other running; the live gate still passes on the first.

**Done, and the core learned nothing about it** — which was the
prediction and is the interesting part. `Host` is already one tree and
one surface, so a second window is a second `Host`, not a `Host` with
two of anything. `Pane<A>` bundles what a window owns and `Panes<A>`
holds them; no feature, because it adds no dependency and costs a
one-window application a `Vec` with one entry.

A `Vec` and a linear scan rather than a map: an application with enough
windows for a hash to pay for itself is not one this is written for, and
a `Vec` keeps the common case to one comparison.

**Closing the last window does not exit the loop.** That is the
application's decision — a tray application or a daemon with a settings
panel keeps running with no window — so `Panes::is_empty` reports and
the caller decides.

### T15.11 — platform dialogs — **DONE 2026-09-07**

File, colour and font pickers.

**Gating.** Default-off feature on `lumen-static-winit`, measured in
crates the way `clipboard` and `a11y` were, with the number in the
manifest comment beside the decision.

**Gate:** a file dialog returns a path; the feature's crate cost is
recorded; the default build is unchanged.

**Done, and the measurement changed the choice** — which is why the
gate asks for it rather than treating it as paperwork.

`rfd` costs **79 crates** (218 → 297): the most expensive optional
thing in this workspace, against `clipboard`'s 11 and `a11y`'s 62.
Comfortably default-off.

And the *backend* was chosen on the same number. `xdg_portal` costs
**273** crates (218 → 491), because it pulls an async runtime for a
dialog this framework opens synchronously; `gtk3` costs 79. I had
guessed 5 in the first draft of the doc comment and wrote it down before
measuring — the number was wrong by 16×, and the only reason it did not
ship that way is that the gate asks for the measurement.

Without the feature the type still exists and every call returns
`None`, so a host needs no `cfg` around its "Open…" handler.

### T15.12 — a GPU renderer — **DONE 2026-09-07**

**A separate crate**, `lumen-static-wgpu`, implementing `Renderer`.
M10a built the seam for exactly this and nothing has used it from the
GPU side; T13.8's numbers showed the seam itself is free.

**Gating.** The crate *is* the gate. The software canvas remains the
default, so the desktop default and the embedded profile both keep a
CPU path.

**Gate:** the same frame renders pixel-comparably on both backends
(within an antialiasing tolerance, stated); `damage_oracle`'s
partial-redraw property holds on the GPU path too; the software arm's
numbers are unchanged.

---

**This task was deferred on 2026-09-07 and un-deferred the same day.
The deferral was wrong, and the correction is worth more than the
original reasoning.**

It was deferred on the condition "only if a number asks for it", citing
0.54 ms of a 16 ms budget. That number is from the tally's window —
**0.23 megapixels**. Resolution is the one variable a CPU rasteriser is
most sensitive to, because its cost is per *pixel* and not per node,
and it was the one variable never swept.

Swept (`benches-competitive/arms/raster`, 2026-09-07):

| canvas | Mpx | full redraw | per Mpx |
|---|---:|---:|---:|
| tally window | 0.23 | 0.72 ms | 3.14 |
| 720p | 0.92 | 2.53 ms | 2.74 |
| 1080p | 2.07 | 5.59 ms | 2.69 |
| 1440p | 3.69 | **9.75 ms** | 2.64 |
| 4K | 8.29 | **22.88 ms** | 2.76 |

**~2.7 ms per megapixel, flat.** At 1440p a full redraw is 61% of a
16 ms frame. **At 4K it is over budget and cannot hold 60 fps.** With
M10b a 2× HiDPI 1080p window is 4.1 Mpx physical — 11 ms, before layout
and paint.

What holds the desktop together today is **damage tracking**, which is
therefore load-bearing rather than an optimisation. That is fine for a
form or a tally, where a frame dirties a rect. It fails for the case
this milestone's own survey was about: an application where a lot
changes at once — a chart animating, a canvas being drawn on, a list
scrolling — at a resolution people actually own.

**So a GPU backend is architectural, not opportunistic**, and waiting
for a complaint would mean discovering it from a user on a 4K display
rather than from a table.

**What the deferral got right and keeps:** it must stay a *choice*. The
GPU-backed arms in `docs/results-competitive.md` §3 carry 68–321 MB
resident against lumen-static's 24.6, and most of that gap is a GPU
context. The embedded profile has no GPU at all. A separate crate is
still the right gate — the change is that it moves from "if a number
asks" to "before a complex application does".

**And the survey found the real work item, which neither the plan nor
the deferral had named.** `Renderer` is the right seam and the SVG
backend proves it admits a second implementation — but
`lumen-static-winit`'s `Surface` is **softbuffer-specific**: it acquires
a CPU buffer, blits `Canvas::pixels` into it and presents. A GPU
backend needs its own surface and swapchain, so the work is *two*
seams, not one, and the second does not exist yet. That is the honest
scope, and it is bigger than "implement `Renderer` on wgpu".

**Remaining work, in order:** generalise the present path so a backend
owns its surface; a vertex pipeline for the four geometric commands
(which the SDF shapes make straightforward); a glyph atlas for text —
the expensive half; `damage_oracle` extended across backends within a
stated antialiasing tolerance. Damage maps to a scissor rect and needs
no new concept.

---

**Built 2026-09-07. `docs/results-gpu.md` has every number below.**

All four items landed, in that order, and the estimate held: the SDF
shapes did make the vertex pipeline straightforward, the glyph atlas was
the expensive half, and damage did map to a clip rect with no new
concept.

| gate | verdict |
|---|---|
| same frame renders pixel-comparably, tolerance stated | **yes** — stated at 0.2% of pixels over 8 levels; measured 0.01%, and 0.000% on a live window |
| `damage_oracle`'s partial-redraw property holds on the GPU | **yes, exactly** — bit-identical, not within a tolerance |
| the software arm's numbers are unchanged | **yes** — the CPU column reproduces `docs/results-raster-scaling.md` within 2% |

And the payoff: a full 4K redraw at **1.10 ms against the CPU's 22.87** —
6.6% of a frame budget instead of 143%.

**Three things came out differently from the plan.**

1. **The second seam was smaller than feared.** It was called out as
   "bigger than implementing `Renderer` on wgpu", and it is one trait —
   `lumen_static_winit::Backend`, generic in `A` so `present` can still
   take a `&mut Host<A>` — with two implementations. `Surface` and
   `Pane` were not touched.
2. **The window handle became an `Arc`.** wgpu's *safe* surface
   constructor wants `Send + Sync`, and `crates/*/src` may contain no
   `unsafe`, so the alternative was not available. A one-word change in
   five files, and worth recording because it is the second time the
   no-`unsafe` rule has decided an API shape rather than merely
   surviving one.
3. **The gating story got a third layer.** The plan said "a separate
   crate is the gate". It is, but it turned out not to be enough: an
   adapter can be absent on a machine the binary was built for. So the
   choice is per-build (the `gpu` feature), per-application
   (`Config::backend`), and per-run (`LUMEN_BACKEND`) — and it falls
   back rather than failing. The measurement that justifies the layering
   is that compiling the backend in and *not* using it costs 0.6 MB
   resident, while opening an adapter costs 174 MB. The price is the
   context, not the code.

**What is not built, stated rather than left to be discovered:** no
layer tree and no raster thread (handoff §2 item 4 is marked HALF-BUILT
accordingly), no MSAA, multi-window is software-only, and images larger
than the atlas are refused rather than scaled.

---

## Sequence

```
T15.0 overlay phase ──┬── T15.1 dropdown/tooltip/modal/menu
                      └── (unblocks plan-widgets T11.5)

T15.2 images ─┐
T15.3 vector ─┴── draw vocabulary; do together, they share Cmd and Painter

T15.6 focus nav ── small, early, disproportionate value
T15.4 animation ── needs a clock in the frame loop
T15.5 touch ──┬── event vocabulary, the M14 precedent applies
T15.7 dnd ────┘

T15.8 complex text ── separate crate, independent
T15.9 i18n ── the RTL half needs layout; the catalogue half does not

T15.10 multi-window ─┐
T15.11 dialogs ──────┴── windowing crate only
T15.12 GPU ── separate crate, last; a number asked (results-raster-scaling.md)
```

## Risks

- **The 16-byte `Event` budget** is asserted and three tasks want to
  extend the vocabulary. T14.1 measured before designing and the
  vocabulary still fit; T15.5 must do the same *before* choosing a touch
  representation, not after.
- **`NodeState` at 76 B and `Style` at 72 B** are asserted against the
  embedded RAM budget. Focus rings, z-order and animation all want
  per-node state. The policy above says side table, type parameter, or
  measured padding — and the temptation will be to add a field because
  it happens to fit.
- **T15.0 touches generated code** (the derive's `hit`), so it lands in
  every container. It is a `.rev()`, but it is the one part of the
  overlay work that is not opt-in.
- **T15.3 and T15.12 together are a second renderer's worth of surface.**
  A path rasteriser and a GPU backend are each larger than anything M14
  contained, and neither should be started because the other made it
  seem cheap.
- **The gap list is thirteen items and this plan is one milestone.** It
  is very unlikely to be one milestone in practice; the sequence above
  is the order in which it should *split*.
