# lumen-static against Qt, GTK, iced, Xilem, Slint and Lumen

**Measured 2026-09-05.** Every number here was run on this machine on
this day. Nothing is estimated, and no number comes from a single run.

> **Read §6 first if you read nothing else.** The benchmark's most
> valuable output was not a ranking. It found **two real defects in this
> framework**, worth 46× and 6.4× on two different frames, and both were
> invisible to every gate here because every gate here draws a handful
> of labels.

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

- **masonry's number is an upper bound**, not a like-for-like: it is
  driven through `masonry_testing::TestHarness`, a *test* harness that
  may carry validation overhead. Lumen's report says so and it is
  inherited here rather than re-discovered.
- **GTK's number is a floor.** Its box clamps to the window, so it lays
  out ~63 rows at any N while every other arm lays out all 3 000. It
  also has no paint number, because `gtk_widget_snapshot` returns NULL
  outside a live frame-clock pass — the harness asserts rather than
  quietly timing nothing.
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
one: the same core, `no_std` with no allocator, links a **20 524-byte
`.text`** firmware for `thumbv7em-none-eabihf`
(`docs/results-embedded.md`).

## 5. lumen-static configurations

| configuration | effect |
|---|---|
| default | the rows above |
| `--no-default-features` on the windowing crate | −156 KB binary, −0.3 MB RSS |
| `observe` (the dev surface) | **+64 µs a frame** at 3000 nodes: `+paint` 33.8 → 97.6 µs |
| `Rebuild` retention | the `+render (full)` row: 215.2 µs against 71.8 µs — damage is **3.0×** |
| `no_std`, no allocator | not on this table: it has no window. 20 524 B of `.text` (`docs/results-embedded.md`) |

**`observe` costs nothing until it runs.** Layout, paint and render are
within noise of the default build ; the whole cost is the per-frame journal and ambient audit.
That is worth stating because the first measurement here said `observe`
made layout 46% slower — and that was a cold run, caught by repeating it.
**No number in this file comes from a single run.**

---

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
- **Real applications.** A 3 000-row flat list is not how any competent
  application in *any* of these frameworks builds a long list — all of
  them have virtualized containers, and none is used here, on purpose,
  so that the arms are shaped identically. Lumen's report makes the same
  disclaimer and it applies unchanged.
