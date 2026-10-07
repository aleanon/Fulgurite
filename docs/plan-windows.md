# M89 — more than one window, and none at all (uses S1 and D1 move d)

> **Written 2026-09-28 as a section of `docs/plan-iced-roadmap.md`;
> revised 2026-09-29 after that plan's seven-lens review, and moved to
> its own file. Not started.** The shared byte inventory, seams (S1,
> S2), test and doc discipline, and plan-wide kill rules K1–K4 are in
> `docs/plan-iced-roadmap.md` §2–§4 and bind this plan; this file does
> not restate them.

*Split into **M89a** (size L: T89.0–T89.8) and **M89b** (size M:
T89.9–T89.11). Closes iced's **multi-window** and **`Daemon` API**
(M89a) and the shared GPU device a second window wants (M89b, which
may end on its kill rule without closing anything iced lists). Reopens
T15.10 on M54's condition, as amended by T89.0 (`docs/plan-a11y-platform.md`
§4): a two-window example driven through **one** agent port, with a
close that leaves the other answering. **Depends on** M88's S1 task
(`Request`, the request queue, `App::requests`, `Session::take_requests`,
`Session::answer`, `App::answered`, `Answer`). If M89 is scheduled
first, T89.3 lands that core under S1's names and M88 adds only its own
variants. T89.5 also depends on M88/T88.2–T88.3 and on M80/T80.5, and
builds only the parts whose producer has landed. **Consumers:** M90's
T90.6–T90.7 (a tray is a daemon) need T89.6 only; T90.1–T90.5 need
nothing here (M26). M89b depends on M89a, and on M83/T83.2 only for how a
factory backend treats the shared device.*

## Decisions

- **Windows and programs are named by data (D1 move d).**
  - A **window** is a `u32` key. `Win` allocates keys from 1 and never
    reuses one within a process. Key 0 names a daemon's windowless
    session (T89.6), never a window.
  - A **program** is a `u16` index. 0 is the program itself, and 1 and
    up are entries of its window table. A job, a request, a saved state
    and the loader hold only these numbers. Nothing holds a `fn` pointer
    into a library across a frame (rule 3, `docs/events-decision.md`
    :18–21, :124), and nothing holds a winit `WindowId` outside
    `lumen-static-winit` (P21).
  - Cost: 0 B per node. The key and index live in the loader's
    per-window state.
- **M89 adds five `Request` variants and four `Answer` variants to
  S1.** They live where S1 puts `Request` (`lumen-static`, behind
  `std`, as `spawn::Outbox` is).
  - Requests:
    - `OpenWindow { program: u16, tag }`;
    - `CloseWindow { window: u32 }`;
    - `Close`, which closes the window this session runs in;
    - `Post { window: u32, desc: Descriptor }`;
    - `Quit`, which ends the process.
  - Answers:
    - `WindowOpened { window: u32 }`;
    - `WindowClosed { window: u32 }`, sent to the opener when the user
      closes a window it opened;
    - `NoSuchProgram { program: u16 }`;
    - `Posted { from: u32, desc: Descriptor }`.
  - `Posted` and `WindowClosed` arrive unasked. They carry S1's reserved
    unsolicited tag, `u64::MAX` (S1's tag is a `u64`; the index doc's
    §2.8 reserves this value for M89).
  - **Bytes.** `Post` makes `Request` 32 B: a 24 B `Descriptor`, a
    `u32` and the discriminant, padded to 8. `tests/requests.rs` asserts
    the size. That is `CAPACITY × Δ` per application state that declares
    a queue, not per node.
  - The firmware links none of it. `fixtures/embedded` builds
    `lumen-static` without `std`. **The firmware row is an `nm` count,
    not a `.text` delta** (K4 as amended by the index's second review
    M26; M88/T88.3's shape). A `.text` diff would be a bound that
    cannot move, as `docs/results-tasks.md` T28.0 records for the inbox
    (B7, G32). T89.3 and T89.4 run `nm` on each
    `scripts/lib/firmware.sh` binary and print 0 symbols matching
    `OpenWindow|CloseWindow|Posted|WindowOpened|OpenProgram` (bound 0).
    The positive control is the same count on the
    `lumen-static-shell/tests/window_programs.rs` test binary, which
    must be > 0. **Injection** (M88/T88.3's, with M89's variants):
    remove `cfg(feature = "std")` from `Requests` and push a
    `Request::OpenWindow` from the embedded fixture's handler. The
    count becomes > 0 and the gate names the binary.
    `Style`, `NodeState` (at their sizes at landing, printed;
    `docs/plan-iced-roadmap.md` §2.1), `Cmd` 60 B and `Event` 16 B do
    not change. They stay asserted by
    `crates/lumen-static/tests/cursor.rs:32–33`,
    `crates/lumen-static/tests/focus.rs:203–204`,
    `fixtures/embedded/src/lib.rs`:409, :415 and :416 (the `const`
    asserts on `NodeState`, `Style` and per-node RAM), `tests/glass_cmd.rs`
    and `tests/gesture.rs`.
    Cross-window data is an `Answer`, never an `Event`.
- **A window program is a table the program carries, resolved in the
  build that is loaded.** All of this is in `lumen-static-shell`,
  `program.rs`.
  - `pub trait OpenProgram` is object-safe, with three methods:
    - `spec(&mut self) -> WindowSpec`, where `WindowSpec` is width,
      height, page, `Prefer`, and the images taken out;
    - `session(self: Box<Self>, recover: bool) -> Box<dyn Session>`;
    - under the shell's `reload` feature,
      `hot(self: Box<Self>, saved: Option<&[u8]>) -> Box<dyn Session>`.
  - It is implemented for `Program<A, I>`, where `A: WindowApp`.
    `WindowApp` is `App + Reloadable` under `reload` and `App` without
    it. The precedent is `impl Run for Program` under `hot`
    (`lumen-static-winit/src/lib.rs`).
  - `pub type WindowTable = fn(u16) -> Option<Box<dyn OpenProgram>>` is
    set with `Program::windows(table)` and read with
    `Program::window_program(i)`.
  - The table is an ordinary `fn` that the *program* holds. The loader
    only ever calls its **own** statically linked copy, which is loader
    code and is never swapped. A library's copy is reached only through
    `lumen_boot(index, …)` of the build that is loaded (T89.7), so no
    library pointer is held (A3, P22, M5, M6).
  - **Cost, recorded rather than hidden:** `session(recover)`
    instantiates both `Local<_, Recovering>` and `Local<_, Crashing>`
    for each table entry. A release build without `reload` that declares
    window programs therefore links M36's recovery code (M52 removed
    13,964 B of it from a plain window). `just release-window-shape`
    keeps measuring `examples/minimal`, which declares none and stays at
    0. T89.3 records the count for `examples/two-windows`.
    - The alternative is a third type parameter on `Program` carrying
      the table as a type. It would spread into `Local` and
      `hot::boot`, so it was not chosen.
    - Reopen condition: a request for multi-window in a release build
      where M52's bytes matter.
- **`Win` becomes shared state plus a `Vec<WindowSlot>`**
  (`lumen-static-winit`).
  - A slot holds the key, the program index, the opener's
    `(key, tag)`, and `Arc<Window>`, `Box<dyn Backend>`,
    `Box<dyn Session>`, the images, the cursor, `ClickCounter`, `Mods`,
    `cursor_shown`, `title_shown`, `last_frame`, `wait`, `own_redraw`,
    `must_present`, `present_drops`, `presents`, `input_at`, the a11y
    adapter, the hot bar flag, the keyed `ProxyWaker`, M80's
    `ime_area: Option<[f32; 4]>` (20 B, `docs/plan-text-fit.md` T80.5)
    and the render `FontStack`.
  - **The render font is per slot (second review X2).** M81 makes
    shaping a `Program` setting (`docs/plan-shaped-paint.md`,
    "Choosing is a `Program` setting"), and each table entry is its own
    `Program` with its own `Local`. A slot's stack is built
    `.with_shaping(slot.session.shaping())` when the slot is made, and
    its backend from that stack, not from one shared font (today
    `pick_backend(window, self.font.clone(), ..)`, winit `lib.rs`
    :1265–1266). A `Basic` primary and a `Complex` detail therefore
    each measure and paint with one stack.
  - `app.imeArea` resolves the `window` member like every other method
    and answers that slot's `ime_area` (second review G24).
  - `Win` keeps the shared parts: the clipboard, `hot`, `journal`,
    `quit`, `frames`, `report_after`, the proxy, `test_resume`, the
    primary `Config`, and the optional windowless session.
  - `window_event` finds the slot by `WindowId` with a linear scan.
    `Panes`' reasoning is kept in the rustdoc.
  - `UserEvent::Wake` becomes `Wake(u32)`, and `ProxyWaker::new` takes
    the key, so a worker wakes its own window. `examples/wake-gate`
    follows.
  - **`Pane` and `Panes` are deleted** (A27). `Pane<A>` holds a
    loader-side `Host<A>`, which M37 moved behind `Session` because
    loader code cannot be reloaded (`session.rs` module doc). Only the
    "deleted" branch was ever possible.
- **Closing a window and quitting are different (X5).**
  - **What closes one window:**
    - `CloseRequested` on a window;
    - `Request::Close` or `KeyOutcome::Quit` / `Session::wants_quit`
      from that window's session;
    - `KeyOutcome::Quit` / `Session::wants_quit` from a daemon's
      windowless session (key 0) closes nothing and ends nothing. It is
      ignored, with one stderr line naming `Request::Quit`, because key
      0 has no window (second review A21; M90's tray `on_menu` relies
      on it, `docs/plan-desktop-integration.md` "A menu is data" and
      T90.7 injection (5));
    - `Request::CloseWindow` or `app.close` naming a window.
  - **What ends the process:**
    - `Request::Quit`;
    - `app.quit`;
    - closing a non-daemon program's **primary** window (program 0).
      The primary session holds the program's state and is the agent's
      default target, and a program whose windows must outlive it is a
      daemon.
  - One-window behaviour is unchanged: counter-window's Escape still
    exits. `app.quit` keeps its meaning because `scripts/live/launch.py
    --stop quit` depends on it exiting 0. The rustdoc on
    `KeyOutcome::Quit`, `Host::quit_requested` and `Session::wants_quit`
    changes to "closes the window it came from".
- **The agent addresses a window by a top-level `"window"` member on
  one port** (one listener).
  - Absent means the **primary**: window 1, or for a daemon session 0.
    It is never the focused window, because every live script opens
    windows with `LUMEN_WINDOW_INACTIVE=1` (`scripts/drive_window.sh`,
    P23).
  - An unknown or closed key is `-32602`, and the error names
    `app.windows`.
  - `Win` answers `app.windows` (key, program, title, windowless) and
    `app.close` itself, **before** any backend lookup.
  - Every reply gains `"window": key`, so a drive prints who answered
    (G24, A28).
  - Only the primary program's `agent` and `accessible` apply. A table
    entry that sets an address is ignored, with one stderr line (M15).
  - `app.quit` is unchanged. Closing one window is `app.close`, not
    `app.quit` with a field. **Changed from X5's fix:** a method whose
    meaning flips on an optional field would let a stale script close a
    window where it meant to exit.
- **Data between windows is `Request::Post` (P20).**
  - `Win` delivers a post to the target session as
    `answer(unsolicited, Answer::Posted { from, desc })`, and the
    target's `App::answered` turns it into a `Msg` in the build that is
    loaded.
  - **Routing an answer into slot k asks k for a redraw** (second
    review A10). `answer` applies the `Msg` to k's state at once and
    sets k's `Host::answer_changed` (M88, second review A1), which only
    k's own `next_frame(.., more_deferred(), ..)` reads (winit `lib.rs`
    :982–986), and `answer_agent` asks a redraw of its own window only
    (:1162). So `Posted`, `WindowOpened` and `WindowClosed` into
    another slot set k's `own_redraw` and call its `request_redraw`;
    into the windowless session they schedule a windowless step. An
    idle target shows the post on its next frame, not its next input.
  - This is the "window-addressed message path" P20 asks for, instead
    of process-global state, which a hot library holds one copy of per
    loaded build.
  - One state with N views was rejected: `Host` is one tree and one
    surface, and making it two is the rework T15.10 avoided.
- **A daemon is `daemon(boot)`**, a `Program` with no window (shell).
  - `Win` builds its session as the windowless session, key 0, and
    opens no window.
  - Its timers, subscriptions and inbox run through
    `Session::frame(dt, 1, 1)`. The display list is dropped and nothing
    is presented. This is called from `new_events` at the session's
    deadline and from `Wake(0)`, the same two paths an idle window uses
    (M47/T47.1). **No new `Session` method** (A14's
    `tick_windowless` is rejected). An idle daemon's frame is a
    `Boundaries` skip.
  - **`Win` drains and answers the windowless session** after every
    windowless frame and after any post or answer routed into it
    (second review P9). M88's two drain sites, after the
    present-or-skip decision on every `redraw` and after an agent
    call (`docs/plan-platform-requests.md`, "Who answers"), never run
    for a session that is never redrawn and not addressed, so without
    this a daemon's `OpenWindow` and `Quit`
    would never leave its queue.
  - **`UserEvent::Built` and `Building` schedule a windowless step**
    when the windowless session exists (second review P13). Today both
    call `ask_redraw`, which does nothing without a window (winit
    `lib.rs`:782–786, :1127–1132), so a daemon with every window closed
    would not swap until an unrelated timer fired.
  - A daemon never exits on its last window closing.
  - The agent answers it through an `Offscreen` 1×1 screen, so
    `app.diagnostics` (the launcher's identity check), `ui.resolve` and
    `ui.getTree` work with no window (G23).
  - A panic in the windowless session is M36's failure state, readable
    through `app.failure` and cleared by `app.retry`. No screen shows
    it.
- **Hot reload boots every window from the build that is loaded (P22,
  M6).**
  - `export_program!`'s `lumen_boot` takes an index:
    `lumen_boot(index: u16, saved, len)`. 0 boots `program()`, and `n`
    boots `program().window_program(n)?.hot(saved)`. An unknown `n`
    returns null, with "no window program n" in `hot_take_panic`.
  - `hot::boot`, `hot::save`, `unpack` and `RESERVED` move from
    `lumen-static-reload` into `lumen-static-shell::session` under the
    shell's `reload` feature. `OpenProgram::hot` must reach them, and
    the shell cannot depend on the reload crate. The reload crate
    re-exports them.
  - **The moved `boot` gains `I: Invalidation`**, as `save` already has
    (second review P12). Today it is `boot<A: App + Reloadable>(program:
    Program<A>, ..)` (reload `lib.rs`:458) against `save<A, I>` (:415),
    and `OpenProgram` is implemented for `Program<A, I>`.
  - `Reloader` keeps the build it last swapped in and gains two things:
    - `open(index)`, a session from that build. `None` before the first
      swap, and the static table serves until then — **through
      `OpenProgram::hot(None)`, never `session(recover)`** (second
      review A17). `hot` installs the saver (reload `lib.rs`:458–459);
      `session` does not, and `Session::save` without a saver returns
      empty (`session.rs`:774–779), so a pre-swap window's
      `#[reload(keep)]` state would reset at the first swap.
    - `swap_all`, which boots **every** window's session and the
      windowless one from the new build before it replaces any. One
      refusal leaves them all on the old build (rule 2).
  - **`swap_all` adopts each session into its own slot** (second
    review A8). Today's closure takes everything from the one window:
    `set_waker(hot.waker)`, `set_scale`, `resize(w, h)` (winit
    `lib.rs`:806–812). Per slot, after the swap returns true:
    - `set_waker` with the slot's keyed `ProxyWaker` (the windowless
      session gets key 0's);
    - `set_scale` and `resize` from that slot's window;
    - `Backend::set_shaders` on the slot's backend (M83,
      `docs/plan-gpu-extension.md` "Delivery");
    - M81's shaping compare against the slot's own stack, with
      `Backend::set_font` and a repaint of that slot only;
    - M90's tray re-read where the session is a tray's.
    `hot::Hot`'s single `waker` (winit `lib.rs`:1682–1689) goes.
  - The swap moves from `redraw` to a `Win` step taken at the start of
    any slot's frame and at the start of a windowless step. The loop is
    single-threaded, so every session is between frames there.
  - **No `OpenWindow` is pending at a swap** (index §2.8, second
    review A19, P10). `Win` creates the window and answers
    `WindowOpened` (or `NoSuchProgram`) to the opener inside the same
    drain that took the request, before the drain returns. Every drain
    site runs inside an `ApplicationHandler` callback that holds
    `&ActiveEventLoop`, so `create_window` is reachable there; `redraw`
    and `answer_agent` gain that parameter (today they lack it, winit
    `lib.rs`:790, :1142). The swap step runs only at the start of a
    frame, never inside a drain, so no request crosses a build and no
    v1-drained/v2-answered gate is needed. Gated in T89.3 gate 2.
  - A window's `WindowSpec` (size, backend, images) comes from the
    loader's static table, as the primary's `Config` does today
    (`run_program`), because an `ImageSource` from the library would be
    library code held across a swap. An index v2 added that the loader
    does not know opens at `Config::default`'s size, with a stderr line.
  - The ABI stamp changes through the reload crate's source hash
    (M49/T49.1). No `abi_fields` entry is added, because M89 adds no
    shell feature.
- **M89 adds no `Session` method** (A14, P17). All host–window traffic
  goes through S1's two required methods, which M88 adds. The compiler
  is their forward gate (S1), so there is no stamp-change test or
  forward-dropping injection (C4, G1).
- **M89b: one wgpu device, a surface per window** (P25, K89a, K89b).
  - `lumen-static-wgpu` splits `Gpu` into two parts:
    - `GpuContext`, shared by `Rc`: the instance, adapter, device,
      queue, adapter name, the format-independent pipelines, bind-group
      layouts and samplers;
    - a per-surface `Gpu`: the surface, config, target, scratch, bind
      groups, instance buffers, the glyph atlas (windows may differ in
      scale) and the **image atlas with its `ImageSource`**. Two
      programs' `ImageId(0)` are different images (P25).
    - **A pipeline built against a surface format is not shared**
      (second review P11). Each surface picks its format from its own
      capabilities and the blit pipeline targets it (wgpu `lib.rs`
      :476–503, :626). The context caches such pipelines keyed by
      format, so a second surface with a different format builds its
      own instead of failing validation.
  - `Gpu::for_window` stays as "a new context plus one surface", so
    headless use and the single-window path are unchanged.
    `Gpu::for_window_in(&ctx, …)` adds a surface.
  - A surface the context's adapter does not support gets its own
    context, with one line on stderr.
  - A factory backend (M83/T83.2) makes its own device. The factory is
    taken from the primary `Program` once, never re-read on a swap, and
    called **once per window and per resume** (M13, D1 move b).
  - 174 MB is the GPU-over-software idle RSS in `docs/results-gpu.md`
    §5 (199.3 against 25.6 MB), cited, not restated elsewhere (B23).

## Tasks

- **T89.0 — gates that compile today, the baselines, and the amended
  reopen gate.**
  - `scripts/windows_drive.sh` and `.py` are added to
    `scripts/live.sh`'s list, with a `just windows-drive` recipe. Phase
    0 runs against `examples/counter-window` and asserts today's
    behaviour. Each assertion is labelled with the task that inverts
    it:
    - `app.windows` answers `-32601` (T89.2);
    - a request carrying `"window": 7` is answered by the one window
      anyway (T89.2);
    - `app.quit` exits 0 in under 2 s. This one stays, as the accept
      direction.
  - Its red run is recorded in the commit body: phase 0 with (a)
    inverted prints `app.windows: -32601 method not found`.
  - `docs/results-windows.md` is created (its `scripts/doc-paths.allow`
    entry from M80/T80.0 removed in the same commit) and records these
    baselines:
    - `just idle-count` on one window: frames, wakes and context
      switches in 60 s, with and without the 120 ms timer (the Idle
      section of `docs/results-frame-cost.md` is the source);
    - `just motion-present`'s presents for 200 motions (bound 2);
    - `just tasks-drive`'s unobserved ticks with its bound `want − 1`,
      which read 25–26 of 26 at T47.1 (`docs/plan-timers-wake.md`
      :346, C31).
  - **Docs:**
    - amend `docs/plan-a11y-platform.md` §4 and
      `docs/plan-capabilities.md` T15.10: the reopen gate is "one agent
      port, windows addressed by a `window` field, and a close that
      leaves the other answering". The departure from "one port per
      window" is named beside the old text (C31, G30, P24, M14);
    - a handoff OPEN marker for multi-window pointing at M89.
  - **Injection:** as above. The phase is red under (a) inverted and
    green as committed.
- **T89.1 — `WindowSlot`, routing by `WindowId`, keyed wakes; `Pane` and
  `Panes` deleted.** One window behaves as it does today.
  - **Docs and static checks:**
    - rewrite the `windows.rs` module doc as the slot's rationale, or
      delete the file;
    - `book/src/platform.md` loses `Pane`;
    - `Panes` goes into `scripts/doc-identifiers.allow` as "removed by
      M89/T89.1" wherever docs still cite it (`Pane` stays live as a
      test enum);
    - `crates/lumen-static-winit/tests/inval_param.rs` is rewritten
      against `WindowSlot`'s strategy-free session;
    - `examples/wake-gate` follows `Wake(u32)`.
  - **Gates:**
    - `just live` is green;
    - `just motion-present` reads ≤ 2 presents for 200 motions, and ≥ 20
      redraws asked for, printed. The setup landed only if that holds;
    - `just idle-count` reads the T89.0 baselines within their recorded
      spread (K89c).

    Routing by id **cannot be observed with one window**, so its gate
    is T89.3's XTest click.
  - **Injection:** leave `own_redraw` on `Win` instead of the slot, and
    never clear it on the slot's frame. `motion-present` prints ~200
    presents against bound 2. Accept direction: the committed build
    reads ≤ 2.
- **T89.2 — the agent's `window` member, `app.windows`, `app.close`,
  and a reply that names its window.**
  - `answer_agent` resolves the target first. `app.*` methods that
    belong to the window are answered before any backend lookup (G23,
    first half).
  - **Docs:**
    - `.claude/skills/verifying-apps/SKILL.md` "Methods that exist" and
      `book/src/agent.md` name `app.windows`, `app.close` and the
      `window` member, and `scripts/check_agent_methods.sh` stays green
      (G31, M16);
    - `ui.getTree`, not `ui.tree`, everywhere;
    - rustdoc on `Win::answer_agent`.
  - **Gates** (phase 1 of `windows_drive`, one window, both
    strategies):
    - `app.windows` lists exactly `[1]`, printed;
    - `"window": 7` answers `-32602` naming `app.windows`;
    - an `input.click` with `"window": 1` on counter-window's `+1` is
      `handled`, and the reply carries `"window": 1`.
  - **Injection:** the router ignores `window` and always answers from
    the primary. The `"window": 7` request is answered, and the phase
    fails by name. The accept direction is the `"window": 1` click.
- **T89.3 — window programs, open and close, and
  `examples/two-windows`.**
  - This task lands `OpenProgram`, `WindowTable`, `Program::windows`
    and `WindowSpec`, and `Request::{OpenWindow, CloseWindow, Close,
    Quit}` with their answers.
  - It also lands X5's semantics, the per-window a11y adapter, the
    per-slot render font (X2, second review), the redraw request on an
    answered slot (A10, second review; gated in T89.4), and
    secondary windows offset by +40, +40 px per key under
    `LUMEN_WINDOW_POS`.
  - `examples/two-windows` is a list window (program 0) whose rows open
    a detail window (program 1). The detail has its own counter
    `#detail-bump`, which exists in no other tree. There is an
    `everything` feature, following `examples/tasks-demo`, and a
    workspace member entry plus lock update (`check_locks.sh`).
  - **Docs:**
    - handoff "Platform integration": multi-window DECIDED → this plan;
    - `book/src/platform.md` gains a two-window section by named anchor
      from the example;
    - rustdoc on `KeyOutcome::Quit`, `Session::wants_quit` and
      `Host::quit_requested`;
    - `docs/results-windows.md` records the release-without-`reload`
      recovery-symbol count of `examples/two-windows` (the
      `OpenProgram` cost).
  - **Gates:**
    1. Headless, `lumen-static-shell/tests/window_programs.rs`:
       `window_program(1)` boots a session whose `title()` is the
       detail's, and `window_program(9)` is `None`, both printed.
       **Injection:** the table ignores its index, so 9 returns `Some`.
    2. Live, `windows_drive` phase 2, both strategies:
       - `input.click` on the list's row 2 is `handled`, and
         `app.windows` goes 1 → 2, printed;
       - answered inside the drain (second review A19, P10): the
         list's `WindowOpened` handler writes the key to `#opened`,
         and the first `ui.resolve #opened` after the click reply,
         with no frame waited for, reads `2`, printed. **Injection:**
         `Win` queues the open and creates the window at the next
         `about_to_wait`. `#opened` reads its boot text and the gate
         names "answered after the drain";
       - `#detail-bump` clicked with `"window": 2` is `handled`, and the
         detail's count reads 1 while the list's tree has no
         `#detail-bump` and its own values are unchanged (both trees
         printed, A28);
       - **OS routing:** an XTest click (python3-xlib, as
         `scripts/motion_present.py` does) at `#detail-bump`'s centre
         in window 2's screen origin raises the detail count to 2.
         Window 2 is found by `_NET_WM_PID` and title.
         **Injection:** `window_event` hands every event to slot 0, and
         the count stays 1 (printed). A missing python3-xlib fails
         the phase with the install line, never exit 77 on the
         reference machine (`scripts/live.sh` already refuses a skip;
         D7's rule).
    3. `app.close` on window 2 (or `Request::Close` from its Escape
       key) leaves window 1 answering `ui.getTree` and a `handled`
       click, and `app.windows` reads `[1]`. `app.close` on window 1
       exits the process with status 0 inside 2 s. **Injection:**
       `wants_quit` exits the loop (today's behaviour), and closing
       window 2 kills window 1 (the drive names the lost port).
    4. A panic in the detail window shows M36's screen there, and the
       list's click is still `handled`. **Injection:** `failed()` is
       read from slot 0 for every slot.
    5. One listener: `ss -ltnpH` counts the process's listening sockets
       and prints 1 against bound 1. The detail program sets
       `.agent("127.0.0.1:7999")` on purpose. **Injection:** start
       `agent::serve` for a secondary program's address, and the count
       is 2 (M15).
    6. `just a11y-drive` gains a `two-windows` leg: AT-SPI lists two
       frames for the pid, each with its button. **Injection:** attach
       the adapter for slot 0 only.
    7. With M81 landed, shaping per slot (second review X2): the list
       is `Basic`, the detail program declares `.shaping(Complex)` and
       shows an RTL label. The detail's rendered run matches its own
       `Local`'s cluster geometry (`ui.getTree` boxes against the
       painted glyph extents, printed). **Injection:** build every
       slot's backend from the primary's font. The run's extents differ
       from its clusters, and the gate names the label.
- **T89.4 — `Request::Post`: data between windows.**
  - **Docs:** the `Answer::Posted` rustdoc states which tag it carries.
    `book/src/events.md` places requests beside descriptors (named
    anchor).
  - **Gates:**
    - Headless, the pure `route_requests(from, requests, slots)` in
      `lumen-static-winit` (unit tests in `lib.rs`): a post reaches its
      target key with `from` set to the sender, and a post to a closed
      key is dropped and counted. **Injection:** `from` set to the
      target. The unit test names it.
    - Live (phase 3, both strategies): row 2 is clicked, the detail
      opens, and its `#item` reads "item 2", the row that was clicked.
      Rows 1 and 3 read "item 1" and "item 3", printed. The setup
      landed only if the click was `handled` and `app.windows` grew.
      **Injection:** `Win` drops posts, and `#item` keeps its boot text
      "no item".
    - Live, idle target (second review A10): the detail is already open
      and idle, the drive sends no agent call to window 2, and a post
      from window 1 updates `#item` within one frame; window 2's
      `app.presents` grows by 1, printed before and after.
      **Injection:** no redraw request on the target slot. `#item`
      keeps the old text and the present count does not move.
- **T89.5 — per-window answers for the requests that came before
  (M12).** A request is answered by the slot whose session it was
  drained from.
  - Built only for producers that have landed:
    - M88's `Screenshot` and `ClipboardRead`/`ClipboardWrite` (the
      clipboard stays one per process);
    - M80/T80.5's `set_ime_cursor_area`, called on the slot's own
      window.
  - If neither has landed, this task is recorded as empty and closes
    with the one that lands last.
  - **Docs:** rustdoc on the screenshot and clipboard request variants
    names the answering window.
  - **Gates** (phase 4, both strategies):
    - a `Screenshot` pushed by the detail's handler returns bytes whose
      hash equals `ui.screenshot` with `"window": 2` and differs from
      window 1's. Both hashes are printed;
    - with M80, the physical caret rect reaches window 2's IME, ±1 px at
      scale 1.0: `app.imeArea` with `"window": 2` reads the detail's
      focused field and `"window": 1` reads the list's (or `null`),
      both printed (second review G24).

    **Injections:**
    - answer every request from slot 0's backend. The hashes are equal,
      and the gate names them;
    - `app.imeArea` reads slot 0's `ime_area` for every window. Window
      2's answer equals window 1's, and the gate names it.
- **T89.6 — `daemon(boot)` and the windowless pump.**
  - `examples/daemon`: a 120 ms timer, and a `#ticks` label bound to its
    count on a tree nobody sees. It opens program 1 at tick 10 and
    closes it at tick 20. `Request::Quit` comes at tick 40. Its
    `on_key` answers `KeyOutcome::Quit` for Escape (A21's gate).
  - **Docs:**
    - handoff: `Daemon` DECIDED;
    - `book/src/platform.md`: the windowless program;
    - rustdoc on `daemon`, `Win::answer_agent` (G23),
      `Session::frame`'s windowless use;
    - `scripts/idle_count.sh` gains a `daemon` leg.
  - **Gates** (`windows_drive` phase 5, both strategies; the launcher's
    `app.diagnostics` identity check must pass with no window, which is
    itself the first assertion):
    - with no window open, one `ui.resolve #ticks` on `"window": 0`
      after 3.0 s unobserved reads `want − 1` or more, with want =
      elapsed ÷ 120, printed (the `scripts/tasks_drive.py` formula,
      C31);
    - `app.windows` reads `[0, 1]` after tick 10 and `[0]` after tick
      20, and the process is still alive;
    - the daemon exits with status 0 after `Quit`;
    - a windowless `on_key` that answers `KeyOutcome::Quit` (sent as
      `input.key` on `"window": 0` before tick 40) leaves
      `app.windows == [0]`, printed, and stderr carries the one line
      naming `Request::Quit` (second review A21);
    - K89d's idle leg is read.

    **Injections:**
    - pump the windowless session only inside a slot's redraw. `#ticks`
      reads 0, printed. The answer is 0, not an error, because G23's
      path is in place;
    - skip the drain after a windowless frame (second review P9).
      `app.windows` stays `[0]` after tick 10, printed;
    - treat a windowless `wants_quit` as `Request::Quit` (second review
      A21). The process exits before tick 40, and the gate names the
      lost port.
- **T89.7 — hot reload with several windows.**
  - This task lands the indexed `lumen_boot`, `boot_hot` moved into the
    shell, `Reloader::open` and `Reloader::swap_all`, and a swap step
    taken for every slot and the windowless session.
  - `fixtures/hot-window` gains program 1, a `#[reload(keep)]` panel
    count with an `// EDIT panel` format line, and an `everything`
    feature.
  - `fixtures/miri-reload` follows the new `lumen_boot` signature.
  - **Docs:**
    - `lumen_static::reload::symbols::BOOT` rustdoc;
    - `export_program!` rustdoc;
    - `book/src/hot-reload.md`, keeping `fixtures/hot-window`'s
      `program` anchor live;
    - `docs/plan-window-reload.md` gains a note that v1 still never
      unloads.
  - **Gates** (`just hot-window-drive`, extended, both strategies):
    - open the panel **before the first swap** (`app.reloads == 0`,
      printed), bump it to 3, edit the panel's format, and wait on
      `app.reloads`. The panel shows the **new** format with count 3,
      and the primary keeps its own count. Both are printed (second
      review A17);
    - the panel window is a different size from the primary. After a
      swap, a timer in the panel redraws the panel (its `app.presents`
      grows, the primary's does not), and its layout uses its own size
      (root box printed against `inner_size`) (second review A8);
    - a daemon leg: `examples/daemon` under `hot` with every window
      closed swaps on `Built`, and `app.reloads` on `"window": 0`
      increments, printed (second review P13);
    - a panel opened **after** the swap shows the new format;
    - a v3 whose panel boot panics leaves **both** windows on v2, and
      the bar shows the report.

    **Injections:**
    - (a) open new windows through the first loaded build. The late
      panel shows v1's format, which is rule 3's defect demonstrated;
    - (b) `swap_all` swaps slot 0 only. The panel keeps the old format;
    - (c) replace sessions as each one boots. The primary runs v3 while
      the panel runs v2, and the drive names both builds;
    - (d) open pre-swap windows through `session(recover)`. The panel's
      count reads 0 after the swap;
    - (e) adopt every slot with slot 0's waker and size. The panel's
      timer redraws the primary, and its root box is the primary's;
    - (f) `Built` calls only `ask_redraw`. The daemon's `app.reloads`
      stays put.
- **T89.8 — M89a closes.** `docs/results-windows.md` summary, and the
  AGENT.md queue entry citing `just ci-verify <sha>` green (S5). No
  behaviour and no other docs.
- **T89.9 (M89b) — gates that compile today.** After M89a these run
  with one context per window:
  - `just resume-drive` gains a `two-windows-gpu` leg: a debug
    `examples/two-windows` with `LUMEN_BACKEND=gpu` and
    `LUMEN_TEST_RESUME=4000`, each window read back with `xwd -id`.
    Differing pixels per window are printed against 0.1%, and the leg
    also records that two backends were made per resume (G35);
  - `docs/results-windows.md` records each window's `VmRSS` delta: one
    GPU window, then two, median of three launches (the
    `docs/results-gpu.md` §5 method).

  **Injection:** skip the second slot's `resume_backend`. Window 2 comes
  back blank, and the per-window count is printed.
- **T89.10 (M89b) — `GpuContext` and per-surface `Gpu`.** `Win` holds a
  `Weak<GpuContext>` and makes one when none is alive.
  - **Docs:**
    - `docs/results-gpu.md` §5 gains the shared row;
    - rustdoc on `Gpu`, `GpuContext` and `GpuBackend`;
    - handoff GPU section.
  - **Gates:**
    - the second GPU window's RSS delta is printed against K89b's bound;
    - T89.9's resume leg stays green (K89a);
    - `just gpu-parity` is unchanged;
    - each window's stderr backend line says `shared` and prints its
      surface format (second review P11);
    - each program gets an `ImageSource` whose `ImageId(0)` is a
      different solid colour. The pixel at each window's image centre is
      its own colour, printed.

    **Injections:**
    - move the image atlas into `GpuContext`. Window 2 shows window 1's
      colour;
    - share one `target` texture. The xwd readbacks of the two windows
      are equal.
- **T89.11 (M89b) — M89b closes.** Results summary, or K89a/K89b's
  record, and the queue entry with `just ci-verify <sha>` (S5).

M89 exports no widget, so none of S4's widget obligations apply (drive
inventory, observe, a11y role, showcase entry). M89 adds no cargo
feature, so K2 is not engaged.

## Kill rules

- **K89a** (M89b). The shared device loses a window's pixels (> 0.1%
  differing on either window) or panics across the `two-windows-gpu`
  resume leg (`just resume-drive`). **Fires →** every window keeps a
  context of its own (M89a's behaviour), and the number is recorded.
- **K89b** (M89b). With the device shared, the second GPU window's
  `VmRSS` delta is ≥ 50% of T89.9's separate-context delta (median of
  three, `docs/results-gpu.md` §5 method). **Fires →** not shipped,
  because sharing did not buy what it costs in code. Recorded.
  - **Amended by the owner, 2026-10-07: K89b is judged on the second
    GPU window's GPU share** — its GPU `VmRSS` delta less the software
    canvas's delta for the same window, against T89.9's separate-context
    share (17.5 MB) — **not on the raw delta above.** The text above is
    kept as written. Reason: the raw ratio is bounded below by the
    fallback fonts both backends pay (74.5 / 91.9 MB = 81%), so it could
    not tell a shared context from a separate one — a bound computed
    from the wrong quantity, recorded at T89.9 before T89.10 measured.
    It fired on that quantity at T89.10 and its response was followed;
    re-judged, the share read 0.9 MB (5%) on the side branch and 1.0 MB
    (6%) on the ported build, and T89.10 shipped
    (`docs/results-windows.md` T89.10). `just windows-rss` prints both
    ratios beside the bound. This is the owner's decision, not a re-read
    by the task that measured it.
- **K89c** (M89a). With one window, `just idle-count` reads more frames
  than T89.0's baseline, or `just motion-present` exceeds 2 presents
  per 200 motions. **Fires →** T89.1 stops and finds the per-slot state
  that leaked. It does not re-bound.
  - **This replaces K3 for M89.** K3's arms, the
    `benches-competitive/arms/frame` idle row and `benches --bin
    cascade`'s `cascade_deep (whole table)`, build neither `Win` nor a
    `Session`. For this milestone they are bounds computed from the
    wrong quantity (B10). S1's drain row is M88's.
- **K89d** (M89a). An idle daemon with no timer wakes more than 0 times
  in 60 s, or with the 120 ms timer runs more than one windowless frame
  per period (`just idle-count`'s daemon leg). **Fires →** T89.6 stops.
  The pump is being driven by something other than a deadline or a
  wake.

## What the review found (M89)

- **C14** — adopted: there is no Job answered by `Win`. M89 uses S1's
  request queue, drained by `Session::take_requests`.
- **C31** — adopted: the tick bound is `tasks_drive.py`'s `want − 1`,
  25–26 of 26 at T47.1, printed. The reopen gate is amended rather than
  copied verbatim (T89.0).
- **G23** — adopted: `Win` answers `app.*` before any backend lookup,
  and the windowless session answers through a 1×1 `Offscreen`. T89.6's
  gate reads 0, not an error, under its injection. Its requests are
  drained after every windowless frame (second review P9).
- **G24** — adopted, both routes: the agent router's injection is in
  T89.2, and OS-level `WindowId` routing is gated by an XTest click in
  T89.3.
- **G30** — adopted: T89.0 amends `plan-a11y-platform.md` §4 and records
  the departure.
- **G31** — adopted: `ui.getTree` is used throughout, and T89.2 updates
  the skill and `book/src/agent.md` under `check_agent_methods.sh`.
- **B10** — changed: K3's arms cannot see `Win`, so K89c
  (`idle-count`, `motion-present`) replaces K3 for M89. The arms are
  named in the rule.
- **B23** — adopted for M89: 174 MB cited to `docs/results-gpu.md` §5.
  The a11y 62-vs-71 manifest fix belongs to K2's owner (§0). M89 adds no
  feature.
- **A1** — adopted: no continuation `fn` is held. Answers become
  messages through `App::answered` in the build that is loaded.
  **Changed by the second review (A19, P10):** an answer never crosses
  a swap, because `Win` creates the window and answers `WindowOpened`
  inside the drain that took the request (Decisions, "No `OpenWindow`
  is pending at a swap"; T89.3 gate 2). T89.7's injection (a) proves
  which build boots a late window, a different question.
- **A3** — adopted: programs are indices into a table, resolved by
  `lumen_boot(index)` of the loaded build. Windows talk through `Post`.
- **A14** — changed: M89 needs **no** new `Session` method. S1's two
  methods, which are M88's, carry everything, and `tick_windowless` is
  rejected in favour of `frame(dt, 1, 1)`.
- **A27** — adopted: `Pane` and `Panes` are deleted in T89.1.
- **A28** — adopted: `#detail-bump` exists only in window 2, and both
  trees are printed.
- **P17** — adopted as S1. The per-method stamp test is dropped: the
  compiler is the forward gate (C4, G1).
- **P20** — adopted: `Request::Post` / `Answer::Posted`, gated by
  "detail shows the clicked item" (T89.4).
- **P21** — adopted: a `u32` key, a `u16` program index, and
  `WindowSpec` in the shell. No winit type leaves the window crate.
- **P22** — adopted: indexed `lumen_boot`, `Reloader::swap_all` and
  `open`, and the reload state stays on `Win`, not per slot. The
  post-swap adopt step is per slot (second review A8).
- **P23** — adopted: the default is the primary, never the focused
  window, and every drive runs under `LUMEN_WINDOW_INACTIVE=1`.
- **P24** — adopted (as G30).
- **P25** — adopted in M89b: `GpuContext` and per-surface `Gpu`, image
  atlas per window, images from each program's `WindowSpec`, and the
  resume leg covers windows opened at runtime.
- **M3** — adopted: no trailing docs task. The closing tasks carry only
  the results summary and the queue entry.
- **M5** — adopted: requests carry data, and the table is never held
  across a swap.
- **M6** — adopted: program-table export via the indexed boot (T89.7).
- **M12** — adopted: T89.5 answers from the draining slot, gated by a
  two-window screenshot.
- **M13** — adopted: a factory backend makes its own device and is
  called per window. Recorded as M89b's dependency on M83.
- **M14** — adopted: both `plan-a11y-platform.md` §4 and
  `plan-capabilities.md` T15.10 are amended (T89.0).
- **M15** — adopted: only the primary's `agent` and `accessible` apply,
  and the listener count is gated at 1.
- **M16** — adopted (as G31).
- **M18** — adopted: `book/src/platform.md`, `events.md`,
  `hot-reload.md` and `agent.md`. There is no "windows chapter".
- **M20** — adopted: every task names an injection, including T89.1,
  T89.5 and T89.7.
- **M26** — adopted: M89 is split into M89a and M89b, and only
  T90.6–T90.7 depend on M89 (T89.6). (The index's second review M26,
  a different finding, is in the list below.)
- **X5** — changed: close and quit are separated as proposed, but closing
  a window is `app.close`, not `app.quit` with a field, and closing a
  non-daemon's primary ends the process.

## What the second review found (M89)

Ids are the second review's own; G24, A17 and the rest here are not
the first review's findings of the same name.

- **G24** → adopted: `ime_area` is a `WindowSlot` field, `app.imeArea`
  resolves the `window` member, and T89.5 injects "read slot 0's
  `ime_area`".
- **A8** → adopted: `swap_all` adopts each session into its own slot
  (keyed waker, its window's scale and size, `set_shaders`, M81's
  shaping compare with `set_font`, the tray re-read). T89.7 gates two
  windows of different sizes, injection (e).
- **A10** → adopted: an answer routed into slot k asks k for a redraw.
  T89.4 gains the idle-target gate, with its present count printed.
- **A17** → adopted: before the first swap a hot window's secondaries
  open through `OpenProgram::hot(None)`. T89.7 opens the panel at
  `app.reloads == 0`, injection (d).
- **P9** → adopted: `Win` drains the windowless session after every
  windowless frame and after any answer into it. T89.6 injects a
  skipped drain. §2.8's "Who answers" and M88's drain sites are asked
  to name it.
- **P11** → adopted: format-keyed pipelines (blit) are cached in the
  context by format, not shared blindly. T89.10 prints each surface's
  format.
- **P12** → adopted: the moved `boot` gains `I: Invalidation`, as
  `save` has.
- **P13** → adopted: `Built` and `Building` schedule a windowless step
  when the windowless session exists. T89.7 gains a daemon leg,
  injection (f).
- **X2** → adopted, first route: the render font is per slot, built
  `.with_shaping(slot.session.shaping())`. Refusing a mismatched table
  entry was not chosen: it would forbid an RTL detail under a `Basic`
  list, which is M81's use. T89.3 gate 7 checks an RTL label under a
  `Basic` primary.
- **D6** → no change: M89 adds no `Painter` method. Its firmware row is
  the `nm` count (M26 below), not a `.text` delta.
- **D7** → applied by analogy: T89.3's XTest leg fails loudly without
  python3-xlib on the reference machine. M89 needs no `xclip`; T89.5's
  clipboard answers are M88's, gated there.
- **A19, M26, C9** (from `plan-iced-roadmap.md`) → changed: the
  statement route, not a v1/v2 gate. `Win` creates the window and
  answers `WindowOpened` inside the drain, so nothing is pending at a
  swap (Decisions, hot reload). T89.3 gate 2 reads `#opened` with no
  frame waited for; injection: open at the next `about_to_wait`. First
  review A1's ":663 cross-swap proof is T89.7's injection (a)" is
  corrected. K4: the firmware row is a positive-controlled `nm` count
  over each `scripts/lib/firmware.sh` binary (M88/T88.3's shape), with
  M88's reachability injection. Size asserts cited in full:
  `cursor.rs:32–33`, `focus.rs:203–204`, `fixtures/embedded/src/lib.rs`
  :409, :415, :416.
- **A1** (from `plan-platform-requests.md`) → adopted: `answer` applies
  the `Msg` at once and sets k's `Host::answer_changed`; no host-side
  queue. Routing and the redraw request are unchanged.
- **A18/P8** (from `plan-platform-requests.md`) → adopted: the redraw
  drain runs after the present-or-skip decision on every `redraw`. The
  windowless argument holds: neither site runs for a session that is
  never redrawn or addressed.
- **P10** (from `plan-platform-requests.md`) → adopted, with A19 above:
  stated in Decisions and gated in T89.3 gate 2.
- **A21** (from `plan-desktop-integration.md`) → adopted: a windowless
  `KeyOutcome::Quit` / `wants_quit` is ignored with one stderr line
  naming `Request::Quit`. T89.6 gates `app.windows == [0]` and the
  line; injection: treat it as `Request::Quit`, and the process exits.
- consistency-3 item 19 → "(one listener, D5)" loses "D5"; owner D5 is
  the 80 KB ceiling.
- consistency-3 item 24 → T89.0 removes `docs/results-windows.md`'s
  `doc-paths.allow` entry when it creates the doc.

## What building found (M89)

- **T89.0, the pins' red runs.** The plan's own red run, phase 0 with
  (a) inverted, prints `app.windows: -32601 method not found`. Inverting
  an assertion proves the script can fail, not that it watches the
  router, so each pin was also turned red by perturbing the mechanism it
  pins (§3.1, "a pin is still watched red"): a temporary `app.windows`
  answer in `Win::answer_agent` fails (a) by name, and a temporary
  `-32602` for any `"window"` other than 1 fails (b) by name. (c) is the
  launcher's existing `--stop quit` check.
- **T89.0, phase 0 runs one leg.** It asks the window crate's router,
  which the invalidation strategy does not reach, and `counter-window`
  has no `everything` build; T89.2's phase 1 is where both strategies
  start (§3.1 allows a stated reason).
- **T89.0, the baselines were read on private Xephyr displays**, the
  condition `docs/results-frame-cost.md` "Idle" requires for idle
  counts; `tasks-drive` on one also keeps its clipboard writes off the
  desktop's. T89.1's K89c re-reads use the same setup.
- **T89.1, `WindowSlot` is public, its fields are not.** The rewritten
  `tests/inval_param.rs` names a slot, so `WindowSlot` is re-exported
  with two accessors, `key()` and `session()` (the strategy-free
  `&mut dyn Session`); everything else is `pub(crate)`. The slot does not
  yet carry the program index or the opener's `(key, tag)`: nothing reads
  them before T89.3, which adds them with their first reader.
- **T89.1, the swap already uses the slot's waker.** `hot::Hot`'s single
  `waker` was the only copy of the primary's waker; with keyed wakers the
  swap's adopt closure takes the slot's own, so the field went here
  rather than in T89.7. The swap itself stays in the slot's `redraw` until
  T89.7 makes it a step over every slot.
- **T89.1, `new_events` with several deadlines.** A `ResumeTimeReached`
  is the soonest slot's instant; any other slot is due only once its own
  has passed, so one window's timer does not redraw another.
- **T89.1, `just live` was run without the all-widget drive** (the
  implementation brief forbids it on the shared desktop); `widget-drive`
  ran one widget (`Button`, both strategies, 0 failed) instead, and the
  rest ran on a private Xephyr display: 12 of 12 passed there.
  `resume-drive` failed there only on setup — glasswall's 997 px window
  does not fit Xephyr's 900 px screen, so the canvas and the window
  differ in height — and passed on the desktop (0 differing pixels on
  every leg).
- **T89.2, the slot carries its program index now.** `app.windows`
  reports each window's program, so the field T89.1 deferred lands here
  with that first reader (`WindowSlot::program`, 0 for the primary). The
  opener's `(key, tag)` still waits for T89.3, whose `WindowOpened` and
  `WindowClosed` are its first readers.
- **T89.2, `examples/counter-window` gains an `everything` feature**,
  following `examples/tasks-demo`: phase 1 runs under both strategies and
  T89.0 recorded that counter-window had no `everything` build. The
  strategy is chosen in `main` (`Program::invalidation`), not forwarded
  to the window crate.
- **T89.2, `app.close` is built with its method, and gated by T89.3.**
  On the primary it ends the process at the next `about_to_wait`, as
  `app.quit` does; any other window's slot is dropped. With one window,
  exercising it ends the leg, which the launcher's `--stop quit` reads as
  a death mid-drive, so its live gate is T89.3 gate 3, where a second
  window exists to survive it.
- **T89.2, phase 0 is replaced by phase 1**, not kept beside it: its two
  pins are exactly the assertions phase 1 inverts. The flip's red run —
  T89.0's phase 0 against the T89.2 router — fails (a) with
  `app.windows: answered {'windows': [{'key': 1, ...}]} -- want -32601`,
  and with (a) removed fails (b) with `"window": 7: input.click answered
  {'code': -32602, ...}`. (c), `app.quit` exits 0, stays as phase 1 (d).
- **T89.2, `ui.tree` appears nowhere as a method name**: a grep of the
  repository for it in backticks or quotes finds only this plan's own
  instruction, so "`ui.getTree` everywhere" needed no edit.
- **T89.3, `OpenProgram::hot` and `WindowApp` wait for T89.7.** `hot`
  calls the boot that T89.7 moves into the shell, so the trait lands
  with `spec` and `session` only and `Program<A, I>` implements it for
  any `A: App`. Until T89.7 a hot window's swap step touches the primary
  slot only: the reloader boots the library's program 0, and swapping a
  window program's slot would replace it with the primary's application.
- **T89.3, `WindowSpec` carries the entry's `agent` address.** The plan's
  spec is width, height, page, `Prefer` and images; the address is added
  so the window can print the one stderr line that it ignored it, and so
  gate 5's injection ("start `agent::serve` for a secondary program's
  address") has the address to start.
- **T89.3, gate 2's assertion could not fire under its injection.** A
  sequential `ui.resolve #opened` after the click's reply read `2` with
  the open deferred to `about_to_wait`: the loop goes idle between the
  reply and the next request, and the deferred open answers and asks a
  redraw too, so every later read converges. A read sent on a second
  connection 3 ms into the click also stayed green (the click is handled
  in under 3 ms). The only instant that differs is the click's own reply,
  so **a call whose drain opened windows carries `"opened": [keys]`**
  (`Win::answer_agent`), and the gate asserts it: green `[2]`; under the
  injection `None`, with `#opened` still reading `2`, which is the
  finding.
- **T89.3, gate 4's injection could not move `app.presents`.** With
  `failed()` read from slot 0 for every slot, window 2's panic screen
  still presented, and so did every later own redraw of it: a panicked
  session's frames carry their own damage, so `present_needed`'s forced
  flag never decided them. The per-slot read that *is* observable is the
  input swallow, M36's R-to-retry: R pressed in window 2 through XTest
  clears its failure, and under the injection the R is posted to the
  failed session as input and the failure stays (printed). The gate is
  that.
- **T89.3, gate 2's routing injection was narrowed.** "`window_event`
  hands every event to slot 0" also hands window 2's resize and redraw
  to slot 0, so window 2 never lays out and the drive fails at the
  agent's click, before the XTest step. Handing only pointer input to
  slot 0 leaves the agent path intact and fails the XTest step by name.
- **T89.3, the drive runs a release build with `reload`.** A release
  window without it does not catch panics (M36), so gate 4's panic would
  end the process; `examples/two-windows` gains a `reload` pass-through,
  as `fixtures/panic-app` has, and the launcher declares `reload` and
  `shaping` for every phase-2 leg.
- **T89.3, the firmware `nm` count cannot fire, as at T88.3.** With
  `Requests` ungated and an `OpenWindow` pushed from the embedded
  fixture's `on_inc`, the push inlines and leaves no symbol (0 on all
  four), while `.text` grows +88 / +84 / +84 / +84 B. The strict
  `just firmware-delta`, 0 on all four, is the gate; the count is
  printed beside it, with its positive control (3 symbols in the
  `window_programs` test binary).
- **T89.3, `Request::tag` answers `Option<u64>`.** `CloseWindow`,
  `Close` and `Quit` are never answered and carry no tag; the three
  callers that answered every request by its tag say so with `expect`.
- **T89.3, `WindowClosed` is sent on every close of a window that has
  an opener**, not only a close by the user: one rule, and an opener
  that closed a window itself learns it landed.
- **T89.4, posts are routed last in a drain.** `Win::window_requests`
  does a drain's opens and closes in order, then routes its posts against
  the windows open after them (`windows::route_requests`, pure, taking
  the open keys rather than the slots so it is testable without a
  window). A post names only a key its sender already knows, so none can
  name a window opened in the same drain; one behind a `Close` of its
  target is dropped rather than handed to a session about to go.
- **T89.4, the drop is counted on `app.windows`.** "Dropped and counted"
  needed a place to read the count: `app.windows` answers `posts:
  {delivered, dropped}`, and a drop prints one stderr line naming both
  keys. Phase 3 (c) reads it live.
- **T89.4, the idle-target injection moves the pixels, not `#item`.**
  With no redraw request on the target slot, `ui.resolve #item` on window
  4 read the new text anyway: `Session::answer` applies the message at
  once, so the tree is ahead of the screen (the T89.3 shape again). What
  the injection holds still is the present count (3 → 3) and the pixels,
  so the gate reads `ui.regionHash` over `#item` — the canvas last
  drawn, which draws no frame — before and after, beside
  `app.presents`. The injection is posts-only: the A10 redraw in
  `answer_window` serves every answer, and removing it there would also
  break `WindowOpened`.
- **T89.4, the list posts from its `apply`.** The post for a row is
  pushed when the list hears `WindowOpened`, so it waits on the list's
  queue for the list's next drain — its next frame, which the answer
  asked for — and reaches the detail within 0–4 ms of the click's reply.
  Phase 3 (a) polls `#item` with a 3 s bound and prints the time.
- **T89.5, both producers had landed and nothing was missing.** M88's
  `Screenshot` and clipboard requests and M80/T80.5's IME area are all
  built, and both drain sites already pass the draining slot's session
  with **that slot's** backend (T89.1), as `app.imeArea` already reads
  the slot the request names (T89.2). So the task is its gates, which
  hold that in place, and the rustdoc on the screenshot and clipboard
  variants; no code changed in the window crate.
- **T89.5, the screenshot's answer is shown in the title.** The detail
  hashes the answered pixels (FNV-1a, as `examples/tasks-demo` does) and
  the drive compares that with `ui.screenshot`'s. A label carrying the
  hash would change the very pixels the next `ui.screenshot` hashes, so
  the detail's title is a function of its state and carries it instead;
  `app.windows` reports each window's title.
- **T89.5, the IME gate uses the harness's caret rule.** "±1 px of the
  physical caret rect" is read the way `scripts/drive/harness.py`'s
  `_ime_check` reads it: the caret column found in window 2's screenshot
  against the area's centre, never `caret_rect` compared with itself.
  Its injection is checked for equality before nullness, so it fails by
  the name the plan gives it ("window 2's answer equals window 1's").

- **T89.6, the windowless session is a slot.** Key 0 sits in `Win`'s
  `Vec<WindowSlot>` with no window, renderer or adapter, rather than in a
  field beside it, so the router, `app.windows`, the keyed waker and the
  deadline fold (`soonest_wait`) serve it unchanged; `resumed` skips it,
  `close_window` never closes it, and "the primary" is the slot running
  program 0 (a daemon's windows are all table entries, so closing the last
  ends nothing). A windowless step is taken from `about_to_wait` when it is
  due: its deadline (`new_events`), its wake (`Wake(0)`), an answer or post
  routed into it, and an agent call addressing it — the last two where a
  window would ask a redraw — so an idle daemon still takes none.
- **T89.6, nothing asked for a daemon's first frame.** A window's first
  frame is the `request_redraw` in `open_window`; no deadline is armed
  before a frame computes one, so a daemon with no endpoint never ticked —
  every live gate passed, because the launcher's `app.diagnostics` is an
  agent call and kicked it. Found by running `examples/daemon` with no
  endpoint (still running at 8 s); `Win::new` now marks the first
  windowless step due, and phase 5 gained a no-endpoint leg (`--stop wait`,
  exit 0 within 10 s), red under the removed first step (killed at 10 s).
- **T89.6, the daemon's timer is its own deadline.** A `Timers::every`
  firing is delivered by a job, so it costs two frames a period — the
  deadline's and the wake's (`docs/results-frame-cost.md`, "Idle") — and
  K89d's "more than one windowless frame per period" would fire on the
  mechanism the window crate does not choose. `examples/daemon` counts with
  `App::wake_in_ms` and `App::tick`, one frame a period, which is what K89d
  bounds.
- **T89.6, K89d's two quantities, read as measured.** "Wakes more than 0
  times" is read as the loop thread's wakes beyond the two sampling reads'
  own: a still window reads 2 (T89.0), and a still daemon read 1. "More
  than one frame per period" is bounded at periods + 1: an interval of
  N.x periods holds N + 1 deadlines, and the first run, bounded at N, read
  501 against 500 — the fencepost, not a second frame. Both are judged by
  `scripts/idle_count.py`, which exits 1 when K89d fires.
- **T89.6, the tick gate reads twice, as `tasks_drive.py` does.** "One
  `ui.resolve #ticks` after 3.0 s unobserved" against `want = elapsed ÷
  120` needs a start: the ticks count from the daemon's boot, which the
  drive cannot see. So it reads once, waits 3.0 s while polling only
  `app.windows` (answered by `Win`, which runs no frame), and reads again,
  `want` being the interval's — `tasks_drive.py`'s formula exactly. Ticks
  10 and 20 fall inside the interval, so the [0, 1] and [0] reads are the
  deadline's alone.
- **T89.6, `Built` and `Building` still only ask redraws.** The decision
  that they schedule a windowless step (second review P13) is gated by
  T89.7's daemon leg and injection (f), and lands there with the swap step
  it serves.
- **T89.7, the primary needed a way to ask for the panel.**
  `fixtures/hot-window`'s application derives `App`, and the derive had no
  `requests` key, so its handlers could push no `Request::OpenWindow`.
  `#[app(requests = path)]` is added (`App::requests` for a derived
  application, beside `images`), with its row in both attribute tables and
  the unknown-key error's pin updated; the panel itself is hand-written,
  because its `Timers` tick is not derivable either.
- **T89.7, "before the first swap" is staged by a failing first build.**
  The drive's warm build makes the window's own first build land within a
  second of launch, before a drive can open and bump a panel. So the
  copy is broken after the binary and the warm library are built: the
  window's first build fails, nothing swaps, and the fix — written in the
  same edit as the panel's new format — is the first swap.
- **T89.7, injection (a) is read as "through the binary's own table".**
  With the first successful swap carrying the edit, "the first loaded
  build" that would show v1's format is the one the window started with;
  the injection opens every late window through the static table, and the
  late panel reads `panel 0`.
- **T89.7, a refused build may be booted twice.** One edit can make the
  watcher build twice; the second build's bytes match the refused one,
  and `Reloader` retries a library that was refused when it is built again
  (its rustdoc's rule), so v3's panel boot panics once or twice.
  `scripts/live/launch.py --expect-panics` takes `N-M` for that leg (1-2);
  every other leg stays exact.
- **T89.7, the daemon leg runs on a copy too.** The launcher's identity
  check is an agent call on window 0, so it takes a windowless step of
  its own, and the first build can land before or after it: injection (f)
  could not fire on the first swap. The drive waits for that swap, then
  edits the copy (`target/hot-live/ws/daemon`), so the second build lands
  with nothing else happening — under (f) it stays pending.
- **T89.7, the root box needed something to fill it.** A column hugs its
  content, so the panel's root box read 260 × 96 in a 260 × 180 window
  under the committed code; a `FILL` spacer (`#panel-fill`) makes the
  root's box the window's logical size, which is the quantity the gate
  compares.
- **T89.7, every window program in a `reload` build is `Reloadable`**
  (`WindowApp`, as the plan says): `examples/two-windows`' detail and
  `examples/daemon`'s panel derive it with every field `fresh`, because
  `two-windows --features reload` turns the shell's `reload` on.
- **T89.9, the resume leg found a defect before it could pass.** Its
  first green attempt read both windows 100% different after the round
  trip, one flat colour each: `GpuBackend` asked its session for damage
  at age 1 on every present, and a backend made by a resume has a target
  holding no frame, so an unchanged `Boundaries` session drew nothing
  into it and the empty target was presented. Its first present now asks
  for age 0 (softbuffer's answer for a new buffer). glasswall's
  `gpu-resume` leg could not see it: the window manager resizes its
  997 px window on a 1,080 px monitor, and a resize resets retention.
  The defect's red run is that attempt; the plan's injection (skip slot
  2's `resume_backend`) reads window 2 at 100% and "1 by it (want 2)".
- **T89.9, the leg's hook is at 8,000 ms, not 4,000.** `LUMEN_TEST_RESUME`
  counts from `Win::new`, and a debug launch that opens two GPU contexts,
  one per window, used most of 4 s: the first reads straddled the round
  trip (`xwd -id` met a BadWindow error on a window the suspend had destroyed).
  The leg polls the detail's first present instead of sleeping, and
  refuses a read the hook's line precedes or interrupts.
- **T89.9, `xwd -id` is found again after the round trip.** `resumed`
  makes a new X window per slot, so the leg finds both windows by
  `_NET_WM_PID` and title before each read rather than holding an id.
- **T89.9, K89b's quantity holds 74.5 MB the GPU does not cost.** The
  second window's `VmRSS` delta is +91.9 MB on the GPU, but +74.5 MB on
  the software canvas too: the detail lays Arabic out, and each slot's
  font stacks load fallback faces until one has it. A shared context
  cannot remove what the software window also pays, so half of the raw
  delta (46.0 MB) is a bound a shared second window would exceed by its
  fonts alone — a bound computed from the wrong quantity. `just
  windows-rss` prints the GPU's share beside it, the difference of the
  two backends' second-window medians (+17.5 MB), and T89.10 reads K89b
  on both.
- **T89.10, K89b fired and the sharing is not shipped.** `GpuContext`
  shared by `Rc`, a surface per window and `Win`'s `Weak` were built as
  the Decisions describe and measured: K89a held (0 differing pixels on
  both windows across the round trip, no panic, the backend lines
  `shared context (2 surfaces)` for window 2, each program's
  `ImageId(0)` its own colour), and the second GPU window's `VmRSS`
  delta read 75.4 MB against T89.9's 91.9 — **82%, against a bound of
  50%**. Its response was followed: every window keeps a context of its
  own, and the implementation is on the side branch
  `t89.10-shared-gpu-context` (`9cc2e22`). **The quantity, as T89.9
  recorded before the measurement, is the wrong one**: 74.5 MB of both
  arms' delta is the detail's fallback faces, which the software canvas
  pays too, so the raw ratio cannot fall below 81%. On the GPU's share
  (each arm's GPU delta less the software delta of the same launches)
  sharing took a second window from 17.5 MB to 0.9 MB, 5%. Re-judging
  K89b on that share, and shipping from the side branch, is an owner
  decision, not this task's (`docs/results-windows.md` T89.10).
- **T89.10, shipped on the owner's decision (2026-10-07).** K89b is
  judged on the GPU share (its amendment, "Kill rules"), and the side
  branch's `9cc2e22` was re-applied onto the M90a tree. It applied
  without a conflict: M90a's `Win` changes (menu state after every
  frame, the tray seam, `Event::Menu`, accelerators) do not touch
  `pick_backend` or `resumed_impl`'s backend line. Re-measured there:
  K89a 0 / 0 differing pixels, K89b's share 1.0 MB of 17.5 (6%), raw
  82%. `just windows-rss` now prints both ratios against T89.9's
  separate-context arm rather than half of its own medians.
- **T89.10, a shared `target` cannot make the readbacks equal.** The
  plan's second injection predicts the two windows' `xwd` readbacks
  are equal; `examples/two-windows`' windows are 340 × 440 and 300 ×
  400, so a shared target shows **the same frame stretched to each
  window**, never byte-equal ones. The leg still goes red on it, by
  the mechanism: after the round trip window 1 presents the detail's
  frame, its `#swatch` reads the detail's blue, and 16.4% of its pixels
  differ (`docs/results-windows.md` T89.10).
