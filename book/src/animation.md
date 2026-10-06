# Animation

The whole module rests on one constraint, and everything else follows
from it.

## The core has no clock

`std::time` is not available in a `no_std` build, and an embedded
target's timer is a peripheral — a thing the *host* owns. So the core
does not ask what time it is. The host says, once a frame, and
everything here is arithmetic on the number it was given.

That is why animation works identically headless, in a window, and on a
microcontroller: none of them differ in how time reaches the arithmetic,
only in where the number comes from.

## `Animated<T>`

```text
Animated<T: Lerp> {
    from: T,
    to: T,
    duration: f32,   // ms for the whole move
    easing: Easing,
    elapsed: f32,    // private; at >= duration the value is `to`
}
```

`Lerp` is the trait that says a type can be interpolated. Anything
implementing it can be animated — a length, a colour, an offset.

## `Easing`

Four values, and the comments on them are worth following rather than
guessing:

| easing | use for |
|---|---|
| `Linear` | mechanical motion, progress that must read as steady |
| `EaseIn` | something **entering** — slow to start |
| `EaseOut` | something **leaving** — slow to stop |
| `EaseInOut` | the default; the one to reach for when unsure |

## Why there is no animation pass

An `Animated<T>` is an ordinary field on whatever owns it, advanced by
the host's frame time. There is no per-node tick, no registry of running
animations and no traversal that visits every node asking whether it is
animating.

This is the M15 gating policy again: an application with no animations
does **no** animation work, rather than a small amount per node per
frame. The alternative — a pass — would have been simpler to write and
would have taxed every tree in the framework, including the embedded ones
that have no business paying for it.

## Driving one

The host advances time; you read the value during layout or when setting
a bound field. A frame in which an animation is still running is a frame
in which something changed, so it invalidates the way any other change
does — and a frame between ticks in which nothing moved is *known to be
still*, because the bindings compare before they write. See [What a frame
does](./frame.md).

That is the property that lets an idle animated interface cost nothing:
"animating" and "changing this frame" are different questions, and only
the second one costs anything.

## An animated image

An animated GIF is the case where "animating" and "changing this frame"
come furthest apart: ten changes a second, against a display that would
happily draw sixty. M84/T84.3 keeps the clock in the application and the
frame index on the widget, so there is still no animation pass and no
second image widget — an animated image is an `Image` whose `frame`
moves. The clock is `Playback`, beside `Animated<T>`, behind `alloc`
because its delays are a `Vec`:

```text
{{#include ../../crates/lumen-static/src/anim.rs:playback}}
```

Two of its answers carry the design. `tick` answers a frame **only when
the frame changes**, so an application's tick that forwards it reports
`true` ten times a second and not sixty — `App::tick`'s contract, and the
reason the whole-tree mark a reporting tick costs is paid at the image's
rate. And `next_change_ms` says when the next change is due, which is
what `App::wake_in_ms` answers: the window sleeps until that instant
rather than ticking at vsync, and a paused image answers `None` and
presents nothing at all. The showcase's Image entry is the whole shape —
the tick:

```text
{{#include ../../examples/showcase/src/main.rs:image_tick}}
```

and the wake, which a derived application supplies with
`#[app(wake = f)]`:

```text
{{#include ../../examples/showcase/src/main.rs:image_wake}}
```

The pixels for every frame are the host's: the showcase decodes its GIF
once into an `ImageStore` and hands it to the window, and each frame's
command names the frame it draws. What one frame costs, and what the
whole-tree mark per animated frame costs, are in
`docs/results-images.md` (T84.3).
