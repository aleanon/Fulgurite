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
the software frame); it is T84.2's `lumen-static-wgpu` test under `just
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
| `crates/lumen-static-shell/tests/subscriptions.rs` `a_stopped_subscription_keeps_delivering_today`: a ticker sends every 10 ms under `Threads`; the application un-declares it when the first message lands; the next frame's diff stops it, `subscription_counts()` (1, 1); then 20 frames 10 ms apart | after the stop: **20** messages applied (pinned > 0), **20** job iterations (pinned > 1) | T84.7: 0 messages, ≤ 1 iteration |

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
