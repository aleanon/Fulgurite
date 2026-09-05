# lumen-static against Qt, GTK, iced, Xilem, Slint and Lumen

**Status: in progress, 2026-09-05.** Arms are added as they are measured;
a row that is not here has not been run. Nothing in this file is
estimated.

## Method — inherited, not invented

The workload, the stopping points, the size and RSS definitions and the
fairness rules are **Lumen's**, from
the sibling Lumen checkout at `/mnt/Utvikling-linux/Lumen` —
its `benches-competitive/` harness and the report beside it dated
2026-08-29. Re-deriving them would
have produced numbers that cannot be compared with the ones that already
exist.

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

## The defect this benchmark found on its first run

Before any comparison, the harness found a real bug in this framework,
which is the most valuable thing it has produced so far.

`Canvas::render_impl` tested every command against the damage rect
before drawing it — **except text**, which was shaped and rasterised
whatever the rect said, and only its individual *glyphs* were clipped.
On a 3000-row list that is 3000 shaping calls a frame for the ~40 rows
an 800 px viewport can show.

| | before | after |
|---|---:|---:|
| `+render (full)` | 7315.8 µs | **158.6 µs** |
| `+render (damage)` | 7304.0 µs | **73.6 µs** |

**46× on the full render, and damage went from useless to 2.2× better
than a full redraw** — it had been buying nothing, because the expensive
half ignored it. The cull is padded by one em, since a glyph may
overhang its run's rect; `damage_oracle.rs` still proves a partial
redraw pixel-identical to a full one.

This is what a competitive benchmark is *for*: the number was not
believable, and chasing it found a defect that no existing gate could
see, because every gate here draws a handful of labels.

---

## Frame cost, one row of 3000 changing

lumen-static, measured 2026-09-05:

| stage | µs | what it is |
|---|---:|---|
| `layout` | 23.2 | the layout pass — comparable to Qt's `layout`, GTK's `measure` |
| `+paint` | 40.2 | plus the display list — where the Rust arms in Lumen's comparison stop |
| `+render (full)` | 158.6 | plus rasterising the whole 400×800 viewport — apples-to-apples with Qt's `+paint` |
| `+render (damage)` | 73.6 | plus rasterising **only what changed** — what a real frame does |

Both render numbers are reported because quoting only the damage one
would measure an optimisation the other arms were not allowed to use.
