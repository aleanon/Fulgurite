# M36 — a panic shows where it happened, and the window survives it

> **Written 2026-09-23, not started. Rewritten the same day** after a
> seven-lens review (§5) and two decisions: **a release build shows the
> screen when the `reload` feature is on**, and the reload half of the
> story is its own milestone, M37 (`docs/plan-window-reload.md`),
> modelled on `hot_ice`.
>
> The ask: *on a panic, show a screen saying the application panicked
> and where in the code the panic is.*
>
> **The first draft's premise was wrong, and the rest followed from
> it.** It said the window reloads through `Host::reload`. Nothing in
> the workspace calls `Host::reload`, and `lumen-static-winit` has no
> `reload` feature. So the motivating scenario, "you save, the reloaded
> code panics", cannot happen in any window today. **This milestone
> makes the window survive a panic and say where it happened, and it
> recovers by retry.** M37 adds the reload, and with it "a successful
> reload clears the screen".
>
> The prior art is `hot_ice` (`/mnt/Utvikling-linux/hot_ice`). Its macro
> wraps every hot function in `catch_panic` **inside the library**. A
> hook installed once *per loaded library* records `location()` into a
> thread-local, which is cleared before each call. The previous hook is
> deliberately forgotten rather than chained, because it may belong to a
> library that has since been unloaded. A failed `view` renders a
> "View Error" card, and any other failed function records an error
> entry in a bar over the application. The screen clears the moment a
> frame succeeds.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and prints the measured quantity beside its bound.*

---

## 0. The rules for this milestone

1. **"Where" means a line a person can open.** The screen shows
   `file:line:column`. When that location is inside the framework, it
   also shows the nearest frame in the *application's* code. A report
   that only ever points into `crates/lumen-static/src` answers the
   wrong question. T36.0 has a site that makes the framework filter do
   real work (§5, finding 4).
2. **The screen does not use the machinery that just failed.** It is a
   `DisplayList` built directly, not a widget tree (§2.3).
3. **Where it is compiled in** (decided 2026-09-23): in **debug
   builds**, and in **release builds with the `reload` feature**, which
   is a new feature of `lumen-static-winit` that forwards to
   `lumen-static-shell/reload`. Always under `cfg(panic = "unwind")`. A
   release build without `reload` keeps the default behaviour, because
   a shipped application should crash where a crash reporter can see it.
4. **One guarded entry, and every path goes through it.** The redraw,
   window events, IME composition, paste, ticks, and **the agent's
   `Live::pump`** all go through it. The review found that the agent's
   inputs bypass redraw entirely (§5, finding 1).
5. **Every site a panic can come from is exercised, not assumed.** There
   are seven, listed in T36.0.

## 1. What exists today

- **No window reloads.** `Host::reload` exists under the shell's
  `reload` feature and nothing calls it. `lumen-static-winit` links the
  application statically, and its `redraw` calls `tick`, `pump` or
  `pump_with`, `tick(0)` and `present`.
- **The agent drives the window through `user_event`.** `answer_agent`
  builds an `agent::Live`, whose `pump` calls `host.pump_with` and
  `backend.present` directly. `input.key` enqueues into the host's
  queue. The agent's key parser knows named keys only, with no letters,
  so `input.key R` is rejected today. Non-input methods see only the
  tree, and `AgentHost` has no accessor for host state.
- **Application code also runs outside `pump_with`:** `tick` before and
  after the pump (`App::tick` and timers), `compose` (IME) and `paste`
  from window events, and the agent's per-frame observe walk.
- **`fixtures/hot-app` is a `cdylib`**, so it statically links its own
  copy of std, including its own panic-hook global. A hook the loader
  installs never sees the library's panics. The dylib's entry points
  are `extern "C"`, and since Rust 1.81 a panic unwinding out of one
  aborts the process.
- **`pump_with` pops each event before dispatching it**, so when a
  handler panics, the event that caused it has already left the queue.
  Only events queued behind it remain.
- **Dispatch writes pointer state before any handler runs.** A press
  sets `hovered`, `pressed` and `input.pressed`, and only a release
  clears them.
- **`Backtrace::frames()` is unstable** (`backtrace_frames`). On stable,
  frames are read by parsing the `{:#}` rendering, which prints absolute
  source paths when debug info is present.
- M28's spawners catch job panics in `spawn.rs`. `Immediate` and
  `Queued` run jobs **synchronously inside `pump_with`**, so the hook
  fires on the frame thread for a panic that is not the frame's.
  `crates/lumen-static-shell/tests/failure.rs` swaps the process-wide
  hook while its tests run in parallel.

## 2. The decisions

### 2.1 Capture: a hook for the location, a catch for control

- **The hook** records the message, `location()` and, where debug info
  exists, a `Backtrace::force_capture()` rendering into a thread-local.
  It then calls the previous hook, so stderr still gets the ordinary
  report. *Chaining is safe here and was not in hot_ice*: this
  milestone never unloads a library, so the previous hook cannot belong
  to one. M37 has to revisit this if it unloads.
- **The guarded entry clears that thread-local before it runs**, as
  hot_ice does, so the stale record of a job panic that M28's spawner
  already caught cannot be reported as the frame's.
- **Robust to the hook being replaced.** If a panic is caught and the
  thread-local holds nothing, the hook has been replaced, for example
  by a test that swapped it. The report then says *location unavailable
  — the panic hook was replaced*, instead of showing a blank line, and
  the hook is reinstalled for next time. M36's tests live in their own
  test binary, away from `failure.rs`.
- **The application frame.** `application(..)` (M32) is
  `#[track_caller]`, so it learns the application crate's source path
  from `Location::caller()`, with no macro needed. The backtrace
  rendering is filtered to frames whose paths fall under that crate's
  directory, and the first one is reported. Without debug info, the
  screen says so.

### 2.2 Recovery: reset the host, keep the state, swallow input, retry

- **The host's own invariants are restored.** `list` and `overlays` are
  cleared, `retain` is reset so the next frame's damage is
  `Damage::All`, `mark_all()` is called and `arenas_dirty` is set.
  **Pointer state is reset**: capture is released, `input`'s hovered
  and pressed are cleared, and every `NodeState.hovered` and `.pressed`
  is cleared. Focus is kept.
- **Events queued behind the trigger in the same pump are dropped.**
  They were produced against a frame that never finished. (The first
  draft said the drain stops the *triggering* event re-firing. That
  event is already gone. §5, finding 6.)
- **Input is swallowed while the screen is up**, all of it except the
  retry key, close and resize, which are intercepted before
  `translate`/`post`. Nothing piles up in the 64-slot queue to replay
  on retry.
- **The application's state is kept as it is.** It may be partly
  mutated, and there is no last-good copy. hot_ice made the same call,
  for the same reason: a snapshot costs every frame, to protect against
  something that happens once per bad edit.
- **Retry is explicit.** It happens on R in the window, or through the
  agent's `app.retry` (§2.4). Nothing retries by itself, so a panic in
  `layout` cannot spin the CPU. A retry that panics again shows the new
  report. **Every agent `input.*` call is refused while the screen is
  up**, because pumping would be an automatic retry. M37 adds "a
  successful reload retries".

### 2.3 The screen is a display list, not a tree

A widget tree would go through the layout, measurement and paint code
that may have just panicked. So the screen is a `DisplayList` with a
fixed set of lines:

- a title, *"The application panicked"*;
- the message, wrapped to the window's width;
- the location, or *location unavailable* with the reason;
- up to eight application frames;
- the hint, *"Press R to retry"*, extended by M37 to *"…or save a change
  to reload"* only when reload is actually active.

If drawing the screen panics, the window gives up. The report has
already gone to stderr through the chained hook.

### 2.4 The agent sees the failure

`AgentHost` gains an accessor for the host's failure state. `app.failure`
returns the report, or null. `app.retry` retries. Both follow the
existing `app.*` names (`app.logs`, `app.perf`, `app.setPalette`).
Neither needs the agent's key parser to learn letters.

### 2.5 The dylib seam catches inside the library, with its own hook

A `cdylib` has its own std, so the capture machinery (hook,
thread-local and catch) lives in a crate the **library** links, and is
installed from inside the library on the first shim call of each
loaded build. A `Once` in the library runs once per loaded build,
because every `dlopen`ed copy gets fresh statics. The shim returns a
status, with the report written as plain bytes that the loader copies
straight away. `extern "C-unwind"` is rejected: the payload is a
`Box<dyn Any>` from another build, and a `TypeId` is not stable across
builds (`docs/00-handoff-static-tree.md`).

## T36.0 — the fixture, and today's behaviour recorded

> **DONE 2026-09-24.** `fixtures/panic-app`: seven sites, each panic
> line marked `// SITE n` and found by scanning the fixture's own source
> (`panic_app::site_line`). The first version recorded `line!() + k`
> constants and **four of seven were off by one to three lines** — the
> hand-copying the constants were meant to avoid. `tests/dies.rs`: every
> site dies today with its marker's line in stderr, site 7's location is
> inside `crates/lumen-static/src/`, and an unarmed run exits cleanly.
> *Injection:* site 4's key made a no-op — "site 4 did not die".


A fixture application that can panic at seven sites, chosen by a key:

1. a click handler;
2. an applied message;
3. `update` (a `#[bind]` source whose `Display` panics);
4. a widget's `layout`;
5. a widget's `paint`, both through a small custom widget in the
   fixture;
6. `App::tick`;
7. **a panic inside the framework**, caused by the application: the
   fixture hands a framework API an argument that makes a framework
   function index out of bounds, and the test asserts that
   `location()` is under `crates/` and not under the fixture.

Each site records its own `line!()` in a constant.

**Gate:** each site, run as a subprocess, **dies** today, and its stderr
contains the recorded line. This is the defect, shown, and T36.3 turns
it around. *Injection:* point one site's key at a no-op. That site's
"it died" assertion fires, which shows each site is reached and not
just declared.

## T36.1 — capture and host recovery, headless

> **DONE 2026-09-24.** `lumen_static_shell::recover` (the recording hook,
> `PanicReport`) and `Host::{guard, failure, retry, set_source_root}`;
> `program::application` is `#[track_caller]` and hands the host its
> caller's source directory. Gates in `fixtures/panic-app/tests`:
> `recover.rs` — all seven sites caught at their marker lines, site 7's
> application frame at the fixture's calling line (`lib.rs:136`) though
> the panic is in `style.rs`, no pointer state left, every retried frame
> full damage and byte-identical to a never-armed host after the same
> pointer move; events queued behind a trigger dropped; a job's panic not
> reported. `hook_replaced.rs` — a replaced hook reported as replaced,
> not as the stale job record. *Injections, each firing:* hook not
> installed, `retain` not reset, hover not released, queue not drained,
> record not cleared at entry.
>
> **Three findings from building it.** (1) **The shell is built with
> `debug-assertions = false` in dev** (the workspace profile, for speed),
> so a `cfg(debug_assertions)` in the shell was false in every debug
> build: the recovery the plan scoped to debug builds was absent from all
> of them. The shell now compiles recovery wherever panics unwind, and the
> *window crate*, which is built with debug assertions, decides when to
> use it (T36.3). (2) **The backtrace prints paths relative to the
> current directory** (`./src/lib.rs`, from a test), so the application
> frame was never found until relative paths were resolved first.
> (3) **The first damage assertion was vacuous**: `take_damage_for_age`
> accumulates what nobody took, so the retry frame read `All` with the
> `retain` reset removed. It now takes the damage before the retry.


§2.1 and §2.2 in `lumen-static-shell`. The core crate is untouched, and
it keeps no `unsafe` (`AssertUnwindSafe` is a safe wrapper).

**Gate:** for each site, the report's file and line equal the recorded
constant, printed side by side. Site 7's reported *application* frame is
the fixture's calling line. Then a well-defined comparison: **host A
panics and retries with the trigger cleared. Host B is built from the
same program and given the same inputs with the trigger disabled.**
A's retried frame must be byte-identical to B's frame. *Injections:*

- **Do not install the hook.** The location assertion fires, naming the
  site.
- **Do not reset `retain`.** The retried frame's `host.damage()` is not
  `Damage::All`. The gate prints it. (The first draft asserted on pixels
  here, and a headless render repaints the whole list regardless, so
  that injection could not fire. §5, finding 5.)
- **Do not reset pointer state.** Site 1's click leaves `hovered` set,
  and A's retried frame differs from B's by the hover shade, with the
  pixel count printed.
- **Do not drop the events behind the trigger.** The trigger key is
  queued twice in one pump. Without the drop, the second fires on retry
  and the test counts a second report.
- **A job that panics inside a frame produces no report.** `Immediate`
  runs it synchronously, and the stale-record clear is what keeps it out.

**The other direction:** a frame that does not panic goes through the
guarded entry byte-identical and with no report, and the allocation gate
stays at zero.

## T36.2 — the screen

> **DONE 2026-09-24.** `recover::failure_frame(report, w, h, measure)` —
> a display list, every line through one greedy `wrap` (a word wider
> than a line breaks by characters). `crates/lumen-static-shell/tests/
> failure_screen.rs`: every line inside the window at 480×320 and
> 260×420 (rightmost 451.5 / 234.0 px printed beside the widths),
> contrast 8.29 / 13.65 / 8.60 : 1 against AA's 4.5, the location and
> the application frame on screen, and goldens at both sizes (`truetype`
> off). **The viewport check found a defect on its first run**: only the
> message wrapped, and at 260 px the 22 px title ran to 288. The goldens
> recorded in that run were deleted and re-recorded after the fix.
> *Injection:* no wrapping — "a line runs 1089.0 px, past the window's
> 480", and the golden differs.


§2.3, as `failure_frame(&report, width, height) -> DisplayList`.

**Gate:**

- a golden image at two window sizes;
- **every text command lies inside the viewport**, with the rightmost x
  printed beside the width;
- title and body reach WCAG AA contrast, by the `verifying-apps`
  measure;
- the location line contains the fixture's file and line.

*Injection:* remove the wrapping. The viewport assertion fires with the
overrun printed.

## T36.3 — the window, and the agent

Rule 4's single guarded entry, used by the redraw, window events,
`compose`, `paste`, both ticks, the agent's observe walk and
`Live::pump`. §2.2's input policy. §2.4's two agent methods. The
`reload` feature on `lumen-static-winit`, and rule 3's `cfg`.

**Gate:** a drive script, in the manner of `scripts/widget_drive.sh`,
built **`--release --features reload`**. It asserts through `app.failure`
that recovery is compiled in (a build without it answers
*unsupported*, and the script fails rather than watching the process
die). It runs under both invalidation strategies, and for each site:

1. triggers the panic;
2. reads `app.failure` and asserts the file and line;
3. checks that a screenshot pixel in the title band is the screen's;
4. asserts that `input.click` is refused while the screen is up;
5. clears the trigger and calls `app.retry`;
6. takes a screenshot and compares it with a fresh window's, driven the
   same way without the trigger, printing the differing-pixel count.

*Injection:* make `app.retry` skip the `retain` reset. The backends
present damage regions, so stale failure-screen pixels survive, and
step 6 fires.

**And the build that must not have it:** a release build *without*
`reload` still dies on the first site. That is checked in the same
session, so the `cfg` is known to be switched both ways.

## T36.4 — the dylib seam

§2.5 in `export_reloadable_app!` and `fixtures/hot-app`. `just hot-demo`
gains a third build that panics in its frame verb and a fourth that
fixes it. The loader prints the report and carries on with the fourth.

**Gate:** the demo asserts the report's line and the recovered frame's
hash. *Injections:*

- **Install the hook only in the loader.** The report has no location.
  That is the `cdylib` fact, demonstrated.
- **Remove the catch from the frame verb.** The demo aborts, which is
  Rust 1.81's behaviour, and the script reports the abort instead of
  hanging.

## T36.5 — the docs

`docs/step4-hot-reload.md` gains the panic story: what is caught, what
is kept, and what is not recovered. `book/src/hot-reload.md` shows the
screen. `docs/00-handoff-static-tree.md` records the decision as
**DECIDED** and points here.

## 3. What this is not

- **Not a reload.** That is M37.
- **Not state rollback.** See §2.2.
- **Not a crash reporter for shipped builds.** See rule 3.
- **Not a change to task panics.** M28's `Failure::Panicked` still
  handles those. T36.1's job case shows the frame guard leaves them
  alone.

## 4. Ordering

M36 needs M32, for `application(..)`'s `#[track_caller]` source path and
for `Program`'s seam. M37 needs M36's in-library catch (T36.4). M34's
shadow oracle reports through the agent rather than panicking, so this
milestone's catch never swallows it.

## 5. What the review found (2026-09-23)

A seven-lens review, with an adversarial verifier per finding, confirmed
ten findings against the first draft (one blocking), plus two cross-plan
ones (X-3, X-4). What changed:

| # | finding | what changed |
|---|---|---|
| 1 | **blocking:** the agent's inputs go through `user_event` and `Live::pump`, not the redraw the draft wrapped. The key parser has no `R`. `app.failure` had no path to host state. And any agent input would auto-retry | rule 4, §2.2, §2.4 |
| 2, X-3 | no window reloads, so "a reload clears the screen" and the hint were unreachable | header, §1, §2.3. The reload is M37 |
| 3 | a `cdylib` has its own std, and a loader-installed hook never sees its panics | §2.5, and T36.4's first injection |
| 4 | `Backtrace::frames` is unstable, the application's root never reached the shell, and no site exercised the framework filter | §2.1, and T36.0 site 7 |
| 5 | "do not reset `retain`" could not fire headless, because every frame repaints the list | T36.1 asserts `damage()`. T36.3 compares a screenshot after the retry |
| 6, X-4 | the drain's rationale was wrong, because the triggering event is already popped | §2.2 restated. The injection queues the trigger twice |
| 7 | pointer state latched by dispatch was not reset (confirmed for `hovered`; `pressed` only from a widget's own `on_event`) | §2.2. T36.1 injection |
| 8 | a `Once` hook is lost when tests swap it, and a caught job panic left a stale record | §2.1: clear at entry, reinstall, and say "replaced" |
| 9 | tick, IME, paste and the agent's observe run outside `pump_with` | rule 4. T36.0 site 6 |
| 10 | input on the screen piled up and replayed on retry | §2.2: swallowed, with R intercepted before `post` |

Scope, decided 2026-09-23 and not by the plan: a release build shows the
screen when `reload` is on (rule 3).
