# Windows, dialogs, clipboard, cursors

Everything in this chapter lives in the windowing crate, and the pattern
is the same in all four cases: **the core learns nothing about it.**
That is not modesty, it is what keeps the `no_std` profile honest.

## Multi-window

**Not reachable from an application yet**: `run` and a `Program` open
one window. M89 is building the rest (`docs/plan-windows.md`), and its
first step is already in: the window crate holds one `WindowSlot` per
window, with what the process has one of (the clipboard, the reload
state, the agent's `app.quit`) beside the slots rather than inside them.
A window event finds its slot by window id, and a worker's wake carries
its window's key, so it asks *its own* window for a frame.

`Host` is already one tree and one surface, which turns out to be the
right unit. A second window is a **second session** — its own tree,
stylesheet and damage stream — not a `Host` with two of anything, and
the core learns nothing about it. A slot holds its session behind the
same `Session` seam a hot library uses, so the window's state is never
loader-side code that a reload could not replace.

The practical consequence: two windows share nothing except what your
application passes between them, and M89 makes that a message rather
than shared state.

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

Four capabilities, four different mechanisms, one rule:

| capability | mechanism | why | reachable |
|---|---|---|---|
| multi-window | none needed | a second `Host` was already the right shape | no: types only |
| clipboard | default-on feature | small, and close to table stakes | yes |
| dialogs | default-off feature | large dependency, measured | no: types only |
| cursors | a `Style` field | cascades, and fits in existing padding | yes |

Platform drag and drop is in the same state as multi-window and dialogs:
`translate::dropped` exists behind `dnd`, and the turnkey window routes
no drop (`docs/plan-a11y-platform.md` §2.5).

The rule is that **none of it is paid for by an application that does not
ask** — and which mechanism enforces that is chosen per capability, from
the measurement, rather than by habit.
