# Decisions: text input and scrolling

*Raised 2026-09-01, **answered 2026-09-02**. These two features share one
problem, so they share one document: both need a child to be larger than
the box it is shown in.*

**Status, 2026-09-04: all twenty-three questions DECIDED**, one of them
REVISED (A2). The eleven raised by the 2026-09-02 review (A5–A7, B9–B11,
C1–C5) were settled on 2026-09-04 by adopting each section's recorded
recommendation, unchanged; where a recommendation offered two shapes the
chosen one is named in the heading (A7: forbid and lint; C3: option ii).
The 2026-09-02 reviews found that two rationales here were false and
that the routing layer both halves depend on does not exist yet. The
decisions themselves survived; what changed is stated at each one.

The framing that opened this document ("the framework currently
guarantees that cannot happen") turned out to be **false**, which is what
made A1 easy to answer: `Repeat` had been violating the invariant since
Step 3.

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

**DECIDED: (b), with (c) retained as the specialization.**

Scoping preserves everything the invariant was actually buying. Early
exit survives unchanged: children placed at a negative scroll offset keep
their sequential, non-overlapping relative order, so two of them still
cannot both contain a point, and a child scrolled out of view cannot
contain a point that is inside the viewport. The rule is re-anchored, not
weakened.

(a) gives up a real property to buy generality nothing has asked for.

**(c) is not an alternative, because it is not doing the job today.**
Measured, not assumed — `crates/lumen-static/tests/scroll_containment.rs`:

| `Repeat`, 24px rows, 100px viewport | escapes above | escapes below | clip cmds |
|---|---|---|---|
| `scroll = 0` (on an item boundary) | 0px | **44px** | **0** |
| `scroll = 12` (half an item) | **12px** | 32px | **0** |

`Repeat::place` puts row 0 at `first * extent - scroll` (`repeat.rs:175`),
which is negative for any scroll off an item boundary, and
`visible_range` deliberately materializes one row past the viewport
(`repeat.rs:87`) because a viewport is rarely an exact multiple of the
item extent. Neither is a bug in virtualization — both are *required* by
it. `paint` then emits no clip at all: `repeat.rs` contains zero
occurrences of "clip".

So the invariant in `hit.rs` is **already false**, and has been since
Step 3. It went unnoticed because the guard test in
`lumen-static-agent/tests/lint.rs` walks `SUB_TABLE`, and a repeater's
rows live in an off-table arena — the one mechanism that breaks the
invariant is invisible to the test written to catch it.

Virtualization decides *how many* children exist; it has nothing to say
about where the ones at the boundary are allowed to draw. Those are
orthogonal, which is why (c) cannot substitute for (b):

- **(b) scoping is a correctness mechanism** — it bounds where a subtree
  may draw and be hit. Every scrolling construct needs it, including
  virtualized ones.
- **(c) virtualization is a cost mechanism** — it bounds work per frame
  regardless of item count. Only uniform homogeneous lists can have it.

Three things (c) cannot do at all, independent of the above. It cannot
handle a scroll region of heterogeneous static children, because in a
static tree instantiation is not optional — fields exist because the type
says so, and there is nothing to leave unmaterialized. It cannot handle
continuous content with no item boundary, such as a text field scrolling
horizontally through one run. And it cannot handle non-uniform item
extents without first measuring everything, which is the cost it exists
to avoid.

**Live defect, independent of A1.** Hit testing already gates on the
repeater's own rect before examining rows (`repeat.rs:188`), so it
behaves as though clipped — while paint draws rows up to 44px outside.
Hit and paint disagree, which is precisely the class of bug P4 fixed for
probe-vs-click. Answering A1 fixes it; until then a scrolled list paints
over its neighbours.

### A2. Where does "this node clips" live?

**Options.** A `NodeState.flags` bit; a field on the widget; a `Style`
property.

**DECIDED, then REVISED 2026-09-02:** `Style.overflow: Overflow {
Visible, Clip }` supplied by `Node::defaults()` — **but not cascadeable**
(no `Patch` field), **and stamped into `NodeState.flags` by `place`**, so
that `paint` and `hit` read it from the array they already carry.

The first version of this answer rested on a false statement: *"Paint
and hit both already carry `&[Style]`, so neither needs a new
parameter."* They do not — only `layout` and `place` do
(`widget.rs:52,55`). As written, A2 was a `Node` signature change of M0's
class: every leaf, all four hatches, `Repeat`, `Assembled`, the derive,
the shell and the journal. Review also showed the cited precedent does
not transfer: `Style.visible` works without paint or hit reading style
because hiding has a *geometric proxy* — layout zero-sizes the subtree
and `contains` fails. Clipping has no proxy; a clip scope's box is
exactly the size a non-clipping one's would be.

The revision keeps what A2 was for and drops what it got wrong:

- `Overflow` lives in `Style` so `defaults()` supplies it and a
  `ScrollView` declares itself. That ergonomic survives.
- It has **no `Patch` field**, so a stylesheet cannot set it. Review's
  concrete scenario: `Rule::new(Selector::Any, Patch::new().overflow(Visible))`
  would re-create the exact defect A1 was written to fix — the derive's
  `hit` still gates on the node rect while paint stops clipping — after
  the code was fixed to prevent it. The test for whether a property may
  cascade is *whether a stylesheet can break an invariant another
  subsystem assumes*. `visible`: no. `overflow`: `hit.rs`, `W1003` and
  `W1005` all read it. If a scroller must ever be un-clipped, that is a
  widget-API decision.
- `place` already has `&[Style]` (`widget.rs:48`) and already writes
  `NodeState`; it stamps a `CLIP` flag. Paint and hit read the flag. No
  signature changes. This is the option the first version dismissed on
  the false premise.

### A3. Where does the scroll offset live?

**DECIDED: a widget field, `#[reload(keep)]`.**

Consistent with `Repeat.scroll`, which the hot-reload demo already
asserts survives a swap (`scroll=240`). It is user state, not
presentation and not styling.

The traversal does not need to see it. A scroller places its content at a
negative offset during `place`, so the offset is already expressed in the
children's `pos`; paint and hit need only the clip flag from A2. This is
the smallest change that works and it adds nothing to `NodeState`.

### A4. What happens to the guard test and the reserved lint codes?

**DECIDED: re-pin the guard test, do not delete it; activate
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

### A5. What constraints does a scroller give its content? — **DECIDED 2026-09-04, recommendation adopted**, **BUILT 2026-09-04**

**What shipping it taught.** The recommendation said "the viewport as
the reference for `Fill` and an unbounded *maximum* for
`Shrink`/`Fixed`", and the first implementation tried to satisfy both
with one number — the viewport, unchanged — on the theory that
`Length::Shrink` returns content size and so content would overflow
anyway. It does not: `resolve_self` ends with `limits.clamp(...)`, so a
finite maximum squeezes the content and there is nothing to scroll. The
two numbers are genuinely two numbers. `Constraints` did not grow a
field (69 struct literals would have had to change); the reference goes
in `LayoutCx`, which exists for exactly this — a service layout needs
that a widget cannot own — as `with_fill_span`/`fill_extent`. An
unbounded `Fill` at the content *root* falls back to its content size in
`resolve_self`, so no infinity reaches a `Size` on any path.

Upstream of A1 and A3 and not covered by either. To exceed the viewport
the content must be measured under a loosened main axis. Then, in the
derive's second scan (`macros/src/lib.rs:340-352`), a `FillPortion`
child inside that content resolves against `max_h = ∞`: `used += ∞`,
`free = (∞ − ∞).max(0.0)` = **NaN**. NaN sizes land in `NodeState`, make
every `contains` false, hash by bit pattern into the journal, and slip
past the audit's `EPS` comparisons. Concrete repro once a `ScrollView`
exists: `ScrollView { Column { Spacer /* Fill×Fill */, Label } }`.

Every framework has a rule here; CSS resolves flex against the viewport,
not the content. **Recommendation:** a scroller passes its own main-axis
extent as the reference for `Fill` and an unbounded *maximum* for
`Shrink`/`Fixed` — i.e. `Fill` inside a scroller means "the viewport",
never "the content". Needs a decision and a NaN-guard test either way.

### A6. Who clamps the scroll offset? — **DECIDED 2026-09-04, recommendation adopted**, **BUILT 2026-09-04**

`ScrollView::layout` clamps after measuring, and resets a non-finite
offset to zero rather than clamping it into another non-finite one.

A3 settles where the offset lives and stops. Content extent is known only
after layout — the same reasoning B5 uses to put caret-into-view in
`layout` — but A3 does not apply it to its own offset. Today
`repeat.rs:85` clamps `first` to `count − 1` while `place` uses raw
`self.scroll`, so `scroll = 1e9` on a ten-row list shows an empty box. A
wheel handler will write that value directly. **Recommendation:** clamp
in `layout` after measuring, same place and shape as B5.

### A7. A `Repeat` inside a `ScrollView` materializes every row — **DECIDED 2026-09-04, recommendation adopted** (forbid and lint), **BUILT 2026-09-04** as `W1008`

**Reported, not refused.** The first implementation also had
`ScrollView::layout` assert against it in debug builds. That was wrong
twice: it turns a slow frame into a crash, and — the part that settled
it — it made the release-path lint impossible to test, because the
assert fired before any frame the lint could examine. A check that
prevents its own verification is not a check. `Node::virtualizes()`
carries the signal, ungated, so the invariant does not hold only in dev
builds.

`visible_range(limits.max_h)` derives the window from the *constraint*,
not from an intersection with a viewport. A scroller gives its content
an unbounded main axis (A5), so a nested `Repeat` computes
`vis = ceil(∞ / extent) + 1`, saturates, and is clamped to **`count`**. A
1M-row list inside a page-level scroller materializes and binds all
1M rows every frame, silently — and it is the first composition anyone
will write. **Recommendation:** either `Repeat` takes a visible rect
rather than a constraint, or nesting a `Repeat` in a scroller is
forbidden and linted. The second is simpler and probably right for v1:
a `Repeat` *is* a scroller.

---

## Cluster B — text input

### B1. Who owns the editing buffer?

**DECIDED: the widget. One allocation, grown once, reused.**

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

**DECIDED: a separate `Shaped`, leaving `Measured` alone.**

`Measured` is one `Cell<Option<(Constraints, Size)>>`. Every label in a
100k-node tree carries one. Adding an advance array would put an
allocation in every label to serve the few that need caret placement. A
separate cache keeps the cheap case cheap, which is the same shape as
`Repeat`'s opt-in arena.

### B3. How are edits applied — `&mut String`, or methods?

**DECIDED: methods only (`insert_str`, `remove_range`), never a
raw `&mut` to the storage.**

Free to adopt now, expensive to retrofit. Undo, IME preedit and
change-notification all need a seam where edits are observed. Exposing
`&mut String` forecloses all three; a method boundary costs nothing today
and keeps them possible. This is the one recommendation here that is
cheap now and irreversible later, so it is the one worth taking even if
undo is never built.

### B4. How do caret and selection reach the screen?

**DECIDED: ordinary `Cmd::Rect`. No new display-list command.**

A caret is a one-pixel rect; a selection is one rect per line. The
display list stays minimal and the software renderer needs no change,
which keeps it usable as the headless test oracle.

Caret *blink* is animation and is deferred — it would be the first thing
in the framework needing a frame clock, which is a separate decision.

### B5. Where does "scroll the caret into view" happen?

**DECIDED: in the widget's `layout`, after measuring.**

Not in the event handler, which runs before layout and has no geometry to
clamp against — it does not yet know how wide the field is or where the
caret landed. This is the one piece of text-input behaviour that is a
layout-time side effect rather than an event-time one, and getting it
wrong produces a caret that is correct one frame late.

> **REVISED 2026-09-04.** B7 below says "one widget type, with the field
> reserved". M9 shipped `TextInput` with **no** reserved field —
> deliberately, on the ground that an unused field is a claim the code
> has not earned — and M11/T11.3 (`docs/plan-widgets.md`) makes
> multi-line a separate `TextArea` rather than a flag. The decision
> below stands ("no multi-line in the first cut"); its *mechanism* does
> not. Recorded here because this file is indexed as live and a reader
> would otherwise trust the reserved-field sentence.

### B6. Is IME in the first cut?

**DECIDED: no — but reserve the field, and say so plainly.**

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

**DECIDED: no. One widget type, with the field reserved.**

Topology is identical — one node either way — so the static-tree
preference for types over flags does not apply. Selection across line
boundaries is where the real complexity is; single-line first, `wrap:
bool` reserved.

### B8. How does the application read the value?

**DECIDED: directly — `ui.field.as_str()`. Defer `#[bind_from]`.**

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

### B9. `Shaped`'s allocation, reload mode and `no_std` story — **DECIDED 2026-09-04, recommendation adopted**, **BUILT 2026-09-04** as `TextBuffer::Clusters`

B1 says the buffer is "one allocation, grown once, reused". B2 says
nothing about the advance array. A `Vec<f32>` that grows as the user
types is an allocation in steady state — the non-negotiable. Also
unstated: `Shaped` is derived state and must be `#[reload(fresh)]`,
never migrated (as `StyleState` in T1.6); `Measured`'s `Cell<Option<_>>`
shape does not transfer because the payload is not `Copy`, so `Shaped`
is a plain buffer mutated in `layout` (`&mut self`); and `TextInput` has
no `no_std` story at all where `Label<&'static str>` deliberately does.
**Recommendation:** make the advance buffer an associated type of the
storage, so `String` pairs with `Vec<f32>` and an inline buffer pairs
with an inline array; grow both by the same policy.

### B10. `Repeat::bind` defeats the memo the text design exists for — **DECIDED 2026-09-04, recommendation adopted**, **ALREADY CLOSED**

Both halves were already in place when this was revisited on 2026-09-04.
`Label::set_str` compares before writing (F4.6, 2026-09-02), and the
window is a ring, so a one-row scroll rebinds every row but only *one*
writes — the measured 31-of-31 became 1-of-31 then. The recommendation's
second half ("skip `bind` for rows whose index is unchanged") saves only
the call itself, which writes nothing.
`crates/lumen-static/tests/repeat_memo.rs` pins both.

`repeat.rs:145-147` calls `bind(item, row)` for every materialized row on
every layout. The natural bind body is `row.label.set_str(..)`, which
sets the dirty bit unconditionally, so `Measured` recomputes on every
idle frame. Measured by review: 11 visible rows, 100 idle frames, 1,100
shaper calls with a `set_str` bind versus 0 with a `format_into` bind.
Nothing in `fn(&mut T, usize)` steers an author to the comparing form. A
`Shaped` cache layered on the same keying inherits the hole and is more
expensive to rebuild. **Recommendation:** absorb `set_if_changed` into
`Label::set_str` — compare-then-write is the only sane default for a
memoized field — and skip `bind` for rows whose index is unchanged.
Must land before B2.

### B11. `TextStorage::from_string` blocks the no-alloc profile — **DECIDED 2026-09-04, recommendation adopted**, **BUILT 2026-09-04**

`TextStorage` is `AsRef<str> + Default` and nothing else; `from_string`
moved to `TextRestore`, gated behind `reload`. The advance buffer B9
asked for is `TextBuffer::Clusters`, an associated type — so a
fixed-capacity storage can pair with a fixed-capacity cluster array and
neither trait drags in an allocator.

It takes an owned `String` by value. A fixed-capacity inline buffer must
depend on `alloc` just to satisfy the signature, and has no way to
signal overflow. There is no `no_std` feature today, so this is
aspiration rather than regression — but it forecloses the profile.
**Recommendation:** `TextStorage: AsRef<str> + Default` only;
`TextBuffer` (already split out 2026-09-02) for repeated writes; move
`from_string` to a `reload`-gated `TextRestore` trait.

---

## Cluster C — routing: the layer both halves depend on

Not in the first version of this document at all, which claimed under
"Not questions" that *"the event vocabulary is ready … so a widget-owned
buffer is drivable by an agent on day one."* The vocabulary exists. The
routing is a stub: `pass.rs:154` returns before any handler lookup for
every event that is not `PointerUp`, and says so in its own comment. The
agent's `input.type` and `input.scroll` are enqueued, drained, and
dropped — finished-but-unreachable, the failure mode `audit.rs` says was
adopted from Lumen's review rather than re-earned.

### C1. Route `Key`, `Text` and `Scroll` — **DECIDED 2026-09-04, recommendation adopted**, **BUILT 2026-09-04**

`dispatch` must deliver them. To what, is C3. **Recommendation:**
keyboard and text to the focused node, walking ancestors as pointer
events do; scroll to the hit node, walking ancestors (C4 decides how the
walk ends).

### C2. Event payload — **DECIDED 2026-09-04, recommendation adopted**, **BUILT 2026-09-04**

`EventCtx { node, row, inner, pos, local }` has nowhere to put
`Event::Text { ch }` or `Scroll { dy }`. A handler cannot learn what
happened. **Recommendation:** `EventCtx.event: Event` — the ctx already
carries derived data; carrying the source is one field.

### C3. How does a handler reach widget state? — **DECIDED 2026-09-04, recommendation adopted**: (ii) `Node::on_event`, **BUILT 2026-09-04**

Handlers are `fn(&mut S, &EventCtx)` with `S: App`. B1 puts the buffer in
the widget; A3 puts the scroll offset there. From `&mut S`, a widget is
reachable generically only as `&mut dyn Node` — no editing methods, no
`Any`, `unsafe` banned. Two ways out:

- (i) hand-written per-app handlers navigating the concrete tree
  (`app.ui.field.insert(ch)`). Works, defeats the idea of a `TextInput`
  that edits itself.
- (ii) `Node::on_event(&mut self, ev, cx) -> bool`. A `Node` trait change
  of M0's class — every leaf, all hatches, `Repeat`, the derive — and it
  reorders `dispatch`'s copy-handler-then-`&mut S` sequence, which today
  relies on `tree()` being a *shared* borrow.

**Recommendation: (ii).** It is the only shape under which a widget
library exists. Cost it as its own milestone task, not as part of
"text input".

### C4. Scroll chaining — **DECIDED 2026-09-04, recommendation adopted**, **BUILT 2026-09-04** — it fell out of C3's `bool` with no separate mechanism

The ancestor walk stops at the first node that *has* a handler, and
handlers return `()`. Nested scrollers need "at my end, take it". Adding
a return value to the handler `fn` pointer changes the signature the
hot-reload contract is written around. **Recommendation:** `bool` from
`Node::on_event` (C3) is the natural carrier; the `Handlers` fn-pointer
signature stays as it is for click handlers.

### C5. Focus for off-table nodes, Tab order, visibility — **DECIDED 2026-09-04, recommendation adopted**, **PART-BUILT 2026-09-04**

**Built:** hidden focus is dropped, and Tab walks the focusables in
declaration order, wrapping, skipping anything with no area. Both are
checked before any routing, so a stale index cannot be used by *any*
event (F1.8 had checked only the positionless arm).

**Not built: focus as a `NodeSel`.** T8.0 removed its main motivation —
`Either` branches are ordinary table nodes now, addressed by index like
anything else — leaving only `Repeat` rows, and no focusable widget
currently lives in a template. Deferred until one does, rather than
changing a public field's type for a case that does not exist. Recorded
so the next reader does not have to rediscover why it looks half-done.

`focus: Option<u32>` is a main-table index. A `TextInput` inside a
`Repeat` row, an `Either` branch or a `Dyn` island has no focus
representation (`Hit` already models this via `Inner`; focus does not).
Nothing consumes `Key::Tab`. And focus does not track visibility: click a
field in `Either::A`, switch to `B`, keep typing — the hidden field takes
the keystrokes, because `dispatch` never rechecks `style[f].visible`
despite `pass.rs:113` saying that is why style is threaded.
**Recommendation:** focus becomes a `NodeSel` (table index plus optional
`Inner`), `dispatch` drops focus that is hidden, Tab order is
declaration order within a container. Each is small; together they are
a task.

---

## What A1 changed on the day it was answered

Two things were wrong immediately and are now fixed, both committed with
this decision:

1. **`Repeat::paint` emits no clip.** It now brackets its window in
   `PushClip`/`PopClip` using the same rect `hit` gates on, so the two
   cannot diverge. `Repeat` always clips — it is a viewport — so this
   needs no `Style.overflow`; when A2 lands, `Repeat` keeps it as its
   `defaults()` and the generic mechanism subsumes the special case.

2. **The software renderer ignored the clip stack.** It maintained the
   stack but never intersected against it — its own comment said so
   ("tracked but not yet applied per-pixel; the stack is here so the
   command vocabulary is exercised end to end"). So step 1 alone emitted
   a correct command that the project's only renderer discarded, and the
   pixel test still showed 30% ink where no click could land. Both halves
   were needed; the command-level test could not have caught it.

The invariant's precise statement, corrected: it is **not** "children
never escape their parents". It is *content may extend beyond a clipping
ancestor and is clipped there; it may not extend beyond a non-clipping
one.* A repeater's rows still overrun its viewport by up to a full item —
that is required by virtualization — they are simply no longer drawn or
hit outside it.

## Consequences still to handle

1. `hit.rs`'s module comment states the global invariant and must be
   rewritten to the scoped one. **Still open.**
2. The guard test in `lumen-static-agent/tests/lint.rs` — rewrite, do
   not delete (A4). It walks `SUB_TABLE` and cannot see arenas.
3. `docs/plan-architecture-v2.md` T2.6 re-pinned the containment
   invariant under the `Length` model; it needs an amendment.
4. `audit.rs`'s `_OVERFLOW_RESERVED` / `_OVERLAP_RESERVED` doc comments
   name the trigger as "a scrollable content region" — that trigger fires
   here.
5. ~~`List`, `Dyn` and `Assembled` have the identical unclipped
   divergence~~ — **fixed 2026-09-02**, with tests.
6. **`Repeat::observe` still emits the overrun rows** (`repeat.rs`
   observe) with bounds outside the viewport. Clipped from paint,
   rejected by hit, announced to AccessKit and the agent anyway — the
   probe-vs-click class one surface over. Not in the first version of
   this list.
7. **The clip rect is the border box, not the padding box.** Children
   are placed inside padding; content will draw over a scroller's
   padding. Every other toolkit clips to the padding box.
8. **The clip stack has no RAII bracket.** An early `return` in a `paint`
   body — and A2 will want `if !visible { return }` — leaks a clip onto
   every later sibling. A debug assertion on balance at frame end.
9. **`Either::layout` never calls `resolve_self`** — it writes the
   branch's size straight to `st[i]` and ignores `style[i]`. Its
   width/height style is dead today, and it cannot be a clip scope by
   A2's mechanism.
10. **Unrelated, found while writing this:** M4 T4.1 specified
    `NodeState.flags: HIDDEN`; the code uses `Style.visible`. The plan
    should record the deviation and its reason.

## Not questions

- **`NODE_COUNT` and `DEPTH` are unaffected.** A scroll viewport is one
  node with one child subtree; both stay compile-time constants.
- ~~The event vocabulary is ready … drivable by an agent on day one.~~
  **False** — see Cluster C. The vocabulary exists; nothing routes it.
- **Clipping primitives exist.** `Cmd::PushClip`/`PopClip` are in the
  display list and the software renderer honours them.
