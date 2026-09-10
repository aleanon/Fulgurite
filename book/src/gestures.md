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
