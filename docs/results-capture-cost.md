# Results: what routing pointer moves costs

*2026-09-04, plan M11/T11.2. Machine: i9-13900KF, governor `powersave`.
Live window, `app.perf` over its 128-frame ring.*

## The objection this had to answer

M9 deliberately did **not** route `PointerMove`. `pass::dispatch` said
why: it fires continuously, its only consumer was the hover bit, and
routing it would walk the ancestor chain on every motion looking for a
handler that is almost never there.

A slider needs those moves — and needs them *after the pointer has left
the control*, which a hit test cannot deliver at all. So T11.2 had to
add routing without reinstating the cost the objection named.

## The design that keeps both

**Pointer capture.** A node that consumes a `PointerDown` owns the
gesture until the release. While a capture is in flight the event is
delivered **directly to that node** — no hit test walk, no ancestor
walk, one virtual call. With no capture, a move does what it did before:
hover bookkeeping, then return.

The capture check is one `Option<u32>` comparison on a path that already
hit-tests for hover.

**It has to happen before the hit test**, which is the part that is easy
to get wrong and which the tests caught: once the pointer leaves every
node, hit testing finds nothing and the pointer arm returns early, so a
capture checked afterwards never fires in the one case it exists for.

## The number

Live tally, 128-frame window:

| | frame p50 | frame p95 |
|---|---:|---:|
| 60 clicks (state changes) | 0.092 ms | 0.204 ms |
| 200 hover moves | **0.033 ms** | **0.061 ms** |

Motion frames are **cheaper than click frames**, because a move damages
only the hovered control's shade while a click changes the value and
relayouts. Routing moves did not make a frame more expensive; the frame
cost is dominated by how much was damaged, not by what dispatch did with
the event.

**Verdict: accepted.** M9's objection was to an ancestor walk per
motion. Capture does not add one — it removes the need for one — and the
measured motion frame is a third the cost of a click frame.

## What this does not measure

Frame time, not dispatch time. Dispatch is a small part of a 33 µs
frame dominated by raster, so this shows the *change is not visible at
the frame level* rather than putting a number on dispatch itself. If a
future change makes dispatch expensive enough to matter, it will need
its own bench — `benches/` has none today, and that is a gap rather
than a decision.
