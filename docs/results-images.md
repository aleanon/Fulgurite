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
