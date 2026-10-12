# M84 — images in motion: decoding, animated images, video

> **Written 2026-09-28 as a section of `docs/plan-iced-roadmap.md`;
> revised 2026-09-29 after that plan's seven-lens review, and moved to
> its own file. M84a (T84.0–T84.4) built 2026-10-06 and closes on
> `just ci-verify` of its T84.4 commit (S5), cited in AGENT.md; M84b
> (T84.5–T84.11) built 2026-10-06 and closes on `just ci-verify` of its
> T84.11 commit (S5), cited in AGENT.md; K84b fired on its third
> condition, so `fulgurite-video` is deferred (T84.10,
> `docs/results-images.md`).** The shared byte inventory, seams (S1,
> S2), test and doc discipline, and plan-wide kill rules K1–K4 are in
> `docs/plan-iced-roadmap.md` §2–§4 and bind this plan; this file does
> not restate them.

*Size **L, split in two**. **M84a (M)**: decoding, an image's version
carried in the command, the GPU atlas rewriting a slot in place, and
animated images (T84.0–T84.4). **M84b (L)**: an image store the
application owns and the renderer reads through the frame, cooperative
stop for subscriptions, the `Video` widget, and the decoder crate or its
deferral record (T84.5–T84.11). Closes iced's "Animated images" and
"`Video` widget" (§1.2), and `docs/plan-capabilities.md` T15.2's
decoding half, which M54/T54.9 recorded as not built. **Depends on**
nothing in M80–M90; M87's images want M84a first. M84b depends on
M84a. Read against `553fa5e` (the merge directory's HEAD when this was
drafted; line numbers below are that tree's).*

The first draft could not be built. It kept the command as it was and
put the version in the store (`ImageSource::generation`), but damage is
decided from the command stream. `Recording::image` hashes rect, id and
tint (`retain.rs:1018-1023`). So a version bump produced an identical
list, `Damage::None`, and no present on either backend. The store also
moves into the backend as a `Box` (`program.rs:306-310`, winit
`lib.rs:1529`, `backend.rs:68-85`), so nothing in `App::tick` or
`App::apply` could reach it. And the GPU atlas has no per-key eviction
(`atlas.rs:59-114`). The design below puts the version **in the
command**, which fixes damage and the atlas together. It gives the
application a store **it owns**, which the renderer borrows through the
frame each present.

**Decisions.**

- **An image's version is in the command.**
  `Cmd::Image { rect, id, tint, frame: u32 }` (payload 24 → 28 B; `Cmd`
  stays 60 B, asserted at `tests/glass_cmd.rs:15`).
  `Painter::image(rect, id, tint, frame)` stays provided as a no-op:
  that degraded default is its documented contract (`display.rs:840-846`).
  `ImageSource::image(&self, id, frame)` takes the frame, so the source
  can answer for it. `frame` is **which version of this surface the
  command draws**. A sequence (a GIF) reads it as an index, and the store
  answers `frames[frame % len]`. A stream (video) reads it as a counter
  and answers its latest pixels. A still image paints 0. Crate:
  `fulgurite` (core, `no_std`).
  - The consumers, in full: `Recording::image` (the fold that decides
    staleness, `retain.rs:1018`); `Cmd::hash_into`
    (`display.rs:716-729`, the journal's `hash_from`); `wire.rs`
    (`VERSION` up by one from the current version, expected 4 → 5 after
    M80's bump and M82's two (T82.1 for `Stops::at`, T82.2 for tag 6's
    kind/geom/corner), because tag 7's fields change meaning,
    `wire.rs:31-33`); `svg.rs` (`data-frame`); the software `Canvas`
    (`software.rs:1334`); the GPU `image_slot` (`lib.rs:1330-1363`);
    `contrast.rs:122` (pattern unchanged).
  - The callers of `Painter::image`, each passing a frame: `Image`
    (`image.rs:146`). M87's `Markdown` call does not exist yet when
    T84.2 lands (§5: M84/T84.3 before M87/T87.4), so **T87.4 owns it**
    and writes `Painter::image(rect, id, tint, 0)`, a still (R20).
  - The painters: `DisplayList` (leaf); `Scaled`, `Deferring` and
    `Recording` (wrappers, which all override `image` today); and
    `Checksum` (`fixtures/embedded`) and `Sink` (`benches` `retain`),
    which inherit the no-op.
  - The `ImageSource` implementors that change signature:
    `ImageStore` (new), `Checker` (`tests/clip_test.rs`), `Card`
    (`tests/image_pixels.rs`, wgpu `tests/parity.rs`), `OnePixel`
    (winit `lib.rs:1865`) and `Wallpaper` (`examples/glasswall`).
  - **Budget:** `Style` and `NodeState` +0 B. `Image` (the widget) +4 B
    for `frame`; it lives in the tree struct, not per node, and no
    firmware `Ui` holds one. Firmware `.text`/`.rodata` are expected to
    stay +0 B, because no firmware paints an image. T84.2 measures this
    against the parent commit.
- **The GPU atlas rewrites a slot in place.** It keeps
  `image_frames: HashMap<ImageKey, u32>` beside `image_atlas`, holding
  the frame each slot holds. If a command's frame differs and the size
  is unchanged, `write_texture` goes into the existing `Slot`. If the
  size changed, the image is inserted again, and the orphaned space is
  reclaimed by the existing `Overflow::Image` reset. `Atlas` gains no
  eviction. `AtlasStats` gains `image_uploads` and `image_upload_bytes`
  (+16 B, `lib.rs:298-305`). Crate: `fulgurite-wgpu`.
  - **The key is `ImageKey { origin, id }`**, `origin` being the
    application's store or the window's (M84b; `Window` only in M84a).
    An id that falls through from one store to the other — while
    `images()` answers `None` on M36's panic screen — then misses
    instead of hitting the other store's pixels (A14).
  - **One list may paint one id at two frames** (M87/T87.4 and the
    showcase can place a registered id twice). Every `write_texture`
    lands before the frame's `submit` (wgpu `lib.rs:1560-1580`), so a
    rewrite in place would leave both quads sampling the last upload,
    where the software `Canvas` resolves each command
    (`software.rs:1339-1343`) and draws both. So `image_slot` records
    the frame it wrote to each key **this draw**; a second command at a
    different frame takes a side slot keyed `(ImageKey, frame)` in
    `image_alt`, which the same `Overflow::Image` reset reclaims (P1).
    The common case, one version per id, keeps the in-place write.
  - **The atlas is invalidated whenever the store behind a key may
    have been replaced.** The atlas lives in the loader-side backend
    across a swap (winit `Win.backend`, `lib.rs:597-598`), and a
    swapped-in v2 boots a fresh store that can answer the same
    `(id, frame 0)` with new pixels, so the slot would hit and keep
    v1's. `Gpu::invalidate_images()` resets `image_atlas` and clears
    `image_frames` and `image_alt`, as `set_images`/`take_images`
    already reset the atlas (`lib.rs:755-764`). The window calls it on
    a successful swap (beside `reloaded`, winit `lib.rs:813`) and on an
    M36 retry. M83 keys its pipelines on content for the same reason
    (`docs/plan-gpu-extension.md`) (A14).
- **Decoding is a default-off shell feature, `image`**, using the pure
  Rust `png` and `gif` crates. Neither is in `Cargo.lock` today, and
  their count is measured under K84a. It adds `decode_png` and
  `decode_gif` → `DecodedImage { w, h, frames, delays_ms }`, and GIF
  frames are composited to full RGBA at decode time (disposal applied).
  `ImageStore` (an `ImageId` → frames map implementing `ImageSource`,
  plus `replace` for streams) is **not** behind the feature: it needs no
  decoder. Crate: `fulgurite-shell` (`std`).
  - The feature gates crates and compile time only. It does not gate
    linked bytes: the linker drops a decoder no code calls whatever the
    feature says. That is M30's finding
    (`docs/results-widget-crate.md`), and it is why B7's symbol check is
    not adopted.
  - `image` is not added to `abi_fields`. Nothing it adds crosses the
    seam (see "What the review found", A25).
- **Animation is a clock in the application, and the widget holds only
  the frame.**
  - `Playback` (core `anim`, behind `alloc`, beside `Animated<T>`) holds
    the delays, the elapsed time and a `playing` flag.
    `Playback::tick(dt) -> Option<u32>` answers a new frame index only
    when the frame changes. `Playback::next_change_ms()` says when that
    will be.
  - The application's `App::tick` writes `ui.img.frame` and returns
    `true` only on a change. That is `App::tick`'s contract: a tick that
    writes the tree returns `true` (`event.rs:1216-1227`).
  - **There is no `AnimatedImage` type.** An animated image is an
    `Image` whose `frame` moves. A second widget would duplicate
    `Image`'s layout, fit, tint and `observe` for one field.
- **An animation wakes the window at its next frame, not at vsync.**
  `App::wake_in_ms(&self) -> Option<f32>` is provided and answers
  `None`. `Host::next_deadline_ms` (`lib.rs:2254`) folds it in beside
  timers, `scroll_end` and `linger`, and the window already waits on
  that deadline (winit `lib.rs:848`).
  - The derive gets `#[app(wake = fn)]` beside `tick =`, and its
    unknown-key error (macros `lib.rs:2197`, "expected `after_update =
    path` or `tick = path`") lists `wake` too; T84.6 adds `images` the
    same way (M23, P3).
  - **`Local::next_deadline_ms` is guarded** (second review A3). It
    calls `self.host.next_deadline_ms()` unguarded today
    (`session.rs:662-664`), which was safe while the deadline read only
    host state; folding `wake_in_ms` in makes it application code. It
    goes through `Local::caught` with fallback `None`. `caught` hands
    out `&Host` and `Host::next_deadline_ms` takes `&mut self`
    (`lib.rs:2254`), so the application half is split out: `Local`
    reads `self.caught(None, |h| h.app.wake_in_ms())` and passes it to
    `Host::next_deadline_with(&mut self, wake)`, which runs no
    application code; `Host::next_deadline_ms` stays, as
    `next_deadline_with(self.app.wake_in_ms())`, for headless callers
    and T84.3's deadline gate.
    Fallback `None`: a panicking `wake_in_ms` raises M36's screen,
    which is static and needs no deadline, and a retry (R,
    `app.retry`) is input, which wakes the window anyway; the host's
    own timers still fold in.
  - Without the deadline, a tick answering `false` idles the window for
    good. If `tick` instead answered `true` on every vsync, it would mark
    the whole tree 60 times a second (`lib.rs:2231-2235`) for 10 frames
    of change.
  - `App` has no wrappers (`git grep "App for"` finds none generic), so
    a provided method is safe here, unlike on `Painter`.
- **The whole-tree mark per animated frame is accepted and priced.**
  Under `Boundaries`, a tick answering `true` with `dt > 0` marks
  everything (`lib.rs:2231-2235`). Pixel damage stays the image rect
  (retention decides that), and layout re-runs. T84.3 prints the
  showcase's layout time per animated frame. K84c builds a narrow path
  (`App::tick_marked`, shaped like `App::update_marked`) only if that
  number exceeds its bound.
- **(M84b) The application owns a mutable store, and the renderer
  borrows it through the frame.**
  - `FrameSource` ("what a renderer reads from a frame",
    `session.rs:44-52`) gains a **required**
    `fn images(&self) -> Option<&dyn ImageSource>`.
  - `Host` answers it with `App::images(&self)`, which is provided and
    answers `None` (core). `Local` and `Hot` forward it. A derived
    application supplies it through `#[app(images = path)]` (the derive
    accepts only `after_update` and `tick` today, macros
    `lib.rs:2186-2200`, and `fixtures/hot-window` derives `App`, P3).
  - **`Barred` forwards it to the session it was built from.** `Barred`
    (winit `lib.rs:1695-1750`) is not a separate screen: it is the
    running session's list, copied, with the bar over it, and the last
    good build keeps running under it (rule 2, `lib.rs:1692-1693`).
    Answering `None` would blank every application-store image whenever
    the bar is up, and the bar's whole-surface damage repaints them all.
    So `Barred<'a>` keeps the `&'a mut dyn Session` that `Barred::new`
    already takes and answers `images()` from it (A13, P2). Its list
    copy is a named-field literal (`lib.rs:1709`) today; whichever of
    M82/T82.5 and M83/T83.3 lands first derives `Clone` on
    `DisplayList` and has `Barred` copy with `src.clone()`, so every
    side store (M82's `paths`, M83's `uniforms`, M84's own if it adds
    one) travels with the commands without a field in the literal.
    `..Default::default()` is the wrong fix: it compiles and leaves the
    store empty while the commands still index into it.
  - The renderers resolve each id through the frame's store first and
    then through the window's own (`Program::images`), in that order.
    `ImageChain` (shell) is the two-source lookup `Canvas`, `Gpu` and
    `Offscreen` take.
  - No `Arc` and no lock: the application writes the store in
    `apply`/`tick` with `&mut`, and the renderer borrows it `&` during
    `present`. Those never overlap, because both run on the frame
    thread. That is also why `image()` can keep returning a slice
    borrowed from `&self`.
  - Across a hot swap the loader never holds the pointer: it re-asks the
    session each present. That satisfies rule 3
    (`docs/plan-program.md`). A hot build's assets therefore reach the
    renderer, which the window-side `Program::images` cannot do (it is
    read once, winit `lib.rs:1529`). On the GPU that holds only because
    the swap calls `Gpu::invalidate_images()` (above, A14); without it
    the loader-side atlas keeps v1's pixels for an unchanged
    `(id, frame)`.
  - A session showing M36's panic screen answers `None`.
  - **`Local::images` is guarded** (second review A3): `App::images` is
    application code, so `Local` asks it through `self.caught(None,
    |h| h.images())`, exactly as M80's `Local::caret_rect` and today's
    `copy` (`session.rs:450,561`). Fallback `None`: a panicking
    `images` paints the window's store only and raises M36's screen,
    which answers `None` anyway. Unguarded, the unwind in a hot library
    crosses into the loader, which `session.rs`'s module rustdoc
    forbids.
  - Budget: one vtable slot. Crates: `fulgurite-shell`
    (`FrameSource`, `ImageChain`), `fulgurite` (`App::images`),
    `fulgurite-reload` (the forward).
- **Where each new setting is consumed (A5):**

  | setting | side | read |
  |---|---|---|
  | `Program::images` (exists) | window | once, into `Config`, moved into each backend (M47/T47.7) |
  | `App::images` (T84.6) | session | every present, through `FrameSource::images`, `Local::caught(None, ..)` |
  | `App::wake_in_ms` (T84.3) | session | every frame, through `Session::next_deadline_ms` (exists), `Local::caught(None, ..)` |
  | `ImageSource::image(id, frame)` | either | per command at render |

- **(M84b) A subscription can be told to stop.** Today
  `Host::diff_subscriptions` (`lib.rs:1344-1388`) removes a stopped
  subscription from its `running` list and nothing else. The worker runs
  on, and its later messages are applied. `docs/plan-tasks.md` §4
  deferred "cooperative cancellation" until "a long job exists whose
  wasted work is worth the interface". A video decoder is that job, and
  a seek re-declares its subscription with a new key.
  - `JobTrack` (`inbox.rs:134-139`) gains `stop: AtomicBool` (+1 B).
    The host sets it on stop. `Sender::is_stopped()` reads it.
  - The drain drops what a stopped job sent afterwards and counts it
    (`Host::stopped_dropped`).
  - A hot swap needs nothing new: v1's `Inbox` is dropped with v1's
    host, so a feed's next `send` answers `Disconnected`, and the feed
    returns.
  - **M88/T88.1 rewrites the same `JobTrack`** (`JobTrack<M>` with a
    coalescing `latest`/`fresh` progress slot that `pump_with` drains
    before the channel, `docs/plan-platform-requests.md`). Neither is
    ordered before the other in §5, so **whichever lands second extends
    the other**: the slot drain honours `stop` (the value is dropped
    and counted in `stopped_dropped`), and `settled` treats a stopped
    job as settled. If T88.1 has landed, T84.7 carries that gate; if
    not, T88.1 does (M11).
  - Crate: `fulgurite` (`inbox`, `std`) and `fulgurite-shell`.
- **(M84b) Video is a controlled widget plus a feed, not a codec.**
  - `Video<S>` (`fulgurite-widgets`, `std`) shows `(id, frame)` and
    draws transport controls: play/pause, a seek track and the time.
  - It is controlled the way `Select` is (M33): the application writes
    `playing`, `position_ms`, `duration_ms` and `frame`.
    `on_toggle: fn(&mut S, &EventCtx)` and `on_seek: fn(&mut S, f32)`
    report intent. **The widget computes the seek fraction** in
    `on_event`, from the press's `local` against the track rect it laid
    out, clamped to 0..=1, and passes it; the handler multiplies by its
    own `duration_ms`. `EventCtx` carries no geometry of the receiving
    widget (`event.rs:403-460`) and under M33 a handler cannot reach the
    tree, so an associated `Video::seek_ms(&EventCtx)` could not know
    the track's width (P19).
  - The seek track is its own, not a nested `Slider`, which has no
    change handler (M57, "Not built: `Slider::on_change`").
  - The producer is `VideoFeed` (shell, `std`). It is **not**
    `FrameSource`, which is `Session`'s supertrait
    (`session.rs:47,148`). It is opened by data:
    `VideoFeed::open(key: u64)`, with the source index and start
    position packed into the subscription key, never a path or a `fn`
    in flight (D1(d)).
  - `video_job::<F, M>` is the `JobFn` a `job_for` returns. It paces
    frames to their `pts_ms`, sends `VideoFrame` as the application's
    message, and returns on `is_stopped()` or `Disconnected`.
  - `GifFeed` (behind `image`) is the reference feed. It exists so that
    `Video` ships whether or not K84b passes.
- **(M84b) The GStreamer decoder crate is conditional on K84b.** It is
  `fulgurite-video`, `GstFeed: VideoFeed` over `appsink`,
  default-off and Linux-first. It carries `#![forbid(unsafe_code)]`
  like every crate but `fulgurite-reload`
  (`scripts/check_no_unsafe.sh`). It is not added to the reload source
  hash: it defines no type that crosses the seam, which is the
  exclusion `just abi-stamp` already checks for `fulgurite-widgets`.

**Tasks.**

*M84a*

- **T84.0 — the gates that compile today.** Each **pins today's
  behaviour, green**: it asserts the current gap and prints it, and the
  task that closes the gap flips the assertion (§3.1: no `#[ignore]`).
  - **(a) Software stale image**
    (`crates/fulgurite-shell/tests/image_frame.rs`, flipped by
    T84.2).
    - Setup: a `Host` + `Offscreen`, with `canvas.images` a store whose
      pixels a `Cell` switches from red to blue under the same `ImageId`.
    - Prints the centre pixel and `damaged_px` after the switch, a pump
      and a present.
    - Setup landed: frame 0 shows red, asserted first.
    - Pinned today: the pixel stays red with 0 px damaged, asserted and
      printed. T84.2 adds the one line that could not exist yet
      (`img.frame += 1` in the tick) and flips the assertion to blue.
  - **(b) GPU atlas stale** (`crates/fulgurite-wgpu/tests/
    image_frames.rs`, flipped by T84.2).
    - Setup: the same switching store under `Gpu::set_images`, `draw`
      with full damage twice, then `Gpu::read_rgba`.
    - Prints the centre pixel of each draw and `atlas_stats()`.
    - Pinned today: the second draw is still red (the atlas caches by
      `ImageId`), asserted. This isolates the cache from damage, because
      damage is full both times.
    - Skips without an adapter; `just gpu` requires one.
  - **(c) Default crate count** (`scripts/check_default_crates.sh`,
    green).
    - K2's exact command, `cargo tree -p fulgurite-winit -e normal
      --prefix none | sed 's/ (\*)$//' | sort -u | wc -l`, printed
      beside the parent commit's count. Bound: Δ = 0.
    - Accept direction: green today.
    - Injection: add a dependency to `fulgurite-shell`'s default
      set; Δ is printed and the check fails.
    - Printed as a measurement and never hard-coded, because
      `check_ci_claims.sh` refuses a bare count.
  - **(d) Firmware delta**: M80/T80.0's `scripts/firmware_delta.sh
    BASE [BIN=B ...] [print]` (`just firmware-delta <sha> [BIN=B ...]`,
    positional; default `firmware_interactive=1000`), cited, not
    re-added (G32). M84 adds no interface; its own injection runs where
    it bounds a task (T84.2):
    - Injection: the leaf fixture paints one `Image` (a paint no
      firmware makes today); `just firmware-delta <parent sha>
      firmware=0 firmware_interactive=0 firmware_bind=0
      firmware_cascade=0` prints a `.text` delta > 0 on `firmware` and
      fails by name.
    - Accept direction: `just firmware-delta HEAD firmware=0
      firmware_interactive=0 firmware_bind=0 firmware_cascade=0` on a
      clean tree gives 0 on all four.
  - **(e) Subscription stop** is M84b's; see T84.5.
- **T84.1 — the `image` feature: PNG and GIF decode into
  `ImageStore`.** `fulgurite_shell::image`: `decode_png`,
  `decode_gif`, `DecodedImage`, `ImageError`. `ImageStore` is
  unconditional. At this task `ImageStore` answers today's
  `image(&self, id)` with frame 0; T84.2 changes the signature.
  - Same commit:
    - the shell manifest comment gives `image`'s crate count and the
      release-size delta of a program that decodes, as `a11y`'s does;
    - `image.rs:10-14` rustdoc now names the feature
      (`check_doc_identifiers.sh`'s feature rule needs `image` to be a
      `[features]` key by then);
    - `docs/plan-capabilities.md` T15.2 and
      `docs/plan-a11y-platform.md` :28 and T54.9 are corrected;
    - handoff: a **DECIDED** line under images, pointing to
      `docs/results-images.md`;
    - `docs/results-images.md` is created with a T84.1 row, and its
      `scripts/doc-paths.allow` entry (M80/T80.0) is removed;
    - the book's `leaves.md` `## Image` gains decoding by named anchor;
    - `DecodedImage` and `ImageStore` are removed from
      `scripts/doc-identifiers.allow`.
  - **Gates:**
    - **PNG round trip.** Encode a 7×5 RGBA pattern with `png`'s encoder
      and decode it. Prints the differing bytes, bound 0. Setup landed:
      w, h and length are asserted first. Accept: identical.
      *Injection:* drop the alpha channel (decode as RGB). The byte
      count differs by name.
    - **GIF compositing.** A 4-frame GIF encoded in the test with `gif`'s
      encoder: frame 2 covers only the top-left 2×2 with disposal
      `Keep`. Prints the frame count, delays (bound `[100; 4]`), and
      frame 2's pixel at (4, 4), which must equal frame 1's.
      *Injection:* return raw frames uncomposited. The pixel reads
      transparent.
    - **Malformed input.** A truncated file answers `Err(ImageError)`
      and never panics, for every prefix length of the test GIF.
      Prints the prefixes tried and the panics (bound 0). *Injection:*
      `unwrap()` in the frame loop, which panics at the first short
      prefix.
    - **Default stays off.** T84.0(c) is green: Δ = 0 with `image`
      default-off. *Injection:* put `image` in the shell's `default`;
      Δ equals K84a's count and the check fails. This crate count is
      K2's falsifiable half for `image`: the index records `image` as
      K2's `nm` exception (`plan-iced-roadmap.md` K2, second review
      R12), because an uncalled decoder links 0 symbols either way.
    - **Lint.** `cargo clippy -p fulgurite-shell --features image
      --all-targets -- -D warnings`, added to the `justfile`'s feature
      clippy lines beside `--features simd` (`justfile:328`), since the
      workspace line builds default features only and never sees the
      feature's code. Beside it, `RUSTDOCFLAGS="-D warnings" cargo doc
      -p fulgurite-shell --no-deps --features image`, since the
      feature adds public items (`decode_png`, `DecodedImage`; index
      §3.1).
    - No live drive: a decoder has no user-visible behaviour of its own.
      T84.3's drive paints a decoded GIF.
- **T84.2 — the frame in the command, and the atlas in place.**
  - Changes: every consumer, painter and implementor listed under
    Decisions. `Image` gains `frame`. The wgpu `image_frames` map and
    the in-place `write_texture`. `AtlasStats::image_uploads` and
    `image_upload_bytes`. `ImageStore` answers `frames[frame % len]`.
    T84.0(a) and (b)'s assertions are flipped.
  - Same commit:
    - the rustdoc on `ImageSource`, `Cmd::Image`, `Painter::image`,
      `Image::frame` and `wire::VERSION`;
    - handoff: **DECIDED**, an image's version is in the command
      (→ `docs/results-images.md` T84.2);
    - the book's `damage.md` gains a paragraph, with the `Recording`
      fold by anchor;
    - a results row with the firmware delta and K3.
  - **Gates:**
    - **T84.0(a), software.** Blue after the frame bump. Damage ratio
      printed against the image rect: bound 1.00×. Setup landed: the
      tick returned `true` and the frame reads 1.
      *Injection:* `Recording::image` stops folding `frame`. 0 px
      damaged, the pixel stays red, and the assertion names the frame.
    - **T84.0(b), GPU.** The second draw is blue.
      `image_uploads` = 2 (printed). *Injection:* skip the frame
      comparison in `image_slot`, keying on `ImageId` alone (today's
      behaviour). The pixel stays red.
    - **One id at two frames in one list** (P1). Two `Image`s of one id
      at frames 1 and 2 (red, blue), one draw. GPU `read_rgba` at both
      centres equals the software `Offscreen`'s at both (four pixels
      printed). Setup landed: the list holds two `Cmd::Image` with one
      id and frames 1 and 2, asserted. *Injection:* drop the per-draw
      record so the second command rewrites the first's slot (the
      id-only key). Both GPU centres read blue, and the first named
      pixel differs from software.
    - **Origin.** One id in both stores (red in the application's,
      blue in the window's); draw with the frame's store, then with
      `images()` answering `None`. The second draw is blue (printed).
      *Injection:* key on `ImageId` without `origin`. It stays red.
      (M84b: this gate lands with T84.6, which brings the application
      store.)
    - **The unchanged neighbour.** Two `Image`s, and only the first
      advances. Prints the damage ratio against the first's rect (bound
      1.00) and whether the second's node is in `Host`'s
      changed-signature list (bound: absent). *Injection:* `Image` paints
      a store-wide counter instead of its own `frame`, the S2 shape of a
      shared index. Ratio 2.00, and the second node is listed.
    - **Journal.** `DisplayList::hash_from` differs for the advanced
      node. *Injection:* `Cmd::hash_into` omits `frame`. The journal's
      per-node signature is unchanged, and the test names the node.
    - **No atlas churn** (K84d). A 300×300, 4-frame loop over 100 frame
      changes after a first draw. Prints `resets` (bound 0),
      `image_uploads` beside its bound of 1 + changes = 101 (the first
      draw's insert counts, as in the gate above: one draw plus one
      change gives 2) and `image_upload_bytes / image_uploads` (bound
      360,000 = 300·300·4). Accept direction: 100 draws at an unchanged
      frame after the first give `image_uploads` = 1, printed (P4).
      *Injection:* insert again instead of rewriting. A 1024² atlas
      holds 9 such slots, so `resets` ≥ 10, printed.
    - **Size change.** Replace a 300×300 frame with a 200×200 one: it
      draws at the new size, with `resets` ≤ 1 printed.
    - **Deferring.** `deferring_forwards_every_command`
      (`tests/overlay.rs:435`) paints its image with `frame: 3`. It
      already compares whole `Cmd`s. *Injection:* `Deferring::image`
      forwards `frame: 0`. The streams differ, by name.
    - **Wire.** A `Cmd::Image` with `frame: 7` round-trips, and a
      buffer of the previous version decodes to `None` (both versions
      printed). *Injection:* the encoder skips
      `frame`. The round trip fails.
    - **K1.** `size_of::<Cmd>() == 60`, printed.
    - **Firmware.** `just firmware-delta <parent sha> firmware=0
      firmware_interactive=0 firmware_bind=0 firmware_cascade=0` — the
      parent commit, `HEAD` for uncommitted work (T84.0(d), K4) —
      prints `.text` and `.rodata` per firmware. Bound 0 B on each of
      the four, since none paints an image; a
      non-zero delta is a finding and stops the task. Printed against the
      80 KB ceiling (81,920 B, D5) and 28,616 B
      (`docs/results-embedded.md`).
    - **K3.** `R=1 benches-competitive/run.sh -p fulgurite-frame`,
      rows `idle (floor)`, `ALL rows: +paint` and `+render (damage)`
      (index K3), and the one-row rows by their printed labels
      `layout`, `+paint` and `+render (damage)`
      (`benches-competitive/arms/frame/src/main.rs:147,173,331-334`;
      no row is called "one-row-change"), under `just bench-lock`,
      eight invocations a side, inside their spread (printed).
- **T84.3 — animated images: `Playback`, `App::wake_in_ms`, the
  showcase entry and its drive.**
  - Changes: `Playback` (core `anim`, `alloc`) and `App::wake_in_ms`,
    folded into `Host::next_deadline_ms` through
    `Host::next_deadline_with`, and `Local::next_deadline_ms` reading
    it through `caught` (Decisions, A3). `#[app(wake = …)]` in the
    derive. The showcase gets an **Image** entry: a committed 4-frame
    GIF (`examples/showcase/assets/spin.gif`) decoded at boot into an
    `ImageStore` registered with `Program::images`, and a pause
    `Button` that writes `Playback`'s `playing` through the store.
  - Same commit:
    - `scripts/drive/widgets/image.py` (the inventory below);
    - `Image | M27 | takes no input` removed from
      `scripts/drive/undriven.allow`;
    - `Image`'s `observe` (role `Image`) is unchanged: `check_observe.sh`
      already passes it, and its a11y role is M54/T54.3's
      (`crates/fulgurite-widgets/tests/a11y_roles.rs:71` already has
      its row);
    - `scripts/drive/roles.py` gains `"Image": "Image"` for the new
      inventory, since `run.py --manifest-only` refuses an inventory
      entry with no row there;
    - the attribute table in the macros' rustdoc and `book/src/
      attributes.md` gain `wake` (`check_attr_table.sh`), and the
      showcase is its user (`check_derive_attrs.sh`);
    - the derive's unknown-key message lists `wake`, and
      `crates/fulgurite-macros/tests/ui/fail_app_unknown_key.stderr`,
      which pins it, is updated with it (M23);
    - `AnimatedImage` is removed from `scripts/doc-identifiers.allow`
      (never built; see Decisions);
    - the book's `animation.md` gains `Playback` by anchor;
    - `book/src/gaps.md` and `docs/competitive-feature-gaps.md` drop
      "animated images";
    - the rustdoc of `App::tick` names `wake_in_ms`;
    - a results row for K84c.
  - **Gates (headless):**
    - **Timing.** A `Playback` with delays `[100; 4]` starts at frame
      0, read from its state. `tick(0)` answers `None` and moves nothing
      (the `App::tick` contract); four `tick(100)` then answer
      `Some(1)`, `Some(2)`, `Some(3)`, `Some(0)` (all five printed, P5). *Injection:* `tick` answers `Some` on every
      positive `dt`. The sequence at 50 ms steps prints duplicates.
    - **Deadline.** `Host::next_deadline_ms` equals
      `Playback::next_change_ms()` ± 0.01 ms while playing and is `None`
      when paused. *Injection:* `next_deadline_ms` ignores
      `wake_in_ms`. It prints `None` while playing.
    - **Allocation.** `alloc_gate.rs` gains an arm in which a `Playback`
      advances an `Image` through 100 frame changes on the software path.
      Allocations per frame are printed, bound 0. *Injection:*
      `ImageStore::image` clones the frame. 100 allocations, printed.
  - **Panic guard** (`just panic-drive`, second review A3).
    `fixtures/panic-app` gains a site whose `wake_in_ms` panics; the
    site count (seven today, `scripts/panic_drive.py`, plus M80's
    `caret_rect` site if it has landed) and `--expect-panics` go up by
    one, printed. The recovering leg asserts M36's screen, the
    location, refused input and a clean retry, as for the other sites.
    *Injection:* `Local::next_deadline_ms` reads `wake_in_ms` outside
    `caught`. The panic escapes the session, the site never reports
    through `app.failure`, and the leg fails on that site by name.
  - **Gates (live, `just widget-drive Image`, under all four legs:
    `Everything`, `Boundaries`, `Boundaries+oracle`,
    `Boundaries+scale2`):**
    - **"time advances the frame"** (`visible=True`).
      - `do`: `app.presents` is read, then the drive sleeps 1,000 ms.
      - `expect`: `ui.regionHash` of `#d_image` differs; `presents` Δ is
        10 ± 1 (printed); `skipped` Δ ≤ 11 (the one re-check frame per
        change, printed); `ui.lastDamage` is 1.00× the image's box
        (printed).
      - `isolate`: paused, the same wait gives `presents` Δ 0 and an
        unchanged hash. That is also the idle rule of M47/T47.8.
      - *Injection:* the showcase's tick writes the frame and answers
        `false`. The `Boundaries` legs show an unchanged hash;
        `Everything` stays green. That shape is the reason for the four
        legs.
      - *Second injection:* `next_deadline_ms` ignores `wake_in_ms`.
        `presents` Δ is 0 while playing.
    - **"pause holds the frame"** (`visible=True`). A click on the pause
      button keeps the hash constant over 500 ms (printed), and a second
      click resumes. *Injection:* the handler sets `playing` instead of
      flipping it. The resume fails by name.
  - **GPU (not agent-observable).** `ui.regionHash` reads `frame_rgba`,
    which the GPU backend does not provide
    (`fulgurite-agent/src/lib.rs` `ui.regionHash` → `host.frame()`;
    winit `gpu.rs` leaves `fetch_rgba` at `None`). The GPU half is
    therefore T84.2's `fulgurite-wgpu` test under `just gpu`. It
    becomes agent-observable when `ui.regionHash` reads the GPU's
    `frame_rgba`, which M88/T88.2 ships (`fetch_rgba` stays `None` on
    the GPU); re-running this drive on the GPU after T88.2 is the reopen
    condition.
  - **K84c.** Prints `app.perf` `layout_ns` per animated frame on the
    showcase entry, the median of 50 frames, beside `just grid`'s
    `mark_all` layout row (23.15–23.60 µs,
    `docs/results-damage-precision.md`).
- **T84.4 — M84a closes.**
  - `docs/results-images.md` summary.
  - AGENT.md queue entry citing `just ci-verify <sha>` green (S5).
  - No behaviour changes here: every doc above moved with its task.

*M84b*

- **T84.5 — M84b's gates that compile today.** Committed green,
  pinning today's gap, in
  `crates/fulgurite-shell/tests/subscriptions.rs`; T84.7 flips it.
  - **A stopped subscription stops.** A subscription job sends `n` every
    10 ms, and the application un-declares it after the first message.
    Prints the messages applied after the stop and the job's iterations
    after the stop.
  - Setup landed: the first message was applied, and
    `subscription_counts()` reads (1, 1).
  - Pinned today: messages keep arriving (`lib.rs:1359-1363` only
    forgets the entry), so the gate asserts the count after the stop is
    > 0, printed. T84.7 flips it to bound 0 (and iterations ≤ 1).
- **T84.6 — `FrameSource::images` and `App::images`.**
  - Changes: the required method; `Host`, `Local`, `Hot` and `Barred`
    (which forwards to its session, never `None`); `ImageChain`;
    `Canvas`, `Gpu`, `Offscreen`, `Surface` and the GPU backend consult
    the frame's store first; the atlas key's `origin`;
    `Gpu::invalidate_images()` and the window's calls to it on a swap
    and an M36 retry; `ImageStore::replace(id, w, h, rgba) -> u32` for
    streams. **`#[app(images = path)]`** in the derive (chosen over
    hand-writing `fixtures/hot-window`'s `impl App`, which would drop
    the derive the fixture exists to exercise); `fixtures/hot-window`
    is its user (P3).
  - Same commit, for the derive key: rows in the macros' rustdoc table
    and `book/src/attributes.md` (`check_attr_table.sh`), the user in
    `check_derive_attrs.sh`, and the unknown-key message plus
    `fail_app_unknown_key.stderr` updated again (M23).
  - Same commit:
    - the rustdoc on `FrameSource`, `App::images` and `Program::images`
      (window-side, read once);
    - handoff: **DECIDED**, the application's images reach the renderer
      through the frame, with the table above;
    - the book's `hot-reload.md` covers a hot build's assets;
    - a results row.
  - **Gates:**
    - **Compile.** The forward: a `Hot` or `Local` without `images` does
      not compile. The compiler is that gate (S1). No stamp test.
    - **Headless write.** An application's `apply` calls `replace`,
      paints the returned frame, and the next `Offscreen` present shows
      the new pixels. Prints the centre pixel and the damage ratio
      (bound 1.00×). Setup landed: `apply` returned `true`.
      *Injection:* `Host::images` answers `None`. The pixel is the page
      colour, by name.
    - **Order.** The same id in both stores: the application's wins. An
      id only in the window's store still draws (accept direction).
      Prints both pixels. *Injection:* consult the window's store first.
      The first pixel is wrong.
    - **Across a swap** (`just hot-window-drive`, extended).
      `fixtures/hot-window` paints an `Image` from `App::images`, and
      the script's `sed` edit changes the store's colour. After the swap
      the region's pixels are v2's (printed). *Injection:* `Hot::images`
      answers `None` instead of forwarding. The region shows the page
      colour, and the drive names it.
    - **Under the bar** (the same drive's failed-build step, A13/P2).
      While the error bar is up, the application-store image's
      `ui.regionHash` equals its hash before the broken edit (both
      printed). Setup landed: the bar is shown (the drive already
      asserts it, M37). *Injection:* `Barred::images` answers `None`.
      The hash changes to the page colour's, by name.
    - **Across a swap, GPU** (A14). The live drive cannot read GPU
      pixels until M88/T88.2 (T84.3's GPU note), so the leg is a
      `fulgurite-wgpu` test under `just gpu`: a `Gpu` draws id 1
      frame 0 from store A (red), then store B answers the same
      `(id 1, frame 0)` in blue, the test calls
      `invalidate_images()` as the window's swap does, and draws again.
      Blue, printed with `image_uploads`. Setup landed: the first draw
      is red, asserted. *Injection:* skip the invalidation. The region
      stays red (v1's colour). The window's call site is checked by
      `just hot-window-drive --gpu` once T88.2 lands (reopen condition,
      recorded in the results row).
    - **Stamp, accept direction.** `fixtures/stamp-lib` gains an `image`
      feature and a direct `fulgurite-shell` edge.
      `scripts/abi_stamp.sh` asserts `accept` for it, and its M49 half
      still refuses a reordered `Cmd`.
    - **Panic.** While `failed()` is true, `images()` answers `None`
      (unit test). *Injection:* forward regardless. The test reads
      `Some`.
    - **Panic guard** (`just panic-drive`, second review A3).
      `fixtures/panic-app` gains a site whose `App::images` panics;
      the site count and `--expect-panics` go up by one again, printed.
      The recovering leg asserts M36's screen, the location, refused
      input and a clean retry. *Injection:* `Local::images` calls
      `self.host.images()` outside `caught`. The panic escapes the
      session and the leg fails on that site by name.
    - **Resume.** `just resume-drive` stays green: the window-side
      source still round-trips (M47/T47.7).
- **T84.7 — cooperative stop for subscriptions.** `JobTrack::stop`,
  `Sender::is_stopped`, the drain's drop, and `Host::stopped_dropped`.
  T84.5's assertion is flipped.
  - Same commit:
    - `docs/plan-tasks.md` §2.3 and §4: "cooperative cancellation" is
      marked built, with its reopen condition met by T84.8;
    - the handoff's tasks section gets a **DECIDED** line;
    - `docs/results-tasks.md` gets a row.
  - **Gates:**
    - **T84.5.** Messages after the stop are 0, and the job returns
      within one iteration (printed).
    - **Accept direction.** A subscription still declared keeps
      delivering: 50 of 50 (printed).
    - **Re-key.** The same kind with a new key: only the new key's
      messages are applied after the change (printed per key).
    - *Injection 1:* the drain ignores `stop`. The messages after the
      stop are counted, by name.
    - *Injection 2:* the host never sets `stop`. The job's iterations
      after the stop exceed 1.
    - **Progress after stop** (M11; here if M88/T88.1 has landed,
      otherwise T88.1 carries it). A job sends a `progress` value, is
      stopped, and sends one more. The stopped job's final value is not
      applied (count printed, bound 0), `stopped_dropped` counts it, and
      `settled` answers true. *Injection:* the slot drain ignores
      `stop`. The final value is applied, counted by name.
    - **K3.** `benches --bin spawn` shows no movement outside its
      spread, eight runs a side (printed).
- **T84.8 — `Video<S>`, `VideoFeed`, `GifFeed`.**
  - Changes: the widget, the trait, `VideoFrame`, `video_job` and
    `GifFeed`. The showcase gets a **Video** entry fed by `GifFeed` from
    the T84.3 asset at 10 frames/s.
  - Same commit:
    - `Video`'s `observe` gives role `Video` (AccessKit 0.25 has it),
      the actions `Click` (toggle) and `SetValue` (seek), and the value
      "m:ss / m:ss" (M54/T54.3);
    - `scripts/drive/widgets/video.py`, with `"Video": "Video"` in
      `scripts/drive/roles.py` (`run.py --manifest-only` refuses an
      inventory entry without one);
    - a `row("Video", "Video", Role::Video, true)` in
      `crates/fulgurite-widgets/tests/a11y_roles.rs`, whose
      `every_node_impl_has_a_row` fails on a new `Node` impl without
      one;
    - the showcase entry;
    - `Video` is exported from `fulgurite-widgets` and covered by
      `check_drive_coverage.sh`;
    - the book's `leaves.md` gains `Video` by anchor, and `gaps.md`
      drops "video";
    - handoff: **DECIDED**;
    - a results row.
  - **Gates (live, `just widget-drive Video`, all four legs).** Each
    `visible=True`, with its consequence printed:
    - **play**: the frame advances, and the regionHash of the picture
      rect changes within 250 ms;
    - **pause**: the hash is constant over 500 ms;
    - **seek**: a press at 75% of the track passes 0.75 to `on_seek`
      and sets `position_ms` to 75% of `duration_ms` ± one frame
      (100 ms), and the picture shows frame
      `⌊0.75·n⌋`;
    - **scrub**: a drag from 25% to 50% ends at 50% ± 100 ms, and no
      frame from before the drag lands after it (T84.7's stop, seen
      live);
    - **isolate**: a press on the picture, not the controls, changes
      nothing (`handled` false, per `AgentHost::pump`).
    - *Injections, one per action:* `on_toggle` not called (play); the
      `playing` field ignored by the paint of the play glyph (pause);
      the fraction computed against the widget's box instead of the
      track's (seek, off by the controls' inset, printed); T84.7's drain check removed
      (scrub, a stale frame lands).
  - **Gates (headless):**
    - **Seek fraction** (P19). A pumped press at the track's horizontal
      midpoint passes 0.5 ± 1/track-width to `on_seek` (value and
      width printed). Setup landed: `handled` is true and the handler
      ran once. *Injection:* the widget measures against its own box.
      The fraction is off by the inset, printed against the bound.
    - **UI-thread allocations per video frame** (`alloc_gate.rs` arm):
      0. The worker allocates each frame's `Vec`, and `apply` moves it
      in with `replace`. *Injection:* `replace` copies into a new `Vec`.
      One allocation per frame, printed.
    - **Pacing.** 20 frames at 100 ms arrive 100 ± 10 ms apart, and
      `Inbox::refused` stays 0 (printed). *Injection:* no sleep to
      `pts_ms`. The inbox refuses, and the count is printed.
    - **K84e.** A 1280×720 `GifFeed` stream at 30 frames/s on the
      software backend. Prints `app.presents` per second over 10 s.
- **T84.9 — the K84b probe.** A scratch manifest under `target/`,
  never committed, names `gstreamer`, `gstreamer-app` and
  `gstreamer-video`.
  - Measured: the deduplicated `cargo tree` count; whether the
    `appsink` → RGBA path needs `unsafe` in our code; and whether the
    reference machine has the GStreamer development libraries.
  - A results row with all three. The commit is the row alone.
- **T84.10 — `fulgurite-video`, or its deferral.**
  - **If K84b passes:**
    - the crate is added, with `GstFeed: VideoFeed`, default-off and
      `#![forbid(unsafe_code)]`;
    - the gate plays a `videotestsrc` pipeline (no committed asset):
      frames arrive at 30 ± 1/s and a seek lands within one frame, both
      printed;
    - the gate is skipped (exit 77) without GStreamer, and **a skipped
      gate means the milestone is not done** (M43's R17-F6 rule);
    - *Injection:* `appsink` caps are left unset, so frames arrive in the
      decoder's native format, not RGBA. The byte-length assertion
      fails by name.
  - **If K84b fires:** a deferral record in `docs/results-images.md`
    with the measured numbers and the reopen condition ("a GStreamer
    binding below 150 crates with a safe `appsink`, or a pure-Rust
    decoder of one common codec"). `Video` ships with `VideoFeed` and
    `GifFeed`.
- **T84.11 — M84b closes.** The results summary, and the queue entry
  citing `just ci-verify <sha>` green.

**Kill rules** (written before building; a rule that fires stops its
task and is recorded with its number in `docs/results-images.md`):

- **K84a — decoder crates.** `image`'s own crates (T84.0(c)'s count
  with the feature on, minus off) > 15, measured in T84.1. If it fires,
  PNG decoding is left to the application, only `gif` ships, and the
  count is recorded.
- **K84b — the GStreamer crate.** Measured in T84.9. It fires on any
  one of these, and then `fulgurite-video` is deferred (T84.10's
  second branch):
  - the dependency set exceeds 150 crates;
  - `fulgurite-video` needs **any** `unsafe`. No exemption is added
    to `scripts/check_no_unsafe.sh`, whose only one is
    `fulgurite-reload`;
  - the reference machine lacks the development libraries, so its gate
    could only skip.
- **K84c — the whole-tree mark per animated frame.** The median
  `layout_ns` per animated frame on the showcase's Image entry
  (`app.perf`, T84.3) > 100 µs, about 4× `just grid`'s 2,227-node
  `mark_all` layout. If it fires, `App::tick_marked(dt, mark) -> Wrote`
  is built before T84.4, shaped like `App::update_marked`, with the
  default conservative, and the gate re-runs.
- **K84d — the atlas under animation.** In T84.2's 100-change loop,
  `resets` > 0 even with the in-place write. If it fires, an image whose
  frame changes gets a dedicated texture, and the atlas keeps stills
  only.
- **K84e — video on the software backend.** 1280×720 at 30 frames/s
  presents fewer than 28 per second (T84.8). If it fires, `Video`'s
  rustdoc and the results doc state the largest size the software
  backend holds at 30/s, measured. The widget ships.

**What building found (M84).**

- **T84.0(a)'s pin is stronger than the plan asked.** The tick in
  `crates/fulgurite-shell/tests/image_frame.rs` answers `true`, so
  under the default `Boundaries<256>` the host marks the whole tree and
  re-runs layout, and the frame still reports 0 px damaged: the gap is
  the command fold, not invalidation. Its perturbation at T84.0 was a
  per-call counter folded into `Recording::image`, which is T84.2's
  mechanism in miniature: the image's box (1,600 px) was damaged and the
  centre read blue, red by name. The test needs `truetype` (`Offscreen`
  lives in `session`), so it is a `[[test]]` with `required-features`,
  which the workspace build satisfies.
- **T84.0(b)'s perturbation** re-uploaded on an atlas hit in
  `image_slot` (the store asked again under the cached slot): the second
  full-damage draw read blue and the pin failed by name.
- **T84.0(c) chooses its base.** "The parent commit's count" is the work
  tree against `HEAD` while a manifest or the lock differs from it, and
  `HEAD` against `HEAD^` on a clean tree, so CI checks the last commit
  rather than comparing `HEAD` with itself (`scripts/check_default_crates.sh`
  header). When no manifest differs the two graphs are equal by
  construction and nothing is extracted. Injection: `default = ["simd"]`
  in the shell's manifest moved the count by 2 and failed by name.
- **T84.0(d)'s injection trips a second gate first.** Adding an `Image`
  to the leaf fixture's tree fails the build on `assert!(UI == 1216)`
  (`fixtures/embedded/src/lib.rs`) before the comparator can run, so the
  injection relaxes that assertion too. With it, the strict four-zero
  invocation against `1024cba` printed `.text` +1,264 on `firmware`
  (+1,472 / +1,208 / +1,240 on the other three) and failed by name. Run
  at T84.0 rather than deferred to T84.2, since it costs one build.
- **T84.1: K84a counts 9, not 10, because the lock holds `flate2` at
  1.1.9.** `flate2` 1.1.10 moved to `miniz_oxide` 0.9 while `png` 0.18
  asks for 0.8, which put both in the graph. `gif` is taken without its
  default `color_quant` (encoder only). `docs/results-images.md` T84.1
  lists the nine.
- **T84.1: a PNG missing only its end chunk's CRC decodes.** `png` has
  read every pixel by then, and the answer is the whole picture. The
  truncation gate therefore holds the GIF to the plan's "every prefix is
  an error" (133 of 133) and the PNG to "never a panic, and any answer is
  the whole picture" (4 of 97 prefixes answer, all 93–96).
- **T84.1: K2's exception, measured.** A probe program with `image` on
  and no decoder called links a `.text` byte-identical to the feature
  off; the stripped file is +1,648 B, all `.gcc_except_table`. Calling
  both decoders costs +200,232 B. The crate count remains the only gate
  that can see the feature.
- **T84.1: the API's small additions, not in the plan's list.**
  `ImageStore::insert` refuses a frame whose length is not `w × h × 4`
  (a renderer reads a short slice as a missing image, so storing one
  would blank it silently); `ImageStore::frames` reaches an animated
  image's later frames until T84.2's `frame` does; `insert_decoded` sits
  behind the feature beside the type it takes; and a decode refuses more
  than `MAX_DECODED_BYTES` (256 MiB across all frames) with
  `ImageError::TooLarge`, since compositing a GIF allocates a full canvas
  per frame.
- **T84.1: the feature's lines are in `dev-shape`**, so `just test` runs
  them, not beside `just simd`'s clippy line (`check-extra` only). And
  `pub mod image` carries a `//` comment, not `///`: an outer doc joins
  the module's inner one, whose intra-doc links then failed to resolve
  under `cargo doc -D warnings`.
- **T84.2: the side slot is a key, not a map.** The plan's `image_alt`
  would need its own space in the one image texture the shader binds, so
  the atlas key is an `ImageSlot` with two kinds: `Current(id)`, the
  id's own slot whose frame `image_frames` records, and `Alt(id, frame)`,
  the side slot. `image_drawn` is the per-draw record (cleared at the
  head of every build of the frame, retries included), and
  `reset_images` clears all three wherever the atlas resets. The key is
  `ImageId`, not `ImageKey { origin, id }`: the origin and
  `Gpu::invalidate_images()` arrive with the application's store in T84.6,
  as its own Changes list says.
- **T84.2: the sources answer by frame, not by a switch.** T84.0's
  pins switched a `Cell` behind an unchanged id. `ImageSource::image`'s
  rustdoc now says the answer for one `(id, frame)` must not change while
  a renderer may hold it (the GPU caches by that pair), so the flipped
  gates use sources that answer by frame: the software one red at even
  frames and blue at odd, the GPU ones a colour per frame.
- **T84.2: the neighbour and journal gates are one test.** Both read the
  same two-image frame: the neighbour's damage ratio and pixels, and each
  leaf's signature as `DisplayList::hash_from` over its own paint (what
  `sign_leaves` and the agent's journal compute). Each still has its own
  injection and its own named assertion.
- **T84.2: the size-change injection panics.** Writing in place whatever
  the size makes `write_texture` slice a 300-square slot's bytes from a
  200-square frame, which panics before any frame draws. That is red, by
  a panic rather than a named assertion; a write of the wrong size has no
  drawn frame to compare.
- **T84.2: no `Markdown` call site.** M87/T87.4 owns it (Decisions), and
  `fulgurite-widgets` has no other `Painter::image` caller than
  `Image`.
- **T84.3: the wake is asked through `Local::guarded`, not `caught`.**
  `caught` answers its fallback and records nothing, so a panicking
  `wake_in_ms` would never raise M36's screen or report through
  `app.failure`, which the panic drive's site asserts. Since
  `Session::next_deadline_ms` takes `&mut self`, the session can use the
  guard every other `&mut` entry uses: `Local::next_deadline_ms` asks
  `h.app.wake_in_ms()` guarded and passes the answer to
  `Host::next_deadline_with`, as planned. Run with `caught` in its place,
  `image_wake.rs`'s panic test reads `failed false` and fails by name. The
  agent's `ui.waitSettled` (`Live::settled`) asks it the same guarded way.
  `next_deadline_with` still asks the application one thing, its timers'
  accessor (`App::timers`), as `next_deadline_ms` always did.
- **T84.3: the window asks for the frame that draws the screen.** The
  deadline is asked after the frame's own guarded entry, so a panic there
  raised the failure with nothing left to draw it: `Win::redraw` now
  waits for nothing, drops the clock and requests a redraw when the
  session failed during the deadline query, as `place_ime` does for site
  8's caret.
- **T84.3: `AnimatedImage` stays in `scripts/doc-identifiers.allow`.**
  The plan asked for its removal, but this plan names it three times as
  the type not built, and the checker reads plan files: removed, the
  check fails on it as dead. Its reason now says "decided against".
- **T84.3: `book/src/gaps.md` never listed animated images.** Its
  absences are from the code, and images were never among them; the
  `docs/competitive-feature-gaps.md` table's Images row did say "animated
  frames: M84", and now says what shipped.
- **T84.3: every live action ends paused.** The scale-2 leg compares each
  action's damaged frame with a full repaint (M57/D6), which a moving
  frame fails for no defect. "Time advances" therefore starts paused
  (its setup), resumes, watches a second, and pauses again on a frame
  other than the one it started on; its isolate is the same second
  paused. "Pause holds" isolates with the same two presses outside the
  button. The pause button has a fixed 96 px width: its label flips
  between "Pause" and "Play", and a shrinking button re-centred the row
  and moved the image.
- **T84.3: K84c is read per presented frame.** `app.perf`'s ring p50
  mixes the animated frames with the re-check redraws between them, so
  the drive's K84c action polls `app.presents` (which draws nothing) and
  reads `app.perf`'s `last` the moment a present lands, keeping frames
  with damaged pixels, until it has 50.
- **T84.3: the drive runs six legs, not four.** `scripts/widget_drive.sh`
  has had two `+shaping` legs since M81/T81.6; the Image inventory runs
  under all six.
- **T84.3: the showcase's image id is `catalog::SPIN`, `ImageId(1)`**,
  which M87/T87.4's Markdown image names. The entry is flagged animating,
  so M57's D5 parity skips it (its frame is the clock's), and the
  headless frames (`--shot`, `--demo`) hand their canvas the same store,
  so a shot draws what the window draws.
- **T84.3: the live injection's shape is the plan's, once the read was
  fixed.** A tick that writes the frame and answers `false` holds the
  picture on the `Boundaries` legs (presents Δ 0, skipped Δ 10: the
  deadline wakes the window and nothing is marked) and stays green under
  `Everything` (presents Δ 10, pixels moved). Its first run read red under
  `Everything` too, and that was the read below, not the window: the
  picture was back on its first frame after twelve changes.
- **T84.3: the first count read one present too many.** An agent request
  is answered before the redraw it asks for, so the drive's own hash
  request landed a redraw inside the counted second (Δ 11 on all six
  legs); the count now opens 30 ms after it. And one hash read either
  side of a second cannot say the picture moved: four frames round, the
  image can be back on its first frame (the next full run read "held" on
  five legs at Δ 10), so a still-equal read is followed by one more,
  150 ms later.
- **T84.3: a short GIF delay plays at 100 ms.** `DecodedImage::delays_ms`
  reports delays as encoded (T84.1), and a zero delay played as zero would
  never leave its frame; `Playback::new` plays a delay of 10 ms or less
  at `Playback::SHORT_DELAY_MS`, as browsers do.
- T84.4 (M84a's close): **every M84a kill rule held** — K84a,
  K84c, K84d, and K4 on all four firmware binaries at every task; the
  numbers are `docs/results-images.md`'s, "M84a's kill rules, at its
  close". Nothing was built at the close:
  `App::tick_marked` stays unbuilt (K84c), and T84.2's origin gate moves
  with the application's store to T84.6, as T84.2's entry above says.
- **T84.5: the pin is M84b's only gate that compiles today.** T84.6's
  headless write, order, swap and bar gates name `FrameSource::images`,
  `App::images` or `ImageChain`, and T84.8's name `Video` and
  `VideoFeed`; none exists before its task. T84.6's GPU swap gate could
  be written against `Gpu::set_images`, but `set_images` already resets
  the atlas, so a pin there would read blue today and pin nothing: the
  gap it guards is a store reaching an unchanged backend through the
  frame, which only T84.6 creates.
- **T84.5 pins both halves, not only the messages.** The plan asked for
  the messages after the stop to be asserted and the iterations printed.
  The first perturbation (the drain drops what the stopped job sends,
  `Inbox::cancel` at the stop) read 0 messages and **20** iterations: a
  drain check alone leaves the worker running, which is T84.7's
  injection 2. So iterations > 1 is asserted too, and its own
  perturbation (the host replaces its inbox at the stop, so the job's
  next send answers `Disconnected`) read 1 and failed by name.
- **T84.5: "after the stop" opens at the frame whose diff stops it.**
  That frame's drain runs after its diff (`Host::pump_with`), so what it
  applies was applied after the stop, and T84.7's drain check sees the
  flag the diff just set. The application un-declares in `apply`, on the
  first message, so the stop is the application's and the gate runs
  under `Threads`, a real worker, not `Queued`.
- **T84.5: M88/T88.1 has not landed** (`JobTrack` has no progress slot),
  so if T84.7 lands first, T88.1 carries the progress-after-stop gate
  (Decisions, M11).
- **T84.6: the host's guard asks `App::images` in the frame, and the
  renderer's read is caught.** `FrameSource::images` is `&self`, because a
  renderer holds the list beside the store, and `Host::guard` takes
  `&mut`, so the guard cannot wrap the read itself; and `Local::caught`
  records nothing, so a panic there could not raise M36's screen (T80.5's
  and T84.3's finding). So the frame's guarded entry (`Local`'s
  `Session::frame`, and the agent's `Live` pump) asks `App::images` after
  the pump, where a panic is recorded before any present asks again, and
  `Local::images` reads it under `catch_unwind` with fallback `None` and
  answers `None` without asking while the screen is up. The panic drive's
  injection removed both guarded asks: the agent's present read the
  host's store unguarded and the window died at site 10.
- **T84.6: `Backend::invalidate_images`, required.** The plan names
  `Gpu::invalidate_images()` and "the window's calls to it", but the window
  holds a `Box<dyn Backend>`, so the call reaches the GPU through a new
  required trait method: the software backend writes the no-op itself (it
  caches nothing), and `examples/glasswall`'s forwarding `Counting`
  forwards it.
- **T84.6: the retry is seen as a failed-to-healthy transition.** R is a
  window key, but `app.retry` arrives through the agent inside the session,
  which the window never sees, so the window compares `Session::failed`
  with the last frame's at the head of every redraw and invalidates on the
  first healthy frame after a failed one.
- **T84.6: `Barred` is built at the present.** It keeps the
  `&mut dyn Session` as planned, and that borrow cannot be held across the
  three questions the window asks the session between where `Barred` was
  built and the present (`repaint_all`, `failed`, `damage`); the bar's
  text is decided early and the frame built inside the present branch. The
  bar's own damage is still the whole surface (`present_needed` forces it).
- **T84.6: `ImageStore::replace` answers `Option<u32>`, not `u32`.** It
  counts an id's replacements from 1 and answers the count, the frame to
  paint; a refused frame (wrong length, a zero side) answers `None` and
  leaves the last picture, where a bare `u32` would have had to answer
  something for a frame it did not take.
- **T84.6: the GPU resolves through the chain on a hit too.** The key's
  origin is which store answered, so every image command asks the frame's
  store first; the answer is a borrowed slice, so a hit costs a map
  lookup and copies nothing.
- **T84.6: the drive edits the format and `#pic` in one write**, so the
  watcher sees one change and the counts the drive waits on (two, three,
  four reloads) are unchanged. Step 1 asserts the first hot build's colour,
  so a `Hot` forward answering `None` is named there, before any edit.
- **T84.6: the swap's `invalidate_images` call is gated live now, not
  after M88/T88.2.** The plan deferred the window's call site to
  `hot-window-drive --gpu` once `ui.regionHash` reads the GPU, but the GPU
  legs already read the window with `xwd` (M83/T83.8). `#pic`'s colour is
  edited in the same write as the WGSL colour, its frame stays 0, and after
  the swap its centre must be the new build's: with the swap's call
  removed it read v1's colour, distance 156. The retry's call stays
  unobserved — the panic drive has no GPU leg.
- **T84.6: panic-app's job trigger moved to 11**, since site 10 is
  `App::images`; the drive's `// SITE n` scan takes two digits.
- **T84.7: a stopped job is not re-issued across a swap.** Not in the
  plan. Stopping a subscription takes it off `running`, so its job, still
  unsettled, became an ordinary job in flight to `Host::reissuable`, and a
  swap would have restarted the stream the application stopped (true
  before T84.7 too; the flag makes it visible). `Ledger::unsettled` leaves
  a stopped job out; `reissue.rs`'s gate went red with the filter removed
  (`[8, 14]` against `[14]`).
- **T84.7: the gate waits 35 ms before the stopping frame.** The first
  message's frame un-declares and the next frame stops it about a
  millisecond later, so the inbox was empty at the stop and the plan's
  injection 1 (the drain ignores `stop`) could not fire. With the wait,
  three messages are waiting (asserted as setup from iterations less
  landed, not from the mechanism) and injection 1 reads 3 applied, by
  name. The iterations assertion runs before the messages one, so
  injection 2 is named by the iterations (20) and injection 1 by the
  messages.
- **T84.7: `settled` is unchanged; M88/T88.1 carries progress-after-stop**
  (Decisions, M11), as T84.5 found. A stopped job settles the ordinary
  way once its body returns and the drain has consumed what it sent;
  T88.1's slot drain honours `stop` and makes `settled` treat a stopped
  job as settled.
- **T84.7: the drain checks the epoch first.** A result both stale and
  stopped is counted in `Inbox::stale`, not `stopped_dropped`.
- **T84.8: a handler cannot be handed a value, so the pass gained a
  route for one.** The plan wrote `on_seek: fn(&mut S, f32)`, but a
  handler is the one `fn(&mut S, &EventCtx)` a node's `Handlers::handler`
  answers, copied out of the tree before `&mut S` exists: no table can
  return a `fn(&mut S, f32)` and no `fn` can capture the widget's pointer.
  Handlers also run only on activation, and a captured release activates
  nothing, so a drag's value had no way to reach the application at all.
  `Node::take_action` (core, behind `std` with `App::outbox` and
  `App::subscriptions`) is the route: after `on_event` answers `true`, in
  the walk and under capture, the pass takes `(k, action)` once and runs
  node `i + k`'s handler with `ctx.event = Event::Action { action, .. }`.
  So `on_seek` is `fn(&mut S, &EventCtx)` and reads the fraction with
  `video::seek_fraction(ctx)`; the widget still computes it (P19), and
  P19's objection to an associated `seek_ms(&EventCtx)` — the ctx carries
  no geometry — does not apply to a value the widget put there. Behind
  `std`, the firmware links none of it: Δ 0 on all four. Its first build,
  ungated, cost `firmware_interactive` **+488 B** (`run_taken` inlined at
  both sites and the ctx copied in); out of line with the ctx by reference,
  **+320** (`run_taken` 184, `dispatch` +108, 25 default `take_action`s
  4 B each, `.rodata` +44 for the slots), which the strict K4 row of
  `docs/results-images.md` would have stopped the task at.
- **T84.8: the video root routes a reader's actions to its parts.** A
  reader's default action and `SetValue` arrive at the `Video` node, whose
  handler is neither the play button's nor the track's; the root consumes
  an action aimed at itself (`ctx.target == ctx.node`, so a press on the
  picture that bubbles there does nothing) and names the part in
  `take_action`'s `k`.
- **T84.8: the derive could not express a message.** The showcase is a
  derived application, and a derived app's `Msg` was `Never` (M28/T28.2),
  so a feed sending frames could not be derived — the plan's showcase
  entry assumed it could. `#[app(msg = T, apply = f)]`,
  `#[app(subscriptions = f)]` and `#[app(jobs = f)]` are the four keys;
  `msg` without `apply`, `apply` without `msg`, and `jobs` without `msg`
  are refused by name (`fail_app_*_without_*.stderr`), and the unknown-key
  message lists all eight.
- **T84.8: a drag previews and its release seeks.** A seek re-keys the
  feed, and one per pointer move restarted it per move; the press seeks,
  the knob follows the drag, and the release seeks where it ended.
- **T84.8: a key is an identity, so it carries a seek counter.**
  `VideoKey { source, seek, start_ms }`: a second seek to the position the
  running feed started from would otherwise be the same subscription, still
  declared, and the host would restart nothing.
- **T84.8: a paused seek shows its frame on the frame thread.** No feed
  runs while paused, so the showcase's `on_seek` opens the feed at the new
  position and copies its first frame into the store itself — per seek,
  never per video frame.
- **T84.8: a video frame frees once on the frame thread.** The plan bounds
  allocations at 0, and they are; the message's own buffer is dropped once
  `replace` has copied it, **1** free per frame, printed and bounded at
  one. Handing buffers back to the worker needs a return channel, and the
  no-allocator profile has no video.
- **T84.8: the scrub's planned live injection cannot fire.** With the
  drain's stop check removed the scrub stayed green, 0 stale frames: the
  window drains each frame as its waker fires, and the worker asks
  `is_stopped` before every send, so at the stop nothing from the old feed
  is waiting and nothing more is sent. T84.7's headless gate waits 35 ms
  before its stopping frame for the same reason, and it is the gate that
  owns the drain's check. The live injection is the host never setting the
  flag, which removes both halves: **15** frames from stopped feeds landed
  in 400 ms, by name.
- **T84.8: two live injections needed moving to fire.** The seek's, first
  written in `Video::layout` (the track's width set to the video's), left
  the live seek green on the `Boundaries` leg: a press marks only the
  track, whose own layout sets its width back before the event that reads
  it. Written in `SeekTrack::layout`, it fires (0.253 against 0.75). And an
  injection that breaks the stop or the seek also breaks the pause of
  whichever action runs first, so each live injection run puts its own
  action first, and that action fails by name.
- **T84.8: an injection helper must restore with a fresh mtime.** Moving
  the backup back left the source older than the last build, so cargo kept
  the injected binary, and later runs drove a window with an earlier
  task's injection still in it; it surfaced as a pause that did not hold.
  Every injection was re-run on a clean build, and the numbers recorded are
  from those runs.
- **T84.8: the showcase's clip is 400 ms**, so the plan's "m:ss / m:ss"
  reads "0:00 / 0:00" on it; the readout carries the position in ms for the
  drive.
- **T84.8: `book/src/gaps.md` never listed video**, as it never listed
  animated images (T84.3); `docs/competitive-feature-gaps.md`'s Images row
  said "Video: M84b" and now says what shipped.
- **T84.8: the drive runs six legs**, as T84.3 found, and the track is
  `SeekTrack`, a second `impl Node`, so it has its own `a11y_roles.rs` row
  (a slider named "Seek") and a `SeekTrack = Video` line in
  `scripts/drive/undriven.allow`.
- **T84.9: K84b fires on the machine, not on the binding.** The probe's
  dependency set is 58 crates (bound 150) and its `appsink` → RGBA path
  needs no `unsafe` (it type-checks under `forbid`), so the first two
  conditions hold; the reference machine has GStreamer 1.24.2's runtime
  and plugins and none of its `-dev` packages, so the third fires
  (`docs/results-images.md` T84.9). With linker-name symlinks in the
  probe's own directory the feed links and plays a `videotestsrc` frame
  of 320 × 240 RGBA: the path is sound, and only the gate would skip.
  `gstreamer` 0.24.5 also needs `kstring` held at 2.0.2 on the pinned
  rustc 1.94.0 (2.0.5 asks for 1.96).
- **T84.9: the `check_no_unsafe.sh` list needs no edit either way.** The
  standing gate (every crate under `crates/` carries `forbid(unsafe_code)`
  but `fulgurite-reload`) was K84b's second condition, and the probe
  shows a `GstFeed` would meet it as an ordinary crate; the bindings'
  own `unsafe` is a dependency's, as `winit`'s is. No FFI exemption was
  weighed further because none is needed.
- **T84.10: the reopen condition's first clause is already met**, so the
  deferral record names the condition that actually blocks it. The plan
  wrote "a GStreamer binding below 150 crates with a safe `appsink`, or a
  pure-Rust decoder of one common codec"; `gstreamer` 0.24.5 is both
  below 150 and safe from our side, and what fired is the third
  condition. `docs/results-images.md` T84.10 keeps the plan's two clauses
  and adds the operative one: the reference machine gaining
  `libgstreamer1.0-dev` and `libgstreamer-plugins-base1.0-dev` (an owner's
  install, never a task's), after which T84.10's first branch is built as
  written. `fulgurite-video` is not added; `Video` ships with
  `VideoFeed` and `GifFeed`.
- T84.11 (M84b's close): **K84e held and K84b fired, on its third
  condition only**; K4 held at every task and M84b's total, measured
  `15d4aec` → `0bf7103` in one invocation, is +0 on all four; K1, K2 and
  T84.7's K3 held. The numbers are `docs/results-images.md`'s, "M84b's
  kill rules, at its close". Nothing was built at the close. Recorded
  there as well as above: `Node::take_action` (core, behind `std`) and the
  derive's `msg`, `apply`, `subscriptions` and `jobs` keys are M84b's
  additions outside its Changes lists (T84.8); the coordinator accepted the
  hook as the same kind as `activation_redirect` and `selects_on_focus`,
  changing no other milestone and no owner decision. Two fixes landed
  inside M84b for defects it surfaced elsewhere: `553ac3c` (the drives read
  the showcase's compiled catalogue through `--entries`, since T83.6's
  `cfg`'d entry had skewed every later index read from `catalog.rs`'s text)
  and `11c7aa2` (`scripts/check_default_crates.sh` on a parentless `HEAD`,
  which CI's selftest copy is).

**What the review found (M84).**

- **C11** → adopted, changed: the version is in the command as
  `frame: u32`, not `generation`. One field serves a sequence's index
  and a stream's counter, and `ImageSource::image` takes it. The
  sentence "the trait does not change" is deleted. T84.0(a)/T84.2 gate
  non-`None` damage and the new pixels.
- **C12** → adopted: an in-place `write_texture` on a same-size frame
  change, `AtlasStats::image_uploads`/`image_upload_bytes`, and a gate
  of 0 resets over 100 changes (T84.2, K84d).
- **C13** → changed: no `Arc` is shared with the backend. M84a needs no
  shared store (frame selection is in the command). M84b's store is
  application-owned and borrowed by the renderer through
  `FrameSource::images` each present. That survives
  `take_images`/resume untouched and follows a swap (T84.6); on the
  GPU only with the swap's `Gpu::invalidate_images()` (second review,
  A14).
- **C29 / A26 / P4 / M21** → adopted: the producer trait is
  `VideoFeed`.
- **G15** → adopted: `frame` is folded in `Recording::image` and
  `hash_into`; software gate T84.0(a)/T84.2 with the `Recording`
  injection.
- **G16** → adopted: `AtlasStats::image_uploads`. GPU pixels are read
  through `Gpu::read_rgba` in `fulgurite-wgpu`'s tests (`just gpu`).
  The agent cannot see the GPU, and T84.3 says so, with M88/T88.2's GPU
  `frame_rgba` as the reopen condition (`fetch_rgba` stays `None`).
- **G28 / M20 / P30** → adopted: every gate in T84.0–T84.10 names a
  mechanism-breaking injection. T84.3's is a tick that writes and
  answers `false`, plus a deadline ignored.
- **G29 / A24 / M4** → adopted: `Image` gets a drive inventory and
  loses its `undriven.allow` line (T84.3). `Video` gets an inventory,
  `observe`, an a11y role and a showcase entry in T84.8. There is no
  `AnimatedImage` type to cover.
- **G32** → adopted for the core change: `scripts/firmware_delta.sh`
  (M80/T80.0's, cited in T84.0(d)) gives an exact per-firmware diff
  against the parent, bound 0 in T84.2.
- **B7** → changed: the firmware clause for `image` is dropped, since
  no firmware can reach a shell feature. The crate-count half is kept,
  exact (T84.0(c)). The `nm` symbol half is rejected: the linker drops
  an uncalled decoder whatever the feature says (M30,
  `docs/results-widget-crate.md`), so a leak into the default set could
  never turn a symbol count red. The crate count can. The index now
  records `image` as K2's `nm` exception with T84.0(c) as its
  falsifiable half (second review R12, T84.1).
- **B20** → adopted: in place on same size, re-insert only on a size
  change. Gate: 100 changes, `resets` 0, bytes per upload = w·h·4. A
  dedicated texture is K84d's fallback.
- **A5** → adopted for M84's settings: the window/session table in
  Decisions. `App::images` is read per present through the session,
  never through `Config`.
- **A6** → changed: frame identity is in the command stream (adopted).
  The mutable store is application-owned behind a required
  `FrameSource::images`, not a `Send + Sync` table. `Hot`'s forward is
  compiler-enforced, and T84.6 drives it across a real swap.
- **A7** → adopted: the consumer list is in Decisions. T84.2's
  injection "Recording omits the frame" reads 0 px damaged.
- **A12** → adopted, as priced: T84.3 prints `layout_ns` per animated
  frame, and K84c builds `App::tick_marked` if it exceeds 100 µs.
  `App::wake_in_ms` keeps the mark at the animation's rate, not at
  vsync.
- **A20** → adopted: K84b fires on any `unsafe` in
  `fulgurite-video`. No `check_no_unsafe.sh` exemption.
- **A25** → rejected: nothing `image` adds crosses the seam. The store
  crosses only as `&dyn ImageSource`, a core trait under M49/T49.1's
  source hash. A feature entry in `abi_fields` would refuse a swap that
  is safe. The accept direction is checked in T84.6 (`stamp-lib
  --features image` → `accept`).
- **P1** → adopted (see C11). Damage after a bump is gated at 1.00× the
  image rect, with the fold-dropping injection.
- **P2** → changed: `ImageSource::image` keeps a borrowed slice. The
  store is owned by the application and read by the renderer through the
  frame on the same thread, so no guard is needed, and the verifier's
  point that a GIF needs no trait change is subsumed by `frame`. A hot
  build's store reaches the backend through `FrameSource::images`,
  gated after a swap by `just hot-window-drive` (T84.6), and on the GPU
  by T84.6's invalidation gate (second review, A14).
- **P3** → adopted: keyed on `ImageId`, frame stored beside it,
  `write_texture` in place, `uploads` counter, 0 resets over 100 frames.
  The second review changed the key to `ImageKey { origin, id }`, added
  a per-draw side slot for one id at two frames, and invalidation on a
  swap (P1, A14).
- **M3** → adopted: there is no trailing docs task. Each task carries
  its handoff, results, rustdoc, book and allowlist changes. T84.4 and
  T84.11 are only the results summary and the queue entry with the
  `ci-verify` SHA.
- **M7** → adopted: the consumer list is `Recording`, `hash_into`,
  `wire`, `svg`, `Canvas`, `Gpu` and `contrast`, and the caller
  `Image`; M87's `Markdown` call is T87.4's (second review, R20). The injection "omit the
  frame from `hash_into`" is named by the journal gate. The
  `Boundaries`-vs-`Everything` shape is T84.3's live injection.
- **M17** → adopted: T84.1 corrects `image.rs:10-14`,
  `plan-capabilities.md` T15.2 and `plan-a11y-platform.md` :28/T54.9,
  and adds the handoff marker. T84.2, T84.6, T84.7 and T84.8 add their
  own **DECIDED** lines.
- **M18** → adopted: no "images chapter". The book's existing
  `leaves.md` (`## Image`), `animation.md`, `damage.md`,
  `hot-reload.md` and `gaps.md` take the prose, by named anchor.
- **Found while redrafting, not in the digest.** A removed
  subscription is only forgotten by the host
  (`lib.rs:1359-1363`), so a seek's old decoder would keep delivering.
  T84.5/T84.7 build the cooperative stop that `plan-tasks.md` §4
  deferred until a job like this existed.

**What the second review found (M84).**

- **A13** → adopted, M84's part: `Barred` forwards `images()` to its
  session, and T84.6's failed-build step gates the region hash under
  the bar. `paths`/`uniforms` in `Barred`'s copy are returned to M82
  T82.5 and M83 T83.3 as requests.
- **A14** → adopted: atlas key gains `origin`; `Gpu::invalidate_images()`
  on a swap and an M36 retry; a `just gpu` swap test, the live `--gpu`
  leg reopened after M88/T88.2 because `ui.regionHash` cannot read the
  GPU (T84.3's note).
- **P1** → changed: in-place rewrite kept for the common case; a second
  frame of one id in one draw takes a side slot `(ImageKey, frame)`.
  Gate compares GPU with software at both centres; injection the
  id-only key.
- **P2** → adopted (with A13).
- **P3** → adopted: `#[app(images = path)]`, with its table, user and
  `.stderr` rows, chosen over a hand-written `impl App`.
- **P4** → adopted: bound 1 + changes = 101, both printed.
- **P5** → adopted: `tick(0)` → `None`, then four `tick(100)` →
  1, 2, 3, 0.
- **P19** → adopted: the widget computes the fraction;
  `on_seek: fn(&mut S, f32)`; `Video::seek_ms` dropped; midpoint gate
  at 0.5 ± 1/width.
- **M11** → adopted: whichever of T84.7 and M88/T88.1 lands second
  extends the other; progress-after-stop gate, bound 0. §5 and M88's
  text returned as requests.
- **M23** → adopted: (a) `fail_app_unknown_key.stderr` named in T84.3
  and T84.6; (b) firmware compared against the parent commit.
- **R20** → adopted: T87.4 owns the `Markdown` call; the conditional is
  dropped. M87's sentence returned as a request.
- **D6 / D7 / X5** → no text in this plan is affected: M84 adds no
  required `Painter` method (`Painter::image` stays provided), needs no
  clipboard tool, and adds no `deny.toml` acceptance.
- **B15, M3, X7, R12** (from `plan-iced-roadmap.md`) → adopted: T84.2's
  K3 names `R=1 benches-competitive/run.sh -p fulgurite-frame`, eight
  invocations a side, and the printed labels; T84.3 adds the `Image`
  row to `roles.py` (its `a11y_roles.rs` row exists, `:71`); T84.8
  adds `Video` rows to both; T84.1 adds the `--features image` clippy
  line and cites the index's K2 exception.
- **B5/B16** (from `plan-text-fit.md`) → adopted: T84.0(d) cites
  `firmware_delta.sh BASE [BIN=B ...] [print]`, positional, and
  T84.0(d) and T84.2 write their invocations out in full with all four
  `BIN=0` bounds.
- **A3** (from `plan-text-fit.md`) → adopted: `Local::images` and the
  `wake_in_ms` read go through `Local::caught` with fallback `None`;
  because `Host::next_deadline_ms` takes `&mut self` (`lib.rs:2254`)
  and `caught` hands out `&Host`, the wake value is read in `caught`
  and passed to a new `Host::next_deadline_with`. Each gets a
  `panic-app` site and a recovering-leg step with an outside-`caught`
  injection (T84.3, T84.6).
- **G26** (from `plan-iced-roadmap.md`) → adopted, merged with B15:
  K3's rows are index K3's (`idle (floor)`, `ALL rows: +paint`,
  `+render (damage)`), checked against `arms/frame/src/main.rs:147,
  172-173,331-334`.
- **C7** (from `plan-paint-vocabulary.md`) → adopted: the expected wire
  bump is 4 → 5, after M80's bump and M82's two (T82.1, T82.2).
- consistency-3 item 6 → T84.0(c) names K2's exact command, with the
  `sed` step, not "deduplicated".
- consistency-3 item 11 → `Barred`'s sentence follows M82/M83: the
  first of T82.5/T83.3 derives `Clone` and `Barred` copies with
  `src.clone()`, so no store is added to the literal.
- consistency-3 item 12 → T84.1 adds `cargo doc -p
  fulgurite-shell --features image` (`decode_png`, `DecodedImage`).
- consistency-3 item 24 → T84.1 removes `docs/results-images.md`'s
  `doc-paths.allow` entry.
