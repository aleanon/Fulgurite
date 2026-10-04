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
  cross-face fallback (between regular and bold until M45/T45.3; since
  then a list of fallback faces — CJK, Hebrew, Arabic, symbols — read the
  first time a run needs one, `docs/results-text.md` T45.3). One dependency, no `lumen-core`. What it lacked
  is what T7.2's own justification named: per-glyph positions for M8/M9's
  `Shaped` cache. **T7.2b adds them** (`TextMeasure::shape`), which is
  the task the plan already said must be designed before wiring.
- **Raster:** the software `Canvas` behind `Renderer` — AA rounded
  corners, one-pass shadows, clip scopes, glyph blitting, pixel-tested
  (`shell/tests/clip_test.rs`, `glyph_placement.rs`) and profiled
  (`docs/results-frame-cost.md`).

What this defers, honestly: no complex-script shaping (parley does
bidi, Indic reordering, cluster-correct segmentation; fontdue's layout
does not — `lumen-static-text` later added it **for measurement only**,
M15/T15.8 as corrected by M45/T45.4: nothing paints the shaper's glyphs
— until M81, "Shaped paint" below), no GPU backend, no SVG/gradient/image commands. None of them
is on the path to M8b/M8c/M8/M9, and each can arrive behind the same
seam. The `no_std` and zero-allocation profiles stay reachable, which
adoption would have ended.

## Shaped paint: renderer-side, no glyph command — DECIDED (M81, 2026-10-04)

"Nothing paints the shaper's glyphs" (above) is closed by M81
(`docs/plan-shaped-paint.md`). **Decided: the renderer shapes, at paint,
in `FontStack::place_run_into`** — the one function both backends already
place every run through, every frame — and `Cmd::Text` stays the command.
A `FontStack` in `Shaping::Complex` (the shell's default-off `shaping`
feature) lays a run that needs shaping out with `lumen-static-text`'s
`Shaper` for the measure, the clusters and the paint (T81.3); a run with
nothing to shape keeps fontdue's layout even there, because shaping Latin
cost 2.2–3.6× (K81a, `docs/results-text.md` M81/T81.3). Since T81.4 a
program chooses it with `Program::shaping`, and the window, the session's
measurer, the agent's audit and every headless render take it from there
(`Session::shaping`, `Backend::set_font`), so they cannot disagree.

**Why not a glyph command** (`Cmd::Glyphs` over a per-frame store of
shaped runs, which `docs/plan-text-correctness.md` §2.1 A proposed): a
shaped run would have to travel from layout, the only place a measurer
is, to paint, which has only the node states and a `Painter`; and a
store index folded into `Cmd::hash_into` is the M27/T27.3 phantom-damage
defect (`docs/plan-iced-roadmap.md` S2). Keeping the string costs nothing
the command already had — `Cmd` stays 60 B, and damage hashing,
retention, `svg.rs` (whose `<text>` a viewer shapes itself) and the
contrast walk are unchanged — at the price of shaping per paint, which
the canvas's per-frame run shaping (M50/T50.3) already paid.

**Subpixel pens — DECIDED (M81/T81.7–T81.8).** A shaped glyph is drawn at
the nearest third of a pixel; a `Basic` glyph keeps fontdue's 1× bitmap,
so no golden moves. fontdue exposes no pen offset, so the thirds are
derived from its public `rasterize_indexed_subpixel` inside
`FontStack::rasterize`, keyed by `GlyphKey::phase`, and both renderers
take the column and the phase from one shell function, `text::pen_of`.
The derivation was probed first and holds within K81d's bound, so no
second rasteriser (`ab_glyph`) entered; the thirds did not thrash the
glyph cache on a grid whose edited runs are shaped (K81b). The numbers
are `docs/results-text.md` M81/T81.7 and M81/T81.8. Building it fixed a
defect M81a had shipped: a shaped glyph's bearing fraction was counted
twice, one pixel right whenever it was 0.5 or more
(`docs/plan-shaped-paint.md`, "What building found").

## The embedded face (M13/T13.7, 2026-09-05)

"The `no_std` profile stays reachable" turned out to have a hole in it,
and M13 found it: **`FontMetrics` is `fontdue`, which needs `alloc`, is
a `std` crate, and lives in `lumen-static-shell` — which M13 keeps
`std`.** So the embedded target had no measurer at all, and would have
laid out a `Label` whose glyphs nothing could draw. The seam was right;
nothing had been put through it from the other side.

`lumen_static::font::BakedFace` is that second implementation: advances
and 8-bit coverage in `const` data, indexed by a sorted glyph table,
measured and drawn with no allocation and no shaping.
`fixtures/bake-font` generates one from a TTF, and the data lives in the
*application* — the machinery is generic, the face is the app's, and a
GUI framework should not ship a font.

**Two further deferrals, and they are this decision's, not fontdue's:**

- **One size per baked face.** Other sizes scale advances linearly,
  which is not what a hinted rasteriser produces. A second size is a
  second bake, and costs flash rather than code.
- **No hinting.** The coverage is whatever the source rasteriser
  produced at bake time.

Complex-script shaping stays deferred and is now deferred *twice over*:
a baked face has no shaping engine at all.

**What is gated:** `fixtures/embedded/tests/baked_face.rs` re-rasterises
every glyph with a live `fontdue` and compares advance, extent,
bearings and **every coverage byte**. Generated data rots silently — a
bake at the wrong size, a hand-patched advance, or a fontdue version
bump all yield a face that still compiles and draws subtly wrong
glyphs. It skips, with the path printed, where the system font is
absent.

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
