# M37 — hot reload inside a window, modelled on `hot_ice`

> **DONE 2026-09-24, T37.0–T37.5.** Written 2026-09-23 and asked for the same day. The
> review of M36 found that no window can reload, so a panic screen could
> only ever be left by retrying. The instruction was to look at
> `hot_ice` (`/mnt/Utvikling-linux/hot_ice`) for inspiration. This plan
> is what that reading suggests, adapted to where this framework
> differs from iced.
>
> **What exists:** `fixtures/hot-host` and `just hot-demo` swap a real
> `cdylib` headlessly, with `#[reload(keep)]` state carried by
> `save`/`restore` and pending M28 chains carried as data (T28.7). There
> is no window, no `Host`, no build step and no watcher. The loader
> leaks every library it has loaded.
>
> **What hot_ice does, in brief:**
>
> 1. It builds the UI crate itself with
>    `cargo rustc --crate-type cdylib` into a separate target directory,
>    then re-runs the build on every save.
> 2. It watches the output `.so` and accepts a build only when its CRC
>    changes. Each build is copied to a unique path, which defeats
>    `dlopen`'s cache, before it is loaded.
> 3. It keeps the application state in the host and resolves each hot
>    function by symbol on every call. Its reload handshake — pause,
>    serialize, swap, deserialize, resume — runs at a frame boundary.
> 4. It shows compile errors over the running application: the last
>    good build keeps running.
> 5. It catches panics per function, inside the library.
> 6. It unloads retired libraries once their async work has drained.
> 7. It runs `boot` exactly once, in the host.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and prints the measured quantity beside its bound.*

---

## 0. The rules for this milestone

1. **Nothing constructs through a pointer held across a swap.** This is
   `docs/events-decision.md` rule 3. The new library is asked for its
   own `program()` (M32), and M28/T28.7's injection is this milestone's
   injection too (T37.4).
2. **A failed build or a failed swap never takes the window down.** The
   last good library keeps running, and the failure is shown over it,
   as hot_ice shows it.
3. **The swap happens between frames, never inside one.**
4. **Measured before it is called usable:** reload latency (save to
   first new frame) and memory per reload, because v1 unloads nothing
   (§2.5).

## 1. Where this framework differs from iced, and why that decides the architecture

hot_ice can keep the state in the host and call into the library
because iced's `view` **rebuilds the widget tree every frame** from the
state. No code pointer outlives a call.

**Here the tree is retained, and it holds code.** Handler `fn` pointers
live in the tree, and every widget's `layout` and `paint` is
monomorphised over the application's types. A `Host<A>` compiled into
the loader is the *loader's* code and would never change. So:

- **The `Host` lives in the library.** The loader owns the window, the
  backend, the event loop and the agent socket. That is the direction
  M31/T31.3 already found: a loader cannot name the application type
  across the boundary.
- **Per frame, the loader hands in input and gets back a frame**: a
  display list, its damage, the title and any failure. The backends
  need a present call that takes a list rather than a `Host<A>`, since
  `Backend::present` takes the host today.
- **The agent's requests are forwarded** into the library as the JSON
  they already are, because the tree they inspect is there.

## 2. The decisions

### 2.1 The application crate is a library and a binary

`src/lib.rs` defines `pub fn program() -> Program<App>` (M32) and
invokes `lumen_static_reload::export_program!(program)`. `src/main.rs` is
`fn main() { app::program().run() }`. Without the `reload` feature
that is the whole story: the application is statically linked. With
it, `run()` starts the **static** application at once, as hot_ice's
static fallback does, and then swaps to the first hot build when it
lands, carrying state through the same `save`/`restore` as any other
swap. So there is no start-up wait.

### 2.2 Construction on a swap re-runs the new build's boot

The new library's exported entry calls its own `program()`, runs boot,
and grafts the saved `#[reload(keep)]` fields on top. Boot may capture
(M32 §2.1), because it is re-created by the new build's code rather
than held from the old one. **This retires `Reloadable::fresh()` from
the window path**, which is the last place M32 §2.3 left a second
constructor.

### 2.3 The boundary: pointers with an ABI stamp, or bytes

Two options, and T37.0 chooses **by measurement**:

- **Rust ABI, borrowed pointers** (hot_ice's choice). The loader
  borrows the library's `DisplayList` for the frame. Nothing is copied,
  but this is sound only if both sides lay out the types identically.
  So the library exports an **ABI stamp**: the rustc version, the
  `lumen-static` version and feature set, and the `size_of` and
  `align_of` of every type that crosses. The loader refuses a library
  whose stamp differs and shows why.
- **`extern "C"` with plain bytes**, as the fixture does today. This is
  sound by construction, and it costs a copy of the display list every
  frame.

**Kill rule, written before the probe:** if copying showcase's display
list costs less than the frame arm's fast-cluster spread, use bytes and
drop the stamp. Otherwise, use pointers with the stamp.

### 2.4 State that no longer fits

A `#[reload(keep)]` field whose type changed incompatibly fails to
restore. hot_ice falls back to the default and logs it. Here, **that
field keeps its boot value, and an error bar names the field and the
serde error.** The application keeps running on the new code. Nothing
is refused, because refusing the swap would trap the user on the old
code with a field they have already changed.

### 2.5 Retired libraries are leaked in v1

hot_ice unloads a retired library once its async worker has drained.
Here three things would have to be proven first: every M28 thread that
may run old code must be joined, no `&'static` from the old library may
survive (M31/T31.3's stylesheet selectors are the known case), and the
chained panic hook (M36 §2.1) must not point into it. **v1 never
unloads**, measures what that costs per reload, and records unloading
as deferred with those three conditions.

### 2.6 Build and watch without `cargo-watch`

hot_ice spawns `cargo watch`, which is an external install. Here, a
`notify` watcher on the application crate's sources triggers
`cargo rustc -p <app> --lib --crate-type cdylib --features reload` into
`target/reload`. The build runs as a child process whose stderr the
window shows. On completion, the output's CRC is checked, the file is
copied to `lib<app>-hot-<N>.so`, and the swap is scheduled for the next
frame boundary. Builds are coalesced: a save during a build queues
exactly one more.

## T37.0 — the boundary, measured

> **DONE 2026-09-24 — pointers, behind a stamp.** A real byte encoding
> (`lumen_static::wire`) exceeds the frame arm's spread on three of
> showcase's four probed entries. A `memcpy` would not, but it is no byte
> boundary at all. The numbers and the two things the rule did not count
> are in `docs/step4-hot-reload.md` (T37.0). `lumen_static_reload::abi`
> is the stamp, and `just abi-stamp` is the gate: an identical build is
> accepted, and an `observe` build is refused by `lumen-static features`.
> *Injections:* the feature field removed makes the `observe` build
> accepted, because no listed size moves. A check that refuses
> everything refuses the identical build.

Probe: the size of showcase's display list, its copy cost per frame, and
the list of types that would cross. Apply §2.3's kill rule, and write
down the choice and the number in `docs/step4-hot-reload.md`.

**Gate:** if the stamp is chosen, a library built with a different
feature set is refused, and the refusal names the first field that
differs. *Other direction:* an identical build is accepted.

## T37.1 — `export_program!`, the library side

> **Landing 1, 2026-09-24: the window drives a `Session`.** The winit
> handler is no longer generic over the application. Everything it asks
> of one goes through `lumen_static_shell::session::Session`, which is
> object-safe and passes only `Copy` or caller-owned values, plus the
> display list a renderer borrows. `Local<A>` is the statically linked
> implementation. `Backend` takes a `&mut dyn FrameSource` in place of a
> `Host<A>`, and `sync_size` reports whether the host must be told.
> `Ret` and `Inval` moved to the shell, since a hot library builds the
> same `Host` without linking a window system. The agent's `Live` moved
> there as well, behind a new `live-agent` feature, because the tree
> lives wherever the session does. M36's recovery policy is decided in
> the window crate as ever (`RECOVER`) and handed to the session. This is
> a refactor, so its evidence is the gates it had to keep green:
> `just test`; `just agent-drive` (both strategies, shadow oracle quiet
> over 29 frames); `just widget-drive` (94 actions, both strategies);
> `just panic-drive` (all seven sites, plus release without `reload`
> dying); and the tasks drive (five capabilities, both strategies).

The entry points are:

- boot-with-saved-state;
- frame (events and `dt` in, frame out);
- save;
- agent request;
- drop.

Each is wrapped in M36/T36.4's per-library catch and hook.

**Gate:** a headless loader test drives v1, swaps to v2, and compares
v2's frame with a fresh v2 host given the restored state, byte for
byte. *Injection:* **hold v1's handler `fn` pointer across the swap**,
which is M28/T28.7's injection. The frame answers with v1's text, and
the gate prints both strings.

> **Landing 2, 2026-09-24: `export_program!` and the headless gate.**
> `lumen_static_reload::hot` has both sides (feature `window`). The
> library side is `boot`: the library's own `program()`, with the saved
> bytes grafted on through the new `Host::restore`, so construction is
> the new build's. The loader side is `Build::open`, which checks the
> stamp (now with the shell's fields, including a hash of `session.rs`,
> since the trait's vtable is part of the ABI), then `Build::boot`. A
> `Hot` hands its session back to the library to be dropped there,
> caught. `export_program!` exports the stamp, `lumen_boot`, `lumen_drop`
> and `hot_take_panic`.
>
> The gate is `just window-reload-demo`. `fixtures/reload-app` has v1
> formatting "count {}" and v2 formatting "clicks: {}". It is clicked
> once in v1, swapped, and compared with v2 clicked once from fresh, and
> the two agree over 3 commands, byte for byte. **The first run
> disagreed**: the direct session still had the pointer over its button
> and drew it hover-shaded. Hover is not kept state, so the pointer
> leaves before the comparison, and the assertion has fired on something
> real. *Injections:* constructing through **v1's** `lumen_boot` after
> the swap (the plan's injection) gives a frame reading `count 1`, v1's
> format; skipping the restore gives `clicks: 0`.

## T37.2 — the loader, in `lumen-static-winit`

> **DONE 2026-09-24.** `lumen-static-winit`'s `hot` feature, which
> implies `reload`, has these parts:
>
> - **Builder.** `hot::builder` is a thread running `Crate::build`
>   (`cargo rustc -p <app> --lib --crate-type cdylib` into
>   `target/reload`). Saves that arrive during a build coalesce into
>   exactly one more.
> - **Watcher.** `hot::watch` uses `notify` on the crate's `src/`.
> - **Swap.** It happens in `redraw` before the frame, through
>   `lumen_static_reload::hot::Reloader`. That hashes the library, skips
>   an unchanged one, copies a changed one to a path of its own, and boots
>   the new session **before** dropping the old, so a refused or
>   panicking build leaves the old one running.
> - **Status.** A bar over the application shows why a build did not
>   take (`hot::Barred`, the session's list plus the bar). The title
>   reads "Reloading…" during a build, and `app.reloads` reports the
>   count, whether a build is running, what is pending, and the error.
> - **Start-up.** With `hot`, `Program::run` requires a `Reloadable`
>   application. It starts the static one at once and fires the first
>   build (§2.1).
> - **Features.** The library is built with `LUMEN_HOT_FEATURES`
>   (default `hot`). The stamp refuses a library built otherwise, naming
>   the field. The stamp gained `debug_assertions`, since the loader and
>   library are two invocations that could be given two profiles.
>
> **Gate** (headless, in `just window-reload-demo`): a failed build whose
> output path holds v2's library leaves the frame at v1's `+ | count 1`
> with the log kept; the next good build swaps to `+ | clicks: 1` and
> clears the error; the same bytes again queue nothing. *Injection:*
> the plan's own, a failed outcome that swaps anyway. The frame after
> the failed build reads `+ | clicks: 1`, and the assertion fires.

§2.6's build and watch, the swap at a frame boundary, the list-taking
present, the agent forwarding, and the status surface: a
*Reloading…* indicator in the title (hot_ice prefixes it), and the build
log in an error bar when a build fails. M36's screen handles a panic,
and **a successful swap retries it**, which is the "reload clears the
screen" M36 deferred.

**Gate:** see T37.4. This task's own gate is headless. A failed build
leaves the running library in place, and the next good build swaps.
*Injection:* swap on any build completion, including a failed one. The
test asserts that the frame is still v1's after a failed build, and
fires.

## T37.3 — state across the swap

> **DONE 2026-09-24.** Restore was already per field: the derive
> restores each kept field on its own and logs one whose saved value no
> longer fits. A window library's log is a thread-local in its own copy
> of the core, so `hot::boot` drains it right after the restore and
> hands it over with the session (`Session::take_restore_log`). Bytes
> that cannot be read at all are logged too, where they used to be
> silently ignored. `Reloader::warning` carries the log after a swap
> that **took**, as distinct from `error`, which means a swap that did
> not. The window bar heads each differently (`hot::FAILED`,
> `hot::RESET`), and `app.reloads` reports both. **Gate**
> (`just window-reload-demo`): `fixtures/reload-app` v3 changes the
> kept `name` from `String` to `u32`. From v2's `+ | clicks: 1 | name
> first` the swap completes to `+ | clicks: 1 | name 7`, and the warning
> reads `Counter.name: incompatible saved value: invalid type: string
> "first", expected u32`. *Injection:* restore made all-or-nothing, by
> re-booting whenever any field fell back. The frame reads `clicks: 0`,
> and "every other kept field survives" fires.
>
> **Not carried across a window swap: M28 work in flight.** The old
> session's inbox and its running jobs stay with the old session, which
> is dropped. `fixtures/hot-host` proves descriptors can cross as data
> (T28.7), but the window's `Session` has no method that hands pending
> descriptors over, so a chain started before a reload does not complete
> after it. Recorded, not built.

§2.4, plus the M28 carry-over that `fixtures/hot-host` already proves.
**Gate:** change a kept field's type between v1 and v2. The swap
completes, that field holds its boot value, the error bar names it, and
every other kept field survives with its value printed. *Injection:*
make restore all-or-nothing. Every field resets, and the per-field
assertion fires.

## T37.4 — the live gate

A drive script against a real window built with `--features reload`:

1. Read a bound label through the agent.
2. Bump a kept counter.
3. **Edit the source**: change the format string with `sed`.
4. Wait on an `app.reloads` counter.
5. Read the label. It must show the new text **and** the old count.
6. Introduce a panic, and see M36's screen through `app.failure`.
7. Fix it, and see the screen clear on the next swap.

*Injections:*

- **Skip `restore`.** The count resets, and step 5 fires.
- **Swap on a failed build.** The window dies or shows a stale frame,
  and the script fails at the build-failure step, which it runs between
  steps 4 and 6.

> **DONE 2026-09-24.** `just hot-window-drive` runs every step above
> against a live window. `fixtures/hot-window` is **copied** into a
> workspace of its own under `target/hot-live`, and its copy is edited,
> never the checked-in source. The failed-build step comes between 5
> and 6. It checks four things: the error names E0308, the label still
> reads `clicks: 2`, a click still counts under the last good build, and
> the bar is on screen. Fixing the source back is byte-identical to what
> is running, so it must clear the bar **without** a swap. Latency and
> memory are in `docs/step4-hot-reload.md` (T37.4).
>
> **Two defects, found by driving it.** First, fixing a failed build
> back to the running source left the error up forever. The rebuilt
> library hashed equal to the running one, so the outcome returned
> before clearing anything. The hash is now recorded on a successful
> swap, not on queueing, and a matching build clears the error. Second,
> **the bar outlived its fix**: its pixels were the window's, never in
> the session's damage, so nothing repainted where it had been. That
> held in every older swapchain buffer too, so the window now calls
> `Session::repaint_all` (`Host::repaint_all`, through the retained
> history) on the frame the bar goes away. *Injection:* without that
> call, the bar pixel still reads (101, 30, 33) after 2 s of polling.
>
> *The plan's injections:* skipping the restore gives `clicks: 0` at
> step 5. Swapping on a failed build first surfaced as a ten-minute
> **timeout**, because the swap cleared the error the drive was waiting
> for. The wait now stops on any swap too, and the injection fails fast
> and by name: "a failed build swapped: reloads 3".

## T37.5 — the docs and the book

> **DONE 2026-09-24.** `docs/step4-hot-reload.md` has the windowed
> architecture and §1's reason for it, next to T37.0's boundary numbers
> and T37.4's latency and memory. `book/src/hot-reload.md` now opens on
> a window: `fixtures/hot-window`'s program, main and features are
> included by anchor, and the program snippet compiles and runs, since
> `just book-deps` builds `hot-window`. *Injection:* its assertion
> changed goes red. `docs/00-handoff-static-tree.md` records the
> decision as **DECIDED**.

`docs/step4-hot-reload.md` gains the windowed architecture and §1's
reason for it. `book/src/hot-reload.md` is rewritten around a window,
not the headless fixture. `docs/00-handoff-static-tree.md` records the
decision.

## 3. What this is not

- **Not unloading.** That is §2.5, deferred with its conditions.
- **Not state migration.** A changed type gets its boot value (§2.4),
  not a conversion.
- **Not a replacement for the headless fixture.** `just hot-demo` keeps
  proving the swap without a display.

## 4. Ordering

It needs **M32**, for `program()` and the description a new build is
asked for, and **M36**, for the in-library catch (T36.4) and the screen
a successful swap clears. It is independent of M33, M34 and M35.
