# lumen-static against Qt, GTK, iced, Xilem, Slint and Lumen

**Measured 2026-09-05.** Every number here was run on this machine on
this day. Nothing is estimated, and no number comes from a single run.

> **Read §6 first if you read nothing else.** The benchmark's most
> valuable output was not a ranking. It found **two real defects in this
> framework**, worth 46× and 6.4× on two different frames, and both were
> invisible to every gate here because every gate here draws a handful
> of labels.

## Corrections, 2026-09-08 (M16/T16.4)

The performance review of 2026-09-08 re-ran this comparison across four
UI sizes and found three statements here to be wrong. They are corrected
in place below; this list is so a reader who remembers the old text can
find what moved.

1. **GTK's caveat gave the wrong reason.** It said GTK "lays out ~63
   rows at any N". Measured, nothing clamps — see §1.
2. **masonry's caveat named the wrong overhead.** It is not test-harness
   validation; it is an accessibility tree rebuilt every frame — see §1.
3. **§5's `Rebuild` row was not a configuration measurement.** It
   relabelled two numbers from the single `Incremental` arm.

Two things this review did *not* overturn, recorded because it is easy
to assume otherwise:

- **No number here came from the four differential Lumen arms**
  (`lumen-lean`, `-noa11y`, `-nodict`). Those binaries were invalid on
  disk — built in one cargo invocation, feature-unified, byte-identical
  — but they are cited nowhere in this file, so nothing published here
  was affected. `M16/T16.1` fixed them so they *can* be used; §4's
  clipboard delta was already measured the careful way and its note
  already says why.
- **Lumen's default-on ambient audit is not the asymmetry it looks
  like.** Its rows below are the audit-on build and lumen-static's are
  audit-off, which is not symmetric — but measured both ways the audit
  costs Lumen **1.6%** (85.1 vs 86.4 µs at 100 rows; 1 432 vs 1 456 at
  3 000). The gap is architectural, not configurational.

`docs/results-gpu.md` §5 also records a correction of its own from the
same review, about release profiles, which affects how its sizes may be
compared with §4 here.

---

## Method — inherited, not invented

The workload, the stopping points, the size and RSS definitions and the
fairness rules are **Lumen's**, from
the sibling Lumen checkout at `/mnt/Utvikling-linux/Lumen` — its
`benches-competitive/` harness and the report beside it dated
2026-08-29. Re-deriving them would have produced numbers that cannot be
compared with the ones that already exist, which is the opposite of the
point. Its caveats are inherited too, and marked where they apply.

**Machine.** i9-13900KF, 62 GB, X11, governor `powersave` — the same
machine every other `docs/results-*.md` number here was taken on.

**Frame workload.** N = 3000 rows in a flat column at 400 px wide, one
row's text replaced each iteration, **min of 200 iterations after 20
warm-ups**. Row heights are natural, not squashed into the viewport.
**No arm virtualizes**: `List` materializes every element, as `GtkBox`,
`QVBoxLayout` and iced's `column` do.

**App workload.** A window with a label and a button, shown, then idle —
byte-for-byte the same content in every arm, because a size comparison
between differently-shaped applications measures the shapes.

---

## 1. Single field update — one row of 3000 changes

The measurement a counter, a clock or a progress bar actually performs,
and the one where 2 999 of 3 000 rows hit whatever text cache the
framework has.

**Rust arms** stop at the matched point Lumen's harness defines — a
display list, iced's primitives, masonry's vello scene plus AccessKit
tree. Nothing is rasterised. Criterion, 20 samples.

| arm | 3000 rows | 100 rows |
|---|---:|---:|
| **lumen-static** (layout + display list) | **33.8 µs** | — |
| iced 0.14 | 340.0 µs | 16.5 µs |
| Lumen (memoized) | 724.0 µs | 67.2 µs |
| masonry 0.4 (Xilem's widget layer) | 887.6 µs | 32.8 µs |
| Lumen | 1 486.8 µs | 87.1 µs |
| Lumen + semantics | 1 872.1 µs | 100.6 µs |

**C/C++ arms**, which rasterise, so they belong in their own block:

| arm | change → layout | → +pixels | idle floor |
|---|---:|---:|---:|
| **lumen-static** | **18.7 µs** | **71.8 µs** (damage) / 215.2 µs (full) | **51.7 µs** |
| Qt 6.4.2 | 304.2 µs | 564.0 µs | 251.2 µs |
| GTK 4.14.5 | 111.5 µs | — | — |

**lumen-static is 10× faster than iced and 44× faster than Lumen** at
the matched stopping point, and 16× faster than Qt on the layout half.

The architectural reason is not subtle and is worth stating plainly:
**every other arm rebuilds or diffs a view tree for a one-row change,
and lumen-static has no view tree to rebuild.** The topology is in the
type; a text change is a `Dirty` flag on one node and a memo miss on one
label. That is what the whole design is *for*, and this is the row that
shows it.

Four caveats, or the table flatters us:

- **masonry's number is an upper bound, and the overhead is not the one
  named here until 2026-09-08.** The caveat said `masonry_testing::
  TestHarness` "may carry validation overhead". In a release build that
  validation is `debug_assertions`-gated and off, and its tracing spans
  are once per pass rather than per widget.

  What the test harness actually adds is **`WindowEvent::
  EnableAccessTree`, sent unconditionally at construction** — so masonry
  is charged for an accessibility tree rebuilt every frame that
  lumen-static and plain Lumen are not, and only `Lumen + semantics` is.
  A probe driving masonry through the public `RenderRoot` instead
  produces 203 AccessKit nodes at 100 rows with it enabled and 0
  without, while shaping all 100 rows either way. It *can* be driven
  without the test harness, in about fifty lines
  (`benches-competitive/probes/masonry-raw`), which is what makes the
  difference measurable rather than merely arguable.
- **GTK's number is a floor — but not for the reason this file gave
  until 2026-09-08.** It said GTK's box "clamps to the window, so it
  lays out ~63 rows at any N". It does not. `gtk_widget_measure(box,
  VERTICAL, 400)` returns exactly 16 px × N — 1 600 / 16 000 / 48 000 /
  160 000 at N = 100 / 1 000 / 3 000 / 10 000 — and the arm's timing
  scales 9.0 → 375.9 µs across that sweep. Nothing is clamped. The
  sibling report's original wording was about *rasterisation*, and it
  was restated here as layout, which is not true of the layout number
  being quoted.

  It is still a floor, for the real reason: **GTK caches per-widget size
  requests**, so a one-row change re-measures one label and reuses 2 999
  cached ones, and **no paint happens at all** — `gtk_widget_snapshot`
  returns NULL outside a live frame-clock pass, and the harness asserts
  rather than quietly timing nothing.
- **Qt's four numbers are min-of-200 over separate loops** and cannot be
  subtracted from one another. Its idle floor is quoted so a reader does
  not read `natural update+paint` as the marginal cost of the change.
- **`+render (full)` is the honest comparison with Qt**, which is not
  being allowed a damage rect. The damage number is what lumen-static
  ships and is reported beside it, not instead of it.

## 2. Full frame — every row of 3000 changes

The measurement that denies every framework its text cache: no arm can
serve 3 000 new strings from one.

| arm | 3000 rows | 100 rows |
|---|---:|---:|
| **lumen-static** | **1 392 µs** | — |
| Lumen | 15 924 µs | 1 586 µs |
| iced 0.14 | 18 047 µs | 590 µs |

**11.4× faster than Lumen and 13.0× faster than iced.**

That margin is *not* the static tree — when every string must be shaped,
nobody's tree matters. It is that §6.2 removed a rasterisation from the
measurement path. Before that fix this row read **8 736 µs** and
lumen-static was 1.8× ahead instead of 11×; the framework has been
paying for it since the shell gained real glyphs.

At 100 rows iced is faster than Lumen (590 µs vs 1 586 µs) and the order
flips by 3 000. Small-N and large-N are different questions and this
table answers both rather than picking the flattering one.

## 3. Idle RSS — a window with a label and a button

Median of 3 launches, `VmRSS` 4 s after launch.

| arm | RSS |
|---|---:|
| **lumen-static (lean)** | **24.3 MB** |
| **lumen-static** | **24.6 MB** |
| Qt 6.4.2 | 46.4 MB |
| Slint | 68.5 MB |
| GTK 4.14.5 | 136.9 MB |
| Lumen | 169.6 MB |
| iced | 196.3 MB |
| Xilem | 321.1 MB |

**Re-measured after M16, 2026-09-08:** default **25.75 MB**, lean
**25.59 MB** — within the drift of the 2026-09-05 figures above and
still the smallest of the eight. The `gpu` feature is its own row
(`docs/results-gpu.md` §5): 197.8 MB with an adapter open, 26.58 MB in
the same binary forced to software.

**lumen-static is the smallest resident of the eight, by 1.9× over the
next one.** The gap over the GPU-backed arms (iced, Xilem, Lumen) is
mostly a GPU context: a wgpu device and its driver-side allocations are
tens of megabytes before a frame is drawn. That is a real cost a user
pays, and it is also not a fair measure of *framework* overhead — both
statements are true and the table means less without the second.

## 4. Distributable size

Executable, plus the unique shared objects `ldd` resolves for it,
excluding the C runtime every binary on the system already has.

| arm | exe | shared libs |
|---|---:|---|
| **lumen-static (lean)** | **2.5 MB** | 0.2 MB in 1 |
| **lumen-static** | **2.7 MB** | 0.2 MB in 1 |
| iced | 10.7 MB | 0.2 MB in 1 |
| Xilem | 10.8 MB | 0.2 MB in 1 |
| Slint | 14.8 MB | 2.0 MB in 9 |
| Lumen | 16.8 MB | 28.0 MB in 66 |
| GTK 4.14.5 | 0.0 MB | 32.1 MB in 65 |
| Qt 6.4.2 | 0.0 MB | 70.1 MB in 45 |

**Re-measured after M16, 2026-09-08:** default **2.71 MB**, lean
**2.56 MB**, and the `gpu` feature **6.58 MB** — all three built one
package per `cargo` invocation under this workspace's release profile,
which is the only way these are comparable (M16/T16.1, T16.5). M16 added
about 500 bytes to the default binary.

**4× smaller than the next self-contained arm**, and the whole
distributable is smaller than the *delta* between any two of the others.
Qt and GTK ship a near-empty executable and link tens of megabytes of
system libraries — which is the right answer on a distribution that
already has them and the wrong one anywhere else, exactly as Lumen's
BENCH3 wrote it.

**The clipboard costs 159 728 bytes** (2 827 184 → 2 667 456), which is
the `arboard` stack from T14.5. Building both arms in *one* `cargo`
invocation reported them byte-identical — cargo unifies features across
a multi-package selection, which is the trap this repository's own
justfile documents, and it hid the number until each was built alone.

And the number that has no column here, because no other framework has
one: the same core, `no_std` with no allocator, links a **22 458-byte
`.text`** firmware for `thumbv7em-none-eabihf`
(`docs/results-embedded.md`).

## 5. lumen-static configurations

| configuration | effect |
|---|---|
| default | the rows above |
| `--no-default-features` on the windowing crate | −156 KB binary, −0.3 MB RSS |
| `observe` (the dev surface) | **+64 µs a frame** at 3000 nodes: `+paint` 33.8 → 97.6 µs |
| `Rebuild` retention | **not a configuration measurement** — see below |
| `no_std`, no allocator | not on this table: it has no window. 22 458 B of `.text` at 2026-09-10 (`docs/results-embedded.md`) |

**Why the `Rebuild` row cannot say what it used to.** It quoted 215.2 µs
against 71.8 µs and called damage 3.0×. Both numbers came from the
*same* `Incremental` arm — they are its `+render (full)` and `+render
(damage)` rows relabelled — so the line described a rendering choice
within one build, not two configurations of the framework.

There is no build flag that separates the two, and by design there
cannot be: retention is a type parameter with a default, deliberately
kept out of the feature system so `Rebuild` stays available in the same
binary as the oracle every incremental gate is compared against
(M8b/T8b.0). A real comparison needs one process running both, which is
what `benches-competitive/arms/stages` now does — it reports every stage
under `Rebuild` and `Incremental` side by side, and that is where the
question belongs.

**`observe` costs nothing until it runs.** Layout, paint and render are
within noise of the default build ; the whole cost is the per-frame journal and ambient audit.
That is worth stating because the first measurement here said `observe`
made layout 46% slower — and that was a cold run, caught by repeating it.
**No number in this file comes from a single run.**

---

## 5b. Four UI sizes, and what M16 changed (2026-09-08)

Every number above was taken at one tree size. Resolution was swept in
`docs/results-raster-scaling.md`; **tree size never was**, and small N
is where a static tree's advantage should be smallest because fixed
per-frame costs dominate. It is not: the margin over iced is *wider* at
100 rows than at 3 000.

Re-run after M16 (`docs/plan-invalidation.md`) on the same machine the
same day. **The cross-arm rows below are the ones M16 did not touch**,
which is the point — see "what M16 changed" at the end.

### One row of N changes — the matched stopping point, no raster

| arm | 100 | 1 000 | 3 000 | 10 000 |
|---|---:|---:|---:|---:|
| **lumen-static** (layout + display list) | **1.9** | **13.0** | **37.4** | **129.0** |
| GTK 4.14.5 (measure only) | 8.9 | 40.7 | 126.5 | 374.0 |
| iced 0.14 | 15.6 | 98.8 | 287.9 | 920.9 |
| Qt 6.4.2 (natural update) | 23.4 | 99.9 | 300.6 | — |
| masonry 0.4 | 32.7 | 283.0 | 889.8 | — |
| egui 0.29 | 37.2 | 261.1 | 779.1 | — |
| Lumen (audit off) | 84.7 | 498.4 | 1 400.5 | — |

At 3 000 rows: **7.7× iced, 8.0× Qt, 20.8× egui, 23.8× masonry, 37.4×
Lumen.** GTK measures and never paints, so it belongs against
lumen-static's `layout` alone — 1.4 / 7.3 / 20.2 / 66.2 µs — a **6.3×**
margin rather than the 3.4× the raw columns suggest.

The 10 000-row column has four gaps: masonry, egui and Lumen have
compile-time size arrays that stop at 3 000, and Qt's harness was not
driven past it.

### Every row of N changes

| arm | 100 | 1 000 | 3 000 |
|---|---:|---:|---:|
| **lumen-static** | **43.7** | **484.6** | **1 478** |
| Lumen | 1 575 | 5 923 | 16 124 |
| iced 0.14 | 585 | 5 921 | 17 767 |

**10.9× Lumen and 12.0× iced at 3 000.** At 100 rows the margin over
Lumen is 36×, collapsing as shaping comes to dominate every arm
equally. Reporting only one end of that would misdescribe the other.

Four arms have no all-rows workload — masonry, egui, Qt and GTK have no
churn group — so this table cannot be widened without new harness code.
Slint still has no frame arm at all.

### What M16 changed, and what it deliberately did not

**The cross-arm rows above are unchanged.** That is the correct outcome
and it was checked rather than assumed: M16's two optimisations are
`Boundaries` invalidation, which is opt-in and off by default, and paint
culling, which requires a host to declare a surface. Neither is on in
the arm, because an arm that culls cannot be compared with iced or
Lumen, which do not.

**And neither does M17's SIMD** (2026-09-09, `docs/results-simd.md`).
Every row in this section is 1.00× within noise between the scalar and
`simd` builds at all four sizes, because the matched stopping point is a
display list and there is no rasterisation in it to vectorise. M17 moved
`+render`, which is not a cross-arm row.

**The relayout boundary costs nothing when unused**, measured directly
rather than inferred from a cross-day comparison: the generated check
compiled in gives 65.6 µs of layout at 10 000 rows, compiled out 65.8,
restored 65.8. Within noise, in the noise's own direction.

What M16 *did* buy, in configurations an application opts into:

| | before | after |
|---|---:|---:|
| layout, one subtree marked (901 nodes) | 5.29 µs | **0.53 µs — 10×** |
| layout, still frame | 5.29 µs | **0.01 µs — ~450×** |
| paint stage at 10 000 rows (culled) | 62.8 µs | **7.5 µs — 8.4×** |
| commands emitted for an 800 px viewport | 10 002 | **39** |

The culled paint figure is reported by the arm as its own row and must
not be put in the cross-arm table: forty rows of work against ten
thousand is not a comparison.

### Per-stage, one row of N changes

`benches-competitive/arms/stages`, `Incremental` retention, default
`Everything` invalidation, min of 5 runs of min-of-200. The summed
stages and an independently timed whole frame agree to under 1%, which
is what makes the decomposition usable.

| stage | 100 | 1 000 | 3 000 | 10 000 |
|---|---:|---:|---:|---:|
| dispatch | 0.03 | 0.03 | 0.03 | 0.04 |
| update (bindings) | 0.16 | 1.57 | 4.85 | 17.01 |
| layout | 1.07 | 6.23 | 17.65 | 57.90 |
| place | 0.05 | 0.71 | 2.27 | 7.64 |
| **paint** | **1.30** | **12.84** | **38.49** | **127.93** |
| damage | 0.01 | 0.01 | 0.01 | 0.01 |
| raster (software) | 4.88 | 7.32 | 12.27 | 29.67 |
| blit | 0.96 | 0.97 | 0.97 | 0.98 |
| **sum** | **8.51** | **29.88** | **76.74** | **242.22** |
| frame (independent) | 8.73 | 30.43 | 77.08 | 242.51 |
| raster (GPU, alternative) | 33.01 | 34.61 | 41.08 | 88.66 |

**Paint is the largest stage of a small-update frame** and the damaged
region is 2 002 px in every cell — identical rasteriser work — so the
growth is the walk, not the pixels. That is what culling addresses and
what the `Everything` column above still pays.

For the full-redraw workload the bottleneck is a different one
entirely: at 10 000 rows layout is **4 911 µs of a 5 292 µs frame, 93%**,
and that is text shaping, because every row's string genuinely changed.
The two workloads need different fixes and no single number describes
both.

An idle frame — nothing changed, damage `None` — costs 211 µs at 10 000
rows under `Everything`, of which raster and blit are 0.01 each. Under
`Boundaries` that frame is the 0.01 µs row in the table above.

Reproduce: `benches-competitive/competitors.sh`, `just stages`,
`benches --bin invalidation`.

## 6. What the benchmark found

Two defects, both real, both fixed, neither visible to any existing
gate — because every gate here draws a handful of labels and both only
appear at hundreds.

### 6.1 Text was shaped even when culled — 46×

`Canvas::render_impl` tested every command against the damage rect
before drawing it, **except text**, which was shaped and rasterised
whatever the rect said; only its individual *glyphs* were clipped
afterwards. On a 3 000-row list that is 3 000 shaping calls a frame for
the ~40 rows an 800 px viewport can show.

| | before | after |
|---|---:|---:|
| `+render (full)` | 7 315.8 µs | **158.6 µs** |
| `+render (damage)` | 7 304.0 µs | **73.6 µs** |

Damage had been buying **nothing** on a text-heavy frame, because the
expensive half ignored it.

### 6.2 Measuring text rasterised it — 6.4×

`FontMetrics::measure` called `FontStack::glyphs` and kept only its
`Size` — discarding a `Vec` of freshly rasterised coverage bitmaps.
fontdue has no glyph cache, so **every layout pass rasterised every
glyph it then threw away**.

| | before | after |
|---|---:|---:|
| all-rows `layout` | 8 228.8 µs | **1 299.6 µs** |
| all-rows `+paint` | 8 736.0 µs | **1 392.3 µs** |
| one-row `layout` | 23.0 µs | **18.7 µs** |

`measure_run` does the same width and height arithmetic — lifted
verbatim, so the two cannot disagree about a box — without the raster.

### 6.3 And one that was not a defect

The first `observe` measurement read **46% slower on layout**, which
would have been a striking finding about the dev surface. It was a cold
run. Repeating it five times showed `observe` within noise of the
default build on every stage, its whole cost being the per-frame journal
and audit. **That is why nothing in this file comes from a single run.**

---

## 7. Re-measured 2026-09-22 — and the arm was lying

Run because M29 re-opened the question and five milestones (M25–M30)
had landed since §5b's sweep of 2026-09-08. Same machine, same
governor, same harness, `./competitors.sh`.

### The finding is about the harness, not the frameworks

The first run said lumen-static had regressed **22% on paint** since
2026-09-05. It had not. `refactor(M30): one crate owns the widgets`
(`6052492`) added `features = ["observe"]` to the *unconditional*
dependency line of `benches-competitive/arms/frame/Cargo.toml`, so the
**plain** arm was compiled with the widgets' observability methods in —
the one thing that arm exists to be free of, given there is a second
arm beside it for exactly that comparison.

Bisected, with the arm rebuilt at each point:

| commit | layout | +paint | idle floor |
|---|---:|---:|---:|
| `aed779d` — the commit before M30 | 14.7 µs | 33.1 µs | 49.4 µs |
| `6052492` — the widget move | 21.1 µs | 46.6 µs | 63.6 µs |
| with the feature moved to where it belongs | **14.5 µs** | **32.3 µs** | **49.1 µs** |

**Two wrong explanations were measured and discarded before the right
one**, which is the only reason this is in the file rather than a
guess. First: `Style` grew 72 → 80 B and `NodeState` 76 → 84 B in
M27/T27.1, and paint streams one `NodeState` per node, so cache
pressure was the obvious cause. Adding a further **16 B** of padding to
`NodeState` and re-running moved nothing — 41.4 against 41.2 µs. Per-node
bytes are not this frame's cost. Second: a crate boundary blocking
inlining; the arm builds with `lto = true, codegen-units = 1`, and the
fix had nothing to do with the boundary.

**What the observability feature costs when merely compiled in** — no
agent attached, no journal running, nothing asking for a role:

| arm at 3 000 rows | +paint |
|---|---:|
| widgets built without `observe` | **32.3 µs** |
| widgets built with `observe` | 40.2 µs (**+24%**) |
| …and the journal actually running | 112.5 µs |

The feature adds `probe` and `observe` methods to every widget's `Node`
impl. The hypothesis first written here was that `probe`'s body calls
`layout` and so gives it a second call site. **That was the right kind
of cause and the wrong source**, as the attribution below shows. An
application that enables `observe` so an agent *can* attach pays this on
every frame, whether or not an agent ever attaches. The gating policy in
`docs/plan-capabilities.md` is written about runtime cost; this is a
compile-time inclusion cost, which that policy does not cover.

#### Attributed, 2026-09-23 — a vtable, not `probe`

Three builds of the frame arm at 3 000 rows, run interleaved and
**unstripped**, so the symbol table can say what the optimiser did:

| build | layout (µs) | +paint (µs) | `Label::layout` as its own symbol |
|---|---:|---:|---|
| plain | 13.5–14.3 | 30.7–33.1 | no — inlined into `List::layout` |
| `lumen-static-widgets/observe`, no agent | 17.3–19.4 | 38.3–41.1 | **yes**, 0x21b B; `Label::paint` too, 0x118 B |
| the same, with `List::observe`'s one `emit_subtree` call removed | 13.8–14.7 | 31.0–33.0 | no |

Ranges are min–max. The plain and observe rows are over 14 runs each
(8, then 6 beside the third build); the third build is over 6.

**The mechanism.** `List::observe` hands each item to
`observe::emit_subtree`, which walks it as a `&dyn Node`. The coercion
builds `Label`'s vtable, and the vtable holds the addresses of `layout`
and `paint`. Once those addresses are taken, both functions need
standalone copies, and LLVM stops inlining them into `List`'s per-row
loops. `probe` is not involved: `Label` overrides it, and its override
does not call `layout`. Removing that single call site restores the
plain build's frame and the inlining together. `perf` is unavailable on
this machine (`perf_event_paranoid` = 4), so the symbol table stood in
for a profile.

**What this reaches, and what it does not.** The cost is per *row
widget type inside a runtime container*: `List` and `Repeat` reach their
statically typed items through `emit_subtree`. (`Dyn`'s content is
already a `dyn Node` in every build.) A widget on the
derived table already has a vtable in every build: a non-`fuse` derived
container coerces each child to `&dyn Node` in its layout and paint
bodies (the derive's `call_child`), and a leaf's `node(0)` returns
`self` as one, which dispatch reaches in every build. *Corrected
2026-09-23: this first said the derive's `node(i)` does the coercing.
It is a static call; a review found the error.* So the 24% is the frame arm's shape (3 000
`Label`s in one `List`), and the whole-application cost is **not
measured**. `docs/plan-observe-cost.md` owns the fix and the measurement
that scopes it.

### One row of N changes — the matched stopping point, no raster

| arm | 100 | 1 000 | 3 000 |
|---|---:|---:|---:|
| **lumen-static** (layout + display list) | **1.7** | **11.2** | **32.3** |
| GTK 4.14.5 (measure only) | 8.8 | 41.0 | 112.2 |
| iced 0.14 | 15.6 | — | 290.1 |
| Qt 6.4.2 (natural update) | 23.6 | 101.9 | 304.8 |
| masonry 0.4 (Xilem's widget layer) | 32.9 | — | 896.3 |
| egui | 36.7 | — | 812.3 |
| Lumen (memoized) | 66.7 | — | 734.6 |
| Lumen | 87.5 | — | 1 447.4 |
| Lumen + semantics | 101.4 | — | 1 737.3 |

At 3 000 rows: **9.0× iced, 9.4× Qt, 25.2× egui, 27.8× masonry, 44.8×
Lumen.** GTK measures and never paints, so it belongs against
lumen-static's `layout` alone — 1.2 / 5.3 / 14.5 µs — a **7.7×** gap at
3 000.

lumen-static's own stages, for reference: idle floor 1.6 / 16.5 / 49.1
µs; `+render (damage)` 13.4 / 32.1 / 73.5; `+render (full)` 189.2 /
200.5 / 229.4. A full-frame churn at 3 000 rows is 1 364 µs of layout
and 1 477 with paint — every row's text replaced, which is the workload
§2 describes.

### Full frame — every row of N changes

The workload that denies every framework its text cache. **Two arms
cannot answer it**: `vs_toolkits` has no churn group for masonry and
neither does `vs_egui`, so Xilem and egui are absent here rather than
quietly folded into the point table's ranking.

| arm | 100 | 3 000 |
|---|---:|---:|
| **lumen-static** (layout) | **42.7** | **1 333.7** |
| **lumen-static** (layout + display list) | **46.1** | **1 446.8** |
| GTK 4.14.5 (set + measure) | 459.8 | 14 311.6 |
| iced 0.14 | 585.2 | 17 608.0 |
| Qt 6.4.2 (set + layout) | 688.2 | 20 472.8 |
| Qt 6.4.2 (set + layout + paint) | 968.9 | 20 848.2 |
| Lumen | 1 584.1 | 15 932.0 |

At 3 000 rows: **12.2× iced, 14.4× Qt, 11.0× Lumen**, and 10.7× GTK
compared layout-to-layout, since GTK's arm measures and never paints.

**The margin is a third of what the point workload shows, and that is
the honest reading of both.** On a one-row change lumen-static is 9–45×
ahead because nobody else has a tree that can decline to rebuild itself.
When all 3 000 strings are new, everybody shapes 3 000 strings and no
tree architecture helps — what is left is the text path and the
rasteriser. A framework can lead one of these axes and be ordinary on
the other; quoting only the flattering one is the easiest way to mislead
with true numbers.

GTK's bench5 arm also reports a sustained frame time its point arm
cannot: **27.18 ms at 3 000 rows** against a 16.67 ms vsync floor, and
20.13 ms at 100 — not vsync-bound at either size, i.e. missing frames on
this workload.

### Both backends, and what the comparison above does *not* depend on

**Every row in both tables stops before rasterisation**, so the CPU/GPU
choice moves none of them. That is what makes the comparison possible at
all: Qt and GTK rasterise to pixels, iced and Xilem to a GPU surface,
egui to a texture, and a matched stopping point is the only place the
seven can be put on one axis.

lumen-static's own backends are measured against each other on the same
`DisplayList` in `docs/results-gpu.md` §1 (re-run 2026-09-22): **17.6×
at 4K**, with the crossover at 3–5% surface damage regardless of command
count. `benches-competitive/arms/raster` is the only arm here that
rasterises on both sides; composing its figures onto the tables above
would be mixing two different scenes, which is why it is cited rather
than added as a column.

### The control, and what actually moved

Every foreign arm is within a few percent of §5b's 2026-09-08 numbers,
which is what makes the lumen-static comparison meaningful rather than
a story about the machine:

| arm, 3 000 rows | 2026-09-08 | 2026-09-22 |
|---|---:|---:|
| iced | 287.9 | 290.1 |
| Qt (natural update) | 300.6 | 304.8 |
| masonry | 889.8 | 896.3 |
| egui | 779.1 | 812.3 |
| Lumen (audit off) | 1 400.5 | 1 447.4 |
| GTK | 126.5 | 112.2 |
| **lumen-static** | **37.4** | **32.3** |

**lumen-static is 9–14% faster than it was**, across all three sizes
(1.9 → 1.7, 13.0 → 11.2, 37.4 → 32.3), which is M26 Phase B's
optimizations arriving. Nothing regressed.

### Idle RSS and distributable size

| arm | idle RSS | size |
|---|---:|---|
| **lumen-static** | **24.7 MB** | 2.8 MB exe + 0.2 MB in 1 lib |
| lumen-static (lean) | — | 2.6 MB exe + 0.2 MB in 1 lib |
| Qt 6.4.2 | 46.5 MB | 0.0 MB exe + **70.1 MB in 45 libs** |
| Slint | 68.3 MB | 14.8 MB exe + 2.0 MB in 9 libs |
| GTK 4.14.5 | 136.9 MB | 0.0 MB exe + 32.1 MB in 65 libs |
| iced 0.14 | 196.2 MB | 10.7 MB exe + 0.2 MB in 1 lib |
| Xilem | 321.3 MB | 10.8 MB exe + 0.2 MB in 1 lib |

The shape is unchanged from §3 and §4: the Rust GPU-backed toolkits
carry a wgpu device and its shader pipeline in RSS, and the C++ ones
carry their library set on disk instead.

### What this run is worth

The ranking did not change and was not the point. **The run's output was
a harness defect that would have made every future comparison wrong**,
and it was found only because the foreign arms held still while ours
appeared to move. A benchmark whose control arms are all in the same
repository as the thing being measured cannot do that.

## What is not measured here, and why

- **Slint has no frame arm.** It has a size and RSS arm and appears in
  those tables; nobody has written a headless frame harness for it, and
  a number invented for this report would not be comparable with the
  rest.
- **Xilem's view layer.** `masonry` is its widget/layout half and the
  part with a headless driver. A full Xilem frame also diffs a view
  tree, so its row understates it — stated because it is the asymmetry
  that flatters masonry.
- **GPU rasterisation.** Every arm here is CPU. iced via `tiny_skia`,
  masonry stopped before vello's GPU pass, Qt via `QWidget::render`,
  lumen-static via its own software canvas. No arm gets a GPU
  advantage, and none of these numbers describe a GPU frame.
- **The rasteriser, until 2026-09-09.** Every arm above stops at a
  display list, which is right for comparing *frameworks* and means none
  of them says anything about the renderer underneath. `docs/results-vello.md`
  adds the arm that does: our software canvas against `vello_cpu`, both
  rasterising the same scene. It is also the first arm this project
  loses — below 1 Mpx, to a worker heuristic rather than to the
  rasteriser.
- **Real applications.** A 3 000-row flat list is not how any competent
  application in *any* of these frameworks builds a long list — all of
  them have virtualized containers, and none is used here, on purpose,
  so that the arms are shaped identically. Lumen's report makes the same
  disclaimer and it applies unchanged.
