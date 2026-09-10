# Where the decisions live

This book teaches the framework. It does not adjudicate it. When a page says
"because X", the reasoning and the measurement behind X are in `docs/`, and
this page is the index to that.

The split is deliberate and it is a rule, not a habit — `AGENT.md`:

> **Numbers are stated once.** In the results doc; cited everywhere else. A
> doc comment that must carry a figure carries the citation in the same
> sentence.

So a figure in this book always names its source. If you find one that does
not, that is a bug in the book.

## The design spec

`docs/00-handoff-static-tree.md` is the design document, and it is *live*:
sections carry **DECIDED**, **OPEN** and **SUPERSEDED** markers that move with
the code. §12 is the standing list of open questions. Read it when you want to
know whether something is settled.

## Decision records

| document | settles |
|---|---|
| `docs/layout-and-storage-decisions.md` | where a widget's values live |
| `docs/events-decision.md` | how input reaches a handler |
| `docs/text-and-scrolling-decisions.md` | text measurement and scroll state |
| `docs/text-render-adoption-decision.md` | which glyph rasteriser, and why |
| `docs/bindings-decision.md` | what `#[bind]` is allowed to do |
| `docs/damage-amplification-decision.md` | why damage is not a bounding box |
| `docs/mobile-decision.md` | the mobile target, and its status |
| `docs/either-reopened.md` | `Either`, and the case for reopening it |

## Measurements

Every `docs/results-*.md` states a number, the threshold it was judged
against, and the verdict — so the gate can say no and the next reader can see
why it did. The ones this book cites most:

| document | measures |
|---|---|
| `docs/results-allocation.md` | zero allocation in steady state, and its gate |
| `docs/results-threading.md` | band-parallel rasterisation, 4K redraw at 2.50 ms |
| `docs/results-frame-cost.md` | per-stage frame cost and input latency |
| `docs/results-gpu.md` | the wgpu backend against the software one |
| `docs/results-vello.md` | the software rasteriser against `vello_cpu` |
| `docs/results-embedded.md` | what fits in a microcontroller's memory |

## Plans

Work is queued in `docs/plan-*.md`, task ids appear in commit subjects, and a
shipped plan is kept rather than deleted — its header records what was built,
what was decided *against*, and what it measured. `AGENT.md` holds the current
queue.
