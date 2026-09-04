# M11 — the widget vocabulary

*Planned 2026-09-04. Was Phase B of `docs/plan-lumen-parity.md`; that
document is superseded as a plan and this replaces its widget section.
Status: **not started**, and gated on M10 because every widget encodes
the allocation answer.*

## What is already built

Phase B's list was written when this project had four widgets. It now
has eleven, and three of Phase B's items are among them:

| Phase B item | status |
|---|---|
| text input | `TextInput` (M9) — no IME, no selection, single-line |
| scrollable | `ScrollView` (M8) — with chaining |
| §6 escape hatches as first-class widgets | `Either`, `Option`, `List`, `Dyn`, `Repeat`, `Assembled` |
| checkbox / radio / switch | **not built** |
| slider | **not built** — blocked, see T11.2 |
| text area | **not built** — reopens B7 |
| container / card | *not needed* — any derived container with a background is one |
| tabs | **not built** |
| dropdown / select | **not built** — blocked, see T11.5 |
| tooltip | **not built** — blocked, see T11.5 |
| modal | **not built** — blocked, see T11.5 |

The interesting result of that table is that **the remaining work is not
uniform**. Three items are cheap, one needs an event-vocabulary change,
one reopens a closed decision, and three need a layout capability that
does not exist. Treating them as one list of nine widgets is what the
original phase did, and it is why the phase was never actionable.

## The four groups, in order

### T11.1 — toggles: checkbox, radio, switch — *cheap*

One toggle core with three presentations. Each is a `Button` with a
`bool`, a different `paint`, and an AccessKit role
(`CheckBox`/`RadioButton`/`Switch`) with `set_toggled`.

Most of what they need exists: `NodeState::pressed` and `hovered` (C6),
focus and Tab order (C5), and `set_hidden`/roles in `observe`. These are
the first widgets to use `pressed` for something other than a shade.

**One thing does not exist, and it is the convention:** every desktop
toolkit toggles a checkbox with **Space**, and the `Key` enum has no
variant for it. That is an event-vocabulary gap of the same kind T11.2
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

**Gate:** the `ui_check.py press` check passes on all three; `ui.lint`
is clean; a toggle round-trips through `input.click` *and*
`input.key Space`; the AccessKit role and toggled state appear in
`ui.getTree`.

### T11.2 — slider — *needs a routing change*

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

**Measure before deciding (1).** The C6 note says routing every move
would cost an ancestor walk per motion event. Pointer capture avoids
the walk entirely, so the cost should be near zero — but that is a
prediction, and this project's rule is that a traversal change is
re-measured. Record it in a results doc.

**Gate:** `app.perf` shows no regression in `frame_ns` p95 during a
drag; a slider driven by `input.pointerDown` → moves → `input.pointerUp`
reaches the right value; and the value is reachable by keyboard
(Left/Right), because a control only draggable by mouse is not
accessible.

### T11.3 — text area — *reopens B7*

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

**Gate:** the UTF-8 caret walk from `tests/text_input.rs`, extended to
up/down across wrapped lines and asserting a boundary at every stop; a
`ScrollView`-free vertical scroll that keeps the caret visible (B5's
rule, one dimension over).

### T11.4 — tabs — *cheap, and T8.0 made it cheaper*

*Ordered fourth in this document and buildable second: it has the same
dependency depth as T11.1 (T8.0 only) and needs neither the routing
change nor B7. Grouped with the text work only because it reads better
that way; take it whenever, and the review was right to notice the two
cheap items are not adjacent.*

A header strip plus `Either` (or an enum-derived tree for more than two
tabs). T8.0 put branch nodes on the table, so a tab's content is
addressable by selector, styleable by `#id`, and visible to the audit —
none of which was true before 2026-09-04.

**Gate:** switching tabs changes `ui.lastChange`'s reported subtree and
nothing else; the hidden tab's nodes are `set_hidden` in the AccessKit
tree (already true — `emit_table` marks zero-area nodes); focus does not
survive into a hidden tab (already true — C5).

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

**Recommendation: do not start T11.5 as a widget task.** It is a layout
milestone with three widgets on the end of it, and estimating it as
"three widgets" is how it would get underestimated.

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
