# Results: frame cost and input latency of the live window

**Date:** 2026-09-03. **Machine:** i9-13900KF, governor `powersave`
(the reference machine in `justfile`). **Subject:** `examples/
counter-window`, 440×520, `truetype` feature, Noto Sans. Per-stage
numbers from `FULGURITE_PROFILE=1 just tally-frame 12` (median of 50);
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
   `opt-level = 3` on `fulgurite-shell` and `fontdue`, `clear` was
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

## M8b — incremental raster (2026-09-04)

The round above made a *full* frame cheap. M8b stops drawing the parts
that did not change. Same machine and method.

| stage, one click on the tally | full frame (M8b in) | partial |
|---|---:|---:|
| clear | 96 µs | **4 µs** |
| render | 478 µs | **25 µs** |
| blit | 69 µs | **6 µs** |

Damage for a count change is the value label: `[175, 143.5, 265,
247.5]`, **9 360 px of 228 800** — 4% of the frame.

Live, warm (128 frames, 140 agent clicks):

| | before M8b | after | budget |
|---|---:|---:|---:|
| frame p50 | 1.05 ms | **0.084 ms** | — |
| frame p95 | 2.33 ms | **0.154 ms** | 0.2 ms (T8b.3) |
| render p95 | 2.12 ms | 0.089 ms | — |
| input → presented p50 | 2.22 ms | **0.838 ms** | — |
| input → presented p95 | 4.36 ms | **1.421 ms** | 3 ms (T8b.3) |
| damaged px p50 | 228 800 | **9 464** | — |

**Verdict: both T8b.3 gates pass.** A click frame is 15× cheaper than
before M8b and 30× cheaper than at the start of 2026-09-03.

### The shadow, a second time

Profiling the partial path found the shadow was **586 µs of an 811 µs
full render** — one command, 72%. `shadow_within` computed the signed
distance for every pixel in the shape's bounding box and then discarded
the ~83% that fell inside the card. The rounded rect *is* its
inset-by-`radius` rect dilated by `radius`, so every pixel in that inset
rect is at distance ≤ −radius and cannot contribute: it is skipped
structurally now, by advancing `x` past it, with no arithmetic at all.
Full render 811 → 478 µs. The lesson is the same one this document
opened with — the expensive command was found by measuring per command
class, not by reading the code.

**Still the largest command, 2026-09-09** (`docs/results-simd.md`). The
skip took it from 72% of a full render to about half, not to a tenth —
a claim that had drifted into `software.rs`'s comment and is corrected
there. Two later passes brought it to 180 µs of a 270 µs render, 67%,
and it remains the one worth attacking. **The measurement per command
class had a hole in it the whole time:** `just profile`'s full-render
breakdown reported `rect`, `rounded_rect` and `text` and omitted
`Shadow`, so its rows summed to 86 of 346 µs and the largest class was
invisible in exactly the tool this section credits for finding it.

### Correction, same day: `no damage, no frame` was wrong

T8b.4 said "a redraw whose diff is empty presents nothing", and the
first implementation returned before the present. **It emptied the
window.** `RedrawRequested` is not only ours: X11 sends it on *expose*
— when the window is mapped, uncovered or moved — and an expose means
the display server has lost the content, which the application's own
damage knows nothing about. Skipping the present left whatever was in
the shared-memory segment on screen, which was another window's pixels.

The fix separates two questions that the first version conflated:

- the **buffer** needs only the damage written into it, because at
  `age >= 1` it already holds the frame presented last, and at age 0 the
  damage is `All` by construction;
- the **present** must be whole, every time, because this redraw may be
  an expose and winit 0.30 does not say which kind it is.

So `blit_0rgb_within(damage)` then `present()`. What T8b.4 actually
saves is the raster, which is the expensive half: render p95 0.089 ms
against present p95 0.039 ms. Costs after the correction — frame p50
**0.084 ms**, p95 **0.154 ms**; input → presented p50 0.838 ms, p95
1.421 ms — so both T8b.3 gates still hold.

**Why no test caught it.** Every check read the *canvas*:
`ui.screenshot` returned the renderer's buffer, and `ui.lint`,
`ui.lastChange` and the whole of `scripts/ui_check.py` are downstream of
that. Nothing between the canvas and the display server — the blit, the
damage rect, the present, the buffer age — was observable at all. That
gap is now `ui.screenshot { source: "surface" }`, which reads the window
back through `softbuffer::Surface::fetch`, and `ui_check.py presented`,
which compares the two. Re-injecting the bug makes it fail at 83% of
sampled pixels; softbuffer implements read-back on X11 and Web only, so
elsewhere it reports "not checkable" rather than passing.

### What the numbers say about the design

`Incremental` costs a copy of the display list per frame and a walk over
it. Both are in the p50 above, and `shell/tests/alloc_gate.rs` asserts
the copy allocates **zero** bytes in steady state once its buffers have
grown — the same standard every other path is held to.

The oracle held: `shell/tests/damage_oracle.rs` renders the tally at
every count from −12 to +12 both ways and compares the full pixel
buffers, plus a buffer-age case where a two-frame-old surface is redrawn
from two frames of unioned damage. That test is why the retention is a
type parameter and not a cargo feature: both paths must be alive in one
process to compare them (`docs/plan-architecture-v2.md` T8b.0).

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

- ~~**No press or hover feedback.**~~ **Fixed 2026-09-04 (M9/C6).**
  `NodeState` carries `pressed`, `hovered` and `focused`; `dispatch`
  sets them; `record_painted` resolves `Style::pressed_dim`/`hover_dim`
  into the painted background. `scripts/ui_check.py press` checks it
  live. Left visible rather than deleted: this entry is why the check
  exists.
- **Damage is one rect, not a list of them.** Two changes at opposite
  corners damage everything between. The tally's worst case is a digit
  count changing, which re-centres the label and shifts the card:
  58 368 px, 25% of the frame, against a 9 464 px median. A rect list
  would fix it and is not worth it until a screen exists where it
  matters.

## Idle — what a window costs when nothing happens (M47/T47.8, 2026-09-26)

The figures four comments cite ("the idle frame numbers in this file",
`crates/fulgurite-winit/src/lib.rs`, `crates/fulgurite/src/event.rs`,
`crates/fulgurite/src/anim.rs`). Until M47 this file had none;
`scripts/check_idle_citations.sh`, in `just static`, now refuses a
comment that cites idle numbers from a file with no Idle heading.

**Counts, not clocks.** `scripts/idle_count.sh` (`just idle-count`)
launches `examples/tasks-demo` (release, `Boundaries`) and makes **one
read before and one after** 60 s with no RPC traffic — a read asks for a
frame, so a polled window is never idle (`docs/plan-timers-wake.md`
rule 1). Frames are the agent's frame counter less the first read's own
frame; wakes are the event-loop thread's context switches from `/proc`;
threads are the process's task list, sampled every 50 ms.

**Run on a private nested X server** (`Xephyr :47`). On the shared `:0`
display, with other windows opening and the pointer moving, the still
window drew 84 and 552 frames in two 60 s runs, all window-system events
(`Resized`, expose, pointer) — a real cost, but not the framework's idle
cost. On `:47` the same binary drew none.

| phase, 60 s | frames | loop-thread wakes | process context switches | threads alive / created |
|---|---:|---:|---:|---|
| still (nothing scheduled) | **0** | 3 | 9 | 4 / 0 |
| a 120 ms timer (500 periods) | **1000** | 2413 | 2916 | 5 / **0** |

- **Still is zero frames.** `about_to_wait` arms `Wait` when no deadline
  is pending (M47/T47.1), and nothing else asks.
- **A timer costs two frames a period**, not one: the deadline's frame
  ticks the timer and spawns its job; the job's result wakes the loop
  and a second frame applies it. 1000 frames for 500 periods is that
  exactly. Folding the two needs a timer that applies without a job, which
  `docs/plan-tasks.md` §2.4's one-path rule does not want.
- **No thread per firing** (M47/T47.3): the timer phase created no thread
  in 500 firings; the one pool worker (4 → 5 alive) was started by the
  subscription's first job before the phase began. Before T47.3 each
  firing was a `std::thread` (R7-F5).

### A pointer moving over nothing (M53/T53.4, 2026-09-27)

R7-F6: every `CursorMoved` asked for a redraw, the redraw ran a whole
pump and the surface presented all of itself. Since M53/T53.3 the pump is
skipped when nothing changed, and since T53.4 a redraw the window asked
for itself whose damage is `None` presents nothing.

`just motion-present` (`scripts/motion_present.sh`): 200 XTest motions
over a 40×16 px region inside `#blurb` in `examples/showcase`, release,
on a private Xephyr display; counts from the window's `app.presents`.

| per 200 motions | presents | redraws that presented nothing |
|---|---:|---:|
| before (T53.4's injection: always present), `Everything` | 199 | 0 |
| after, `Everything` | **0** | 199 |
| after, `Boundaries<256>` | **0** | 199 |

- **`Everything` presents nothing too**: it lays out and paints every
  frame, but retention compares each command's hash, so an unchanged
  frame's damage is `None` all the same. The present decision keys on
  damage, not on the pump's fast path.
- **The pump a motion costs** is the idle pump: 0.49–0.50 µs before
  T53.3, 0.06 µs after, headless (`docs/results-damage-precision.md`
  T53.3).
- **Accepting cases**, both strategies: crossing `#d_button_a`'s edge in
  12 motions presents exactly **1** frame (the hover shade; 12 with every
  frame presented); a window covered and uncovered by another presents
  (3 frames) and the surface read back from the server equals the canvas.

### While the panic screen is up

T47.1 arms no deadline while a panic is shown, because the timers' clock
is frozen and a deadline would only redraw the screen at the timer's
rate. Nothing the agent reports counts those frames (the journal is not
fed on them), so `scripts/panic_drive.py` counts the loop thread's
context switches over the 2 s the screen is up with a 120 ms timer on:

| build | loop-thread wakes in 2 s | bound |
|---|---:|---:|
| T47.8, both strategies | **0** | 4 |
| a deadline armed while failed (injection) | **786** | 4 |

The injection was worse than the ~16 the plan predicted: the frozen
remainder is re-armed from each frame's start, so a past deadline fires
again at once and the loop spins.

M53/T53.4 adds a per-motion row here.

## M44/T44.6 — an event writes the nodes it changed (2026-09-26)

`pass::dispatch` published hover, press and focus by writing every node
of the state array — the hover sweep on every move and press, the focus
sweep on every event, the press sweep on every release (R3-F12). T44.6
writes the node an index left and the node it reached, and publishes
focus after the event instead of before it.

**The gate is a count, not this clock** (M35's rule 3): one
`PointerMove` on a 1,728-node table writes **2** states against a bound
of 4, where it wrote 3,457 before
(`crates/fulgurite/tests/hover.rs`, `InputState::state_writes`).

**The clock, as a record.** `benches --bin invalidation`, rows
"dispatch, one PointerMove" (901 nodes, `Ten10`) and "(tiny)" (2 nodes),
min of 2,000 per run, eight runs a side under the bench lock, before at
`ad36bd7` and after with T44.6. **LOADED**: load average 4.9–7.4 while
other agents compiled.

| row | before (µs, 8 runs) | after (µs, 8 runs) |
|---|---|---|
| one move, 901 nodes | 1.18–1.90 | 0.03–0.06 |
| one move, 2 nodes | 0.02–0.03 | 0.02–0.03 |

The 901-node row now costs what the 2-node row does, within its spread:
the per-node term was the sweep. The plan named a 1,685-node arm; the
bench's existing table is 901 nodes and the count gate uses 1,728, so no
third table was added for the clock.
