# Images in motion: decoding, animated images, video

M84's results (`docs/plan-images-in-motion.md`). Every figure an M84 gate
prints is recorded here once and cited from the rustdoc, the manifests,
the plan and the commits. The plan-wide budgets and kill rules K1–K4 are
`docs/plan-iced-roadmap.md` §2 and §4; the firmware's four-binary table
is `docs/results-embedded.md`'s.

## Thresholds, written before anything is built (copied at T84.1, 2026-10-06)

M84's kill rules, copied from the plan so each later row has its
threshold beside it. A row's verdict is filled in by the task that
measures it, never edited here.

| rule | quantity | threshold | measured by | if it fires |
|---|---|---|---|---|
| K84a | `image`'s own crates: K2's count of `lumen-static-winit` with `--features lumen-static-shell/image`, minus without | ≤ **15** | T84.1 | PNG decoding is left to the application, only `gif` ships, and the count is recorded |
| K84b | `lumen-static-video`'s dependency set; any `unsafe` in it; the development libraries on the reference machine | ≤ **150** crates; **no** `unsafe`; present | T84.9 | `lumen-static-video` is deferred (T84.10's second branch) |
| K84c | median `layout_ns` per animated frame on the showcase's Image entry (`app.perf`) | ≤ **100 µs** | T84.3 | `App::tick_marked` is built before T84.4 and the gate re-runs |
| K84d | `resets` in T84.2's 100-change loop with the in-place write | **0** | T84.2 | a changing image gets a dedicated texture; the atlas keeps stills only |
| K84e | 1280 × 720 at 30 frames/s on the software backend | ≥ **28** presented per second | T84.8 | the rustdoc and this doc state the largest size that holds 30/s; the widget ships |
| K4 | the four firmware binaries' `.text`, `just firmware-delta <parent> firmware=0 firmware_interactive=0 firmware_bind=0 firmware_cascade=0` | Δ **0 B** each (no firmware paints an image) | every task that touches `lumen-static` or `lumen-static-widgets` | the task stops and attributes the delta by symbol diff |

## T84.0 — the gates that compile today, at `1024cba` (2026-10-06)

Pins, green, each flipped by the task that closes its gap
(`docs/plan-iced-roadmap.md` §3.1).

| gate | prints today | flipped by |
|---|---|---|
| (a) `crates/lumen-static-shell/tests/image_frame.rs`: a store switches one id from red to blue; tick (answering `true`), pump, present | centre **red**, `damaged_px` **0** (the image's box is 1,600 px) | T84.2 |
| (b) `crates/lumen-static-wgpu/tests/image_frames.rs`: the same store, two full-damage draws (RTX 4070, Vulkan) | second draw's centre **red**; `resets` 0 | T84.2 |
| (c) `scripts/check_default_crates.sh` (`just static`): K2's count against the parent commit's | **96** at the work tree and at `1024cba`, Δ **0** | stays green |
| (d) `just firmware-delta 1024cba firmware=0 firmware_interactive=0 firmware_bind=0 firmware_cascade=0` | Δ **0** on all four (`firmware_interactive` 63,480 B) | — |

## T84.1 — the `image` feature: PNG and GIF decode into `ImageStore` (2026-10-06)

`lumen_static_shell::image`: `ImageStore` (unconditional), and behind the
shell's default-off `image` the `decode_png` and `decode_gif` that answer
a `DecodedImage` of composited RGBA8 frames.

**K84a — does not fire.** K2's command on `lumen-static-winit`:

| build | crates |
|---|---:|
| default | **96** |
| `--features lumen-static-shell/image` | **105** |
| **`image`'s own** | **9** (bound 15) |

The nine: `png`, `gif`, `fdeflate`, `flate2`, `miniz_oxide`, `adler2`,
`simd-adler32`, `crc32fast`, `weezl` (`bitflags` and `cfg-if` were
already in the window's graph). `gif` is taken without its default
`color_quant`, which only its encoder uses, and the lock holds `flate2`
at 1.1.9: 1.1.10 moves to `miniz_oxide` 0.9 while `png` 0.18 asks for
0.8, which put both versions in the graph for **10**. The shell alone
goes 8 → 19 (`bitflags` and `cfg-if` are new to it, not to the window).

**The release-size delta of a program that decodes.** One probe program
— an `ImageStore` filled from a file's bytes and read back — built three
ways in its own workspace with this workspace's lock, `--release`,
`strip = true`:

| build | stripped bytes | `.text` |
|---|---:|---:|
| `image` off | 376,128 | 269,152 |
| `image` on, no decoder called | 377,776 | **269,152** |
| `image` on, `decode_png` and `decode_gif` called | **576,360** | — |

So a program that decodes PNG and GIF pays **+200,232 B** stripped. With
the feature on and no decoder called, `.text` is byte-identical: the
+1,648 B is all `.gcc_except_table` (and `.relro_padding` falls by the
same page arithmetic). That is K2's recorded exception measured rather
than argued — the linker drops an uncalled decoder, so a symbol count
cannot see the feature and the crate count, gate (c) above, is its
falsifiable half (`docs/plan-iced-roadmap.md` K2).

**Gates** (`crates/lumen-static-shell/tests/image_decode.rs`, run by
`just test` with `--features image`):

| gate | green | injection → red |
|---|---|---|
| PNG round trip, 7 × 5 RGBA | 7 × 5, 140 bytes; **0** of 140 bytes differ (bound 0) | alpha dropped (decoded as RGB): the decoder's own length check answers `Malformed("105 bytes of pixels for 7x5")`; with that check removed too, the test's "length" assertion fails at 105 against 140 |
| GIF compositing, 4 frames over 8 × 8, frame 2 a 2 × 2 at the top-left with `Keep` | delays `[100, 100, 100, 100]` (bound `[100; 4]`); frame 2 at (4, 4) = frame 1's green `[40, 160, 60, 255]` | each frame drawn on a cleared canvas (uncomposited): (4, 4) reads `[0, 0, 0, 0]` |
| every truncation is an error, never a panic | GIF: 133 prefixes, **0** panics, **0** decoded; PNG: 97 prefixes, 0 panics, 4 decoded (93–96), each the whole picture | `unwrap()` in the GIF frame loop: **98** of 133 prefixes panic (35–132) |
| default stays off | `scripts/check_default_crates.sh`: 96 against 96 at `55f3962`, Δ 0 | `image` in the shell's `default`: 105, Δ **9** (= K84a's count), exit 1 |
| lint and docs with the feature on | `cargo clippy -p lumen-static-shell --features image --all-targets -- -D warnings` and `cargo doc -p lumen-static-shell --no-deps --features image` with `-D warnings`, both clean | an unused binding in `decode_png`: the feature's clippy line fails, the default line stays green |

The PNG's four decoding prefixes are missing only the end chunk's CRC:
`png` reads every pixel before it needs the end chunk, and the answer is
the whole picture, so the gate holds the PNG to "never a panic, and any
answer is the whole picture" and the GIF to the plan's "every prefix is
an error".

**Firmware.** `just firmware-delta 55f3962 firmware=0
firmware_interactive=0 firmware_bind=0 firmware_cascade=0`: Δ **0** on
all four — the task touches only the shell, which no firmware links.

## T84.2 — the frame in the command, and the atlas in place (2026-10-06)

`Cmd::Image { rect, id, tint, frame: u32 }`, `Painter::image(rect, id,
tint, frame)`, `ImageSource::image(id, frame)`. `Recording::image` and
`Cmd::hash_into` fold the frame; `Scaled`, `Deferring` and `Recording`
forward it; `Image` gains `frame` (+4 B in the tree struct, not per node);
`ImageStore` answers `frames[frame % len]`; the software canvas resolves
each command at its own frame; the SVG writer emits `data-frame`;
`wire::VERSION` 4 → **5**. On the GPU an id's slot is rewritten **in
place** for a same-size frame, inserted again on a size change, and a
second frame of one id in one draw takes a side slot.
`AtlasStats::image_uploads` and `image_upload_bytes` count every write.

**Gates** (software: `crates/lumen-static-shell/tests/image_frame.rs`;
GPU: `crates/lumen-static-wgpu/tests/image_frames.rs` under `just gpu`,
RTX 4070, Vulkan):

| gate | green | injection → red |
|---|---|---|
| T84.0(a) flipped: a tick writes `img.frame += 1`, pump, present | tick `true`, `img.frame` 1; centre **blue**; `damaged_px` **1,600** against the image's box of 1,600 px: **1.00×** (bound 1.00×); accept: the next frame damages 0 | `Recording::image` stops folding `frame`: centre red, **0** px damaged, "frame 1 of img (id 7) did not reach the canvas" |
| the unchanged neighbour: two `Image`s of id 7, only the first advances | ratio **1.00** against the first's box; the second's centre red; its journal signature unchanged (**absent** from the changed list) | `Image` paints a store-wide shared frame (`fetch_max`, the S2 shape): **3,200** px, **2.00×**, both centres blue, the second's signature moved (listed) |
| journal: the advanced node's signature (`DisplayList::hash_from` over its own paint, as `sign_leaves` and the agent's journal sign it) | `…16c5` → `…16c6`, moved | `Cmd::hash_into` omits `frame`: `53ab…ebf1` → `53ab…ebf1`, "node 1 (img): its journal signature did not move with its frame" |
| T84.0(b) flipped: frame 0 then frame 1, both full damage | second centre **blue**; `image_uploads` **2**, 512 B | `image_slot` ignores the frame (the id-only key, T84.0's behaviour): red, `image_uploads` 1 |
| one id at two frames in one list (P1): frame 1 at A, frame 2 at B | GPU A, B = red, blue = software's; a redraw adds **0** uploads | the per-draw record dropped (the second command rewrites the first's slot): GPU A **blue**, software red, "A (id 7, frame 1): the GPU differs from software" |
| no atlas churn (K84d): 300 × 300, four frames, 100 changes after a first draw | `resets` **0** (bound 0); `image_uploads` **101** (bound 1 + changes = 101); bytes per upload **360,000** (bound 300·300·4); draws not showing their frame **0** of 100; accept: 100 draws at an unchanged frame leave `image_uploads` at **1** | insert again instead of rewriting: `resets` **11** (the plan predicted ≥ 10: nine such slots fit 1024²) |
| size change: 300 × 300 → 200 × 200 → 300 × 300, quadrants rotated per frame | all four quadrant centres right at each size; `resets` **0** (bound ≤ 1); 3 uploads | write in place whatever the size: `write_texture` slices the 300-square slot's bytes from a 200-square frame and panics ("range end index 360000 out of range for slice of length 160000") — red, but by a panic, not an assertion: a wrong-size write has no frame that draws to compare |
| `ImageStore` answers `frames[frame % len]` (`image_decode.rs`, `--features image`) | frame 2 answers frame 2, frame 5 answers frame 1; a stream answers its latest at frame 9 | answer `frames[0]`: "frame 2 did not answer the id's third frame" |
| `deferring_forwards_every_command` (`tests/overlay.rs`) paints its image at frame 3 | 7 commands in, 7 out, equal | `Deferring::image` forwards `frame: 0`: "first difference at Some(3): Image { … frame: 3 } became Image { … frame: 0 }" |
| wire: an image at frame 7 round-trips; a version-4 literal is refused | 39 B at version 5, frame 7 back; `a version-4 buffer decodes: false`; `wire::VERSION = 5 (want 5)` | the encoder skips `frame`: both round trips fail to decode (`expect("decodes")`, the trailing-length check) |
| K1 | `size_of::<Cmd>()` **60** B (bound 60; `tests/glass_cmd.rs`); the image payload is 28 B under the 56 B gradient | — |

**K84d — does not fire**: 0 resets over the 100-change loop.

**Firmware (K4).** `just firmware-delta 9f48fd6 firmware=0
firmware_interactive=0 firmware_bind=0 firmware_cascade=0`: Δ **0**
`.text` and **0** `.rodata` on all four — `firmware` 30,606 / 13,344 B,
`firmware_interactive` **63,480** / 21,886 B (headroom 18,440 B to the
81,920 B ceiling), `firmware_bind` 34,014 / 13,948 B, `firmware_cascade`
32,694 / 14,132 B. No firmware paints an image, so the new parameter and
the fold link nowhere; `docs/results-embedded.md`'s table does not move.

**K3 — the frame arm, eight interleaved invocations a side** (`R=1
benches-competitive/run.sh -p lumen-static-frame` under `just
bench-lock`, base `9f48fd6` from its firmware-delta worktree, every run
quiet at load1 1.00 against the lock's bound of 8). Medians and spreads
in µs:

| row | base median | base spread | T84.2 median | inside |
|---|---:|---:|---:|---|
| `idle (floor)` | 58.05 | 56.0–62.7 | 59.10 | yes |
| `ALL rows: +paint` | 1,280.90 | 1,217.9–1,286.2 | 1,281.85 | yes |
| `+render (damage)` | 73.65 | 72.9–81.9 | 73.25 | yes |
| `layout` | 19.25 | 19.1–21.0 | 19.20 | yes |
| `+paint` | 38.95 | 38.7–43.4 | 38.70 | yes |

Not named by K3, printed: `ALL rows: layout` 1,159.15 → 1,160.20 (inside
1,100.2–1,166.1), `+paint (culled)` 21.45 → 21.25 (inside), `+render
(full)` 132.20 → 131.30, 0.1 µs *under* the base's minimum of 131.4 — a
faster median on a row K3 does not name, read one-sided as M82's K82c
ruling reads it.
