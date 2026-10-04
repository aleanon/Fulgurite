# M81 — shaped paint: RTL drawn, the measurer chosen, subpixel pens (includes D1 moves a and c)

> **Written 2026-09-28 as a section of `docs/plan-iced-roadmap.md`;
> revised 2026-09-29 after that plan's seven-lens review, and moved to
> its own file. Not started.** The shared byte inventory, seams (S1,
> S2), test and doc discipline, and plan-wide kill rules K1–K4 are in
> `docs/plan-iced-roadmap.md` §2–§4 and bind this plan; this file does
> not restate them.

*Split in two. **M81a, size L**: closes RTL painting and shaping
strategy selection (iced "RTL text", "Shaping strategy selection"), and
carries D1 moves (a) and (c). **M81b, size M**: closes subpixel glyph
positioning. M81a depends on **all of M80** (T80.2 line height, T80.3
overflow, T80.4 `Selectable`, T80.5 `caret_rect`): each is re-gated
under `Shaping::Complex` here (M11). M81b depends on M81a, because a
fractional pen exists only under `Complex` (T2). This is
`docs/plan-text-correctness.md` §2.1 option A, built renderer-side
rather than as a display-list command: see the first decision.*

**Decisions.**

- **Shaping happens where it already happens: at paint, inside
  `FontStack::place_run_into`.** Both backends already shape every run
  every frame through that one function (`text.rs` `place_run`'s doc,
  "Both paths shape here"; `software.rs:161`, `lumen-static-wgpu`
  `lib.rs:1296`), so the renderer holds the shaper beside its faces and
  `Cmd::Text` stays the command (T4, option (a)). **No `Cmd` variant, no
  `Painter` method and no `TextMeasure` method are added.** The per-frame
  `GlyphRuns` store and `Cmd::Glyphs` are not built: a run would have to
  travel from layout (the only place a measurer is, `cx.rs` `LayoutCx`)
  to paint (`Node::paint` has only `&[NodeState]` and a `Painter`), and a
  store index folded by `Cmd::hash_into` is the M27/T27.3 phantom-damage
  defect. `Cmd::Text` keeps its string, so `hash_into`/`hash_from`,
  retention, `svg.rs` (`<text>`, which a viewer shapes itself) and the
  contrast walk are unchanged, and S2 does not arise in M81. **Budget:**
  `Cmd` 60 B unchanged; D3's accepted `glyphs` growth is not used.
  **Crate:** `lumen-static-shell`.
- **The shaping mode is a property of the `FontStack`.** A `Shaping {
  Basic, Complex }` enum (`#[repr(u8)]`, `Copy`; `Complex` exists only
  with the shell's `shaping` feature) and `FontStack::with_shaping`.
  The frame measurer (`FontMetrics::new(stack)`), the agent's audit
  measurer (`session.rs:397`, built from `font.clone()`) and both
  renderers hold clones of one stack, so they cannot shape differently.
  `FontMetrics` stays concrete, so `Local::set_scale` still reaches
  `FontMetrics::set_scale` (T8) and `generation()` needs no new forward.
  **Budget:** `FontStack` +1 B (the mode) and one `Arc`; under `Complex`
  only, each face's bytes are kept resident once per process (an intern
  table keyed by (path, collection index), or by content hash for
  `from_bytes`), because `rustybuzz::Face` borrows its bytes and fontdue
  keeps none (`Bold::get` `take()`s them, `Slot::get` drops them).
  The pinned face is 759,720 B; K81c bounds the rest. A hot swap loads a
  new library image with its own table, so each swap re-reads the faces
  once, as it already leaks the library (M37: v1 never unloads).
  **Crate:** shell.
- **D1 (a): the dependency turns round.** `lumen-static-text/src/lib.rs`
  names only `lumen_static`; its `lumen-static-shell` dependency serves
  `tests/paint_shaped.rs` alone, so it becomes a `[dev-dependencies]`
  entry, and the shell gains `shaping = ["truetype",
  "dep:lumen-static-text"]`, default-off. `lumen-static-winit`,
  `lumen-static-wgpu` (dev), `benches` and
  `benches-competitive/arms/frame` forward it, all from T81.1 (the
  frame arm has only `simd` and `observe` today, so K3's "shaping on,
  `Basic` selected" could not otherwise be built). In T81.1–T81.2 the
  text crate's dev edge to the shell (`features = ["truetype"]`) does
  not enable `shaping`, so no cycle forms; when T81.3 moves the pins
  into the shell, that dev-dependency is dropped rather than kept as a
  dev cycle under `shaping`, which would compile two copies of the text
  crate. Crate count and stripped size are recorded under K2 and K81e.
  `deny.toml` accepts RUSTSEC-2026-0206 (rustybuzz unmaintained) as
  "lumen-static-text's shaper ... a default-off crate"; T81.1 rewrites
  that reason and its comment to the new placement (the shell's
  default-off `shaping` feature, and the window's `Complex` paint), in
  the same commit that moves the dependency.
- **D1 (c): `GlyphKey` in the core.** `lumen_static::text::GlyphKey {
  px_bits: u32, index: u16, face: u8, phase: u8 }`, `Copy + Eq + Hash`,
  8 B. It is the type the shaper (text crate) hands the rasteriser
  (shell), which neither crate could otherwise name across D1 (a). The
  shell's `GlyphId` becomes a newtype over it with a private field: it
  keeps its contract (equal ids from one stack are identical bitmaps),
  converts to a `GlyphKey` publicly and from one only inside the shell.
  `GlyphId` stays 8 B (its 1 spare byte takes `phase`), so
  `GlyphCache` and `Atlas<GlyphId>` keep their sizes. **Budget:** a
  type the firmware never names links nothing: 0 B `.text`, printed.
- **The shaper: `Shaper` in `lumen-static-text`, over a `FaceSource`.**
  `FaceSource` is a two-method trait the shell implements for
  `FontStack`: `face_for(ch, weight) -> (u8, char)` answers by the
  stack's own rule (`face`, `other`, `fallback_for`, the substitution
  table in `prepare`), so the rule is not copied into a second crate
  (T3 (2)); `bytes(face) -> Option<&'static [u8]>` returns the interned
  bytes, loading a fallback on first use as `Slot::get` does. `Shaper`
  keeps one parsed `rustybuzz::Face<'static>` per face id, rustybuzz's
  shape plans per (face, direction, script), and its `UnicodeBuffer` for
  reuse. Per paragraph: `unicode_bidi` levels (skipped when no character
  has an RTL bidi class, so a Latin run allocates nothing); runs split by
  face; shaped in logical order with chars pushed one by one so a
  substitute keeps its source byte; greedy line breaking at UAX#14
  opportunities from `unicode-linebreak` (+1 crate, no dependencies)
  within `max_w` (T3 (3)); then each line reordered visually. Output is
  a `ShapedRun` of `ShapedGlyph { key: GlyphKey, x, y, line }` and
  `Cluster`s. Joining across a line break is not re-shaped (recorded).
  `ComplexText` stays, as a single-face `FaceSource` over `Shaper`, so
  its API and `tests/shaping.rs` survive. **Crate:** text.
- **Clusters keep the `GlyphSink` contract, which `ComplexText` broke.**
  `cx.rs` says clusters arrive "in source order" with `x` "the left
  edge"; `ComplexText` reports them in visual order. From T81.2 every
  measurer reports source order, with `x` the **leading** edge (the right
  edge in an RTL run) and a new `Cluster::rtl: bool`. `Cluster` is 10 B
  of fields in 12, so `rtl` takes padding: **12 B unchanged**, printed.
  Every `Cluster` constructor sets it; `BakedFace`'s does, so the
  firmware's `.text` delta is printed in T81.2.
- **Choosing is a `Program` setting, and the window follows it.**
  `Program::shaping(Shaping)` (default `Basic`, M32's rule that window
  settings live on `Program`). `Shaping` lives in the shell's `text`
  module, which is `#[cfg(feature = "truetype")]` (shell `lib.rs:42-47`)
  while `program.rs` is not, so `Program::shaping` and its stored field
  are `#[cfg(feature = "truetype")]`; the default build that
  `just test-crates` makes still compiles. `Local::from_program` builds its stack
  `.with_shaping(program's)`. A new **required** `Session::shaping()`
  tells `Win` which stack to render with; `impl Session for Hot` must
  forward it or fail to compile, which is the whole forwarding gate (no
  stamp-change test: the stamp is a whole-crate source hash, M49/T49.1).
  `Win::new` builds its stack with `FontStack::system()` today,
  independent of the session (`lumen-static-winit` `lib.rs:676-677`), so
  it applies `session.shaping()` **before the first backend is built**;
  otherwise a static window with `.shaping(Complex)` would measure
  `Complex` in `Local` and render `Basic`. After a swap, `Win` compares
  `session.shaping()` with its own and, on
  a change, calls a new **required** `Backend::set_font` (surface
  backend through a new `Canvas::set_font`; the GPU through the existing
  `Gpu::set_font`) and repaints all. With several windows (M89), each
  window slot keeps its own stack, built from that slot's
  `session.shaping()`, and runs this compare in `swap_all`'s per-slot
  adopt step (`docs/plan-windows.md`, second review X2/A8). `abi_fields` and
  `process_features` gain `shaping` in T81.1, the task that adds the
  feature (index §2.3), because `Complex`'s discriminant
  crossing to a loader built without it is exactly the M37 "the feature
  field is the load-bearing one" case (A25). `Offscreen::new`, the
  `LUMEN_SHOT` path, and the showcase's headless `shot` and `demo_shot`
  (`examples/showcase/src/main.rs:638ff`, `:768-785`, which build
  `FontStack::system()` directly for the measurer and the `Canvas`, and
  which M57's D5 parity compares the live window against) take the program's
  `Shaping`. The other `FontStack::system()` calls (tests, examples,
  benches) keep `Basic`.
- **Subpixel pens (M81b) are thirds, from fontdue's public API.**
  fontdue 0.9.4 has no public pen offset (`metrics_raw`'s offset is
  private and always 0.0; T1). Its public `rasterize_indexed_subpixel`
  draws the outline at 3× horizontal resolution; summing three
  sub-columns at an integer shift gives the pixel coverage of the
  outline moved by a third of a pixel, less the 2/3·`offset_x` the 3×
  draw applies, which the shift corrects to within 1/6 px. So
  `GlyphKey.phase` is `0` = unphased (fontdue's 1× bitmap, every
  `Basic` glyph, so goldens cannot move) and `1..=3` = derived at thirds
  0, ⅓, ⅔ (every `Complex` glyph, so a run never mixes two rasterisers).
  **Phase is `Complex`-only**: under `Basic`, fontdue floors every glyph
  x and ceils every advance (`layout.rs:461`, `:508`) and
  `Scaled::text_spaced` rounds the run origin (moved there from
  `text_weighted` by M80/T80.1, which deletes the wrappers'
  `text_weighted` overrides), so the fraction is
  always 0 and "on request for Basic" would do nothing (T2). The three
  `.round()` sites (`software.rs:164`, `:248`; `lumen-static-wgpu`
  `lib.rs:1302`) call one shell function, `text::pen_of`, so the
  backends cannot disagree. **The phase is decided there**, by
  `pen_of(rect_x + p.x)` at the round sites, not in `place_run_into`,
  which sees no rect; `place_run_into` only marks a placement
  phaseable (`Complex`). In a real frame `rect_x` is whole
  (`Scaled::text_spaced` rounds run origins after M80/T80.1; today
  `text_weighted`, `display.rs:1260-1265`, M57/A15), so production fractions come from shaped advances inside a
  run. T81.7's probe decides; K81d is the fallback.
- **Not in M81, with reopen conditions.** Per-widget shaping (iced's
  `Text::shaping`): reopen if K81a fires or an application needs mixed
  modes. **K81a fired in T81.3 (2.17× at 1 KB, 3.63× at 100 KB), so it is
  reopened**; K81a's response is built, and `Complex` stays opt-in
  (`docs/results-text.md` M81/T81.3, "What building found"). Visual caret movement (Left/Right stay logical, as today).
  Bidi word order in `RichText`, `Markdown` and `RichTextEditor`, whose
  breaker lays words out left to right: each word is shaped, the order is
  not reordered; reopen with a paragraph direction on `RichText`. The
  `i18n::mirror_*` helpers mirror *layout*, which is not this roadmap
  item; they stay `plan-deep-review.md`'s. **No widget is added**, so
  S4's drive-inventory, `observe`, a11y-role and allowlist obligations
  arise only for the showcase entry T81.6 adds. `Style` and
  `NodeState` do not grow: their sizes, and the per-100-node figure,
  are unchanged from the parent commit's, printed (M82 and M86 grow
  them under D4 and are not ordered against M81).

**Tasks — M81a.**

- **T81.0 — pins and baselines that compile today.**
  - `crates/lumen-static-text/tests/paint_shaped.rs` gains
    `hebrew_paints_right_to_left`, `#[should_panic(expected = "the
    painter drew Hebrew in logical order")]`: `FontStack::place_run` on
    the pinned face for `שלום` must give glyph x strictly decreasing in
    logical order. **Printed:** ids and x per glyph. **Setup:** every
    letter has a non-zero glyph index in the pinned face (printed).
    Today the painter draws logical order, so it panics as expected.
    **Accept direction:** fed `ComplexText::shaped_glyphs`' x in place of
    `place_run`'s, the assertion passes and the harness reports "did not
    panic" (red), shown once in this commit and recorded in its body.
  - `crates/lumen-static-shell/tests/basic_placements.rs`: every
    placement of `tests/text_containment.rs`' corpus at scales 1, 1.25,
    2 through `FontStack::place_run_into` on the pinned face, folded into
    one checksum and asserted equal to `BASIC_CHECKSUM`, the cross-build
    pattern `tests/banded.rs` uses. **Printed:** checksum, glyph count
    (setup: > 0). **Injection:** floor instead of round one glyph's `y`
    in `place_run_into`, and the checksum names the difference. T81.3
    runs it again with `shaping` compiled in.
  - Baselines into `docs/results-text.md` (an "M81 baselines" row set):
    `just firmware-size` and `just firmware` (the four binaries), and
    `tests/font_heap.rs`' printed heap for the pinned face.
  - **S4:** `docs/results-text.md` rows; nothing else moves.
- **T81.1 — D1 (a) and (c): the dependency turned, and `GlyphKey`.** No
  behaviour change. `lumen-static-text/Cargo.toml` moves the shell to
  `[dev-dependencies]`; the shell gains `shaping` (empty of code yet,
  pulling the text crate); `lumen-static-winit`, `lumen-static-wgpu`
  (dev), `benches` (`shaping = ["lumen-static-shell/shaping"]`; its
  features today are `threads`, `observe`, `rdyn_opaque`, `look`,
  `cached`) and `benches-competitive/arms/frame` (same line) forward it.
  `abi_fields` and `process_features` gain `shaping` (index §2.3: in
  the task that adds the feature). Core gains `GlyphKey`; `GlyphId`
  wraps it.
  - **S4:** the text crate's manifest comment (graph count), the shell's
    and winit's feature comments with the measured crate counts (as
    `clipboard`'s and `dialogs`' are, `cargo tree -e normal` unique
    packages), `GlyphId`'s rustdoc, `Cargo.lock` (`check_locks.sh`),
    `benches-competitive/Cargo.lock`. `deny.toml`'s RUSTSEC-2026-0206
    reason and comment become "rustybuzz: the shell's default-off
    `shaping` feature (the window's `Complex` paint, via
    lumen-static-text); the harfrust port is not scheduled", and
    `just deny` runs green with it; the RUSTSEC-2026-0192 (ttf-parser)
    reason, which names rustybuzz as a path, is reread and kept.
  - **Gates:**
    - `size_of::<GlyphKey>() == 8` and `size_of::<GlyphId>() == 8`,
      printed. Accept: both hold at HEAD after the change.
    - every `GlyphId` of the corpus above round-trips through
      `GlyphKey` field for field (count printed; setup > 0).
      **Injection:** the conversion drops `face`; the gate names the
      field and the first id.
    - `scripts/release_window_shape.sh` (`just release-window-shape`)
      counts `rustybuzz::` and `unicode_bidi::` symbols in a release
      `examples/minimal`: **0 by default** (printed). The symbol-count
      positive control and its injection wait for T81.3: `shaping` is
      empty of code here, so nothing references either crate and the
      linker keeps 0 symbols with the feature too. Here the positive
      side is the graph: `cargo tree -e normal -p lumen-static-shell`
      unique packages with and without `--features shaping` differ by
      the text crate and its dependencies (both counts printed; bound:
      > 0). **Injection:** drop `dep:lumen-static-text` from `shaping`;
      the difference prints 0.
    - `abi_fields`: a library built with `shaping` and a loader without
      are refused (`just abi-stamp` gains the pair). Accept: both with
      it load. **Injection:** leave `shaping` out of `abi_fields`; the
      mismatched pair loads.
    - `just firmware-delta <parent> firmware=0 firmware_interactive=0
      firmware_bind=0 firmware_cascade=0` (K4's comparator,
      `scripts/firmware_delta.sh`): the four binaries' `.text`/`.rodata`
      deltas printed, Δ 0 on all four (`GlyphKey` is named by no
      firmware).
    - `cargo clippy -p lumen-static-shell --features shaping
      --all-targets -- -D warnings` and `RUSTDOCFLAGS="-D warnings"
      cargo doc -p lumen-static-shell --no-deps --features shaping`
      (the feature adds public items, `Shaping::Complex`; index §3.1)
      join `just fonts`, from the task that adds the feature.
- **T81.2 — `Shaper`, `FaceSource`, and clusters in source order.**
  Everything in the second and third decisions that lives in the text
  crate, plus `Cluster::rtl` in the core and the leading-edge contract
  in `cx.rs`. `unicode-linebreak` joins the text crate.
  - **S4:** `cx.rs`' `Cluster`/`GlyphSink` rustdoc (the contract),
    `ComplexText`'s rustdoc, `just deny` for the new crate, the text
    crate's graph count.
  - **Gates** (text crate, pinned face; each prints its quantity):
    - Hebrew clusters arrive in source order with leading-edge x
      strictly decreasing, and `rtl` set (x list printed). Accept: a
      Latin run in the same test is strictly increasing with `rtl`
      clear. **Injection:** skip `visual_runs` (lay runs out in logical
      order); the gate prints the increasing x.
    - wrap: the corpus paragraph at `max_w` 80..400 px step 8: every
      line's width ≤ `max_w` (worst overshoot printed, bound 0.5 px, the
      painter's `paint_width` slack); a break falls only at a UAX#14
      opportunity or inside a word wider than `max_w` (count printed).
      Setup: the 80 px case has more than one line (printed).
      **Injection:** ignore `max_w`; the 80 px case prints one line and
      its overshoot.
    - weight and fallback: a `FaceSource` whose face 0 lacks Hebrew and
      whose face 2 has it gives Hebrew keys `face == 2`; a 700 run takes
      face 1 (faces printed per char). **Injection:** always answer
      face 0; the gate names the first Hebrew char.
    - `size_of::<Cluster>() == 12`, printed; `just firmware-delta
      <T81.1> firmware=64 firmware_interactive=64 firmware_bind=64
      firmware_cascade=64` (K4's comparator) prints the four binaries'
      `.text`/`.rodata` deltas; bound 64 B of `.text` per binary (one
      `bool` store in `BakedFace`'s `Cluster` constructor, core
      `font.rs:101`), tighter than K4's 1,000 B. The headline and
      four-binary table in `docs/results-embedded.md` are rewritten in
      this commit whatever `check_text.sh`'s 1% says (K4).
      **Injection:** `BakedFace` computes `rtl` through an
      `#[inline(never)]` range-check chain over the Hebrew and Arabic
      blocks, called per cluster; `firmware_interactive`'s `.text`
      delta prints over 64 and the comparator fails, naming it. (The
      comparator bounds `.text` only, M80/T80.0, so a table in
      `.rodata` could not fire it.)
    - `tests/shaping.rs` stays green on the rebuilt `ComplexText`.
- **T81.3 — `Shaping` on the stack: both backends paint shaped.**
  `FontStack::with_shaping`, the intern table, `FaceSource for
  FontStack`, and under `cfg(feature = "shaping")` the `Complex` branch
  of `measure_run`, `shape_into` and `place_run_into`; `ShapeScratch`
  (and `FontMetrics`) keep a `Shaper` and a `ShapedRun`. A `Placed` is
  built from each `ShapedGlyph` with fontdue's `metrics_indexed` for
  size and ink offset, on the baseline and cap shift the `Basic` path
  uses. M80's `line_height` reaches the `Complex` branch the way it
  reaches the `Basic` one: the placer re-places each line at the pitch
  itself, and fontdue's `LayoutSettings.line_height` is not used (M80,
  `docs/plan-text-fit.md`). The pins move into
  `crates/lumen-static-shell/tests/paint_shaped.rs`
  (`required-features = ["shaping", "truetype"]`), lose `should_panic`
  and build the stack `.with_shaping(Shaping::Complex)`; since the pin
  reads `FontStack::place_run`, which *is* the painter's placement, it
  now reads what is painted (G2, T7). The companion
  `painted_arabic_is_its_isolated_letters_in_logical_order` stays, on a
  `Basic` stack, relabelled as the `Basic` pin. `just fonts` gains
  `--features truetype,shaping` runs of the tests below.
  - **S4:** `docs/plan-capabilities.md` T15.8 ("measurement only" →
    painted under `Complex`, not yet selectable in a window), the text
    crate's rustdoc (the "measurement only" and "A window cannot install
    it" sections), `docs/results-text.md` K81a/K81c rows,
    `docs/results-allocation.md` for the new arms.
  - **Gates:**
    - the Arabic pin: painted ids equal the shaper's, ink inside the box
      (ids and box printed); the Hebrew pin: x strictly decreasing.
      **Injection:** `place_run_into` ignores the stack's mode; both pins
      print the unshaped ids.
    - `tests/text_containment.rs` on a `Complex` stack, through a `Host`
      label at scales 1, 1.25, 1.5, 2, with the corpus plus Hebrew,
      Arabic and mixed runs: wrapped count and overhang printed beside
      the existing bounds. It measures with one stack and paints with a
      clone, as a window does (A29). Setup: the mixed runs report more
      than one direction. **Injection:** the `Complex` measure ignores
      `max_w` while paint honours it; the wrapped count names cases.
    - `tests/glyph_placement.rs` on `Complex`: vertical centring within
      1 px. **Injection:** drop the cap shift from the `Complex`
      branch; the gate prints the offset.
    - `basic_placements.rs` in a `shaping` build: checksum equals
      `BASIC_CHECKSUM`. **Injection:** take the `Complex` branch
      whenever the feature is compiled; the checksum differs.
    - `tests/font_heap.rs` gains a `Complex` arm: resident heap above a
      `Basic` stack ≤ the face's file size + 64 KiB (printed; K81c), and
      the existing `Basic` bound unchanged. **Injection:** keep the bytes
      under `Basic` too; the `Basic` bound fires.
    - `tests/alloc_gate.rs` gains `mod complex`: a changing Latin label
      and a still RTL screen allocate **0** per frame; a changing RTL
      label allocates only inside `unicode_bidi`, counted per changed
      run and asserted at that count (printed), the file's one text
      budget, recorded in `docs/results-allocation.md` with its reopen
      condition (a bidi pass into caller-kept buffers). **Injection:**
      build a shape plan per call instead of from the scratch; the Latin
      arm prints its allocations.
    - M80 under `Complex`: a `line_height` 12 label's second baseline
      is one gap below the first, measured and painted (both printed);
      a `TextOverflow::Ellipsis` RTL label's `…` glyph is the leftmost ink
      and a mixed label's `…` sits at its base direction's end (x
      printed). **Injection:** the `Complex` branch ignores
      `line_height`; the painted gap prints short.
    - GPU, `just gpu` (`lumen-static-wgpu/tests/parity.rs`): a new
      region holding `שלום world 123` on a `Complex` stack, bounded and
      printed like the others; setup: the region has ink. **Injection:**
      the GPU shapes with a `Basic` clone of the stack it was given
      (a GPU-only break; G9's "logical order" injection could not fire,
      since glyph positions are absolute); the region's differing pixels
      exceed its bound.
    - K81a arm: `--bin textarea` today prints only "keystroke + layout,
      {kb} KB" (`benches/src/bin/textarea.rs:65`) on one
      `FontStack::system()` (`:41`), and its positional arguments are
      document sizes in KB (`:45`). T81.3 adds, as **new rows** beside
      it, a paint phase (render into a truetype `Canvas`) and a stack
      selector (`--shaping basic|complex`, `complex` only under the
      benches `shaping` feature), printing "keystroke + layout + paint
      [basic]" and "[complex]". The existing "keystroke + layout" row,
      K3's arm, is unchanged (its median stays inside its parent
      spread). Setup: the `complex` run shaped > 0 runs through the
      `Shaper` (count printed). Run: `cargo run --release -p benches
      --features shaping --bin textarea -- --shaping complex 1 100`
      against `basic`, eight runs a side under `just bench-lock`, the
      ratio beside 1.5. **Injection:** shape each run twice; the ratio
      roughly doubles.
    - `--bin grid` gains the same `--shaping complex` switch for its
      truetype-canvas pass (K81b, T81.8); setup: shaped runs > 0
      (printed).
    - `just release-window-shape` positive control (moved from T81.1,
      where `shaping` held no code): above 0 `rustybuzz::` and
      `unicode_bidi::` symbols with `--features lumen-static-winit/shaping`
      (printed), still 0 by default. **Injection:** make `shaping` a
      default feature of the shell; the default count goes above 0.
    - K2/K81e: `cargo tree` counts for the shell and winit with
      `shaping` (printed into their manifests); stripped release
      `examples/showcase` delta with it, printed beside 3 MB.
- **T81.4 — `Program::shaping`, `Session::shaping`, and the window.**
  Everything in the sixth decision.
  - **S4:** `docs/00-handoff-static-tree.md`: "Shaped paint **OPEN**"
    becomes **DECIDED** with a pointer to this plan and
    `docs/text-render-adoption-decision.md`, which gains a DECIDED
    section (renderer-side shaping, and why not a glyph command);
    T15.8 in `docs/plan-capabilities.md` done; `book/src/text.md`'s
    capability-table row for `lumen-static-text`; `Program` and
    `Session` rustdoc; `plan-text-correctness.md` §4's pointer.
  - **Gates:**
    - `tests/program.rs`: a `Program` with `.shaping(Shaping::Complex)`
      and a Hebrew label, through `Local` and an `Offscreen`: the label's
      region equals a direct `Canvas` render on a `Complex` stack and
      differs from a `Basic` one (differing pixels printed, both ways).
      Setup: the session's list holds a `Cmd::Text` with the Hebrew
      string (asserted). **Injection:** `Local` ignores the program's
      mode and keeps `FontStack::system()` (G10); the region equals the
      `Basic` render.
    - `tests/agent_audit_face.rs`: under `Complex`, `ui.lint` reports no
      W1005 on an Arabic label whose joined width differs from its
      isolated width (both widths and the finding count printed).
      **Injection:** build the audit from a fresh `Basic` stack; W1005
      fires (C26, T8).
    - `tests/scale_memo.rs` on `Complex`: after `Session::set_scale(2.0)`
      the label's box equals the physical measure over 2 within 0.5 px
      (printed). **Injection:** `Local::set_scale` stops forwarding to
      `FontMetrics::set_scale`; the gate names the stale box (T8).
    - a static (non-hot) window with `.shaping(Shaping::Complex)` and a
      Hebrew label: `ui.regionHash` of the label equals the
      `Complex` `Offscreen` render's and differs from the `Basic` one
      (all three printed). **Injection:** `Win::new` ignores
      `session.shaping()` and keeps `FontStack::system()`; the hash
      equals the `Basic` render.
    - the showcase's `shot`/`demo_shot` take the program's `Shaping`:
      with `.shaping(Complex)`, `showcase --demo` on a Hebrew entry
      differs from a `Basic` build of it (differing pixels printed).
      **Injection:** `demo_shot` keeps `FontStack::system()`; the two
      renders are identical.
    - `scripts/live/launch.py`'s `TRACKED` and `NAMES` learn `shaping`
      ("shaped text (shaping)") here, because the hot-window step below
      passes `--expect-features shaping` and launch.py refuses any
      feature outside `TRACKED` (`launch.py:82`, `:251-253`); the shell's
      `process_features` reports it from T81.1. Accept: a `shaping`
      build passes `--expect-features shaping`; refuse: a build without
      it fails with "expected shaped text (shaping) in this build".
    - `just hot-window-drive` gains a step: the copy's program is edited
      to add `.shaping(Shaping::Complex)`; after the swap a Hebrew
      label's `ui.regionHash` equals a fresh launch of the edited
      source, and differs from the pre-swap hash (both printed). Run
      twice, the second with the copy's program edited to
      `.invalidation::<Everything>()`, so both strategies see the
      repaint-all. The fixture and loader build with `shaping`, and
      `--expect-features` names it. **Injections:** `Hot`'s `shaping`
      returns `Basic` instead of forwarding (it compiles, and the hash
      matches neither); `Win` skips `Backend::set_font` after a swap.
    - `just abi-stamp`'s `shaping` pair (T81.1) re-runs now that the
      feature carries code: refused without, accepted with.
    - `cargo check -p lumen-static-shell` (default features, truetype
      off) builds with `Program::shaping` in place. **Injection:** drop
      the `cfg` on `Program::shaping`; the build fails naming
      `Shaping`.
- **T81.5 — caret, selection and hit test over bidi runs.** `TextInput`'s
  `x_of` (`input.rs:502`) and `TextArea`'s `cluster_at`
  (`textarea.rs:322`, read by `caret_at`, `:463`) look a byte up exactly
  (not `find(byte >= at)`, which assumed monotonic x); `offset_at`
  (`input.rs:512`, `textarea.rs:565`) picks the nearest leading
  edge; the tint is one rect per visually contiguous span of selected
  clusters, using `rtl` for each cluster's trailing edge; the end-of-run
  caret is the paragraph direction's end. M80's `Selectable` and
  `caret_rect` read the same helpers.
  - **S4:** the widgets' rustdoc for `TextInput::x_of`,
    `TextArea::cluster_at`/`caret_at` and both `offset_at`s; the book's
    text chapter by named anchor for the tint rule.
  - **Gates** (headless, pinned face, run on an `Everything` and a
    `Boundaries<256>` host; each asserts `handled` for the input):
    - typing into an RTL `TextInput`, and into an RTL `TextArea`: each
      keystroke's caret x is below the last (both lists printed).
      **Injection:** restore `find(byte >= at)` in `TextInput::x_of`,
      then separately in `TextArea::cluster_at`; the matching list
      prints non-monotonic.
    - selecting across the boundary of `abc שלום` paints two tint rects
      whose union covers exactly the selected clusters' ink columns
      (covered and stray columns printed; bound 0 stray, ±1 px).
      **Injection:** one rect from `x_of(a)` to `x_of(b)`; stray columns
      over `abc` are printed.
    - a click on the visual left end of an RTL `TextArea` line puts the
      caret at the line's logical end (offset printed). **Injection:**
      `TextArea::offset_at` picks trailing edges; the offset prints the
      line's logical start.
    - `Selectable<Label>` over a mixed run: the same tint gate, with the
      same one-rect injection.
    - `caret_rect` (M80/T80.5) at scale 2 under `Complex`: within 1 px
      of the painted caret's x (both printed). **Injection:**
      `caret_rect` keeps `find(byte >= at)` while the painter uses the
      exact lookup; the RTL caret prints a cluster's width off.
    - firmware: `TextInput` is linked by `firmware_interactive`, so
      `just firmware-delta <parent>` (default
      `firmware_interactive=1000`) prints the four binaries'
      `.text`/`.rodata` deltas; K4 applies (1,000 B on
      `firmware_interactive`).
- **T81.6 — the live drive, both strategies, and the close of M81a.**
  The showcase gains a `shaping` feature that sets
  `.shaping(Shaping::Complex)`, and a second `TextInput` entry,
  `#d_textinput_rtl`, seeded `שלום world 123`. `scripts/widget_drive.sh`
  gains two legs, `Everything+shaping` (`agent,everything,shaping`) and
  `Boundaries+shaping` (`agent,shaping`), each declaring `shaping`,
  which `scripts/live/launch.py`'s `TRACKED` learned in T81.4. Both legs
  drive the
  whole catalogue: every entry renders Latin under `Complex`, the widest
  assembly check this milestone has, and `scripts/drive/parity.py` (M57's D5)
  compares each entry's live pixels with `showcase --demo` built the same
  way, whose `demo_shot` renders with the program's `Shaping` since
  T81.4 (before it, `--demo` built `FontStack::system()` directly and
  `#d_textinput_rtl` would differ by construction). Setup: the parity
  script prints the `--demo` stack's mode, and it equals the leg's.
  - **S4:** `TEXTINPUT_RTL` in `scripts/drive/widgets/text_and_slider.py`
    (the entry's inventory; `check_drive_coverage.sh` stays green, since
    `TextInput` is already driven); a `scripts/drive/keys.py` `KEYS`
    entry for the new demo (`#d_textinput_rtl`, "typing edits it",
    since the runner fails a focusable demo with no entry); a
    `scripts/drive/expected.json` update naming the entry and its
    actions, with `counts` moved (run.py fails an addition until the
    manifest names it); the showcase entry's blurb; AGENT.md's
    queue entry citing `just ci-verify <sha>` (S5), in a one-line
    follow-up commit once that SHA exists.
  - **Gates:** `TEXTINPUT_RTL`'s action "a click at the visual left end
    then a character appends at the logical end": `handled`, the value
    ends with the character (printed), `visible=True`
    (`ui.regionHash` moves). **Injection:** `offset_at` picks trailing
    edges; the value prints the character at the start. M57's D5 parity
    per entry in both new legs: differing bytes 0, printed. **What a live
    drive cannot see:** glyph ids; T81.3's pins own that claim.

**Tasks — M81b.**

- **T81.7 — the phase probe, compiling today.**
  `crates/lumen-static-shell/tests/subpixel_phase.rs` (truetype) holds
  the derivation as a test-local function over fontdue's public
  `rasterize_indexed_subpixel`, for `l`, `o`, `W` and `ע` at 12, 16 and
  32 px on the pinned face. **Printed:** per glyph, `offset_x`; phase
  1's ink centroid beside `rasterize_indexed`'s 1× centroid for the same
  glyph (the **absolute** check); each later phase's ink-centroid shift
  from phase 1 (the relative check); and total coverage against
  `rasterize_indexed`'s. **Bounds:** absolute |phase 1 − 1×| ≤ 1/6 +
  0.05 px (the decision's residual after an integer sub-column
  correction); shift = k/3 ± 0.05 px; coverage within 1%. The relative
  check alone cannot see the correction: fontdue 0.9.4 draws the 3×
  outline once with a single `offset_x` from `metrics_raw(scale, glyph,
  0.0)` (`font.rs:614-626`), so a missing 2/3·`offset_x` displaces
  every phase of a glyph by the same constant, which cancels in every
  shift from phase 1 and leaves coverage unchanged. Setup: at least one
  probed glyph has `offset_x` ≥ 0.4 (printed), so the uncorrected error
  (2/3·`offset_x` ≥ 0.27 px) exceeds the absolute bound (0.217 px).
  **Injection:** shift by k sub-columns without the 2/3·`offset_x`
  correction; the absolute check names the glyphs with a fractional
  left bearing and prints their error. K81d reads this test's output.
  - **S4:** a `docs/results-text.md` row with the probe's table.
- **T81.8 — phase in the key, the pens, and the close of M81b.** The
  derivation moves into `FontStack::rasterize` for `phase >= 1`;
  `text::pen_of` replaces the three `.round()` sites; `Complex`
  placements carry a phase, `Basic` ones never do. The probe calls the
  shipped function.
  - **S4:** `GlyphId`'s and `GlyphCache`'s rustdoc ("no subpixel
    phase" becomes the phase rule), `FontStack::rasterize`'s,
    `docs/text-render-adoption-decision.md`'s DECIDED section, the book's
    text chapter by anchor, AGENT.md's queue entry with `just ci-verify
    <sha>` (S5) in a follow-up commit.
  - **Gates:**
    - one `Canvas` on a `Complex` stack draws `l` at x 10, 10⅓ and 10⅔:
      exactly three cache entries (`glyph_cache_stats().misses == 3`,
      printed) and centroid shifts of ⅓ ± 0.05 px (printed), plus
      T81.7's absolute check on the phase-1 bitmap through the shipped
      `FontStack::rasterize` (≤ 1/6 + 0.05 px, printed). This case
      reaches fractions through run origins on a raw `Canvas`, which
      `pen_of(rect_x + p.x)` honours. **A second case reaches them the
      production way**: a whole-pixel run origin and a `Complex` string
      whose shaped advances are fractional (e.g. `llll` at 13 px),
      counting the distinct phases its placements land on (printed;
      bound ≥ 2) and the misses (= distinct phases × distinct glyphs,
      printed). Setup: the `Complex` corpus has glyphs with a non-zero
      fraction (count printed, T2). **Injections:** `GlyphId` equality
      ignores `phase` (misses print 1 and the shifts 0, G33); drop the
      2/3·`offset_x` correction in `FontStack::rasterize` (the absolute
      check prints its error).
    - `basic_placements.rs` still equals `BASIC_CHECKSUM`, and the
      `Basic` corpus has 0 phased glyphs (printed). **Injection:**
      `pen_of` phases `Basic` too; the count goes above 0.
    - `just gpu`: the T81.3 parity region, now phased, within its bound.
      **Injection:** the wgpu site rounds instead of calling `pen_of`;
      the region's differing pixels print over the bound.
    - `tests/alloc_gate.rs`' `mod complex` arms stay at their T81.3
      values.
    - K81b on `just grid --shaping complex` (its truetype canvas, T81.3's
      switch): phase on over the twenty-edit sequence, printing misses
      per edit after warm-up beside the distinct (glyph, phase) keys
      each edit introduces that the cache has not seen, and `resets`.
      **Injection:** quantise to twelve phases while the key counter
      still counts thirds; misses print above 3× the keys.
    - Live: the T81.6 legs re-run; M57's D5 parity covers phased glyphs, and a
      phase is not separately observable live.

**Kill rules (M81).**

- **K81a — `Complex` costs too much on Latin.** `cargo run --release -p
  benches --features shaping --bin textarea -- --shaping complex 1 100`
  against `--shaping basic`, the "keystroke + layout + paint" rows
  T81.3 adds (they do not exist today), eight runs a side under `just
  bench-lock`. **Bound:** 1.5× `Basic`. **When it fires:** a run whose every character needs no
  shaping (no RTL bidi class, no joining or combining script) takes the
  fontdue path under `Complex`, gated byte-identical to `Basic`; the
  ratio is re-measured and both numbers recorded. It stays opt-in in
  either case, and per-widget shaping is reopened.
- **K81b — phases thrash the glyph cache.** `just grid --shaping
  complex`, phase on. **Bound:** `resets` stays 0, and misses per edit
  after warm-up ≤ 3× the distinct (glyph, phase) keys that edit
  introduces (both printed). Not a ratio to phase-off: the grid's own
  note expects misses after warm-up "only for digits not yet drawn"
  (`benches/src/bin/grid.rs:644-673`), about 0, so 3× phase-off is
  about 0 and fails correct code.
  **When it fires:** phases ship off (`Complex` glyphs use phase 1 only)
  and the numbers are recorded; reopen with a larger `GlyphCache`.
- **K81c — kept face bytes.** `tests/font_heap.rs`' `Complex` arm.
  **Bound:** file size + 64 KiB per face used. **When it fires:** T81.3
  stops; the alternative (bytes in an `Arc`, a `rustybuzz::Face` parsed
  per run) is measured under K81a, and the cheaper of the two ships.
- **K81d — no honest phase.** T81.7's probe. **Bound:** absolute ≤ 1/6
  + 0.05 px, relative k/3 ± 0.05 px, and 1% coverage for every probed
  glyph. **When it fires:** the `Complex`
  raster moves to `ab_glyph`, which takes a fractional position, behind
  the shell's `shaping` feature. The rasteriser is
  `FontStack::rasterize` in `lumen-static-shell`, whose graph has no
  `ab_glyph` (its dependencies are fontdue, lumen-static,
  lumen-static-lss, wide, the agent and serde_json); `Offscreen`,
  `LUMEN_SHOT`, hot libraries and every shell test link it without
  winit. So the count is taken on the shell's own `cargo tree -e normal
  --features shaping` (+3 expected: `ab_glyph`, `ab_glyph_rasterizer`,
  `owned_ttf_parser`, `Cargo.lock`), and on winit's Linux graph, where
  `sctk-adwaita` already pulls it (0 new); macOS and Windows counts
  printed. If that also misses the bound, or adds more than five crates
  on any of them, M81b is deferred. Reopen when fontdue exposes a pen
  offset.
- **K81e — the `shaping` feature's price.** `cargo tree` and a stripped
  release `examples/showcase`. **Bound:** 3 MB (a11y's bound,
  `docs/results-a11y-platform.md`, not its 4.12 MiB measurement).
  **When it fires:** T81.3 stops; the crate that moved it is named, and
  `unicode-linebreak` falls back to breaking at whitespace if it is the
  one.
- **K3 for this milestone.** `shaping` off is `cfg`'d out entirely; with
  it on and `Basic` selected, K3's arms by their printed labels
  (index K3) stay inside their parent spreads, eight invocations a
  side under `just bench-lock`: `R=1 benches-competitive/run.sh -p
  lumen-static-frame` with the arm's `shaping` feature (forwarded
  since T81.1), rows `idle (floor)` and `ALL rows: +paint`, and the
  one-row rows `layout`, `+paint` and `+render (damage)`; `cargo run
  --release -p benches --features shaping --bin textarea -- 100`, row
  `keystroke + layout, 100 KB`; and
  `cargo run --release -p benches --features shaping --bin cascade`,
  row `cascade_deep (whole table)`. **When it fires:** the branch in `place_run_into`
  moves behind a `FontStack` method chosen once per stack.

**What building found (M81).**

- T81.0: **flooring a glyph's `y` cannot move a `Basic` placement.**
  Every glyph the corpus places sits at a whole-pixel x and y (0 of
  6,600 fractional, printed by `basic_placements.rs`): fontdue lays
  glyphs out on whole pixels and `cap_shift` rounds the shift added to
  them. The planned injection ("floor instead of round one glyph's `y`
  in `place_run_into`") therefore left the checksum equal. The
  injection used is the rounding the plan meant: `cap_shift` floors
  instead of rounding, and the checksum names scale 1 first
  (`docs/results-text.md`, M81 baselines). The zero fractional count is
  also T81.8's "the `Basic` corpus has 0 phased glyphs" baseline.
- T81.1: **the graph's positive control compares against `truetype`,
  not the bare default.** `shaping = ["truetype", "dep:lumen-static-text"]`
  implies `truetype`, so the shell's graph with `--features shaping`
  against no features counts fontdue's eight packages too, and with the
  planned injection (drop `dep:lumen-static-text`) still printed a
  difference of 8. Against `--features truetype`, what every window
  builds, the difference is 11 and the injection prints 0
  (`scripts/release_window_shape.sh`).
- T81.1: **`just abi-stamp` had no shell fields to refuse on.**
  `fixtures/stamp-lib` stamped the core's fields alone
  (`export_abi_stamp!()`), so no shell feature could reach it. It and
  `fixtures/stamp-host` gain a `shell` feature (stamp and compare with
  `session::abi_fields`, as a window library and its loader do) and a
  `shaping` feature over it; the script builds the pair both ways.
- T81.1: the round-trip gate cannot turn a `GlyphKey` back into a
  `GlyphId` outside the shell (the constructor is crate-private, as D1 c
  requires), so it checks each id's key against fields known from
  outside the id (the size passed in, the face the weight picks, phase 0)
  and that distinct ids and distinct keys agree in number; the id is made
  from a key inside `place_run_into`, so the trip is both ways.
- T81.2: **one test in `tests/shaping.rs` asserted the defect.**
  `a_mixed_line_produces_runs_in_both_directions` required the reported
  bytes to go backwards somewhere, which is the visual order this task
  removes. "Stays green on the rebuilt `ComplexText`" held for the other
  eight; that one now asserts what it was for in source-order terms:
  bytes rising, x falling somewhere, both `rtl` values present.
- T81.2: **the first build was over the 64 B bound, and not by the
  `bool`.** `BakedFace::shape` grew 8 B; the literal `Cluster { ..,
  rtl: false }` in `InlineClusters::default` took `TextInput::default`
  out of line at 602 B (+150 B on `firmware`). Through a `const` element
  it is a clear again, and the net is **−220 B** (`docs/results-embedded.md`
  M81/T81.2). That saving hides the planned injection from a comparison
  against the parent (−48 B with it on `firmware_interactive`), so the
  injection was run against a dangling snapshot commit of this task's
  tree, where it prints +192 B and fails all four by name.
- T81.2: **the corpus has no paragraph and no word wider than 80 px at
  16 px.** The wrap gate joins the containment corpus's twelve strings
  into one paragraph; its 80..400 sweep breaks only at opportunities, so
  a 40 px case follows that breaks inside five words and exercises the
  in-word check.
- T81.2: **one committed face**, so the fallback gate's "face 0 lacks
  Hebrew" is the source's rule (it answers face 2 for the Hebrew block),
  with the pinned face behind all three ids; only the key's face can
  tell them apart, which is what the gate reads.
- T81.2: details the decisions left open, now the shaper's rustdoc: a
  `ShapedRun` also carries `ShapedLine`s (byte range, width, base
  direction), which T81.3's measure and ellipsis need; the trailing
  insertion point sits at the paragraph direction's end of the last line
  (x 0 for a right-to-left paragraph, lines being laid out from 0); a
  soft-wrapped line's width leaves out the whitespace it ends in; a line
  separator is shaped as a space, drawn as nothing and takes no width.

- T81.3: **K81a fired, and its response is built.** Every `Complex` run
  through the shaper cost 2.17× `Basic` at 1 KB and 3.63× at 100 KB on
  `--bin textarea`'s paint rows (eight runs a side, quiet), and the cost is
  rustybuzz's: 10.2 ms of the shaper's 12.7 ms on 100 KB of Latin,
  against fontdue's whole measure at 3.8 ms. As K81a says, a run with
  nothing to shape (`text::needs_shaping`: one pass over the characters
  against a list of ranges that need no shaping, conservative — a
  character not listed goes to the shaper) takes fontdue's path under
  `Complex`, gated to `BASIC_CHECKSUM` on a `Complex` stack with 0 runs
  through the shaper. Re-measured: **0.97×** and **0.99×**. `Complex`
  stays opt-in; per-widget shaping is reopened (Decisions). Two
  consequences for later tasks: the K81a rows' "setup: shaped runs > 0"
  now reads 0 under `complex`, because the document is Latin; and **`just
  grid --shaping complex` shapes nothing** (909 runs painted, 0 through
  the shaper), so K81b in T81.8 has no phased glyph to count on the grid
  as it stands — T81.8 needs a grid with text that needs shaping, or a
  different arm.
- T81.3: **a cut's `…` needed a widget change.** A cut `Label` paints its
  prefix and its `…` as two runs (M80/T80.3, for damage), so shaping
  alone cannot put the mark at a right-to-left paragraph's end. `Label`
  asks the measurer the prefix's direction (`fit::rtl`: the trailing
  cluster's `rtl`, one shape query on the memo's miss) and places the
  mark at the left, encoded in its existing `fit` field. Under `Basic` it
  answers left to right, so nothing moves; firmware Δ 0 (the cut is behind
  `alloc`). `AppBar`'s title cut is not moved: owed to whichever task next
  touches it.
- T81.3: **a stack built from bytes cannot shape.** fontdue keeps no
  bytes, and keeping them under `Basic` is K81c's injection. So the stack
  remembers where its faces came from: `FontStack::from_files` (and
  `system`, which builds through it) re-reads a file once per process,
  `FontStack::from_static` keeps the caller's `'static` bytes, and a
  `from_bytes` stack in `Complex` lays out as `Basic`. The intern table
  is keyed by path; content-hashing `from_bytes` input, which Decisions
  allowed, would have needed the bytes it cannot keep.
- T81.3: **`basic_placements.rs` builds its stack with `from_static`.**
  With `from_bytes` the planned injection ("take the `Complex` branch
  whenever compiled") could not reach the shaper and left it green.
- T81.3: **the allocation gate counts bidi runs, not shaped runs.** A
  Latin label in the same frame went through the shaper too and allocates
  nothing, so "per changed run" over every shaped run asserted the wrong
  number. `ShapedRun::bidi` and `TextCounters::bidi` count the runs
  `unicode_bidi` ordered; the RTL arm asserts allocations = bidi runs ×
  what `BidiInfo::new` and one `visual_runs` allocate (12 for its string).
  The Latin arm became two: a combining-mark label that needs the shaper
  but not bidi (0), and plain Latin, which reaches the shaper 0 times.
- T81.3: **the cap-shift injection could not fire on the 1 px centring
  bound.** On the pinned face the cap shift is 0 px at 16 px and 1 px at
  32 and 64, so dropping it kept every case within 1 px. The `Complex`
  gate now also requires the shaped run's ink rows to equal `Basic`'s at
  16–64 px, and the injection prints rows one off at 32 and 64.
- T81.3: **plain Latin no longer reaches the shaper**, so the `Complex`
  centring and line-height gates force it with a zero-width joiner
  (`needs_shaping` sends U+200D to the shaper; it inks nothing and takes
  no width), and the heap arm draws a run with Hebrew in it.
- T81.3: **gate details.** The containment arm needed a 240 px window:
  at 80 px the paragraph's label box was cut short by the window and its
  ink below the canvas, so "wrapped" is `Case::taller` there (the painted
  layout taller than the rect), a paragraph being meant to wrap. At
  `line_height` 12 consecutive lines' ink overlaps, so the line-height gate
  reads line starts from the placements (each line of its prose starts
  with a capital at the left edge) rather than from ink rows. The GPU
  region's setup first demanded two bidi runs, which the GPU-only
  injection broke before the bound could speak; it asks for one (the
  canvas's), and the injection is named by the bound (885 px over 2).
  K81c's injection (bytes kept under `Basic`) fires the new `Basic`
  bound (a file-built stack within 64 KiB of a bytes-built one); the
  older peak bound did not fire under it (22,045,185 under 23,966,104).

- T81.4: **the audit's W1005 has not read `set_text_measure`'s measurer
  since M57/A3.** `Local::agent_frame_done` hands the agent the session's
  own `FontMetrics` (`Agent::frame_done_measured`), so the planned
  injection — the audit built from a fresh `Basic` stack where `Local`
  installs it — left `tests/agent_audit_face.rs`' `Complex` arm green.
  The injection that breaks the mechanism builds that per-frame measurer
  from a fresh `Basic` stack instead; W1005 then fires (natural 183 px in
  a 126 px box).
- T81.4: **the showcase has no text that needs shaping.** No catalogue
  string holds a character `text::needs_shaping` sends to the shaper, so
  every entry's `--demo` is byte-identical under `Complex` and the planned
  "Hebrew entry" is T81.6's `#d_textinput_rtl`. The gate gives the
  `TextInput` demo a Hebrew value through `demo_frame`'s hook — the
  function `demo_shot` draws with — and the shipped Latin entry is its
  accept direction. The showcase's `shaping` feature, which T81.6 names,
  lands here (it forwards the shell's and sets `Complex` in `program()`),
  since the gate's `Complex` program needs the variant compiled in.
- T81.4: **`lumen_static_reload::hot::boot` took only `Program<A>`**, the
  default strategy, so a library whose program said
  `.invalidation::<Everything>()` could not be exported and the drive's
  `Everything` leg could not build. `boot` is generic over the strategy
  now (the `Session` hides it). The fixture names its strategy as `pub
  type Strategy` (`// EDIT strategy`), which the leg edits before it
  builds: an inline `.invalidation::<Everything>()` would change
  `program()`'s return type too.
- T81.4: **`Offscreen::new` keeps `Basic`; `Offscreen::with_shaping`
  takes the session's mode.** A signature change would have touched every
  headless drive for no gain; a session passes `Session::shaping`, which
  is what makes `Local` ignoring its program's mode (the `tests/program.rs`
  injection) land in the canvas too, so the region equals the `Basic`
  render as the plan says. `Canvas::set_font` drops the text scratch and
  the glyph cache with the old stack.
- T81.4: **the static-window gate and the hot step share one fixture.**
  `fixtures/hot-window` gains `#hebrew`, a `shaping` feature and `hot-window
  --hash basic|complex`, the headless `Local`-and-`Offscreen` render the
  live hashes are compared with; the static leg is "a fresh launch of the
  edited source", the copy as the hot leg left it, built without `hot`.
  The live hashes are taken at scale 1 (`WINIT_X11_SCALE_FACTOR=1`, and a
  setup check). **Under the `Win::new` injection the hash is not the
  `Basic` render's**, which the gate's text predicted: `Local` still measures
  `Complex`, so the window draws `Complex` boxes with `Basic` glyphs, which
  matches neither render — the same frame the two swap injections produce.
  The gate asserts equality with the `Complex` render, which all three
  break.

- T81.5: **leading edges cannot give a span's trailing edge at a
  direction change, so the shaper reports a second insertion point
  there.** In `abc שלום` the edge between the space and the Hebrew word
  is neither cluster's leading edge (the space's is its left, the
  word's first letter's is the word's right end), and the line's width
  and the trailing insertion point leave it underdetermined: one unknown
  per left-to-right-then-right-to-left transition. "Using `rtl` for each
  cluster's trailing edge" had nothing to read it from. So `Shaper`
  pushes, at every direction change on a line and at a paragraph's end
  when its last run runs against the paragraph, the ending run's
  trailing edge as a cluster with the next byte, *before* that byte's
  own cluster (`ShapedRun::clusters`; `Cluster`'s contract in `cx.rs`).
  `Cluster` stays 12 B and a run with no direction change reports
  exactly what it did; an offset at a direction change has two places on
  screen, and the list now says so. Two text-crate tests that asserted
  strictly rising bytes take the pair (`shaper.rs`'s mixed line now
  asserts it: bytes 4 and 12, one edge seen from both sides; injection:
  no pair pushed, the pair count prints 0).
- T81.5: **`find(byte >= at)` is the exact lookup on a list in rising
  byte order with one cluster per byte**, so the planned injection could
  not fire on a pure right-to-left string; it differs only where a byte
  appears twice. The typing gate types `שלום 123`, whose digits run left
  to right inside a right-to-left paragraph, so its end is a direction
  change; under the injection the caret answers the digits' trailing
  edge and both lists stop falling at the sixth keystroke.
- T81.5: **"each keystroke's caret x is below the last" cannot hold on
  screen**: lines are laid out from x 0 whatever their direction (T81.2),
  so a right-to-left paragraph's end, where the caret is, stays at the
  run's left while its start moves right. The gate measures the caret
  from the run's right edge, where a right-to-left paragraph starts.
  Aligning a right-to-left paragraph to its box's right is not this
  task's and is not built.
- T81.5: the click injection ("`offset_at` picks trailing edges") prints
  offset 6, before `ם`, not the line's logical start: the nearest
  trailing edge to the left end is the last letter's. Recorded as
  measured. The click gate works for a line a `\n` ends (the separator
  is a zero-width cluster at the paragraph's level, at the line's far
  end) and for the last line; a soft-wrapped right-to-left line has no
  insertion point at its far end and lands on its last letter (the
  `TextArea::offset_at` rustdoc says so).
- T81.5: **the three widgets share one `caret` module**, not three
  copies: `caret::at` (the lookup) and `caret::spans` (the tint), the
  latter non-generic over a `&mut dyn FnMut`, because `TextInput` is in
  every firmware binary and a generic copy per closure cost +1,464 B on
  `firmware_interactive` against K4's 1,000; shared, +844 B. `TextInput`'s
  `offset_at` also dropped the run's right end as its seed candidate when
  the measurer reports the text's end: at a right-to-left run's right end
  it tied the first letter's leading edge and won. `Selectable`'s line
  ends go to the left of a right-to-left line's last letter.

**What the review found (M81).**

- C4 → adopted: `Session::shaping` is required, the compiler is the
  forward gate, no stamp-change test; the runtime injection is `Hot`
  returning `Basic` (T81.4).
- C6 → changed: no `Program` enum constructs `ComplexText`; the mode
  lives on `FontStack`, the shell depends on the text crate behind
  `shaping` (D1 a), and face bytes are interned, bounded by K81c.
- C7 → adopted: `GlyphKey` in the core (D1 c); no core store holds it.
- C24 → rejected for M81: M81 adds no `Painter` method, so no wrapper
  list. The corrected list (leaf `DisplayList`; wrappers `Scaled`,
  `Deferring`, `Recording`; plus `Checksum` and `Sink`) is M82's/M83's.
- C25 → adopted: the `Stops`/`ImageSource` analogy is gone with the
  store.
- C26 → adopted: the audit shares the stack's clone; gated in T81.4.
- G2 → adopted: the pin reads `place_run`, the painter's own placement,
  on a `Complex` stack; the companion is the `Basic` pin.
- G7 → rejected: no `Painter` method, so no `Deferring` surface in M81.
- G8 → rejected: no per-frame store; `Cmd::Text`'s hash is unchanged.
- G9 → changed: the GPU injection shapes with a `Basic` clone (T81.3).
- G10 → adopted: one injection, `Local` ignores the program's mode.
- G27 → adopted: T81.0 and T81.7 hold only gates that compile today.
- G28 → adopted: every gate names its injection (T81.5's click and
  `caret_rect` gates had none until the second review, G32).
- G29 → adopted: T81.6's two `shaping` legs drive the catalogue under
  both strategies; no widget is added.
- G33 → changed: one canvas, three entries printed; K81b was a ratio to
  phase-off and is now bounded on the keys an edit introduces (second
  review G27).
- B6 → adopted: M81 adds no `Painter`/`TextMeasure` method; T81.1 and
  T81.2 print the four firmware binaries through K4's
  `just firmware-delta` (second review T13, R10).
- B11 → changed: the K81a arm is new rows T81.3 writes into
  `--bin textarea` (paint phase, `--shaping` selector); the existing
  K3 row is unchanged (second review C17, T18, M22).
- B12 → adopted: K81b on `just grid`'s truetype canvas, misses and
  resets.
- B16 → rejected: no run or uniform index exists to fold.
- B17 → adopted: `GlyphKey`; no `shape_run` method, so no provided-method
  hazard and no `BakedFace` slot.
- B18 → adopted: the "move behind a default-off shell feature" branch,
  via D1 (a).
- A8 → rejected: no out-of-line store.
- A15 → adopted: as C4; `Program::shaping` is a boot setting, and the
  injection is `Local` ignoring it.
- A16 → rejected for M81 (no `Painter` method); the transform-under-
  overlay question is M82's.
- A18 → adopted: `GlyphKey` in core; the shell owns the mode.
- A29 → adopted: ids never leave the stack that rasterises them; the
  measure/paint agreement is gated with two clones (T81.3).
- T1 → adopted: thirds from `rasterize_indexed_subpixel`, probed first
  (T81.7), `ab_glyph` or deferral under K81d; the probe gained an
  absolute check against `rasterize_indexed` (second review T2, R2).
- T2 → adopted: phase is `Complex`-only; T81.8 prints the non-zero
  fraction count.
- T3 → adopted: bytes interned, itemisation through `FaceSource`
  (the shell's own rule), `Shaper` wraps, weighs and falls back.
- T4 → adopted: option (a), renderer-side shaping in `place_run_into`.
- T5 → rejected: nothing indexes a shared buffer.
- T7 → adopted: as G2.
- T8 → adopted: `FontMetrics` stays concrete; both gates in T81.4.
- T9 → adopted: as C4.
- T21 → adopted: firmware deltas printed per task through
  `just firmware-delta`; M81 adds no command.
- M2 → adopted: as T21.
- M3 → adopted: no docs task; each task carries its own.
- M8 → adopted: as C4.
- M10 → changed: with no `Cmd::Glyphs`, M82's transform is honoured by
  `Path` alone; no ordering change (assembler note).
- M11 → adopted: M81a depends on all of M80; `line_height`, ellipsis,
  `Selectable` and `caret_rect` gated under `Complex` (T81.3, T81.5).
- M17 → adopted: the handoff's "Shaped paint" marker flips in T81.4.
- M18 → adopted: `book/src/text.md` named; no i18n chapter is invented.
- X9 → rejected: `Cmd::Text` keeps its string, so `svg.rs` and the
  contrast walk are unchanged.

**What the second review found (M81).**

- C1 → adopted: `shot` and `demo_shot` take the program's `Shaping`
  (decision; T81.4 gate with injection); T81.6's parity prints the
  `--demo` mode.
- C16 → adopted: `TextArea`'s site is `cluster_at`/`caret_at`; T81.5's
  text, S4 and injections name it.
- C17 → adopted: T81.3 writes the paint phase and `--shaping` selector
  as new rows; K3's "keystroke + layout" row is unchanged.
- G25 → adopted: `TRACKED`/`NAMES` learn `shaping` in T81.4, before its
  hot-window step passes `--expect-features shaping`.
- G26 → adopted: the frame arm forwards `shaping` from T81.1 (D1 a);
  K3 uses the index's exact row names. M84's row name is a request.
- G27 → changed: bound is misses per edit ≤ 3× the distinct (glyph,
  phase) keys the edit introduces, plus `resets` 0 (K81b, T81.8).
- G32 → adopted: T81.5's RTL click, `Selectable` and `caret_rect`
  gates name injections.
- T2 → adopted: T81.7 adds an absolute phase-1-vs-1× centroid check at
  1/6 + 0.05 px with an `offset_x` ≥ 0.4 setup; the relative check
  stays.
- T8 → adopted: T81.1 gates default = 0 plus a `cargo tree` count
  difference; the symbol positive control moves to T81.3.
- T13 → adopted: T81.1 (bound 0), T81.2 (bound 64 B) and T81.5
  (1,000 B) use `just firmware-delta`. T83.3's part is M83's.
- T14 → adopted: K81d counts the shell's own graph (+3 expected),
  `ab_glyph` behind the shell's `shaping`.
- T18 → adopted: as C17, plus the benches `shaping` feature in T81.1.
- T21 → adopted: the phase is decided by `pen_of(rect_x + p.x)` at
  the round sites; T81.8 adds a case through shaped fractional
  advances.
- T24 → adopted: `Win::new` applies `session.shaping()` before the
  first backend; T81.4 gates a static `Complex` window.
- M21 → changed: (b) `abi_fields`/`process_features` and the
  abi-stamp pair move to T81.1, re-run in T81.4; (a) reworded: the
  T81.1–T81.2 dev edge does not enable `shaping`, so no cycle forms.
- M22 → adopted: benches `shaping` in T81.1; `--bin textarea` and
  `--bin grid` gain `--shaping` with a shaped-runs > 0 setup (T81.3).
- R2 → changed: the absolute check adopted, at 1/6 + 0.05 px rather
  than ±0.05, since an integer sub-column correction leaves up to 1/6
  px by the decision's own derivation; applied in T81.8 too.
- R10 → adopted: as T13. The T83.3/K83b and T86.1/K86a parts belong to
  M83 and M86.
- R22 → adopted: as M21 (b).
- X5 → adopted: T81.1 rewrites `deny.toml`'s RUSTSEC-2026-0206 reason
  to the shell's `shaping` placement, `just deny` green. The §6 risk
  is a request to the index.
- D6 → no change: M81 adds no `Painter` method; nothing here
  attributes `text_spaced` or `fills` to D3.
- D7 → no change: M81 has no primary-selection gate.
- B4, B15, X5, X7, M3 (from plan-shaped-paint.md) → changed: the
  `firmware-size` line it names is gone; T81.1 already used the
  comparator, now in M80's positional form, Δ 0 on all four. The
  `deny.toml` reword was already in T81.1. The `shaping` clippy line
  joins `just fonts` in T81.1. T81.2 keeps its tighter 64 B per binary
  over K4's 1,000 B (T13 above), in positional form, and its injection
  now grows `.text`, since the comparator bounds `.text` only (the
  `.rodata` table injection could not fire it). K3 names the printed
  labels with `R=1`, eight invocations, `--bin textarea -- 100`.
  T81.6's S4 adds the `keys.py` entry and the `expected.json` update.
- A20, B16 (from plan-shaped-paint.md) → adopted: `Scaled::text_spaced`
  holds the origin rounding after M80/T80.1 (both sites); the
  firmware invocations take M80's positional form (T81.1, T81.5).
- T6 (from plan-shaped-paint.md, M83's digest) → adopted:
  `Program::shaping` and its field are `#[cfg(feature = "truetype")]`;
  T81.4 gates `cargo check -p lumen-static-shell` with the cfg-drop
  injection.
- R16 (from plan-shaped-paint.md) → adopted: `Style`/`NodeState` sizes
  are "unchanged from the parent commit's, printed", not literals.
- X2 (from plan-shaped-paint.md) → adopted: per-slot stacks under M89,
  compared in `swap_all`'s per-slot adopt step.
- consistency-3 item 12 → T81.1 adds `RUSTDOCFLAGS="-D warnings"
  cargo doc -p lumen-static-shell --features shaping` beside its clippy
  line (`Shaping::Complex` is public).
- consistency-3 item 19 → M57's parity gate is "M57's D5" (four
  sites), since owner D5 is the 80 KB ceiling.
