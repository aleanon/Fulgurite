# Adopting Lumen's `lumen-text` and `lumen-render`: measured, and declined

*2026-09-04, plan M7a (T7.1, T7.2). The plan said "adopt `lumen-text`
behind `dyn TextMeasure`" and "adopt `lumen-render` behind the existing
`Renderer` trait". Both tasks were written in 2026-09-01, before this
project had any text or raster stack of its own. It now has both. This
records what adoption would actually cost, and what is adopted instead.*

## The measurement

`cargo tree --edges normal`, unique crates in the graph:

| crate | transitive crates |
|---|---:|
| Lumen's `lumen-text` | **74** |
| Lumen's `lumen-render` | **84** |
| this project's `lumen-static` (core) | **6** |
| this project's `lumen-static-shell` (with `truetype`) | **7** |

Both Lumen crates hard-depend on `lumen-core` (7,684 lines): its signal
graph, its store, its `State: Serialize + DeserializeOwned` bound and
its snapshot machinery. `lumen-render` additionally pulls `tiny-skia`,
and `wgpu` + `lyon` behind a default-on feature.

## Why that is not a trade, but a contradiction

`lumen-core` is the reactive-rebuild architecture this project exists to
be an alternative to (§3 of the handoff: no rebuild, no signals). Taking
it as a dependency to obtain a font measurer would put the whole of it
in the graph of a framework whose stated properties are zero allocation
in steady state, no `unsafe` in the core, and a `no_std` profile that is
still reachable (B11). 6 crates to ~90 is not a dependency bump; it is a
different project.

The seam was always the point, and the seam is fine: `dyn TextMeasure`
and `Renderer` are traits, and Lumen's stack can still be composed in
behind them by anyone who wants it — in a separate crate, not this
workspace's default graph.

## What is adopted instead

**Lumen's interface, not Lumen's crates.** T7.1 and T7.2 are satisfied
by what `lumen-static-shell` already ships and by extending it:

- **Text:** `FontStack`/`FontMetrics` (fontdue) behind `dyn TextMeasure`
  — real faces, real rasterization, advance-based measurement, wrapping,
  cross-face fallback. One dependency, no `lumen-core`. What it lacked
  is what T7.2's own justification named: per-glyph positions for M8/M9's
  `Shaped` cache. **T7.2b adds them** (`TextMeasure::shape`), which is
  the task the plan already said must be designed before wiring.
- **Raster:** the software `Canvas` behind `Renderer` — AA rounded
  corners, one-pass shadows, clip scopes, glyph blitting, pixel-tested
  (`shell/tests/clip_test.rs`, `glyph_placement.rs`) and profiled
  (`docs/results-frame-cost.md`).

What this defers, honestly: no complex-script shaping (parley does
bidi, Indic reordering, cluster-correct segmentation; fontdue's layout
does not), no GPU backend, no SVG/gradient/image commands. None of them
is on the path to M8b/M8c/M8/M9, and each can arrive behind the same
seam. The `no_std` and zero-allocation profiles stay reachable, which
adoption would have ended.

## Consequence for T7.3

T7.3 measures what `Label`'s memo is worth "against the real shaper".
The real shaper is now `FontMetrics`, not parley. The trap T7.3 names —
a shaper with its own content-keyed cache making a memo *miss* cost a
hash lookup rather than a shape — does not apply: fontdue has no such
cache, so a miss is a real re-layout and the number is the honest upper
bound.

**Ran 2026-09-04** (`docs/bindings-decision.md`, T7.3 section): a memo
hit is 4.8 ns; a miss is 2.46 µs unwrapped and **16.7 µs** for a
60-character label wrapped at 200 px. The memo stays. The larger
finding is that one wrapped label costs more to measure than a
901-node tree costs to lay out, which is where the next frame-budget
risk lives now that the raster is fixed.
