# Open questions: text input and scrolling

*Raised 2026-09-01, unanswered. These two features share one problem, so
they share one document: both need a child to be larger than the box it
is shown in, and the framework currently guarantees that cannot happen.*

**Status: every question below is OPEN.** Recommendations are mine, with
the reasoning that produced them; none is settled until it is answered
here and the marker flips.

---

## Why these two are one decision

`crates/lumen-static/src/hit.rs:3-6` states the invariant hit testing
rests on:

> Because containers place children sequentially by measured size and
> **children never escape their parents**, at most one child can claim
> the point.

That property does three jobs. It lets hit testing stop at the first
claiming child. It is why audit codes `W1003` (overflow) and `W1004`
(sibling overlap) are marked *structurally impossible* rather than
unimplemented, with a guard test in `tests/lint.rs` that fails if it ever
stops holding. And `W1005` (truncation) reads "a `Shrink` node whose
parent gave it less than it asked for" — which inside a scrolling region
is not a bug but the normal case.

A scrollable region is a child escaping its parent. So is a single-line
text field whose content is wider than the field. Neither is a widget
task; both are changes to a documented invariant that three subsystems
read.

---

## Cluster A — the containment invariant

### A1. Scoped, virtualized, or abandoned?

**Options.**

- **(a) Abandon it.** Children may be placed anywhere; hit testing tests
  every child and takes the topmost. Costs the early exit, and makes
  `W1003`/`W1004` real lint work rather than free guarantees.
- **(b) Scope it.** The invariant becomes *children never escape their
  nearest clipping ancestor*. A clip node is a barrier: if the point is
  outside it, the whole subtree is out; inside it, children are still
  placed sequentially and at most one claims the point.
- **(c) Virtualize instead of clipping.** Materialize only what is
  visible, as `Repeat` already does — nothing ever escapes.

**Recommendation: (b), with (c) retained as the specialization.**

Scoping preserves everything the invariant was actually buying. Early
exit survives unchanged: children placed at a negative scroll offset keep
their sequential, non-overlapping relative order, so two of them still
cannot both contain a point, and a child scrolled out of view cannot
contain a point that is inside the viewport. The rule is re-anchored, not
weakened.

(c) alone is not sufficient — it works for long homogeneous lists, which
is exactly what `Repeat` is for, but not for a text field scrolling
through one continuous run, nor for a scroll region holding arbitrary
widgets.

(a) gives up a real property to buy generality nothing has asked for.

### A2. Where does "this node clips" live?

**Options.** A `NodeState.flags` bit; a field on the widget; a `Style`
property.

**Recommendation: `Style.overflow: Overflow { Visible, Clip }`.**

Precedent, and it is not the precedent the plan predicted. M4 T4.1
specified `NodeState.flags: HIDDEN`; the implementation put hiding in
`Style.visible` instead, because M1's style array had landed in the
meantime and visibility is a styleable property. That deviation is
undocumented (see "Consequences" below) but it was the right call, and
clipping is the same kind of property — CSS calls it `overflow` for the
same reason.

It also gets the ergonomics free. `Node::defaults()` already exists, so a
`ScrollView` returns `Overflow::Clip` from it, and a stylesheet can still
override. Paint and hit both already carry `&[Style]`, so neither needs a
new parameter.

### A3. Where does the scroll offset live?

**Recommendation: a widget field, `#[reload(keep)]`.**

Consistent with `Repeat.scroll`, which the hot-reload demo already
asserts survives a swap (`scroll=240`). It is user state, not
presentation and not styling.

The traversal does not need to see it. A scroller places its content at a
negative offset during `place`, so the offset is already expressed in the
children's `pos`; paint and hit need only the clip flag from A2. This is
the smallest change that works and it adds nothing to `NodeState`.

### A4. What happens to the guard test and the reserved lint codes?

**Recommendation: re-pin the guard test, do not delete it; activate
`W1003`; make `W1005` clip-aware; leave `W1004` reserved.**

- The guard test in `tests/lint.rs` must be rewritten to the scoped
  invariant, not removed. Deleting a guard test when its invariant
  changes is how an invariant rots silently; rewriting it is how the
  change gets noticed next time.
- `W1003` becomes implementable and *more useful* than originally
  specified: content exceeding its box is expected inside a clip scope
  and a bug outside one. That is a real diagnostic rather than a
  structural impossibility.
- `W1005` currently fires on any `Shrink` node given less than it asked
  for. Inside a scroller that is the normal case and the lint would
  scream on every scrolling view. It must learn about clip scopes at the
  same time, not afterwards — the `plan-agent-parity.md` P3 note about
  Lumen's contrast analyzer sitting finished-but-unreachable applies
  exactly here, in reverse.
- `W1004` stays reserved: sequential placement still holds within a clip
  scope, so overlap remains impossible until absolute/stack layout.

---

## Cluster B — text input

### B1. Who owns the editing buffer?

**Recommendation: the widget. One allocation, grown once, reused.**

Considered settled enough to build on unless someone objects, and
recorded here because it *looks* like it contradicts
`layout-and-storage-decisions.md` §1.

It does not. §1's rule was never "application state owns text" — it was
*whoever can invalidate the cache must own the storage*. A `Label`'s data
originates in app state, so the widget holds a copy (`#[bind]`). A text
field's data originates at the widget, because the user typed it there.
Same rule, opposite direction.

The deciding argument is not allocation — T6.2's `set_from_str` already
made the copy path allocation-free, so "buffer then copy" and "own it
outright" are both zero-alloc in steady state. It is that **caret and
selection geometry are by-products of shaping**. Split the text into app
state and the shaped cache stays in the widget with no way to know the
text changed — the getter design §1 rejected, arriving through a side
door. And an editing buffer is not a string: it is text plus caret plus
selection anchor plus IME preedit, and separating the text from the
indices into it invites desynchronization.

### B2. What does the shaped cache store, and does `Label` share it?

**Options.** Extend `Measured` with advances; a separate cache type for
editable/selectable text; one type generic over whether advances are
kept.

**Recommendation: a separate `Shaped`, leaving `Measured` alone.**

`Measured` is one `Cell<Option<(Constraints, Size)>>`. Every label in a
100k-node tree carries one. Adding an advance array would put an
allocation in every label to serve the few that need caret placement. A
separate cache keeps the cheap case cheap, which is the same shape as
`Repeat`'s opt-in arena.

### B3. How are edits applied — `&mut String`, or methods?

**Recommendation: methods only (`insert_str`, `remove_range`), never a
raw `&mut` to the storage.**

Free to adopt now, expensive to retrofit. Undo, IME preedit and
change-notification all need a seam where edits are observed. Exposing
`&mut String` forecloses all three; a method boundary costs nothing today
and keeps them possible. This is the one recommendation here that is
cheap now and irreversible later, so it is the one worth taking even if
undo is never built.

### B4. How do caret and selection reach the screen?

**Recommendation: ordinary `Cmd::Rect`. No new display-list command.**

A caret is a one-pixel rect; a selection is one rect per line. The
display list stays minimal and the software renderer needs no change,
which keeps it usable as the headless test oracle.

Caret *blink* is animation and is deferred — it would be the first thing
in the framework needing a frame clock, which is a separate decision.

### B5. Where does "scroll the caret into view" happen?

**Recommendation: in the widget's `layout`, after measuring.**

Not in the event handler, which runs before layout and has no geometry to
clamp against — it does not yet know how wide the field is or where the
caret landed. This is the one piece of text-input behaviour that is a
layout-time side effect rather than an event-time one, and getting it
wrong produces a caret that is correct one frame late.

### B6. Is IME in the first cut?

**Recommendation: no — but reserve the field, and say so plainly.**

Preedit is `Option<(String, Range<usize>)>` overlaying the buffer; it
affects the shaped cache, since preedit text is displayed but not
committed. Designing the buffer around it now costs nothing and prevents
a rewrite.

State the consequence honestly in the widget's own documentation rather
than letting it be discovered: **without IME this is not usable for
Chinese, Japanese or Korean input.** "Text input" that silently means
"Latin text input" is the kind of claim that erodes trust in every other
claim the project makes.

### B7. Multi-line in the first cut?

**Recommendation: no. One widget type, with the field reserved.**

Topology is identical — one node either way — so the static-tree
preference for types over flags does not apply. Selection across line
boundaries is where the real complexity is; single-line first, `wrap:
bool` reserved.

### B8. How does the application read the value?

**Recommendation: directly — `ui.field.as_str()`. Defer `#[bind_from]`.**

A reverse binding (widget → state) is the symmetric feature and would be
easy to add, but nothing needs it yet. It earns its place when a real
case appears — validation, or commit-on-Enter where app state should hold
the committed value while the widget holds the in-progress edit. Building
it speculatively means guessing at semantics that a use case would have
told us.

Note that the dangerous direction is already a compile error, and should
stay one: `#[bind(ui.field, "{}")]` pointing app state *into* an input
would overwrite the buffer every frame and destroy the caret mid-
keystroke. `bind::format_into` takes `&mut Label<S>`, so it simply does
not accept a text input. Do not add a convenience impl.

---

## Consequences to handle in whichever commit settles A1

1. `hit.rs`'s module comment states the global invariant and must be
   rewritten to the scoped one.
2. The guard test in `tests/lint.rs` — rewrite, do not delete (A4).
3. `docs/plan-architecture-v2.md` T2.6 re-pinned the containment
   invariant under the `Length` model; it needs an amendment.
4. `audit.rs`'s `_OVERFLOW_RESERVED` / `_OVERLAP_RESERVED` doc comments
   name the trigger as "a scrollable content region" — that trigger fires
   here.
5. **Unrelated, found while writing this:** M4 T4.1 specified
   `NodeState.flags: HIDDEN`; the code uses `Style.visible`. The plan
   should record the deviation and its reason (M1's style array landed
   first, and visibility is styleable).

## Not questions

- **`NODE_COUNT` and `DEPTH` are unaffected.** A scroll viewport is one
  node with one child subtree; both stay compile-time constants.
- **The event vocabulary is ready.** `Event::Text { ch }` and
  `Key::{Backspace, Delete, Left, Right, Home, End}` shipped with E0, and
  the agent's `input.type` already enqueues `Text` events, so a
  widget-owned buffer is drivable by an agent on day one.
- **Clipping primitives exist.** `Cmd::PushClip`/`PopClip` are in the
  display list and the software renderer honours them.
