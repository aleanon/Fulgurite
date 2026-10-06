# Gestures, touch, drag and drop

## Why recognisers are types you own

Recognition is stateful: a pinch is two touches over time, a long press
is one touch and a clock. A framework *pass* that recognised gestures
would run for every tree, including the overwhelming majority that want
none — which the M15 gating policy forbids.

So they are plain structs that a widget or an application holds and feeds
the same `Event::Touch` stream everything else sees. Nothing is paid by
an application that holds none.

## The three recognisers

**`Touches`** — the set of fingers currently down. A fixed-size array,
so no allocator: `feed(&Event)` returns whether the set changed.

**`Pinch`** — holds a `Touches` and the spread at which the second finger
landed, because a scale is relative to something. `1.0` is the spread the
gesture started at.

**`LongPress`** — `hold` in milliseconds and `slop` in logical pixels:
how long the finger must be down, and how far it may drift and still
count.

Note that raw `Event::Touch` is in the core event vocabulary but
*recognition* is not. That line is drawn deliberately, and the module
header says so.

**The first finger is also a mouse.** A `Host` dispatches every
`Event::Touch` as it arrives, so a recognizer sees all of them, and then
turns the *primary* touch — the first finger down, until it lifts — into
the pointer events a mouse would have sent: a move and a press on
`Begin`, a move on `Move`, a release on `End`. That is why a tap presses
a `Button` although no widget handles touch itself. A `Cancel` becomes
`Event::PointerCancel`, which ends the gesture and activates nothing: a
cancelled tap is not a click. Other fingers never drive the pointer.

## The one recogniser the host owns: a held touch

There is one exception to "an application owns its recognisers"
(M90/T90.3). A `Host` holds one `LongPress` for the primary touch. When
the finger has been down and still for its 500 ms, the host asks for a
context menu there — `Event::ContextRequest` at the touch point, offered
to what is under it and its ancestors, the same event a right click and
Shift+F10 produce — and marks the touch **spent**, so its `End` becomes
an `Event::PointerCancel` rather than a release.

The reason is the release. A button, a checkbox and a menu row activate
when the finger lifts, so the lift of a long press would press whatever
it is over and close the menu that just opened. Only the intake can turn
that release into a cancel: an application's own `LongPress` sees the
`End` after the host has already delivered it as a release. A drift past
the 10 px slop makes it a drag and the lift a tap again, and while a hold
is pending the host's next deadline includes it, so a window that sleeps
until something is due still fires it:

```text
{{#include ../../crates/lumen-static-shell/tests/context_routes.rs:held_touch}}
```

**A control that acts on the press has already acted.** The primary
touch's `Begin` is a press at once, so 500 ms before the hold fires a
collapsible has toggled and a tab has been selected; cancelling the
release cannot undo that. Holding the press back until the hold resolves
would delay every tap's press, and its press shade, by the length of the
tap, so it is not built. The numbers are in
`docs/results-desktop-integration.md` (T90.3).

## `Drag<T>`, and the two halves not to conflate

**Within an application** is `Drag<T>`: a payload, drop targets and hover
feedback. The payload is the *application's* type, which is why it is a
type parameter — it costs nothing when nothing drags, and it needs no
allocator.

**Across the platform boundary** — a file dropped on the window, text
dragged out to another program — is the windowing layer's business, not
the core's.

### The threshold is the part people get wrong

```text
pub threshold: f32   // logical px before a press becomes a drag
```

Without one, **every click is a one-pixel drag**, and a list whose rows
both click and reorder cannot tell the two apart. Four pixels is the
platform-conventional value, and it is deliberately smaller than the
multi-click slop, which answers a different question — that one is about
whether two clicks are near enough to be a double-click.

If you build anything that both selects and reorders, this field is the
one to think about before shipping.

## Animation

The core **has no clock and cannot have one**: `std::time` is not
available in a `no_std` build and an embedded target's timer is a
peripheral. So nothing here asks what time it is. The **host** says, once
a frame, and everything in the module is arithmetic on the number it
gave.

```text
Animated<T: Lerp> {
    from: T,
    to: T,
    duration: f32,   // ms
    easing: Easing,
}
```

`Easing` is four values, and the doc comments are opinionated in a useful
way: `EaseIn` is the default for something *entering*, `EaseOut` for
something *leaving*, and `EaseInOut` — the actual default — is "the one
to reach for when unsure".

The tick is not a per-node pass. An `Animated<T>` is a field on whatever
owns it and is advanced by the host's frame time, which means an
application with no animations does no animation work at all rather than
a cheap-per-node amount of it.
