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

It unblocks six of the thirty-three directly — DataGrid, Tree,
Pagination, Menu, PickList, Combobox are all repeaters whose rows must
be addressable to be tested at all — and it unblocks the harness for all
thirty-three, which must pick a widget by name out of a fifty-one-row
menu.

#### The option space

Four separable problems, and reading the code shows they are *not*
equally hard — two are nearly solved already.

| | problem | state today |
|---|---|---|
| **1** | ~~**no syntax** names a row~~ **LANDED 2026-09-18** | `Term::TextContains` (`~text`) is the one term that matches instance nodes at all — it reads the AccessKit node, not the table, so it needs no `idx`. It reads `value()`, and `Button` publishes `set_label`, not `set_value`. **So `~Switch` fails on a menu row by one field.** |
| **2** | **no decode** from a synthetic id | `observe::instance_id` is `1<<63 \| node<<44 \| row<<12 \| template`. The triple is *already in the id* the agent holds; `index_of_node` just answers `None` and nothing decodes the rest. |
| **3** | ~~**no action** without a table index~~ **LANDED 2026-09-18** (pointer only) | every path goes through `tree.states[i]`. But `pointer_target` wants only a **centre**, and `Entry::bounds()` already has one. |
| **4** | **no reachability** for a row outside the viewport | `Repeat::observe` announces only rows that overlap the viewport — deliberately (F4.7: announcing overrun rows created probe-versus-click divergence). So an offscreen row has no id, no bounds and no entry, and `input.scrollIntoView`, which is the fix, **refuses virtualized content**. Circular. |

Six candidate solutions, with what each does and does not buy:

- **(a) Coordinates — the status quo.** Zero cost, and it is the vacuous
  shape. The baseline, not a candidate.
- **(b) Let the text term fall back to `label()`. DONE 2026-09-18.** One
  condition, and the term was renamed `TextContains` to stop describing
  half of what it matches.
  Turns `~Switch` into a working row selector immediately, with no id
  decoding at all. Solves 1.
- **(c) Drop the `idx` requirement in the pointer path. DONE 2026-09-18.** deriving the
  centre from `Entry::bounds()`. Solves 3 for click, hover and the held
  press — which is most of what a drive does. Leaves `ui.explain`,
  `ui.getAppliedStyles` and `input.fill` refusing, because those read
  per-node state the main table owns.
- **(d) A row term over the decoded id** — `#select-menu:row(7)`, plus
  the decoder problem 2 says is missing. Solves 1 and 2 properly, and is
  the only route to 3 for the *query* methods, since those need
  `(container, row, inner)` to reach `Repeat::instance_state`, which
  already exists.
- **(e) Application-supplied row keys.** `Repeat` gains a way to name row
  *k* — the natural shape being a `fn(usize) -> &str` alongside the
  existing bind, so the app can name a row **without materializing it**.
  Strictly better than (b) or (d) for test quality: a drive that says
  "the Switch row" cannot silently retarget, where both an index and a
  label-match can.
- **(f) `input.scrollToRow`** — set `Repeat::scroll` to `row × item_extent`
  directly and re-lay-out. Sidesteps 4's circularity without touching
  `scrollIntoView`'s refusal. Needs (e) to know *which* row, unless the
  caller passes an index, which is the thing being avoided.

**The showcase forces the hard half, and this is the argument that
settles it.** At eighteen entries the menu resolves to 300 × 560 px and a
row is 30 px, so the whole catalogue fits and problem 4 never fires. At
fifty-one entries the content is 1 530 px against the same window:
**roughly two-thirds of the widgets are unmaterialized at any moment**,
and a harness that must pick each of fifty-one by name cannot reach them
at all. (The popup's height policy is worth checking before relying on
that arithmetic — but the direction is not in doubt.)

**Recommended, in three landings, cheapest first:**

1. ~~**(b) + (c)**~~ — **DONE 2026-09-18.**
   `input.click {"selector": "~Switch"}` now works on any *visible* row.
   `scripts/agent_drive.sh` picks the Switch by name instead of
   multiplying a row height, and `crates/lumen-static-agent/tests/rows.rs`
   holds the two claims with four injections behind them. T27.3's harness
   is unblocked for the eighteen that exist today.
2. **(e)** — row keys, so the drive names a row instead of computing one.
   This is the one that makes the protocol's claim about itself true.
3. **(d) + (f)** — the decoder and scroll-to-row. *Not built: the
   offscreen rows turned out to be reachable by **scrolling the menu**,
   which is a real user action and needed no new endpoint (T27.4).*
   **What did land, 2026-09-19, is the thing this list did not name:**
   a **descendant step**, `#outer > #inner`. Four `W1011`s in, the
   pattern was plain — a part id lives on a *type's* table, so a
   composite used twice is unaddressable by construction, and splitting
   the type each time does not scale and does nothing for an application
   that legitimately wants two dropdowns. A step means a part id only has
   to be unique **within its owner**, and the owner is named by the
   `#[widget_id]` the *application* gave its own field. `W1011` is scoped
   to match: it now reports only what a step cannot separate.

Splitting it this way means T27.3 is not gated on all four: it starts
after landing 1, and 2 and 3 arrive before the catalogue outgrows its
own menu — which happens in T27.6, the first tranche that takes the
count past what the popup can show.

### T27.1 — the stroked rect · **DONE 2026-09-18**

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
  exists to keep sharp. *(Not measured — the four-line form was never
  built, because `ops::focus_ring`'s rustdoc already records it being
  tried there and caught by the live gate within a frame: "a square ring
  over a rounded button covers the antialiased corner", `ui_check.py`'s
  corner test down to five blended pixels. One stroked command inks one
  rect, so the region question does not arise.)*

**No style field — and this is where it goes. DECIDED 2026-09-18:
border is a `Style` property, not a widget field.**

The alternative is the one M25/T25.10 already ran: colour fields were put
on the widgets that needed them, and then had to be given palette *roles*
in stylesheets so `Host::set_palette` could re-resolve them. A border
living on a widget cannot be restyled by a sheet, cannot follow a
palette, and cannot be set once for all twelve controls that want one.
That is the definition of a cascading property.

**What it costs, measured 2026-09-18 rather than estimated: `Style` goes
72 B → 80 B — and `NodeState` goes 76 B → 84 B with it, which the
estimate missed.** `Node::paint` has no `&[Style]` and must not
re-cascade, so the border travels to paint the way `bg` and `radius` do,
and *that* struct's tail was exactly full too. Per node: **148 B → 164 B,
+10.8%**, plus **+676 B of `.text`** on the embedded profile.
`docs/results-embedded.md` has the before/after. `Style` is *exactly*
packed at 72 with **zero spare tail padding** — `align`, `focus_ring`,
`cursor` and `visible` fill one 4-byte slot completely — so even a lone
`u8` costs four bytes. Two fields, `border_width: u8` (px; a border wider
than 255 px is not a border, which is `focus_ring`'s own argument) and
`border_color: Color`, land in the same 8-byte step that two `f32`s
would. Take the `u8` for the semantics, not for the size.

**The 76-byte version is a false economy.** Colour alone at a fixed 1 px
hairline saves four bytes by giving up Tabs' active underline — which is
one of the twelve widgets this task exists to unblock.

`tests/cursor.rs` asserts the 72 and its comment says a per-node field
"either fits or gets re-budgeted rather than paid for silently". Eight
bytes do not fit. So the task **re-budgets with a measurement** — the
embedded `.bss` and `.text` delta, in `docs/results-embedded.md`'s terms
— and does not simply edit the number in the assertion. `Patch` grows
with it, 112 B → ~120 B, and that one is cheap: a `Patch` is per *rule*,
and rules number in the tens.

#### Why `align` is in `Style` — asked 2026-09-18, and the budget answer that came out of it

The question was whether layout belongs on the widget and only appearance
in `Style`, since **not every widget needs `align`**. The second half is
right, the first half is not the seam this struct has, and following the
second half pays for half the border.

**`Style` is not "appearance" — it is *everything that cascades*.**
`width`, `height`, `padding` and `spacing` are layout and are 37 of its
72 bytes. Applying a layout/appearance split consistently would move
those four out too, and then no stylesheet could lay anything out — which
is the model the sheet lint (`W1010`), the showcase, and every decision
in `docs/plan-appearance.md` rest on. The seam is **cascades / does
not**, and `align` is on the cascading side by more than habit: `spacing`
has a derive attribute and **`align` has none**, so a stylesheet is the
*only* way to set it today.

**But the observation is exactly right.** Every reader of `align` is in
`crates/lumen-static-macros/src/lib.rs` — the derive's container layout,
off `let me = style[i]`. A leaf pays a byte it never reads.

**And that byte is worth four.** The tail slot — `align`, `focus_ring`,
`cursor`, `visible` — is 4 bytes and exactly full. Free one and
`border_width: u8` lands in it: **`Style` 72 → 76 rather than 72 → 80,
halving what the border costs.**

Two things stop that being a simple win:

- **The same test condemns its neighbours**, which is the sign it is the
  wrong axis. Only focusable nodes read `focus_ring`; only hovered ones
  read `cursor`. Three of the four tail bytes fail "not every widget
  needs it".
- **`i18n::mirror_styles` rewrites `s.align` and `s.padding` in one flat
  pass over the style array**, once per locale change, and its doc says
  this "costs a laid-out frame nothing". Per-widget alignment makes RTL
  mirroring a tree walk through a new trait method that every custom
  container must implement — and one that forgets is silently
  left-to-right in an RTL locale. That is the `Deferring` shape again: a
  provided body that is right for a leaf and wrong for a container.

**So take the bytes without moving the property.** The four-byte tail
holds 22 bits of real information: `align` 2 (four variants), `cursor` 3
(eight), `visible` 1, `focus_ring` 8, `border_width` 8. Packed, every
field keeps cascading, the border's width is free, and `Style` is 76.
The cost is named rather than hidden: `s.align` becomes `s.align()`,
`Style` stops being a plain aggregate that the cascade can assign
field-wise, and `Patch` application needs setters — churn across the
derive, `i18n`, the agent's style report and every sheet.

**Weigh it against T25.5 before building it.** That task ran the adjacent
experiment — six *appearance* fields off `Style` and onto the widget,
iced's shape — with its thresholds written before the pilot, and it was
**stopped on its own rule**; the code is in history at `0df160b` and
`docs/results-appearance.md` is the evidence. It says nothing directly
about `align`, but it is the same seam, and its protocol — thresholds
first, then measure — is the one to reuse here.

**Decided for now: `align` stays in `Style`, and the border ships at
80 B with the budget re-cut.** Packing the tail is a separate,
measurable task with a real API cost, and pairing it with a widget
milestone would hide one behind the other.

**And one design instruction, from the `Deferring` lesson.** `Painter`
is five required methods and six provided, with four implementors of
which **three are wrappers** — `Deferring`, `Recording`, `Scaled`.
AGENT.md records what a provided method did to the first of them: it
overrode seven of eleven, inherited the rest, and every `Line` and
`Image` painted under an overlay pass was dropped. A new *provided*
`stroked_rect` reproduces that bug exactly and silently. **It ships as a
required method**, so each wrapper is a compile error until it forwards,
and the task's test is that a stroke survives an overlay pass.

### T27.2 — the three taken names · **DONE 2026-09-18**

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

**DECIDED 2026-09-18**, ranked by how much the collision actually costs:

- **Canvas → the widget is PaintBox.** `software::Canvas` is the raster
  backend's pixel buffer, with dozens of methods, cited from
  `lumen-static-shell`'s front page, from `lumen-static-wgpu`'s shader
  comments, and from AGENT.md itself (`Canvas::bands_for`). Renaming
  *that* to free a widget name would be the tail wagging the dog. The
  reasoning is recorded on `software::Canvas` itself, where the next
  person tempted to rename it will read it.
- **Menu → the alias is now `SelectMenu`. Landed.** Seven uses, all
  inside `select.rs`. The
  narrower name is the more accurate one regardless of the collision: it
  is the menu *of a `Select`*. The widget keeps the plain name.
- **Tree → stays Tree.** `App::Tree` is an associated type in a different
  trait and `StaticTree` is the derive; there is no ambiguity, only a
  reader meeting three senses of one word, and none of the three is in
  the prelude. Renaming to TreeView would buy prose clarity at the price
  of a name every other toolkit spells the short way. Decided, not
  inherited from the two above — which is the point of listing it
  separately.

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
| **T27.0** | *(no widget)* | per-row addressing for `Repeat` — **landing 1 of 3 done 2026-09-18** | — |
| ~~**T27.1**~~ | *(no widget)* | ~~the stroked rect~~ **DONE 2026-09-18** | — |
| ~~**T27.2**~~ | *(no widget)* | ~~the three taken names~~ **DONE 2026-09-18** | — |
| ~~**T27.3**~~ | *(no widget)* | ~~the drive harness~~ **HARNESS DONE 2026-09-19**; six widgets retrofitted (18 actions), twelve left | T27.0 |
| ~~**T27.4**~~ | ~~Card, Toast, Tooltip~~ **DONE 2026-09-19** | leaves with a shadow and a tip — and two more framework defects | T27.1 |
| ~~**T27.5**~~ | ~~NumberInput, RangeSlider, SearchField~~ **DONE 2026-09-19** | composites over `TextInput` and `Slider` | T27.1 |
| ~~**T27.6**~~ | ~~Tabs, AppBar, BottomNav, NavigationRail, PullToRefresh~~ **DONE 2026-09-19** | ~~one shape five times~~ — **three of the five are**; see below | T27.0 |
| ~~**T27.7**~~ | ~~Modal, Sheet, Drawer~~ **DONE 2026-09-19** | the overlay layer M15 built | — |
| ~~**T27.8**~~ | ~~Wrap, Grid, SplitPane, PaneGrid, Accordion~~ **DONE 2026-09-19** | the layout group; needs `Widget`-level work, and is the tranche most likely to move the const-eval number | — |
| ~~**T27.9**~~ | ~~Menu, PickList, Combobox~~ **DONE 2026-09-19** | one `Chooser<KIND>` — and T27.0's third landing, which this forced | T27.0 |
| ~~**T27.10**~~ | ~~DataGrid, Tree, Pagination~~ **DONE 2026-09-19** | repeaters with addressable rows | T27.0, T27.1 |
| ~~**T27.11**~~ | ~~ColorPicker, DatePicker, TimePicker, FilePicker~~ **DONE 2026-09-19** | one `Picker<KIND>` over a grid of cells | T27.1, T27.8 |
| **T27.12** | RichText, Markdown, RichTextEditor, PaintBox | the text-heavy tail and the draw callback | — |

### T27.3, as built — and the prediction it was written to test

**The prediction held.** The plan said the harness would find defects in
the eighteen that already exist, and that if it found nothing the
tranche would not close. It found **six** while retrofitting the first six widgets,
five of them in the framework rather than the showcase:

| | what | how it surfaced |
|---|---|---|
| 1 | **Eight duplicate `#[widget_id]`s** in the showcase — every labelled pair. An id lives on a *type's* field, so a composite used twice duplicates every id in it, and both copies become unaddressable | `ui.resolve` refused `#d_checkbox_a`: "matched 2 nodes". New lint `W1011` |
| 2 | **`#[app(after_update)]` could not report**, so the derive marked every pump changed and the showcase never settled | recorded as a known cost in M23; now `-> bool`, mirroring `tick` |
| 3 | **`ui.lastDamage` reported a bounding box**, which is the union M18 replaced — the surface was never updated after M18 | a bound against it read 170× for a click on a button |
| 4 | **The journal folded a container's signature from its children's**, so one leaf's repaint reported every ancestor with its own rect — the root's being the window | damage 691 200 px² on a 4 059 px² button |
| 5 | **A container's own paint was never signed at all** — background, radius, shadow, border. *Hidden by 4*: the fold made every container look busy anyway | no test could have seen it while 4 stood |
| 6 | **`Cmd::hash_into` folded a text run's buffer offset**, so a length change anywhere reported every node painted after it | 61 056 px² of phantom damage from a one-byte-shorter readout |

**Damage went from 170×–1 200× the widget's own box to 1.00×**, measured
on the live window under both strategies.
`docs/damage-amplification-decision.md` has the table; that document had
recorded the renderer's 37× as known and unfixed and is where this
belongs beside it.

**Five of the harness's own assertions were vacuous and each was caught
by the isolate arm or by an injection aimed at it** — which is the protocol working on itself:
a painted-background assertion that could not tell a toggle from a hover
shade; a sibling that paints a rect and so cannot notice a shifted
*text* offset; `summed <= hull`, an inequality a bounding box satisfies
exactly; a slider `expect` written as a *state* (`abs(v - 0.5) < 0.12`)
rather than a transition, which no isolate arm can falsify once a real
click has put it there; and a focus-then-record ordering that blamed an
arrow key for the click that had focused the slider. All five were
strengthened. The plan said an
injection that leaves a test green is a finding about the test, and it
was, five times.

**One non-finding worth recording**: `Boundaries` and `Everything` have
so far produced *identical* results on every widget. That is not the
protocol failing — M26's four hidden defects were in layout skipping,
which none of these four widgets exercises — but it is a claim to keep
checking rather than to assume, and the first tranche whose widgets
manage their own subtree invalidation is where it should be re-read.

### T27.4, as built — the harness earning its keep on new code

Three widgets, and **two more framework defects**, both found by driving
rather than by reading and neither visible to a headless test:

| | what | how it surfaced |
|---|---|---|
| 7 | **A `Repeat` could not be scrolled.** Nothing in `Repeat`, `Popup` or `Select` handled a wheel — every repeater before M24 sat inside a `ScrollView`, which drove `scroll` as a plain field. So **a dropdown with more options than fit was unusable**, by anyone. | the drive could not reach the last three widgets in the showcase's *own* menu |
| 8 | And after the wheel landed it still did nothing: `visible_range` and the clamp measured against `limits.max_h`, which for a popup's content is the whole window — a 630 px list in a 560 px panel computed `max_scroll = 630 - 720 = 0`. **M24/T24.2 fixed this exact asymmetry for the width and left the height**, and its comment says so. | the wheel arrived, `on_event` ran, the clamp pinned the offset to zero |
| 9 | **A tooltip drawn over its own control takes every click on it.** The overlay pass hit-tests *before* `Node::hit`, so `Tooltip::hit` declining to offer the tip made no difference at all. | the press counter did not move; the headless test calls `Node::hit` directly and cannot see the overlay pass |

The fix for 9 is also the right design — a tip belongs *below* its
control — which is the useful kind of bug: the correct behaviour and the
working behaviour turn out to be the same edit.

**The reachability problem the plan put at fifty-one entries arrived at
twenty-one.** §2's option (f) predicted that a catalogue longer than its
own menu would strand two-thirds of the widgets; three new entries were
enough. The harness scrolls to find a row for now — a real user action
needing no new endpoint — and T27.0's third landing still owns the
proper answer.

**Three more harness bugs, all found by using it**: a drive left an
overlay open and the next fifteen widgets reported "never came into the
menu's viewport", which is a true statement about a menu nothing could
reach (`_pick` now dismisses and un-hovers first); a menu scrolled to the
bottom could never reach the top again (it scrolls back up first); and
the tip's area was read from the `Popup` node, which is a zero-sized
marker whose *content* carries the geometry — reported `0 -> 0` for a
tip that had opened perfectly well.

### T27.5, as built — and §4's inventory, kept

`NumberInput` shipped against §4's eleven-row inventory written before it
existed, which is the point of writing one first. **Two of the eleven
were wrong about the widget and the widget was right**: "type 5 and get
5" reads `0.05`, because typing appends at the caret like any field —
so row 10 (select-all, retype) is not a nicety, it is what makes the
other rows reachable. Both tests were corrected, and a second test now
asserts the appending behaviour so it is a choice rather than an
accident.

Two more framework findings:

| | what | how it surfaced |
|---|---|---|
| 10 | **No widget in `lumen-static-widgets` published anything to the agent.** The crate had no `observe` feature at all, so `ui.getTree` saw geometry and nothing else for every widget tranche 1 shipped. | a drive asked a number field for its value and got `""` |
| 11 | **A wrapper must loosen a child's limits to let it shrink.** A `SearchField` is `FILL`, so its own `min_w` is what its parent insisted on — and `resolve_self` clamps a `Fixed` *up* to `min_w`. A 20 px magnifier resolved to 296, centred, with the text off the right edge. | `ui.getAppliedStyles` said `Fixed(20.0)` while `ui.resolve` said 296 wide — the two disagreeing is the tell |

11 is worth keeping as a rule: **the maximum is inherited, the minimum
is not.** A wrapper that wants a child to shrink has to say so.

**And one of the showcase's own gates was found bounding the wrong
quantity** — `the_dropdown_is_absent_when_closed` asserted an open menu
draws more than `CATALOG.len()` extra commands, which was the same number
as the rows it materialized until the catalogue outgrew the menu. It now
bounds by `Repeat::window_len`, which is the thing it is about.

Three more harness assertions were vacuous and each was caught by running
it: a range check comparing `image_base64[:64]`, which is the PNG
*header* and identical for any two frames of a size — a bound that can
never be met, which is the same defect as one that can never be exceeded;
an action that clicked the same point three times and correctly reported
that nothing had reached the widget; and a bound written as `low > 0.5`,
a fact about where the *other* handle happened to be, which failed on the
correct answer `0.50..0.50`.

### T27.6, as built — where the grouping was wrong

The tranche table called these "one shape five times: a strip with a
selected index". **Three of the five are**, and forcing the other two
would have been worse than regrouping:

- `Tabs`, `BottomNav` and `NavigationRail` really are one type with three
  presentations, chosen by a `const` parameter the way `Chart` is — so
  `kind_name` and `defaults` answer at compile time and a `Kind` selector
  can style a rail differently from tabs.
- **`AppBar` is not a selector.** Nothing in it is selected and there is
  no index; giving it one to fit the group would be a field that is
  always zero and a keyboard that moves it nowhere.
- **`PullToRefresh` is not a strip at all.** It is a gesture wrapper.
  What it shares with `Tabs` is the word "navigation".

That is the tranche table doing its job: it grouped work plausibly from a
widget list, and the work corrected it. The grouping is worth keeping in
§6 as written, with this note beside it, because the next tranche's
grouping is a guess of the same kind.

**One defect, and the render found it.** A `NavigationRail`'s selected
label is drawn *in the style's background*, because it sits on a pill
filled with the foreground — and the rail's defaults had no background,
so the current item's label was invisible: dark text on a dark fill.
Every test about the selection was about geometry or about the index and
both were correct. `docs/plan-widgets-gallery.md` §3b's ghost button is
the same shape, for the third time in M27: **a transparent default takes
the widget's own affordance away.** `a_filled_strip_has_a_ground_for_its_selected_label`
now asserts it, in both directions.

**And one of this tranche's own tests was vacuous, caught by its
injection.** `an_app_bar_does_not_grow_with_its_title` laid the bar out
under its defaults, where `height: Fixed(56.0)` makes `resolve_self`
ignore the content outright — so making `layout` measure the title left
it green. The test proved the *style*, not the code it named. Laid out
under `Shrink`, the same injection reads "the bar grew from 58 to 122
with a longer title".

### T27.7, as built — and the composite-id shape, for the third time

One `Veil<W, KIND>`; the three differ only in where the panel sits. It
defers **itself** rather than letting the panel defer, because a modal is
two pieces of paint that must land together and above everything — a
wrapper that drew the scrim inline would put the dim *under* what it
dims.

Two decisions worth the argument:

- **Closed is absent, not unpainted.** `layout` zeroes the subtree, which
  is what stops a shut modal swallowing every click in the window. A
  veil that only skipped `paint` would still hit-test at full size, and
  the test that catches it says so by name.
- **Dismissal is a request.** A press on the scrim sets a flag;
  `take_dismissed` hands it over once. The widget never closes itself,
  because an application that must confirm before discarding cannot undo
  a close that already happened — the same take-don't-read shape as
  `PullToRefresh::take_triggered`.

**A modal's panel is capped at 80% of the window.** Found by a test whose
panel was a plain `Button`: `FILL` measured to the whole window, so there
was no scrim at all — nowhere to press to dismiss, and nothing to show
the panel was above the page.

**And `W1011` fired for the third time in M27.** `VeilBody` was one type
used by all three veils, so every id inside it was on three nodes and
`ui.resolve` refused all of them. The showcase now stamps a body type per
veil — the `PairCheckA`/`PairCheckB` split again. **The harness now
refuses to drive a tree with any `W1011` at all**, rather than leaving it
to the per-action lint delta, which only sees findings a *new* action
introduced and would miss one present from the start.

Three instances is a pattern, not a coincidence: **an author id lives on
a type's field, so a composite used twice is unaddressable by
construction.** `Select`, `Tooltip` and `SearchField` all publish part
ids with the same property. Instance-scoped addressing is T27.0's third
landing, and this is now the strongest argument for it.

**And a twelfth framework finding, from the sheet lint.** A
**struct-level `#[widget_class]` was accepted and silently ignored** —
`attributes(...)` let it through, nothing read it, and the class never
reached the table, so a rule written against it matched no node. That is
`docs/plan-review-fixes.md` F1.5's shape one level up: the
documented-looking spelling producing `&[]`. Found by
`every_stylesheet_rule_changes_something` reporting the rule
`Unmatched`, which is the sheet lint (M25/T25.2) doing exactly what it
was built for. The derive reads it now, the attribute table says so in
both places, and `tests/selectors.rs` holds it in both directions — it
lands on the container and does *not* leak onto the children.

### T27.8, as built — two axes, two widgets, and no Accordion

The plan said this group "needs `Widget`-level work", and it did, but not
evenly:

- **`wrap` and `grid` are derive axes, not widgets.** A container's
  children are its *fields* here, so a wrapping row is a way of arranging
  fields rather than something that holds them. They are **one
  lowering**: a grid is a wrap whose line break is a *count* instead of a
  width, and two lowerings would have been two places for the cross-axis
  arithmetic to disagree — the `Deferring` shape in a macro.
- **`SplitPane` and `Collapsible` are widgets**, because each holds
  something an author does not write as a field list: a divider between
  exactly two panes, and a body that hides.
- **There is no `Accordion`.** It is a `column` of `Collapsible`s, and
  the one-open-at-a-time rule is three lines in the application. An
  `Accordion<A, B, C>` to own that rule would be a type per section
  count.

The property that matters for a flow and is easy to get wrong: **`place`
must produce the same lines `layout` did.** `place` has no `LayoutCx` and
re-runs the break test against the sizes already in `NodeState`; the
injection that makes it forget a line's height breaks *all five* flow
tests, which is the coverage that claim needs.

**A stated limitation rather than a discovered one**: a flow cannot
divide `Fill` among its children, because it does not know how many will
share a line until it has measured them. A `Fill` child in a wrap takes
its natural size — a wrap of `Fill` items is a column with extra steps.

**Two more harness lessons, both from running it.** An action whose whole
point is to resize its own widget legitimately damages what moved, so
`Action.reflows` moves the damage bound to the *stage* — a larger bound,
not an absent one. And squeezing a pane to its stop *does* truncate the
text in it, so `Action.expects_lint` names the codes an action may
introduce: named codes only, because a run that accepted every new
finding would stop catching the ones nobody intended.

### T27.9, as built — and the two findings it forced

One `Chooser<KIND>`; `PickList`, `Combobox` and `Menu` differ in exactly
two things — whether there is a trigger, and whether typing in it
filters.

**They do not use `Repeat`, and that is the interesting part.** `Select`
is the core's dropdown over *someone else's* data: its rows are bound by
a plain `fn` the application supplies, and a plain `fn` cannot capture
the widget that owns the options. So a chooser that owns its own list
draws it, the way `Strip` draws its items — which also keeps a
six-option picker at four nodes instead of an arena, and sidesteps the
per-row addressing debt rather than inheriting it. **`Select` stays the
primitive.**

| | found | how |
|---|---|---|
| 13 | **A combobox filtered on its own selection.** Choosing "Apple" puts "Apple" in the trigger; filtering on that leaves one row, so re-opening shows only what is already chosen and every other option is unreachable. | the drive opened one and counted **one** row where six were seeded |
| 14 | **`ui.resolve` answered with no text for a `Button`.** Its summary carried `value` and never `label`, which `ui.getTree` has had since M15 — so anything reading a control's text through `resolve` got the empty string and could not tell that from a control with no text. | `Ctx.value()` returned `""` |

13's first fix was a flag set from `on_event`, and **it never fired**: a
`Text` event goes to the *focused* node, which is the field itself, and
the field consumes it — so the wrapper's arm only ever saw events the
field had declined. A query is now *derived* — "the trigger is no longer
showing the selection" — which needs no plumbing and cannot go stale.

**And one design tension, recorded rather than papered over.**
`dispatch_overlaid` dismisses every overlay on a press that misses them
all, clears focus, and **consumes** it. A trigger is outside its own
popup, so a second press on it closes the list rather than re-opening —
which is `Select`'s documented dropdown behaviour and is *desirable*.
But the rule cannot tell a press on the overlay's **owner** from a press
somewhere else entirely, and a combobox wants both: close the list *and*
focus the field. It cannot have both today.
`crates/lumen-static-widgets/tests/focus_overlay.rs` pins both halves,
and a chooser opens on **focus** rather than on a press — which a
keyboard user needs anyway, and which a press arm could not give them
because the field consumes the press first.

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

### T27.10, as built — the observer changed the observed

`Pagination`, `Tree` and `DataGrid`: three widgets that answer *where
the data lives* three different ways, because their data differs. A
pagination has none (thirty pages is a number). A tree owns its shape,
so collapsing is arithmetic over one array and `MAX_TREE_NODES` bounds
it. A grid owns **nothing** — its rows are a `Repeat` the application
binds, because a grid is a view of a table someone else holds.

| | found | how |
|---|---|---|
| 15 | **The agent's per-frame audit reset every repeater's scroll offset.** It measures each leaf twice, under a 1e6 and a 2e6 px box, to tell an intrinsic node from a filler — and `Repeat::layout` clamped its *stored* offset against the box it was handed, where `max_scroll` is 0. A `DataGrid` in the showcase could not be scrolled at all. | `just widget-drive`; a bare-widget unit test, a headless `Host` test and `tests/repeat_scroll.rs` all said it scrolled |
| 16 | **And the same probe left the agent reading the wrong rows.** `Node::probe`'s provided body is `self.layout(..)`, which for a repeater *picks a window*: after the audit, `first` was row 0 against a 2 000 000 px viewport whatever the frame had painted. The window on screen was right and the tree the agent read was wrong. | the drive saw the grid scroll one row and jump back |

Three fixes, in the order the evidence arrived, each with its own test:

1. **`layout` derives; it does not decide.** The offset is the user's;
   `first` and the window are derived from it. `Repeat::offset` clamps at
   use and never writes back, so a speculative pass cannot destroy a
   number nobody can recompute.
2. **The viewport is what was *placed*, not what was offered.** `place`
   runs once per frame on the real box; `layout` runs as often as anyone
   wants to ask a question. `Repeat::viewport()` is published so a test
   can assert the cause rather than the symptom.
3. **`Repeat` overrides `Node::probe`.** The hook exists for exactly
   this — `Label` already overrode it to keep the audit out of its text
   memo — and the answer a probe wants is the arithmetic `layout` starts
   with, without any of what it then does. This is AGENT.md's provided-method
   rule again: *a provided body is right for a leaf and wrong for
   anything that keeps state.*

**Both injections left the new test green the first time**, and the
reason is the first shape AGENT.md names: the fixture pinned the list's
height with `Fixed`, so its resolved height — and with it `max_scroll` —
is the same under a 2e6 px box as under the real one, and no probe could
move it. Rewritten on a **filling** repeater, which is what the
showcase's grid actually is, both fire by name (injection BO: offset 120 → 0;
injection BP: the wheel after the probes moves nothing), and the setup asserts
the probe resolved the list taller than its panel so a future `Fixed`
cannot quietly make it vacuous again.

**A fourth thing the drive found, fixed in the fixture rather than the
code**: folding two rows out of a shrink-wrapped tree on a centring
stage moves the demo 26 px and repaints everything below the header —
11.94x the stage, measured. A file tree lives in a sidebar with a size,
so the demo has one; the collapse is then 1.00x its own box. The
measurement is in the sheet rule's comment, beside the rule.

**One limitation recorded, not fixed.** `#outer > ~text` cannot reach a
repeater's rows: the descendant step filters by the table's parent
chain, and virtualized rows are not in the table, so a row is a
descendant of nothing. The drive uses the bare `~text` form, which is
safe because the file names are unique in that window. Addressing a row
*within* a named owner needs the step to understand arenas, which is
T27.0's fourth landing and is not built.

### The adversarial review of T27.10 — §5, run for real

A separate agent, given the inventory, the widget source, the harness,
the tests and the live window, and told to answer §5's four questions
with artifacts. It applied **sixteen headless injections and eleven live
ones**, and it was itself tested: one assertion in the inventory was
relaxed to a tautology and written down before the review started
(`_page(c) <= c._p0` in place of `== c._p0 - 1`, which the very defect
the action guards against would satisfy). **It found it**, quoted the
run printing `page 4 -> 1` beside the prose ruling that out, and named
the line. A gate that has never said no has not been tested; this one
has.

Fifteen findings. Four were defects in shipped widgets, three were
defects in the *framework* that the widgets only revealed, and six were
about the tests and the drive — which is the ratio §5 was written
expecting.

| | what | fixed by |
|---|---|---|
| 1 | **A pagination's current page number was invisible.** The selection pill is centred in its slot and the digit was left-aligned in it, so the glyph sat 3 px outside the pill — painted in the pill's contrast colour, on the control's own background. The review counted **zero** glyph pixels inside the pill. | measure at layout, centre at paint, the way `Strip` already did |
| 2 | **A tree painted outside its own box** — 116 px of rows over the note beneath it, on a fixture that fit by 2 px. | `push_clip`, as `Repeat` does |
| 3 | **A tree's hover highlight was unreachable**, and so was an option list's. A `PointerMove` reaches a widget only under capture, and capture follows a press. | `Event::Hover`, synthesized by `dispatch` for the hit node |
| 4 | **A grid's header did not sit over its rows** — the `Size` values 72 px left of the `Size` heading, with a correct header and correct rows and nothing reconciling them. | the inset moves onto the **grid**: it insets its header and hands the rows a box already inset and already divided, so a row whose children are `FILL` lines up for itself |
| — | and, found while fixing 4: **no arena had ever been cascaded** *after* the sheet was set. A stylesheet is applied before the first frame; a repeater's row template lives in an arena belonging to a widget the application may build later. Every rule naming a node inside a row built after that was dead. | `style::cascade_arenas_deep`, owed by any change that could be structural and paid after `App::update`, before layout |
| 5, 6 | two drive assertions that could not fail — the planted one, and "pressing a leaf changes nothing", whose consequence is by definition invariant | the plant reverted; the leaf action now asserts `handled` |
| 7 | the pagination's **window offset was never exercised live**: the drive never left page 4, so `first + k` was dead code under it | a new action that goes to the last page and presses the first *number*, where the window starts at 22 |
| 8 | both grid actions named an injection that does not redden them — one of which breaks the catalogue picker, so the drive never reaches the grid at all | both rewritten, with the reason kept beside them |
| 9, 10 | two headless tests vacuous for the property their names claim | one strengthened to assert the viewport against the grid; the other **deleted** — its other half is covered twice |
| 11 | `demo_shot` cropped by "narrower than 300 px", so a `Pagination` and a `DataGrid` were excluded from their own crop and `--demo` rendered the caption | crop the demo's **subtree**, by the table |
| 12 | a sheet rule the laid-out box never took (`Fixed(380)` on a 320 px card) | removed; the lint cannot catch this one and the comment says so |
| 13 | `Action.isolate`'s doc said the runner counts the actions without one; nothing counted anything | it counts and prints them |
| 14 | `Tree::on_event` hard-coded the padding its own seed declares it reads | read from the resolved style |
| 15 | page numbers past a 33-entry table all rendered "32" | `PageLabel`, five digits into a stack buffer |

**The one that changes the harness for every widget is 5/6's fix.**
Until now "did that input land" was *inferred* — from a changed value, a
damaged rect, a different readout — and an action whose correct outcome
is that **nothing happens** has no consequence to infer from. So
`AgentHost::pump` now answers `(frame, handled)` and `input.click`
reports it. A leaf press that is silently taken is visible for the first
time, and every future "this input must do nothing" row has a mechanism
to assert rather than an absence to hope about. It is a **required**
trait method, not a provided one: a default of `(self.pump(), false)`
would make every test double claim nothing was handled and be believed.

**Finding 4 was fixed twice, and the first fix is the instructive
one.** The demo's row was given its column widths by *stylesheet* — two
id rules and the arena cascade above to make them reach a repeated row.
It worked, and the live drive then failed on two unrelated widgets:
`ui.lint` reports `W1010` for a rule that matches nothing, and a rule
naming a node inside the `DataGrid`'s row matches nothing during the
forty-one frames when some other demo is on stage. Narrowing the lint
was the obvious move and was wrong — `Unmatched` is how a misspelled
selector is caught, with a test that says so. So the demo declares its
columns in the **type** instead (a cell is a container, and a derived
container is `FILL`), the grid owns the inset, and the sheet is not
involved at all. The arena cascade stays, because the hole it closes is
real, with its own fixture in
`crates/lumen-static-shell/tests/arena_cascade.rs` rather than a
showcase rule that would reintroduce the false positive.

**Two findings were closed as correct-as-built**, with the reason
recorded rather than a change: a press on a grid's header does nothing
because sorting is not built (a header that swallowed a press would be a
control that lies), and a `Tree` cannot be seeded collapsed because
`Tree::new` takes rows and nothing else — which is a real gap, and is
`Tree::toggle`'s job to fill when something needs it.

### T27.11, as built — four pickers, one geometry

`Picker<KIND>`: a trigger and a popup of cells. The four differ in three
things — how many columns, what cell `k` says, and what picking it means
— and in nothing else, because **a colour swatch grid, a month, a clock
and a file list are the same geometry**. That is the observation the
tranche is built on, and it held: one table, one hit test, one open/close
rule, four `type` aliases.

The cells are *painted*, not spliced. Forty-two day cells as forty-two
table nodes would put a month's data in the type, which is the thing
this architecture exists to keep out — so a pad is one node that paints
its cells and hit-tests them by arithmetic, the way `Pagination` draws
page numbers and `Strip` draws its items.

**The date arithmetic is the only real computation in the tranche**, and
it is the only place a widget can be wrong in a way nobody notices for a
month. Sakamoto's method, spelled out rather than pulled in (`chrono` is
`std`; this runs on a microcontroller), and asserted against five dates
a person can check — **cross-checked against Python's `datetime` in the
same session**, because a test that agrees with the implementation that
produced its expectations is not a test.

Eight injections (CP–CW) against the unit tests, four more (CX–DA)
against the live drive. **Two left a test green**, and both were the
same shape:

  * the month header's paging arm returns *before* the cell arithmetic
    is reached, so an injection that dropped the header's height from
    `cell_at` could not be seen through a press on the header. The test
    now presses the row *below* it and names the day: September 2026
    starts on a Tuesday, so the first week's Wednesday is the 2nd, and a
    grid that began its rows at the pad's top answers with the 9th.
  * nothing pressed a **blank** lead-in cell, so "a blank picks nothing"
    was never exercised. It is now, and the assertion is that the press
    is *not taken* — which is also what tells the overlay it may close.

**Three things the render caught that no test would have.** The swatch
grid was six columns wide over a palette of four lightnesses per hue, so
its first row read grey, grey, grey, grey, red, red — twenty-four
colours and not a palette; it is four columns now, one hue per row. The
month grid had no weekday strip, so the only way to tell which column
was Monday was to count from a date you already knew. And the clock had
no seam between its hours and its five-minute marks, so `18 19 20 21 22
23` sat directly above `:00 :05 :10` with nothing saying they meant
different things. All three are §3b's ghost-button lesson in a new form:
*the frame is the artifact, and reading the code does not render it.*

**And one gap found while drafting, fixed first**: `Style` gained
`border_width` and `border_color` in T27.1 and `Patch::fields` never
learned about them, so a rule that set only a border reported the empty
set and `SheetIssue::Unread` could never fire for one. Every picker's
trigger is a bordered control, and its seed had no bit to declare.
`Fields::BORDER` is the seventeenth bit — which is why `Fields` is a
`u32` now — and **which kinds declare it was measured, not guessed**:
`style_reads.rs` probes every kind's output field by field and named the
two that were wrong. It also corrected the test written for it, whose
two directions had their controls backwards.

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
