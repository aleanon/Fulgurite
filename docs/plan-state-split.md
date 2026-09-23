# M33 — state apart from the tree, so a handler cannot write a widget

> **DONE 2026-09-24 — T33.0–T33.4.** §5 records what building it
> changed, including a pass defect it found on the way.
>
> **Written 2026-09-23. Revised the same day after a
> seven-lens review** (§4), and after the decision to **redesign
> showcase** rather than leave it unsplit. Both halves exist as
> prototypes on `proto/state-split` (`d861f4f`, `a1ded79`). The decision
> to ship them was made on 2026-09-23. **This milestone must land before
> M29's T29.2–T29.4 and T29.6** (`docs/plan-damage-precision.md`),
> because it is what makes them apply to the frames applications
> actually produce.
>
> M29 measured what narrowing a binding's mark is worth. It is a large
> share of a frame on the grid fixture, and **nothing on any frame a
> click produces** (`docs/results-damage-precision.md` T29.1, the
> `handled` arm). `Host::pump_with` calls `mark_all()` for every
> consumed event before `App::update` is asked anything. The comment
> there explains why: a handler is `fn(&mut App, &EventCtx)`, so it may
> write any field, the tree included.
>
> With a separate `State`, a handler is `fn(&mut State, &EventCtx)`.
> **Every tree write the *application* makes then comes from a
> binding**, and a binding knows which node it writes. Widgets that
> write *themselves* when they consume an event, such as a `TextInput`
> taking a character, are a different path, and M29/T29.6 marks them
> from dispatch's set of consumed nodes.
>
> **The first draft claimed that no handler in any example writes the
> tree. That was false**, and the claim came from the prototype's commit
> message without being checked. showcase's row handler closes the
> dropdown and rewrites four labels, tasks-demo's handlers call a
> `refresh()` that writes two labels, and glass's handlers write a blur
> radius. §2.4 and T33.3 are the redesign this requires.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red.*

---

## 0. The rules for this milestone

1. **Derived applications are unchanged; hand-written impls are
   source-breaking.** Without `#[state]`, the derive emits
   `type State = Self`. A required associated type still breaks every
   hand-written `App` impl, including ones in other workspaces, and the
   milestone counts and fixes them all (T33.0).
2. **The split and the source path land together, or neither is
   worth it.** Without `#[bind]`'s source path, a split application can
   only bind through `with =`, which M29 §2.4 lists as permanently
   unnarrowable.
3. **This milestone does not narrow anything.** It changes types.
   Narrowing is M29/T29.6.

## 1. What exists today

- On `main`: `App::Tree: Handlers<Self>`, and dispatch calls
  `h(app, &ctx)`.
- On `proto/state-split`: `App` gains `type State` and `state_mut()`,
  `Tree: Handlers<Self::State>`, and dispatch calls
  `h(state.state_mut(), &ctx)`. `examples/counter` is split into
  `Counter`, which holds the tree and the bindings, and `Tally`, which
  holds what a handler may touch.
- `#[bind(ui.value, count, "count: {}")]` on a `#[state]` field formats
  `self.state.count`.
- **The branch point is `29c1238`**, after all of M31. The branch's own
  `b7ddf07` duplicates `main`'s `25f360d` and drops out on rebase. The
  only real source of conflicts is M32, which the queue puts first.
- **Hand-written `App` impls:** the branch touches 29, and adds five
  lines to each, not two. It misses one in a separate workspace,
  `benches-competitive/arms/stages`, which `just stages` builds and
  `just test` never sees. The prototype's own rustdoc on `App::State`
  says "eleven". Three counts that disagree, so T33.0 counts
  mechanically.
- **The derive always emits `Msg = Never`**, so every application that
  defers work, tasks-demo and M28's chain fixtures included, has a
  hand-written `App` impl.
- `App::apply` takes `&mut self`. `App::tick` takes `&mut self`, and
  `Host::tick` marks nothing of its own (M29 §2.5).
- **Handlers that write the tree today:** showcase's `choose` (calls
  `picker.close()`, then `rebuild` writes `note`, `trigger.label`,
  `blurb` and `readout`); tasks-demo's `on_cancel` and `on_toggle_sub`,
  and its `apply`, all through `refresh()`; glass's `less` and `more`
  (`material.blur`, an `f32`).

## 2. The decisions

### 2.1 What "cannot reach the tree" promises, and what it does not

The promise is structural: `State` and the tree are sibling fields, so
`&mut State` cannot name a widget on the tree. **It does not cover
interior mutability shared between the two.** An `Rc<Cell<_>>` held by
both a widget and the state would let a handler change what a widget
shows with no binding involved. The rustdoc on `App::State` says so.
The invalidation oracle, which T29.6 runs, is the backstop.

### 2.2 A constant says the tree is split, and the derive refuses misuse

M29/T29.6 may narrow only when handlers cannot reach the tree. The
derive emits a `const` on `App` that is true when `#[state]` is present.
It defaults to false for hand-written impls. The derive **refuses** two
misuses, each with a `compile_fail` test and a passing twin:

- `#[state]` on the `#[tree]` field, which would make `State` the tree;
- a second `#[state]` field (the prototype silently took the first).

**A hand-written impl that sets the constant to true is making a
promise nothing checks.** tasks-demo has to do exactly that, because
the derive cannot express its `Msg`. So the rustdoc names the promise,
and T29.6's oracle over tasks-demo is what checks it.

### 2.3 `apply` receives the state; `tick` and `after_update` do not

A deferred message is handler-shaped work, so the signature becomes

```text
fn apply(state: &mut Self::State, m: Self::Msg) -> bool
```

and `pump_with` calls `A::apply(self.app.state_mut(), m)`. The outbox
stays reachable through `App::outbox`, which a split application
forwards to a field of its state. `tick` and `after_update` are the
application's own hooks and keep `&mut self`.

**An open question for M34/T34.0, not decided here.** `Host::tick`
marks nothing, so a `tick` that writes the tree *directly*, rather than
through a binding, is unmarked under `Boundaries` today. That is an
inventory item for M34, not a change this milestone makes.

### 2.4 Two new tools the migrations need

**A value binding.** `#[bind(target, source)]` with no format string
copies `source` into the target when they differ (`PartialEq`), and
reports whether it wrote, exactly as the format form does. The target
implements a small trait for the value it accepts. That covers glass's
`f32` blur and `Select`'s `open`. Parsing needs no lookahead: a
string-literal third argument means the format form, and none means
the value form.

**`Select` becomes a controlled widget, following `Toggle`'s
precedent.** A `Toggle` does not flip itself: a click calls the
application's handler, and `on` is written back by a binding. A
`Select` today opens itself when its trigger is pressed, and the
overlay pass dismisses it. With `open` owned by the state, both of
those would fight the binding, and the next frame would reopen or
close it again. So the trigger's press calls an `on_toggle` handler,
dismissal (Escape or a press outside) calls an `on_dismiss` handler,
and `open` is value-bound. That is the showcase redesign the user asked
for. It also settles M24's "closing is the application's", because
opening becomes the application's too.

## T33.0 — the split lands

Rebase `d861f4f` and add §2.2's constant and refusals.

- **Count mechanically:** `git grep` for `impl .*App for` across *both*
  workspaces, with the command in the commit. Fix the stale count in
  the rustdoc.
- **Build `benches-competitive`'s `stages` arm** (`just stages`) as part
  of the gate, since `just test` cannot see it.

**Gate:** a pair of doctests on `App::State`, **identical except for the
handler's body**. The `compile_fail` one writes `t.ui.title.set(..)`.
The passing one writes `t.count += 1`. Stable rustdoc does not check
the error code a `compile_fail` block names (the review showed a block
failing with E0308 passing as `compile_fail,E0609`), so the pairing is
what makes the refusal specific, and **the commit records the observed
rustc message** (E0609, `no field 'ui' on type '&mut Tally'`). The
§2.2 refusals get the same pairing.

## T33.1 — `#[bind]` learns a source path, and a value form

`a1ded79`, rebased, plus §2.4's value form. The attribute table moves in
the rustdoc and in the book in the same commit.

**Gate:** the prototype's two tests. The one that matters is
`a_source_path_binding_still_reports_precisely`: a second update that
wrote nothing must answer `false`. The value form gets the same test.
*Injections:*

- **Ignore the source path.** It fails to compile, because `Nested` is
  not `Display`.
- **Answer `changed = true` unconditionally**, as `with =` does. The
  second test fails, for each form.

## T33.2 — `apply` receives the state

§2.3's signature and call site. The commit counts the impls it touches.
**It builds what its gate needs:** a hand-written split fixture with a
real `Msg`, an outbox in its state, and `App::outbox` forwarding to it.

**Gate:** M28's suite, with its assertions unchanged. *Injection:* the
fixture's `apply` writes the tree. That is a compile error, pinned by a
paired doctest as in T33.0. **Other direction:** the same `apply`
writing the state compiles, and a chain completes through it.

## T33.3 — `Select` as a controlled widget

§2.4. `on_toggle`, `on_dismiss` and a value-bound `open`. Every
existing `Select` user migrates in the same commit.

**Gate:** `just widget-drive Select` under both strategies. Its
existing actions still land. A new action asserts that pressing outside
calls `on_dismiss`, and the popup stays closed on the next frame.
*Injection:* let the overlay pass close the popup itself as well. With
the state still saying open, the next frame reopens it, and the drive's
"closed on the next frame" assertion fires.

## T33.4 — split the applications a reader learns from

`counter-window`, `tasks-demo`, `showcase`, `glass` and a split twin of
the grid fixture. **The unsplit grid stays**, because T29.1's recorded
figures were measured on it. Each migration replaces its tree-writing
handlers with bindings, and the commit counts how many `with =` hooks
remain and why.

- **showcase:** `choose` writes `selected` and `open = false` to the
  state. `note`, `trigger.label`, `blurb` and `readout` become
  source-path bindings.
- **tasks-demo:** `refresh()` goes away, and `status` and `counts`
  become bindings. Its `apply` used to write them on purpose, so that
  the drive's "consequence landed" check was about `apply`. After the
  split, `apply` changes the state and the binding renders it, so the
  drive checks the same consequence, reached through the path that
  can now be narrowed. The drive's assertion text says so.
- **glass:** `material.blur` becomes a value binding.

**Gate:** each migrated example's frame is byte-identical before and
after. showcase, glass and counter-window have headless frame paths
(`--shot`, `--headless`). tasks-demo gains one in M32/T32.1, which the
queue orders first. The grid twin prints its frame hash beside the
unsplit grid's. `just widget-drive` and the tasks drive stay green
under both strategies.

## 3. What this is not

- **Not narrowing.** That is M29/T29.6.
- **Not a reactive store.** Nothing observes reads, and the state is a
  plain struct.

## 4. What the review found (2026-09-23)

A seven-lens review, with an adversarial verifier per finding, confirmed
eight findings against the first draft. What changed:

| # | finding | what changed |
|---|---|---|
| 1 | three examples' handlers write the tree, two of them on the migration list | header and §1 corrected. §2.4 and T33.3/T33.4 are the redesign |
| 2 | nothing checked the constant: the derive accepted `#[state]` on the tree and a second `#[state]`, and a hand-written impl could claim true | §2.2: refusals, and the hand-written promise named |
| 3 | stable rustdoc does not check `compile_fail`'s error code | T33.0: paired doctests, with the rustc message recorded |
| 4 | a missed impl in `benches-competitive`, and three counts that disagree | T33.0: a mechanical count, and `just stages` as part of the gate |
| 5 | T33.2's injection had no fixture to inject into, because the derive cannot express a `Msg` | T33.2 builds it, and the signature is given |
| 6 | a byte-identity gate for apps with no headless frame, and a grid migrated in place would lose T29.1's baseline | T33.4: which comparison each app uses, and the grid keeps an unsplit original |
| 7 | the branch point was misdescribed (it is after M31) | §1 |
| 8 | the header said every tree write comes from a binding, and a widget's own `on_event` is also a tree write | header reworded, pointing at T29.6's consumed set |

It also removed a false claim that "`tick` keeps `mark_all`, which
T29.4 asserts". `Host::tick` marks nothing, and T29.4 does not list
`tick`. §2.3 now records that as an open question for M34.

## 5. What building it changed (2026-09-24)

- **T33.0.** There are 32 hand-written `App` impls, counted by
  `git grep` over both workspaces, and the branch had missed two:
  `benches-competitive`'s `stages` arm, and M32's `tests/program.rs`.
  The refusals are **trybuild** cases, not doctests, because trybuild
  checks the exact message. The doctest pair stays on `App::State` as
  documentation.
- **T33.1.** The value form landed as `#[bind(target, source)]`, backed
  by a blanket `BindValue` for every `PartialEq + Clone` field. **A
  source path is an expression, so it can be a method call**: counter-
  window's pill binds `sign_word()`, and glass's blur binds `blur()`.
  That took the pill off `with =`.
- **T33.2.** Scripted and mechanical: 31 impls in 28 files. The 20 whose
  `apply` never touches the state take `_`.
- **T33.3 found a pass defect** (`f72fb05`). The press that dismisses an
  overlay was consumed, but its *release* still activated whatever it
  landed on. That broke nothing visible until a controlled trigger
  opened on release, when pressing the trigger while the menu was open
  closed it and reopened it in the same click. Fixing it then broke the
  live Tooltip action, whose green had depended on the defect. A
  tooltip is a *passive* overlay, and `Popup::passive` now says so.
- **T33.4.** 59 showcase frames are identical before and after its
  split, including five that press a demo's button. So are glass (2),
  tasks-demo, counter-window (3) and the counter's own frame. The
  grid's split twin draws the unsplit grid's frame byte for byte after
  one click, and injecting one character into its format string makes
  the check fire. **Two `with =` bindings remain in the examples**, both
  in counter-window's `--form` fixture. Both bind a bare `bool` field,
  and the value form needs a path *inside* the annotated field, which a
  `bool` has none of.
