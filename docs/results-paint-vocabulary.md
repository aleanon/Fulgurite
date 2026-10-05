# The paint vocabulary: gradients, paths, vector text

M82's results (`docs/plan-paint-vocabulary.md`). Every figure an M82
gate prints is recorded here once and cited from the rustdoc, the plan
and the commits. The plan-wide budgets and kill rules K1–K4 are
`docs/plan-iced-roadmap.md` §2 and §4; the firmware's four-binary table
is `docs/results-embedded.md`'s.

## M82a's headline — gradients, closed at T82.4 (2026-10-04)

**A gradient is now radial or linear, rounded or square, and reachable
from a stylesheet, for no `Cmd` growth and no frame cost.** The figures
are the task sections below and the kill-rule table at the end; this
section cites them and states none of its own.

- **The command did not grow.** `Stops` gave up 15 B (T82.1) and the
  wider `Gradient` spent them: the K82a mirror reads 56 of 56 and `Cmd`
  stays 60 (T82.1, T82.2).
- **A `Style` byte reaches paint as content, never as an index.**
  `background_fill` is resolved against `Painter::fills` at every one of
  the border sites, so retention, the journal and the window see an
  ordinary gradient (T82.3: three sites, the palette, `set_fills`, the
  journal at 1.00×, the shadow oracle, the live Card).
- **What it cost.** `Style` grew by one 4-byte slot and `NodeState` not
  at all (T82.3's sizes row); the K3 arms stayed inside their spreads
  with no fill in the tree (T82.3's K82c table); and the firmware's
  milestone total is in the kill-rule table, worst on
  `firmware_interactive`.
- **Two shipped defects found on the way**, neither a gradient's own:
  the GPU had only ever drawn the first gradient in a frame (T82.2), and
  the D4 contrast walk judged text on the stage behind a fill (T82.3).
  `Host::show` dropping the fill table on a retry was caught before it
  shipped (`docs/plan-paint-vocabulary.md`, "What building found").

## Thresholds, written before anything is built (T82.0, 2026-10-04)

M82's own kill rules and the bounds its tasks state, copied from the
plan so each later row has its threshold beside it. A row's verdict is
filled in by the task that measures it, never edited here.

| rule | quantity | threshold | measured by | if it fires |
|---|---|---|---|---|
| K82a | `Gradient` payload, `tests/glass_cmd.rs`' `GradientPayload` mirror | ≤ **56 B** (`Cmd` stays 60) | T82.1, T82.2 | `corner` leaves the command (payload 52), rounded gradients deferred |
| K82b | `firmware_interactive` headroom under 81,920 B | ≥ **1,024 B** after every task; no binary over 81,920 B | every task, `just firmware-delta <parent>` and `scripts/check_text.sh` | to the owner with the symbol diff; if moved, `path`/`fills` provided under `cfg(feature = "alloc")` |
| K82b / K4 | one task's `firmware_interactive` `.text` Δ | ≤ **1,000 B** (T82.2: **400 B**; T82.1: printed, expected ≈ 0) | every task | the same |
| K82c (K3) | frame idle and paint rows, `--bin textarea` `keystroke + layout`, `--bin cascade` `cascade_deep`, eight interleaved runs, no fill and no path | inside each row's spread | T82.3, T82.5 | `ops::fill` out of line (`#[cold]`); then the fill honoured only by derive containers and `Card` |
| K82d | path-mask texels on the showcase's first GPU frame with PaintBox and VectorText, share of the glyph atlas's start size | ≤ **25%** | T82.6 | paths get their own R8 atlas and binding |
| K82e | `benches --bin vocabulary`, a 24-glyph `VectorText` at 32 px, uncached, `just bench-lock` | ≤ **250 µs** a frame | T82.8 | the software canvas gets the GPU's `MaskKey` cache |
| K2 (`vector-text`) | new crates; stripped showcase's size delta | **0** crates; against **3 MB** | T82.7 | the feature stays default-off, size recorded |
| §2.1 | `Style` / `NodeState` | exactly the measured size, expected **84 / 84** B (16.4 KB per 100 nodes against 24 KB) | T82.3 | K1 |

## T82.0 — the pins, at `b873ab0` (2026-10-04)

Gates that compile today and hold today's behaviour green, so that the
task that changes it turns them red by name.

| what | gate | at `b873ab0` | injection (red) | flipped by |
|---|---|---|---|---|
| text over a gradient | `lumen-static-shell/tests/contrast_ground.rs` | setup 1 text run over 1 gradient; measured **0**, unmeasured **1** (`"gradient"`), clipped 0 | `ground_at` reads a gradient as its middle sample: measured 1 at **5.46:1**, fails by name | T82.2 |
| `Gradient` payload | `lumen-static/tests/glass_cmd.rs` `the_gradient_payload_fits` | **56 B** of 56 (`Stops` 36 B), `Cmd` 60 B | `Stops` gains a 4-byte field: **60 B** (`Stops` 40, `Cmd` 64), fails by name | kept at 56 by T82.1, T82.2 |
| `Deferring`'s stream | `lumen-static/tests/overlay.rs` `deferring_forwards_every_command` | **6** commands in, 6 out (unchanged since M80/T80.1) | `Deferring::gradient` drops its command: 6 in, **5** out, first difference at index 2 | T82.5 (6 → 7) |
| firmware | `just firmware-delta b873ab0` | Δ **0** on all four; `firmware_interactive` **63,084 B**, headroom **18,836 B** to 81,920 | (T80.0's) | — |

## T82.1 — `Stops` offsets in 1/255 (2026-10-04)

`Stops::at` is `[u8; MAX_STOPS]`; every consumer reads `Stops::offset`
(`at[k] as f32 / 255.0`). Wire `VERSION` 2 → 3.

| gate | measured (green) | bound | injection (red) |
|---|---|---|---|
| size, `tests/vector.rs` | `Stops` **21 B** | = 21 | — (see the mirror) |
| K82a mirror, `tests/glass_cmd.rs` | T82.2's layout **56 B**, today's variant 44 B, `Cmd` 60 B | ≤ 56 | `at: [u16; 4]`: **60 B** (`Stops` 26), fails naming `Gradient`; today's variant would read 48 and stay green |
| sampling, `tests/vector.rs`, 1,001 `t` against the `f32` sampler | offsets 0, 51/255, 1: max channel delta **0** (0 differ); `Stops::two`: **0** | 0 | `offset` without `/ 255.0`: delta **254**, 1,000 of 1,001 differ |
| — not representable | 0.5 as 128: max delta **1**, 564 of 1,001 differ | ≤ 1 | — |
| golden, `shell/tests/golden.rs` `gradient3.ppm` (3 stops, interior at 0.2, recorded at `8301746` with `f32` offsets) | max channel delta **0**, byte-identical | 0 | `offset` without `/ 255.0`: delta **239**, 6,656 of 9,600 pixels inside `[8, 8, 112, 72]` |
| SVG, `shell/tests/svg.rs` | offsets `0.000`, **`0.200`**, `1.000` | `0.200` | the raw byte formatted: `0`, **`51`**, `255` |
| wire, `wire.rs` tests | 3-stop round trip equal (52 B); `VERSION` **3**; the version-2 literal (a rect, no gradient) decodes: **false** | `VERSION == 3`; `None` | `VERSION` left at 2: the literal decodes **true**, the literal assertion fails |
| firmware, `just firmware-delta 8301746 print` | **+16 B** `.text` on all four, `.rodata` +0; `firmware_interactive` 63,100 B, headroom 18,820 B | printed (expected ≈ 0) | — |

The +16 B is `Stops::sample` alone (symbol diff): the firmware reaches it
through `Painter::gradient`'s provided flat fill, `stops.sample(0.0)`.

Silent consumers of `at` found by grep (a format string or an `as`
cast that would have compiled with a byte): `svg.rs`'s `{:.3}` only.
Every other consumer failed to compile and was changed to `offset`
(wgpu's `grad_of`) or to fold the byte (`Cmd::hash_into`,
`Recording::gradient`, `wire.rs`).

## T82.2 — radial and rounded gradients (2026-10-04)

`Cmd::Gradient { rect, stops, kind: GradKind, geom: [f32; 3], corner }`;
`Painter::gradient(rect, stops, GradShape, corner)`; `GradAxis` is the one
parameter formula (software, GPU upload, SVG, contrast). Wire `VERSION`
3 → 4. Gates: `lumen-static-shell/tests/gradient_shapes.rs`, the GPU half
in `lumen-static-wgpu/tests/parity.rs` (`just gpu`).

| gate | measured (green) | bound | injection (red) |
|---|---|---|---|
| K82a mirror, `tests/glass_cmd.rs` (now the variant's own fields over the real `Stops` and `GradKind`) | payload **56 B** (`Stops` 21, `GradKind` 1), `Cmd` **60 B** | ≤ 56 | `GradKind` as `repr(u32)`: payload **60 B**, fails naming `Gradient`; `Cmd` still reads **60** (rustc puts the tag in the slack), which is why the mirror exists |
| radial, software | centre (47, 47) **0**/255 from stop 0; radius (95, 47) **0**/255 from the last stop; setup 1 radial | ≤ 2/255 | `t` from the rect's centre, ignoring `center`: **213** and **119**/255 |
| radial, GPU (RTX 4070; llvmpipe) | both **0**/255 on both; region parity **0** px over 8 (worst 1) | ≤ 2/255; region ≤ 2 px | centre and edge swapped in the shader: **220**/255 both, region **7,936** px over 8 |
| corner, software | (1, 1) coverage **0**/255; centre the stop colour; corner 0 keeps (1, 1) inked (accept) | ≤ 1/255 | coverage dropped: **255**/255 |
| corner, GPU (both adapters) | (1, 1) coverage **0**/255; centre the stop colour; region parity **0** px over 8 | ≤ 1/255; region ≤ 2 px | `params.x` ignored for `KIND_GRADIENT`: **255**/255, region 152 px over 8 |
| scaled, software | list holds corner **24** at scale 2; physical (6, 6) the page | the page | `Scaled` forwards `corner` unscaled: corner 12, (6, 6) the stop colour |
| retention, `Recording` | centre moved: **8,192** px² in 1 region (the rect's 8,192); unchanged: **0** (accept) | = the rect's area; 0 | `Recording::gradient` omits `geom`: moved damages **0** |
| `Deferring`'s stream, `tests/overlay.rs` (now a radial with a corner) | **6** in, 6 out | equal streams | `Deferring::gradient` forwards `Linear`: 6 in, 6 out, first difference at index 2 (`Radial` became `Linear`) |
| contrast, `shell/tests/contrast_ground.rs` (T82.0's pin inverted; its old assertion against this code: measured 1 at 1.27:1, red by name) | failing second stop: **3.44**:1 (ground 98, 98, 98); passing stops: **7.56**:1; translucent: unmeasured `"translucent gradient"` | < 4.5; ≥ 4.5 | centre only: the failing run reads **9.25**:1 and passes |
| SVG, `shell/tests/svg.rs` | `<radialGradient … gradientUnits="userSpaceOnUse" cx="10.00" cy="10.00" r="31.62">`, `rx="6.00"`, no linear | the element | the radial written by the linear arm: no `<radialGradient` |
| wire, `wire.rs` tests | radial round trip equal (65 B); `VERSION` **4**; the version-3 literal (a rect, no gradient, from the encoder at `dfb72c5`) decodes **false**; an unknown kind byte decodes `None` | `VERSION == 4`; `None` | `VERSION` left at 3: the literal decodes **true**, the literal assertion fails |
| firmware, `just firmware-delta dfb72c5 firmware_interactive=400` | `firmware` **+136 B**, the other three **+16 B**, `.rodata` +0; `firmware_interactive` 63,116 B, headroom **18,804 B** | ≤ 400 B on `firmware_interactive`; K82b floor 1,024 | — |

The firmware delta is `Painter::gradient` alone (symbol diff): its
provided default draws a `rounded_rect` with the corner, and takes two
more arguments. K82a did not fire (56 of 56), as designed.

**A defect T82.2 found and fixed: the GPU drew only the first gradient in
a frame.** `shader.wgsl`'s `Grad` ended in a `vec3<u32>` pad, which aligns
to 16, so the WGSL struct was 112 B against the Rust `Grad`'s 96 and every
index past 0 read the wrong entry (length 0, so nothing). No parity frame
had held two gradients. The radial-and-rounded gate holds two, the
rounded one second; with the old pad restored its GPU centre is the page
and its region **4,708** px over 8.

`benches --bin vocabulary` (one gradient over 1024 × 1024, software
canvas, 31 renders, under `just bench-lock`, quiet):

| arm | min ns/Mpx | median ns/Mpx |
|---|---:|---:|
| `linear` | 10,320,631 | 10,328,859 |
| `radial` | 10,158,923 | 10,271,508 |
| `rounded-radial` | 11,093,184 | 11,642,109 |

The radial costs what the linear does: the per-pixel cost is
`Stops::sample`, not the parameter. A 24 px corner adds about 8–13%
(every pixel pays `sd_round_box`, not only the corners'). Setup asserted
per arm (exactly one `Gradient` of its kind); with `DisplayList::gradient`
storing every kind as `Linear`, the `radial` arm refuses to time.

## T82.3 — `Style.background_fill` and the fill table (2026-10-04)

`Style.background_fill: u8` (`0` none, `n` entry `n - 1`) is stamped into
`NodeState::fill` by `record_painted`; `ops::fill`, at every one of
`ops::border`'s call sites and before the flat background, resolves it
against `Painter::fills` (required) and paints an ordinary
`Painter::gradient`. The table comes from `Program::fills`, resolved
against the palette by `Host::set_fills` / `Host::set_palette` into the
frame list (`DisplayList::set_fills`) and handed to the agent's journal
through `TreeMut::with_fills`. `.lss`: `background-fill: <index>`
(`Takes::Index`, lowering to `Val::U8`). `Fields::BACKGROUND_FILL` is bit
**19** (`Fields::EACH` 19 → 20). Gates: `lumen-static-shell/tests/fill.rs`
unless named.

| gate | measured (green) | bound | injection (red) |
|---|---|---|---|
| sizes, `lumen-static/tests/cursor.rs` (and `focus.rs`, the embedded `const` asserts) | `Style` **84 B**, 81 used; `NodeState` **84 B**, 84 used; **168 B**/node, **16.4 KB** per 100 nodes | = 84 / 84; used 81 / 84; 24 KB | a second `u8` beside the field: E0027 at the used-bytes destructuring, then used **82** against 81 (size still 84); a `u32`: size **88** against 84 |
| reaches paint: a derive container, a `Repeat`, a `Button` | **1** gradient each, rect = the node's, stops = the table's; setup `st.fill` 1 and the rule's bit set | 1 | `record_painted` does not copy the byte: **0** gradients at all three, each named with `st.fill = 0`; `Scaled::fills` or `Recording::fills` answering `&[]`: 0 at all three with `st.fill = 1` |
| palette (a role-named stop) | `LIGHT.accent` then `DARK.accent` after `set_palette`; damage **3,600** px² | = the node's 3,600 px² | `set_palette` does not re-resolve: stop unchanged, damage **0** |
| `set_fills` marks, `Boundaries<256>` | a changed table: 1 gradient with the new stops; the same table again: damage **0**, can skip | new stops; 0 | `set_fills` without `mark_all`: the frame keeps the old stops |
| index 0 is today | table 2 entries, 5 nodes, 0 asking: **0** gradients | 0 | `ops::fill` paints entry 0 for `fill == 0`: **4** |
| disabled | stops' alpha **102**, 102 (40%) | 102 | `ops::fill` ignores `disabled`: 255, 255 |
| journal, `lumen-static-agent/tests/fill_journal.rs` (transparent `bg`, role-named fill, palette swap) | container **1.00×** its box (6,400 px²), `Button` **1.00×**; total 12,800 = the two boxes | 1.00× | the container signature omits the fill: container **0.00×**, `Button` 1.00×; `TreeMut::as_ref` hands the journal `&[]`: both **0.00×** |
| W1010, `lumen-static/tests/sheet_lint.rs` | on a `Card`: **0** findings; on a `CheckBox`: `Unread(BACKGROUND_FILL)` | 0; 1 | `Patch::background_fill` records no bit: no finding |
| `Deferring`, `lumen-static/tests/overlay.rs` (a popup's content with a fill) | **1** gradient, after the scroller's last pop (index 4 against 3) | 1, in phase 2 | `Deferring::fills` answers `&[]`: **0** |
| shadow oracle, `tests/shadow.rs` (`shadow-oracle`), a filled `Button`, hover and click | 6 frames compared, no divergence | none | the scratch list is not given fills: diverges at frame 2, node 0, `"paint"` |
| masks, `lumen-static-widgets/tests/style_reads.rs` (probe `background_fill(1)` over a one-entry table) | every kind's mask equals its measured reads, 34 kinds | equal | before the masks were edited: **16** kinds read `background_fill` and did not declare it (`label: … omits read [background_fill]`) |
| `scripts/check_fill_paint.sh` (`just static`) | `sites 29 in 18 files, filled 29, allowed 0` | all filled | `fill(` removed from `Button::paint`: `ops.rs:736: Button draws a border with no fill( before it`, `Label` still filled; `BACKGROUND_FILL` removed from `input.rs`'s seed: names `TextInput` |
| `.lss`, `lumen-static-lss` tests and `scripts/check_lss_setters.sh` | 25 setters, 25 declared; `background-fill: 1` lowers to `Val::U8(1)`; `1px`, `1.5` and `256` refused with "a whole number from 0 to 255, no unit, like `1`" | — | the `PROPERTIES` line deleted: the script names `background_fill`; declared `Takes::Byte`: `background-fill: 1` refused ("a whole number of px … like `2px`, not `1`") |
| `.lss` both ways, `lumen-static-shell/tests/lss_parity.rs` (`pass_accept.lss` gains `background-fill: 1`) | equal both ways | equal | the shell's `set` arm deleted: `check_lss_setters.sh` stays green and `lss::load` panics "no setter `background_fill` taking U8(1)" |
| D4, showcase `every_demo_text_reads_in_both_palettes` (now samples a gradient ground at a run's centre and ends) | the Card's two runs judged on the fill in both palettes, worst **12.02**:1 / **8.89**:1 (title), 590 runs, 0 under | ≥ 4.5; ≥ 4 runs on a gradient | the fill's edge stop as `Text`: title **1.63**:1 / **1.34**:1, red; the same with the gradient arm removed: green at 15.93:1 — the walk had judged text on the stage behind a fill |
| live, `scripts/widget_drive.sh Card` (Everything, Boundaries, Boundaries+oracle, scale 2, both `+shaping` legs) | "the card's fill is a rounded radial": centre (254, 254, 254) against sides (216, 221, 230), **38** levels; corner against the stage **0** levels, at scale 1 and 2; D5 parity 1 of 1 byte-identical; oracle 544 frames, none diverged | ≥ 16; ≤ 2 | `ops::fill` removed from `Card::paint` (Boundaries leg): centre and sides all (255, 255, 255), **0** levels, FAIL by name |
| firmware, `just firmware-delta 22ce8f8` | `firmware` **+156**, `firmware_interactive` **+288**, `firmware_bind` / `firmware_cascade` **+220**; `.rodata` +20–32; headroom **18,516 B** | ≤ 1,000 on `firmware_interactive`; K82b floor 1,024 | — |

**K82c** — the K3 arms with no fill and no path, `22ce8f8` against this
tree, eight interleaved invocations a side under one `just bench-lock`
(quiet; load1 0.87 before, 1.12 after, bound 8):

| row | base median (spread) | T82.3 median | verdict |
|---|---:|---:|---|
| frame `idle (floor)` | 60.70 µs (54.60–61.60) | 60.45 | inside |
| frame `ALL rows: +paint` | 1,287.25 µs (1,229.10–1,295.10) | 1,290.40 | inside |
| frame `layout` | 20.10 µs (19.00–20.80) | 20.20 | inside |
| frame `+paint` | 38.75 µs (36.70–42.00) | 40.60 | inside |
| frame `+render (damage)` | 74.20 µs (70.40–80.50) | 76.30 | inside |
| `textarea` `keystroke + layout, 100 KB` | 5.34 ms (5.07–5.47) | 5.38 | inside |
| `cascade` `cascade_deep (whole table)` | 15,761.53 ns (15,663.74–15,841.23) | 15,724.37 | inside |

K82c holds, so neither of its steps (`#[cold]`, then the fill honoured by
derive containers and `Card` only) is taken. K82b holds (288 of 1,000 B;
18,516 B of headroom against its 1,024 B floor). K1 holds at the plan's
assignment (84 / 84; `Cmd` 60 and `Event` 16 untouched); the embedded `Ui`
is unchanged at 1,216 B. The firmware attribution is in
`docs/results-embedded.md` M82/T82.3: `ops::fill`'s body and its call
sites, which neither D3 nor D6 accepted, are reported to the owner there.

## M82a's kill rules, at its close (T82.4, 2026-10-04)

Every rule M82a could read, with its number, fired or not (S5). The
milestone's firmware total is T82.0's base (`b873ab0`, the T82.0 row
above) against T82.3's head: the sum of the three task deltas, each
printed by `just firmware-delta <parent>` against its own parent. T82.4
changes no linked code: `just firmware-delta 975b9cb firmware=0
firmware_interactive=0 firmware_bind=0 firmware_cascade=0` prints **+0**
`.text` and **+0** `.rodata` on all four.

| rule | bound | measured | verdict |
|---|---|---|---|
| K82a — the gradient fits | `GradientPayload` ≤ 56 B, `Cmd` 60 | **56 B**, `Cmd` **60** (T82.1, T82.2) | holds; `corner` stays in the command |
| K82b — firmware floor | `firmware_interactive` ≥ 1,024 B under 81,920 after every task; no binary over | least headroom **18,516 B** (T82.3); largest binary 63,404 B | holds |
| K82b / K4 — per task | `firmware_interactive` Δ ≤ 1,000 B (T82.2: 400) | T82.1 **+16**, T82.2 **+16** (of 400), T82.3 **+288** | holds |
| — M82a's total | recorded | `firmware` **+308**, `firmware_interactive` **+320**, `firmware_bind` / `firmware_cascade` **+252** | recorded |
| K82c (K3) — no fill, no frame cost | each row inside the base's eight-run spread | all seven rows inside (T82.3) | holds; neither step taken |
| K1 — the struct budget | `Style` ≤ 84, `NodeState` ≤ 88; `Cmd` 60, `Event` 16 | `Style` **84** (81 used), `NodeState` **84**; `Cmd` 60, `Event` 16; embedded `Ui` 1,216 B unchanged (T82.3) | holds |
| K2 — default-off | crate count, new features | M82a adds no feature and no dependency | not applicable |
| K82d, K82e, K2 (`vector-text`) | — | M82b's (T82.6, T82.8, T82.7) | not yet read |

**`ops::fill`'s body and its call sites** (T82.3's symbol diff:
`ops::fill` +312 B in all four binaries, the call sites about +32–40
each) are in neither D3's list nor D6's, which accepts the `fills`
getter. They are recorded as part of the accepted `fills` feature, by
the coordinator at M82a's close, unless the owner says otherwise
(`docs/results-embedded.md` M82/T82.3).

## T82.5 — `Painter::path` and `Cmd::Path`, on the software canvas (2026-10-05)

`Painter::path(els, xf, paint)` is required; `PathEl` (move, line,
quadratic, cubic, close), `Affine` (`[a, b, c, d, e, f]`, per path, no
stack) and `PathPaint` (fill, stroke, width). `DisplayList::path` applies
`xf` as it copies the elements into `DisplayList::paths` and pushes
`Cmd::Path { rect, els, fill, stroke, width }` (36 B), whose `rect` is the
transformed control-point box grown by `width / 2`. `Cmd::hash_into` folds
the element count, never the offset; `DisplayList::hash_from` and
`Recording::path` fold the elements (`hash_path`). The software canvas
draws it with `rasterize_path` (signed-area accumulation, non-zero, curves
flattened at `FLATTEN_TOLERANCE` = 0.1 px, a stroke as quads and round
16-gons); the GPU arm is empty and pinned until T82.6; SVG writes
`<path d=…>`; the wire gives it tag **11** with `VERSION` still **4** (its
elements follow the text only when there are any); `contrast.rs` measures
a path as non-text ink in `Report::shapes`. `DisplayList` derives `Clone`,
and winit's `hot::Barred` copies the session's list with it. Gates:
`lumen-static-shell/tests/path_coverage.rs` and `path_damage.rs` unless
named.

| gate | measured (green) | bound | injection (red) |
|---|---|---|---|
| coverage, a star of lines (10-vertex outline), 32 × 32 supersampled non-zero reference, edge pixels only | **451** edge px; mean \|err\| **0.29**/255, max **1.12**/255; interior 2,552 px all 255, exterior 12,473 px all 0 | ≥ 200 px; ≤ 1/255; ≤ 8/255; exact | coverage rounded to 0 or 1: mean **49.7**/255, max 127.5/255 |
| coverage, a circle of four cubics (the reference is the true circle) | **320** edge px; mean **10.30**/255, max **26.48**/255; interior 4,559 / exterior 10,865 exact | ≥ 200; ≤ 17.0/255 (`2/3 · tol · 255`); ≤ 36.1/255 (`√2 · tol · 255`) | rounded: mean **50.7**/255, max 146.2/255 |
| round join, a 90° turn stroked 16 px, the pixel on the outer bisector `w / 4` out (3.54 px from the vertex, outside both quads, inside the disc) | coverage **1.000**; the segment's own interior 1.000 | > 0.5 | the join discs omitted: **0.000** |
| no phantom damage: A, a text run, B; A gains an element (same bounds) | damage **1,936** px² in 1 region = A's ink; B's offset 5 → 6, its `hash_from` signature unchanged; A reshaped at the same count: **2,200** px² (old ∪ new ink); unchanged: **0** | = A's ink; unchanged; = the union; 0 | `hash_into` folds `els.0`: B's signature moves (`0ee6a5e6…` → `3d121f46…`); `Recording::path` hashes only the count: the reshaped A damages **0** |
| transform: a wedge rotated 30° and moved, `Recording(Scaled(list))` | Recording's ink = the hand-rotated box grown by `w / 2 + 1` at scale 1 and 2; the list's rect = `k ×` the box grown by `k · w / 2` (scale 2: `[118.0, 58.0, 260.56, 162.78]`, width 4); the rotated tip pixel inked at (122, 73) and (245, 146) | equal (1e-3); the path's colour | `Scaled::path` forwards `xf` unscaled: the scale-2 list rect is the scale-1 box, red by name |
| `Deferring`'s stream, `lumen-static/tests/overlay.rs` (a path with a non-identity `xf` added) | **7** in, 7 out; the element stores equal | equal | `Deferring::path` empty: **7 in, 6 out** |
| banded, `tests/banded.rs` (a filled and stroked circle-and-star across the surface) and `scripts/check_banded.sh` (`just threads`) | **10** scenes byte-identical to the single-band render; every case asserts `bands_for` ≥ 2 | identical | the mask's rows taken relative to the band's `y0`: the threaded render differs |
| wire, `wire.rs` tests | every command round-trips (12, now with a path); a path of all five element kinds round-trips (112 B), its store equal; the buffer without its element section is refused; an empty list encodes exactly as version 4 did | equal; `None`; the v4 bytes | the decoder drops the elements: both round trips fail to decode (the range check refuses the command) |
| contrast, `shell/tests/contrast_ground.rs` | a path at **2.01**:1 is below 3 (1 shape); at **4.54**:1 it is not; text over the path's box unmeasured as `"path"` | < 3; ≥ 3 | the walk skips `Path`: no shape measured, the text judged against the page |
| GPU pin, `lumen-static-wgpu/tests/parity.rs` (`just gpu`, RTX 4070) | software inks **6,400** px, the GPU **0**; `KINDS` 11 → **12**, the `Path` row compared in "tests/parity.rs: path pin, until T82.6" | 0 | the arm pushes a quad: the GPU inks 6,400 px, red by name |
| SVG, `shell/tests/svg.rs` | `<path d="M0.00 0.00 L9.00 9.00" fill="rgba(255,0,0,1.000)" fill-rule="nonzero" stroke="none" …>`; 12 kinds drawn | the element | — (the vocabulary test's own) |
| clip, `shell/tests/clip_test.rs` | `Path` inside a `PushClip`: **0** px outside (ink inside 3,200 px; unclipped 14,796 px outside) | 0 | — (the clip test's own) |
| live, `scripts/widget_drive.sh PaintBox` (Everything, Boundaries, Boundaries+oracle, scale 2, both `+shaping` legs) | "the star path is filled inside its own outline": **4,182** px of its colour inside its outline of 4,346 px² (**96.2%**), 0 outside the box; at scale 2 **17,049** of 17,383 (**98.1%**); D5 parity 0 of 67,584 bytes differ | ≥ 90%; 0 | the canvas skips `Cmd::Path` (Boundaries leg): **0** px (0.0%), FAIL by name |
| error bar, `just hot-window-drive`'s failed-build step (`fixtures/hot-window`'s `#star`) | `#star` `[0, 0, 320, 48]` under the bar: `ui.regionHash` **f74fce21295f6269** over 15,360 px, the same as before the failed build; setup: the box's centre is the path's colour; the hot, static and everything legs all PASS | equal | `Barred` rebuilds the list by literal with `..Default::default()` instead of `src.clone()` (hot leg): **71c2a6b3e0395325**, red by name ("the bar's copy of the list lost its paths") |
| firmware, `just firmware-delta a77cc52` | `firmware` **+64 B**, the other three **+68 B**, `.rodata` +4 (the vtable slot), all of it `Checksum::path` (66 B); `firmware_interactive` 63,472 B, headroom **18,448 B** | ≤ 1,000 B on `firmware_interactive` (K4, K82b's per-task half); K82b floor 1,024 | — |

`benches --bin vocabulary` (one command over 1024 × 1024, software
canvas, 31 renders, under `just bench-lock`; load1 5.4 from a concurrent
`just ci` in another work tree, under the lock's bound of 8):

| arm | min ns/Mpx | median ns/Mpx |
|---|---:|---:|
| `linear` | 10,327,315 | 10,450,316 |
| `radial` | 10,352,179 | 10,417,833 |
| `rounded-radial` | 11,767,335 | 11,822,668 |
| `star-fill` (a 960 px star of lines, filled) | 1,296,102 | 1,324,094 |
| `circle-stroke` (a 960 px circle of cubics, stroked 8 px) | 1,155,034 | 1,176,834 |

A filled star covering a third of the canvas costs about an eighth of a
full-canvas gradient: the accumulation touches every pixel of the ink's
rows once to sum them, and the gradient samples its stops at every pixel.
Each path arm asserts one `Path` and elements in the store before timing.

**K82c** — the K3 arms with no path in the tree, `a77cc52` against this tree,
eight interleaved invocations a side under `just bench-lock`, run five
times. **Read literally it fired in four of the five**, each time by one
or two rows, and three of the departures are on the *fast* side:

| run | load1 (before) | rows outside the base's eight-run spread |
|---|---:|---|
| 1 | 5.08 (a concurrent `just ci`) | `+render (damage)` **77.15** µs, base 77.50–78.00 (faster) |
| 2 | 5.75 (the same) | `+paint` **40.90** µs, base 41.00–41.60 (faster) |
| 3 | 1.37 (the same, in its live phase) | `textarea` **5.53** ms, base 5.32–5.51; `cascade_deep` **15,962** ns, base 15,799–15,922 (slower) |
| 4 | 1.18 (the same) | none |
| 5 | 0.15 (quiet, `just ci` finished) | `+render (damage)` **72.85** µs, base 72.90–82.10 (faster) |

Run 5, the only one on a quiet machine, had every other row inside:
idle 59.25 in 58.80–103.10, `ALL rows: +paint` 1,228.60 in
1,220.50–1,284.90, `layout` 19.20 in 19.10–21.00, `+paint` 38.85 in
38.60–43.30, `textarea` 5.21 in 5.03–6.09, `cascade_deep` 15,060.58 in
14,881.99–15,670.77. T82.5 touches no code the textarea and cascade arms
run, and the frame arms paint no path; the paint rows' departures are
the work tree being faster. K82c's responses (`ops::fill` out of line,
then the fill restricted) answer a cost and do not apply to a speed-up.
Recorded as fired, on the fast side, and sent to the coordinator
before T82.5 was committed (the brief's stop rule). **Ruled not a
firing (coordinator, 2026-10-05):** K82c guards the *cost* of an
unused feature, and every response it names answers a cost; a median
below the parent's minimum on arms that paint no path is not one. The
two slower rows were on arms T82.5 does not run (`textarea`,
`cascade_deep`), and runs 4 and 5 held on a quiet machine. K82c stays
two-sided as a measurement and is read one-sided as a stop.

## T82.6 — GPU path masks (2026-10-06)

`Cmd::Path` on `lumen-static-wgpu`: each non-transparent paint is an R8
coverage mask in the **glyph** atlas (`Atlas<MaskKey>`; no stencil, no
new pipeline, binding or texture), rasterised on a miss by the software
canvas's `rasterize_path` from the same device-space elements and at the
same origin (x from the ink's left floor, y absolute), uploaded and drawn
as `KIND_GLYPH` quads in `PATH_TILE` (256 px) tiles. `MaskKey::Path {
hash, paint, frac, tile }`: `hash` is FNV-1a over the elements less the
ink box's integer floor, so a whole-pixel scroll hits; `paint` is `Fill`
or `Stroke(width bits)`, one key per paint. `AtlasStats` gains
`path_masks` (tiles rasterised) and `path_texels` (resident, counted from
the atlas). The window's GPU backend logs K82d's line once. Gates:
`lumen-static-wgpu/tests/parity.rs` (`just gpu`) on the RTX 4070
(Vulkan, `DiscreteGpu`) and llvmpipe (lavapipe, `Cpu`) unless named.

| gate | measured (green) | bound | injection (red) |
|---|---|---|---|
| parity, each paint alone over the page (`paths_agree_across_backends`: star fill 2,764 px, circle fill 4,198, circle stroke 948, rotated bar stroke 618) | worst channel delta **1** on every paint, both adapters | 1 | one key for both paints: the circle's stroke drawn from its fill's mask, worst **230** over 4,399 px, red by name ("a paint's mask disagrees"); the mask rasterised at the integer origin without the fractional offset: worst **34 / 145 / 230 / 106** |
| parity, the whole scene (7,922 px inked on both) | RTX worst **1** (838 px differ, 0 over 1); llvmpipe worst **2** (31 px over 1, all on the circle's stroke edge over its translucent fill) | 2, one per blend | as above: worst **230** |
| masks for a filled and stroked path | **2** | 2 | — (the one-key injection reads 3 for the scene's 4 paints) |
| frame region "path" (`every_region_is_within_its_own_bound`, a filled and stroked triangle) | **0** px over 8, both adapters; `KINDS` 12, the `Path` row now `None` ("in the frame") | 2 | one key: **356** px over 8 |
| cache (`path_masks_are_cached_and_survive_a_whole_pixel_scroll`) | first frame 1, idle **0**, changed path **1**, moved 1 px **0**; the moved frame is the original shifted 1 px at 0 differing px (setup) | 0 / 1 / 0 | the key hashes absolute position: moved 1 px **1**, red by name |
| tiles (`a_wide_path_is_drawn_in_tiles_without_a_reset`, 2,000 × 100 px on a 2048 × 128 target) | **8** tiles, 200,000 texels; resets **0**, growths **0**, atlas 1024²; inked at x = 30 and 2,010 (setup) | 0, 0 | `PATH_TILE` = 4096 (one slot): 1 tile, resets **1**, growths **1**, atlas 2048² |
| T82.5's pin, re-run against this code | software 6,400 px, the GPU **6,400** px: "the GPU drew a path", red by name — the inversion | — | — |
| live, `just gpu-parity`'s PaintBox leg (showcase entry 10, 1120×760, RTX) | worst channel delta **3**, 0 px over 8; the minimal leg unchanged (worst 1) | 1% over 8 | the GPU's `Path` arm skipped: "the PaintBox window (entry 10) logged no path-mask line -- the GPU drew no path", exit 1 |
| K82d, the PaintBox entry's first GPU frame | **15,580** texels in 2 tiles, **1.49%** of 1024² | ≤ 25% | holds; read again with VectorText at T82.8 |
| resume (`just resume-drive`) | masks are atlas contents, no new GPU resource: gpu-resume leg 0 / 458,620 px differ after a suspend/resume round trip (2 backends made) | 0.1% | — (the drive's own) |
| firmware, `just firmware-delta a10ecec firmware=0 firmware_interactive=0 firmware_bind=0 firmware_cascade=0` | **+0** `.text`, **+0** `.rodata` on all four (`firmware_interactive` 63,472 B) | 0 | — |

## T82.7 — `TextMeasure::outline` and the `vector-text` feature (2026-10-06)

`TextMeasure::outline(text, size, weight, sink) -> bool` is provided,
answering `false`; `PathSink` takes `PathEl`s (and announces each glyph
through a provided `glyph`). `FontMetrics` overrides it under the shell's
default-off `vector-text` (forwarded by `lumen-static-winit/vector-text`):
`FontStack::outline` lays the run out as `measure_run` does, on one line,
and writes each glyph's ttf-parser outline at the pen `extent_by` advances
by, on its line's baseline plus the cap shift, at `size × scale` divided
back down. Under the feature a stack keeps each face's bytes (regular,
bold, and a fallback once read); ttf-parser is the one fontdue already
builds, with its features. `ComplexText` overrides it from its shaper's
pens, through rustybuzz's face. Gates: `lumen-static-shell/tests/vector_text.rs`
(`--features vector-text`, and `vector-text,shaping` for `ComplexText`)
unless named.

| gate | measured (green) | bound | injection (red) |
|---|---|---|---|
| outline, `outline("AVWay", 32)` on the pinned face | **87** elements over 5 glyphs; box `[0.25, 7.67, 113.98, 37.66]` against fontdue's glyph boxes `[0, 7, 114, 38]` (run measured 115 × 38); ids `place_run` = cmap = announced: A 36, V 57, W 58, a 68, y 92 | > 0; 1 px per edge; equal | pens ignored (every glyph at x = 0): right edge **30.59** against 114, red by name ("edge 2") |
| pens, `AVWay` and `A V` | every glyph's outline xmin = its pen + ttf-parser's bbox xmin, **0** difference (A 0 + 0.25, V 22 + 0.25, W 44 + 1.0625, a 76 + 1.92, y 96 + 0.95); the space announced with **0** elements; V at 22.25 in `AV`, 33.25 in `A V` | 1/64 px; 0; further right | pens from `Placed.x`: `'W' in "AVWay": outline xmin 46.0625 is not pen 44 + bbox xmin 1.0625` (A and V, bearing 0.25, floor to their pens and pass) |
| pens, `ComplexText` | 87 elements; each glyph at its shaper pen + bbox xmin, **0** difference (pens 0, 19.84, 41.73, 71.33, 90.94: rustybuzz kerns `AV`) | 1/64 px | its override deleted: answers **false**, 0 elements, red by name |
| scale | scale 2 against scale 1, logical px: the same 87 elements, x edges equal, y edges 0.5 apart (the cap shift is rounded at physical px) | 1 px per edge | — |
| a stand-in | `BlockMetrics.outline` answers `false`, 0 elements | false | — |
| memory, `tests/font_heap.rs` `vector_text_keeps_one_copy_of_each_face` (a regular + bold stack on the pinned face, after a bold run) | kept **1,519,960 B** above the two parsed faces (38,022,960 B), the two files 1,519,440 B; RSS moved **2,605,056 B** over the same steps (in-process, allocator reuse included) | files .. files + 64 KiB | `Bold::new` keeps the bytes in its parse buffer as well: **2,279,680 B**, red by name |
| symbols, `scripts/release_window_shape.sh` (`examples/minimal`, release) | default **0** outline symbols, `.text` 2,811,158 B; with `lumen-static-winit/vector-text` **10**, `.text` 2,819,478 B (+8,320) | 0; > 0 | `vector-text` a default feature of the window crate: the default window links **10** (`FontStack::outline_into`, `OutlinePen`'s builder methods, …), exit 1 |
| crates (K2's method) | `lumen-static-winit` **96 → 96**; `lumen-static-shell` 16 → 16 over `truetype` | 0 new | — |
| stripped release showcase (K2) | 9,689,320 → 9,699,560 B, **+10,240 B** | 3 MB | — |
| clippy, `cargo clippy -p lumen-static-shell --features vector-text --all-targets -- -D warnings` (the `fonts` recipe) | green | — | an unused binding in `outline_into`: the feature line fails (`unused variable: injected`), the workspace line stays green |
| firmware, `just firmware-delta 8db6ba5` | **+8 B** `.text`, **+4** `.rodata` on all four: the provided `TextMeasure::outline` in `BakedFace`'s vtable (8 B by `nm`); `firmware_interactive` 63,480 B, headroom **18,440 B** | ≤ 1,000 B on `firmware_interactive` (K4); K82b floor 1,024 | — |

**K2 for `vector-text` holds**: 0 new crates and +10,240 B stripped,
against 3 MB. It ships default-off as designed — a stack keeps its faces'
bytes under it (the memory row: one copy of each file), which no
application that never rotates text should pay. **The outline slot is in
neither D3's nor D6's list**: 8 B on every firmware binary, reported to the
owner (`docs/results-embedded.md` M82/T82.7). Under `shaping` and
`vector-text` together a `Complex` stack holds a face's bytes twice — the
shaper's interned copy and this feature's — which is recorded, not
deduplicated.

## T82.8 — `VectorText` (2026-10-06)

`VectorText<S>` in `lumen-static-widgets` (`alloc`): a run at an angle
(clockwise degrees), measured once into its outline
(`TextMeasure::outline`) and memoised on (a hash of the text, size,
weight, scale, the measurer's generation); painted as one `Painter::path`
whose `Affine` turns the run about its centre in the middle of the node.
Its box is the turned run box (the measured line box grown to the ink), so
the ink stays inside it; with no outlines it paints the string level, in a
box its own size. It publishes `Role::Label` with the run as its value.
The showcase's entry turns one run three ways (0°, 30°, 90°), with the
window's `vector-text` on. Gates: `lumen-static-shell/tests/vector_text.rs`
`mod widget` (the pinned face) unless named.

| gate | measured (green) | bound | injection (red) |
|---|---|---|---|
| rotated, "Vertical" at 32 px | 0°: **139** elements, ink 121.64 × 24.77 (w/h **4.9117**) in node `[10, 20, 135, 58]`; 90°: ink 24.77 × 121.64 (w/h **0.2036**) in node `[10, 20, 48, 145]`; w/h at 90° × w/h at 0° = 1.0000 | > 0 elements; 1%; ink inside the node | paint ignores the angle: the 90° ink `[-33.25, 70.19, 88.39, 94.95]` leaves the node `[10, 20, 48, 145]`, red by name; the box sized from the unrotated run: the 90° ink `[60.05, -23.25, 84.81, 98.39]` leaves `[10, 20, 135, 58]` |
| memo, a counting measurer that forwards `outline` | outline calls: still **0**, text changed **1**, generation bumped **1** (setup: the first layout 1) | 0 / 1 / 1 | the key omits the generation: the bump asks **0** times — a stale outline served (M45/T45.2's shape) |
| fallback, `BlockMetrics` at 90° | **1** `Cmd::Text`, **0** `Path`; the box the level run's (`[10, 20, 138, 52]`) | 1 / 0 | the fallback removed: **0** `Text`, red |
| speaks, `lumen-static-widgets/tests/a11y_roles.rs` (`--features a11y`) | `VectorText role=Label`; rows checked 65, `Node` impls 49 (49 covered) | every impl a row | the row deleted: `Node impls with no row in the table: ["VectorText"]` |
| speaks, `scripts/check_observe.sh` / `check_drive_coverage.sh` (`just static`) | "every widget speaks or is listed"; widgets exported 78, driven 46, allowed 32, missing 0 | 0 missing | `observe` removed: `no observe: crates/lumen-static-widgets/src/vector.rs: VectorText` |
| masks, `lumen-static-widgets/tests/style_reads.rs` | `vectortext` measured = declared: width, height, padding, cursor, visible, color, background, radius, font_size, font_weight, pressed_dim, hover_dim, border, background_fill | equal | — (the lint test's own) |
| palette, `tests/palette_defaults.rs` | the run on the page **17.85**:1 light, **15.53**:1 dark | 4.5 | — |
| D4, showcase `every_demo_text_reads_in_both_palettes`, now reading path ink at 3:1 | VectorText's three paths **15.93**:1 light, **13.38**:1 dark (6 judged; 10 paths in all, PaintBox's star and arc the other 4) | 3:1; 6 VectorText paths | the run painted at 16% alpha: **1.37**:1 / **1.57**:1, red. Its first run found **PaintBox's star at 2.67:1 in the dark palette** (`#c2410c`, T82.5's), fixed to `#e14d0a` (3.99 / 3.47) |
| book, `book/src/text.md` "Text at an angle" (`just book`) | a stand-in measurer's 80 × 20 run laid out at 90° is **20 × 80**, `is_vector()` | equal | the box sized from the unrotated run: `left: (80.0, 20.0)`, `right: (20.0, 80.0)`, the chapter's test fails |
| live, `scripts/widget_drive.sh VectorText,PaintBox` (Everything, Boundaries, Boundaries+oracle, scale 2, both `+shaping` legs) | "drawn rotated": the 90° run's ink **19 × 85** px (scale 2: **37 × 168**), 0 px outside its box; "it speaks its text": value `Vertical`, role `Label`, `~=Vertical` resolves to node 212, the 90° run's; D5 parity 29,760 px, 0 bytes differ (both strategies, both shapings); oracle 682 frames, none diverged; PaintBox's star with the new colour 4,180 px of 4,346 px² (96.2%), 98.1% at scale 2 | h ≥ 2w; 0 outside | paint ignores the angle (a Boundaries leg alone): ink **30 × 18** px, 65 px outside the box, FAIL by name |
| K82d, `just gpu-parity` (now a PaintBox and a VectorText leg, RTX 4070) | VectorText's first GPU frame: **5,654** texels in 3 tiles, **0.54%** of 1024²; PaintBox's **1.49%** (unchanged); both windows worst channel delta 3, 0 px over 8 | ≤ 25%; 1% over 8 | (T82.6's: the GPU's `Path` arm skipped) |
| firmware, `just firmware-delta 996c878 firmware=0 firmware_interactive=0 firmware_bind=0 firmware_cascade=0` | **+0** `.text`, **+0** `.rodata` on all four (`firmware_interactive` 63,480 B) | 0 | — |

**K82e** — `benches --bin vocabulary`, built `--features vector-text`, a
24-glyph `VectorText` ("Vector text, turned 30°!") at 32 px, one whole
frame (layout, paint, the software canvas rasterising the path) on a
512 × 512 canvas, 31 frames under `just bench-lock` (load1 2.99): **445**
elements (setup > 0, one `Path` a frame).

| arm | min µs | median µs |
|---|---:|---:|
| `uncached` (the outline memoised, the path rasterised every frame) | 108.2 | **109.4** |
| `cold` (the memo missing every frame: the outline too) | 119.7 | 120.7 |

**K82e holds**: 109.4 µs a frame against 250, so the software canvas does
not get the GPU's `MaskKey` cache. The outline itself is about 11 µs of a
frame (the `cold` arm's difference), which the memo saves.
