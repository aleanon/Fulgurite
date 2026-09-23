# M33 — state apart from the tree, so a handler cannot write a widget

> **Written 2026-09-23, not started.** Both halves exist as prototypes on
> `proto/state-split` (`d861f4f`, `a1ded79`). The decision to ship them
> was made on 2026-09-23. **This milestone must land before M29's
> T29.2–T29.4** (`docs/plan-damage-precision.md`), because it is what
> makes them apply to the frames applications actually produce.
>
> M29 measured what narrowing a binding's mark is worth. It is a large
> share of a frame on the grid fixture, and **nothing on any frame a
> click produces** (`docs/results-damage-precision.md` T29.1, the
> `handled` arm). `Host::pump_with` calls `mark_all()` for every
> consumed event before `App::update` is asked anything. The comment
> there gives the reason: a handler is `fn(&mut App, &EventCtx)`, so it
> may write any field, the tree included.
>
> **The constraint the type would enforce is already obeyed.** The
> prototype checked every handler in every example, and none writes the
> tree. This milestone turns that habit into a type. With a separate
> `State`, a handler is `fn(&mut State, &EventCtx)`. The tree is not
> reachable from there, so every tree write comes from a binding, and a
> binding knows which node it writes.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red.*

---

## 0. The rules for this milestone

1. **Backwards compatible by construction.** Without `#[state]`, the
   derive emits `type State = Self`, and every existing application
   means what it meant. This is why the prototype built the whole
   workspace unchanged.
2. **The split and the source path land together, or neither is
   worth it.** Without `#[bind]`'s source path, a split application can
   only bind through `with =`, and M29 §2.4 lists `with =` as
   permanently unnarrowable. The split would win on the handler path and
   give the win straight back on the binding path. The prototype found
   this in its first commit and fixed it in its second.
3. **This milestone does not narrow anything.** It changes types.
   Narrowing the `handled` mark is M29/T29.6, which needs T29.3's
   per-binding ranges before it has anything to narrow *to*.

## 1. What exists today

- On `main`: `App::Tree: Handlers<Self>`, and dispatch calls
  `h(app, &ctx)`.
- On `proto/state-split`: `App` gains `type State` and `state_mut()`,
  `Tree: Handlers<Self::State>`, and dispatch calls
  `h(state.state_mut(), &ctx)`. `examples/counter` is split into
  `Counter` (tree and bindings) and `Tally` (what a handler may touch).
- `#[bind(ui.value, count, "count: {}")]` on a `#[state]` field formats
  `self.state.count`. The source path is relative to the annotated
  field, as the target is relative to `self`.
- **Twenty-six hand-written `App` impls** gained two lines each on the
  branch. M31 and M32 add or change impls after the branch point, so the
  count on landing will differ. Rebasing means re-counting, not
  replaying.
- `App::apply` (M28's deferred messages), `App::tick` and
  `#[app(after_update)]` all still take `&mut self`, so any of them can
  reach the tree.

## 2. The decisions

### 2.1 What "cannot reach the tree" does and does not promise

The promise is structural: `State` and the tree are sibling fields, so
`&mut State` cannot be used to name a widget on the tree. **It does not
cover interior mutability shared between the two.** A `Rc<Cell<_>>`
held by both a widget and the state would let a handler change what a
widget shows, with no binding involved. Nothing in the workspace does
that. The invalidation oracle (T29.6's gate) is what would catch it, and
the rustdoc on `App::State` must say so, not imply more.

### 2.2 The tree reports whether it is split, as a constant

M29/T29.6 may narrow only for an application whose handlers cannot
reach the tree. `State = Self` must keep `mark_all`. Rust cannot compare
two associated types for equality in a `const`, so the derive emits a
`const` on `App`: true when `#[state]` is present, false otherwise. A
hand-written impl defaults to false, which is the conservative answer.

### 2.3 `apply` moves to `&mut State`; `tick` and `after_update` do not

A deferred message is handler-shaped work. It arrives from a task, not
from a pointer, but it is the same kind of write, so M28's `apply`
should receive the state, not the whole application. `tick` and
`after_update` are the application's own hooks, and they return a bare
`bool`. They keep `&mut self`, and they keep `mark_all`, which M29/T29.4
asserts.

## T33.0 — the split lands

Rebase `d861f4f` onto `main` and re-count the hand-written impls. Add
§2.2's constant.

**Gate:** the prototype's injection, kept as a compile-fail test.
Writing `t.ui.title.set(..)` inside a handler of a split application
must fail with E0609 (`no field 'ui'`). Put it in a `compile_fail`
doctest on `App::State`, so the refusal is part of the documentation.
*Other direction:* the same handler writing `t.count` compiles.

## T33.1 — `#[bind]` learns a source path

`a1ded79`, rebased. The attribute table moves in the rustdoc and the
book in the same commit, and `just static` checks that they agree.

**Gate:** the prototype's two tests. The one that matters is
`a_source_path_binding_still_reports_precisely`: a second update that
wrote nothing must answer `false`. *Injections, both already seen to
fire on the branch:* ignoring the source path fails to compile
(`Nested` is not `Display`), and answering `changed = true`
unconditionally, as `with =` does, fails the second test.

## T33.2 — `apply` receives the state

§2.3. `examples/tasks-demo` and M28's chain tests are the users.

**Gate:** the M28 suite, unchanged in what it asserts. *Injection:* a
message handler in a split fixture writes the tree. That must be a
compile error.

## T33.3 — split the applications a reader learns from

`examples/counter` is already split on the branch. `counter-window`,
`tasks-demo`, `showcase` and the grid fixture follow, so that M29's
measurements run on split applications. Each migration moves its
`with =` bindings to the source-path form wherever the hook only
formatted a value, and the commit counts how many remain and why.

**Gate:** each migrated example's frame is byte-identical before and
after (M32/T32.1's comparison), and `just widget-drive` stays green
under both invalidation strategies.

## 3. What this is not

- **Not narrowing.** That is M29/T29.6.
- **Not a reactive store.** Nothing observes reads, and the state is a
  plain struct.
