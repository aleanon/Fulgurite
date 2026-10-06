# Windows, dialogs, clipboard, cursors

Everything in this chapter lives in the windowing crate, and the pattern
is the same in all four cases: **the core learns nothing about it.**
That is not modesty, it is what keeps the `no_std` profile honest.

## Multi-window

A program can open more windows than its own (M89, `docs/plan-windows.md`).
Each window runs **its own program** — a `Program` like the one you
already write — and the main program lists them in a **window table**:

```text
{{#include ../../examples/two-windows/src/main.rs:windows}}
```

A handler asks for entry `n` by pushing
`Request::OpenWindow { program: n, tag }` onto its `Requests`, and the
window creates the window and answers `Answer::WindowOpened { window }`
with its key **before the drain that took the request returns**, so
nothing is pending when a hot swap lands. An index the table has no
entry for answers `NoSuchProgram`. When a window you opened closes, you
hear `WindowClosed` under the unasked tag, `request::UNSOLICITED`.

**Closing a window and quitting are different.** `KeyOutcome::Quit`
from a key hook and `Request::Close` close the window they came from;
`Request::CloseWindow { window }` closes another; closing the **primary**
window — the program's own — ends the process, and so does
`Request::Quit` from anywhere.

A second window is a **second session** — its own tree, stylesheet,
damage stream, renderer face and accessibility adapter — not a `Host`
with two of anything, and the core learns nothing about it. The window
crate holds one `WindowSlot` per window, with what the process has one
of (the clipboard, the reload state, `app.quit`) beside the slots. A
window event finds its slot by window id, and a worker's wake carries
its window's key, so it asks *its own* window for a frame. A slot holds
its session behind the same `Session` seam a hot library uses.

**One agent port addresses every window**, by a `"window"` member on the
request (`agent.md`); only the primary program's address and
accessibility setting apply. `examples/two-windows` is the worked
example, and `just windows-drive` drives it under both strategies. What
two windows share is what your application passes between them, and
M89's next step makes that a message (`Request::Post`), not shared state.

## A program with no window

A **daemon** is a program that runs without a window of its own — iced's
`daemon` — and opens windows only when it wants them. It is described
exactly as an application is, with `daemon(boot)` where you would write
`application(boot)`:

```text
{{#include ../../examples/daemon/src/lib.rs:daemon}}
```

Its session runs **windowless**: no window, no renderer, and nothing
presented, but its timers, its `App::wake_in_ms` deadline and its inbox
run as a window's do, through the same `Session::frame` at a 1×1 size,
pumped at its deadline and on its waker. After each of those frames the
window crate drains its requests, so a daemon opens its table's windows
with `Request::OpenWindow`, closes them with `Request::CloseWindow`, and
ends with `Request::Quit`. Closing its last window ends nothing: a
daemon's windows are all table entries. Its key hook's `KeyOutcome::Quit`
closes nothing either — there is no window to close — and the window
crate says so on stderr.

`examples/daemon` counts 120 ms ticks on a tree nobody sees, opens its
panel at tick 10, closes it at tick 20, and quits at tick 40. Its timer
is its own deadline rather than `Timers::every`, because a `Timers`
firing is delivered by a job and costs two frames a period
(`docs/results-frame-cost.md`, "Idle"); a daemon that only counts needs
one:

```text
{{#include ../../examples/daemon/src/lib.rs:tick}}
```

The agent addresses the windowless session as window 0, which is a
daemon's default target, through a 1×1 offscreen screen, so `ui.resolve`
and `app.diagnostics` answer with no window open. `just windows-drive`
phase 5 drives it, and `just idle-count`'s daemon leg holds what it costs
idle (`docs/results-windows.md` T89.6).

## Clipboard

Copy and paste is split in half, deliberately:

* **The framework's half** — `Host::copy` (what is selected) and
  `Host::paste` (type this in). This is where the selection lives, so
  this is where it belongs.
* **The platform's half** — the actual system clipboard, in the
  windowing crate.

It is on by default: five crates by the per-crate count the windowing
crate's manifest records, which the gating policy judged cheap enough
for something this close to table stakes.

**An application can reach it, too** (M88/T88.4). Not by holding a
clipboard — a handler reaches only its own state — but through the same
queue a screenshot travels on (see *Events*): it pushes a
`ClipboardWrite` or a `ClipboardRead` naming the clipboard or the
**primary selection** (X11's select-to-copy buffer), and the window
answers by tag. A write's bytes are not in the request; the window asks
`App::clip_for` for them while it drains, in the build loaded then:

```text
{{#include ../../examples/tasks-demo/src/main.rs:clip}}
```

Text and HTML go on the clipboard and text on the primary selection,
with **no crate beyond the five** (`docs/results-tasks.md` T88.4). A
platform without a primary selection answers a primary request
`Refused(Unsupported)`, and so does HTML on the primary selection, which
a reader pastes as text. `just tasks-drive` checks what landed with an
independent `xclip -o -selection primary`, a tool the reference machine
is required to have.

**Images** go on the clipboard and come back the same way — a
`Clip::Image` write and a `ClipFormat::Image` read, RGBA8 — but only in a
window built with the windowing crate's default-off `clipboard-image`
feature (M88/T88.5), because it brings an image-decoding stack; its crate
count and size are in `docs/results-tasks.md` T88.5. Without it, an image
request is answered `Refused(Unsupported)`: the build cannot, which is
not the same answer as a clipboard holding nothing.

**Native Wayland has no clipboard in this build.** arboard is built
without its Wayland backend, so the clipboard is X11's: under XWayland
it works, and on a Wayland-only session the window finds none and every
clipboard request fails.

## Dialogs

**Not reachable from an application yet**: `Dialogs` compiles and
nothing calls it, and it has no test (`docs/plan-capabilities.md`
T15.11). What follows is why it is gated the way it is.

File, colour and font pickers, behind a **default-off** `dialogs`
feature — and the measurement is the reason rather than a formality.
`rfd` costs seventy-nine crates against clipboard's eleven and
accessibility's sixty-two (`docs/plan-capabilities.md` T15.11).

That is the gating policy working as designed: a capability whose cost is
large gets a switch, and the number is what decides which side the switch
defaults to. Nothing here is paid for by an application that does not ask.

## Cursors

The pointer shape is a **`Style` property**, not an event and not a
widget method — which means it cascades. A stylesheet says what the
pointer looks like over a kind or a class, and every node of that kind
gets it, including custom widgets that implement nothing.

It is also free in the sense the gating policy cares about: `Cursor` fits
in `Style`'s existing tail padding, which is asserted in
`crates/lumen-static/tests/cursor.rs` — because it would stop being free
if a future field pushed the struct over, and a byte in `Style` is a byte
times every node of every tree.

## The pattern worth taking away

Five capabilities, five different mechanisms, one rule:

| capability | mechanism | why | reachable |
|---|---|---|---|
| multi-window | a window table and three requests | a second session was already the right shape | yes, since M89/T89.3 |
| daemon (no window) | `daemon(boot)`, a windowless session pumped at its deadline | the same `Session::frame`, no new method | yes, since M89/T89.6 |
| clipboard | default-on feature | small, and close to table stakes | yes |
| dialogs | default-off feature | large dependency, measured | no: types only |
| cursors | a `Style` field | cascades, and fits in existing padding | yes |

Platform drag and drop is in the same state as dialogs:
`translate::dropped` exists behind `dnd`, and the turnkey window routes
no drop (`docs/plan-a11y-platform.md` §2.5).

The rule is that **none of it is paid for by an application that does not
ask** — and which mechanism enforces that is chosen per capability, from
the measurement, rather than by habit.
