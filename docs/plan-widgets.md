# M11 — the widget vocabulary

*Planned 2026-09-04. Was Phase B of `docs/plan-lumen-parity.md`; that
document is superseded as a plan and this replaces its widget section.
Status: **T11.1–T11.4 done 2026-09-04; T11.5 deliberately not started.**
Gated on M10a (the `paint` signature), not on the allocation answer —
the review corrected that reasoning and the milestone order with it.*

## What is already built

Phase B's list was written when this project had four widgets. It now
has eleven, and three of Phase B's items are among them:

| Phase B item | status |
|---|---|
| text input | `TextInput` (M9) — no IME, no selection, single-line |
| scrollable | `ScrollView` (M8) — with chaining |
| §6 escape hatches as first-class widgets | `Either`, `Option`, `List`, `Dyn`, `Repeat`, `Assembled` |
| checkbox / radio / switch | `Toggle<S, SHAPE>` (T11.1) — shape is a const, not a field |
| slider | `Slider` (T11.2), with pointer capture |
| text area | `TextArea<S>` (T11.3), a separate type — B7 revised |
| container / card | *not needed* — any derived container with a background is one |
| tabs | a composition, not a type (T11.4) — buttons + `Either` |
| dropdown / select | **not built** — blocked, see T11.5 |
| tooltip | **not built** — blocked, see T11.5 |
| modal | **not built** — blocked, see T11.5 |

The interesting result of that table is that **the remaining work is not
uniform**. Three items are cheap, one needs an event-vocabulary change,
one reopens a closed decision, and three need a layout capability that
does not exist. Treating them as one list of nine widgets is what the
original phase did, and it is why the phase was never actionable.

## The four groups, in order

### T11.1 — toggles: checkbox, radio, switch — **DONE 2026-09-04**

One toggle core with three presentations. Each is a `Button` with a
`bool`, a different `paint`, and an AccessKit role
(`CheckBox`/`RadioButton`/`Switch`) with `set_toggled`.

Most of what they need exists: `NodeState::pressed` and `hovered` (C6),
focus and Tab order (C5), and `set_hidden`/roles in `observe`. These are
the first widgets to use `pressed` for something other than a shade.

**`Key::Space` was added here**, which is what the task was actually
missing: every desktop toolkit toggles a checkbox with Space, and the
enum had no variant for it. That is an event-vocabulary gap of the same kind T11.2
names for `PointerMove` — a shell mapping, an enum variant, and a line
in `dispatch`'s activation match. It is small, but it must be named
rather than substituted: an earlier draft of this plan quietly gated on
`input.key Enter` instead, and `pass.rs` carried a comment claiming
Space already worked (corrected 2026-09-04).

**Radio needs a group.** A radio button's state is "am I the selected
one", which is not local. The honest shapes are (a) the *app* holds the
selection and each radio is bound to it with `#[bind]`, or (b) a
`RadioGroup<N>` container owning an index. **(a) is the one that fits
this architecture** — it is exactly what `#[bind]` exists for, it needs
no new container, and it keeps the selection in application state where
a reader can find it.

**Its cost, which an earlier draft did not state.** `#[bind]` targets a
single field and its `with =` form is `fn(&mut Target, &Source)`;
nothing in the derive fans one binding out across N radios by index. So
(a) needs **one hand-written `with` function per option**, each
hard-coding its own value to compare against, and that scales with the
number of options. (b) would compute `i == self.selected` once — which
is exactly how `Repeat` routes N rows through one template function
using `ctx.row`, the codebase's own idiom for the shape. Decide it in
the task with that on the table; (a) is still the recommendation, but
not a free one.

**Gate — MET.** Eight tests, and three findings worth keeping:

- **Shape is a const type parameter, not a field.** `kind_name` feeds
  the cascade and the agent's selectors, and `defaults` is snapshotted
  once when a stylesheet is set — a shape in a field would make both
  state-dependent, which is the hazard `Option::defaults` was fixed for
  (F3.2).
- **The widget does not flip itself.** `on` is a presentation copy and
  the application owns the boolean, exactly as a `Label`'s text is a
  copy. A widget that flipped itself would consume the event — C3's
  contract is that `true` stops the walk — so the app's handler would
  never run and the two copies could disagree.
- **A default must be visible in both states.** The first version
  inverted `fg`/`bg` for the on state, and `Style::background` defaults
  to *transparent* while `rounded_rect` drops a transparent command — so
  an "on" switch drew no knob at all. Caught by the knob-travel test,
  which measured the body and called it the knob. Both colours are
  opaque now, and a test pins that nothing vanishes in either state.

And one consequence of the vocabulary: **a checkmark is a staircase.**
`Cmd` has no path and no rotation, so the tick is small squares stepped
along two diagonals. It reads correctly at 24 px, and it is the same
missing vocabulary an icon needs.

### T11.2 — slider — **DONE 2026-09-04**

A slider is the first widget that needs **pointer drag**, which M9
deliberately does not route: `PointerMove` is unrouted because it fires
continuously and its only consumer is the hover bit (C6, recorded in
`pass::dispatch`).

So T11.2 is two tasks wearing one name:

1. **Route `PointerMove` to a node that asks for it.** Not to everything
   — the reason it is unrouted is that walking the ancestor chain on
   every mouse motion looks for a handler that almost never exists. The
   shape that keeps the objection answered: a node that has captured the
   pointer (set on `PointerDown`, cleared on `PointerUp`) receives moves
   directly, and nothing else is walked. That is one `Option<u32>` on
   the host beside `focus`, and it is also what a drag needs to keep
   receiving events after the pointer leaves the control.
2. **The slider itself**, once (1) exists.

**Measured, not predicted** (`docs/results-capture-cost.md`): a motion
frame is **0.033 ms p50 against a click frame's 0.092**. Capture does
not add an ancestor walk — it removes the need for one — and frame cost
is dominated by damage size, not by dispatch.

**The part that was easy to get wrong**, and which the tests caught:
the capture check must run **before** the hit test. Once the pointer
leaves every node, hit testing finds nothing and the pointer arm returns
early — so a capture checked afterwards never fires in the one case it
exists for. The first version did exactly that and the slider stopped
following the pointer the moment it left the track.

**Gate — MET.** Six tests, including the one that matters: a drag
5 000 px off the control still tracks, and a move with nothing captured
is not routed at all. Arrow keys, Home and End step it, because a
control only draggable by a mouse is not accessible. The value is
clamped with NaN mapped to zero — a non-finite value would reach
`NodeState` through the knob's rect, where every `contains` is false and
the control silently stops being clickable.

**Who owns the value:** the slider does. Unlike a toggle's `on`, it is
produced by a gesture rather than mirrored from application state — the
same reason a text field owns its text (B1), and the application reads
`value()` (B8). The rule is identical in both and only looks opposite:
whoever can invalidate the derived state owns the storage.

### T11.3 — text area — **DONE 2026-09-04**

`docs/text-and-scrolling-decisions.md` B7 decided **no multi-line in the
first cut**, and M9 shipped `TextInput` with no reserved field for it —
deliberately, because an unused field is a claim the code has not
earned.

What already exists in its favour: the shaping seam reports
`Cluster::line` and `TextMeasure::line_height`, so wrapped-run geometry
is answerable today, and `tests/shaping.rs` covers wrapping. What does
not: caret movement across lines (up/down is not a byte step, it is a
geometric query), and vertical scrolling of the run.

**It must stay generic over `TextBuffer`, like `TextInput`.** Multi-line
caret geometry and vertical scrolling are exactly what tempts a hidden
`Vec<usize>` of line-start offsets, and that would put a second
allocating widget in the vocabulary. Line bookkeeping rides on
`Cluster::line`, which the shaping seam already reports.

**Recommendation: build it as a distinct `TextArea` rather than a flag
on `TextInput`.** A `multiline: bool` puts two caret models in one type
and every method grows a branch; the storage and the edit methods are
shared through `TextBuffer` regardless. B7 should be re-recorded as
"multi-line is a separate widget", not "multi-line later".

**Gate — MET.** Six tests. Two are worth naming:

- **Enter behaves oppositely in the two widgets** — inserted here,
  declined by `TextInput` so a form's submit handler still sees it. That
  single contrast is the clearest argument they are two types.
- **Up and down decline when the measurer reports no lines.**
  `BlockMetrics` reports no clusters by design (T7.2b), so there are no
  lines to move between and the widget says so rather than moving the
  caret somewhere arbitrary. A second test supplies a wrapping stand-in
  that *does* report lines, and the caret then moves geometrically.

### T11.4 — tabs — **DONE 2026-09-04, and it needed no new widget**

**The finding: tabs are a *composition*, not a primitive.** Two buttons
and an `Either`, wired by the application's handlers — which is what
"topology in the type" is for. Adding a `Tabs<A, B>` widget would have
meant a third merged const table and a header strip nobody could style
independently, to express something the derive already expresses.

T8.0 is what made it worth doing rather than merely possible: branch
nodes are ordinary table nodes now, so a tab's content is addressable by
selector, styleable by `#id` and visible to the audit. None of that was
true before 2026-09-04.

It landed in the settings panel rather than as a type, which is also the
honest place for it — a pattern is demonstrated, not exported.

A header strip plus `Either` (or an enum-derived tree for more than two
tabs). T8.0 put branch nodes on the table, so a tab's content is
addressable by selector, styleable by `#id`, and visible to the audit —
none of which was true before 2026-09-04.

**Gate — MET** by the panel: a tab strip sits above the controls, its
buttons are ordinary focusable nodes, and the three properties this task
would have had to prove were already true and already tested elsewhere —
`emit_table` marks zero-area nodes hidden, and C5 drops focus that
leaves the screen.

### T11.5 — overlays: dropdown, tooltip, modal — *needs a layout capability*

These three need **overlay and z-order**, which the layout model has no
concept of. Containers place children sequentially by measured size,
which is precisely why `W1004` (sibling overlap) is a *reserved* lint
code whose comment reads: "Structurally impossible while containers
place children sequentially by measured size; ships with the first
layout mode that can produce it (absolute/stack)."

**That reserved code is the marker for this work.** Whoever builds
overlays makes `W1004` real, in the same commit, because the moment
overlap is possible the lint that detects an unintended one becomes
worth having.

What has to be decided first, and none of it is widget code:

- **A stack/absolute layout mode.** The `Length` model (M2) has no
  z-axis and no out-of-flow positioning.
- **Paint order versus tree order.** An overlay must paint last and hit
  first. Today both follow the table's pre-order, and the display list
  *is* paint order — so an overlay either moves in the table or the
  paint pass gains a second phase. This interacts with M8b: the damage
  diff compares command lists positionally, and a second phase changes
  what "the same index" means.
- **Dismissal.** A dropdown closes on click-outside, which is an event
  that reaches *no* node — the one input case `dispatch` currently
  answers with `false`.

**Recommendation, and the decision: T11.5 is not started.** It is a
layout milestone with three widgets on the end of it, and estimating it
as "three widgets" is how it would get underestimated. Building the
other four confirmed the split rather than blurring it — none of them
needed overlap, z-order or an out-of-flow position, so nothing about
this work argues for taking the layout change now.

**Two of those three prerequisites landed in M15** (`docs/plan-capabilities.md`
T15.0/T15.1), which is worth recording because it was not framed as
paying this debt at the time:

| prerequisite | status |
|---|---|
| a stack/absolute layout mode | **still missing** — T11.6 below |
| paint order versus tree order | **done**: the overlay phase, `Painter::defer`, `pass::paint_overlaid` |
| dismissal | **done**: `pass.rs` closes the topmost overlay on Escape and all of them on a press outside, consuming that press |

So the three widgets themselves are close to free now — a dropdown, a
tooltip and a modal are each `Popup<W>` with an anchor. What is left of
T11.5 is T11.6, and the widgets are a day after it.

### T11.6 — the `stack` axis — **DONE 2026-09-10**

**Added 2026-09-10.** `docs/plan-glass.md` needs content *underneath* a
panel and no stock container produces overlapping rects, which is what
turned T11.5's first prerequisite from a deferred nicety into a
dependency.

**What it is.** A third container mode beside `row` and `column`:

```rust
#[widget(stack)]
struct Screen { content: ScrollView<Rows>, bar: Glass<Toolbar> }
```

Children share the container's rect. `layout` measures each against the
same constraints and takes the max; `place` puts every child at the same
origin, offset by its own `Style.align` on **both** axes rather than
just the cross one; `paint` walks forward, so the last child is on top.

**`Axis` is the wrong place for it, and that is the interesting part.**
Every helper on that enum — `main`, `cross`, `size`, and the two
`Length` accessors — presupposes that one of width/height is the main
axis. A stack has none, and a third variant would put a meaningless arm
in five methods. The derive branches on the declared mode already; this
is a third branch in codegen, not a third value in `Axis`. `spacing` is
meaningless here and `#[widget(stack, spacing = ..)]` should be a
compile error rather than a silently ignored attribute.

**`hit` must walk children in reverse.** Paint order is bottom-to-top,
so the topmost child is the last painted and must be the first hit.
Every other container can walk forward because its children cannot
overlap; this one cannot.

**`W1004` becomes real in the same commit.** It is reserved in
`crates/lumen-static-agent/src/audit.rs` as `_OVERLAP_RESERVED`, its
reason is pinned by `crates/lumen-static-agent/tests/lint.rs`, and
`docs/text-and-scrolling-decisions.md` says to leave it reserved because
"sequential placement still holds within a clip". This is the layout
mode that stops that being true, and T11.5 above already committed
whoever builds overlap to making the lint real at the same time.

**The lint's whole difficulty is that a stack's children overlap on
purpose.** A code that fires on every stack is noise, and noise is how
an audit code stops being read. It must fire on overlap *between
siblings of a sequential container* — where it means a layout bug — and
stay silent inside a stack. That is a scoping question, not a geometry
question, and it is the part to get right.

**Why not the alternatives**, since both were considered:

- *A `pinned: bool` on `Popup` that opts out of dismissal.* Smaller, and
  wrong: it keeps popup semantics — out of flow, zero size, overlay
  phase, anchor-relative placement — on a thing that is not a popup, and
  a persistent bar built that way still swallows the click that would
  have dismissed it if the flag is ever wrong.
- *Letting a glass panel read across siblings.* Does not work at all. A
  `column` allocates non-overlapping rects, so a toolbar above a
  scroller has the page behind it and nothing to blur.

**Shown to fail:**

| test | injection | proves |
|---|---|---|
| children share the rect | place children sequentially | the stack is not a column |
| hit returns the topmost | walk children forward | z-order is real, not incidental |
| `W1004` fires on unintended overlap | remove the overlap from the fixture | the code is not vacuous |
| `W1004` is silent inside a stack | drop the scoping | the code is not noise |

The last two are the pair `AGENT.md` requires: a gate that refuses bad
input must be checked to accept good input in the same session, or the
next tightening turns it into one that refuses everything.

**Sizing.** A day, plus the lint. It unblocks T11.5's three widgets and
M19's motivating scene, and nothing else in the queue depends on it.

#### What it turned out to be

Every design note above survived contact. Three things the plan did not
say:

- **`record_painted` is the template's, not each body's.** The three
  mode-dependent bodies (`layout`, `place`, `hit`) are spliced into one
  shared impl, and a node's own box is recorded once after them. The
  first version had the stack body record it too, which produced two
  calls in sequence and an unparsable expansion — caught by dumping the
  token stream, not by reading it.
- **`paint` is not one of the three.** A stack paints its children in
  field order exactly as a row does, and that order *is* the z-order.
  Only measuring, placing and hitting change.
- **A stack keeps the container default of `FILL` width, `Shrink`
  height**, shared with row and column. Two tests asserted the largest
  child on both axes and were wrong: the stack was viewport-wide because
  its own style said `FILL`. The convention is pinned by
  `a_stack_defaults_to_fill_width_like_every_other_container` so a
  change to it is deliberate.

**Shown to fail** — five injections, each caught by name:

| injection | result |
|---|---|
| place a stack's children sequentially | `a stack's children must occupy the same rect`, 5 of 9 fail |
| walk `hit` forward | `hit must return the topmost child (node 3), not node 2`, 1 of 9 |
| drop the `W1004` stack exemption | `a stack must be exempt: ["W1004"]` |
| never report an overlap | `expected W1004: []` **and** the quiet test's own control |
| `>=` instead of `>` on the overlap test | `a shared edge is not an overlap: ["W1004"]` |

The fourth is why the quiet test carries its own control. "No `W1004`"
is also what a lint that never ran says — and **the first version of
`stack_overlap.rs` said exactly that**: its helper built a fresh `Agent`
to dispatch `ui.lint`, so both quiet tests passed with the audit never
having run. The firing case in the same file is what exposed it. A quiet
assertion needs a firing one beside it, in the same test, or it is
checking nothing.

`spacing` on a stack is a compile error rather than a silently ignored
attribute, pinned by `tests/ui/fail_stack_spacing.rs`.

## Selection and IME

Both belong to the text widgets and **neither is blocked on widget
code**:

- **Selection** needs modifier state on `Event::Key` (for Shift+Arrow)
  or pointer drag (T11.2's capture). Once either exists, selection is
  an anchor beside the caret and a `Cmd::Rect` behind the run — B4
  already decided the rectangle.
- **IME** needs a preedit event in the vocabulary and a composition seam
  in `TextBuffer`. B6 decided it out of the first cut and named it; it
  is not deferred silently.

Sequence them after T11.2, since pointer capture unblocks the cheaper
half of selection.

## One example, growing — the gate behind every gate

Every task gate above is agent-protocol-shaped: `ui.lint` is clean, the
role appears in `ui.getTree`, `ui_check.py press` passes. That is a real
risk the review named: a widget can satisfy all of it without anyone
having put it on a screen next to another widget, which is the fastest
route to a vocabulary that is individually correct and **collectively
unusable**. There are two examples in this repository and both are the
same tally.

So each task additionally lands in one growing example — a settings
panel: toggles, then a slider, then tabs, then a text area — and the
task is not done until that panel still looks right and
`scripts/live_gate.sh` still passes on it. The panel is the artifact
that would notice spacing that only works in isolation, a focus ring
that collides with a neighbour, or four controls that each chose a
different padding.

## What M11 is not

- **Not "Lumen's 17.5k lines of widgets".** That number is a proxy for a
  whole library including its own animation, theming and platform
  affordances. The list above is what an application here has actually
  wanted, plus the three it will want next.
- **Not an icon or image widget.** There is no display-list command for
  one — `Cmd` is `Rect`, `Text`, `RoundedRect`, `Shadow`, `PushClip`,
  `PopClip` — so unlike a progress bar (composable today from styled
  containers) an icon cannot be built from the vocabulary at all. It is
  a vocabulary change of the same class as overlays, it is unscheduled,
  and it is recorded in `docs/plan-architecture-v2.md`'s Phase D
  paragraph rather than silently omitted here.
- **Not a styling milestone.** Widgets get `Style` fields as they need
  them; a `.lss` parser (parity Phase C) remains unscheduled and
  unjustified — nothing has yet been hard to express in Rust rules.
