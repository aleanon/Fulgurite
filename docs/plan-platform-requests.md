# M88 — the runtime missing pieces: Sipper, screenshot requests, rich clipboard (defines S1)

> **Written 2026-09-28 as a section of `docs/plan-iced-roadmap.md`;
> revised 2026-09-29 after that plan's seven-lens review, and moved to
> its own file. Not started.** The shared byte inventory, seams (S1,
> S2), test and doc discipline, and plan-wide kill rules K1–K4 are in
> `docs/plan-iced-roadmap.md` §2–§4 and bind this plan; this file does
> not restate them.

*Size M: six commits, T88.0–T88.5. Closes three iced items: `Sipper`,
screenshot as a task, and rich clipboard. **Depends on nothing.** M89
depends on T88.3, because its `OpenWindow`/`CloseWindow`/`Quit` are
variants appended to T88.3's `Request`, and its per-window routing
follows from T88.3's rule that a session's requests are answered to
that same session. T88.2 changes `frame_rgba`'s receiver on `Backend`.
If M83 has already moved `Backend` into `lumen-static-shell` (D1(b)),
the edit happens there. Otherwise it happens in
`crates/lumen-static-winit/src/backend.rs` and M83 moves the new
signature. The first draft modelled all three items as `Job`s. That
cannot be built: a `Job` is
`{desc, job: JobFn, on_fail}`, `Copy`, and run by a spawner on a
worker (`spawn.rs:115-131`, `Host::spawn` at shell `lib.rs:1305`). No
job body can reach `Win`, a handler cannot build a `Job` at all (it
pushes a `Descriptor` into an `Outbox`), and every continuation the
draft named was a `fn` pointer held across frames (rule 3). This
section replaces that design.*

**Decisions.**

- **A sipper is a job body that calls `Sender::progress`. Progress
  coalesces into a one-value slot, never blocks, and never passes
  through the channel.**
  - *Mechanism.* `inbox::JobTrack` becomes `JobTrack<M>` and gains a
    `latest: Mutex<Option<M>>`, a `fresh: AtomicBool` and the epoch
    the sender was minted at. `Sender::progress(&self, m)` replaces
    the slot's value, counting the value it replaced in
    `Inbox::coalesced`. It wakes the loop only when `fresh` goes from
    false to true, so the loop gets at most one wake per drained
    value. `Host::pump_with` checks the ledger's slots before it
    drains the channel (`lib.rs:1849`). For each slot whose `fresh`
    is set, it takes the value and applies it with `A::apply`. A value
    carrying a stale epoch is dropped and counted in `Inbox::stale`.
    **A slot value that `apply` reports as a change ORs into the same
    `applied_changed` the inbox drain sets** (shell `lib.rs:1848-1866`),
    before the idle-skip check (`:1933-1937`). That flag is the only
    route to `mark_all` for an unsplit application
    (`!A::STATE_IS_SPLIT`) and to `arenas_dirty`; without it, a
    `Boundaries<256>` frame whose only news is a progress value skips
    and paints the stale label (second review A2).
    `spawn::Ledger` becomes `Ledger<M>`. `JobTrack::settled` also
    requires `!fresh`, so a job cannot leave the ledger while its last
    value is undelivered.
    - M84/T84.7 adds `JobTrack::stop` to the same struct
      (`docs/plan-images-in-motion.md` M84b). Whichever lands second
      extends the other: the slot drain honours `stop` (the value is
      dropped and counted in `Host::stopped_dropped`), and `settled`
      treats a stopped job as settled (second review M11).
  - *What is guaranteed.* A sipper's pump applies at most one progress
    value per pump, and the last value `progress` stored is always
    applied. `progress` and `send` are not ordered relative to each
    other. A body that needs an ordered final message sends it with
    `progress`, because the last `progress` value cannot be coalesced
    away.
  - *Why not blocking.* `Sender::send` records the decision against it
    (`inbox.rs:190-197`: "Refuses rather than blocks … on a
    single-threaded spawner a deadlock"). `Immediate` runs the body on
    the frame thread (`spawn.rs:429-446`), and `Threads` caps at
    `MAX_WORKERS = 4` with timer ticks queued behind busy workers
    (`spawn.rs:176-182`). While a panic is shown, or across a swap,
    nothing drains. A coalescing slot needs no back-pressure, so the
    blocking send and the per-job quota of 8 are dropped. This
    answers `docs/plan-tasks.md` §4's "a streaming task needs
    back-pressure": the slot is the back-pressure.
  - *Rule 3.* Progress is converted to `M` by the job body, on the
    worker, from the build loaded at spawn. That is the accepted
    `JobFn` form (`spawn.rs:9-24`). No `on_progress`/`on_done`
    pointers exist anywhere. Across a swap, the sipper's descriptor is
    in the ledger, so `Host::reissue` re-runs it through the new
    build's `job_for` (M49/T49.10, at least once). The old worker
    writes into a slot that nothing reads any more.
  - *No `Job::sipper`.* `App::job_for` and `JobFn` are unchanged, and
    any job body may call `progress`. An untracked sender
    (`Inbox::sender`, used in tests) has no slot, so its `progress` is
    a plain `send`. The rustdoc says so.
  - *Cost.* `size_of::<JobTrack<M>>()` grows by
    `size_of::<Mutex<Option<M>>>() + 9` bytes, inline in the `Arc`
    that `tracked_sender` already allocates, so there is no new
    allocation per spawn. T88.1 prints the size for `M = u64`. The
    per-pump slot check is O(jobs in flight), which is zero on an idle
    frame. `Style`, `NodeState`, `Cmd` and `Event` do not change (D4
    is not engaged). Everything is behind `feature = "std"`, like the
    inbox, so the firmware links none of it (D5's 81,920 B ceiling and
    1,000 B per-task cap are not touched).
  - *Crate:* `lumen-static` (`inbox.rs`, `spawn.rs`).
- **S1 is defined here: platform requests are data, answered by the
  current build.**
  - *Core types, in a new `crates/lumen-static/src/request.rs` behind
    `std`, as `Outbox` is:*
    - `Request` is a `Copy` enum: `Screenshot { tag: u64 }`,
      `ClipboardWrite { tag: u64, kind: ClipKind }`, and
      `ClipboardRead { tag: u64, kind: ClipKind, format: ClipFormat }`.
      Its variants are append-only. M88 asserts its size at 16 B. M89
      appends five (`OpenWindow`, `CloseWindow`, `Close`, `Post`,
      `Quit`; `docs/plan-windows.md` owns them), and `Post`'s 24 B
      `Descriptor` takes `Request` to 32 B, which M89 re-asserts. M88
      adds no window key; the `u32` window key is M89's.
    - `ClipKind { Clipboard, Primary }` and `ClipFormat { Text, Html,
      Image }`, each `u8`-sized.
    - `Requests` is a fixed-capacity queue, `CAPACITY = 8`, that
      refuses when full and counts the refusal. It is a copy of
      `Outbox`'s shape (`spawn.rs:522-590`) and allocates nothing.
    - `Clip<'a>` is what the app writes: `Text(&str)`,
      `Html { html, alt }` or `Image { width, height, rgba }`.
    - `Answer<'a>` is what comes back: `Image { width, height, scale,
      rgba }`, `Text(&str)`, `Html(&str)`, `Written`, or
      `Refused(Refusal)`, where `Refusal` is `{ Unsupported, Empty,
      Failed }`.
  - *How an application issues one.* This mirrors how it reaches the
    `Outbox` (`event.rs:1040`; `examples/tasks-demo` keeps its outbox
    on its `#[state]`). A new `App::requests(&mut self) -> Option<&mut
    Requests>` defaults to `None`. A handler, an `apply` or a boot
    pushes a `Request` onto its state's `Requests`. A handler cannot
    reach the host, so this is the only route, as with the outbox.
  - *What the app writes.* `App::clip_for(&mut self, tag) ->
    Option<Clip<'_>>` defaults to `None`. The host asks it while
    draining a `ClipboardWrite`, synchronously, in the current build.
    `None` is answered `Refused(Empty)` at once.
  - *How an answer becomes a message.* `App::answered(tag, Answer<'_>)
    -> Option<Self::Msg>` defaults to `None`, like `App::failed`
    (`event.rs:1133`). It is an associated fn, called at answer time
    in the build loaded then. `None` is counted in
    `Host::unmodelled_answers`.
    - **`Host::answer` applies the resulting `Msg` at once**, with
      `A::apply(state_mut(), m)`, inside the call. It is **not** parked
      on a host-side `Vec<A::Msg>` for the next pump (the first
      draft): `Win::redraw` swaps before it pumps (winit
      `lib.rs:802-825`), so a `UserEvent::Built` landing between the
      drain at the end of redraw N and the pump of redraw N+1 would
      replace the session with a v1 `Msg` still queued. `hot::save`
      carries only kept state and `Host::reissuable()` descriptors
      (reload `lib.rs:415-420`, shell `lib.rs:1499-1502`) and cannot
      carry a v1 `Msg`, so the answer would be lost after the platform
      had acted (second review A1).
    - Applied at once, the answer's effect is ordinary state: kept
      across a swap exactly when the field is `#[reload(keep)]`, like
      anything else a handler wrote. Nothing converted is pending.
    - A change reported by that `apply` sets `Host::answer_changed`,
      which the next pump ORs into `applied_changed` before the
      idle-skip check, as the slot drain does (second review A2), and
      which `Host::more_deferred` reports, so the window asks for the
      frame that shows it.
    - It is **not** posted to the inbox: a refused post at
      `CAPACITY = 256` would drop an answer the platform has already
      acted on (the P15(5) shape).
  - *The session seam.* This adds two **required** `Session` methods:
    - `take_requests(&mut self) -> Vec<Outgoing>`. `Outgoing` is
      `{ request: Request, payload: Payload }`, and `Payload` is an
      owned `{ None, Text(String), Html { html, alt: String }, Image {
      width, height, rgba: Vec<u8> } }`. Both are shell types.
      `String` and `Vec<u8>` already cross the seam in `copy` and
      `save`. An empty queue returns `Vec::new()`, which does not
      allocate.
    - `answer(&mut self, tag: u64, a: Answer<'_>)`.

    `Local` and `Hot` (reload `lib.rs:833`) implement both.
    `fixtures/miri-reload`'s `Session` is a local two-method stand-in
    (`count`, `bump`; `main.rs:22-35`), not the shell's trait, so it
    has nothing to implement (second review P18); M89/T89.7 owns its
    `lumen_boot` change. Because the methods are required, a
    missing `Hot` forward does not compile, so the compiler is that
    gate. `Local::answer` and `Local::take_requests` run `answered`,
    `apply` and `clip_for` inside the session's panic guard, the M36
    entry that `frame` uses.
  - *Who answers.* `Win` drains `take_requests` in two places in M88
    (three once M89 lands; see below):
    - **in `redraw` on every frame, after the present-or-skip
      decision**, whether or not `backend.present` ran. A frame with
      no damage presents nothing (`present_needed`, winit
      `lib.rs:888-904`, M53/T53.4), and a request pushed on it would
      otherwise wait for an unrelated present on an idle window. The
      drain sits after the `if self.session.failed() { return; }` at
      `:917-919` and before `next_frame` reads `more_deferred()`, so
      it is **skipped while the panic screen is up** (M36: a panic
      being shown runs no application code, `session.rs:607-612`), and
      a request pushed before the panic waits on its queue until a
      retry. Second review A18, P8, R21.
    - after every `session.agent` call.

    It answers every request before it returns: `Screenshot` from
    `backend.frame_rgba()`, which on a skipped frame is the last frame
    drawn and still on screen, and the clipboard requests from
    `Clipboard`.
    - M89 adds a third site: after every windowless `frame(dt, 1, 1)`
      and after any answer routed into the windowless session
      (`docs/plan-windows.md`, second review P9).
    - **No M88 request outlives the call that drained it, and no
      answer outlives the `answer` call that applied it**, so nothing
      is pending at a swap. M89 decides that no `OpenWindow` is either
      (`docs/plan-windows.md`; `docs/plan-iced-roadmap.md` §2.8): `Win`
      creates the window and answers `WindowOpened` inside the drain
      that took the request, gated in T89.3 gate 2. Second review P10: the first draft said it was the first
      pending request and that M89 carried its cross-swap gate; M89 has
      no such gate, so the claim is removed.
    - A headless `Host` answers nothing by itself. A test calls
      `Host::take_requests` and `Host::answer`.
    - A session's requests are answered to that same session. M89
      keeps a session per window, so a screenshot answers from the
      window that asked. M12's routing falls out of this, and M89's
      two-window gate owns the injection.
  - *Cancel.* A request is not a job. `App::take_cancel` bumps the
    inbox epoch and does not withdraw a request that has already been
    taken. The rustdoc says so.
  - *Cost.* `Requests` is 16 × 8 = 128 B of slots plus the queue
    header after M88, and 32 × 8 = 256 B plus the header after M89's
    `Post`, carried only by an application that declares one. `Win` pays one vtable call per
    frame, returning an empty `Vec`. There is no `Style`, `NodeState`,
    `Cmd` or `Event` change, and nothing reaches the firmware.
  - *Crates:* `lumen-static` (`Request`, `Requests`, `Clip`, `Answer`,
    `ClipKind`, `ClipFormat`, `Refusal`, and the three `App` hooks),
    `lumen-static-shell` (`Outgoing`, `Payload`, the `Session`
    methods, `Host::take_requests`/`answer`), `lumen-static-reload`
    (`Hot`'s forwards) and `lumen-static-winit` (`Win::serve_requests`).
- **A screenshot is what the renderer drew, and the GPU gains
  `frame_rgba`, not `fetch_rgba`.**
  - `fetch_rgba` stays `None` on the GPU, as `winit/src/gpu.rs:132-140`
    records: the render target is not the present.
  - `frame_rgba` changes from `&self` to `&mut self` on `Backend`,
    `Screen` (`session.rs:78`) and `AgentHost::frame`
    (`agent/src/lib.rs:1970`). `Gpu::read_rgba` is `&mut self`
    (`wgpu/src/lib.rs:1591`). `GpuBackend::frame_rgba` reads the
    target into an owned `shot: Vec<u8>` on each call and lends it
    out. Nothing is read back on a frame that nobody asked about.
  - The callers already hold `&mut`: `handle_input` (`lib.rs:771`),
    `Live::pump_app`, `AsScreen` and `Offscreen`. `ui.regionHash`
    reads `host.scale()` before `host.frame()`, to end the borrow.
  - The mismatch this closes is `docs/plan-timers-wake.md`:641-643,
    "`ui.screenshot` cannot see the GPU backend".
- **Rich clipboard is reach plus formats. Only images need a new
  feature.**
  - The pinned arboard 3.6.1 (`Cargo.lock`) exposes `set_html` and
    `get().html()` **ungated**. Primary selection comes through the
    `GetExtLinux`/`SetExtLinux` extension traits with
    `LinuxClipboardKind::Primary`. Only `set_image`/`get_image` sit
    behind arboard's `image-data`, which pulls the `image` crate.
  - Text, HTML and primary therefore ride the existing default-on
    `clipboard` feature, with **0 new crates**. Primary is answered
    `Refused(Unsupported)` off Linux.
  - Images go behind a new default-off `clipboard-image = ["clipboard",
    "arboard/image-data"]`, measured under K2. The first draft called
    this `clipboard-rich`. It is renamed because HTML and primary are
    not in it.
  - **Native Wayland has no clipboard in this build**, because the
    workspace builds arboard with `default-features = false` and no
    `wayland-data-control` (`lumen-static-winit/Cargo.toml:147`). It
    works under XWayland. This is recorded in the `clipboard`
    feature's manifest comment. Reopen when a Wayland-only session is a
    target, with that feature's crate count measured first.
  - *Crate:* `lumen-static-winit` (`clipboard.rs`, `Cargo.toml`).

**Tasks.**

- **T88.0 — pin the gaps with gates that compile today.** No new API.
  Each test asserts today's measured behaviour, so `just test` stays
  green. The task that closes each gap flips the assertion, and the
  flip is named there.
  - `crates/lumen-static-shell/tests/sipper.rs`: one job under
    `Immediate` sends 10,000 `u64`s through `Sender::send` into a
    one-node host, which is then pumped to empty.
    - *Printed:* refused sends, the largest number applied in one pump,
      and whether the value 9,999 was applied.
    - *Asserted today:* refused ≥ 9,744 (10,000 − `Inbox::CAPACITY`),
      the per-pump maximum = `Inbox::PER_FRAME` (64), and the final
      value **lost**. That is the gap: a streaming job loses 97% of
      its values and its last one.
    - *Setup landed:* `Host::in_flight` = 1 before the pump.
  - `scripts/resume_drive.py`'s GPU leg asserts that `ui.screenshot`
    answers the error text "this host does not render". T88.2 flips
    it.
  - *Accept direction:* the same test at 200 sends has 0 refused and
    delivers the final value, which proves the harness can see a
    delivery.
  - *Injection:* raise `Inbox::CAPACITY` to 16,384 and the "final value
    lost" assertion fires. The gate reads the mechanism, not a
    constant.
  - *Docs (S4):* `docs/results-tasks.md` gains an "M88 baseline" row
    with the three printed numbers.
  - **Prerequisite: `xclip` on the reference machine (owner decision
    D7, 2026-10-02; `docs/plan-iced-roadmap.md` §3.4).** T88.4's
    independent primary-selection reader is `xclip`, and the reference
    machine has none today (`which xclip xsel xdotool` finds nothing;
    only `/usr/bin/xwd` and `/usr/bin/gdbus` exist; second review R11).
    The owner runs `sudo apt install xclip` there once, before T88.4.
    It is a machine step, not a commit; T88.4 adds the probe case that
    checks it and quotes its `present` line.
- **T88.1 — `Sender::progress`: the sipper.** This adds `JobTrack<M>`'s
  slot, `Ledger<M>`, the slot drain in `pump_with`, `settled`'s
  `!fresh` term and `Inbox::coalesced`. `tasks-demo` gains a `#sip`
  button: one job, 10,000 `progress` calls, each value's text shown as
  `sip N`. `fixtures/chain-app` gains a sipper kind (v1 yields `p`,
  v2 yields `p + 100`; v1 blocks after its fifth value until
  `chain_app_release`).
  - *Docs (S4):*
    - `docs/plan-tasks.md` §4's "Streaming results" is marked DECIDED
      with a pointer here.
    - The events section of `docs/00-handoff-static-tree.md` (the M28
      "MADE CONCRETE" paragraph) gains the slot, which is a third
      route back into the frame.
    - `docs/results-tasks.md` gets the T88.1 rows: `JobTrack` size,
      the spawn arm, and the per-pump maximum.
    - The rustdoc of `inbox.rs` changes in the module header and on
      `Sender::send` (which must keep the "refuses rather than blocks"
      decision), `Sender::progress` and `JobTrack`.
    - `book/src/events.md` gains a sipper paragraph and a
      `{{#include ../../examples/tasks-demo/src/main.rs:sip}}`.
  - *Gates:*
    - `tests/sipper.rs`, flipped. Under `Immediate`, `Queued` and
      `Threads::with_cap(2)`, 10,000 `progress` calls give at most one
      applied value per pump (maximum printed, bound 1), the value
      9,999 applied exactly once (count printed, bound 1), and 0
      refused (bound 0). `Inbox::coalesced` is printed beside the
      count applied.
      - *Setup landed:* `in_flight` = 1 before the first pump.
      - *Accept:* a job that `send`s 3 values still gets all 3.
    - **An unsplit application sees its progress**, in the same file
      (second review A2). Every other M88 gate runs on `tasks-demo`,
      which sets `STATE_IS_SPLIT = true` (`main.rs:128`), so its
      bindings mark and would hide the gap. A one-label host with
      `STATE_IS_SPLIT = false` under `Boundaries<256>`, whose `apply`
      writes the label directly, runs one sipper of 3 values. After
      the pumps the frame's text reads `3` (printed beside the
      expected value).
      - *Setup landed:* the label read `0` before the job, and
        `Inbox::coalesced` plus the count applied equals 3 (printed).
    - **No block while nothing drains**, in the same file. Under
      `Threads::with_cap(2)`, two sippers of 10,000 values each and a
      120 ms timer run for 3 s with **no pump for the first 1 s**. That
      is the panic-screen and swap case, P15(4). Each body returns
      within 50 ms of starting (elapsed printed). The timer's tick
      count equals the count from a control run without sippers ± 1
      (both printed).
      - *Setup landed:* the timer ticks at least 20 times in the
        control run.
    - `just window-reload-demo` gains `sipper_across_a_swap`. v1 is
      clicked and shows `sip 5` in flight. After the swap and the
      release, the label must read v2's final value, `sip 10099`, and
      must not read v1's `sip 9999`.
      - *Setup landed:* the ledger lists the sipper kind before the
        swap, as `chain_across_a_swap` asserts (`reload-host`
        `main.rs:344`).
      - *Accept:* v2 with no swap reaches `sip 10099`.
    - **Progress after stop**, in `tests/sipper.rs`, only if M84/T84.7
      has landed first (otherwise T84.7 carries it; M11). A stopped
      job's final `progress` value is not applied (count printed,
      bound 0), and `Host::stopped_dropped` counts it.
      - *Injection:* the slot drain ignores `stop`. The final value is
        applied, and the gate names it.
    - `just tasks-drive`, both strategies. A click on `#sip` is
      `handled`. The label reaches `sip 9999`. `app.presents` counts
      the frames during the run, and the presents are printed beside
      the number of distinct labels seen, which must not exceed them.
  - *Injections:*
    - Make `progress` a `send`, so there is no slot. The per-pump
      maximum becomes 64, refused becomes ≥ 9,744 and the final value
      is lost, and the gate names all three.
    - Make `progress` block on a bounded channel of 8. The no-drain
      gate's 50 ms watchdog fires under `Threads`, and `Immediate`
      hangs; the test runs it on a watchdog thread, so it fails
      rather than hangs.
    - Drop `!fresh` from `settled`. Under `Immediate`, the ledger
      prunes the job before its final value is applied, and "9,999
      applied exactly once" reads 0.
    - Skip `Host::reissue` in `hot::boot`. The swap gate reads `sip 5`
      and never completes, and names v1.
    - The slot drain does not OR into `applied_changed`. The unsplit
      gate's label reads `0` (stale, the idle skip), and the gate names
      it; the split `tests/sipper.rs` arms stay green, which is why the
      unsplit arm exists.
- **T88.2 — `frame_rgba` on the GPU, and `&mut self`.** This changes
  the receiver on `Backend`, `Screen`, `AgentHost::frame` and their
  implementations: `Software`, `AsScreen`, `Offscreen`, `Live`, and
  the test `Screen`s in seven files in `crates/lumen-static-shell/
  tests/` (`agent_paste`, `agent_settled`, `agent_motion`,
  `process_info`, `agent_compose`, `agent_panic`, `agent_frames`).
  It adds `GpuBackend::frame_rgba` backed by `shot`. `fetch_rgba` stays
  `None`, and the comment at `gpu.rs:132` keeps its reason and gains a
  pointer to `frame_rgba`.
  - `examples/tasks-demo` gains a 16×16 swatch at the origin, a
    `#swatch` button that sets it to a colour derived from its click
    count, and a `gpu` feature that passes `lumen-static-winit/gpu`
    through. `just tasks-drive` gains a GPU leg that exits 77 with no
    adapter, as `gpu-parity` does. These land here, not in T88.3,
    because the per-call gate below needs a control whose click
    changes pixels, and `resume-drive`'s `examples/glasswall`
    (`scripts/resume_drive.sh:24-37`) has none: `grep -ci swatch` finds
    0, and its four tab buttons share one `tab` id (second review
    G20).
  - *Docs (S4):*
    - The correction at `docs/plan-timers-wake.md`:641 gets a pointer
      here.
    - `docs/results-gpu.md` gains a row for the readback's cost at
      1920×1080 (median of 8, measured by a new `readback_cost` test
      in `crates/lumen-static-wgpu/tests/parity.rs`).
    - The rustdoc on the `frame_rgba` of `backend.rs` and the `Screen`
      trait says a call may read the GPU back.
    - `scripts/resume_drive.sh`'s header drops "because the GPU backend
      cannot answer `ui.screenshot`".
  - *Gates:*
    - `just resume-drive`, GPU leg, flipped. `ui.screenshot` (canvas)
      is compared with the `xwd` readback of the same still frame.
      The worst channel delta is printed against a bound of 1, the
      bound `just gpu-parity` holds for the same comparison
      (`docs/results-gpu.md`).
      - *Setup landed:* the launch log's startup line names `gpu (`.
      - *Accept:* the software leg gives the same comparison against
        `xwd`, with delta 0.
    - `just tasks-drive`, GPU leg: `ui.screenshot`, then an
      `input.click` on `#swatch` (`handled`), then a second
      `ui.screenshot`. The second must differ from the first in the
      swatch's `ui.regionHash` rect. Both hashes are printed. This
      proves the readback is per-call.
      - *Setup landed:* the click is `handled`, and the software leg's
        two hashes differ too.
  - *Injections:*
    - `GpuBackend::frame_rgba` returns the cached `shot` without
      re-reading. The `tasks-drive` GPU leg's second swatch hash equals
      the first, and the gate names it (both printed).
    - Implement it as `fetch_rgba` instead. `source: "surface"` then
      answers from the renderer, and the existing surface leg's
      "not checkable" assertion fires. This is the rule
      `gpu.rs:132-140` records, kept.
- **T88.3 — the request seam (S1) and screenshot requests.** This adds
  the core types, the three `App` hooks, `Outgoing`/`Payload`,
  `Host::take_requests`/`answer`/`unmodelled_answers`/
  `answer_changed`, the two `Session` methods in both implementations
  (`Local`, `Hot`), and `Win::serve_requests` with `Screenshot`
  answered, drained on every redraw after the present-or-skip decision
  and after every agent call.
  - `ClipboardWrite`/`ClipboardRead` are answered
    `Refused(Unsupported)` until T88.4, and a test pins that.
  - `examples/tasks-demo` (whose swatch and `gpu` feature T88.2
    added) gains:
    - a `#shot` button, which sets the swatch to a colour derived from
      its click count and pushes `Screenshot { tag }`. Its request is
      served by the **post-agent** drain, because the click arrives
      through the agent: `input.click` runs `Live::pump_app`, which
      pumps and `present_counted()`s inside the request (shell
      `session.rs:902-931`), and `Win` drains after the call;
    - a `#shot-timer` button, which arms `timers.every(KIND_SHOT, 1,
      500.0)`. Its first tick's `apply` sets the swatch to a fresh
      colour and pushes `Screenshot`; its second tick pushes a
      `Screenshot` and changes nothing, then cancels the timer. A
      tick is applied in a pump that `redraw` runs on a timer wake,
      with no agent call in flight, so these requests are served by
      the **redraw** drain only (second review G19). The second
      tick's frame has no damage, so it is the skipped-present case
      (A18, P8, R21);
    - `answered`, which reads the pixel at physical (4, 4) and
      `apply`s it to a `shot N WxH #rrggbb` label, `N` counting
      answers.
  - `fixtures/chain-app` gains a `#ask` button that pushes
    `ClipboardRead { tag: 7, .. }`. `answered` labels the text
    `v1:` or `v2:`, depending on the build. The answer's text lives on
    a field the swap keeps (`#[reload(keep)]`), so an answer applied
    before a swap is visible after it.
  - An unsplit one-label test app (`STATE_IS_SPLIT = false`) in
    `tests/requests.rs`, whose `apply` writes its label directly.
  - `fixtures/panic-app` gains a site, a panic in `answered`, at the
    next free site number at landing (printed; seven today, and M80
    and M84 add sites before it may land).
  - *Docs (S4):*
    - The events section of `docs/00-handoff-static-tree.md` gains
      the third queue, out to the window.
    - `docs/events-decision.md` gains the request case under rule 3.
    - `book/src/events.md`'s direction table gains a `request::Requests`
      row, with `{{#include ../../examples/tasks-demo/src/main.rs:shot}}`.
    - `docs/step4-hot-reload.md` gains the two new crossing types.
    - The rustdoc on the `Session` trait and on `request.rs` changes.
    - `docs/competitive-feature-gaps.md`'s screenshot row is closed.
  - *Gates:*
    - `crates/lumen-static-shell/tests/requests.rs` (new).
      - A click on a button pushes `Screenshot { tag: 7 }`.
        `take_requests` returns exactly one `Outgoing` (length
        printed, bound 1), and a second `take` returns 0.
      - `answer(7, Image { 2×2 })` applies inside the call (`shots` =
        1 before any pump, printed), and the next pump shows it.
      - **The unsplit app under `Boundaries<256>`** (second review A2):
        `answer(7, Text("x"))` and one pump, and the frame's label
        reads `x` (printed beside the expected value). Every
        `tasks-demo` gate is on a split app (`main.rs:128`), whose
        bindings would mark and hide the gap.
      - An answer to an unmodelled tag increments
        `unmodelled_answers` to 1.
      - A ninth push is refused and counted.
      - *Setup landed:* the click is `handled`.
      - *Accept:* an application with no `requests()` returns an empty
        `Vec` and counts nothing.
    - `crates/lumen-static-shell/tests/alloc_gate.rs` gains an arm: a
      steady frame plus `take_requests` on an application whose
      `Requests` is empty, 0 allocations per frame (printed, bound 0).
      - *Setup landed:* the arm's `requests()` is `Some`.
    - `just window-reload-demo` gains `request_across_a_swap`.
      `reload-host` stands in for the window. It clicks `#ask` on v1
      and takes one `ClipboardRead { tag: 7 }` (setup landed, count
      printed). It then swaps to v2 and calls `answer(7, Text("x"))`
      on the current session, and the label must read `v2:x`.
      - This is the property M89's pending requests rely on: an
        answer is read by whatever build is loaded, and the window
        holds only the tag.
      - *Accept:* with no swap, v1 reads `v1:x`.
    - `answer_then_swap`, in the same recipe (second review A1).
      `reload-host` clicks `#ask` on v1, takes the request, calls
      `answer(7, Text("x"))` on v1's session, and swaps to v2 **before
      any pump**. After v2's first frame the label reads `v1:x`: v1
      was loaded at answer time, and its applied answer is kept state.
      - *Setup landed:* v1's kept answer field read `x` right after
        `answer`, before the swap (printed).
      - *Accept:* with no swap, the label reads `v1:x` after one pump.
    - `just tasks-drive`, both strategies, software and GPU legs (the
      GPU leg from T88.2).
      - **Agent path.** The `#shot` click is `handled`, and after an
        **unobserved** 1 s wait the label names this click's colour,
        not the previous click's. Both colours are printed. The wait
        is unobserved because every agent reply requests a redraw (the
        `tasks_drive.sh` header, M47/T47.1).
      - **Redraw path.** The `#shot-timer` click is `handled`. The
        drive calls `ui.waitSettled` and reads `app.presents`
        (presented and skipped) as P0, before the first tick at
        500 ms. After an **unobserved** 1.5 s wait it reads P1 and the
        label.
        - The label reads `shot 2` with the first tick's colour, not
          the colour before it (both printed). `shot 2` proves the
          skipped frame's request was answered.
        - Under `Boundaries<256>`, P1 − P0 is exactly 3 presented
          (tick 1, answer 1, answer 2) and at least 1 skipped (tick
          2), both printed beside their bounds. Under `Everything`
          both are printed and presented + skipped is 4.
        - **Robust to M86** (second review X6): P0 is read after
          `ui.waitSettled`, so a hover or press transition on the
          `#shot-timer` `Button` (M86/T86.4, 120 ms) has finished
          before the first tick, and the frames counted are the
          ticks' and the answers' only. Neither milestone has to land
          first. The first draft counted "exactly 2 for the click",
          which a 120 ms transition breaks on correct code.
    - `just panic-drive`: the `answered` site reports
      `fixtures/panic-app/src/lib.rs` at its marked line, the screen
      is drawn and a retry recovers. `--expect-panics` goes up by one
      from its value at landing, printed.
    - Firmware: `nm` on each `scripts/lib/firmware.sh` binary finds 0
      symbols matching `request::|Requests|answered` (printed, bound
      0). The positive control is the same count on the `requests.rs`
      test binary, which must be > 0.
  - *Injections:*
    - The redraw-site drain runs before `backend.present`. The
      redraw-path leg's answer is the previous frame's pixels, and the
      drive prints the colour before tick 1 and names it (G17's
      injection, aimed at the path it can reach; second review G19).
      The agent-path leg stays green, which shows the two drain sites
      are separate.
    - The redraw-site drain runs only in the present branch, not after
      a skipped frame. The redraw-path label stays `shot 1` after the
      unobserved wait, and the gate names "skipped frame not drained"
      (A18, P8, R21).
    - `Host::more_deferred` ignores `answer_changed`. On an idle window
      the label stays `shot pending` after the unobserved wait, and
      the gate names it.
    - `Host::answer` parks the `Msg` on a host-side queue for the next
      pump (the first draft). `answer_then_swap`'s label reads `v2:`
      with no text, and the gate names "answer lost at the swap"
      (second review A1).
    - `answer` does not set `answer_changed`. The unsplit
      `Boundaries<256>` test's label stays stale (the idle skip), and
      the test names it (second review A2).
    - `take_requests` copies without clearing. The second `take`
      returns 1 (bound 0).
    - `take_requests` always calls `Vec::with_capacity(Requests::
      CAPACITY)`. The alloc arm reads 1 per frame.
    - `reload-host` answers the session that issued the request (the
      box kept from before the swap). The label stays `v2:` with no
      text, and the gate names "answered to v1".
    - `Local::answer` calls `A::answered` outside the guard. Observed
      on the **recovering** legs (`agent,reload`, both strategies,
      `--expect-panics` one above its value at landing), where the
      guard exists: the process dies at the `answered` site and the
      launcher reports the lost process. Committed, the screen shows,
      `app.retry` recovers, and the expected panic count rises by one,
      printed. The norecover leg (`panic_drive.sh:34-53`,
      `--release` without `reload`, `Crashing` with `ON = false`)
      dies with or without the guard and never reaches that site, so it
      stays as it is and observes nothing here (second review A4).
    - Remove `cfg(feature = "std")` from `Requests` and push one from
      the embedded fixture's handler. The firmware `nm` count becomes
      > 0.
- **T88.4 — clipboard reach: text, HTML, primary.** `Clipboard` gains
  `set_html`, `get_html`, `set_primary` and `get_primary` (Linux, via
  arboard's ext traits; `false`/`None` elsewhere). `serve_requests`
  answers `ClipboardWrite` with `Written` or `Refused(Failed)`, and
  `ClipboardRead` for `Text`/`Html`. `tasks-demo` gains `#copy-html`,
  `#copy-primary` and `#read-*` buttons.
  - *Docs (S4):*
    - `scripts/tasks_drive.sh`'s header names `xclip` as a required
      tool and the install line (D7).
    - `book/src/platform.md` §Clipboard: an application can now reach
      the clipboard, and native Wayland cannot.
    - The rustdoc on `clipboard.rs` changes.
    - The `clipboard` feature's manifest comment gains the Wayland
      record.
    - `docs/competitive-feature-gaps.md`'s clipboard row is updated.
  - *Gates:*
    - `just tasks-drive`, both strategies. The drive first puts `A` on
      the clipboard and `B` on primary (setup landed: each write
      answered `Written`, printed).
      - `#read-text` shows `A` and `#read-primary` shows `B`.
      - `#copy-html` followed by `#read-html` round-trips the HTML byte
        for byte, with its length printed.
      - An independent `xclip -o -selection primary` must read `B`
        (printed). This leg **always runs on the reference machine and
        never reports skipped** (owner decision D7, §3.4 of
        `docs/plan-iced-roadmap.md`):
        - `scripts/probe.sh` gains an `xclip` case, its first
          **required** one: `probe: xclip present (<path>, <version>)`,
          or `probe: xclip absent -- required (D7): sudo apt install
          xclip` and exit **1, not 77**. Its usage line and header list
          it beside `thumb`, `qemu` and `miri`.
        - `scripts/ci.sh`'s probe step (`"$probe" thumb qemu miri`,
          `:148`) gains `xclip`, so every `target/ci/<sha>.json` records
          it.
        - The primary leg calls `scripts/probe.sh xclip` first and
          propagates its status, so a missing `xclip` fails the drive
          with the install line. The drive's own 77 stays for a missing
          display only (M43 §2).
        - *Setup landed:* the commit body quotes the probe's `present`
          line from the reference machine.
        - The first draft skipped this leg without `xclip`, which under
          `docs/plan-iced-roadmap.md` §3.1 left M88 unable to close on
          a machine that had none (second review R11).
      - *Refuse direction:* with `xclip` removed from `PATH`, the leg
        exits non-zero and non-77 and prints the install line; with it
        present, the leg reads `B`. Both runs are quoted in the commit
        body.
      - *Accept:* after the app writes `C` to the clipboard, primary
        still reads `B`.
    - Crate count: `cargo tree -p lumen-static-winit -e normal --prefix
      none | sed 's/ (\*)$//' | sort -u | wc -l`, before and after,
      printed. Bound: 0 new crates. The `sed` strips cargo tree's
      `(*)` repeat markers, so the count is K2's: 96 for the default
      build at 553fa5e, against 117 without it.
    - A platform clipboard cannot be exercised headless (the reason
      `clipboard.rs` lives in winit). The shell tests pin only
      `Payload` construction from `clip_for`.
  - *Injections:*
    - `serve_requests` ignores `kind`. `#read-primary` shows `A`, and
      the drive prints both.
    - `clip_for` is not consulted, so the write answers `Refused
      (Empty)`. The setup assertion fires by name.
    - The probe's `xclip` case exits 77 (a skip, the first draft's
      shape). With `xclip` removed from `PATH`, the drive exits 77, and
      the refuse direction's "non-77" assertion names it.
- **T88.5 — `clipboard-image`.** This adds the feature,
  `ClipFormat::Image`, `Payload::Image`, `Clip::Image` in writes, and
  `Answer::Image` for reads. Without the feature, an image request is
  answered `Refused(Unsupported)`. `tasks-demo` gains a
  `clipboard-image` pass-through and `#copy-image`/`#read-image`
  buttons, using an 8×4 image, deliberately non-square.
  - *Docs (S4):*
    - The feature's manifest comment gets its crate delta and stripped
      size, citing `docs/results-tasks.md` T88.5, where the numbers
      live.
    - `book/src/platform.md` changes.
    - `docs/competitive-feature-gaps.md` rich-clipboard row is closed.
  - *Gates:*
    - K2's crate count with the feature off is unchanged (bound 0) and
      with it on is printed.
    - Lint: `cargo clippy -p lumen-static-winit --features
      clipboard-image --all-targets -- -D warnings` joins `just gpu`'s
      winit clippy line (`justfile:372`), so `check-extra` runs it.
    - The stripped release `showcase` size delta with the feature is
      measured as `docs/results-a11y-platform.md` measured `a11y`, and
      printed beside the 3 MB bound (B9; opt-in either way, recorded).
    - Symbols, modelled on `scripts/check_agent_symbols.sh`: the
      default `showcase` build has 0 `image::` symbols (bound 0), and
      the build with `lumen-static-winit/clipboard-image` has more
      than 0 (the positive control).
    - `just tasks-drive` with the feature: `#copy-image` then
      `#read-image` shows `8x4`, and the pixel hash equals the one
      written (both printed).
      - *Setup landed:* the write answered `Written`.
      - *Accept:* the build without the feature answers `Unsupported`,
        not `Empty`.
  - *Injections:*
    - Make `clipboard-image` a default feature. The symbol gate reads
      more than 0 in the default build.
    - Swap `width`/`height` in the conversion to arboard's `ImageData`.
      The drive reads `4x8` and names it.

**Kill rules.**

- **K88a — the spawn arm.** `just bench-lock cargo run --release -p
  benches --bin spawn -- pool`, eight runs a side, before and after
  T88.1. If the median lands outside the before arm's spread, the slot
  leaves `JobTrack` for an `Arc` allocated on the first `progress`
  only, and the number is recorded in `docs/results-tasks.md` T88.1.
- **K88b — the idle floor.** The `idle (floor)` row of
  `benches-competitive/arms/frame`, eight runs a side under `just
  bench-lock`, before and after T88.1 and T88.3. If it lands outside
  its spread, the slot check moves behind `ledger.is_empty()`, which
  the design already intends, and is re-measured. If it is still
  outside, T88.1 stops and the finding is recorded.
- **K88c — the GPU readback.** `readback_cost` at 1920×1080 is judged
  against 100 ms. Above that, `Win` answers GPU `Screenshot` requests
  `Refused(Unsupported)`, `ui.screenshot` keeps its "does not render"
  answer on the GPU, and the figure is recorded in
  `docs/results-gpu.md`.
- **K88d — `just deny`.** If `arboard/image-data`'s dependency set
  fails `just deny`, `clipboard-image` does not ship.
  `ClipFormat::Image` stays in the enum, answered `Unsupported`, and
  image clipboard is recorded as deferred, with the failing crate
  named.

**What the review found (M88).**

- C14 → adopted: requests are data (`Request`/`Requests`), not `Job`s.
  Handlers, `apply` and boot push. `Win` drains through
  `Session::take_requests`. Nothing crosses a swap, because M88
  answers synchronously, and `request_across_a_swap` proves the
  seam. M89 depends on T88.3 by name. *Second review A1:* the
  converted answer had been parked for the next pump and could be lost
  at a swap; `Host::answer` now applies it inside the call, and
  `answer_then_swap` gates it.
- C15 → adopted: there is one semantics, a coalescing slot outside the
  channel. Blocking and the quota of 8 are dropped.
- C30 → adopted: GPU `frame_rgba` is backed by an owned readback, and
  the receiver becomes `&mut self`. T88.2's comparison is canvas
  against `xwd`.
- G17 → adopted: `fetch_rgba` stays `None` (citing `gpu.rs:132-140`).
  The comparison is against an independent `xwd` readback. The
  injection answers before present. *Second review G19:* that
  injection is now aimed at the redraw drain through `#shot-timer`,
  because the `#shot` click is served by the post-agent drain.
- G18 → adopted: the gate is ≤ 1 per pump, and the final value is
  delivered. The injections are the ones named in T88.1, and none of
  them reuses T28.7's demo.
- G32 → changed: M88 adds no code the firmware can link (everything is
  behind `std`, and the host that calls it is in the shell). An exact
  `.text` diff would be B7's wrong quantity. T88.3 uses `nm` at
  bound 0 with a positive control and a reachability injection,
  following the T28.7 precedent.
- B22 → adopted, the same resolution as C15/G18.
- A1 → adopted: an answer is converted by `App::answered` in the build
  loaded at answer time. `Win` holds a tag only. *Second review A1:*
  it is also applied at answer time, so nothing converted waits for a
  pump that a swap could pre-empt.
- A13 → adopted: blocking is dropped and `inbox.rs`'s decision is
  kept. The no-drain gate is the one A13 proposes: `Threads` cap 2
  plus a 120 ms timer, with ticks printed against a control.
- A14 → adopted: `take_requests` and `answer` are named, with their
  two implementations (`Local`, `Hot`; second review P18 removed
  `fixtures/miri-reload`, whose `Session` is a local stand-in). The forward is gated by the compiler (S1),
  not by a stamp test.
- A19 → adopted, the same resolution as C30/G17.
- P15 → adopted: there is one slot per job and no marker in the
  channel, so nothing can be refused. `settled` requires the slot to
  be empty, so the final value cannot be lost. There are gates under
  `Immediate`, `Queued` and `Threads`, including the case where
  nothing drains.
- P16 → changed: there are no `on_progress`/`on_done` pointers. The
  body converts on the worker, which is the accepted `JobFn` form.
  The gate is a sipper across a real swap, and its injection disables
  reissue.
- P17 → adopted: see C14. The stamp-test half is rejected, because
  `Session` methods are required and a missing forward does not
  compile (C4/A15/M8).
- P18 → adopted: GPU `frame_rgba` is added and `fetch_rgba` stays
  `None`. Its sub-claim that the stamp does not cover `Screen` was
  refuted by its own verifier (`session.rs:1117-1121`: the
  whole-crate source hash covers vtables), so nothing is added for it.
- P19 → adopted: primary and HTML are available now and need no
  feature. `clipboard-image` = `arboard/image-data`. Wayland is
  recorded as having no native clipboard without
  `wayland-data-control`, which is not added.
- M3 → adopted: there is no trailing docs task, and each task carries
  its own documentation.
- M5 → adopted: see A1. Program and window naming by data is S1 and
  D1(d), used by M89.
- M12 → adopted: a session's requests are answered to that session,
  and M89's per-window sessions route by construction. The two-window
  injection is left to M89.
- M16 → no change needed in M88: it adds no agent method, so
  `check_agent_methods.sh` is unaffected. `app.windows` is M89's.
- M18 → adopted: M88 names `book/src/events.md` and
  `book/src/platform.md`, which both exist.
- M20 → adopted: every task has a mechanism injection.

**What the second review found (M88).**

- G19 → changed: the before-present injection could not reach the
  `#shot` click, which the post-agent drain serves after
  `Live::pump_app` presents inside the request (`session.rs:914-931`).
  `tasks-demo` gains `#shot-timer`, whose ticks are applied on a
  timer-woken `redraw` with no agent call, and the injection is
  observed there; the agent-path leg stays green under it.
- G20 → changed: the per-call screenshot gate moved to a `tasks-drive`
  GPU leg, and the swatch, `#swatch` and the `gpu` feature moved from
  T88.3 into T88.2, because `examples/glasswall` has no control whose
  click changes pixels (`grep -ci swatch` = 0; its tabs share one id).
- A1 → changed: adopted the finding's alternative. `Host::answer`
  applies the `Msg` inside the call instead of queueing it, so nothing
  converted is pending at a swap and `save` needs no new field. Gate
  `answer_then_swap` (label `v1:x` after a swap before any pump); its
  injection is the first draft's queue, which reads `v2:` empty.
- A2 → adopted: the slot drain and `answer_changed` both OR into
  `applied_changed` before the idle-skip check. Unsplit
  `Boundaries<256>` arms in `tests/sipper.rs` and `tests/requests.rs`,
  each with an injection that leaves the label stale.
- A4 → adopted: the guard injection is observed on the recovering
  `agent,reload` legs (dies at site 8 injected; screen, `app.retry`
  and `--expect-panics 9` committed; numbers superseded by
  consistency-3 item 2). The norecover leg is unchanged.
- A18 → adopted: the redraw drain runs after the present-or-skip
  decision, after the `failed()` return (`lib.rs:917`), so it is
  skipped while the panic screen is up.
- P8 → adopted, with A18: the drain runs on skipped frames, and
  `#shot-timer`'s second tick is the no-damage request; its injection
  drains only in the present branch and the label stays `shot 1`.
  Windowless frames are M89's third drain site.
- P10 → changed: the claim is removed rather than gated. Whether
  M89's `OpenWindow` can be pending is M89's to state (a T89.7 gate,
  or creation and answer inside the drain);
  `docs/plan-iced-roadmap.md` §2.8 and `docs/plan-windows.md` are
  asked to match.
- P18 → adopted: `fixtures/miri-reload` is dropped from the
  implementor list (`main.rs:22-35` is a two-method stand-in).
- R11 → changed: adopted the finding's second option, made binding by
  owner decision D7. T88.0 names installing `xclip` as an explicit
  prerequisite (`docs/plan-iced-roadmap.md` §3.4); T88.4 adds it to
  `scripts/probe.sh` as a required case (exit 1) and to `ci.sh`'s
  probe step; the primary check always runs, and a missing `xclip`
  fails with `sudo apt install xclip`, never 77.
- R21 → adopted, the same resolution as A18/P8.
- X6 → changed: no order is imposed. T88.3's presents bound is taken
  on the redraw path between P0 (after `ui.waitSettled`, before the
  500 ms first tick) and P1, so an M86 `Button` transition is settled
  before anything is counted. `docs/plan-transitions.md` T86.4 is
  asked to list `scripts/tasks_drive.*` among the scripts it re-runs.
- D6 → no M88 text attributed `text_spaced` or `fills` to D3. The one
  misattribution was the sipper cost's "(D3 is not engaged)" for a
  firmware fact; it now cites D5.
- D7 → adopted: see R11.
- X5 → not affected: M88 adds no shaping crate and does not touch
  `deny.toml`'s `rustybuzz` entry.
- A9, A19, D7, X7 (from another plan's editor) → adopted in part. "Who
  answers" names M89's third site, and the `OpenWindow` sentence now
  leaves pending-ness to M89 (T89.7 gate, or answer inside the
  drain). The `xclip` half was already in place (T88.0
  prerequisite; T88.4's required probe case, exit 1 with the install
  line, `ci.sh`'s probe line, both directions). New: T88.5's clippy
  line for `--features clipboard-image`.
- M11 (from `plan-images-in-motion.md`) → adopted: T88.1's mechanism
  names `JobTrack::stop` and whichever-lands-second; T88.1 carries
  "progress after stop" only if T84.7 landed first (T84.7 states the
  mirror rule, `plan-images-in-motion.md:608`).
- P9 (from `plan-windows.md`) → adopted: the third drain site is a
  sub-bullet under "Who answers": after every windowless
  `frame(dt, 1, 1)` and after any answer routed into the windowless
  session.
- B8 follow-up (from another plan's editor) → adopted: T88.4's
  crate count strips `(*)` with `sed` before `sort -u`; checked at
  553fa5e with `cargo tree --offline`: 96 with it, 117 without.
- consistency-3 item 2 → T88.3's panic site is the next free number at
  landing (printed) and `--expect-panics` rises by one from its value
  at landing, not "site 8" / "8 to 9" (M80 and M84 add sites first).
- consistency-3 item 9 → the swap paragraph states M89's decision (no
  `OpenWindow` pending; `Win` answers inside the drain, T89.3 gate 2)
  instead of leaving it open.

**What building found (M88).**

- **T88.0.** The pin's bound is `10_000usize.saturating_sub(CAPACITY)`:
  the plain subtraction overflowed at compile time under the plan's own
  injection (`CAPACITY` = 16,384), so the first red attempt did not
  build, which is not a red run. The GPU screenshot pin's red run
  perturbed `GpuBackend::frame_rgba` to answer a 1×1 image.
- **T88.1, `settled` and `stop`.** M84/T84.7 landed first, so T88.1
  extends it as the mechanism says: the slot drain drops a stopped job's
  value into `Host::stopped_dropped`, and `settled` treats a stopped job
  as finished — **once its slot and its channel messages are drained**,
  not at the stop. Settling at the stop would let a prune between the
  stop and the drain drop the entry with its slot full, and the dropped
  value would go uncounted. The pump that stops a job therefore also
  takes it off the ledger (its drain follows the diff), which inverted
  the setup of `tests/reissue.rs`'s T84.7 test (a stopped job still
  listed while its body waited): that test now asserts the job leaves,
  and `Ledger::unsettled`'s stop filter — which guards only a result
  sent between a drain and the prune after it — is gated on a ledger
  directly, `crates/lumen-static/tests/ledger_stop.rs`.
- **T88.1, the sipper gate's setup.** "In flight = 1 before the first
  pump" is asserted *after* the claims, not before: under the plan's
  injection (drop `!fresh` from `settled`) the `Immediate` job is pruned
  before the first pump, so a setup assertion first would have fired in
  place of the claim the plan names ("9,999 applied exactly once"
  reads 0), which it now does. Each loop pumps at least once.
- **T88.1, the blocking injection** is "`progress` waits while its slot
  is full", a bounded queue of one rather than a channel of eight. The
  mechanism is the same — any `progress` that blocks blocks when nothing
  drains — and it fires as the plan says (`Immediate` and `Queued` hang
  into the watchdog; the no-drain arm's bodies never return).
- **T88.1, `chain-app`.** The two buttons share the top 60 px (`#start`
  left, `#sip` right), so `reload-host` clicks the chain at (60, 30).
  v1's sipper blocks **after reporting `p = 5`**, which is what shows the
  plan's `sip 5`. It waits on its **own** release,
  `chain_app_release_sip`, not `chain_app_release`: the chain gate
  releases v1's step 0 first and the library stays loaded, so a shared
  flag would already be set and v1 would never be in flight.
- **T88.1, a dropped re-issue reads `sip -`, not `sip 5`.** `Work` is
  `#[reload(fresh)]`, so v2 starts with no value; v1's worker streams on
  into a slot nobody reads. The gate names that.
- **T88.1, a loader defect the second gate found, fixed in
  `lumen-static-reload`.** `Reloader` numbered its copies per instance,
  so the second `Reloader` in one process (the sipper gate's, after the
  chain gate's) wrote `chain-v2-hot-1.so` again — truncating the image
  the loader still had mapped — and the next `dlsym` died with `SIGSEGV`
  in `check_match` (gdb: the link map of `chain-v2-hot-1.so`, its string
  table unreadable). It reproduced at the parent commit with no sipper
  at all (chain gate, then any second swap of the same library: exit
  139). The copy number is process-wide now, and the copy is written
  beside its name and renamed over it, so no mapped image is ever
  rewritten. Red: the per-`Reloader` count, exit 139; green: the demo
  passes.
- **T88.1, `tasks-demo`** grows to 520×420 for its `#sip` button and
  label.
- **K88b at T88.1** is not engaged: the frame arm's binary is
  byte-identical before and after (`cmp`), because its `Msg` is
  uninhabited and the compiler removes the slot drain
  (`docs/results-tasks.md` T88.1).
- **T88.2, where the receiver changed.** M83 had moved `Backend` into
  `lumen_static_shell::backend`, so the edit is there. The test `Screen`s
  are in **eight** files, not seven: `contrast_scale.rs` has one too. Two
  more `Backend`s follow by compile error: winit's test `Fake` and
  `examples/glasswall`'s `Counting` (the book's `factory` anchor).
- **T88.2, the swatch is the demo's own leaf.** No widget in the set
  paints a colour its application picks at run time (`Fixed` paints its
  style's), so `tasks-demo` defines a 16×16 rigid `Swatch` that its
  `update` recolours; the window grows to 520×500.
- **T88.2, the "existing surface leg" did not exist.** No drive asked a
  GPU window for `source: "surface"`, so the second injection had nothing
  to fire. `just tasks-drive`'s GPU leg asks it first and pins "cannot
  read its window back"; the injection (`fetch_rgba` answering the
  readback) turns that red. The GPU leg runs only the phases that read
  pixels: the others are backend-independent and run on three legs.
- **T88.2, the resume gate's own injection.** The plan named none for
  the canvas-against-`xwd` comparison. A readback that copies rows at the
  unpadded stride (`Gpu::read_rgba`) reads delta 255 on 457,176 px.
- **T88.3, Everything's presents.** The redraw-path count is 3 presented
  and 3 skipped under both strategies, not "presented + skipped is 4"
  under `Everything`: a tick is a job, so each tick's deadline frame
  spawns it and presents nothing before its result lands. `Everything`'s
  numbers are printed, as the plan says; `Boundaries`' bound holds.
- **T88.3, the redraw path's log.** The final label alone cannot see the
  before-present injection: tick 2's answer reads tick 1's frame either
  way, so `shot 2` with tick 1's colour passes. `tasks-demo` keeps every
  answer's colour in `#shot-log`, and the drive requires both of the
  timer's answers to be tick 1's; the injection reads the colour before
  tick 1 on the first and is named. A third tick already in flight when
  the second cancelled the timer asks nothing.
- **T88.3, the GPU leg on a sleeping display.** With the reference
  machine's monitor off (DPMS), a FIFO present blocks about a second, so
  no read can land before tick 1. The GPU leg checks the answers after a
  6 s unobserved wait and prints the presents bound as not checkable when
  the click took over 250 ms; the software legs carry the bound.
- **T88.3, the firmware gate.** The `nm` count's injection cannot fire:
  `Requests::push` inlines into the embedded handler and leaves no
  symbol, while the same injection moves `.text` by +128 to +136 B. The
  strict `just firmware-delta`, 0 on all four, is the gate; the count is
  printed beside it.
- **T88.3, `chain-app`.** `Msg` becomes an enum (a step's value, or an
  answer's text), and `Work` is now nested with per-field modes —
  everything fresh but `asked`, which is kept — instead of
  `#[reload(fresh)]` whole. The three buttons take a third of the top
  60 px each, so `reload-host` clicks at x = 40, 120 and 200.
- **T88.3, `panic-app`.** Site 11 is `App::answered`; `JOB_PANIC` moves
  to 12. The window grows to 480 px wide, because `#arm11` was laid out
  past a 420 px window's edge and its click landed on nothing — which the
  drive did not check, so it reported "site 11 never failed". The arming
  click must now be handled or have raised the screen already.
- **T88.3, the screenshot row.** `docs/competitive-feature-gaps.md` had
  no screenshot row to close; one is added under iced, closed.
- **T88.3, `ClipFormat` and `Clip`** carry `Text` and `Html` only: T88.5
  appends `Image` with its feature. `Answer::Image` exists now, for
  screenshots.
- **T88.4, the primary selection carries text only.** `ClipboardWrite`
  with an HTML payload on `Primary`, and `ClipboardRead { Primary, Html }`,
  answer `Refused(Unsupported)`: the primary selection is the
  select-to-copy buffer, which a reader pastes as text, and the plan named
  only `set_primary`/`get_primary` for it. A clipboard the build has but
  the machine did not give it (`Clipboard::is_open` false: no display, a
  Wayland-only session) answers `Refused(Failed)`, and `Unsupported` is
  kept for what the build cannot do at all (`Clipboard::BUILT`,
  `Clipboard::HAS_PRIMARY`). The winit unit test hands `serve_requests` a
  `Clipboard::closed()` so it never touches the desktop's clipboard.
- **T88.4, `#copy-text`.** The plan's buttons were `#copy-html`,
  `#copy-primary` and `#read-*`, but the drive's setup puts `A` on the
  clipboard and its accept direction writes `C`, so `tasks-demo` also has
  `#copy-text`, writing `A` on its first click and two letters on per
  click after (`C`), never `#copy-primary`'s `B`. The window grows to
  520×760.
- **T88.4, the ignore-`kind` injection** reads `#read-text` as `B`, not
  the plan's "`#read-primary` shows `A`": in this drive's order the
  primary write lands on the clipboard after `A` and replaces it. The
  same assertion names it, with both reads printed.
- **T88.4, both directions in every run.** `scripts/tasks_drive.sh` runs
  the primary leg's preflight once with `xclip` off `PATH` (a `PATH`
  holding only `bash` and `dirname`, which `probe.sh` itself needs) and
  requires exit non-zero, non-77, with the install line; every software
  leg then runs it with `xclip` present. So the probe-exits-77 injection
  is observed by the drive itself, before any leg.
