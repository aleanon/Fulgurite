# The rasteriser

The software rasteriser turns a display list into pixels. It matters more
than a tree-walking framework's rasteriser usually does, because
[What it costs](./costs.md) established that the tree walk is *not* where
frame time goes — rasterisation is.

## The command vocabulary

Nine commands: `Rect`, `RoundedRect`, `Text`, `Shadow`, `Line`,
`Gradient`, `Image`, `PushClip`, `PopClip`.

Deliberately small. Every command is something an embedded target can
implement without an allocator, and everything richer is composed from
them. A `Painter` implementation must write four of them; the rest have
provided bodies that degrade — which is the trap [Writing your own
widget](./custom-widget.md) is mostly about.

## Band-parallel rasterisation

The canvas is split into horizontal strips, and strips are pulled from a
**shared queue** rather than assigned up front — so a slow core takes
fewer of them, and a strip containing an expensive shadow does not stall
a whole worker.

The concurrency protocol is deliberately boring: `chunks_mut` proves the
strips disjoint, taking one out of the iterator *moves* the `&mut`, and a
mutex over the iterator is the whole thing. Per-thread deques and a steal
handshake are for tasks that spawn subtasks, and these do not. No
`unsafe`.

`LUMEN_WORKERS` and `LUMEN_STRIP_ROWS` override the defaults.
`docs/results-threading.md` has the sweep and the chosen defaults.

## The bug that made "parallel" slower

This is the most useful story in the chapter, because the shape of the
mistake is general.

Rendering culls each command against the clip stack before doing any work
for it — and for text, that check is what stands between the renderer and
the call that **shapes and rasterises an entire run**.

On a *full* redraw there is no damage rect, so the clip stack started
empty and nothing was culled. Every strip therefore re-shaped every text
run in the whole scene. With hundreds of strips and hundreds of runs, the
"parallel" render came out **an order of magnitude slower than one
thread** (`docs/results-threading.md` §4).

The fix is one push: **a band's own rows are a clip**. The stack starts
with the band rect, intersected with the damage rect when there is one.

Two things fell out of it. Once strips stopped re-shaping, *more* bands
beat fewer — the heterogeneity penalty was real but small and had been
swamped by a cost that grew with band count, so the cap came off. And the
earlier measurement of band-parallel speed-up had been measuring a
defect, not a limit.

The general lesson: **a parallel implementation that scales badly may be
doing redundant work per partition rather than contending.** Those look
alike in a wall-clock number and completely different in a profile.

## Testing a banded renderer

Byte-identity between a banded render and a single-threaded one is the
right property, and it is easy to test vacuously. `tests/banded.rs` once
claimed its five sizes were "large enough to cross the splitter's
threshold at every one of them"; six of its ten cases used **one band**,
so the checksum compared a single-band render with itself and would have
agreed however badly the splitter was broken.

The comment was wrong by a factor of two — the threshold quoted was the
per-*worker* quantum, and splitting needs two workers. `Canvas::bands_for`
exists so the test can **assert its own setup** instead of describing it.

> A threshold named in a comment is not a threshold that was checked.
