# M50 — the software rasteriser allocates what its gate says it does not

> **T50.0, T50.2–T50.7 and T50.9 DONE 2026-09-27; M50 is closed.**
> **T50.0:** every finding re-checked and gated red (truetype arms 230 +
> 1,290 allocs over 50 frames; banded 3w + 1 and 5,375 with text; 2.27 /
> 3.63 shapings per run; a 4K grow reserving 66 MB for 33; a frosted
> sheet's 576 px change costing 21.63 ms through `Host`).
> **T50.2:** glyph cache + shaping scratch: all three truetype arms at 0/0;
> R8-F3's out-of-clip half closed; 1.2× on `render_within`, not Finding
> 15's 6×. **T50.3:** shaped once on the frame thread, 1.00 per run at
> 8 and 32 bands, byte-identical to pre-M50. **T50.4:** the pool's kill
> rule fired (two row copies 615 µs against a 216 µs frame); the budget
> ships — frame thread exactly 3w + 1, worker code 0. **T50.5:**
> `lumen-static-winit/simd` and `/threads`; each vector branch counted
> and I8 red on all three. **T50.6:** exact growth (33,186,240 for
> 33,186,240) and a 720p shrink returns the rest. **T50.7:** bold parsed
> on first use (a regular-only stack holds one face). **T50.9:** closed as
> measured, not built — the bounded rule is unsound for every material
> (8,530 px wrong); every sound region holds the whole panel.
> M51/T51.4 was done here too, on T50.2's scratch.
>
> **T50.1 and T50.8 DONE 2026-09-25 (early landings).** **T50.1:** the raster arm is
> `rasterising_a_block_glyph_frame_allocates_nothing`, and the handoff,
> `docs/results-allocation.md`, the book and the rustdoc say the
> truetype path allocates (re-measured at `0091266`: 25.80 raster +
> 4.60 pump allocations per frame, the review's figures unchanged).
> **T50.8:** `clip_test.rs` pins the
> clip stack for all nine drawing commands (glass twice) and for a
> shadow in a `ScrollView` on a full redraw: 0 px outside the clip on
> every row; `c472f80` turns the `Shadow` row and the `ScrollView` case
> red (3,356 px and 2,888 px).
>
> **Written 2026-09-24.** Closes R8-F1, R8-F2, R8-F6,
> R8-F7, R8-F8, R8-F10, R18-F4, R18-F10 and R6-F5 from M38
> (`docs/results-deep-review.md`), as re-triaged against `main` at
> 1717808 on 2026-09-24. Where the review and the re-triage differ, the
> re-triage is used, because the code moved after the review.
>
> **Phase 4** of the fix order. It comes after M43, because its gates
> run in M43's local CI runner and its size and symbol checks use M43's
> fixed static scripts. It also comes after M40, which changes
> `draw_run`'s far-edge rounding (R1-F8). This milestone rewrites
> `draw_run`, and it must keep M40's gate green. T50.1 and T50.8 may run
> earlier, and each says why.
>
> **The headline is one S1.** The handoff says a rasterised frame
> allocates nothing, and `docs/results-allocation.md` says the gate
> proves it. That gate draws block glyphs on `Canvas::new`. Every window
> draws truetype text on `Canvas::with_font`, and that path allocates on
> every frame (R8-F1). So the fix is a glyph cache and reusable shaping
> scratch, plus a truetype arm on the gate that can fail. R8-F2 here
> depends on that cache. So does the out-of-clip half of M45's R8-F3,
> which needs no task of its own once the cache exists (§4).

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and prints the measured quantity beside its bound.*

---

## 0. The rules for this milestone

1. **A zero is claimed only for the path the gate draws.** An arm's
   name says what it renders: block glyphs, truetype, banded or not.
   `docs/results-allocation.md` already states this lesson ("What a gate
   measures belongs in its own name") and then broke it. Every doc
   sentence this milestone corrects is corrected **in the task that
   fixes the code**, not in a closing pass.
2. **Zero, not a budget, unless §2.2's kill rule fires.** The one place
   this milestone may end with a budget is the threaded rasteriser. If
   it does, the budget is an exact count, printed beside the
   measurement, and recorded as a decision in
   `docs/results-allocation.md` and `docs/results-threading.md`.
3. **The setup is asserted before the quantity.** Every arm first
   asserts that the frame did what the arm is named for: that a run was
   shaped, that the canvas banded, that the vector path ran. Then it
   reads the count. Two of the findings here are a gate whose setup
   never happened: the threads arm's 320×200 fixture never bands
   (R18-F4), and nothing asserts the SIMD branch ran (R18-F10).

## 1. What is wrong, at 1717808

### 1.1 Truetype text allocates on every frame, and the gate draws block glyphs (R8-F1)

`FontStack::glyphs` (`crates/lumen-static-shell/src/text.rs:236`) calls
`place_run` and then `rasterize` per glyph. `place_run` builds a fontdue
`Layout` per run through `FontStack::run` (`text.rs:137`). It calls
`cap_shift` (`text.rs:164`), which builds a second `Layout` to find
where an `H` lands (`text.rs:382`). It collects a `Vec<Placed>`.
`glyphs` then collects a `Vec<Glyph>`, and `rasterize` (`text.rs:221`)
returns fontdue's coverage `Vec<u8>` for each glyph. fontdue allocates
twice per glyph (its accumulator and the bitmap). `draw_run`
(`crates/lumen-static-shell/src/software.rs:122`) runs all of this for
every visible run on every frame. On the layout side,
`FontMetrics::measure_weighted` calls `measure_run` (`text.rs:268`),
which builds one more `Layout` per measure.

The gate that is supposed to catch this is
`rasterising_a_frame_allocates_nothing`
(`crates/lumen-static-shell/tests/alloc_gate.rs:343`). It renders on
`Canvas::new` (`:348`), so it draws block glyphs. The handoff's "What
survives from §3" (`docs/00-handoff-static-tree.md:306-313`) says "the
gate covers a rasterised frame". `docs/results-allocation.md` §"The
blind spot this table had until 2026-09-19" says the same. Both are
false for every turnkey window (R8-F1; `docs/results-deep-review.md`
§2.2). The rustdoc on `FontStack::rasterize` (`text.rs:203-210`) calls
the per-frame raster "a known cost", but it concedes time, not
allocation.

**What the re-triage measured at 1717808.** The review's probe was
cherry-picked unchanged. The per-frame raster, per-event and
`pump_with(FontMetrics)` counts are identical to the review's. One
count moved: the 3000-row list with one row changed now pumps at
**0.00 allocations per frame (was 6.00)**, because M29 and M34
narrowed the pump. They did not change the raster, which still
allocates once per shaped run's worth of glyphs.

### 1.2 The banded rasteriser: an allocation gate that cannot see it, and runs shaped more than once (R18-F4, R8-F2)

`render_banded` (`software.rs:1487`) opens `std::thread::scope` and
`sc.spawn`s every worker on every banded frame (`software.rs:1561-1564`).
`std` allocates for each spawn and for the scope. The gate's counters
are thread-local (`alloc_gate.rs:23-44`), so a worker's allocations are
invisible to them. Its fixtures are 320×200, which never bands. `just
threads` does not run `alloc_gate` at all (`justfile`, recipe
`threads`). The justfile's comment says "Every path asserts **zero**",
and `docs/results-threading.md` §4 says per-thread state "must be
pre-allocated" to satisfy `alloc_gate`. Both are false for this path
(R18-F4; `docs/results-deep-review.md`). The re-triage re-ran the
review's probe at 1717808 and got the review's counts.

Separately, the text arm (`software.rs:1100-1125`) culls each run
against a one-em pad and then calls `draw_run`, which shapes the whole
run. When a run's padded reach straddles a strip boundary, every strip
it touches shapes and rasterises it again (R8-F2;
`docs/results-deep-review.md`). `docs/results-threading.md` §4 says
"Once strips stop re-shaping, 32 bands beats 8". T17.5 removed the
whole-list re-shape, not the straddle re-shape. The verifier lowered
this to S3: with a background fill, which every real window has, the
threaded frame is still faster than one thread.

### 1.3 The `simd` and `threads` features: not reachable from the window, and not provably compiled (R6-F5, R18-F10)

`lumen-static-winit`'s `[features]` has no `simd` and no `threads`
passthrough. Only `examples/counter-window` exposes them, through its
own shell dependency line. Cargo's feature unification makes both
reachable, so this is a configuration and documentation gap (R6-F5,
S3). But the benchmark manifest that calls itself "the default shape"
(`benches-competitive/arms/lumen-static-app/Cargo.toml:7`) is the row
the docs compare with Qt and GTK. No user-facing doc says how to turn
the fast raster path on. Whether `threads` should be on by default for
desktop has no recorded answer.

The vector code is chosen only by `#[cfg(feature = "simd")]`. There are
**three** such branches, not one: the whole-frame `0RGB` conversion
(`software.rs:1256`), `blit_run` (`software.rs:1888`) and `shadow_run`
(`software.rs:1962`). `tests/simd_shadow.rs` compares the shadow's
pixels with a scalar reference, and the scalar path produces those
exact pixels. So compiling the vector branch out cannot turn the test
red. Its only report of which path ran is `cfg!(feature = "simd")` in
a failure message (`simd_shadow.rs:136-137`). The review's injection,
which gates the branch on a target feature the default x86-64 target
lacks (scratch `deeea8b`), leaves `just simd` green (R18-F10). The
re-triage named only the shadow branch. The other two have the same
shape.

### 1.4 Memory the docs do not attribute (R8-F7, R8-F10)

`Canvas::resize` (`software.rs:1229-1237`) does `pixels.clear()` and
then `pixels.resize(w * h * 4, 255)`. Growth is `Vec`'s amortised
doubling, and a `Vec` never shrinks. So a one-pixel grow at 4K doubles
the reservation, and shrinking the window afterwards keeps it. No test
counts resize allocation or retention. The review's injection made
every resize reallocate, and every test stayed green (R8-F7). M37
moved the caller (`surface.rs` now returns whether the size changed)
but not the buffer policy.

`FontStack::system` (`text.rs:70-74`) parses the bold face eagerly. The
review measured fontdue's parsed faces as most of an idle window's
heap. `docs/results-competitive.md` §3 attributes the whole resident
gap to GPU contexts and says nothing about the font heap (R8-F10, S4).

### 1.5 A small change under a glass panel repaints the whole panel (R8-F6)

`glass_within` (`software.rs:284-305`) captures `glass_source(shape)`
whatever `bounds` it is given. On its own, that makes a small damage
rect cost the whole panel's blur. The verifier found it is not the
binding cause. Through the real `Host`, `dilate`
(`crates/lumen-static/src/retain.rs:543-566`) grows any damage that
touches a source region into the whole source before the renderer sees
it. So bounding the capture alone saves nothing (R8-F6, lowered to
S3). A damage-bounded blur is only correct for a material whose
`adapt` is zero. `Material::LIQUID`'s tint is a mean over the panel,
and `docs/results-glass.md` §0e records that reducing it over a
clipped region changes every pixel of the panel.

**One correction to the re-triage's fix.** It proposes widening damage
by "the kernel reach (3σ+4)". The region a panel reads is
`glass_source` (`crates/lumen-static/src/display.rs:160-183`). Its
reach also includes the bevel's refraction, scaled by the dispersion
spread. A reach of 3σ+4 would leave a refracting frosted panel's rim
stale, which is the exact hole that function's rustdoc says it closed.
The reach used here must be `glass_source`'s own expression.

### 1.6 The clip-stack pixel pin covers three commands (R8-F8)

The rustdoc on `clipped` (`software.rs:1600-1607`) says
`tests/clip_test.rs` "pins it at the pixel level". `ink_of`
(`clip_test.rs:104-122`) handles `Rect`, `RoundedRect` and
`StrokedRect`, and sends every other command to `unreachable!()`. With
the shadow arm passing its unclipped reach (scratch `c472f80`),
`clip_test` stays green, and only `damage_oracle` goes red, on three of
its ten cases. A shadow escaping a `ScrollView` clip on a **full**
redraw renders the same in both of the oracle's arms. No gate can see
that case (R8-F8).

## 2. Decisions

### 2.1 The glyph cache lives in the owner that already has `&mut`, not in `FontStack`

`FontStack` is shared. The canvas and the measurer each hold one, and
the banded rasteriser hands one `&FontStack` to every worker at once
(`software.rs:1487-1494`, and the `band.render` call below it). The
options for where the cache and the shaping scratch live:

- **(a) In `FontStack`, behind interior mutability.** The re-triage's
  first suggestion. A `RefCell` makes `FontStack` `!Sync`, and banding
  needs `Sync`. A `Mutex` makes every glyph lookup take a lock, on
  every worker. **Rejected.**
- **(b) In the `Canvas` and in `FontMetrics`, which already take
  `&mut self`.** **Recommended.** `Canvas::render` has `&mut self`, and
  so does `TextMeasure::measure_weighted`. Each owns its own scratch:
  one reusable `Layout`, one placement `Vec` and a cap-shift table per
  (face, px). The canvas also owns the coverage cache. `FontStack` stays
  immutable and `Sync`.
- **(c) One cache per banded worker.** Memory grows with the worker
  count, and each worker warms its own. **Rejected.**

**Banding with (b).** A worker cannot borrow the canvas's cache
mutably. So the frame thread shapes every visible run once, in a
pre-pass before the bands start, and rasterises any glyph the cache is
missing. The bands then read the cache and the placements through a
shared `&`. That pre-pass **is** R8-F2's fix. So T50.2 builds the cache
in a form T50.3 can share, and T50.3 is the shaping pass.

**The key is `GlyphId`**, unchanged. It already carries face, glyph
index and size bits, and its rustdoc (`text.rs:218-220`) says equal ids
give byte-identical bitmaps because there is no subpixel phase. The
re-triage's key adds a subpixel x. That is not needed today, and it
would be wrong to add one without the rasteriser producing one.

**Bounded, and a miss never drops a glyph.** The coverage store is one
arena `Vec<u8>` plus an index sized once. When a frame needs more than
the cap, the cache resets. A glyph that still does not fit is
rasterised straight from fontdue and drawn. That allocates, and it is
counted and reported. It is never skipped. This is the opposite of the
GPU atlas's give-up branch (R9-F1, M51), and deliberately so: a cache
that drops glyphs to keep a zero is the defect M51 is fixing.

**What (b) breaks.** Nothing public. `FontStack::glyphs` is
crate-private. `FontStack::place_run` is public and used by
`lumen-static-wgpu`, and it keeps its signature. A scratch-taking
variant is added beside it, which M51/T51.4 then uses for the GPU
path.

### 2.2 The threaded rasteriser: a persistent pool, or an exact budget

`std::thread::scope` allocates on each spawn, and the only thing a
spawn buys is the right to borrow the frame's `&DisplayList` and pixel
slice. A persistent pool cannot hold those borrows safely, because they
are not `'static`. `scripts/check_no_unsafe.sh` covers
`crates/lumen-static-shell/src`, so the usual lifetime-erasing pool is
not available. The options:

- **(A) A persistent pool with owned inputs.** Workers are created once,
  on the first banded frame. Each frame's inputs are copied into
  pool-owned buffers that are reused: the command list into a retained
  list with a capacity-keeping copy (derived `Clone::clone_from`
  reallocates, so this needs a hand-written copy), and the font as an
  `Arc`. Each worker renders into a band buffer it owns, and the frame
  thread copies the band rows back. The cost is two copies per banded
  frame, and a change to how `Canvas` stores a banded frame.
- **(B) Keep the scoped spawn, and state an exact budget.** The gate
  asserts exactly the counted spawn cost per banded frame, process-wide,
  printed beside the measurement. It still catches any allocation a
  worker adds. The review's injection that allocates per strip moves
  the process-wide count off the budget.
- **(C) A dependency such as rayon.** Its allocation behaviour on this
  path has not been measured, and it is a new dependency for a
  default-off feature. **Rejected** unless (A)'s probe shows both
  in-house options failing.

**Recommendation: build (A) as a probe first, with (B) as the kill
rule's answer.** The kill rule is in §5. If (A) needs `unsafe`, or its
copies cost more than the banded frame's own run-to-run spread at
1080p, T50.4 ships (B) and records the decision. Either way the gate is
process-wide and its setup asserts that the canvas banded.

### 2.3 `threads` stays default-off. `simd` and `threads` become window features

`lumen-static-winit` gains `simd` and `threads` features that forward
to the shell. **Neither is on by default.** The recorded answer for
full redraws on desktop is the GPU (`docs/results-gpu.md`), and the
banded path allocates until T50.4 lands. The default question is
reopened only after T50.4, and only with a measurement. The
`lumen-static-app` manifest stops calling itself the comparison row
for a full 4K CPU redraw, and says which features the fast path needs.

### 2.4 Proving the vector path ran: a counter, not a constant

The re-triage offers a `pub const` next to the branch, or a path
counter. A constant is its own piece of text with its own `cfg`. An
injection that changes only the branch's `cfg`, which is what I8 did,
leaves the constant true. **So each of the three vector branches
increments a counter, compiled only under a test-only feature that the
`simd` recipe turns on.** `simd_shadow.rs` and `wide_stores.rs` assert
that the counter for their branch is above zero after rendering, and
print it. The shipped `simd` build carries no counter.

### 2.5 Damage under glass: measure through the real host first

A reach-bounded damage rule for `adapt == 0` materials is a change to
`dilate`, which M51/T51.3 is also gating. It pays only if a real
interaction under a frosted panel costs a meaningful part of a frame.
The verifier measured the review's number with the host bypassed. So
T50.0 measures it through `Host`, and T50.9 is built only if §5's kill
rule allows it.

## 3. Tasks

### T50.0 — the gates first, red at 1717808

Each finding becomes a check that is red today, committed marked as
expected to fail. The commit body records the red output. Findings
whose defect is a *missing* check, where the property holds today, get
their red state from the review's injection. That is stated per
finding, so that nothing claims to be red when it is not.

- **R8-F1.** Three truetype arms in `alloc_gate.rs`, under
  `cfg(feature = "truetype")`, each on `Canvas::with_font` with
  `pump_with(FontMetrics)`: (i) `PlainApp` at 320×200 with 50 changing
  frames; (ii) the 3000-row list with one row changed, rendered through
  `render_within` over the damage regions; (iii) `TextInput` per event
  (`Text`, `Left`, `Backspace`, a paste). Each asserts
  `(allocs, frees) == (0, 0)` after two warm frames. Beside the count it
  prints the setup: glyph calls per frame and runs shaped per frame.
  **Red today** by the review's counts. `dev-shape` names
  `--features truetype` for this binary explicitly and asserts its test
  count is above zero, so that a feature split cannot compile the arms
  out silently. M43/R17-F1 found that shape twice.
- **R18-F4.** `crates/lumen-static-shell/tests/alloc_gate_threads.rs`,
  its own binary, `required-features = ["threads"]`, counting with an
  armed process-wide atomic. It renders a 1920×1080 list, asserts
  `bands_for(None) >= 2` first, and then asserts zero per frame. **Red
  today** at the review's 3w+1. Wired into `just threads`.
- **R8-F2.** A case in `tests/banded.rs`: runs that straddle strip
  boundaries at 1920×1080 and 3840×2160, with a shaping-call counter
  behind the same test-only feature §2.4 adds. It asserts shaping calls
  per visible run == 1.00 and prints the ratio. **Red today** at the
  review's ratios. The feature switches only a counter, never a path.
- **R18-F10.** No red state is possible before the counter exists. The
  commit body records I8 (scratch `deeea8b`) applied and `just simd`
  green. That is the finding's evidence, and T50.5 turns it into a
  gate.
- **R6-F5.** `cargo build -p minimal --features
  lumen-static-winit/threads` fails to resolve the feature. **Red
  today.** Recorded, since there is no test file to commit until the
  feature exists.
- **R8-F7.** A resize arm with the counting allocator: 4K, then one
  pixel wider; assert capacity == length, printing both. **Red today**
  (capacity doubles).
- **R8-F10.** No gate until §2's decision builds lazy bold (T50.7). The
  commit records the review's font-heap split as the baseline.
- **R8-F8.** `clip_test.rs` cases for `Shadow`, `Gradient`, `Line`,
  `Text` (block glyphs), `Image` and `Glass` inside a `PushClip`, plus a
  shadow inside a `ScrollView` clip on a full redraw. **Green today**:
  the arms do clip. The red state is scratch `c472f80`, which is
  recorded. This finding is a coverage gap, not a defect.
- **R8-F6.** A measurement, not a gate: a 24×24 change under a
  `FROSTED` panel, and one under a `LIQUID` panel, pumped through
  `Host`. It prints the damage the renderer receives and the frame
  time. At 1717808 it prints the whole source for both. §5's kill rule
  reads this number.

*What building found (2026-09-27).* Re-checked at `80ff4c1` (M40, M43,
M45 and M51/T51.3 landed): every finding still reproduces, at the
review's figures where it gave one.

- **The counters are not behind a test-only feature.** §2.4 puts the
  SIMD branch counters behind one, and this section put the banded
  shaping counter behind the same feature. But the truetype arms print
  runs shaped and glyphs drawn too, and a feature every gate binary has
  to name is a feature some binary forgets. `text::text_counters` is
  unconditional instead, like `glyphs_rasterised` before it (M45): one
  relaxed add per run or per blitted glyph, beside work that costs
  microseconds. Only the per-row SIMD counters (T50.5) keep the feature.
- **R8-F1:** arm (i) reads exactly the review's 230 pump + 1,290 raster
  allocations over 50 frames. Arm (ii) reads 0 pump (M29 and M34 narrowed
  it) and 7,800 raster, rasterising 57 glyphs a frame to draw 17 — the
  out-of-clip half of R8-F3 in one line. Arm (iii) had to be written as
  **cycles that leave the field as they found it** (`Text` then
  `Backspace`, `Left` then `Right`, a paste then five `Backspace`s): per
  event, an edit that lengthens the string allocates for the string,
  which is the application's data and not the frame, and the plan's
  per-event wording would have failed on that forever.
- **R18-F4:** 25.00 allocations per banded frame process-wide on block
  glyphs, every one on the frame thread — the spawns, `3w + 1` at eight
  bands — and 5,375 with the pinned face, where the workers shape.
- **R8-F2:** 2.27 shaping calls per visible run at 1080p (8 bands) and
  3.63 at 4K (32); 1.00 without `threads`, which is the accept direction.
- **R8-F6: the kill rule fires.** `benches --bin glass_damage` measures
  through `Host`; the table is in `docs/results-glass.md` §5a. A frosted
  toolbar costs 4.04 ms, 3% under the rule's 4.17, and a frosted sheet
  21.63. The rule names no panel size and a sheet is an ordinary panel,
  so T50.9 is built.

**Effort:** M. **Depends on:** M43 (T43.13's CI runner; T43.9's
zero-test-count check for R17-F1, which this reuses for the truetype
arms).

### T50.1 — truth first: the arm says "block glyphs", and the docs stop claiming truetype

**Closes:** nothing on its own. It makes R8-F1's claim true while the
fix is built, and R8-F1 closes at T50.2.

**Change:** rename `rasterising_a_frame_allocates_nothing` to name
block glyphs. Restrict the handoff's "What survives from §3" and
`docs/results-allocation.md`'s T26.13 table to block glyphs. Say in
both, in one sentence, that the truetype path allocates and that M50
owns the fix. `alloc_gate.rs`'s module doc says the same.

**Gate:** none new. The renamed arm stays green, and T50.0's truetype
arms stay marked as expected to fail.

**Injection:** not applicable, since nothing in the code changes.
`just static`'s doc checks must pass with the new wording.

**Docs in the same commit:** `docs/00-handoff-static-tree.md` ("What
survives from §3", and §10's "Zero allocation for the entire UI" gains
the same qualifier); `docs/results-allocation.md`; the `alloc_gate.rs`
module doc.

*What building found (2026-09-25).* Landed early, **before T50.0**,
which waits for M43. So "T50.0's truetype arms stay marked as expected
to fail" had no arms to refer to: none exist yet, and T50.0 still owns
them. The claim was re-measured instead of assumed, with a throwaway
probe (not committed) on the gate's own fixture: block glyphs 0/0 per
frame, truetype 4.60 pump + 25.80 raster per frame. That table is in
`docs/results-allocation.md`. The docs list above was also short: the
book (`index.md`, `topology.md`, `costs.md`) and `lumen-static`'s crate
rustdoc made the same unqualified claim and are qualified in the same
commit. The old arm name went on `scripts/doc-identifiers.allow` as a
recorded rename, since this plan and the history cite it.

**Effort:** S. **Depends on:** T50.0. **May run before Phase 4,**
because it only stops a false claim being made and has no code to
conflict with. It should land as soon as T50.0 does.

### T50.2 — a glyph cache and reusable shaping scratch: truetype frames allocate nothing

**Closes:** R8-F1.

**Change:** as §2.1 decides.

- The `Canvas` owns a coverage cache keyed by `GlyphId`: an arena
  `Vec<u8>`, an index sized once, a byte cap, and reset-on-full. A miss
  past the cap rasterises directly, draws the glyph, and counts it.
- One reusable `Layout` and placement `Vec` per owner. A cap-shift
  table per (face, px) replaces `cap_shift`'s second `Layout` on every
  run.
- `FontMetrics` owns its own `Layout` scratch for `measure_run`.
- `draw_run` blits from the cache. `FontStack::rasterize` keeps its
  signature for the GPU atlas.

This is the cache `docs/results-frame-and-types.md` Finding 15
promoted. Its time saving is recorded there, beside the allocation
result. M40's far-edge rounding in `draw_run` is kept, and its gate
stays green.

**Gate:** T50.0's three truetype arms go green at `(0, 0)` and print
glyph calls and runs shaped beside the bound. The block-glyph arm stays
green in the same binary, which is the accept direction.

**Injection:** (1) force every cache lookup to miss. Arm (i) goes red
at the review's per-frame count, by name. (2) Build a fresh `Layout`
per run in `draw_run`. Red at one allocation per run. (3) Remove
`FontMetrics`'s scratch. The `pump_with` half of arm (i) goes red.
Accept: after the warm frames, the cached arm reads 0 and the block
arm reads 0.

**Docs in the same commit:** the handoff's "What survives from §3" and
§10 are restated with the truetype arm. `docs/results-allocation.md`
gets truetype rows, before and after. `FontStack::rasterize`'s rustdoc
says allocation as well as time, and that the canvas caches.
`docs/results-frame-and-types.md` Finding 15 is marked built.
`alloc_gate.rs`'s module doc.

*What building found (2026-09-27).*

- **`tests/textarea_visible.rs` went vacuous under the cache**, which is
  the first failure shape again. It bounded glyphs *rasterised* per caret
  move, and a warm cache rasterises none: 0 against a bound of 660,
  whether the area painted one run per visible line or the whole 64 KB as
  one run, so M45/T45.11's own injection could no longer turn it red. It
  bounds glyphs **placed** now (`text::TextCounters::placed`): 180 per
  move, and 107,190 with T45.11 undone. A new case beside it pins the
  out-of-clip half of R8-F3 that §4 assigned here: a cold canvas draws a
  124-glyph run through a 40 px clip and rasterises 4 to draw 4 (62 with
  the cache lookup moved before the clip test).
- **The injections read different numbers from the plan's**, because the
  cache and the scratch are separate mechanisms now: forcing every lookup
  to miss is 15.20 allocations a frame on arm (i), not the review's
  25.80, which also counted the layouts; a fresh layout per run is 4.60
  per run, not one.
- **The time saving is 1.2×, and Finding 15's 6× was a bound computed
  from the wrong quantity**: its stub's coverage map was 8×12, so it
  removed most of the blend with the rasterisation. The table is in
  `docs/results-frame-and-types.md` Finding 15.
- **The grid's timed raster is block glyphs.** §5 asks `just grid` to
  print the uncached count; its timed runs render on `Canvas::new`, so a
  truetype canvas draws the same grid beside them, prints the cache's
  statistics and asserts `uncached` and `resets` at 0, without moving any
  published column.
- **The multi-face path still allocates** (`FontStack::prepare` collects
  faces and segments). It is a run no primary face covers, and
  `docs/results-allocation.md` names it among what the zero excludes.

**Effort:** L. **Depends on:** T50.1; M40/T40.2 (R1-F8's `draw_run`
change must land first); M43/T43.13's CI runner. **Unblocks:** T50.3;
M51/T51.4.

### T50.3 — shape each run once per frame, and let the bands only blit

**Closes:** R8-F2.

**Change:** a pre-pass on the frame thread, before the bands, walks the
visible `Text` commands with the existing one-em cull. It places each
run into the canvas's placement scratch and fills any cache misses.
Workers receive `&` to the placements and the cache, and blit the
glyphs whose boxes meet their rows. Without `threads`, the same
pre-pass runs and the single band blits. That keeps one code path, so
`scripts/check_banded.sh`'s cross-build checksum still compares like
with like.

**Gate:** T50.0's `banded.rs` case: shaping calls per visible run ==
1.00 at both sizes, printed beside the bound.
`scripts/check_banded.sh` stays byte-identical.

**Injection:** move shaping back into the band loop. The ratio returns
to the review's figure, red. Accept: with the worker count forced to 1
through the environment, the same case reads 1.00.

**Docs in the same commit:** `docs/results-threading.md` §4's sentence
("Once strips stop re-shaping") says what T17.5 removed and what this
task removed. The band comment in `Surface::render`. The `Surface`
rustdoc's "The threading itself is not built yet" (`software.rs:52`),
which has been stale since T17.4.

*What building found (2026-09-27).* As planned, with one addition: a
glyph the cache does not hold when a band reaches it — too large to keep,
or lost to a reset in a frame that needed more than the cache holds — is
rasterised by the band and drawn, allocating, rather than skipped; the
bands read the cache through `&` and cannot fill it. The injection that
shapes again inside the band reads 3.27 and 4.63 (the pre-pass's one
plus the straddle ratio), not the review's 2.27 and 3.63.

**Effort:** M. **Depends on:** T50.2.

### T50.4 — the threaded rasteriser: a gate that sees every thread, and a zero or a stated budget

**Closes:** R18-F4.

**Change:** §2.2. First a probe of (A) on a branch, measured against
the kill rule in §5. Then either (A) lands, or the scoped spawn stays
and the gate's bound becomes the exact budget, with the decision
written into `docs/results-allocation.md` and
`docs/results-threading.md`.

**Gate:** `alloc_gate_threads.rs` prints `bands=N, allocs/frame
process-wide X (bound B), frame-thread Y`. B is 0 under (A) and the
exact spawn count under (B). Its setup assertion, `bands >= 2`, comes
first. `just threads` runs it.

**Injection:** (1) under (A), put a per-frame `thread::scope` spawn
back. The gate reports the spawn count and fails. (2) The review's I2,
one `Vec` per strip in the worker, must fail through the process-wide
counter under (A) and under (B). That is the proof that worker
allocations are visible. (3) Force one band through the environment.
The setup assertion must fire. It must not pass at 0. Accept: at base,
the gate passes and prints the band count.

**Docs in the same commit:** the justfile's "Every path asserts
**zero**" comment says what the threaded arm asserts;
`docs/results-threading.md` (the pre-allocation sentence);
`docs/results-allocation.md` (the arm and, under (B), the decision);
`render_banded`'s rustdoc.

*What building found (2026-09-27).* **The kill rule fired, and (B)
ships.** The probe is `benches --bin banded_pool` rather than a pool on a
branch: (A)'s unavoidable work — copying the banded rows out to buffers
it owns and back — is 614.6 µs at 1080p, against a 215.7 µs scoped frame
whose spread is 58.8 and whose spawns are 95.6, so no pool built on it
could come within the spread (`docs/results-allocation.md`, "The threaded
rasteriser's budget"). And **"exactly the spawn count, process-wide" was
not exact**: `std`'s stack-overflow handler enters each new thread in a
`BTreeMap` whose node splits allocate on the spawned thread about one
run in three. So the gate splits the count three ways: the frame thread
exactly 3w + 1, a worker's own code 0 (marked through the test-only
`raster-counters` feature, which T50.5 was to add and this adds first),
and `std`'s remainder printed. Injection (1) belongs to (A) and does not
apply; an extra spawn per frame stands in for it (28 against 25).

**Effort:** M under (B), L under (A). **Depends on:** T50.3, because
the worker's body changes there.

### T50.5 — `simd` and `threads`: reachable from the window, and provably compiled

**Closes:** R6-F5, R18-F10.

**Change:** `lumen-static-winit` gains `simd` and `threads` features
that forward to the shell (§2.3). The three vector branches increment
per-branch counters under a test-only feature (§2.4), which the `simd`
recipe enables. A test in `crates/lumen-static-winit/tests` builds
with `--features threads` and asserts that the shell reports banding
compiled in. It prints the worker count `Canvas::bands_for` gives a 4K
canvas.

**Gate:** `simd_shadow.rs` prints `vector path compiled: true, invoked
N times` and asserts N > 0 under the feature. `wide_stores.rs` does the
same for the `0RGB` conversion and `blit_run`. The winit test asserts
banding is available under `threads` and not available in the default
build.

**Injection:** I8 (gate the shadow branch on a target feature the
default x86-64 target does not enable). `just simd` must fail with
"vector path not compiled" or `invoked 0`. The same edit on each of the
other two branches must fail its own test. Remove the winit
passthrough line, and the winit test fails to resolve. Accept: the
plain `--features simd` build passes, and the default winit build
reports banding off and passes its own default-arm assertion.

**Docs in the same commit:** the justfile's `simd` comment; the
`simd_shadow.rs` header; `lumen-static-winit`'s `Cargo.toml` feature
comments; the handoff's threading paragraph (how to turn it on, and
that it is default-off by decision); the `lumen-static-app` manifest's
"default shape" note; `docs/results-competitive.md`'s raster rows (say
which features the fast row used).

*What building found (2026-09-27).* T50.4 added `raster-counters`
first, for the worker marker, so this task only adds the vector counters
to it (`software::vector_path_counts`). A counter increments at the top
of each vector branch, per call rather than per lane chunk, which is
enough to say "compiled and invoked". I8 on each of the three branches
reads `invoked 0 times` and fails its own test by name.

**Effort:** S. **Depends on:** T50.4 for the handoff sentence about
the default. The rest is independent.

### T50.6 — the pixel buffer grows exactly and gives memory back

**Closes:** R8-F7.

**Change:** in `Canvas::resize`, grow with `reserve_exact` to the new
length. Shrink with `shrink_to` when the new length drops below half
the capacity. Otherwise keep the in-place path. softbuffer's own
power-of-two segment is outside this crate, and the rustdoc says so.

**Gate:** a resize arm, beside `alloc_gate` or in its own
counting-allocator binary. (1) 4K, then one pixel wider: capacity ==
length, one allocation. (2) Shrink to 1280×720: capacity ≤ 2 × length.
(3) A 40-step ±1 px drag at 800×600: allocations ≤ the growth steps
past capacity. Each check prints its capacity and length.

**Injection:** go back to `pixels.resize`. Check (1) reads double the
length, red. Remove the shrink. Check (2) keeps the 4K capacity, red.
Accept: a resize to the same size allocates 0.

**Docs in the same commit:** `Canvas::resize`'s rustdoc (the growth and
retention policy); `docs/results-allocation.md` (resize is outside the
steady state, and this arm is what says so).

*What building found (2026-09-27).* `reserve_exact` and `shrink_to` on
a cleared `Vec` both `realloc`, which copies the stale pixels and holds
two buffers at the peak; the reallocating paths drop the old buffer and
reserve afresh instead. The drag check reads 3 allocations against a
bound of 3.

**Effort:** S. **Depends on:** none.

### T50.7 — the font heap: attributed, and bold loaded when first used

**Closes:** R8-F10.

**Change:** `docs/results-competitive.md` §3 attributes the idle heap
to the parsed faces, citing the review's split, and keeps the GPU
paragraph. `FontStack` parses bold on the first run of weight ≥ 600.
The bytes are read at `system()` time, so a missing file is still
found at startup. Parsing is deferred through a once-cell, which keeps
`FontStack` `Sync` (§2.1).

**Gate:** with the counting allocator, build `FontStack::system()`,
render a regular-only frame, and assert that the peak heap is below one
face's heap plus a stated margin, printing both. Then render one bold
run and assert that the bold face loaded.

**Injection:** parse bold eagerly again. The peak lands on both faces'
heap, red. Accept: the bold assertion passes after a bold run, and a
system with no bold face never parses one.

**Docs in the same commit:** `docs/results-competitive.md` §3;
`FontStack::system`'s rustdoc.

*What building found (2026-09-27).* The gate uses the pinned face for
both weights, so it does not depend on the machine's fonts, and its
"stated margin" has to include the frame's glyph cache, which reserves
2 MiB on first use (T50.2): a first margin of the bold bytes plus 1 MiB
failed on the cache, not on bold. The bold bytes are dropped once parsed.
The idle-RSS table was not re-measured: it needs a window, and whether
a given window draws bold decides the saving.

**Effort:** S. **Depends on:** T50.2 (both change `FontStack`'s
internals, so this goes second).

### T50.8 — the clip stack is pinned for every command, not three

**Closes:** R8-F8.

**Change:** T50.0's `clip_test.rs` cases lose their expected-fail
marker. They are green today. `ink_of` loses `unreachable!()`: each
command kind has a case.

**Gate:** per command kind, render into a 200×120 canvas under
`PushClip([40, 40, 120, 80])`. Count non-background pixels outside the
clip and assert 0, printing the count beside the command's unclipped
reach area. One case puts a shadow inside a `ScrollView` clip on a full
redraw, which is the case no oracle can see.

**Injection:** scratch `c472f80` (the shadow arm passes its unclipped
reach). The `Shadow` case reports ink outside the clip, red by name.
Accept: the same command without the `PushClip` inks outside the rect,
which shows the setup draws something there.

**Docs in the same commit:** `clipped`'s rustdoc names which commands
`clip_test.rs` covers.

*What building found (2026-09-25).* Three things the task text did not
say. (1) **Glass cannot be pinned against the clear colour**: a blur of
a flat backdrop is that backdrop, so a glass command over nothing inks
nothing and its setup check (ink outside without the clip) read 0. The
glass rows draw striped rects *before* the clip, and every row counts
pixels that differ from a render of its backdrop alone. (2) **A shadow
inks only around its rect**, so a shadow covering the clip left nothing
inside it to prove the command drew; its rect sits inside the clip and
the ring crosses it. (3) `clipped`'s rustdoc was **detached**: it sat
on `TextRun`, above that struct's own comment, so rustdoc showed it on
the wrong item. It moved to `fn clipped` with the coverage list. The
injections went beyond `c472f80`: glass, line, gradient, image and
block-glyph text each over their unclipped rect or clip, each red on
its own row (commit body).

**Effort:** S. **Depends on:** none. **May run in Phase 2 or 3,**
because it is a test with no code change and no dependency on M43's
scripts. It uses only `cargo test`. T50.0 as a whole waits for M43, so
an early T50.8 carries T50.0's `clip_test.rs` cases itself, with
`c472f80`'s red run in its own commit body.

### T50.9 — damage under a frosted panel is bounded by what the panel reads, if §5 allows it

**Closes:** R8-F6 (built, or closed as "measured and not worth it" with
the number recorded).

**Change:** if the kill rule allows it, `dilate`'s rule for a dependency
whose material has `adapt == 0` becomes: damage ∪ (damage expanded by
`glass_source`'s reach), clipped to the source. It is no longer
damage ∪ the whole source. `glass_within` captures `bounds` expanded by
the same reach. A material with `adapt > 0` keeps whole-source damage.
The dependency table records the material's `adapt` beside the source
region.

**Gate:** in `damage_oracle` (or `glass_oracle`), a `FROSTED` arm with
a 24×24 change under the panel. Partial and full renders differ by 0
px, and the repainted pixel count is printed beside a bound of the
reach-expanded box's area. A `LIQUID` arm asserts that the damage
still equals the whole source.

**Injection:** (1) apply the bounded rule to `LIQUID`. The oracle
reports differing interior pixels, because the tint mean changed. (2)
Bound the capture by `bounds` without the reach. Ring pixels differ.
(3) Use a 3σ+4 reach on a refracting frosted panel. Rim pixels differ
(§1.5's correction). Accept: at base both arms pass, and the `FROSTED`
arm repaints less than the source.

**Docs in the same commit:** `docs/results-glass.md` §5 (the
damage-bounded variant, and why `adapt` forbids it); `dilate`'s and
`glass_within`'s rustdoc.

*What building found (2026-09-27): closed as measured, not built —
the rule is unsound.* The kill rule allowed it (T50.0: a frosted sheet,
21.63 ms). But a partial redraw clears its region and redraws every
command inside it, so a repainted panel pixel near the region's edge
reads — outside the region — last frame's finished pixels, which inside
the panel are the panel's own previous output. Damage ∪ (damage ⊕
reach) ∩ source leaves 8,530 px wrong on a 600×200 frosted panel, and ⊕
twice the reach 12,546; the whole source 0 (`tests/glass_damage_bound.rs`,
`docs/results-glass.md` §5a). Every sound region contains the whole
panel, whatever `adapt` is: §1.5's "only correct for a material whose
`adapt` is zero" was wrong too. Injections (2) and (3) above therefore
test a rule nobody should ship. What the test pins instead, both ways: a
region that stops 20 px inside the panel reads 4,799 px wrong (red), and
a "bounded" region replaced by the panel alone reads 0 (the second
assertion fires, saying the bound has become possible).

**Effort:** M. **Depends on:** T50.0's measurement and §5's kill rule;
**M51/T51.3**, which gates `dilate`'s `MAX_DEPS` fallback and rewrites
its rustdoc. This task edits the same function and must land after it,
with T51.3's gate green.

## 4. Boundaries of this plan

- **R8-F3** (text raster cost scales with a run's length) belongs to
  M45, which closes it in Phase 3 with part (a), per-line runs in
  `TextArea`, and does not wait for this milestone. Part (b), rasterising
  only glyphs that meet the clip, is not a separate task anywhere:
  T50.2's cache makes an out-of-clip glyph a one-time raster, and
  T50.3's blit skips glyphs outside the rows being drawn (M45 §4).
- **R9-F8** (the GPU frame path allocates) belongs to M51. Its
  framework-owned allocations include `place_run`'s `Vec`, which
  T50.2's scratch-taking variant removes. M51/T51.4 depends on T50.2.
- **R9-F3** (the `MAX_DEPS` fallback is unguarded) belongs to M51, as
  assigned, even though it is core code. T50.9 depends on it.
- **The GPU atlas's give-up** (R9-F1) is M51's. §2.1 takes the opposite
  policy for the software cache on purpose.
- **Not done:** a subpixel-positioned glyph cache, an LRU policy, or a
  default-on `threads`. Each would need a measurement this milestone
  does not take.
- **No finding is marked won't-fix.** R8-F6 may close as measured and
  declined, under §5's kill rule. That is a closure with a number, not
  a won't-fix.

## 5. Kill rules and risks

Written before any build.

- **T50.4, the pool (§2.2).** Kill (A) and ship (B) if the pool needs
  `unsafe` anywhere in `crates/lumen-static-shell/src`, or if its
  1080p banded frame is slower than the scoped version by more than the
  scoped version's own run-to-run spread, over 8 interleaved runs a
  side. Eight, not four, because four runs a side could not separate a
  real effect twice in M26's Phase D.
- **T50.9, glass damage.** Build it only if T50.0's through-the-host
  measurement shows a small change under a frosted panel costing more
  than a quarter of a 16.7 ms frame at 1080p. If it does not, close
  R8-F6 with the measurement in `docs/results-glass.md` §5, and do not
  build.
- **T50.2, the cache.** Not a kill rule, a scope rule: the cache ships
  whatever its time saving is, because the allocation claim needs it.
  The time saving is recorded against Finding 15's prediction. If the
  cache makes a frame slower, that is a defect in the cache.

Risks, and what would reveal each:

- **The cache's cap is too small for a real screen.** Then the
  steady-state arm stays green while a real window allocates. *Reveal:*
  T50.2's counter of uncached rasters. The grid fixture (`just grid`)
  and showcase's headless frame print it, and it must read 0 on both
  after warm-up.
- **M40 and T50.2 both edit `draw_run`.** *Reveal:* M40's far-edge
  residue gate is in the same `just test` run and must stay green.
- **The shaping pre-pass changes paint order.** *Reveal:*
  `scripts/check_banded.sh`'s cross-build checksum, and
  `damage_oracle`, which compares partial and full renders.
- **A truetype arm compiled to 0 tests** by feature unification, as
  `golden.rs` was (M43/R17-F1). *Reveal:* T50.0's test-count assertion
  in `dev-shape`.

## Appendix: finding → task

| ID | sev now | status | task |
|---|---|---|---|
| R8-F1 | S1 | CLOSED | T50.2 (claim corrected in T50.1) |
| R8-F2 | S3 | CLOSED | T50.3 |
| R18-F4 | S2 | CLOSED (budget, by decision) | T50.4 |
| R6-F5 | S3 | CLOSED | T50.5 |
| R18-F10 | S3 | CLOSED | T50.5 |
| R8-F7 | S3 | CLOSED | T50.6 |
| R8-F10 | S4 | CLOSED | T50.7 |
| R8-F8 | S3 | CLOSED | T50.8 |
| R8-F6 | S3 | CLOSED as measured | T50.9 (the bounded rule is unsound) |
