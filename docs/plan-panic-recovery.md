# M36 — a panic shows where it happened, and the window survives it

> **Written 2026-09-23, not started.** The ask: *on a panic, show a
> screen saying the application panicked and where in the code the
> panic is.* The moment it matters most is mid-edit. You save, the
> reloaded code panics, and today the process dies and takes the state
> you were iterating on with it.
>
> The prior art is `hot_ice` (`/mnt/Utvikling-linux/hot_ice`). Its macro
> wraps every hot function in `catch_panic`. A `Once`-installed hook
> records `PanicHookInfo::location()` into a thread-local, the payload is
> downcast to `&str`/`String`, and a failed `view` renders a centred
> "View Error" card with `panicked at {file}:{line}:{col}: {msg}`. It
> keeps no last-good state. The next frame calls the code again, so the
> screen clears once the code stops panicking.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and prints the measured quantity beside its bound.*

---

## 0. The rules for this milestone

1. **"Where" means a line a person can open.** The screen shows
   `file:line:column`. When that location is inside the framework, for
   example an index out of bounds in `lumen-static` that the
   application caused, the screen also shows the nearest frame in the
   *application's* code. A report that only ever points into
   `crates/lumen-static/src` answers the wrong question.
2. **The screen does not use the machinery that just failed.** It is
   drawn from a `DisplayList` built directly, not from a widget tree
   through layout and paint. §2.3 says why.
3. **Only where it can work, and only where it helps.** Recovery is
   compiled in under `cfg(panic = "unwind")` and in debug builds or with
   the `reload` feature. A shipped release build keeps the default
   behaviour, because a screen that swallows a crash also hides it from
   whatever would have reported it. §2.4 records this decision.
4. **Every site a panic can come from is exercised, not assumed.** There
   are five sites in a frame, and T36.0's fixture reaches each one.

## 1. What exists today

- **There is no windowed dylib reload.** `libloading` appears only in
  `fixtures/hot-host`, which is headless, has no `Host` and leaks its
  libraries. `lumen_static_winit::run` links the application
  statically. The seam a user actually meets is in-process:
  `Host::pump_with` and `Host::reload`, called from the window's redraw
  (`tick` → `pump_with` → `backend.present`).
- **The dylib boundary is `extern "C"`** (`export_reloadable_app!` in
  `crates/lumen-static-reload/src/lib.rs`, and the hand-written verbs in
  `fixtures/hot-app`). Since Rust 1.81, a panic that unwinds out of an
  `extern "C"` function **aborts the process**. That is what
  `just hot-demo` would do today on any panic in v2.
- **No workspace profile sets `panic = "abort"`**, except
  `benches-competitive`. So unwinding, and therefore `catch_unwind`, is
  available in every build this milestone targets.
- `catch_unwind` is used only by M28's spawners
  (`crates/lumen-static/src/spawn.rs`), which turn a job's panic into
  `Failure::Panicked`. `set_hook` appears only in tests, which silence
  the hook.
- **What a mid-frame panic leaves behind** (in `pump_with`'s order):
  events partly dispatched, possibly with pointer capture held;
  `arenas_dirty` already cleared if the panic happens during the
  cascade; `list` half-filled, `retain` unfinished and `overlays`
  partial if it happens during paint; the invalidation marks not
  cleared, which is harmless. The application's own state may be
  partly mutated, and nothing can undo that.
- **A frame can be drawn without the application's tree.**
  `DisplayList::new`/`rect`/`text_weighted`, `Canvas::with_font` and
  `render_within` are enough. The backends present `host.list`, so an
  error frame either replaces `host.list` or needs a present call that
  takes a list.

## 2. The decisions

### 2.1 Capture: a hook for the location, `catch_unwind` for control

`std::panic::set_hook` is the only place `PanicHookInfo::location()`
exists, and the payload that `catch_unwind` returns has no location in
it. So a hook records, and a catch decides:

- **The hook** records the message, `location()` and, in debug builds,
  a `Backtrace::force_capture()`, into a thread-local. It then calls the
  previous hook, so stderr still gets the ordinary report. It does not
  decide anything. M28's task threads panic under the same hook, and
  their panics are already `Failure::Panicked` data, which the hook must
  not disturb.
- **The catch** wraps the frame seam in the shell. It takes the
  thread-local report and hands the window a `PanicReport` with the
  message, file, line, column and application frames.

**The application frame comes from the backtrace.** `unwrap`, `expect`
and slice indexing are `#[track_caller]`, so `location()` already points
at the application's line for the common cases. For a panic inside the
framework, the backtrace is filtered to frames whose source path lies
under the application's manifest directory, and the first one is
reported. Without debug info that filter finds nothing, and the screen
says so instead of showing a blank line.

### 2.2 Recovery: reset the host, keep the state, drop the event

After a catch:

- **The host's own invariants are restored.** `list` and `overlays` are
  cleared, `retain` is reset so the next frame is full damage,
  `mark_all()` is called, `arenas_dirty` is set, and pointer capture is
  released.
- **The input queue is drained.** The event that caused the panic would
  otherwise cause it again on the retry, which is a loop presented as a
  recovery.
- **The application's state is kept as it is.** It may be partly
  mutated, and there is no last-good copy. hot_ice made the same
  choice, for the same reason: taking a snapshot every frame costs
  every frame, to protect against something that happens once per bad
  edit.
- **The screen stays up until something changes.** That means a
  successful reload, or the user pressing **R** to retry with the
  current state. A retry that panics again shows the screen again with
  the new report. Nothing retries on its own, so a panic in `layout`
  cannot spin the CPU.

### 2.3 The screen is a display list, not a tree

A widget tree for the screen would be the natural way to write it, and
the agent could read it like any other tree. But it would go through
the same layout, text measurement and paint that may just have panicked.
A panic inside `Label::layout` would then panic again while drawing its
own report. The screen is built as a `DisplayList` from a fixed set of
lines: a title (*"The application panicked"*), the message wrapped to
the window's width, the location, up to eight application frames, and
the hint (*"Press R to retry, or save a change to reload"*). The agent
reads the report through a method of its own (T36.3), not from a tree.

**If drawing the screen panics**, the window gives up. The report has
already gone to stderr through the previous hook, so it is not lost.

### 2.4 Debug and reload builds only

A release build without `reload` keeps today's behaviour. A shipped
application that shows a panic screen instead of crashing looks as if it
works, and it hides the crash from a crash reporter, from a supervisor
that would restart it, and from its user's bug report. The ask was
about the edit loop, and the edit loop is debug builds and `reload`.

### 2.5 The dylib seam catches inside the library

Two options for `export_reloadable_app!` and the fixture's verbs, and
the first is chosen:

- **Catch inside each shim** and return a status code, with the report
  written through an out-parameter as plain bytes (pointer and length,
  copied by the loader straight away). This is hot_ice's shape.
- **`extern "C-unwind"`**, letting the panic cross into the loader.
  Rejected: the payload is a `Box<dyn Any>` allocated by the library,
  and downcasting it needs a `TypeId`, which is not stable across
  builds (`docs/00-handoff-static-tree.md`). A report crossing a build
  boundary must be data.

## T36.0 — the fixture, and today's behaviour recorded

A fixture application that can panic at each of the five frame sites,
chosen by a key:

1. a handler
2. an applied message
3. `update` (a `#[bind]` source that panics on format)
4. a widget's `layout`
5. a widget's `paint`

Each site records its own `line!()` in a constant, so a test can compare
the reported line with the real one without anyone copying numbers.

**Gate:** a test runs the fixture as a subprocess for each site and
asserts that today the process **dies**, with a non-zero status, and
that stderr contains the recorded line. This is the defect, shown, and
it is the assertion that T36.3 turns around. *Injection:* point one
site's key at a no-op. The "it died" assertion for that site fires,
which shows each site is reached, not just declared.

## T36.1 — capture and host recovery

§2.1 and §2.2 in `lumen-static-shell`: the hook, the `PanicReport` and a
guarded frame entry that returns one. The core crate is untouched, and
it keeps no `unsafe`, because `AssertUnwindSafe` is a safe wrapper.

**Gate:** for each of T36.0's five sites, headless, the report's file
and line equal the site's recorded constant, printed side by side. The
next frame, after a retry with the trigger cleared, is byte-identical
to a fresh host's frame of the same state. *Injections:*

- **Do not install the hook.** The location assertion fires, naming the
  site.
- **Do not reset `retain`.** The retried frame is a partial repaint over
  the half-painted list, and the byte-identity assertion fires with a
  pixel count.
- **Do not drain the queue.** The retry panics again, and the test
  counts the second report.

**And the other direction:** a frame that does not panic goes through
the guarded entry with a byte-identical result and no report. The alloc
gate stays at zero, because a guard that allocates per frame is a cost
paid for nothing.

## T36.2 — the screen

§2.3 as `failure_frame(&PanicReport, width, height) -> DisplayList`.

**Gate:**

- A golden image at two window sizes.
- **Every text command lies inside the viewport.** Print the rightmost
  x beside the width. A long message wraps and does not run off the
  edge.
- Title and body text reach WCAG AA contrast against the background,
  using the `verifying-apps` contrast measure.
- The location line contains the fixture's file and recorded line.

*Injection:* remove the wrapping. The viewport assertion fires with the
overrun printed.

## T36.3 — the window

`lumen-static-winit` wraps its redraw and its event handling in the
guarded entry. After a catch it presents `failure_frame` with full
damage on both backends, and **R** retries. With the `reload` feature,
a successful `Host::reload` clears the screen. The agent gains a
`ui.failure` method that returns the report, or null when there is
none, so a live test can read it.

**Gate:** a drive script, in the manner of `scripts/widget_drive.sh`,
run under both invalidation strategies. For each site it triggers the
panic, reads `ui.failure` and asserts the file and line, checks a
screenshot pixel from the screen's title band so the screen is known to
be drawn, clears the trigger, presses R, and asserts that the
application's own tree answers again. *Injection:* have R skip the
reset. The retried frame renders over a half-painted list and the
screenshot check fires.

## T36.4 — the dylib seam

§2.5 in `export_reloadable_app!` and `fixtures/hot-app`. `just hot-demo`
gains a third build that panics in its frame verb, and a fourth that
fixes it. The loader prints the report and carries on with the fourth
build.

**Gate:** the demo asserts the report's line and the recovered frame's
hash. *Injection:* remove the catch from the frame verb. The demo
**aborts**, which is the Rust 1.81 behaviour, and the script reports the
abort instead of hanging.

**Not built here:** a windowed dylib loader. §1 records that none
exists. This task makes the seam safe for one, and `hot-host` shows the
report as text.

## T36.5 — the docs

`docs/step4-hot-reload.md` gains the panic story: what is caught, what
is kept, and what is not recovered (partly mutated state). The book's
reload chapter (`book/src/hot-reload.md`) shows the screen. `docs/00-handoff-static-tree.md` marks
the decision as **DECIDED**, pointing at this plan.

## 3. What this is not

- **Not state rollback.** §2.2 explains why.
- **Not a crash reporter for shipped builds.** §2.4 explains why.
- **Not a change to task panics.** M28's `Failure::Panicked` already
  handles those as data. The hook must leave that path unchanged, and
  T36.1's suite runs M28's failure tests unmodified to show it does.
