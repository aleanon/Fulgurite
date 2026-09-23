# M32 — `Program`: one front door, and one place a stylesheet comes from

> **Written 2026-09-23, not started.** The builder exists as a prototype
> on `main` (`29c1238`, `examples/glass` only). The decision to take it
> forward was made on 2026-09-23.
>
> M31 (`docs/plan-application.md`) collapsed *construction* to one path
> and left *styling* with two. `Application::style` serves the headless
> host and the reload path. `Config::stylesheet` serves the window,
> because `lumen_static_winit::run` takes an application that is already
> built and never asks the trait anything. Edit one and the other
> silently wins. That is the defect M31 existed to remove, one field
> over.
>
> The prototype also closed the gap T31.1 recorded, where `build(sigma)`,
> `build(k)` and `build(entry, open)` could not be written as
> `Application::build()`. **A non-capturing closure coerces to an `fn`
> pointer**, so `application(|| build(14.0))` compiles, and a closure
> that captures anything is refused by rustc:
> *"closures can only be coerced to `fn` types if they do not capture
> any variables"*. That refusal is `docs/events-decision.md`'s no-capture
> rule, enforced at the call site.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red.*

---

## 0. The rules for this milestone

1. **When it is done, a stylesheet has one source.** If the milestone
   finishes with `Config::stylesheet` and `Application::style` both still
   able to style one window, it has not done its job, whatever else
   ships.
2. **The cost is paid knowingly.** `Stylesheet<'_>` is borrowed, so an
   application whose sheet is *computed* has to keep it in a field
   (`sheet: Vec<Rule>`). `Counter` and glass's `Demo` already do. Every
   migrated application that computes its sheet gains that field, and
   §2.2 says why the borrow stays.
3. **Docs move in the same commit**, including the book chapters that
   teach `run(app, Config { .. })`.

## 1. What exists today

- `Program<A>` holds `build: fn() -> A`, an optional
  `style: fn(&A) -> Stylesheet<'_>` and a `Config`. `run` builds the
  application, copies `style(&app)` into `config.stylesheet`, and calls
  `run(app, config)`.
- Only `examples/glass` uses it. `minimal`, `showcase`, `glasswall` and
  `tasks-demo` build a `Config { .. }` literal, and `counter-window`
  drives the window from its own module.
- `.style()` is optional. An application that implements
  `Application::style` but forgets `.style(A::style)` gets an unstyled
  window, while its headless host and its reloads stay styled. **The
  prototype still has two paths.** It only makes the right one easier to
  choose.

## 2. The decisions

### 2.1 The style comes from the trait, and the builder has no `.style()`

`Program<A: Application>` always asks `A::style`. `Config::stylesheet`
goes away, which removes the second path instead of documenting it. An
application that wants a window must implement `Application`, which
M31/T31.2 already requires of every reloadable application.

`application(build)` keeps its `fn() -> A` argument. That is where a
parameterised constructor goes, and the default spelling is
`application(A::build)`.

**Construction is where a second path could reappear, and T32.0 has to
settle it.** If `application(|| build(14.0))` constructs the window's
application while `Host::reload` constructs through
`Application::build()`, construction has two paths again. That is M31's
own defect, moved into the builder. There are two ways to close it, and
T32.0 must pick one and write the argument down:

- **Reload uses the `Program`'s `fn` pointer.** The builder hands it to
  the host, and `fresh()` is not called on a window reload. One pointer
  serves both construction sites.
- **Parameterised constructors stay outside the builder.** Glass,
  glasswall and showcase implement `Application::build` with their
  constants inline, and `application` takes no argument. This is
  simpler, and it gives back the closure-coercion finding.

The first is recommended. The closure coercion is the prototype's best
result, and passing a pointer through is the same move M28 made when it
passed a step index.

### 2.2 `Stylesheet<'_>` stays borrowed

`docs/plan-application.md` §2.4 kept `&[Rule]` rather than `Vec<Rule>`,
so that the trait does not need `alloc`. That reason has not changed.
The cost is the `sheet` field in rule 2. A `const` sheet costs nothing,
because `Rule::new` and `Patch::new` are `const fn`, and the migration
should use one wherever a sheet is not actually computed.

## T32.0 — the decision in §2.1, and the gate that proves one path

Pick the construction route, then remove `Config::stylesheet`.

**Gate:** a headless test that goes through the same entry the window
uses. It builds a `Program` whose `Application::style` returns a sheet
with one distinctive rule, renders the first frame, and asserts that the
rule's colour is on the styled node. It then reloads and asserts the
rule again. *Injection:* restore the prototype's optional `.style()` and
leave it unset. The first assertion fires. *Second injection, for the
construction route:* make the reload call `A::build()` where the
`Program` was given a closure that builds differently. The reload
assertion fires, naming the value that differs.

## T32.1 — migrate every window

`minimal`, `showcase`, `glasswall`, `tasks-demo`, `counter-window` and
glass move to `application(..)…run()`. Where a sheet is a constant,
write it as a `const`. Where it is computed, the application gains the
`sheet` field and the commit counts how many did.

**Gate:** every example's `--shot` frame (`just example <name> --shot
frame.ppm …`) is byte-identical before and after its migration. If one
is not, that migration changed what renders, and that is a finding, not
noise. *Injection:* drop one rule from a migrated sheet, and that
example's comparison must fail. Only showcase, glass and glasswall have
`--shot` today. Adding it to `minimal` and `tasks-demo` is part of their
migration, and it has to happen *before* the migration, or there is no
earlier frame to compare against. `counter-window` has `--headless`,
which serves the same purpose.

## T32.2 — the book and the attribute table

The chapters that teach `run(app, Config { .. })` teach the builder, and
the included anchors move with the examples. `just book` and
`just static` must pass.

## 3. What this is not

- **Not a new trait.** The builder is a value. `Application` stays the
  only trait an application implements.
- **Not iced's `Program` trait.** It borrows iced's builder spelling
  and nothing else.
