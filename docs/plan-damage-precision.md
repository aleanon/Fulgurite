# M29 — what one changed value should cost: a fixture big enough to see it, then a mark that fits

> **T29.0, T29.1 and T29.5 are done (2026-09-21). T29.2–T29.4 and
> T29.6 are open, and they wait for M33** (`docs/plan-state-split.md`).
> Written 2026-09-20. §6 records the seven-lens review that corrected
> it, and the probe's numbers live in
> `docs/results-damage-precision.md`.
>
> **Amended 2026-09-23 after three decisions.** The state/tree split
> ships as M33, so the `handled` path §2.5 ruled out can now be
> narrowed. It is scheduled here as T29.6, where it had been deferred in
> §4. `Boundaries` becomes the default strategy as M34
> (`docs/plan-boundaries-default.md`), so the saving reaches a turnkey
> window. Paint culling stays shelved on §4's condition.
>
> M26's Phase D promoted finding 14 and measured it: `Host::pump_with`
> calls `mark_all()` whenever `App::update()` returns true, so **one
> `#[bind]` writing one label marks the whole tree**, and `mark_all`
> under `Boundaries` costs exactly what `Everything` costs — the
> tracking is paid for and then discarded
> (`docs/results-frame-and-types.md`, finding 14).
>
> **The prescription recorded there does not survive review, and §5 says
> how.** It reads: *"the derive knows which nodes each `#[bind]` writes,
> so `App::update` can report a range instead of a bool."* The first
> clause is false — the derive sees `ui.card.value` as **tokens** and has
> no type information — and the second is three unsolved problems in a
> sentence.
>
> **And every number above was measured on a tree too small to decide
> anything.** The arm is 901 synthetic nodes; the tally is about twenty.
> So this milestone builds the fixture first and lets it kill the work if
> the blast radius is invisible on a realistic frame.
>
> **The probe survived its kill rule by forty times the arm's spread —
> and found that the saving is unreachable on the only frames
> applications actually produce.** A consumed event calls `mark_all()`
> eighty lines before `App::update` is asked anything, so every binding
> in this workspace, all of them written by click handlers, is already
> outside what T29.3 can narrow. §2.5 is that finding; T29.3 now has to
> answer it before it is worth building.

*Each task is one commit unless it says otherwise; each ends with
`just test` green and the arm named in its **Measure** line re-run. Task
ids go in commit subjects. Every gate names the injection it must go red
under, and prints its measured quantity beside its bound.*

---

## 0. The rules for this milestone

1. **The fixture comes first, and may end the milestone.** T29.1 is a
   probe with a kill rule written before it runs.
2. **Measure on the arm the task names**, eight runs, fast and slow
   clusters separated before any spread is computed — the mode rule
   `docs/results-frame-and-types.md` records.
3. **Guard the budgets no frame number shows**: the allocation gate, the
   `.text` ceiling, the const-asserted sizes, and — the one this
   milestone is most likely to trip — **rustc's own time**, which
   `just const-budget` prints. `docs/results-const-eval.md` records an
   O(n²) const assertion that made a 3,711-node tree fail to compile at
   all; the work in T29.2 emits per-field items and is exactly the shape
   that can do it again.
4. **Docs move in the same commit**, including
   `docs/results-frame-and-types.md`, whose finding-14 prescription this
   milestone corrects.

## 1. What exists today

- `#[bind]` expands to one `format_into` per binding, which **compares
  before it writes** and returns whether it wrote. That half is already
  surgical: an unchanged binding touches nothing and the measurement memo
  survives.
- `App::update` ors those together into a **`bool`**, and
  `Host::pump_with` answers a `true` with `mark_all()` plus
  `arenas_dirty = true`.
- **And it has already done that twice by then.** The same `pump_with`
  calls `mark_all()` for a consumed event (`lib.rs:694`) and for a
  deferred message that changed something (`:767`), both *before* it
  asks `App::update` anything (`:776`) — and `Boundaries::mark` returns
  early once `all` is set. The first draft of this plan did not mention
  it, which is why §2.5 exists.
- Layout skips clean subtrees; **paint never does** (T16.8 refused), and
  `Painter::clip_bounds` has one caller in the workspace, `List::paint`
  — so a derived container paints its children whether or not they are on
  screen (M26 Phase D finding 4, closed as "true and worthless" at the
  sizes measured there).
- Virtualized rows are the exception that already works: `Repeat` and
  `List` materialize a window, so offscreen rows are not laid out,
  painted or stored.
- `Dirty<T>` already gives a widget a changed-bit, and `Label` uses it
  for the measure memo.
- **`Repeat::layout` consults nothing**: it re-binds and re-lays its
  whole window every frame, clean or not. Measured (T29.5), that is
  **~1.5 µs for 18 rows** and was decided against — the expensive thing
  was never the repeater but the *bind it is given*, which allocates if
  written the obvious way.
- **And none of this is on by default.**
  `lumen_static_winit::Inval` is `Everything` unless the non-default
  `boundaries` feature is set, so a turnkey window marks everything
  anyway and *nothing in this milestone changes its frame*. M26/T26.x
  found the same thing — no example, fixture or test had ever
  instantiated `Boundaries` — which is why the feature exists at all.
  **A narrower mark is worth zero until that default flips**, and
  flipping it is a decision with its own evidence to gather (four of
  M26's five Phase A defects were invisible under `Everything`, so the
  strategy that needs this work is also the less-exercised one).
  **Decided 2026-09-23: it flips.** The evidence and the flip are M34
  (`docs/plan-boundaries-default.md`).
- The invalidation oracle compares byte-identical frames under
  `Everything` and `Boundaries` over a driven session, and its fixture
  now contains an enum (M26/T26.1). **That is the gate a narrower mark
  rides on**, and it exists already.

## 2. The decisions

### 2.1 The fixture is a spreadsheet, and it needs both halves

Nothing in the tree can show this. The tally is ~20 nodes; the showcase
is a picker and one demo; `just stages` drives 10,000 rows but they are
**virtualized**, so they are not on the table and `#[bind]` never touches
them.

A grid fixture exhibits the defect only if it has:

- **Thousands of *table* nodes** — column headers, a toolbar, a formula
  bar, a status row, frozen columns. These are what a `mark_all` walks,
  and they are the half `just stages` does not have.
- **A virtualized body**, through `Repeat`, because that is how a
  spreadsheet is really built and it keeps the node count honest rather
  than declaring ten thousand struct fields.
- **At least one `#[bind]`-driven label that changes per edit** — the
  formula bar, or a "Sum: …" status. That single value changing is the
  event whose blast radius is the subject.
- **Real text**: varied cell contents, not one string repeated. Shaping
  dominates a real frame (`docs/results-competitive.md`), and a fixture
  of identical short strings would flatter a layout-only win.

**The trap, named because it would make the whole milestone vacuous:** if
the grid is *entirely* virtualized rows, a cell edit never goes through
`#[bind]` at all, and the fixture measures nothing. The bound label must
be on the table with the bulk of the nodes around it.

### 2.2 Tracking belongs at the binding site, not inside the widget

A wrapper in the widget can know *that* it changed — `Dirty<T>` does —
but not **where it is**. Widget types are position-independent: one
`Label` type sits at many indices, and the index arrives as a parameter
during a pass. Giving it one costs either

- a per-widget index field — per-node bytes, the trade M25 spent a
  milestone refusing; or
- a per-frame "are you dirty" walk — a virtual call per node per frame,
  which is the objection written into `App::tick`'s own rustdoc.

There is also an ordering problem: a container decides to skip a child
**before** calling into it, so a bit inside the child is read too late.

The binding site is where the path is known, so that is where the mark is
computed.

### 2.3 The index is a const — but **not** the way the first draft said

The blocker is that a proc macro has no type information: the `App`
derive sees `ui.card.value` as tokens and cannot know that `card` is a
`Card`, so it cannot reach that type's table offsets.

**The first draft answered "associated types", and the review showed
that cannot be written.** Per field, the `StaticTree` derive would emit
a marker type and an impl carrying the field's offset and its type; a
bind site would compose along the path by naming only the first type.
Hop 1 is spellable — the derive holds the `#[tree]` field's type as
tokens. **Hop 2 is not**: the second container arrives only as the
projection `<Ui as Field<M_top>>::Ty`, which has no path tokens, so no
marker belonging to it can be named. The two-hop path in this
milestone's own fixture (`ui.top.formula`) already fails, and
`examples/glass` is three hops.

Two routes survive, and **T29.2 must choose one before it is
scheduled**:

- **A const-generic key instead of a named marker.**
  `<Ui as Field<{key("top")}>>::Ty` composes, because the key is a
  *value* the macro can spell from a token. The design family is
  otherwise unchanged, and so is its compile-time risk (§0.3).
- **`wid_index!`, which M26 Phase C already shipped.** `index_of_wid` is
  a `pub const fn` over the table, and `wid_index!` resolves it in a
  `const` block — refusing a typo and a duplicate at compile time.
  `const I: usize = wid_index!(Ui, "formula")` plus the existing
  `subtree_range` is T29.2's whole product, with no derive work at all.
  The fixture's bound label already carries `#[widget_id("formula")]`.

**The first draft rejected the second route on a number that measures
something else.** It called the id route "a table scan — about 5 µs at
901 nodes"; `docs/results-const-eval.md` measures that figure as *901
`Node::node` calls*, not one id lookup, and M26 Phase C moved the lookup
to compile time regardless. **So the choice is no longer about cost.**
It is about what a binding names: a field path is what the author
already wrote and is unique by construction, while an id must be
authored, is not unique by construction, and `wid_index!` refuses the
duplicate rather than resolving it. That is a real argument for the
derive route and it is the one §2.3 should have been making.

**What neither route reaches: a container the derive did not write.**
`reject_generics` refuses every generic type, so `Glass<W>` is a
hand-written `Widget` with no per-field items — and
`examples/glass`'s `#[bind(ui.bar.content.sigma, …)]` is a *format*
binding, the narrowable kind, whose path crosses one. §2.4 lists it.

### 2.4 What stays conservative, permanently

- **A handled event, and an applied message.** `pump_with` calls
  `mark_all()` at `lib.rs:694` for any consumed event and at `:767` for
  a deferred message that changed something — both *before* it asks
  `App::update` anything at `:776`, and `Boundaries::mark` returns early
  once `all` is set. A handler takes `&mut S` and may write the tree
  directly, so narrowing it is not a matter of bookkeeping. **This is
  §2.5, and it is the largest of these by far.**
- **`with =` bindings.** The hook is `fn(&mut Target, &Source)` and may
  write anything reachable from the target; the derive already reports
  `true` unconditionally and says why. These keep `mark_all`.
- **`#[app(after_update = f)]`.** The hook returns a bare `bool`.
  `examples/showcase` uses one, so the flagship example keeps the full
  radius unless a hook reports ranges — which would move the work to the
  application author, and this milestone does not.
- **A path through a hand-written or generic container** (§2.3's last
  paragraph). No per-field offsets exist for it, and a format binding in
  the workspace crosses one today.
- **Arenas.** The same branch sets `arenas_dirty`; a narrowed mark must
  either keep setting it or show that a binding cannot change a
  template's styles.

A partial fix read as a general one is the failure mode here, so each of
these is asserted rather than assumed (T29.4).

### 2.5 The ordering finding: what is left to narrow

**Measured, not argued** (`docs/results-damage-precision.md` T29.1): the
`handled` arm lands on the `mark_all` row, so the saving is zero on the
frame a click produces. Every non-test `#[bind]` in this workspace is
written by a click handler.

What remains reachable, and it is not nothing:

- **`Host::tick`** marks nothing of its own, so an animation or timer
  that changes a bound value reaches the narrowed path.
- **Host-driven mutation** — `host.app.x = …` then `pump`, which is what
  an embedded main loop and both probe arms do.
- **Anything that changes a binding without an input event**: a polled
  sensor, a clock, a computed total.

What does *not*, without new work: every click, key and gesture, and
every deferred message M28 delivers.

**And one bound to keep in view:** `Boundaries<N>` holds `N` ranges and
`mark` promotes to `mark_all` when it would exceed them. That is
graceful rather than wrong, but per-binding marks are the first thing in
this repository that can produce many ranges in one frame, so T29.3
should print how many a realistic frame produces against `N` — a limit
nothing has approached is also a limit nothing has measured.

**So T29.3 must publish both numbers**, and the milestone must not
describe the saving without naming the frames it applies to. Narrowing
`handled` itself is a separate question with a real answer — a handler
that could not reach the tree would make every tree write a binding —
and it is a type-system change, not an invalidation one. **Amended
2026-09-23:** the type-system change ships as M33, and the narrowing it
enables is T29.6 below.

## T29.0 — the grid fixture — **DONE 2026-09-21**

> `benches --bin grid` (`just grid`): a frozen grid of table nodes, a
> `Repeat` body with a real viewport, varied text through a real face,
> and one `#[bind]`-driven label on the table. Every quantity is in
> `docs/results-damage-precision.md` T29.0, which owns them.
>
> **Seven ways it could not say what it said, and none was visible by
> reading it.** Three were found during the task by running the
> injection and refusing to accept that it passed. **Four more were
> found by a review after it had shipped** — green, with its gates
> already shown to fail, and still measuring something else: a body that
> virtualized by declaration only, text that never reached a shaper, a
> "layout" column that was layout + place, and two arms that both wrote
> the bound value in the one way no application ever does.
>
> The lesson is the fourth entry in `AGENT.md`'s table of methods:
> injection, printing the bound beside the quantity, and using the thing
> all missed these. **What found them was a reader asking what the
> fixture contained rather than what it declared.**

A spreadsheet-shaped example with the four properties §2.1 requires, and
a headless driver that can edit one cell and pump.

**Gate — the fixture must be able to show the thing.** Before any
baseline is taken from it, assert its own shape: the table node count is
above a stated floor, the bound label is on the table (not in an arena),
and a single-cell edit changes exactly one bound value. *Injection:*
point the binding at a virtualized row instead — the "the bound label is
on the table" assertion fires, which is the vacuous shape §2.1 names.

**Measure:** nothing yet; this task only builds the arm.

## T29.1 — the probe — **DONE 2026-09-21: the finding survives, and its scope is smaller than the number**

> `docs/results-damage-precision.md` T29.1 owns the figures. Three
> things it settled, none of them restated here:
>
> 1. **The kill rule did not fire** — the frame moved by forty times the
>    arm's spread. T29.2–T29.5 are justified.
> 2. **The saving is unreachable on a handler-driven frame**, and the
>    fixture now measures that frame beside the others. See §2.5.
> 3. **A clean frame still spends 12.2 µs re-laying out the virtualized
>    body**, because `Repeat::layout` takes no `Changed`. That is a
>    larger and cheaper win than this milestone's own, on a path the
>    `handled` ordering does not block — T29.5.
>
> And two claims the first write-up made that the review removed:
> `place` is a floor rather than residue, and paint's *share* of the
> frame is not evidence about paint's *cullable* share (§4).

### T29.1 as specified

Drive the fixture: edit one cell, pump, and measure a frame under
`Boundaries` with `mark_all` (what `#[bind]` forces today) against the
same frame with the mark narrowed **by hand** to the bound label's range
— a scaffold, not the real mechanism, so the number arrives before the
machinery is built.

Report layout, paint and raster separately, because only layout can
benefit: paint walks the whole table regardless, and the frame's
dominant cost may be shaping.

**The kill rule, written before the number exists:** if narrowing the
mark moves the *frame* by less than the arm's fast-cluster spread, the
finding is closed as promoted-but-not-worth-building, the number is
recorded beside M26's 9.7×, and T29.2–T29.4 do not happen. A layout-only
win invisible in a frame is not a win.

**Gate:** the hand-narrowed run must produce a **byte-identical** frame
to the `mark_all` run — the oracle's property, applied to the scaffold.
If it does not, the narrowing is wrong and the probe is measuring a
broken frame. *Injection:* narrow to the wrong range — the byte-identity
assertion fires.

## T29.2 — the const index — **DONE 2026-09-24, by neither route: an address**

> **Picked, with the argument.** §2.3's two routes both fail on the
> workspace's own bindings. A per-field const cannot cross a
> hand-written or generic container, and `examples/glass` binds through
> `Glass<W>`. `wid_index!` needs an author id on every target, and a
> value binding's target is often not a widget at all: the popup's
> `open` is a field inside one.
>
> **`bind::node_of`** takes the target's *address* and returns the
> deepest table node whose memory contains it, which is the widget that
> owns it, whatever container sits between. **`bind::BindSite`** caches
> the answer per binding site, as a `static`, so the walk runs once per
> build and a frame pays one atomic load.
>
> **Indices that resolve to their container** (an inactive variant, a
> `None`) are skipped by resolving to the same address, size *and kind*
> as their parent. Address alone is wrong, because a struct's first
> field shares its address.
>
> **Gate:** `crates/lumen-static/tests/bind_index.rs`. The oracle is
> the table's **author ids**, which no address arithmetic can move with
> an injection. The cases are a nested label, a single-field parent, an
> enum's active variant followed by an inactive one, and a field inside
> a widget.
> *Injections:*
> - **Drop the skip.** The enum case names index 10 where the id says 8.
> - **Skip on address alone.** `formula` names its parent, 1, where the
>   id says 2.
>
> **Measure:** `just const-budget` is moot, because the route emits no
> per-field items.

### T29.2 as specified

§2.3's mechanism, by whichever of its two routes the task picks — and
**picking is the first half of the task**, with the argument written
down, because the draft that named associated types could not have been
built.

**Gate:** for every field path in the workspace's fixtures, the const
index names the same node as the value itself. **The oracle is pointer
identity, not arithmetic** — `&app.ui.top.formula as *const _ as *const ()`
against the data pointer `app.tree().node(IDX)` answers with. This is
stated because the obvious oracle is vacuous: `Node::node`'s body
accumulates `at += <#field_ty>::NODE_COUNT`, the *same sum* T29.2 would
emit, so an oracle that recomputes the offset moves with the injection
and agrees with it. *Injection:* omit a preceding sibling's node count
from the offset sum — the identity assertion fires, naming the path.
Both indices print side by side.

**And a refusal, not a wrong answer.** `#[bind]`'s target is an
unconstrained `syn::Expr`; the derive accepts whatever parses. Any path
T29.2 cannot resolve to a const — through a hand-written container, a
generic, an index expression — must produce a **compile error or a
documented fall back to `mark_all`**, chosen deliberately and tested.
Silently resolving the wrong index is the one outcome that produces a
frame nobody can debug.

**Measure:** `just const-budget`, four builds. **Its recipe must first
be made to compile the tree it claims to stress** — it touches
`fixtures/const-budget` and builds that crate alone, which reaches
3,711 nodes from four derives totalling 31 declared fields, while the
quantity T29.2 grows is *declared fields per derived type* and the
workspace holds ~380 derives. A budget that cannot see the work is not
a budget.

## T29.3 — `App::update` reports what it wrote

`update` feeds the invalidation set instead of returning a bare `bool`:
one range per binding that actually wrote, the conservative paths of
§2.4 marking everything as they do now.

**First, the scope question §2.5 raises**, because it decides whether
the rest is worth building: publish the `handled` number beside the
host-driven one, and say in the commit which frames the saving reaches.

**Gate:** the invalidation oracle — byte-identical frames under
`Everything` and `Boundaries` across a driven session on a grid-shaped
tree. **The fixture has to move first:** `benches --bin grid` is a
binary, so no test crate can import it, and T29.0's shape assertions run
in its `fn main`. Lift the tree into a module a test can reach, and add
`just grid` to what CI runs, or this gate cannot be written at all.

*Injection:* **not** "mark the bound node but not its ancestors" — the
plan's first draft named that one and predicted in the same breath that
it would leave the gate green, because `is_clean` tests intersection and
a parent's range contains the child's. A gate whose injection is
specified to pass is not a gate. The injection that fires is **mark the
bound node's range off by one** — `i..i` , or a sibling's range — so the
label is skipped while the frame claims to be clean, and the oracle sees
the stale box. The reasoning about ancestors goes in a comment, where an
argument belongs; the gate keeps an injection that can go red.

**Measure:** T29.1's arms, now with the real mechanism — **and the
scaffold arm has to survive the change**. `arm_b` narrows by hand
*because `arm_a` does not*; once `update` reports ranges, both arms
narrow and the comparison collapses into itself. Keep a control that
forces `mark_all` (an explicit `invalidate_all()` before the pump), so
the arm still measures the difference it is named for. The number must
land where the hand-narrowed scaffold did.

## T29.4 — the conservative paths, asserted

`with =`, `after_update`, a path through a hand-written container, a
handled event, an applied message, and the arena flag each keep
`mark_all` — **six paths, six assertions**, so a later reader cannot
mistake the partial fix for a general one.

**Gate:** each path gets its own test with an **observable quantity**,
not a bare "it still works": the count of ranges the invalidation holds
after the frame, printed beside its bound — 0 ranges with `all` set for
a conservative path, exactly 1 for a narrowed one. Both directions in
one test, per `AGENT.md`'s rule that a gate which refuses bad input must
also accept good input. *Injection:* narrow the `with =` path — the
"still marks everything" assertion fires, naming the path.

**Docs:** `docs/results-frame-and-types.md`'s finding-14 prescription is
corrected in this commit — it is the sentence that sent this milestone
down the wrong path, and it should not outlive the correction.

## T29.5 — the repeater's bind, not the repeater — **DONE 2026-09-21**

> **This task's own premise was wrong, and measuring it is what said
> so.** T29.1 reported 12.2 µs of "work nothing asked for" in
> `Repeat::layout`. Split four ways
> (`docs/results-damage-precision.md` T29.5): **10.8 µs was the
> fixture's own `format!`** — one heap allocation per row per frame —
> and the framework's re-bind of an 18-row window is ~1.5 µs, with the
> per-row `layout` call very nearly free because the measure memos hold.
>
> **What shipped** is the fix that needed no contract change:
>
> - `alloc_gate.rs` gained the arm it never had. Both its repeater arms
>   bound with `noop_bind`, so its zero was a zero for a list whose rows
>   are never filled in — the only thing that allocates on a repeater
>   frame is a bind that *writes*. The new arm's injection is the
>   ordinary spelling, and it fires at **19 allocs/frame on a 19-row
>   window**, one per row.
> - `Repeat::bind`'s rustdoc says the thing an author needs at the
>   moment they write one: this runs every row every frame, so it must
>   not allocate, and `format_into` is the tool — the same machinery
>   `#[bind]` generates for a table node.
> - `benches --bin grid`'s own bind was the offender and is fixed, which
>   moved every figure in T29.1.
>
> **Decided against, with its number:** skipping the bind loop on a
> clean frame. It is worth ~1.5 µs, and it would make a list silently
> stale for any application that mutates data without marking — working
> under the `Everything` default and failing under `Boundaries`, which
> is the worst shape a difference can have.
>
> **The lesson is §6's, one layer down.** T29.1 found a real 12.2 µs and
> attributed it to the nearest plausible mechanism, which was in the
> file it had just been reading. Nothing in the measurement was wrong;
> the *story* was, and only splitting the quantity four ways could tell
> them apart.

## T29.6 — a handled event narrows, for a split application

**Needs M33 and T29.3. Its *Measure* also waits for M34/T34.2**
(`docs/plan-boundaries-default.md`), but its gate does not. M33 puts
the tree out of a handler's reach and gives `App` a constant that says
so. T29.3 makes `update` report one range per binding that wrote. With
both in place, a consumed event no longer has to mean `mark_all`.

**What still has to be marked: the nodes the event itself changed.** A
handler writes only the state, but `dispatch_overlaid` also delivers
the event to widgets, and some of them write themselves. A `TextInput`
inserts a character, a `Slider` moves on a track press or an arrow key,
and a `Chooser` opens itself (`Select` stops doing so in M33/T33.3). A `Toggle` does **not** flip itself: it has no
`on_event`, and its `on` is written back by a binding. So dispatch has
to report **a set of consumed nodes, possibly empty**. It does not
report one today: `dispatch_overlaid` returns a bare `bool`. The set
has to cover every consuming path:

- the node that consumed the event, in pass 1 or under pointer capture;
- an overlay's own `on_event`;
- **every overlay a dismissal closed**. Escape closes the topmost, and a
  press outside closes all of them in a loop, so this can be several
  nodes;
- **no node at all** for focus moves (Tab, arrow groups, the overlay Tab
  trap). T25.0's focus and pointer marks already cover those, and an
  empty set means exactly that.

**What stays conservative.** These are T29.4's six paths, named so the
two lists cannot drift apart again:

1. a `with =` binding;
2. an `#[app(after_update)]` hook;
3. a path through a hand-written container;
4. a handled event, **only when `State = Self`**, answered by M33's
   constant;
5. an applied message, only when `State = Self`. After M33/T33.2,
   `apply` receives the state, so a split application's message frames
   narrow the same way its handled frames do, and the gate below sends
   one;
6. `arenas_dirty`, **which a narrowed handled frame still sets**. A
   consumed wheel moves a `Repeat`'s window, and the arena cascade is
   what restyles the new rows. The measurement reports the cascade's
   share of the frame, so the cost of that conservatism is visible.

**T29.4 is rescoped by this task, not extended.** Its handled-event and
applied-message assertions become "`mark_all` when `State = Self`,
narrowed when split". Both halves are asserted, and each has its own
injection.

**Gate:** the invalidation oracle, byte-identical frames under
`Everything` and `Boundaries`, over a **click-driven** session on a
split fixture. The session:

- focuses a `TextInput` (asserting that it is focused) and types into it;
- presses a `Slider`'s track;
- dismisses two open overlays with one outside press;
- sends a deferred message;
- presses a button whose handler changes a bound label **outside the
  button's own subtree**, and asserts that it is outside.

*Injections:*

- **Mark only the binding ranges, not the consumed set.** What goes
  stale is not the typed character, because paint draws `self.text`
  every frame. It is the field's layout: the caret x and the scroll
  offset. The oracle fires, and the gate prints those values beside the
  `Everything` frame's.
- **Report only the first overlay a dismissal closed.** The second
  overlay's area goes stale.
- **Narrow for an unsplit application.** An unsplit twin of the fixture,
  whose handler writes a tree label outside the pressed subtree, goes
  stale.

**Measure:** the `handled` arm against T29.1's **`narrowed`** arm
(`arm_b`). They are not expected to be equal: a click also marks the
pressed button's subtree and T25.0's hover node. So the bound is
`narrowed`'s fast-cluster spread plus the cost of those extra ranges,
measured separately and printed beside the result. Publish both, and
state which frames the saving now reaches.

## 3. What this is not

**Not paint culling.** A derived container still paints offscreen
children; M26 Phase D closed that as true and worthless at the sizes it
could measure. The grid fixture is bigger than anything it had — and
measured, it still does **not** meet that finding's condition, which is
about off-surface nodes and not about paint's share of the frame (§4).

**Not a reactive graph.** Nothing observes reads; the mark comes from the
one place that already knows both the value and the path.

**Not a change to `#[bind]`'s comparison.** That half is already
surgical.

## 4. Deferred, with conditions

- **Paint culling for table containers. The condition is still not
  met**, and the first write-up of T29.1 said it was. **Decided
  2026-09-23: it stays shelved on that condition.** That deferral's
  condition is **off-surface node count**; paint's *share of the frame*
  is a different quantity, and this fixture has 220 off-surface nodes of
  2,227 — 9.9% (`docs/results-damage-precision.md` T29.1). A
  `clip_bounds` cull keeps every on-screen node, including the ~1,400
  whose boxes are degenerate, so the 28.6 µs paint walk is not what the
  share suggests. **Reopen when a fixture puts most of its nodes off the
  surface** — a grid scrolled to the middle of a large sheet would, and
  this one does not. **And price the correctness first, not the
  frame:** out-of-flow content is zero-area *in flow*, so a container
  that culls by its child's rect would skip an on-screen popup and never
  call `defer` at all. T16.8 refused a paint skip for reasons that have
  not changed just because the tree got bigger.
- ~~**Narrowing the handled-event path (§2.5).**~~ **Scheduled
  2026-09-23 as T29.6**, behind M33 (`docs/plan-state-split.md`), which
  is the type-level change this entry asked for.
- **Ranges from `after_update` hooks.** When an application has a hook
  whose blast radius is measured to matter.
- **A dirty bit that propagates from the widget.** Only if a design
  appears that needs neither a per-node index nor a per-frame walk.

## 5. What reviewing my own prescription found

The sentence in `docs/results-frame-and-types.md` — *"the derive knows
which nodes each `#[bind]` writes, so `App::update` can report a range
instead of a bool"* — has five defects, found by checking it against the
code rather than by re-reading it:

| the prescription says | what the code says |
|---|---|
| the derive knows which nodes a binding writes | it sees **tokens**. `&mut self.#target` is pure substitution; a proc macro has no type information, so `ui.card.value` cannot be resolved to an index without §2.3's machinery |
| return a **range** | several bindings can change in one frame, so it is a **set**, which means feeding the invalidation rather than returning — an `App` trait change touching hand-written impls |
| — | **`with =` can never be narrowed**: the hook may write anything reachable from the target, which is why it already reports `true` unconditionally |
| — | **`after_update` defeats it for `examples/showcase`**, whose hook returns a bare bool |
| the win is 9.7× | that is **layout-only**, on 901 synthetic nodes. Paint walks the whole table either way, and no real frame has been measured. Hence T29.1's kill rule |

The direction survives — the defect is real and measured in the right
frame — but the fix is four tasks behind a fixture, not one line.

## 6. What the seven-lens review found (2026-09-21)

Run after T29.0 and T29.1 shipped, against the code rather than the
prose: seven lenses, an adversarial refuter per lens defaulting to
REFUTED, 56 findings judged, **49 confirmed, 1 plausible, 6 refuted**.
The plan keeps the corrections rather than quietly replacing itself,
which is the convention M22 §12 and M28 §5 set.

**Three blocking, and each was a different kind of wrong:**

| finding | kind |
|---|---|
| a consumed event calls `mark_all` eighty lines before `App::update` is asked anything, so no binding in any existing application reaches the path this milestone narrows | **the premise**. Found independently by two lenses; §2.5 |
| §2.3's associated-type chain cannot be named past the first hop | **the mechanism**. §2.3 now offers two routes that can be |
| T29.3's only injection was one the plan itself predicted would leave the gate green | **the gate**. A plan that writes down a vacuous injection has documented it, not avoided it |

**What it corrected about the measurement** is in
`docs/results-damage-precision.md` under "What the review corrected" —
four properties this plan's §2.1 *required* that the fixture satisfied
by declaration rather than in the frame, and a column that was two
passes folded into one.

**Two lessons worth more than the findings.** The first: *a green
fixture whose gates have been shown to fail can still be measuring
something else.* T29.0 did everything `AGENT.md` asks — injected faults,
watched them go red, printed bounds beside quantities — and four of its
seven defects survived all of it, because every one of them was a
question nobody had asked the fixture (how many rows did you
materialize? did anything shape? which passes are in that column? who
wrote the value?). **Asking what a fixture contains is a distinct method
from asking whether its assertions can fail.**

The second: the review's most valuable output was not a defect in the
plan but **a defect the plan's own fixture had made visible and nobody
had looked for** — T29.5's 12.2 µs. The fixture was built to answer one
question and answered a larger one as a side effect, which is the
argument for building the fixture before the mechanism, stated better
than §0.1 stated it.

## 7. What the 2026-09-23 review found in the amendment

A seven-lens review of the M32–M36 plans, with an adversarial verifier
per finding, confirmed eight findings against this document's
2026-09-23 amendment. What changed:

| finding | what changed |
|---|---|
| `dispatch` returning "the node" cannot express a dismiss-all (several nodes) or a focus move (none), and the host calls `dispatch_overlaid`, not `dispatch` | T29.6: a set of consumed nodes, possibly empty, with every consuming path named |
| T29.6's "T29.4's six paths" listed five, and T29.4's handled-event assertion becomes false for a split application | T29.6 lists all six, and rescopes T29.4 |
| a `Toggle` does not flip itself (a binding writes `on`) | the `Toggle` is removed from the premise and the gate. A `Slider` is the second witness |
| the predicted symptom was wrong: paint draws the text every frame, and what goes stale is the layout (caret x, scroll) | T29.6's injection names the actual symptom and asserts focus first |
| "the host-driven arm" was ambiguous, and the results doc gave the `handled` arm two different figures | the comparison is `arm_b` (`narrowed`) with a stated bound. The results doc's stale prose now points to its table |
| T29.6's measurement depends on M34, which only the queue said | stated in T29.6 |
| whether a narrowed handled frame keeps `arenas_dirty` was unstated | kept, and its share of the frame reported |
| the §4 sentence about paint culling was spliced so that it misread | repaired |
