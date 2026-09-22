# Backends

The same tree renders three ways, and the application does not change
between them. That is the property this chapter is about.

## Headless: `Host` and `Canvas`

The smallest thing that produces pixels:

```text
let mut host = Host::launch(320, 200);   // Application::build + ::style
host.pump();

let mut canvas = Canvas::new(320, 200);
canvas.clear(CLEAR);
host.render(&mut canvas);
```

No window, no event loop, no display. `examples/counter` does exactly
this, clicks its own buttons through the agent protocol, asserts the
result and writes a PPM — and `just fixtures` runs it in CI, which is
what makes its assertions mean anything.

This is the mode to reach for in tests. It is also the mode an embedded
target uses, with a different `Painter` on the other end.

## Windowed: `run()`

```text
run(app, Config {
    title: "minimal",
    width: 320,
    height: 160,
    ..Config::default()
});
```

`examples/minimal` is the whole application in about twenty lines. What
the windowing crate adds to the headless case is exactly two things:
window events into the host's queue, and canvas pixels blitted to the
surface. Before that crate existed the same file would have been several
hundred lines, because there was nowhere else for the scale factor, the
buffer age, the damage and the font to live.

`Config::page` is worth noticing: it is the colour cleared behind every
*damaged region* before redraw, not a background you paint once.

## GPU: a feature on the windowing crate

```text
cargo run -p minimal --features gpu
```

**Not one line of the application changes.** That is a gate rather than a
claim — `examples/minimal` is built both ways and the difference is
measured (`docs/results-gpu.md`), and `docs/results-competitive.md`
measures both against other toolkits.

`Prefer::Auto` uses the GPU when the crate was built with the feature and
an adapter exists, and the software canvas otherwise. And there is an
escape hatch for the person *running* the program rather than the person
who wrote it:

```text
LUMEN_BACKEND=software   # or =gpu
```

A driver bug on one machine should not need a rebuild to work around. An
unrecognised value is ignored rather than fatal.

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
