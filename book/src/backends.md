# Backends

The same tree renders three ways, and the application does not change
between them. That is the property this chapter is about.

## Headless: `Host` and `Canvas`

The smallest thing that produces pixels:

```text
let mut host: Host<Counter> = program().host();   // boot, palette, sheet
host.pump();

let mut canvas = Canvas::new(320, 200);
canvas.clear(CLEAR);
host.render(&mut canvas);
```

No window, no event loop, no display. `program()` is the application's
description — how to build it and how it is styled — and `host()` is the
same function a window calls to build the host it drives, so a headless
frame is the window's frame and not a second construction that could
drift from it (`docs/plan-program.md`). `examples/counter` does exactly
this, clicks its own buttons through the agent protocol, asserts the
result and writes a PPM — and `just fixtures` runs it in CI, which is
what makes its assertions mean anything.

This is the mode to reach for in tests. It is also the mode an embedded
target uses, with a different `Painter` on the other end.

## Windowed: `program().run()`

```text
application(boot)          // a function that builds the application
    .title("minimal")      // a constant, or a function of the state
    .size(320, 160)
    .style(stylesheet)     // likewise
    .run();
```

The shape is iced's `application()`. `boot` may be a closure that
captures: `examples/showcase` opens on an entry read from its arguments.

`examples/minimal` is the whole application in about twenty lines. What
the windowing crate adds to the headless case is exactly two things:
window events into the host's queue, and canvas pixels blitted to the
surface. Before that crate existed the same file would have been several
hundred lines, because there was nowhere else for the scale factor, the
buffer age, the damage and the font to live.

`.page(..)` is worth noticing: it is the colour cleared behind every
*damaged region* before redraw, not a background you paint once.

## A screen reader: the `a11y` feature

A window built with the windowing crate's `a11y` feature hands the
platform an accessibility tree — AT-SPI on Linux, UI Automation on
Windows, NSAccessibility on macOS, through `accesskit_winit` — so a
screen reader can see and read it. The tree is the one the agent reads
(`ui.getTree`), with bounds in physical pixels, and it is built only
while an assistive technology is attached.

**It is off by default, and asked for.** The rule that decided that was
written before the adapter was built, and the adapter broke it: the
binary grew past its bound (`docs/results-a11y-platform.md`). So an
application that wants a screen reader says so in its manifest, as the
showcase does, and `.accessible` turns it off at run time:

```text
{{#include ../../examples/showcase/src/main.rs:accessible}}
```

## GPU: a feature on the windowing crate

```text
cargo run -p minimal --features gpu
```

**Not one line of the application changes.** That is a gate rather than a
claim — `examples/minimal` is built both ways and the difference is
measured (`docs/results-gpu.md`), and `docs/results-competitive.md`
measures both against other toolkits.

`Prefer::Auto` uses the GPU when the crate was built with the feature and
a GPU adapter exists, and the software canvas otherwise. A CPU rasteriser
such as Mesa's llvmpipe, which is all a machine with no working driver
may offer, does not count: the software canvas is faster there
(`docs/results-gpu.md` §6). `Prefer::Gpu` takes it anyway. And there is an
escape hatch for the person *running* the program rather than the person
who wrote it:

```text
LUMEN_BACKEND=software   # or =gpu
```

A driver bug on one machine should not need a rebuild to work around. An
unrecognised value is ignored rather than fatal.

## Your own backend: `.backend_with`

A program can build its window's backend itself. The factory is handed a
`BackendTarget` — the window as raw handles, its physical size, the face
the session measures with, and whether a CPU adapter is acceptable — and
answers `Some(backend)`, or `None` to fall through to the built-in
GPU-then-software choice. It is asked on every `resumed`, before that
choice, in a build with or without the `gpu` feature;
`LUMEN_BACKEND=software` skips it, so the escape hatch still works.

`examples/glasswall`'s `custom-backend` feature is the smallest real one:
the software backend inside a counter whose name says which backend of
the run it is. It forwards every method, because the trait makes each one
that acts on what a backend holds required — a wrapper that inherited a
default `set_images` would drop every image without a word.

```text
{{#include ../../examples/glasswall/src/main.rs:factory}}
```

It is read once, from the program the binary started with, and never
across a hot reload: a hot library's factory is ignored, and changing it
needs a restart (`docs/plan-gpu-extension.md`). `just resume-drive`'s
`custom-resume` leg counts the factory's backends across a suspend and
resume, and checks the second one kept the wallpaper.

## What the GPU backend does and does not do

It keeps a persistent target texture that damage is scissored into, so a
partial frame is a few quads over a retained image. There is **no layer
tree** — no per-subtree texture, no transform-only frame, and raster is
still on the UI thread. [The retained-mode baseline](./baseline.md)
covers what that leaves on the table.

## Why the backend never appears in a widget signature

`paint` takes `&mut dyn Painter`. A widget describes *what* to draw and
never names where it goes: a desktop host collects into a `DisplayList`
and a renderer consumes it afterwards; a host with no allocator
implements `Painter` directly on a rasteriser and no list is ever built.

That is why "the same tree renders three ways" is structural rather than
a compatibility effort.
