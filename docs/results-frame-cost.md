# Results: frame cost and input latency of the live window

**Date:** 2026-09-03. **Machine:** i9-13900KF, governor `powersave`
(the reference machine in `justfile`). **Subject:** `examples/
counter-window`, 440×520, `truetype` feature, Noto Sans. Per-stage
numbers from `LUMEN_PROFILE=1 just tally-frame 12` (median of 50);
live numbers from `app.perf` over the last 128 frames after 140 agent
clicks (`scripts/ui_check.py latency`).

## The report that started it

"Response time from clicking a button to it reacting is very slow —
almost a second." Layout and paint were 1 µs. Nothing the agent could
see explained it: `app.perf` reported layout and paint only.

## Where the time went

| stage (dev build, before) | µs | after | notes |
|---|---:|---:|---|
| pump (layout + paint) | 1 | 1 | memoised; not the problem |
| clear | 3 411 | 96 | dev-profile fix |
| render: rounded rects | **184 388** | 201 | 26 cmds → 8; SDF shadow; span fills; AA corners |
| render: text | 595 | 20 | clip per glyph, not per pixel |
| blit RGBA → 0RGB | 1 863 | 69 | moved into the shell |
| **redraw** | **~190 000** | **~800** | |

Release-build figures for the same stages, before: render 5 263 µs;
after: 757 µs. The dev build is now the release build for this work.

Three causes, in order of size:

1. **The shadow was 18 translucent rounded rects.** Each was sampled
   per pixel against corner circles and blended through a
   bounds-checking `blend`. 97% of the frame. Now `Cmd::Shadow` — one
   command, one pass over the bounding box, alpha from the signed
   distance to the rounded rect with a quadratic falloff. Pixels more
   than 1 px inside the shape are skipped; what cast the shadow paints
   over them next.
2. **An unoptimised rasteriser.** `cargo run` is a dev build. With
   `opt-level = 3` on `lumen-static-shell` and `fontdue`, `clear` was
   still 800 µs, because `debug-assertions` also switch on std's
   `ub_checks` inside every slice operation the inner loops are made
   of. With both off for those two packages: 96 µs, equal to release.
   The core, the macros and the examples remain debug-built.
3. **Per-pixel work where per-row would do.** `fill_rounded_within`
   tested every pixel of a card against the corner circles; now the
   interior rows are spans (opaque spans are written, not blended) and
   only the four corner squares are sampled. Glyphs were clipped one
   pixel at a time; now once per glyph.

## Live, warm (128-frame window)

| | p50 | p95 | max |
|---|---:|---:|---:|
| frame (drain → presented) | 1.05 ms | 2.33 ms | 3.98 ms |
| render | 0.94 ms | 2.12 ms | 3.82 ms |
| present | 0.12 ms | 0.44 ms | 0.46 ms |
| **input → presented** | **2.22 ms** | **4.36 ms** | **7.63 ms** |

Budgets (`.claude/skills/verifying-apps/SKILL.md`): frame ≤ 16 ms,
input → presented ≤ 100 ms. **Verdict: pass, by 7× and 13× at p95.**

The first ~5 frames after launch render at 4.7 ms — cold clocks on a
`powersave` governor and first-touch of the pixel buffer. A latency
check reads the warm window: `ui_check.py latency` clicks first.

## What the round also found (fixed in the same commit)

- **Text 4 px low in every button.** `Button` never measured its
  label: width was `0.62 em × chars`, the text box `font_size` tall;
  glyphs are placed from the line top, so the run landed low. Now
  measured (memo keyed on limits, size, weight, label hash) and the
  line box is centred — then shifted so the *cap height* is centred
  (`text::cap_shift`), from where the layout actually puts an `H`, not
  from the face's metrics: the bitmap sits a pixel below the metric
  baseline (`tests/glyph_placement.rs`). Residual: +1 px, inside the
  ±2 px budget.
- **Hard-edged corners.** Coverage was a binary in-or-out per pixel.
  Now a 1 px ramp from the distance to the corner circle.
- **White on the accent blue: 3.7:1.** Under WCAG AA's 4.5:1 for
  16 px text. Accent darkened two steps: 5.0:1.
- **The audit measured with `BlockMetrics`** while the window laid out
  with Noto, reporting every button truncated. `Agent::set_text_measure`
  installs the host's measurer. `Label::probe` also measured at 16 px
  regular with no padding; `Button` had no `probe` and would have
  poisoned its new memo (the F2.1 scar).
- **`frame_done` was never called by the window**, so `ui.lastChange`,
  `ui.lint` and `app.perf` described nothing. Called after present.

## Not fixed, recorded

- **No press or hover feedback.** `NodeState` has no pressed/hovered
  bit and `dispatch` tracks none. A button that does not change while
  held is a GUI-theory failure (`ui_check.py` cannot check it yet).
  Planned under M9 routing in `docs/plan-architecture-v2.md`.
