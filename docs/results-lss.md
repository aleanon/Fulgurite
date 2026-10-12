# Results: `.lss` stylesheets, parsed at compile time (M61)

`docs/plan-lss.md` owns the decisions; this document owns the numbers.
Every figure below is cited, not restated, by the plan, the rustdoc and
the book.

## The kill rules — written before the build (`docs/plan-lss.md` §5)

| | Rule | Measured | Verdict |
|---|---|---|---|
| **K1** | the firmwares' `.text` and `.rodata`, byte for byte, with the sheet authored by `include_lss!` against the hand-written one | **0 bytes** apart in all four firmwares | **holds** |
| **K2** | `include_lss!` adds under 100 ms to a warm incremental build of the invoking crate, 8 runs a side under `just bench-lock` | tally **+9 ms**, showcase **+44 ms** (was +94 ms before the one-pass tree check) | **holds** |
| **K3** | a release build without `reload` links no `fulgurite_lss` symbol | **0** symbols; **63** with `reload` | **holds** |

## K1 — zero bytes

`scripts/check_lss_parity.sh`, in `just firmware` (so `just test`). The
firmwares are linked twice: `SHEET` as the hand-written slice in
`fixtures/embedded/src/lib.rs`, and as `fixtures/embedded/src/sheet.lss`
through `include_lss!` (`FULGURITE_SHEET=lss`). Bytes of each section are
compared, not sizes.

| firmware | `.text` hand / lss | `.rodata` hand / lss | |
|---|---:|---:|---|
| `firmware` | 28,020 / 28,020 | 12,848 / 12,848 | identical |
| `firmware_interactive` | 57,198 / 57,198 | 21,282 / 21,282 | identical |
| `firmware_bind` | 31,620 / 31,620 | 13,424 / 13,424 | identical |
| `firmware_cascade` | 30,248 / 30,248 | 13,840 / 13,840 | identical |

Measured 2026-09-28 at T61.3 and re-run at every task since. The two
that do not read `SHEET` are the comparison's control: they must be
identical whatever the sheet is, and they are only because the switch is
a `cfg` from `build.rs`, not a cargo feature — a feature moved 780
`.text` bytes of functions the sheet never touches (`docs/plan-lss.md`,
"What building found").

**What K1 can see**, from its injections: the macro's stub (an empty
sheet) at −396 B `.rodata` in the two firmwares that cascade; the rules
emitted in reverse order at **0 B of size difference and different
bytes** — which is the case for comparing bytes.

## K2 — build time

`scripts/lss_build_cost.sh`, run as `just bench-lock
./scripts/lss_build_cost.sh 8`; a measurement, not a gate. Each arm is
built 8 times after a `touch`, the arms interleaved:

- **hand** — every `include_lss!(..)` replaced by the rules it expands
  to, written out. What K2 compares with.
- **const** — the same plus everything else the macro emits: the tree
  checks and the tracking `include_bytes!`.
- **macro** — the source as committed.

The written-out arms are cut from the macro's own expansion
(`-Zunpretty=expanded`), so no arm is a hand copy.

2026-09-28, quiet (load 3.35 before, 2.80 after; bound 8):

| crate | hand | const | macro | `include_lss!` adds | of which checks | of which the macro |
|---|---:|---:|---:|---:|---:|---:|
| tally (3 invocations, 30 checks) | 416 ms (376–432) | 435 ms (380–499) | 426 ms (387–468) | **+9 ms** | +18 | −10 |
| showcase (1, 49 rules, 42 checks) | 904 ms (868–933) | 936 ms (914–1021) | 949 ms (911–1001) | **+44 ms** | +31 | +14 |

The tally's delta is inside every arm's spread: a warm rebuild of the
tally cannot tell the two authorings apart. The showcase's is not, and it
is the tree checks' cost, not the parser's.

### The first measurement tripped nearly all of the budget

The same script at T61.4's first cut — one `count_wid(TABLE, id) > 0`
assertion per id, each walking the whole table in `const` evaluation:

| crate | hand | const | macro | adds | checks | macro |
|---|---:|---:|---:|---:|---:|---:|
| tally | 407 ms (388–446) | 409 ms (397–481) | 408 ms (382–441) | +2 ms | +2 | −0 |
| showcase | 910 ms (876–959) | 998 ms (978–1035) | 1,004 ms (983–1037) | **+94 ms** | **+88** | +6 |

+94 ms against a 100 ms bound, 88 of it the 42 assertions over the
showcase's table. The fix walks the table **once**, into the list of the
tree's ids (`style::wids_of`) and classes (`style::classes_of`), and each
assertion reads the short list (`style::names_contain`): **88 → 31 ms**.
Every per-name message is kept. A first attempt at K2 that removed the
rules along with the macro read +104 ms for the showcase, which is why
the arms are cut from the expansion: that number charged the macro for
compiling rules a hand-written sheet compiles too.

## K3 — nothing in a release window

`scripts/check_lss_release.sh`, in `just release-window-shape`
(`check-extra`). The tally — the first program that styles itself from a
`.lss` — built in release twice, counting symbols whose demangled name
contains `fulgurite_lss`:

| build | symbols | `.text` |
|---|---:|---:|
| release, no `reload` | **0** (bound 0) | 3,272,886 B |
| release, `fulgurite-winit/reload` | 63 (bound > 0) | 3,398,102 B |

The reader is compiled into the shell unconditionally and reached only
through `Program::live_lss`, which the window calls under its own
`cfg(any(debug_assertions, feature = "reload"))`; the linker drops the
rest. Injection: the window calls `live_lss()` unconditionally — 63
symbols in the no-`reload` build, and **+96,880 B of `.text`**, which is
what the reader costs a window that links it.

## The pilot — the tally's frames

`examples/counter-window/app.lss` replaces the hand-written
`stylesheet_for`. `FULGURITE_SHOT` at counts −3, 0 and 7, against shots taken
before the migration (686,415 B each):

| count | debug window (file read live) | release window (compiled in) |
|---:|---|---|
| −3 | byte-identical | byte-identical |
| 0 | byte-identical | byte-identical |
| 7 | byte-identical | byte-identical |

In `just test`, `the_lss_frames_are_the_hand_written_frames` renders the
same three counts headlessly from the compiled sheet, the live one and
the hand-written one: 0 px apart of 228,800. Injection: `pos` and `neg`
swapped in `app.lss` — 6,768 px apart at −3, 3,994 at 7, 0 at 0.

Non-test `app.rs`: 249 → 209 lines, plus `app.lss` at 80 (most of it the
header that says where it differs from Lumen's, and why).
