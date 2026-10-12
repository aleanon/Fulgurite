# Images in motion: decoding, animated images, video

M84's results (`docs/plan-images-in-motion.md`). Every figure an M84 gate
prints is recorded here once and cited from the rustdoc, the manifests,
the plan and the commits. The plan-wide budgets and kill rules K1–K4 are
`docs/plan-iced-roadmap.md` §2 and §4; the firmware's four-binary table
is `docs/results-embedded.md`'s.

## M84a's headline — decoding and animated images, closed at T84.4 (2026-10-06)

**An image can now change under its id and repaint only its own rect,
on both backends, and a GIF plays at its own rate without waking the
window in between, for no firmware byte and no frame cost.** The figures
are the task sections below and the kill-rule table at the end; this
section cites them and states none of its own.

- **The version is in the command, not the store.** `Cmd::Image` carries
  `frame`, so the fold that decides damage sees a new frame, and the
  journal signs it (T84.2). The two pins T84.0 committed, stale pixels in
  software and on the GPU, are the gates T84.2 flipped.
- **The GPU atlas rewrites a slot in place**, inserts again only on a size
  change, and gives a second frame of one id in one draw a side slot;
  K84d's 100-change loop is the row that says it does not churn (T84.2).
- **Animation is the application's clock.** `Playback` answers a frame
  only when it changes, and `App::wake_in_ms` is the deadline the window
  waits on, asked through the session's guard so a panicking wake raises
  M36's screen (T84.3, the panic drive's new site). There is no second
  image widget.
- **Decoding is default-off**, and the crate count is the only gate that
  can see it: an uncalled decoder links nothing whether or not the
  feature is on (T84.1, K2's recorded exception measured).
- **What it cost.** The firmware did not move on any task, the frame
  arm stayed inside its spread (T84.2's K3 table), and the whole-tree
  mark per animated frame is priced under K84c's bound, so
  `App::tick_marked` is not built (T84.3).
- **What the live window found that the headless gates did not**: the
  drive's own agent request landing a redraw inside the counted second,
  a four-frame loop reading "held" on a one-second read, and a moving
  frame failing M57's scale-2 comparison for no defect (T84.3;
  `docs/plan-images-in-motion.md`, "What building found").

## M84b's headline — the application's store, stop, and `Video`, closed at T84.11 (2026-10-06)

**An application now owns the pixels it shows and changes them from its
own messages, a subscription it stops stops, and `Video` plays, pauses
and seeks over a paced feed, for no firmware byte; the GStreamer decoder
crate is deferred on K84b, because the reference machine lacks the
development libraries, not because the binding failed.** The figures are
the task sections below and the kill-rule table at the end; this section
cites them and states none of its own.

- **The store is the application's, read through the frame.**
  `App::images` (`#[app(images = f)]`) is asked on every present through
  the required `FrameSource::images`, and `ImageChain` resolves it before
  the window's store; it follows a hot swap and survives the failed-build
  bar, and the GPU atlas keys on the store that answered, invalidated on a
  swap and an M36 retry (T84.6). `ImageStore::replace` is how a stream
  writes a frame.
- **A stopped subscription stops**, both halves: the body returns on
  `Sender::is_stopped`, and the drain drops what it had sent; a stopped job
  is no longer re-issued across a swap (T84.7, `docs/results-tasks.md`).
  T84.5's pin is the gate T84.7 flipped.
- **`Video` is a controlled widget, not a codec**: the application holds
  `playing`, the position and the frame; a `VideoFeed` run by `video_job`
  sends frames as the application's message, paced to their timestamps;
  `GifFeed` is the feed that ships (T84.8). K84e's 1280 × 720 stream holds
  30 frames/s on the software backend.
- **Two framework additions the plan did not foresee** (T84.8, the plan's
  "What building found"). `Node::take_action` (core, behind `std`) is the
  route by which a widget hands its own handler a value, since a handler
  is one `fn(&mut S, &EventCtx)` and a captured drag activates nothing; it
  is a hook of the same kind as `activation_redirect` and
  `selects_on_focus`, and it links nothing into any firmware. And the
  derive gained `#[app(msg = T, apply = f)]`, `#[app(subscriptions = f)]`
  and `#[app(jobs = f)]`, because a derived application's message was
  `Never` and the showcase, which is derived, had to receive frames.
- **The decoder crate's deferral is a measurement, not a guess** (T84.9,
  T84.10): the binding's dependency set is far under K84b's bound and
  safe from our side, and a probe with hand-made linker symlinks played a
  `videotestsrc` frame; only the machine's missing `-dev` packages fire
  K84b, and the reopen condition names that install.
- **What the live window and CI found that the headless gates did not**
  (the plan's "What building found"): the scrub's planned injection could
  not fire live, and two live injections fired only once moved to where
  the mechanism reads them (T84.8). Two defects outside M84's own code
  surfaced while building it: the drives read the showcase's catalogue
  from `catalog.rs`'s *text*, so T83.6's `cfg`'d entry skewed every later
  index by one (`553ac3c`: the drives now ask the binary, `--entries`);
  and `scripts/check_default_crates.sh` could not run on a parentless
  `HEAD`, which is what CI's selftest copy is (`11c7aa2`).

## Thresholds, written before anything is built (copied at T84.1, 2026-10-06)

M84's kill rules, copied from the plan so each later row has its
threshold beside it. A row's verdict is filled in by the task that
measures it, never edited here.

| rule | quantity | threshold | measured by | if it fires |
|---|---|---|---|---|
| K84a | `image`'s own crates: K2's count of `fulgurite-winit` with `--features fulgurite-shell/image`, minus without | ≤ **15** | T84.1 | PNG decoding is left to the application, only `gif` ships, and the count is recorded |
| K84b | `fulgurite-video`'s dependency set; any `unsafe` in it; the development libraries on the reference machine | ≤ **150** crates; **no** `unsafe`; present | T84.9 | `fulgurite-video` is deferred (T84.10's second branch) |
| K84c | median `layout_ns` per animated frame on the showcase's Image entry (`app.perf`) | ≤ **100 µs** | T84.3 | `App::tick_marked` is built before T84.4 and the gate re-runs |
| K84d | `resets` in T84.2's 100-change loop with the in-place write | **0** | T84.2 | a changing image gets a dedicated texture; the atlas keeps stills only |
| K84e | 1280 × 720 at 30 frames/s on the software backend | ≥ **28** presented per second | T84.8 | the rustdoc and this doc state the largest size that holds 30/s; the widget ships |
| K4 | the four firmware binaries' `.text`, `just firmware-delta <parent> firmware=0 firmware_interactive=0 firmware_bind=0 firmware_cascade=0` | Δ **0 B** each (no firmware paints an image) | every task that touches `fulgurite` or `fulgurite-widgets` | the task stops and attributes the delta by symbol diff |

## T84.0 — the gates that compile today, at `1024cba` (2026-10-06)

Pins, green, each flipped by the task that closes its gap
(`docs/plan-iced-roadmap.md` §3.1).

| gate | prints today | flipped by |
|---|---|---|
| (a) `crates/fulgurite-shell/tests/image_frame.rs`: a store switches one id from red to blue; tick (answering `true`), pump, present | centre **red**, `damaged_px` **0** (the image's box is 1,600 px) | T84.2 |
| (b) `crates/fulgurite-wgpu/tests/image_frames.rs`: the same store, two full-damage draws (RTX 4070, Vulkan) | second draw's centre **red**; `resets` 0 | T84.2 |
| (c) `scripts/check_default_crates.sh` (`just static`): K2's count against the parent commit's | **96** at the work tree and at `1024cba`, Δ **0** | stays green |
| (d) `just firmware-delta 1024cba firmware=0 firmware_interactive=0 firmware_bind=0 firmware_cascade=0` | Δ **0** on all four (`firmware_interactive` 63,480 B) | — |

## T84.1 — the `image` feature: PNG and GIF decode into `ImageStore` (2026-10-06)

`fulgurite_shell::image`: `ImageStore` (unconditional), and behind the
shell's default-off `image` the `decode_png` and `decode_gif` that answer
a `DecodedImage` of composited RGBA8 frames.

**K84a — does not fire.** K2's command on `fulgurite-winit`:

| build | crates |
|---|---:|
| default | **96** |
| `--features fulgurite-shell/image` | **105** |
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

**Gates** (`crates/fulgurite-shell/tests/image_decode.rs`, run by
`just test` with `--features image`):

| gate | green | injection → red |
|---|---|---|
| PNG round trip, 7 × 5 RGBA | 7 × 5, 140 bytes; **0** of 140 bytes differ (bound 0) | alpha dropped (decoded as RGB): the decoder's own length check answers `Malformed("105 bytes of pixels for 7x5")`; with that check removed too, the test's "length" assertion fails at 105 against 140 |
| GIF compositing, 4 frames over 8 × 8, frame 2 a 2 × 2 at the top-left with `Keep` | delays `[100, 100, 100, 100]` (bound `[100; 4]`); frame 2 at (4, 4) = frame 1's green `[40, 160, 60, 255]` | each frame drawn on a cleared canvas (uncomposited): (4, 4) reads `[0, 0, 0, 0]` |
| every truncation is an error, never a panic | GIF: 133 prefixes, **0** panics, **0** decoded; PNG: 97 prefixes, 0 panics, 4 decoded (93–96), each the whole picture | `unwrap()` in the GIF frame loop: **98** of 133 prefixes panic (35–132) |
| default stays off | `scripts/check_default_crates.sh`: 96 against 96 at `55f3962`, Δ 0 | `image` in the shell's `default`: 105, Δ **9** (= K84a's count), exit 1 |
| lint and docs with the feature on | `cargo clippy -p fulgurite-shell --features image --all-targets -- -D warnings` and `cargo doc -p fulgurite-shell --no-deps --features image` with `-D warnings`, both clean | an unused binding in `decode_png`: the feature's clippy line fails, the default line stays green |

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

**Gates** (software: `crates/fulgurite-shell/tests/image_frame.rs`;
GPU: `crates/fulgurite-wgpu/tests/image_frames.rs` under `just gpu`,
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
benches-competitive/run.sh -p fulgurite-frame` under `just
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

## T84.3 — animated images: `Playback`, `App::wake_in_ms`, the showcase entry (2026-10-06)

`Playback` (core `anim`, behind `alloc`) holds the delays, the frame and
`playing`, and answers a new frame only when it changes; `App::wake_in_ms`
(provided, `None`; `#[app(wake = f)]` in the derive) is folded into
`Host::next_deadline_ms` through `Host::next_deadline_with`, and asked
guarded by `Local::next_deadline_ms` (and by `ui.waitSettled`). The
showcase's **Image** entry is the committed `examples/showcase/assets/spin.gif`
(four 96 px frames at 100 ms), decoded at boot into the `ImageStore` handed
to `Program::images` under `catalog::SPIN` (`ImageId(1)`), with a pause
button that flips `playing`.

**Gates, headless:**

| gate | green | injection → red |
|---|---|---|
| timing (`anim.rs` `playback_tests`): `[100; 4]` | starts at 0; `tick(0)` → `None`; four `tick(100)` → 1, 2, 3, 0; eight `tick(50)` answer `[1, 2, 3, 0]` | `tick` answers `Some` on every positive `dt`: eight `tick(50)` answer `[0, 1, 1, 2, 2, 3, 3, 0]`, "a frame was answered twice" |
| deadline (`shell/tests/image_wake.rs`) | 30 ms into a 100 ms frame: `Host::next_deadline_ms` **70.0** = `next_change_ms` 70.0 (bound ±0.01), `Local::next_deadline_ms` 70.0; paused: `None`, `None` | the fold drops the wake: `None` while playing, "the wake is not folded in" |
| a panicking wake (same file, `Recovering`) | deadline `None`, `failed` **true**; after retry the wake answers 100.0 | wake asked outside the guard: the panic escapes the call and the test dies on it; asked through `caught` (the plan's design): `failed false`, "the wake's panic was not recorded" |
| allocation (`alloc_gate.rs` `an_animated_image_allocates_nothing`) | 100 ticks, **100** frame changes (setup), 1 image command; **0** allocs, **0** frees (bound 0, 0) | `ImageStore::image` clones the frame: **100** allocs, 100 frees (1.00/frame) |
| the showcase wiring (`the_image_demo_plays_at_its_own_rate_and_its_button_flips_it`) | `tick(60)` → false, wake 40.0; `tick(40)` → true, frame 1; a press: label "Play", wake `None`; a second: "Pause", playing | the handler sets `playing = false`: the second press leaves "Play", paused, by name |

**Panic guard** (`just panic-drive`, both strategies): **nine** sites
(site 9, `App::wake_in_ms`, at `fixtures/panic-app/src/lib.rs:319`),
`--expect-panics` **10** (was 9); site 9 caught, the screen drawn, input
refused, retried, on both legs; the release build without `reload` still
dies. Injection, `Local::next_deadline_ms` asks the wake outside the guard:
the process dies at site 9 (exit 101, 9 panics against 10), the leg fails.

**Gates, live** (`scripts/widget_drive.sh Image`, all six legs green:
`Everything`, `Boundaries`, `Boundaries+oracle`, `Boundaries+scale2`,
`Everything+shaping`, `Boundaries+shaping`):

| action | green, every leg | injection → red |
|---|---|---|
| time advances the frame | over 1,000 ms the pixels moved; `presents` Δ **10** (bound 10 ± 1), `skipped` Δ **0** (bound ≤ 11); the damage region over the image **1.00×** its 9,216 px² box; isolate, paused: Δ **0**, pixels held | the tick writes the frame and answers `false`: `Boundaries` presents Δ **0**, skipped Δ 10, pixels held, red; `Everything` stays **green** (presents Δ 10, pixels moved) — the shape the four legs exist for. `next_deadline_ms` ignores the wake (`Boundaries`): presents Δ **0** while playing, pixels held |
| pause holds the frame | the frame held 500 ms after the press; the second press resumed it; the button's pixels changed; isolate (both presses outside): never resumed | the handler sets `playing = false`: "setup: the Image demo is not playing after its button" (the first action resumes first), and the keyboard pass's Enter reads paused → paused |

The first full runs read `presents` Δ 11 on every leg (the drive's own
hash request's redraw fell inside the count, which now opens 30 ms after
it) and then "held" on five legs at Δ 10 (twelve changes counting the gaps
either side, back on the first of four frames; a still-equal read is now
followed by one more 150 ms later). The figures above and below are the
final run's. The GPU half is not agent-observable (`ui.regionHash` reads
the software frame); it is T84.2's `fulgurite-wgpu` test under `just
gpu`, and re-running this drive on the GPU after M88/T88.2 is the reopen
condition.

**K84c — does not fire.** `app.perf`'s `layout_ns` on each presented
animated frame of the Image entry, the median of 50 (release, the
showcase's 1120 × 760 window):

| leg | median | p10 | max |
|---|---:|---:|---:|
| `Everything` | 29.73 µs | 27.20 | 118.24 |
| `Boundaries` | 28.47 µs | 23.83 | 49.21 |
| `Boundaries+oracle` | 32.31 µs | 21.23 | 49.09 |
| `Boundaries+scale2` | 25.72 µs | 12.96 | 45.43 |
| `Everything+shaping` | 28.96 µs | 15.19 | 52.12 |
| `Boundaries+shaping` | 31.91 µs | 16.57 | 50.19 |

Bound **100 µs** (about 4× `just grid`'s 2,227-node `mark_all` layout row,
23.15–23.60 µs, `docs/results-damage-precision.md`). The whole-tree mark
per animated frame is accepted as priced; `App::tick_marked` is not built.

**Firmware (K4).** `just firmware-delta a1ee7d6 firmware=0
firmware_interactive=0 firmware_bind=0 firmware_cascade=0`: Δ **0** `.text`
and **0** `.rodata` on all four (`firmware_interactive` 63,480 / 21,886 B).
`Playback` is behind `alloc` and `App::wake_in_ms` is a provided method no
firmware calls; `docs/results-embedded.md`'s table does not move.

## M84a's kill rules, at its close (T84.4, 2026-10-06)

Every rule M84a could read, with its number, fired or not (S5). The
milestone's firmware total is T84.0's base (`1024cba`) against T84.3's
head (`da63a7c`): the sum of the four task deltas, each printed by `just
firmware-delta <parent>` against its own parent. T84.4 changes no linked
code: `just firmware-delta da63a7c firmware=0 firmware_interactive=0
firmware_bind=0 firmware_cascade=0` prints **+0** `.text` and **+0**
`.rodata` on all four.

| rule | bound | measured | verdict |
|---|---|---|---|
| K84a — decoder crates | `image`'s own ≤ 15 | **9** (96 → 105, T84.1) | holds; PNG and GIF both ship |
| K84c — whole-tree mark per animated frame | median `layout_ns` ≤ 100 µs | **25.72–32.31 µs** across the six legs (T84.3) | holds; `App::tick_marked` not built |
| K84d — the atlas under animation | `resets` 0 over 100 changes | **0** (T84.2) | holds; no dedicated texture |
| K4 — per task | Δ 0 B `.text` on all four (strict form) | T84.0 **+0**, T84.1 **+0**, T84.2 **+0**, T84.3 **+0** | holds |
| — M84a's total | recorded | **+0** on all four; `firmware_interactive` **63,480** B (headroom 18,440 B to 81,920) | recorded; `docs/results-embedded.md`'s table does not move |
| K3 — the frame arm | each row inside the base's eight-run spread | all five named rows inside (T84.2) | holds |
| K1 — the struct budget | `Cmd` 60; `Style`, `NodeState` +0 B | `Cmd` **60** (payload 28 B, T84.2); `Style` and `NodeState` untouched; embedded `Ui` 1,216 B unchanged | holds |
| K2 — default-off | the default window's crate count Δ 0 | **96** against 96 at every task (T84.0(c), T84.1) | holds |
| K84b, K84e | — | M84b's (T84.9, T84.8) | not yet read |

**Carried to M84b, as planned**: T84.2's *origin* gate, the atlas key's
`origin` and `Gpu::invalidate_images()` arrive with the application's
store (T84.6); T84.0(e), the subscription-stop pin, is T84.5. The GPU
half of the live Image drive reopens after M88/T88.2 (T84.3).

## T84.5 — M84b's gate that compiles today, at `15d4aec` (2026-10-06)

A pin, green, flipped by T84.7 (`docs/plan-iced-roadmap.md` §3.1). It is
M84b's only gate that compiles before M84b's code: the others name
`FrameSource::images`, `App::images`, `ImageChain`, `Video` or
`VideoFeed`, none of which exists yet (`docs/plan-images-in-motion.md`,
"What building found").

| gate | prints today | flipped by |
|---|---|---|
| `crates/fulgurite-shell/tests/subscriptions.rs`, the pin a_stopped_subscription_keeps_delivering_today (renamed `a_stopped_subscription_stops` by T84.7, `docs/results-tasks.md` T84.7): a ticker sends every 10 ms under `Threads`; the application un-declares it when the first message lands; the next frame's diff stops it, `subscription_counts()` (1, 1); then 20 frames 10 ms apart | after the stop: **20** messages applied (pinned > 0), **20** job iterations (pinned > 1) | T84.7: 0 messages, 0 iterations (bound ≤ 1), measured |

Perturbations, each a miniature of what T84.7 builds, put in
`Host::diff_subscriptions` where it stops a subscription and then
removed:

| perturbation | prints | fails |
|---|---|---|
| the drain drops what the stopped job sends (`Inbox::cancel` at the stop) | **0** messages, **20** iterations | "pinned (T84.5): a stopped subscription's messages are applied after the stop -- 0 were" |
| the job is told to end (the host replaces its inbox at the stop, so the next send answers `Disconnected`) | **0** messages, **1** iteration | the messages pin first; with it set aside, "pinned (T84.5): a stopped subscription's job runs on -- 1 iterations after the stop" |

The first perturbation is T84.7's injection 2 seen from the other side:
dropping the messages alone leaves the job running, which is why the pin
counts iterations as well as messages.

**Firmware (K4).** `just firmware-delta 15d4aec firmware=0
firmware_interactive=0 firmware_bind=0 firmware_cascade=0`: Δ **0** on
all four — the task adds a test and no library code.

## T84.6 — `FrameSource::images` and `App::images` (2026-10-06)

The application's store reaches the renderer through the frame: `App::images`
(provided, `None`; `#[app(images = f)]`), asked on every present through the
required `FrameSource::images`, resolved before the window's store
(`ImageChain`). Every gate below was run red under its injection and green
restored in the same session.

| gate | setup asserted | printed (green) | injection | printed (red) |
|---|---|---|---|---|
| compile: `Hot` / `Local` without `images` | — | the forward is required (S1, E0046) | — (the compiler is the gate) | — |
| headless write (`crates/fulgurite-shell/tests/app_images.rs`): `apply` calls `ImageStore::replace`, paints the frame it answers | boot centre red; `apply` ran once; `frame` 1 → 2 | centre blue; damaged **1,600** px against the image's **1,600** px box: **1.00×** (bound 1.00×); neighbour page | the host's `FrameSource::images` answers `None` | centre the page colour `[250, 250, 250]` at boot, failed by name |
| order (same file): id 1 in both stores, id 2 in the window's | the derived application answers a store | id 1 red (the application's), id 2 green (the window's alone still draws) | `ImageChain::resolve` asks the window's store first | id 1 blue, failed by name |
| panic (same file): `App::images` panics once in a `Local`'s frame | a healthy session hands the store over | `failed` true; `images()` `None` while the screen is up; `Some` after the retry | (1) the frame's guarded entry does not ask `App::images`; (2) `Local::images` forwards regardless of the failure | (1) `failed` false — the present's catch swallowed it and recorded nothing; (2) `Some` while the screen is up |
| GPU swap (`crates/fulgurite-wgpu/tests/app_images.rs`, `just gpu`): store A red, then store B blue under the same `(id 1, frame 0)`, `invalidate_images()` between | the first draw is red | blue; `image_uploads` **2** | `Gpu::invalidate_images` does nothing | red (v1's), `image_uploads` 1 |
| GPU origin (same file, T84.2's gate carried here): id 1 red in the application's store, blue in the window's; drawn with, then without, the frame's store | the application's red wins | blue after the fall-through; `image_uploads` **2** (one slot per origin) | the key's origin is always `Window` (the id alone) | red — the application's slot hit, `image_uploads` 1 |
| across a swap (`just hot-window-drive`, hot leg): `#pic` from `App::images`, the store's colour edited with the label's format in one write | the first hot build's colour `(22, 122, 60)` | after the swap **(29, 78, 216)**, the new build's | `Hot::images` answers `None` | `#pic` the page `(255, 255, 255)` at step 1, failed by name |
| under the bar (same leg): `#pic`'s `ui.regionHash` before the broken edit and under the bar | the bar is shown (the drive asserts it, M37) | `90500717c9597325` both, over 10,240 px | `Barred::images` answers `None` | `a556430ac2dc4325` under the bar, centre the page colour, failed by name |
| the window's swap call, GPU (`just hot-window-drive`, `gpu` and `gpu-everything` legs, read with `xwd`): `#pic`'s colour edited with the WGSL colour in one write; its frame stays 0 | `#pic` is v1's `(22, 122, 60)` on the GPU before the edit (distance 0, bound 6) | after the swap **(29, 78, 216)** on both legs, distance 0 from v2's | the swap branch does not call `Backend::invalidate_images` | `(22, 122, 60)` after the swap, distance 156 from v2's, failed by name |
| stamp, accept (`just abi-stamp`): `stamp-lib --features image` under the plain shell loader | — | `accepted: 20 fields agree`; the M49 half still refuses the reordered `Cmd` | `image` added to `abi_fields`' feature list | `refused ... fulgurite-shell features is reload,truetype here and reload,truetype,image in the library` |
| panic guard (`just panic-drive`, both strategies): site 10, `App::images`, armed by its click | ten `// SITE n` markers | site 10 at `fixtures/panic-app/src/lib.rs:335`, screen drawn, input refused, retried; **11** panics (expected 11) on each leg | neither guarded entry (the frame's, the agent pump's) asks `App::images` | the agent's present read the host's store unguarded: the app died (exit 101) at site 10, 10 of 11 panics logged |
| resume (`just resume-drive`) | — | green on every leg: the window-side source still round-trips (gpu-resume 0 differing pixels) | — (unchanged gate, M47/T47.7) | — |

**The swap's call site is gated live; the retry's is not.** The plan expected
neither to be observable before M88/T88.2, because `ui.regionHash` cannot
read the GPU (T84.3's note), but `just hot-window-drive`'s GPU legs already
read the window with `xwd` (M83/T83.8), so `#pic` is read there and the
swap's `Backend::invalidate_images` is the row above. The retry's call has
no GPU leg: `just panic-drive` runs on the software backend, where the call
is the no-op. Its reopen condition is a GPU leg of the panic drive.

**Firmware (K4).** `just firmware-delta 18e2515 firmware=0
firmware_interactive=0 firmware_bind=0 firmware_cascade=0`: Δ **0** `.text` and
**0** `.rodata` on all four (`firmware_interactive` 63,480 B). `App::images`
is a provided method no firmware calls, and `App` is never a trait object;
`docs/results-embedded.md`'s table does not move.

## T84.8 — `Video<S>`, `VideoFeed`, `GifFeed` (2026-10-06)

A controlled widget over a feed, not a codec: `Video<S>` shows the
application's `(id, frame)` with a play/pause button, a seek track and the
time; a `VideoFeed` run as a subscription by `video_job` sends each frame
as the application's message, and `apply` copies it into the
application's `ImageStore`. The seek's fraction reaches `on_seek` through
`Node::take_action` (`docs/plan-images-in-motion.md`, "What building
found", T84.8). Every gate below was run red under its injection and green
restored, on a clean build, in the same session.

**Headless** (`crates/fulgurite-shell/tests/video.rs` unless named):

| gate | setup asserted | printed (green) | injection | printed (red) |
|---|---|---|---|---|
| seek fraction (P19): a pumped click at the track's midpoint | handled; `on_seek` ran twice (press and release) | **0.5**, 0.5 on a **108** px track (bound 0.5 ± 0.0093) | the track's width is the video's (`SeekTrack::layout`) | **0.16875**, failed by name |
| a drag 25% → 50% | the press sought | after the press [0.25], after five moves [0.25], after the release [0.25, **0.5**] | the release does not seek | [0.25]: "the release did not seek" |
| a press on the picture (the plan's isolate) | — | handled **false**, 0 toggles, 0 seeks; accept: the play button is handled and toggles once | `Video::handler` answers `on_toggle` for the picture | handled true, 1 toggle |
| a reader's actions on the video; the button's label | both actions handled | default action → 1 toggle; `SetValue(0.3)` → seeks [0.3]; label "Play" paused, "Pause" playing | (a) the pass ignores `take_action`; (b) the layout never writes the label | (a) 0 toggles, seeks []; (b) ("Play", "Play") |
| pacing: a `GifFeed` at 100 ms under `Threads` | 21 frames in 4 s | gaps over the first 20 **99.5–100.9** ms (bound 100 ± 10); `Inbox::refused` **0** | `video_job` does not sleep to `pts_ms` | refused **11,778**, gaps 0.0 ms |
| `GifFeed` opens where its key says | — | from 300 ms: frames 3, 0, 1, 2, 3, 0 at 300…800 ms; from 250 ms frame 2, then 3 at 300 | the key's start is ignored | from 300 ms: frame **0**, "a start at 75% shows frame 3" |
| UI-thread allocations per video frame (`tests/alloc_gate.rs`) | 50 frames applied; the list paints the newest | **0** allocations (bound 0); **50** frees, 1.00 per frame (bound 1: the message's buffer) | `ImageStore::replace` copies into a new `Vec` | **50** allocations, failed by name |
| the derive's refusals (`fail_app_{msg_without_apply,apply_without_msg,jobs_without_msg}.stderr`) | — | each refused by name; the showcase and these tests are the accept direction | `msg` without `apply` accepted | `fail_app_msg_without_apply.rs` compiles, trybuild fails it |
| role table (`crates/fulgurite-widgets/tests/a11y_roles.rs`) | — | `Video`: role `Video`, label, value "m:ss / m:ss", published Click and SetValue, **2 of 2** handled; `#video-track`: `Slider` "Seek", **4 of 4** | delete the `Video` row | "Node impls with no row in the table: [Video]" |

**Live** (`just widget-drive Video`, `scripts/drive/widgets/video.py`, all
six legs: `Everything`, `Boundaries`, `Boundaries+oracle`,
`Boundaries+scale2`, and the two `+shaping` legs). Each injection was run
on the `Boundaries` leg with its own action first, since a broken stop or
seek also breaks the pause the others end on:

| action | printed (green, every leg) | injection | printed (red) |
|---|---|---|---|
| play advances the picture | the picture changed **151–157** ms after the press (bound 250), readout playing | `on_toggle` not called (the play button's handler `None`) | "held for 250 ms … not playing" |
| pause holds the picture | the button read "Pause" then "Play"; the picture held over **500** ms | the label ignores `playing` | "'Play' then 'Play'" |
| a press at 75% of the track seeks there | `on_seek` **0.75** (bound ± 0.0093), position **300** ms (bound 300 ± 100), frame **3** (⌊0.75 · 4⌋) | the fraction against the video's box (`SeekTrack::layout`) | **0.253**, 101 ms, frame 1 |
| a drag 25% → 50% while playing | on release **0.5**, position **200** ms (bound 200 ± 100); frames from a stopped feed over the next 400 ms **0** | (planned) the drain's stop check removed | **green**, 0 stale: cannot fire live (the plan's "What building found", T84.8) |
| | | the host never sets the stop flag | **15** stale frames in 400 ms, by name |
| a press on the picture changes nothing (static) | handled False, readout and picture unchanged | `Video::handler` answers `on_toggle` for the picture | handled True, readout "playing" |

The keyboard pass reaches the play button in one Tab, and Enter plays and
Enter pauses it; the reader pass finds three leaves on the demo and none
silent; M57's D5 parity skips the entry (animated), as it skips Image.

**K84e** — `cargo run --release -p benches --features image --bin
video_rate`: a 1280 × 720 `GifFeed` at 30 frames/s on the software
backend presented **301 frames in 10.00 s = 30.1/s** (bound ≥ 28; per
second 30–31), every frame applied and none refused, at **11.98 ms** of
raster per present. **Holds**: no size limit is stated.

**Firmware (K4).** `just firmware-delta 479efa9 firmware=0
firmware_interactive=0 firmware_bind=0 firmware_cascade=0`: Δ **0** `.text`
and **0** `.rodata` on all four (`firmware_interactive` 63,480 B).
`Node::take_action` and the pass's route are behind `std`, which no
firmware takes. Ungated, the route cost `firmware_interactive` **+488 B**
(**+320** out of line with the ctx by reference: `run_taken` 184,
`dispatch` +108, 25 default `take_action`s 4 B each; `.rodata` +44),
which this table's K4 row would have stopped the task at.
`docs/results-embedded.md`'s table does not move.

**K1, K2.** `Cmd`, `Style` and `NodeState` are untouched; the default
window's crate count is **96** against 96 at `479efa9`
(`scripts/check_default_crates.sh`), since `image` stays off by default.

## T84.9 — the K84b probe, at `11c7aa2` (2026-10-06)

A scratch manifest under `target/` (never committed, deleted after
T84.10) naming `gstreamer`, `gstreamer-app` and `gstreamer-video` 0.24.5
beside a path dependency on `fulgurite-shell`, resolved against this
workspace's lock, and one `src/main.rs` carrying `#![forbid(unsafe_code)]`
with a `GstFeed: VideoFeed` over `appsink`: caps set to RGBA, each sample
mapped with `gstreamer_video::VideoFrame::from_buffer_readable` and its
plane 0 copied row by row at its stride into `VideoFrame::rgba`.

| K84b condition | bound | measured | verdict |
|---|---|---|---|
| the dependency set, K2's form (`cargo tree -e normal --prefix none \| sed 's/ (\*)$//' \| sort -u \| wc -l`) | ≤ **150** crates | **58** besides the probe (59 lines); **50** of them new over the shell's 8; **66** with build edges (`system-deps`, `pkg-config`, `cfg-expr`, `toml`, `version-compare`, `target-lexicon`, `serde_spanned`, `autocfg`) | holds |
| `unsafe` in our code for `appsink` → RGBA | **none** | **0**: the feed type-checks under `forbid(unsafe_code)` (`cargo check`, with the pkg-config step bypassed, below). Injection: the stride read as `unsafe { *frame.plane_stride().get_unchecked(0) }` → "error: usage of an `unsafe` block", by name; restored → `Finished`. The bindings hold their own `unsafe` (4,224 occurrences across the three crates' `src`), as `winit` does; K84b's condition is the crate's own | holds |
| the GStreamer development libraries on the reference machine | **present** | **absent**. `pkg-config` finds none of `gstreamer-1.0`, `-base-1.0`, `-app-1.0`, `-video-1.0` (it finds `glib-2.0` and `gobject-2.0` 2.80.0). The runtime is installed — `libgstreamer1.0-0` 1.24.2, plugins base/good/bad/ugly/libav, `gst-launch-1.0` — but no `-dev` package: no `.pc` file and no unversioned linker name (`libgstreamer-1.0.so`). Nothing was installed | **fires** |

**What a build does on this machine.** `cargo build` stops in
`gstreamer-video-sys`'s build script: "The system library
`gstreamer-video-1.0` required by crate `gstreamer-video-sys` was not
found". With `system-deps`'s pkg-config bypass
(`SYSTEM_DEPS_<NAME>_NO_PKG_CONFIG=1` and `_LIB`) `cargo check` passes —
which is how the `unsafe` row was read — and `cargo build` fails at the
link, "unable to find library -lgstvideo-1.0" and the other three. With
linker-name symlinks to the runtime `.so.0` files in the probe's own
directory (`_SEARCH_NATIVE`) it links and runs: one `videotestsrc` frame,
**320 × 240, 307,200 bytes** = w·h·4. So the path is sound and only the
machine's packages are missing — but a gate that needs hand-made symlinks
and four environment variables is not one `just test` or CI can run, so
its gate could only skip, which is the condition as written.

**The pinned toolchain needs an older `kstring`.** `gstreamer` 0.24.5
takes `kstring`, whose 2.0.5 requires rustc 1.96 against this workspace's
1.94.0; the MSRV-aware resolve (`rust-version = "1.94"`,
`CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback`) takes 2.0.2. The
crate counts above are from that resolve (the count is 59 either way).

**K84b fires — on the third condition only.** T84.10 takes its second
branch. The probe changes no tracked file but this one: firmware Δ 0 by
construction.

## T84.10 — `fulgurite-video`, deferred (2026-10-06)

**K84b fired (T84.9), so T84.10 takes its second branch**: the crate is
not added, and `Video` ships with `VideoFeed` and `GifFeed` (T84.8).

| K84b condition | bound | measured (T84.9) | |
|---|---|---|---|
| dependency set | ≤ 150 | **58** (66 with build edges) | holds |
| `unsafe` in the crate | none | **0** | holds |
| development libraries on the reference machine | present | **absent** (runtime 1.24.2 only) | **fires** |

**What it would have cost**, measured: 50 crates over the shell (58 in
the crate's own set), eight more build-time crates for the `pkg-config`
step, four system libraries at link time, and `kstring` held below its
latest for the pinned toolchain. None of it reaches a default build: the
crate was planned default-off and outside the reload source hash.

**Reopen condition.** The plan's clauses — "a GStreamer binding below
150 crates with a safe `appsink`, or a pure-Rust decoder of one common
codec" — are kept, and the first is **already met** by `gstreamer`
0.24.5. What blocks the crate is the machine, so the operative
condition is: **the reference machine has `libgstreamer1.0-dev` and
`libgstreamer-plugins-base1.0-dev`** (so `pkg-config --modversion
gstreamer-video-1.0 gstreamer-app-1.0` answers), installed by the owner,
never by a task. Then T84.10's first branch is built as the plan writes
it: `GstFeed: VideoFeed`, default-off, `forbid(unsafe_code)`, its
`videotestsrc` gate at 30 ± 1 frames/s with a seek inside one frame, and
the unset-caps injection. Or: a pure-Rust decoder of one common codec,
which needs no system library and so no such install.

**Firmware.** No code is linked by any firmware; the one source change is
`video.rs`'s module rustdoc, in the shell.

## M84b's kill rules, at its close (T84.11, 2026-10-06)

Every rule M84b could read, with its number, fired or not (S5). The
milestone's firmware total is T84.5's base (`15d4aec`, M84a's close)
against T84.10's head (`0bf7103`), measured directly rather than summed:
`just firmware-delta 15d4aec firmware=0 firmware_interactive=0
firmware_bind=0 firmware_cascade=0` prints **+0** `.text` and **+0**
`.rodata` on all four. T84.11 changes no linked code: the same invocation
against `0bf7103` prints **+0** on all four too.

| rule | bound | measured | verdict |
|---|---|---|---|
| K84b — the GStreamer crate | ≤ 150 crates; **no** `unsafe`; the development libraries present | **58** crates (66 with build edges); **0** `unsafe` under `forbid`; libraries **absent** (runtime 1.24.2 only) (T84.9) | **fires, on its third condition only**; `fulgurite-video` deferred with its reopen condition (T84.10); `Video` ships with `VideoFeed` and `GifFeed` |
| K84e — video on the software backend | ≥ 28 presented per second at 1280 × 720, 30 frames/s | **30.1/s** (301 in 10.00 s; 11.98 ms raster per present) (T84.8) | holds; no size limit is stated |
| K4 — per task | Δ 0 B `.text` on all four (strict form) | T84.5 **+0**, T84.6 **+0**, T84.7 **+0**, T84.8 **+0** (ungated, the `take_action` route had cost `firmware_interactive` +488, then +320, before it went behind `std`), T84.9 **+0**, T84.10 **+0**; the two fixes landed inside M84b, `553ac3c` and `11c7aa2`, **+0** and scripts only | holds |
| — M84b's total | recorded | **+0** on all four (`15d4aec` → `0bf7103`); `firmware_interactive` **63,480** B (headroom 18,440 B to 81,920) | recorded; `docs/results-embedded.md`'s table does not move |
| K3 — the task's named arm | inside the parent's eight-run spread | T84.7, `benches --bin spawn -- pool`: median **2,505** ns against the parent's 1,747.70–3,064.70 (`docs/results-tasks.md` T84.7); no other M84b task names a K3 arm | holds |
| K1 — the struct budget | `Cmd` 60; `Style`, `NodeState` +0 B | untouched by every M84b task (T84.8); `JobTrack`'s stop flag sits in existing padding, size unchanged (T84.7) | holds |
| K2 — default-off | the default window's crate count Δ 0 | **96** against 96 at `479efa9` (T84.8, printed), and `scripts/check_default_crates.sh` in `just static` at every task | holds |
| K84a, K84c, K84d | — | M84a's ("M84a's kill rules, at its close") | held at T84.4 |

**What M84b added to the framework outside its plan's Changes lists**,
each recorded where it was built: `Node::take_action` and the pass's
route, in the core behind `std` (T84.8), the same kind of hook as
`activation_redirect` and `selects_on_focus`, changing no other
milestone and no owner decision; and the derive keys `#[app(msg = T,
apply = f)]`, `#[app(subscriptions = f)]` and `#[app(jobs = f)]` (T84.8),
whose three refusals are pinned by `.stderr` files.

**Carried out of M84, as planned**: progress after stop (M11) is
M88/T88.1's, since its progress slot has not landed (T84.7); the GPU
half of the live Image drive and the retry's `Backend::invalidate_images`
call reopen with a GPU-readable `ui.regionHash` (M88/T88.2) and a GPU leg
of the panic drive (T84.3, T84.6); `fulgurite-video` reopens on the
condition T84.10 writes.
