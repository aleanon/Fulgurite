# M32 — `application(boot)`: one description of a program, used everywhere

> **DONE 2026-09-24 — T32.0, T32.1 and T32.2 landed as one commit**,
> because removing `Config::stylesheet` broke every example that the
> migration then fixed; a T32.0 commit on its own would not have built.
> §6 records what building it changed.
>
> **Written 2026-09-23. Rewritten the same day** after a
> seven-lens review (§5) and the decision that the constructor should be
> **shaped like iced's `application()`, for flexibility**. The prototype
> is on `main` (`29c1238`, `examples/glass` only).
>
> **What M32 exists to fix.** M31 (`docs/plan-application.md`) collapsed
> *construction* to one path and left *styling* with two:
> `Application::style` serves the headless host and the reload path,
> while `Config::stylesheet` serves the window. Edit one and the other
> silently wins.
>
> **What the review showed about the first draft's answer.** The draft
> recommended having a reload construct through the builder's `fn`
> pointer. That cannot work. No window ever reloads, and in the one real
> reload, a dylib swap, a pointer held from startup would rebuild with
> the *old* code. That is `docs/events-decision.md` rule 3's defect,
> and it is exactly M28/T28.7's *injection*, the fault that made that
> demo answer with v1's value. The draft had M28's lesson backwards: M28
> replaced a pointer with an index.
>
> **The answer here takes iced's shape and goes one step further than
> iced has to.** A program is *described* by a value, and the
> description is **the only place** construction, styling and window
> settings come from: for the window, for every headless host and, in
> M37, for a reload, which asks the **newly loaded** build for its
> description instead of holding on to the old one.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and prints the measured quantity beside its bound.*

---

## 0. The rules for this milestone

1. **When it is done, construction and styling each have one source.**
   That source is the program description. `Config::stylesheet` and
   `Application::style` are both gone, and `Application::build` is
   superseded by the boot function (§2.3).
2. **A headless host and the window are built by the same code.**
   `Program::host()` returns the `Host` that `run()` then drives. Every
   `--shot` and `--headless` path goes through it, which is what makes
   a byte-identity gate able to fail (§5, finding 4).
3. **Nothing constructs through a pointer held across a code swap.**
   Boot is *cold*: it runs when the program starts, and M37's reload
   asks the new build for a new description.
4. **Docs move in the same commit**, including every rustdoc site that
   names `Config::stylesheet` (T32.0 lists them).

## 1. What exists today

- `Program<A>` (the prototype) holds `build: fn() -> A`, an optional
  style `fn`, and a `Config`. `run()` calls `lumen_static_winit::run`,
  which always builds an `EventLoop` and blocks. There is no headless
  path through it. `Win` is private, and the sheet is installed only in
  `Win::new`.
- `Config` carries `title`, `width`, `height`, `page`, `backend`,
  `stylesheet`, `palette`, `images` and `agent`. The prototype builder
  exposes only title, size, page, palette and style.
- **Every headless frame path builds its own `Host`**, for example
  glass's and showcase's `shot()`, glasswall's `host()` and
  counter-window's `headless.rs`. Each calls `set_stylesheet` itself,
  so none of them goes through the window's code.
- `Application::build()` is required, and `Host::launch` and the
  derived `Reloadable::fresh()` call it. Only `counter`, `minimal`,
  `counter-window` and test fixtures implement `Application`. glass,
  glasswall and showcase construct through `build(sigma)`, `build(k)`
  and `build(entry, open)`, and showcase's `entry` comes from argv.
- **M28's jobs are spawned through `Host::spawn`**, so a boot that wants
  to start work needs a way to hand jobs over before the first frame.
- `counter-window` implements its own `ApplicationHandler` on purpose.
  Its module rustdoc lists what the turnkey `run` cannot supply: its own
  agent socket, an accessibility adapter, a stylesheet that is
  re-cascaded when the count changes sign, and Up/Down keys.

## 2. The decisions

### 2.1 The shape: iced's builder, with constant-or-function arguments

```text
application(boot)                 // boot: impl Boot<A>
    .title("glass")               // impl Title<A>: &'static str, or Fn(&A) -> String
    .style(SHEET)                 // impl Style<A>: &'static [Rule], or Fn(&A) -> Vec<Rule>
    .size(460, 620)
    .page(PAGE)
    .palette(p)
    .images(Wallpaper::new)       // fn() -> impl ImageSource
    .agent(addr)
    .backend(Prefer::Gpu)
    .run();
```

- **`Boot<A>`** is implemented for `Fn() -> A` and for
  `Fn() -> (A, Vec<Job<A::Msg>>)`, iced's `(State, Task)`. The jobs are
  handed to `Host::spawn` before the first frame. **Capturing closures
  are allowed**, as in iced, which is what showcase's argv entry needs.
  The prototype's rustc refusal of a capturing closure is dropped
  deliberately. That refusal was protecting a *reload* constructor, and
  boot is cold (rule 3). The commit that drops it says so, because the
  prototype's commit presented the refusal as a result.
- **Constant or function**, by iced's trick: one trait with an impl for
  the constant type and a blanket impl for `Fn(&A) -> _`. The two do not
  overlap, because `&'static str` and `&'static [Rule]` are not `Fn`.
- **The title is re-asked after every frame whose `update` wrote**, and
  `set_title` runs only when the text changed. It may allocate. That is
  the window's cost, not the `Host`'s, and the allocation gate is on the
  `Host`.
- **The style is asked when a host is built and when M37 reloads.** It
  is *not* re-asked per frame, because a re-cascade per frame is exactly
  the cost counter-window's sign-dependent sheet pays by hand, and it
  stays counter-window's to pay.

### 2.2 The style is owned, so the `sheet` field goes away

The prototype borrowed (`Fn(&A) -> Stylesheet<'_>`), and that forced
every application with a computed sheet to keep it in a field. The
borrow was inherited from `docs/plan-application.md` §2.4, whose reason
was that the *core* trait must not need `alloc`. The builder is not in
the core. It is std-only, and the style is asked twice in a program's
life, not once per frame. So the function form returns `Vec<Rule>`, the
`sheet` fields that `Counter` and glass's `Demo` gained are removed, and
a sheet that is really constant is written as a `const` and passed as
`&'static [Rule]`, which allocates nothing.

### 2.3 Boot supersedes `Application::build`

With the description as the single source, `Application::build` would be
a second constructor. For glass, showcase and glasswall it would even be
a *different* one, a zero-argument twin of `build(sigma)`. The review
found that bounding `Program` on `Application` would force exactly that
(§5, findings 2 and 3). So:

- `Program<A: App>`, with no `Application` bound.
- `Application::style` is removed. `Application::build` is retired for
  applications that start through `application(..)`. `Host::launch` goes
  with it, since `Program::host()` replaces it.
- **M31's property is kept, not dropped.** T31.2 made a reloadable
  application that also derives `Default` a compile error, so that
  nothing constructs one through `Default`. That stays true, because
  nothing constructs through `Default` or `build()` any more: M37's
  reload re-runs the new build's boot. T32.0's gate re-states T31.2's
  property against the new path.
- `Reloadable::fresh()`, which the derive routes to `Application::build`,
  stays for the in-process `Host::reload` and `fixtures/hot-host` until
  M37 replaces that path. This is the one place two constructors remain
  after M32, and the plan names it rather than hiding it.

### 2.4 Where it lives

The description (`Program`, `Boot`, `Title`, `Style`, `host()`) lives in
**`lumen-static-shell`**. `run()` is an extension trait in
`lumen-static-winit`. This is so M37's hot library can build a
description without linking a window system, and so a shell test can
call `host()` without winit, which `just test` keeps out of
`lumen-static-shell`'s dependency graph.

## T32.0 — the description, `host()`, and the one-path gate

Build §2.1–§2.4, remove `Config::stylesheet`, and move `glass` (already
on the prototype) to it.

**Doc sites, in the same commit:** the rustdoc that names
`Config::stylesheet` (winit `lib.rs`, where one is an intra-doc link
that would break, and `agent.rs`), and `docs/plan-widgets-gallery.md`.

**Gate — one path.** In a shell test, build a program with a boot that
takes an argument and a style with one distinctive rule, and call
`host()`. The first frame must carry that rule's colour on the styled
node, printed beside the rule's colour. *Injections:*

- **Skip installing the style in `host()`.** The colour assertion fires.
- **Build the host with `Host::new(boot())` and no style.** The same
  assertion fires, which shows the test goes through `host()` and not
  around it.

**Gate — M31's property, re-stated.** A program whose application also
derives `Default` still does not compile when it is reloadable (T31.2's
coherence refusal), and the refusal is checked in both directions in the
same session.

## T32.1 — migrate the windows, `--shot` included

`minimal`, `showcase`, `glasswall` and `tasks-demo` move to
`application(..)`. **Their `--shot` and `--headless` paths move to
`program().host()`**, so the frame a shot compares is the one the window
draws. Each application writes its program once, as
`fn program() -> Program<_>`, and `main` calls `program().run()`. M37
relies on that function.

- `minimal` and `tasks-demo` gain `--shot` **before** their migration,
  or there is no earlier frame to compare against.
- **showcase's argv entry** becomes a capturing boot.
- **glasswall's wallpaper** moves to `.images(..)`, and showcase's and
  tasks-demo's agent addresses move to `.agent(..)`.
- **counter-window is excluded, and says why.** It is the documented
  lower level of the seam. It still reads its *initial* sheet from the
  same place the others do: it builds its host through `host()` and
  keeps its own event loop.

**Gate:** each example's shot is byte-identical before and after its
migration. *Injection:* drop one rule from the style **inside the
program description**, which is now the only place it is installed.
That example's shot comparison fires, and the differing-pixel count is
printed. Unlike the first draft's injection, this one reaches the
window's path, because the shot and the window are one path.

## T32.2 — the book

`book/src/backends.md` teaches `run(app, Config { .. })` in a ```text
block, and `book/src/index.md` includes `minimal`'s `app` anchor. Both
move to the builder. `just book` and `just static` must pass.

## 3. What this is not

- **Not iced's `update` and `view`.** Here update is the binding step
  and view is the tree. The builder borrows iced's *entry* shape: a boot
  function, plus settings that are constants or functions of the state.
- **Not a reload.** That is M37, which this description is shaped for.

## 4. Deferred

- **A style re-asked per frame.** It would let counter-window move to
  the builder. Reopen when a second application wants a state-dependent
  sheet, and price the re-cascade first.

## 5. What the review found (2026-09-23)

A seven-lens review, with an adversarial verifier per finding, confirmed
eight findings against the first draft, plus two cross-plan findings on
the same design (X-1, X-2). What changed:

| # | finding | what changed |
|---|---|---|
| 1 | **blocking:** the recommended route, a reload through the builder's `fn` pointer, has no window reload to serve, and across a dylib swap it *is* rule 3's defect | rule 3, and §2.3: boot is cold. M37 re-asks the new build |
| 2, X-2 | bounding on `Application` forced a second, zero-argument constructor beside the closure | §2.3: `Program<A: App>`, and `build` retired |
| 3 | the window's entry had no headless path, so the gate could not go through "the same entry" | rule 2, and `host()` |
| 4 | every `--shot` built its own `Host`, so byte-identity held however wrong the migration was | T32.1: shots go through `host()` |
| 5 | the builder lacked `images`, `agent` and `backend`, and showcase's argv entry needs a capturing boot | §2.1 |
| 6 | counter-window exists for what the turnkey `run` cannot supply | T32.1: excluded, with the reason |
| 7 | the borrowed style's cost was argued from a no-alloc reason that does not apply to a std-only builder | §2.2: the style is owned, and the fields go |
| 8 | doc sites and the book's scope were misstated | T32.0 lists the sites. T32.2 is scoped |

## 6. What building it changed (2026-09-24)

- **Every migrated frame is byte-identical, 62 of 62.** 51 showcase
  entries closed and 3 open, glass at two scroll positions, glasswall,
  minimal, tasks-demo, and counter-window at counts 12, 0 and −3. Each
  "before" was rendered from a worktree at the previous commit, and each
  "after" through `program().host()`.
- **The injection fired — on the second attempt, and the first attempt
  was a finding.** Dropping tasks-demo's `Kind("Label")` rule left its
  frame identical. The rule had never matched anything: widget kinds are
  lowercase (`label`, `button`), and tasks-demo's sheet says `"Label"`
  and `"Button"`. **tasks-demo has been unstyled since its sheet was written**, and
  its migration was byte-identical because both sides were unstyled. It
  is fixed in its own commit, with the sheet lint asserted. The
  injection that fires: removing glass's bar height changes **1 380 of
  285 200** pixels, and restoring it brings back byte identity.
- **`LUMEN_SHOT` instead of a `--shot` per example.** The plan had
  minimal and tasks-demo each gain a `--shot`. That broke the book, which
  includes minimal whole, so the window crate's `run()` now renders one
  headless frame from `host()` when `LUMEN_SHOT=frame.ppm` is set. Every
  program gets it without writing one.
- **counter-window was not excluded after all.** Its style is a
  *function of the state* (the sheet for the count's sign), which the
  constant-or-function `.style(..)` expresses directly. So its window,
  `--headless` and socket all build their host through `program()`. What
  stays hand-written is the per-sign re-cascade between pumps, as §4
  says.
- **A book snippet had been failing, unnoticed.** `first-tree.md`'s
  hidden setup declared `title` and `hint` as `Label<String>`, but the
  counter's are `Label`. Only `just book`, which needs `mdbook`, runs
  these snippets, and nothing else noticed. Fixed, and `just book` is
  green.
- **`Host::launch` and `Application::style` are gone.** `Host::adopt`
  re-asks the program's sheet source, which the host keeps.
  `tests/reload_sheet.rs` still guards the M31 property through it: with
  the re-ask removed, two of its three tests fire.
