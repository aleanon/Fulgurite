# M29 — what one changed value should cost: a fixture big enough to see it, then a mark that fits

> **Not started. Written 2026-09-20.**
>
> M26's Phase D promoted finding 14 and measured it: `Host::pump_with`
> calls `mark_all()` whenever `App::update()` returns true, so **one
> `#[bind]` writing one label marks the whole tree**, and `mark_all`
> under `Boundaries` costs exactly what `Everything` costs — the
> tracking is paid for and then discarded. **6.22 µs, 9.7×**, on every
> frame where a binding changed anything
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

### 2.3 The index is a const, and associated types do what the macro cannot

The blocker is that a proc macro has no type information: the `App`
derive sees `ui.card.value` as tokens and cannot know that `card` is a
`Card`, so it cannot reach that type's table offsets.

**Associated types resolve it.** The `StaticTree` derive emits, per
field, a marker and an impl carrying two things: the field's start offset
within the container's table, and the field's **type**. A binding site
then composes offsets along the path by naming only the *first* type —
each step's associated type supplies the next. The result is a `const`
index, so the runtime cost is zero.

What it costs is **compile time**: one marker and one impl per field per
container, which is why §0.3 makes `just const-budget` a gate on this
task rather than a note.

**The runtime alternative is worse and is recorded as rejected:**
requiring `#[widget_id]` and looking the index up is a table scan —
about 5 µs at 901 nodes (`docs/results-const-eval.md`), the same order as
the 6.22 µs being saved. It is only viable cached, and cached it is
strictly worse than a const.

### 2.4 What stays conservative, permanently

- **`with =` bindings.** The hook is `fn(&mut Target, &Source)` and may
  write anything reachable from the target; the derive already reports
  `true` unconditionally and says why. These keep `mark_all`.
- **`#[app(after_update = f)]`.** The hook returns a bare `bool`.
  `examples/showcase` uses one, so the flagship example keeps the full
  radius unless a hook reports ranges — which would move the work to the
  application author, and this milestone does not.
- **Arenas.** The same branch sets `arenas_dirty`; a narrowed mark must
  either keep setting it or show that a binding cannot change a
  template's styles.

A partial fix read as a general one is the failure mode here, so each of
these is asserted rather than assumed (T29.4).

## T29.0 — the grid fixture — **DONE 2026-09-21**

> `benches --bin grid`: 2,226 table nodes of frozen grid, 5,000
> virtualized rows, varied cell text, and one `#[bind]`-driven label on
> the table. Both gates shown to fail
> (`docs/results-damage-precision.md` T29.0).
>
> **The fixture could not fail twice before it could.** Its identity gate
> first compared `"= 0"` with `"= 7"` — the same character count, so a
> stale box is the same size and the arms agree however wrong the mark
> is. Fixed, it still passed: the bound label hung **directly off the
> root**, and a container lays out *all* its children, so any mark
> anywhere relaid it out and no stale box could exist. The label now sits
> in a toolbar container, which is both what makes the gate able to fail
> and what a spreadsheet actually looks like.
>
> Neither was visible by reading the fixture. Both came from running the
> injection and refusing to accept that it passed.

A spreadsheet-shaped example with the four properties §2.1 requires, and
a headless driver that can edit one cell and pump.

**Gate — the fixture must be able to show the thing.** Before any
baseline is taken from it, assert its own shape: the table node count is
above a stated floor, the bound label is on the table (not in an arena),
and a single-cell edit changes exactly one bound value. *Injection:*
point the binding at a virtualized row instead — the "the bound label is
on the table" assertion fires, which is the vacuous shape §2.1 names.

**Measure:** nothing yet; this task only builds the arm.

## T29.1 — the probe — **DONE 2026-09-21: the finding survives**

> | stage | `mark_all` | narrowed | delta |
> |---|---:|---:|---:|
> | layout | 22.95–23.12 µs | **2.39–2.42 µs** | **9.6×** |
> | paint | 27.72–28.36 µs | 27.68–27.83 µs | unchanged |
> | raster | 1.51–1.63 µs | 1.49–1.54 µs | unchanged |
> | **frame** | **52.20–53.12 µs** | **31.60–31.79 µs** | **−20.6 µs, 39%** |
>
> The kill rule wanted the frame to move by more than the arm's
> fast-cluster spread (0.92 µs). It moved **twenty-two times** that, so
> T29.2–T29.4 are justified and the 9.7× M26 measured on 901 synthetic
> nodes holds up at 2,226 real ones — as a **layout** figure, which is
> 39% of this frame rather than of any frame.
>
> **Two things the probe settled that were not its question.** Paint is
> now 88% of the narrowed frame and is unchanged between the arms, which
> meets §4's reopen condition for paint culling with a number. And raster
> is already surgical: one edit damages 512–640 px of a 1.26 M px surface
> and costs 1.5 µs — the stage the original question guessed at is the
> one that was never the problem.

## T29.1 — the probe: is the blast radius visible on a real frame? *(as specified)*

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

## T29.2 — per-field offsets, as consts

§2.3's markers and impls, emitted by the `StaticTree` derive, with a
composition helper the binding site can use.

**Gate:** for every field path in the workspace's fixtures, the const
index equals what a runtime lookup answers — the two agree or the const
is wrong. *Injection:* omit a preceding sibling's node count from the
offset sum — the agreement assertion fires, naming the path.

**Measure:** `just const-budget`, four builds, against the figure it
prints today. §0.3's budget is the bound, and the grid fixture is the
tree that stresses it.

## T29.3 — `App::update` reports what it wrote

`update` feeds the invalidation set instead of returning a bare `bool`:
one range per binding that actually wrote, the conservative paths of
§2.4 marking everything as they do now.

**Gate:** the invalidation oracle — byte-identical frames under
`Everything` and `Boundaries` across a driven session on the grid
fixture, which is where a too-narrow mark shows as a stale frame.
*Injection:* mark the bound node but not its ancestors' range — the
oracle fires. (It should *not*: `is_clean` tests intersection, so a
parent's range contains the child's. The injection is what proves that
reasoning rather than asserting it.)

**Measure:** T29.1's arm, now with the real mechanism. The number must
land where the hand-narrowed scaffold did.

## T29.4 — the conservative paths, asserted

`with =`, `after_update` and the arena flag each keep `mark_all`, and
each gets a test that says so — so a later reader cannot mistake the
partial fix for a general one.

**Gate:** an app whose only binding is a `with =` still marks everything;
the same app converted to a format binding marks one range. Both
directions, in one test. *Injection:* narrow the `with =` path — the
"still marks everything" assertion fires.

**Docs:** `docs/results-frame-and-types.md`'s finding-14 prescription is
corrected in this commit — it is the sentence that sent this milestone
down the wrong path, and it should not outlive the correction.

## 3. What this is not

**Not paint culling.** A derived container still paints offscreen
children; M26 Phase D closed that as true and worthless at the sizes it
could measure. The grid fixture is bigger than anything it had, so §4
records the condition for reopening rather than this plan absorbing it.

**Not a reactive graph.** Nothing observes reads; the mark comes from the
one place that already knows both the value and the path.

**Not a change to `#[bind]`'s comparison.** That half is already
surgical.

## 4. Deferred, with conditions

- **Paint culling for table containers. The condition is now met**
  (T29.1, 2026-09-21): paint is 27.7 µs of a 31.6 µs narrowed frame,
  88%, unchanged between the arms. M26 Phase D closed it as "true and
  worthless" on arms 2.5× smaller. It is the next 20 µs, and it wants its
  own task rather than absorption into this one.
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
