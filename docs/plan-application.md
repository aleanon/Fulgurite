# M31 — `Application`: one construction path, and a stylesheet that survives a reload

> **T31.0–T31.2 done 2026-09-23. T31.3 open.** Written the
> same day; each task's entry records what building it changed.
>
> An application is currently built by whatever inherent method its
> author wrote — `Counter::new()` in the flagship example — while the
> framework rebuilds it with `Default::default()`, because
> `Reloadable::fresh()` is derived as exactly that. **Two construction
> paths, and nothing checks that they agree.**
>
> The consequences are already measured rather than argued:
>
> - **Handlers assigned outside `Default` do not survive a hot swap.**
>   `Counter::new()` writes `on_click = on_inc`; `fresh()` calls
>   `default()`, which leaves `ignore_click`. `examples/counter` does not
>   derive `Reloadable` so nothing is broken *there* — but it is the
>   example that teaches the shape, and `fixtures/hot-app` had to write
>   `impl Default for Session` by hand to avoid it.
> - **An edited stylesheet never reaches the frame on a reload.**
>   Demonstrated by `crates/lumen-static-shell/tests/reload_sheet.rs`
>   (2026-09-22): set a sheet, run the real `save`/`fresh`/`restore`
>   sequence, and the frame still uses the *previous* build's rules. The
>   host owns `sheet: Vec<Rule>` and nothing in the reload path re-asks.
>
> Both are the same defect wearing two hats: **the framework has no name
> for "construct this application using the code that is loaded now."**
> `docs/events-decision.md` rule 3 already says work in flight may hold
> no code reference; a stylesheet is code-derived data and was never
> treated as such.

*Each task is one commit unless it says otherwise; each ends with
`just test` green. Task ids go in commit subjects. Every gate names the
injection it must go red under.*

---

## 0. The rules for this milestone

1. **The test that motivates it already exists and must invert.**
   `tests/reload_sheet.rs` documents the defect and its header says
   exactly how to turn it into the gate for the fix. T31.0 is not done
   until that inversion is the thing that proves it.
2. **The migration is mechanical and must stay mechanical.** If a type's
   `build()` cannot be a struct literal, that is a finding about the
   type, not a reason to soften the trait.
3. **Docs move in the same commit**, including the attribute table and
   the book chapters that teach `new()`.
4. **No new public abstraction beyond the trait itself.** §2.4 records
   what was considered and rejected, so the next reader does not
   re-propose it.

## 1. What exists today

- `Reloadable::fresh()` is documented as *"a brand-new value built by the
  currently loaded code"* and the derive emits
  `<Self as Default>::default()`.
- `Host::new(app, w, h)` takes an already-built instance; `main` builds
  it, and `main` is not re-run on a swap.
- `Host` owns `sheet: Vec<Rule>`, written only by `set_stylesheet`,
  `restyle_subtree` and the agent. `keep_sheet` copies the `Rule`s with
  `extend_from_slice` — a copy of a fat pointer is the same pointer, so
  the `&'static str` selectors still belong to the build that made them.
- `fixtures/hot-host` never constructs a `Host`, so `just hot-demo` has
  never had a stylesheet in it to notice.
- **41 `#[tree]` sites across 25 distinct application types**, 35 of
  which derive `Default`.
- `pub type Stylesheet<'a> = &'a [Rule]` already exists, and `Rule::new`
  and `Patch::new` are both `const fn` — a `const` sheet is writable
  today and nobody has written one.

## 2. The decisions

### 2.1 One trait, on the application type only

```rust
pub trait Application: Sized {
    /// Construct this application with the code that is loaded **now**.
    /// Handler `fn` pointers must be assigned here: a hot reload calls
    /// this, then grafts `#[reload(keep)]` fields onto the result.
    fn build() -> Self;

    /// The sheet this build wants. Asked for at startup and again on
    /// every reload, so an edited stylesheet takes effect on the same
    /// swap that reloads the code.
    fn style(&self) -> Stylesheet<'_> {
        &[]
    }
}
```

**On the app type, not on every widget.** A trait the derive had to call
per field would need all 33 widgets in `lumen-static-widgets` to
implement it, and a blanket `impl<T: Default>` cannot coexist with
per-widget impls without specialization. Scoped to the application,
there is one impl per program and no widget churn. Container wiring
lives inside that one `build()`, which is where `examples/counter`
already puts it.

### 2.2 `fresh()` routes through `build()`, and the derive can tell

An application is the thing with a `#[tree]` field, and **field
attributes are visible to every derive on the struct** — so
`Reloadable`'s derive needs no new attribute:

```rust
fn fresh() -> Self {
    <Self as Application>::build()   // when the struct has #[tree]
    // Default::default()            // otherwise — widgets, unchanged
}
```

That is the whole mechanism. `Default` keeps meaning *structural zero
value* where the framework calls it per frame — `Repeat`'s
`window.resize_with(vis, T::default)` and arena seeding — and
`Application::build` means *supply the code*.

### 2.3 The app type must not implement `Default`

Adding `build()` while leaving `Default` in place gives **three**
construction paths instead of two. The point is to have one.

Removing it also buys the compiler's help: a `build()` that returns a
struct literal makes **adding a field a compile error until it is
initialised**, which is exactly what `#[derive(Default)]` destroys by
handing every new field a zero and every new handler a placeholder.

The enforcement is a derive refusal: `#[derive(App)]` on a type that
also derives `Default` fails to compile, with a message naming
`Application::build`. §5 records the limit — this is depth-1 only.

### 2.4 `style()` returns `Stylesheet<'_>`, and **not** a generalised trait

`Stylesheet<'a> = &'a [Rule]` is already the abstraction, and both forms
this milestone has to serve collapse into it:

| profile | what `style()` returns |
|---|---|
| `no_std` leaf | a `const &'static [Rule]` in `.rodata` |
| std | `&self.sheet`, or the same const |

**Rejected: `fn style(&self) -> impl StyleSheet` or an associated type.**
Three reasons, recorded so this is not re-proposed:

1. It is abstraction over a set of one. `Vec<Rule>` and
   `&'static [Rule]` both reach `&[Rule]`; a trait adds a name and a
   bound to express a choice the slice already erases.
2. **`Vec<Rule>` in the signature would force the trait to need
   `alloc`**, so it could not live in core unguarded. The borrowed form
   is the *more* portable one, which inverts the intuition that
   motivated the proposal.
3. An associated type cannot have a default (M28 learned this: type
   defaults are unstable, which is why `App::Msg` is required and 19
   impls changed). Every application would have to write one to get the
   empty sheet.

**The widening is available later and is backwards compatible:**
`-> impl AsRef<[Rule]> + '_` accepts every impl written against
`&[Rule]`. Ship the slice; keep that in reserve for an app that wants to
compute a sheet into a scratch buffer it does not own.

### 2.5 The host asks, so `main` and the reload path cannot disagree

```rust
impl<A: Application, …> Host<A, …> {
    /// Build the application and install its sheet.
    pub fn launch(width: u32, height: u32) -> Self;
    /// Rebuild from the currently loaded code, keeping what `save` kept,
    /// and re-ask for the sheet.
    pub fn reload(&mut self);
}
```

`Host::new(app, …)` stays, for tests and for an application that has a
reason to build its own. `launch` is what examples and the book use,
because it is the only shape where the two paths are the same code.

## T31.0 — the trait, and `fresh()` routed through it

`Application` in `lumen-static`, the `Reloadable` derive keyed on
`#[tree]`, and `Host::launch`/`Host::reload`.

**Gate — the motivating test inverts.** `tests/reload_sheet.rs`'s two
`V1` expectations become `V2`, `the_host_is_never_asked_for_a_sheet_again`
is deleted, and the file's header note goes with it. *Injection:* make
`Host::reload` skip the `style()` call — the inverted assertion fires,
which is the same assertion that passes today for the opposite reason.

**Second gate — the handler half, which no in-process test can reach.**
`fixtures/hot-app` gains a `Button` whose `on_click` is assigned in
`build()` and differs between v1 and v2, and `just hot-demo` asserts the
click runs **v2's** handler after the swap. *Injection:* assign it in
`Default` instead — the demo reports v1's value, which is
`docs/events-decision.md` rule 3's defect in the one place that demo
already proves it for chains.

## T31.1 — the migration — **DONE 2026-09-23, scoped down**

> **The plan said 25 types; the right number is four, and measuring
> which ones found two things the plan had wrong.**
>
> **What the derive already forces.** `Reloadable::fresh()` routes to
> `Application::build()` for any struct with `#[tree]`, so every
> *reloadable* application must implement the trait or fail to compile —
> proved by T31.0's second injection, which reverted the routing and got
> `the trait bound `Session: Default` is not satisfied`. That is exactly
> the set where two construction paths can diverge, because `fresh()` is
> the second path. `fixtures/hot-app` is the only one in the tree and it
> migrated with T31.0.
>
> **Most `#[tree]` types are tests, not applications.** Of the 41 sites,
> the large majority are fixtures inside `crates/*/tests`. A fixture that
> never reloads has one construction path already, and forcing `build()`
> on it is churn against a defect it cannot have. They keep `Default`.
>
> **Migrated, because they are applications a reader learns from:**
> `examples/counter` (the motivating case — its `new()` is gone),
> `examples/minimal` (wiring moved out of `main`),
> `examples/counter-window`, and `fixtures/hot-app`.
>
> **Not migrated, and this is a finding about the trait:**
> `examples/glass`, `glasswall` and `showcase` construct through
> `build(sigma)`, `build(k)` and `build(entry, open)` — **parameterised
> constructors, which `fn build() -> Self` cannot express.** They keep
> `Host::new`, which is why that constructor stays. A demo that builds
> variants is a real shape and the trait does not serve it; §4 records
> the condition for revisiting.
>
> `Application` and `Stylesheet` joined the prelude: every application
> names both, and `lumen_static::event::Application` is not a path
> anyone should have to learn.

## T31.2 — `Default` on a reloadable application is a compile error — **DONE 2026-09-23**

> **Scoped to reloadable applications**, for T31.1's reason: `fresh()`
> is the second construction path, so it is the only place two paths can
> exist. A fixture that never swaps has one already and keeps `Default`
> — which it must, since `Repeat` materialises rows with `T::default()`
> every frame.
>
> **Rust has no negative bounds**, so the refusal is a coherence
> conflict the `Reloadable` derive emits for an application:
>
> ```rust
> trait AnApplicationThatReloadsMustNotAlsoDeriveDefault {}
> impl<T: Default> AnApplicationThatReloadsMustNotAlsoDeriveDefault for T {}
> impl AnApplicationThatReloadsMustNotAlsoDeriveDefault for Session {}
> ```
>
> The blanket covers `Session` only if `Session: Default`, and then the
> two overlap and rustc refuses with **E0119 — printing the trait's
> name**, which is why the name is a sentence rather than an identifier.
> Without `Default` the blanket does not apply and nothing conflicts.
>
> **Both directions, one session**, as `AGENT.md` requires — and a
> coherence trick deserves it more than most, because getting it wrong
> yields a blanket that overlaps *everything*, a gate that refuses every
> application including the correct ones:
>
> | case | result |
> |---|---|
> | `fail_reloadable_app_derives_default.rs` | E0119, locked in `.stderr` |
> | `pass_reloadable_app_without_default.rs` | compiles, and asserts the nested widget still derives `Default` |
>
> *Injection:* delete the emitted block — trybuild reports **"Expected
> test case to fail to compile, but it succeeded."**
>
> The macros crate's dev-dependencies gained `reload` on `lumen-static`
> and `lumen-static-widgets`: a UI case that expands the `Reloadable`
> derive needs the module the derive emits paths into, or it fails on a
> missing module instead of on the diagnostic it is locking.

## T31.3 — the `&'static str` half, with two real libraries

`tests/reload_sheet.rs` proves the in-process half and says so. The
other half — that a kept `Rule`'s selector points into the *old*
library's `.rodata` — needs `just hot-demo`'s two dylibs and a
pointer-range check against the loaded maps.

**Gate:** after the swap, every selector pointer in the host's sheet
lies inside v2's mapped range. *Injection:* skip the re-ask (T31.0's
`style()` call) — pointers still lie in v1's range and the assertion
fires with both ranges printed.

**Why it is a task and not a note:** the in-process test cannot see it,
and it is the half that makes this a correctness defect rather than a
missing feature.

## 3. What this is not

**Not a view-tree builder.** `build()` returns the application once, at
load. Nothing re-runs it per frame and nothing diffs its output; the
tree is still the type.

**Not a change to `Default` for widgets.** `Repeat` calls `T::default()`
per row and the arena calls it to seed. Those want a zero value and keep
getting one.

**Not a generalised stylesheet abstraction** (§2.4).

**Not per-widget `build`.** §2.1 — the composition cost is 33 widgets
and a specialization problem, for a property one impl already provides.

## 4. Deferred, with conditions

- **Refusing an `fn` field left at its placeholder.** The check that
  would close §5's depth-1 gap: a widget whose handler is still
  `ignore_click` after `build()` is almost certainly a mistake. Needs a
  way to distinguish "deliberately inert" from "forgotten", which is a
  design question this milestone does not answer. Reopen when a second
  application has shipped the bug.
- **Re-asking `style()` on a palette change.** `Host::set_palette`
  re-resolves roles against the *kept* sheet (M25/T25.11). If an
  application should be able to return *different rules* for a different
  palette, that is a second caller with the same borrow shape as §5.
  Reopen if a theme needs more than role re-resolution.
- **`impl AsRef<[Rule]>`** — §2.4's widening, when an application wants
  to compute a sheet it does not own.
- **A parameterised `build`.** Three examples construct through
  `build(sigma)`, `build(k)` and `build(entry, open)`; `fn build() -> Self`
  cannot express any of them and they keep `Host::new` (T31.1). An
  associated `type Config: Default` with `fn build(cfg: Self::Config)`
  would — at the cost of an associated type every application must name,
  which is the trade M28 already paid for `App::Msg` and would rather not
  pay twice. Reopen when a *reloadable* application needs it, because
  that is when the missing path stops being a convenience and starts
  being the defect this milestone removed.

## 5. Two traps, named before they are hit

**The borrow conflict.** This does not compile, and it is the first
thing T31.0 will run into:

```rust
let sheet = self.app.style();          // borrows self.app
self.sheet.extend_from_slice(sheet);   // needs &mut self
```

Disjoint fields, invisible through method calls. Take the sheet before
the application moves into the `Host`, or copy it into a local first. It
happens at launch and at reload, never per frame.

**Exhaustiveness is depth-1.** A struct literal forces every field of
the *application*, so no state field can be forgotten. It does not reach
`ui.controls.inc.on_click`, three levels down — `ui: Ui::default()`
still compiles and still gives dead buttons. This milestone fixes *which
path runs*; §4's first entry is what would fix *what the path must
cover*, and the plan should not be read as claiming otherwise.
