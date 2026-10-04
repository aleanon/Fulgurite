# The paint vocabulary: gradients, paths, vector text

M82's results (`docs/plan-paint-vocabulary.md`). Every figure an M82
gate prints is recorded here once and cited from the rustdoc, the plan
and the commits. The plan-wide budgets and kill rules K1–K4 are
`docs/plan-iced-roadmap.md` §2 and §4; the firmware's four-binary table
is `docs/results-embedded.md`'s.

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
