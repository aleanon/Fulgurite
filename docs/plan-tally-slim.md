# M60 — the tally on the framework: two hooks, and no event loop of its own

> **T60.0–T60.5 DONE 2026-09-28** on branch `tally-slim`, **`just
> ci-verify 8d496c1` green**: 28 steps, 0 failed, 0 skipped, tree clean.
> - **T60.0** — the gates, red, over the API surface without its mechanism.
> - **T60.1** — `Program::on_key`: a key press dispatch did not take and
>   the focused node does not own reaches the hook, from `pump_with`.
> - **T60.2** — `Program::style_by`: the sheet is re-applied on the frame
>   its key changes; whole, not scoped (2% at the tally's size).
> - **T60.3** — the tally runs on `Program::run`; `Win` measures input
>   latency, and `Local`'s audit measures with its face (a defect every
>   `Program` window had).
> - **T60.4** — `FULGURITE_SHOT`/`FULGURITE_PROFILE`, `Local` drives through
>   `Session::agent` and `Local::live`, `examples/form`.
> - **T60.5** — every recipe green; `examples/counter-window` 1,383 → 514
>   lines, its window loop 538 → 0.
>
> **Written 2026-09-27.** From the coordinator's audit of
> `examples/counter-window`: about 540 of its 1,383 lines
> (`src/window.rs` 399, `src/agent_socket.rs` 139) re-implement what
> `fulgurite_winit`'s `Program::run` and the shell's `Local`/`Live`
> already do. Two things genuinely need the hand-written loop, and both
> are framework gaps, so the milestone is **two hooks, then a
> migration**.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and prints the measured quantity beside its bound.*

---

## 0. The rules for this milestone

1. **One path for every hook.** The window (`run`), a headless
   `Program::host()`, and the agent's `input.*` all reach application
   code through `Host::pump_with`. A hook wired anywhere else is a second
   path, and the second path is where a gate stops seeing it.
2. **A hook that changes state marks what it changed**, so its frame is
   drawn under the default `Boundaries<256>` and not only under
   `Everything` (M25/T25.0's shape: a correct frame under the diagnostic
   and a stale one under the default).
3. **Nothing the drives use is lost.** The default agent address
   (127.0.0.1:9230, `FULGURITE_AGENT_ADDR` overriding), `app.quit`, every
   agent method the live gate calls, `ui.screenshot` with and without
   `source: "surface"`, input latency in `app.perf`, and the `a11y`,
   `simd` and `threads` passthroughs.
4. **What leaves the example goes into a crate, not into a second
   example.** A headless `Screen`, the harness's adapter over a session,
   and the profiler are framework pieces or they are nothing.

## 1. What exists today

- `window.rs` is a full `ApplicationHandler`: window creation, the font,
  the measurer, the canvas, scale changes, IME, event translation with
  click counting, modifiers and cursor, the agent socket's `UserEvent`
  and `Rpc`, `app.quit`, `FrameTiming`/`frame_done` with input latency,
  and the a11y adapter. Every one of these is also in
  `crates/fulgurite-winit/src/lib.rs`'s `Win`, **except input
  latency**: `frame_times` there sets `input_latency_ns: None` and says
  "`counter-window` measures it itself". `ui_check.py latency` fails on
  an empty latency list, so the migration must bring that into `Win`
  first.
- `agent_socket.rs`'s `Live` is a copy of
  `crates/fulgurite-shell/src/session.rs`'s generic `Live`, plus a
  sign re-cascade in its `pump`.
- **App-level keys** (`WinApp::app_keys`): Up and Down nudge the count,
  Escape quits. Run on every key press *before* it is posted, whether or
  not a widget then consumes it — and never from the agent's
  `input.key`, which went through `Live::pump` and did not call it.
- **Per-sign re-styling** (`sync_sign`), written twice — once in the
  window's redraw, which pumps, re-cascades and pumps again, and once in
  `Live::pump` — calling `host.restyle_subtree(&stylesheet_for(sign),
  "card")` when `count.n.signum()` changes. `Program::style` is re-asked
  only at build and on `adopt`/`restore`/`reload` (`Host::reasked`).
- `headless.rs`: `--headless path count`, `--profile` via
  `FULGURITE_PROFILE`, `--drive` (clicks by author id through
  `ui.getLayout`, then `Host::click`) and `--drive2` (M12's `TestApp`,
  over a hand-written `AgentHost` in the example). `form.rs`: `--form`,
  the M8/M9 widget fixture that `just fixtures` runs — a different
  application sharing four palette constants.

## 2. The decisions

### 2.1 The key hook — `Program::on_key`

```rust
pub enum KeyOutcome { Ignored, Handled, Quit }
application(boot).on_key(|state: &mut A::State, key: Key, mods: Mods| -> KeyOutcome)
```

- **A `fn` pointer, taking the state** (M33's `App::State`): a key
  handler is handler-shaped work and gets what a handler gets. For a
  split application it cannot reach the tree, so its change reaches the
  screen the way a click's does — through the bindings, which mark
  their own nodes (M29/T29.3). For an unsplit one `State = Self` and it
  may write anywhere, so `Handled` marks everything, as a handled event
  does (`Host::mark_handled`).
- **Held by the `Host`, called from `pump_with`**, right after
  `dispatch_now` returns `false` for a key *press*. That is the one path
  (rule 1): the window posts a key into the host's queue, `host()` is
  the host, and the agent's `input.key` posts into the same queue.
- **"Consumed" is dispatch's answer, including its focus rules.** Tab
  moves focus and is consumed; an arrow on a focused node whose parent
  has another focusable child moves focus within the group and is
  consumed (T15.6); an arrow in a node that `owns_arrows` is the node's
  (M44/T44.1); Escape with an overlay open closes it. None of those
  reach the hook. **This changes the tally**: after a click on `#inc`,
  focus is on a step button in a row of four, so Up moves focus to
  `#dec` instead of nudging — the old loop nudged *and* moved focus,
  because it ran the key before posting it. The framework's rule wins;
  the old behaviour was the double handling this hook exists to rule
  out.
- **A focused node that owns its arrows owns its navigation and
  editing keys too**, whether or not it acted on them: arrows, Home,
  End, Page Up/Down, Backspace and Delete, while the focused node
  answers `Node::owns_arrows` (a text field, a slider, a list). *What
  building found, 2026-09-27:* dispatch reports an arrow at a field's
  edge as **not consumed** — T60.0's first run pressed Left into an empty
  `TextInput` and got `handled: false` — so "not consumed" alone would
  have let an arrow at the end of a field nudge the tally. `owns_arrows`
  is the only non-mutating question a node answers about its keys, so
  it is the one the rule asks.
- **`Quit` is a request, not an exit.** The host records it
  (`Host::quit_requested`); `Session::wants_quit` carries it across the
  session seam; `Win::about_to_wait` exits on it, as it does for
  `app.quit`. A headless host has nothing to exit and just reports it.
- Rejected: a method on `App`. `App` is derived, and a default method
  there is the provided-method trap for every hand-written impl; and a
  program-level setting is where M32 put everything else a window asks
  of an application.

### 2.2 Style keyed on state — `Program::style_by`

```rust
application(boot).style_by(|t: &Tally| t.count.n.signum(), stylesheet_for)
```

`key: fn(&A) -> K`, `sheet: fn(K) -> Vec<Rule>`, `K: PartialEq + Copy`.
It installs `|a| sheet(key(a))` as the program's sheet source — so
`adopt`, `restore` and `reload` re-ask it exactly as they re-ask
`style` — and gives the host the key. **`pump_with` compares the key
after `update` and before layout** and, when it changed, re-asks the
source and applies it. After `update` because the key reads state that
handlers, `apply`, `tick` and the key hook all write before that point;
before layout because the frame that lands the change must be the frame
that shows its colour.

**Full or scoped** is decided on the tally's own numbers in T60.2, not
on `docs/results-cascade-cost.md`'s 901-node fixture — the tally has
fifteen nodes, and the scoped path carries an invariant (the new sheet
may differ only inside the subtree) that a program-level API would have
to make every caller promise.

## T60.0 — the gates, red

The API surface lands **without its mechanism**: the outcome enum (§2.1),
`Program::on_key`, `Program::style_by`, `Host::quit_requested` and
`Session::wants_quit` exist, and the host stores the hook and the key
but never calls either. So the gates compile and fail for the reason
they exist, and commit as `#[ignore = "red until T60.k: …"]`.

- `crates/fulgurite-shell/tests/program_hooks.rs`:
  - an unconsumed Up reaches the hook, `pump` answers `handled`, and the
    label's new text is **painted** under `Boundaries` — pixels equal to
    an `Everything` host's and different from the frame before (the
    count of changed pixels printed beside its bound of > 0);
  - arrows into a focused `TextInput` (`owns_arrows`) and Tab never
    reach the hook, and the same Up with nothing focused does (the
    accept direction, in the same test);
  - `Quit` sets `quit_requested`, `Ignored` changes nothing and does not
    count as handled;
  - a click that flips the sign repaints the value's colour **on the
    frame the click lands**, under `Boundaries`, counted in pixels of
    the new colour inside the value's box.
- `crates/fulgurite-shell/tests/agent_hooks.rs` (`live-agent`): the
  same two through `Session::agent` — `input.key` reaches the hook, and
  an `input.click` that flips the sign is recoloured in the frame its
  reply names, read back with `ui.screenshot` from a headless screen.
- The tally's acceptance checks, as unit tests in the example: Up, Down
  and Escape against `program(0).host()`, the pill and value recoloured
  with the sign, and the example's source naming no `winit` item.

## T60.1 — the key hook

> **DONE 2026-09-27.** `Host::key_hook`, called from `pump_with` for a
> key dispatch did not take; `Win::about_to_wait` exits on
> `Session::wants_quit`. All four injections fired (commit body).

Wire `on_key` into `pump_with`; `Win` exits on `Session::wants_quit`;
the reload crate's `Hot` forwards it. *Injections:* (a) never call the
hook — the unconsumed-key gates fail; (b) call it for every key press,
consumed or not — the consumed-key gate fails, naming the key; (c) drop
the mark for an unsplit application's `Handled` — the `Boundaries`
pixel comparison fails.

## T60.2 — style keyed on state

> **DONE 2026-09-27.** `pump_with` compares the key after `update` and
> re-applies the **whole** sheet on a change: scoping it to `#card`
> saves 2% of the flip frame at the tally's 13 nodes
> (`docs/results-cascade-cost.md`, M60/T60.2), so the promise
> `restyle_subtree` needs is not worth extracting from every program.
> *Injection:* the comparison skipped — 0 px of the new colour on the
> click's frame, headless and through the agent.

Wire the key comparison into `pump_with` and `reasked`. Measure the
tally's re-cascade full against scoped and record it. *Injection:* skip
the comparison — the sign gate fails with 0 pixels of the new colour.

## T60.3 — the window, migrated

> **DONE 2026-09-27.** `main` runs `app::program(0, 440, 520)`, which
> carries the title, size, page, the agent's default address, `accessible`,
> `on_key(keys)` and `style_by(sign, stylesheet_for)`; `window.rs` and
> `agent_socket.rs` are deleted and the example no longer depends on
> winit or softbuffer. `Win` measures input latency (`journal_times`).
> `just live-gate` passes all eight checks against the migrated window,
> and `just a11y-drive` finds it on the bus.
>
> **What building found, 2026-09-27: every `Program` window's `ui.lint`
> was the block stand-in's opinion.** The first live-gate run on the
> framework's window failed lint with two W1005s (`#dec10` "wants 72x38
> but only got 66x44"): `Local` built its agent with `Agent::new()`,
> whose audit measures with the block stand-in, while its host laid out
> with the system face. The old loop had called `set_text_measure`
> itself. `Local::from_host` does now; `tests/agent_audit_face.rs`
> reproduces the two findings without it.
>
> **Not driven with a real keyboard**: the machine has no `xdotool`. A
> window key reaches the same host queue through `keys::deliver_key`;
> `input.key` Up, Up, Escape against the live window moved `#value` to 2
> and closed it with exit status 0.

`main` becomes `program(count).title(..).size(..).agent("127.0.0.1:9230")
.accessible(..).on_key(..).style_by(..).run()`. `window.rs` and
`agent_socket.rs` are deleted. `Win` learns input latency (§1), with a
`frame_times` test. The live gate and `a11y-drive` are the acceptance.

## T60.4 — the headless half

> **DONE 2026-09-27.** `--headless` is gone: the count is the first
> argument, captured by the boot, and `FULGURITE_SHOT=<path>` renders it —
> byte-identical to the old `--headless` at counts 0, 12 and −7.
> `--drive` sends `input.click` through `Session::agent` on a
> `Local::from_program` session with an `Offscreen` screen, and requires
> `handled`; `--drive2` runs `TestApp` over `Local::live`, the shell's
> adapter. Both now run in `just fixtures`. `FULGURITE_PROFILE` is the
> window crate's (`fulgurite_shell::profile`), with
> `FULGURITE_PROFILE_CLICK=inc` giving the same damaged region as the old
> profile (9,152 px, `[176, 143.5, 264, 247.5]`). `--form` is
> `examples/form`, whose frame is byte-identical to `--form`'s.
>
> **What building found: the old profile's first row measured nothing.**
> It printed `pump(layout+paint) 0 us`: under `Boundaries` since
> M34/T34.2 a pump with nothing marked is skipped (M53/T53.3), and it
> pumped fifty times with nothing marked. The row now marks everything
> before each pump, so it is a whole frame's layout and paint (4 µs on
> the tally, debug build).

- `--headless` becomes `FULGURITE_SHOT=<path>` with the count as the first
  argument, read in boot.
- `--drive` drives `Local::from_program(..)` through `Session::agent`,
  with a headless `Screen` the shell provides.
- `--drive2` drives `TestApp` over the shell's `Live`, through a
  `Local::live(screen)` accessor — the adapter lives in the shell.
- `FULGURITE_PROFILE` becomes the framework's, beside `FULGURITE_SHOT`; the
  partial-frame half clicks the node named by a `FULGURITE_PROFILE_*` variable.
- `--form` moves to `examples/form`, which `just fixtures` runs.

## T60.5 — verify and document

> **DONE 2026-09-27.** Every recipe and script that runs `counter-window`
> was run: `tally-frame`, `tally-drive`, `profile`, `profile-simd`,
> `profile-threads`, `fixtures` (which runs both drives and `form` now),
> `live-gate`, `a11y-drive`, and the `quit-panic` live plant — red as
> expected (`app exited 101`) with its plant moved into the window
> crate. `run-agent`, `tally` and `tally-a11y` are the window itself.
>
> **Lines** (`wc -l`, `src/*.rs`):
>
> | | before | after |
> |---|---:|---:|
> | `examples/counter-window` | 1,383 (6 files) | **514** (3 files) |
> | …of which a window loop (`window.rs`, `agent_socket.rs`) | 538 | **0** |
> | …of which the form fixture | 286 | 0 — `examples/form`, 300 |
> | …of which the M60 acceptance tests (`app.rs`) | — | 75 |
>
> The framework grew by what the example had and it lacked: the two
> hooks, `Offscreen`, `Local::live`, `profile.rs`, input latency in `Win`
> and the audit's face in `Local`.
>
> **`just test` green; `just live` 11 of 12.** `panic-drive` fails its
> wake bound (the loop woke 161–292 times in 2 s with the panic screen
> up, bound 4) **intermittently, and identically at the base commit
> `2b36612`** (9, 11 and 840 wakes in three runs there): the bound counts
> the main thread's context switches, and the display is shared with
> another session's windows. Not this milestone's; recorded for the
> coordinator. It passed in `just ci`'s run of `8d496c1`.

Every recipe and script that runs `counter-window` green; module docs,
the book's citations and `docs/plan-desktop.md` updated; line counts
before and after recorded here.
