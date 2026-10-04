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
