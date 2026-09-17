# M27 — the remaining widget set, each one driven and then attacked

> **Written 2026-09-17.** The ask: build every widget Lumen ships that
> this framework does not, minus FindReplaceBar and Stepper, plus a
> number input Lumen does not have. Thirty-three widgets.
>
> And a second ask that is the larger half of this document: **each one
> is put in the showcase with data that reaches all of its states, every
> action it accepts is written down, an agent drives every one of those
> actions against the running window, and a separate adversarial agent
> then tries to prove the drive worthless.**
>
> M23 (`docs/plan-widgets-gallery.md`) owns the crate, tranche 1, and
> the five defects driving the showcase found. This supersedes its §4
> task list: the widgets are the same, the gate around them is not.

## 0. The count, corrected

`docs/plan-widgets-gallery.md` says "twenty-three of sixty-one …
thirty-eight left". That was true when it was written and is now off by
four — tranche 1 landed Skeleton and Spinner after the table was typed,
and M24/T24.2 landed Select. Recounted against the source on
2026-09-17, by grepping each of the sixty-one names for a `struct`,
`enum` or `type` in `crates/*/src`:

| group | of | built | left |
|---|---|---|---|
| Text & display | 15 | 10 | 5 |
| Buttons & input | 16 | 7 | 9 |
| Choice | 4 | 1 | 3 |
| Layout | 8 | 3 | 5 |
| Lists & data | 4 | 1 | 3 |
| Charts & progress | 5 | 4 | 1 |
| Navigation | 5 | 0 | 5 |
| Overlays | 4 | 1 | 3 |
| **total** | **61** | **27** | **34** |

Thirty-four left; minus FindReplaceBar and Stepper, **thirty-two**; plus
the number input, **thirty-three**. The gallery plan's header is fixed
in this commit — the doc-currency rule applies to a stale count in a
plan exactly as it applies to one in rustdoc.

Three of the thirty-four grep as present and are not. `Menu` is a type
alias in `select.rs` for the popup half of a `Select`; `Canvas` is the
software rasteriser's render target; `Tree` is the derive. All three
collide with a Lumen widget name, and §2 settles them before anything
is typed — this repo already renamed Rule to `Divider` for exactly this
reason, and the precedent is that **a widget set may differ from its
model where the model's name is taken here.**

## 1. The thirty-three

Written without backticks throughout: none of them exists, and a
backticked name that names nothing is what `check_doc_identifiers.sh`
is for.

| group | widgets |
|---|---|
| **Text & display** (5) | RichText, Markdown, Card, Toast, Tooltip |
| **Buttons & input** (8) | RangeSlider, **NumberInput**, SearchField, RichTextEditor, ColorPicker, DatePicker, TimePicker, FilePicker |
| **Choice** (3) | PickList, Combobox, Menu |
| **Layout** (5) | Wrap, Grid, SplitPane, PaneGrid, Accordion |
| **Lists & data** (3) | DataGrid, Tree, Pagination |
| **Charts** (1) | Canvas |
| **Navigation** (5) | Tabs, AppBar, BottomNav, NavigationRail, PullToRefresh |
| **Overlays** (3) | Modal, Sheet, Drawer |

**Excluded, and why it is worth saying:** Stepper and FindReplaceBar are
the two the ask drops. Both were already scheduled — Stepper in M23's
T23.3, FindReplaceBar in T23.10 — so this is a removal from a plan, not
an omission from a survey, and §6 records it where the next reader of
that plan will look.

**NumberInput is not Lumen's.** It is the ask's addition, and it is a
different widget from the Stepper it replaces precisely by having no
`+`/`−` buttons: a text field constrained to a number, with min, max,
step, keyboard increment and commit-on-blur. §4 works it end to end as
the protocol's example, because a widget nobody has built yet is the
honest test of a process that claims to specify before it builds.

## 2. Three things that gate whole tranches

Each is a task, each comes before the widgets that need it, and each is
already known — two of them are written down elsewhere in this
repository as debts nobody scheduled.

### T27.0 — per-row addressing for `Repeat`

`docs/plan-select.md` T24.4, promoted here from a note to a
prerequisite, because **the drive protocol cannot be built without
it.**

The shape is not "rows have no id". It is that **rows are visible to the
agent and untouchable by it.** `NodeDesc::wid` is a `&'static str` in the
*const* table, and a repeater's rows are not in that table at all — N
rows share the template's single `NodeDesc`, so there is one `wid` for
all of them however many are materialized. The agent enumerates them
anyway: `observe::emit_table` gives each instance node a synthetic
64-bit id with **bit 63 set**, and `Index::build` sets `Entry::idx` to
`None` for exactly those. Every action method then does
`e.idx.ok_or(...)` and **refuses, deliberately, with advice**:

> `"selector resolved to virtualized content; pass x/y instead"`

Four sites do this today — `input.fill`, `input.scrollIntoView`, the
shared `pointer_target` behind click/hover/`pointerDown`/`pointerUp`,
and `ui.explain`. So this is not a missing feature; it is a refusal, and
**the refusal's own advice is the vacuous-test shape AGENT.md names.**
`scripts/agent_drive.sh` takes that advice and computes
`box[1] + ROW_H * 8 + ROW_H / 2`, which is one renamed row away from
clicking the wrong widget and passing.

Two notes for whoever builds it, from reading the refusal sites rather
than guessing at them:

- **The click path may be nearly free.** `resolve_one` already returns an
  `Entry` that carries `bounds()`, and `pointer_target` needs `idx`
  *only* to compute a centre from `tree.states[i]` — which the bounds
  already give. Click, hover and the held press want a point, not a
  table index.
- **The rest genuinely need an index**, because they read per-node state
  the main table owns: `ui.explain`, `ui.getAppliedStyles`, and
  `input.fill`'s focus handling. Those need `(template, row, inner)`
  resolved to the arena, which is the real work — `EventCtx` already
  carries exactly that triple (`node`, `row`, `inner`) on the *dispatch*
  side, so the information exists and only the query direction is
  missing.

It unblocks six of the thirty-three directly — DataGrid, TreeView,
Pagination, Menu, PickList, Combobox are all repeaters whose rows must
be addressable to be tested at all — and it unblocks the harness for all
thirty-three, which must pick a widget by name out of a fifty-one-row
menu.

### T27.1 — the stroked rect

`Style` has no border and `Cmd` has no stroked rect. §3c of the gallery
plan found this by putting our TextInput beside Lumen's and recorded it
as "a gap worth a task of its own before T23.9's pickers". Counting the
thirty-three, **twelve** are bordered controls or need a hairline to
read at all — the four pickers, SearchField, NumberInput, Combobox,
Card, DataGrid, Grid, Tabs' active underline, Accordion's separators.

It is two gaps, and the second is the one that makes a workaround fail.

**No command.** `Cmd` has ten variants and none of them strokes. `Line`
exists, and its damage ink is correct — `retain.rs` takes the min/max of
the endpoints and widens by half the stroke, so a diagonal does not
leave its thick part behind. So four lines round a box *almost* works,
and two things stop it:

- `Style::radius` is read by every control this framework draws. Four
  straight lines cut the corners off a rounded rect.
- `MAX_REGIONS` is **4**, and its own doc says a frame's damage is
  "normally one region, or two once something reads the backdrop
  beneath it". A four-line border contributes four ink rects, and the
  widget's fill is a fifth — over the cap, merged into the last slot.
  **Prediction the task must measure, not assume:** twelve bordered
  widgets drawn this way systematically coarsen the region set M18
  exists to keep sharp.

**No style field.** `Style` carries fourteen fields and no border. So
even given the command, no stylesheet rule can say *border* — every
bordered widget would hardcode one, which is precisely the state
M25/T25.10 had to undo for colours.

**And one design instruction, from the `Deferring` lesson.** `Painter`
is five required methods and six provided, with four implementors of
which **three are wrappers** — `Deferring`, `Recording`, `Scaled`.
AGENT.md records what a provided method did to the first of them: it
overrode seven of eleven, inherited the rest, and every `Line` and
`Image` painted under an overlay pass was dropped. A new *provided*
`stroked_rect` reproduces that bug exactly and silently. **It ships as a
required method**, so each wrapper is a compile error until it forwards,
and the task's test is that a stroke survives an overlay pass.

### T27.2 — the three taken names

Settle before typing, not during. **None of the three is a compile
error**, and saying so matters, because the precedent everyone will
reach for is not the same shape: `Rule` *was* hard — `lumen_static::Rule`
is a stylesheet rule and is **in the prelude**, so a `Rule` widget makes
`use ...::prelude::*` ambiguous, and that is why tranche 1 shipped
`Divider`. Menu, Canvas and Tree are none of them in the prelude and all
of them in other crates, so `use` disambiguates.

What they cost instead is **prose**, and the tooling here is blind to
exactly that by design: `check_doc_identifiers.sh` says of itself that
it "does not know scope — only that nothing anywhere is called `hit`".
A backticked `Canvas` in a doc comment passes whichever `Canvas` the
writer meant, forever. The collision is invisible to the one gate that
looks at prose.

Ranked by how much the collision actually costs:

- **Canvas — rename the widget, certainly.** `software::Canvas` is the
  raster backend's pixel buffer, with dozens of methods, cited from
  `lumen-static-shell`'s front page, from `lumen-static-wgpu`'s shader
  comments, and from AGENT.md itself (`Canvas::bands_for`). Renaming
  *that* to free a widget name would be the tail wagging the dog.
  Recommendation: PaintBox for the widget.
- **Menu — cheap either way.** `pub type Menu<S> = Popup<Repeat<Row<S>>>`
  has seven uses, all inside `select.rs`. Recommendation: rename the
  alias to SelectMenu, because it is the menu *of a `Select`* and the
  narrower name is the more accurate one regardless of the collision.
- **Tree — the weakest case, and a judgement call.** `App::Tree` is an
  associated type in a different trait; there is no ambiguity, only a
  reader who meets three meanings of one word. Recommendation: TreeView,
  which is what the widget is called nearly everywhere else anyway — but
  this one is style, and it should be decided rather than inherited from
  the two above.

## 3. The gate: one widget, four artifacts

This is the ask's second half and the part worth arguing about. The
existing evidence says plainly that the failure mode here is not a
widget that does not work — it is a widget that works, is tested, and
is tested for the wrong thing. AGENT.md's table is thirteen rows long
and every row is a test that passed.

So the unit of work is not "a widget" but **a widget and the four
artifacts that make a claim about it falsifiable.**

### (a) The action inventory — written first, and stored as data

Every action a user can perform on the widget, written **before the
widget is built**. An inventory written afterwards describes what was
built; that is the whole reason it comes first.

It does not live in prose. It lives in the drive script as a list, and
the runner asserts that **every row was exercised** — so "the inventory
has nine actions and the script drives six" is a failure, not something
a reviewer has to notice:

```python
Action(
  name      = "arrow-up steps by `step`",
  do        = lambda a: a.key("#d_number_a", "ArrowUp"),
  expect    = lambda a: a.readout() == "3.5",     # was 3.0, step 0.5
  isolate   = lambda a: a.key_at(outside(box), "ArrowUp"),  # must NOT move
  injection = "delete the ArrowUp arm in NumberInput::on_event",
)
```

Coverage stops being a review question and becomes an assertion. What
the reviewer is then for is the question a runner cannot ask: *which
action is missing from the list*, which is §5's job.

### (b) The showcase entry, seeded to reach every state

The ask says "use whatever data it needs to test all functionality", and
that is a real constraint, not a formality. A DataGrid seeded with three
rows cannot be scrolled; a Tree with no collapsed branch cannot be
expanded; a Pagination with one page has no next. **Each inventory row
names the state it needs, and the drive asserts that state was
observed** — not that the click happened.

This is where the gallery plan's own lesson applies: its animated demos
opened at phase 0, an empty bar and an unlit spinner, indistinguishable
from a broken widget. Demo seeds are chosen to be mid-state for the same
reason.

### (c) The drive script — three assertions per action

`scripts/drive/<widget>.py`, run by `just widget-drive <name>` against
the live window through the agent socket, using the surface that already
exists: `ui.getTree`, `ui.resolve`, `ui.getAppliedStyles`,
`ui.getLayout`, `ui.probe`, `ui.probeRegion`, `ui.lastChange`,
`ui.lastDamage`, `ui.lint`, `ui.screenshot`, `ui.waitSettled`, and the
`input.*` family including `input.key`, `input.fill`, `input.hover`,
`input.scroll` and the held `input.pointerDown`/`pointerUp`.

Every action asserts three things, and the three map exactly onto the
three ways this repository has been fooled before:

1. **It landed.** The input reached *this widget*, via `ui.lastChange`
   naming the node or a readout that moved — never merely that the call
   returned. This is the T16.7 shape, where an oracle compared two
   identical no-ops because every click missed its target.
2. **The consequence is the specific one named**, with the measured
   quantity **printed beside its bound**. "The pixels differ" is not an
   assertion; "the knob's centre moved 42 px for a 42 px drag, bound
   ±2 px" is. This is the T16.2 shape, where a bound was computed from
   the wrong quantity and could not be exceeded.
3. **The same input just outside the widget's resolved box does not
   produce it.** A gate that refuses bad input must also accept good
   input; run backwards, a drive that only ever presses the right spot
   cannot tell a working handler from one that fires on everything.

Two assertions run after *every* action, not per action:

- **`ui.lint` is clean.** The ambient auditor already exists (M-series
  P3) and costs nothing to ask.
- **Damage is bounded by the widget's own box** for any local
  interaction, with the ratio printed. A widget that repaints the window
  to show a hover is a defect this framework has a whole milestone about
  (M16, M18) and no widget test has ever checked.

**And the drive runs twice — once under `Everything`, once under
`Boundaries`.** M26/T26.1–T26.5 found five defects of which **four were
invisible under the default `Everything`**, because a clean subtree is
skipped before the wrong answer can be observed. The showcase runs
`Everything` today. A protocol built after M26 that only runs the
default configuration has ignored M26's finding.

### (d) The injection — each drive shown to fail

AGENT.md's binding rule, which applies to a drive script exactly as to a
unit test, and with more force: a drive is a long script with many
assertions, and the number of ways for one of them to be unreachable
grows with its length. Each `Action` carries the injection that must
turn *its own* assertion red, by name, and the tranche's commit body
carries what each injection did and by how much.

An injection that leaves the drive green is a finding about the drive.
The correction is never to weaken it.

## 4. The protocol, worked: NumberInput

The widget nobody has built, specified before building, so the shape of
the artifact is visible rather than described.

**What it is.** A one-line field constrained to a number, carrying
`min`, `max`, `step` and a precision. No increment buttons — that is
what makes it a number input and not the Stepper this plan drops.
Commits on blur and on Enter; normalises what it commits.

**Its inventory — eleven actions**, each with its observable, and its
injection:

| # | action | observable | injection that must go red |
|---|---|---|---|
| 1 | click it | focus ring appears; `ui.getAppliedStyles` reports it | drop the focus arm |
| 2 | type digits | the field's text is what was typed | drop the key arm |
| 3 | type a letter | the text is **unchanged**, and no error state | accept all characters |
| 4 | Enter | value commits; the readout follows | commit only on blur |
| 5 | blur with "007" | normalises to "7" | skip normalisation |
| 6 | blur with a bare "−" | reverts to the last committed value | commit the unparsable string |
| 7 | ArrowUp | value rises by exactly `step` | change the step arm's sign |
| 8 | ArrowDown at `min` | value clamps, does not wrap | remove the clamp |
| 9 | PageUp | rises by 10 × `step` | alias PageUp to ArrowUp |
| 10 | select-all, retype | replaces rather than appends | drop the selection clear |
| 11 | a click 4 px outside | nothing focuses, nothing changes | *(the isolate arm of 1)* |

**Its seed.** `min = 0`, `max = 10`, `step = 0.5`, opening at 3.0 —
mid-range on purpose, so 8 can clamp at a boundary the demo can actually
reach and 7 can be distinguished from "it jumped to the max".

**One decision recorded rather than left implicit:** the scroll wheel
does *not* step it. A wheel over a number input inside a scroll view is
ambiguous, every toolkit resolves it differently, and an action nobody
can predict is worse than an action that is absent. If that is wrong it
is one inventory row to add — the point is it is a decision with a line
of reasoning, not an oversight.

## 5. The adversarial review

After the implementing agent says the widget is done, a **separate**
agent with no access to its reasoning is given five things: the
inventory, the widget source, the drive script, Lumen's rendered
reference from `crates/lumen-widgets/src/doc_shots/`, and our own
`--demo` render.

It answers **four questions, each of which demands an artifact rather
than an opinion** — because "review this widget" returns a summary, and
a summary has never caught any of the thirteen defects in AGENT.md's
table:

1. **Coverage.** Name an action a user can perform that is not in the
   inventory. This is the one question the runner cannot ask itself, and
   it is why a second agent exists at all.
2. **Vacuity.** For each assertion, write an injection that would leave
   it green — then **apply it and run the drive**. Any assertion that
   survives an injection aimed at its own mechanism is a finding. The
   adversary reports the diff and the output; it does not fix them.
3. **Render.** Compare the two PNGs on **geometry, not colour** — the
   gallery plan's ring-versus-sunburst rule, where 0.56 px against
   3.64 px separated a spinner from an hourglass and no amount of
   command counting could.
4. **Reachability.** Name a state of the widget that the showcase's seed
   cannot reach. Anything unreachable is untested whatever the drive
   says.

Findings go back to the implementing agent. **A widget is done when
every inventory row has an injection that reddens it and the adversary
reports no uncovered action.**

### The adversary is itself a gate, so it is itself tested

This is the part that is easy to skip and is the reason the whole
structure is not just one more passing test at a higher level. Once per
tranche, the adversary is run against a drive that has been
**deliberately weakened** — one assertion replaced with a tautology, the
choice made by the implementer and written down before the adversary
runs — and it must find it.

An adversary that misses the planted weakness has told you nothing about
the ones nobody planted, and the correction is to the adversary's
prompt, not to the tranche. This is AGENT.md's rule applied one level
up: *a gate that has never said no has not been tested*, and a reviewing
agent is a gate.

## 6. The tranches

Each is one commit, each ends with `just check` green, each ends with
`just widget-drive` green for its own widgets **and every widget before
it** — a regression surface that grows with the set is the point — and
each names its injections in the commit body.

| id | widgets | why grouped | needs |
|---|---|---|---|
| **T27.0** | *(no widget)* | per-row addressing for `Repeat` | — |
| **T27.1** | *(no widget)* | the stroked rect | — |
| **T27.2** | *(no widget)* | the three taken names | — |
| **T27.3** | *(no widget)* | the drive harness + adversary protocol, **retrofitted to the eighteen that already exist** | T27.0 |
| **T27.4** | Card, Toast, Tooltip | leaves with a shadow and a tip | T27.1 |
| **T27.5** | NumberInput, RangeSlider, SearchField | composites over `TextInput` and `Slider` | T27.1 |
| **T27.6** | Tabs, AppBar, BottomNav, NavigationRail, PullToRefresh | one shape five times: a strip with a selected index | T27.0 |
| **T27.7** | Modal, Sheet, Drawer | the overlay layer M15 built, now with a `Host` that dispatches into it (M24) | — |
| **T27.8** | Wrap, Grid, SplitPane, PaneGrid, Accordion | the layout group; needs `Widget`-level work, and is the tranche most likely to move the const-eval number | — |
| **T27.9** | Menu, PickList, Combobox | generalising `Select`, which the showcase's own picker already is | T27.0 |
| **T27.10** | DataGrid, TreeView, Pagination | repeaters with addressable rows | T27.0, T27.1 |
| **T27.11** | ColorPicker, DatePicker, TimePicker, FilePicker | all four are bordered composites over the above | T27.1, T27.8 |
| **T27.12** | RichText, Markdown, RichTextEditor, PaintBox | the text-heavy tail and the draw callback | — |

**T27.3 is the load-bearing one and it is deliberately placed before any
new widget.** The harness is built and then proved against eighteen
widgets that already work and have already been driven — because a
harness proved only against code written alongside it is a harness
shaped to agree with that code. The prediction, recorded here so it can
be wrong: **it finds defects in the existing eighteen**, because those
were gated on "does it paint" plus a handful of handlers, and none of
them has ever been checked for damage bounds, for lint cleanliness after
an interaction, or under `Boundaries`. If T27.3 finds nothing, that is
a finding about the harness and the tranche does not close.

## 7. The showcase at fifty-one entries

Measured today, not guessed: `Screen::NODE_COUNT` is **91**, `DEPTH`
is **8**, the catalogue holds **18** entries, and the `Demo` enum is
**73** of those 91 nodes — roughly four nodes per demo, because tranche
1 was all leaves.

The thirty-three are not leaves. A DataGrid, an Accordion or a
ColorPicker demo is ten to forty nodes, so fifty-one entries plausibly
lands between 500 and 800 nodes — five to nine times today's tree, in a
type whose const table is the **sum over all variants**, inactive
included.

Four numbers are therefore recorded at every tranche boundary, in a
results doc created by T27.3 and cited thereafter rather than restated:

| quantity | today | the cliff |
|---|---|---|
| `Screen::NODE_COUNT` | 91 | — |
| `Screen::DEPTH` | 8 | `MAX_DEPTH` = 64 |
| `const-budget` build | 3711 nodes < 5000 ms | the same gate, applied to the showcase |
| release `.text` | *(to record)* | the M15 gating policy |

**The kill rule, written before the measurement.** If the const-eval
build of the showcase crosses 5000 ms, or `DEPTH` crosses 32 — half of
`MAX_DEPTH`, leaving the other half for the application that embeds
one — the stage stops being one enum and becomes one enum per group
behind a group picker. That is eight enums of four to nine variants, it
is a task rather than an emergency, and deciding it now means the
measurement can say *no* rather than being read as a surprise.

## 8. What would make this not worth finishing

Stated up front, in this repository's habit:

- **If T27.3 finds nothing in the existing eighteen**, the harness is
  not discriminating and building thirty-three widgets through it buys
  confidence it has not earned. Fix the harness; do not proceed.
- **If the adversary cannot find the planted weakness** in its first
  tranche, the review step is ceremony. Fix the prompt; do not count the
  tranche.
- **If the drive's cost per widget exceeds the widget's**, the protocol
  is the product and the widgets are the excuse. Record the ratio at
  T27.5 — three widgets, all composites over things that exist — and
  decide there, where the sample is real and the sunk cost is small.
