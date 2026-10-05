# M82 — the paint vocabulary: radial and style gradients, paths, vector text

> **Written 2026-09-28 as a section of `docs/plan-iced-roadmap.md`;
> revised 2026-09-29 after that plan's seven-lens review, and moved to
> its own file. M82a (T82.0–T82.4) built 2026-10-04 and closes on
> `just ci-verify` of its T82.4 commit (S5), cited in AGENT.md; M82b
> (T82.5–T82.9) built 2026-10-05–06 and closes on `just ci-verify` of
> its T82.9 commit (S5), cited in AGENT.md
> (`docs/results-paint-vocabulary.md`).** The shared byte inventory, seams (S1,
> S2), test and doc discipline, and plan-wide kill rules K1–K4 are in
> `docs/plan-iced-roadmap.md` §2–§4 and bind this plan; this file does
> not restate them.

*Split in two, each closing on its own `just ci-verify <sha>`.
**M82a — gradients** (size M): closes iced's "background gradients"
(radial, and in `Style`). **M82b — paths and vector text** (size L):
closes "vectorial text", and ships the `Path` command it needs. No hard
dependency. M82a's exact `Style`/`NodeState` sizes depend on what M80
took of the two spare bytes in each (§2's byte inventory), and each task
asserts the size it produces. M82b does not need M81: vector text is
drawn as paths. M81 builds no glyph command; text under a transform is
`VectorText`. Checked against
`lumen-static-merge` at `553fa5e` (the `92a4033` merge plus M61's
`.lss`, which is therefore present, not conditional). No `Request`
(S1) and no new `Session` method: fills are resolved in the host,
before paint, and paths travel in the `DisplayList` that already
crosses.*

**Decisions.**

- **The gradient widens inline. `Stops` shrinks to make room.** Today
  `Cmd::Gradient { rect, stops, angle }` (`display.rs:200`) is 16 + 36
  + 4 = 56 B. That is the largest payload, tied with `Glass`'s 56, and
  it is what sets `Cmd`'s 60 (`tests/glass_cmd.rs:15-26`). `Stops` is
  36 B because `at: [f32; 4]` (`display.rs:557-561`,
  `tests/vector.rs:96`). **`Stops::at` becomes `[u8; 4]`, in 1/255
  steps**, so `Stops` is 21 B with align 1. The variant becomes
  `Gradient { rect, stops, kind: GradKind, geom: [f32; 3], corner: f32 }`:
  16 + 21 + 1 + 12 + 4 = 54, padded to 56. `Cmd` stays **60 B**. That
  was measured with rustc 1.94.0 on a mirror of all twelve variants
  (this draft's scratch probe). T82.1 commits it as the payload mirror
  in `glass_cmd.rs`.
  - Rejected: `[u16; 4]` offsets. `Cmd` also measures 60, but only
    because rustc puts the tag into the payload's slack. The payload
    mirror measures 60 against the 56 bound, so the layout would rest on
    an optimisation the gate cannot see.
  - Rejected: a stops store out of line. It is the M27/T27.3 index
    hazard (S2), for 15 B that shrinking recovers.
  - Crate: `lumen-static` (`display.rs`). `Cmd` budget: +0 B.
- **`GradShape` is relative to the rect.** `Linear { angle }` or
  `Radial { center, radius }`. `center` is a fraction of the rect.
  `radius` is a fraction of the distance from the centre to the
  farthest corner, so 1.0 matches CSS `farthest-corner`. Both are
  resolution-free, for two reasons: a `const` table entry must not
  know its node's size, and `Scaled` then forwards the shape unchanged
  (only `corner` scales). It is circular only; ellipses are not built.
  - `Painter::gradient(rect, stops, shape, corner)` keeps its provided
    degraded default (§2's documented exception): it becomes
    `rounded_rect(rect, stops.sample(0.0), corner)`. The three wrappers
    (`Scaled`, `Deferring`, `Recording`) already override it.
  - **Corner coverage is new code on both backends (T15):** the software
    `fill_gradient` (`software.rs:1076`) multiplies by `sd_round_box`
    (`software.rs:2343`). The GPU's `KIND_GRADIENT` case
    (`shader.wgsl:289-298`) is hard-edged and discards outside the rect
    today. It gains `sd_round_box` coverage from `Quad.params.x`, which
    that kind leaves unused. Radial uses `uv = (cx, cy, R, 1)`; linear
    keeps `(ux, uy, span, 0)`. `Quad` and `Grad` (`lib.rs:103-126`)
    gain no field.
- **A gradient in `Style` is an index, resolved to content before it is
  painted.** `record_painted` runs at layout and draws nothing
  (`ops.rs:360-399`), and `paint` has no `&[Style]` and no table
  (C3, B4, T14). The path to the screen is:
  - `Style.background_fill: u8`. `0` is none; `n` is entry `n - 1` of a
    `&'static [Fill]` handed to **`Program::fills`**.
  - `record_painted` copies it into **`NodeState.fill: u8`**, in both
    its branches.
  - **`ops::fill(st, i, out)`**, beside `ops::border` (`ops.rs:96`), is
    called at every one of `border`'s **29 production call sites in 18
    files** at `553fa5e`, before the flat `bg`: the derive's container
    paint (`macros/src/lib.rs:1244`), `Repeat` (`repeat.rs:754`), and
    27 in 16 `lumen-static-widgets` files. Two of those 27 are the bare
    `border(st, i, out)` in `Label::paint` and `Button::paint`
    (`widgets/src/ops.rs:196`, `:497`, imported at `:21`), which a grep
    for `ops::border` misses; `tests/border_pixels.rs` is a test, not a
    site. T82.3 prints the count at landing. It reads `out.fills()`, a
    **required** `Painter::fills(&self) -> &[Fill]` getter, and issues
    an ordinary inline `out.gradient(..)` with the resolved stops.
  - So what reaches `Recording`, `hash_into`, the journal and the window
    is content, never the index (S2).
  - `Fill { stops, roles: [Option<Role>; MAX_STOPS], shape }` lives in
    `lumen_static::style`. `Host::set_fills` and `Host::set_palette`
    (`lib.rs:886`, which already re-cascades and `mark_all`s) resolve
    the roles against the palette (M25/T25.11's mechanism).
    `Host::set_fills` compares the resolved table with its stored copy
    and calls `invalidation.mark_all()` on a change, as `set_palette`
    does: under `Boundaries<256>`, the default since M34, a clean
    subtree is skipped before paint (`lib.rs:880-897`).
  - The host stores the result in the `DisplayList`s it paints into,
    through `DisplayList::set_fills`, which survives `clear` as `bounds`
    does: the frame list and the shadow oracle's scratch list
    (`lib.rs:528` copies `bounds` the same way; `sign_leaves` paints
    into that same list, so there is no second one).
  - The agent's journal list is not the host's: it is a private field
    of the agent's `Journal` (`agent/src/journal.rs:132`), painted from
    a `TreeRef` (`agent/src/lib.rs:73-81`) that carries no fills.
    `TreeRef` gains `fills: &'a [Fill]` and `TreeRef::with_fills`,
    which `session.rs:939` calls beside `with_sheet` with
    `Host::fills()` (the resolved copy); `measure_into` calls
    `list.set_fills(tree.fills)` before painting. The journal's
    container signature (`journal.rs:237-249`, state reads only) folds
    `st.fill` and a hash of the resolved entry, and `paints[i]` is also
    true for `st.fill != 0` with a non-empty table.
  - `Program::fills` is consumed on the **host side**, through
    `Program::host()` (`program.rs:590`), so a hot swap re-asks the new
    build. It never goes through the window's once-read `Config` (A5).
    The host keeps a resolved copy, not the `'static` pointer.
- **What the fill does and does not do:**
  - The fill paints under the flat `bg`. Hover and press shading still
    act on `bg`, so a fill with a transparent `bg` shows no hover. That
    is recorded, not fixed.
  - A disabled node's stops fade to 40% alpha, as `record_painted` fades
    `bg` (M57/A16).
  - Fills are not mirrored under RTL.
  - A `no_std` host's painter answers `&[]` from `fills()` and paints the
    flat `bg`; `Checksum` does exactly that.
  - **Budget** (`docs/plan-iced-roadmap.md` §2.1 owns the figures):
    - `Style` needs 1 B and `NodeState` needs 1 B.
    - M80 leaves `Style` no spare byte and `NodeState` one, so `Style`
      goes 80 → 84 and `NodeState` stays 84 (84/84). That is 164 →
      168 B/node: 16.0 → 16.4 KB per 100 nodes against 24 KB.
    - If a byte is left in either struct, that struct does not grow.
    - The task asserts the exact value it measures, never a disjunction.
- **`Fields::BACKGROUND_FILL`**: the next free bit at landing, printed
  (expected bit 19, after M80's 17 and 18), and `Fields::EACH` 19 → 20
  (`style.rs:458-495`). `Patch::background_fill` records the bit. Every
  kind whose paint calls `ops::fill` adds the bit to its `Seed::new`
  mask. A kind that does not paint a fill does not claim to read it, so
  W1010 reports a `background-fill` rule on that kind. **`.lss`:
  `background-fill: <index>`** is its own declaration in `lower.rs`'s
  `PROPERTIES` (beside `border-width`, `lower.rs:229`). It does not
  overload `background` (M24).
  - It takes a new **`Takes::Index`**: a bare integer 0..=255, no unit,
    described as "a whole number from 0 to 255, no unit, like `1`".
    `Takes::Byte` is a length: it lowers through `px()`
    (`lower.rs:511-513`), which accepts only `Unit::Px`
    (`lower.rs:553-557`), and describes itself as "like `2px`"
    (`lower.rs:189`). A table index in px would read
    `background-fill: 1px`, which means nothing.
  - It lowers to the existing **`Val::U8`** (`lower.rs:63`): `typed`
    gains a `Takes::Index` arm reading `Value::Number(n, Unit::None)`
    with no fraction and in range, as `Takes::Byte`'s does after
    `px()`. `include_lss!` already emits `Val::U8`
    (`lumen-static-macros/src/lss.rs:229`), so no macro arm and no new
    `Val` variant; the shell's `set` gains only the
    `("background_fill", Val::U8(x))` pairing.
- **Paths are a command whose points live in the list, as `Text`'s bytes
  do.** The pieces:
  - `Painter::path(&mut self, els: &[PathEl], xf: Affine, paint:
    PathPaint)` is **required**.
  - `PathEl` is `Move | Line | Quad | Cubic | Close` over `Point`.
  - `Affine` is `[f32; 6]`.
  - `PathPaint` is `{ fill, stroke: Color, width: f32 }`.
  - `DisplayList::path` applies `xf` as it copies the elements into a
    per-frame `DisplayList::paths: Vec<PathEl>`, and pushes
    `Cmd::Path { rect, els: (u32, u32), fill, stroke, width }` (36 B).
    `rect` is the ink: the transformed control-point box, grown by
    `width / 2`. `els` is `(offset, len)`, like `Cmd::Text`'s.
  - **`hash_into` folds `len` and never the offset**
    (`display.rs:666-694`'s rule). `hash_from` folds the elements,
    special-cased beside `Text` (`display.rs:1077-1085`).
    `Recording::path` hashes `xf`, the elements and the paint (A8, T5).
  - Crate: `lumen-static` (`display.rs`).
  - Budget: `Cmd` +0 B (36 < 56). Firmware growth accepted by D3
    (paths; header of `docs/plan-iced-roadmap.md`).
- **The transform is a per-path argument, not a stack.**
  - The plan's `PushTransform`/`PopTransform` needed three things:
    - a transform stack in `Recording`, to compute ink;
    - balancing in `Deferring`, whose phase 2 repaints deferred
      subtrees outside every scope (`overlay.rs:24-27`);
    - and a rule for every other command under a non-identity
      transform, where the plan wrote two contradictory ones (T17).
  - A per-path `xf` needs none of these. It is exactly as expressive for
    what the plan let the transform reach: `Path` only.
  - `Scaled::path` pre-multiplies `xf` by the scale and scales `width`.
    `Deferring` forwards. `Recording` computes the ink from `xf`, which
    it is handed.
  - Hit testing stays axis-aligned on `NodeState`'s box. M81 builds no
    glyph command; text under a transform is `VectorText`.
  - D3's allowance for `push_transform`/`pop_transform` goes unused.
    **Reopen** when a second command kind needs a transform (a rotated
    image, a rotated glyph run); that is a stack, and a kill rule of its
    own.
- **Software fills by signed-area accumulation.**
  - Coverage is `min(|acc|, 1)`, which is non-zero winding, and it is
    analytic, not sampled. No `unsafe`. The code is in
    `lumen-static-shell` (`software.rs`) as a public `rasterize_path`,
    so the GPU uses the same function.
  - Curves are flattened after `Scaled`, at 0.1 physical px.
  - Rows are clipped to the band before accumulation, so banded raster
    (M17) pays per band.
  - **Stroke** is the non-zero union of one quad per flattened segment
    and a 16-gon per join and cap: round joins and caps only, drawn by
    the fill rasteriser. Miter, dashes and even-odd are not built.
    **Reopen** on an SVG import or a user asking.
- **The GPU draws path masks, not a stencil** (T16).
  - The backend has no stencil or depth attachment (`lib.rs:604`,
    `:630`, `depth_stencil: None`) and runs at `sample_count: 1`.
    Stencil-then-cover would need a new attachment with a resize and
    suspend lifecycle, and would give binary coverage.
  - Instead, T82.6 rasterises each path's coverage with the software
    `rasterize_path` into the glyph atlas (`lib.rs:237`, R8), under
    `MaskKey::Path { hash: u64, paint: Fill | Stroke(width), frac }`.
    `hash` is a content hash of the path moved to its integer-floor
    origin and `frac` the fractional offset, so an integer-pixel scroll
    is an atlas hit.
  - A path has a fill and a stroke colour, and `KIND_GLYPH` draws one
    R8 texel times one colour (`shader.wgsl:299-302`). So one mask and
    one tile run per non-transparent paint: a filled and stroked path
    is two masks under two keys, never one key for both.
  - It draws `KIND_GLYPH` quads in 256 px tiles (`PATH_TILE`), so a
    large path never needs one oversized atlas slot.
  - There is no new pipeline, binding or attachment. Masks reset with
    the atlas, like glyphs. Parity with software is by construction:
    the coverage is the same bytes.
- **Vector text is outlines from the measurer.** A widget sees only
  `cx.text: &mut dyn TextMeasure` (`cx.rs:177`), which has no outline
  query (T18).
  - `TextMeasure::outline(&mut self, text, size, weight, sink: &mut dyn
    PathSink) -> bool` is **provided**, answering `false`, like
    `generation()` (`cx.rs`, "a wrapper must forward this"). All 28
    implementors at `553fa5e` are leaves: 24 test stand-ins and four
    real measurers, each with its intended answer:
    - `FontMetrics` (shell `text.rs:1103`): outlines, under
      `vector-text`;
    - `ComplexText` (`text/src/lib.rs:164`): outlines, through
      rustybuzz's face;
    - `BakedFace` (`font.rs:101`): `false`, a baked bitmap has no
      outlines;
    - `BlockMetrics` (`cx.rs:159`): `false`, it measures blocks, not
      glyphs.
  - No `TextMeasure` wrapper is planned: M81 keeps `FontMetrics`
    concrete with the shaping mode on `FontStack`
    (`docs/plan-shaped-paint.md`). T82.7 re-greps
    `impl TextMeasure for` and gates any wrapper it finds with a
    forwarding test.
  - `FontMetrics` implements it through `FontStack`, behind the shell's
    default-off **`vector-text`** feature, with outlines from
    `ttf-parser`. Glyph ids come from the layout, and **pens are taken
    as `extent_by` takes them** (`text.rs:418-430`), so advances equal
    what `Label` measures. Not from `place_run`: a `Placed.x` is
    fontdue's floored ink origin, `floor(pen + xmin)`
    (`text.rs:521-527`), and zero-size glyphs are skipped
    (`text.rs:511-513`), so outlines placed from it would apply each
    bearing twice and lose the fraction.
  - `ttf-parser` 0.25.1 is already in the default tree through fontdue
    0.9.4 (`Cargo.lock`), with features `no-std-float` and
    `opentype-layout`. Declaring the same features adds **0 crates**
    (C32).
  - Under the feature, `FontStack` keeps each parsed face's bytes. Today
    they are dropped after parse (`text.rs:79-110`).
  - `ComplexText` (`lumen-static-text`) implements it through
    rustybuzz's face, which derefs to `ttf_parser::Face`.
  - **`VectorText`** lives in `lumen-static-widgets` behind `alloc`. It
    memoises its outline on (text, size, weight, `generation()`).
    - Its box is the rotated bounds of the unrotated run, so its ink
      stays inside its box.
    - It paints one `path` with its rotation as `xf`.
    - It publishes `Role::Label` with the string as its value (A24).
    - When `outline` answers `false` it paints the string as ordinary
      axis-aligned text: visible beats faithful, the `glass` precedent.
  - **No `abi_fields` entry.** `FontStack` does not cross the seam: the
    library builds its own (`session.rs:387`) and so does the loader
    (`lumen-static-winit/src/lib.rs:677`). The `DisplayList` and `Cmd`
    that do cross are covered by the source hash (M49), and the wire
    `VERSION` is bumped where a field changes meaning: once in T82.1
    (`Stops::at`) and once in T82.2 (tag 6's `angle` becomes `kind`,
    `geom`, `corner`), each in its own commit. `Cmd::Path` (T82.5) is
    an appended tag and bumps nothing (`wire.rs:31-33`).
- **This supersedes handoff §12 item 8 in part**
  (`00-handoff-static-tree.md`, "How far the paint vocabulary goes",
  DECIDED 2026-09-10: the vector half "is not in `Cmd`"). The measured
  reason behind it was that box styling must not become paths
  (`docs/results-vello.md`: 2.3–2.9× on one thread because "neither
  becomes a path"), and it stands: no box command changes. One
  out-of-line `Path` command for the nodes that ask is a different
  claim. `clip-path`, dashes, skew of other commands and SVG stay out.
  T82.5 moves the marker in its own commit.
- **Painter implementors, and who handles `Cmd`** (C24). Every new
  required method (`fills`, `path`) is a compile error in each until it
  forwards or answers.
  - **Wrappers:** `Scaled` (`display.rs:1220`), `Deferring`
    (`overlay.rs:190`), `Recording` (`retain.rs:923`).
  - **Leaf:** `DisplayList` (`display.rs:1358`).
  - **Also:** `Checksum` (`fixtures/embedded/src/lib.rs:247`), `Sink`
    (`benches/src/bin/retain.rs:215`), and the test painters `Surfaced`
    (`tests/overlay.rs:482`), `Echo` (`painter.rs:20`), `Null`
    (`retain_align.rs:27`), `Flat` (`vector.rs:72`), `Plain`
    (`glass_cmd.rs:168`) and `Counting` (`container_fill.rs:39`).
  - **`Cmd` consumers**, each changed per command: `Canvas`, `Gpu`,
    `Svg` (a `Renderer`, `svg.rs:113`), `wire.rs`, `contrast.rs`,
    `hash_into`/`hash_from`, and `tests/svg.rs`'s exhaustive `kind`.
  - **Exhaustive matches with no wildcard**, which a new variant breaks:
    `lumen-static-shell/tests/clip_test.rs:122-142` (`render_cmds`),
    `lumen-static-wgpu/tests/parity.rs:57-69` (`kind`, and
    `KINDS = 11` at `:47`), `lumen-static/tests/overlay.rs:55-71`
    (`cmds`), `lumen-static/tests/glass_cmd.rs:58-70`, and
    `benches/src/bin/retain.rs`'s `ink`. T82.5 names each.
- **Firmware** (D3, D6, B6, T21, M2):
  - Accepted growth, by owner decision:
    - `path` and the widened gradient: D3 ("paths and the widened
      gradient are accepted firmware growth");
    - the required `fills` getter: **D6 (owner, 2026-10-02)**, with
      M80's `text_spaced`. An earlier draft listed `fills` under D3,
      which the owner had not said; D6 is the decision.
  - `outline`'s vtable slot (a provided `TextMeasure` method, so
    `BakedFace`'s vtable gains a slot) is in neither list. It is
    measured in T82.7 under K4's 1,000 B cap and reported, like
    `ops::fill`'s branch; it is not accepted growth, and the owner is
    told the number.
  - Each task prints its exact `.text`/`.rodata` delta for all four
    firmware binaries.
  - `firmware_interactive` links 61,922 B at the M56 merge
    (`results-embedded.md`, "re-measured at the M56 merge"), which
    leaves **19,998 B** under the 80 KB ceiling (81,920 B; owner
    decision D5, 2026-09-30, raised by M80/T80.0). Under the old 64 KB
    it had 3,614 B, and this milestone was expected to exhaust it.
  - The leaf headline is 28,616 B (`results-embedded.md:48`), and
    `check_text.sh` refuses a 1% drift. So every task that moves it
    rewrites the headline in the same commit.
  - `ops::fill`'s branch in firmware widgets is **in neither D3's nor
    D6's list** (D6 accepts the getter, not the call sites).
    It is measured the same way, and the owner is told the number.

**Tasks.**

*M82a — gradients.*

- **T82.0 — gates that compile today** (shared by M82a and M82b).
  - The comparator is M80/T80.0's `scripts/firmware_delta.sh BASE
    [BIN=B ...] [print]`, wrapped by `just firmware-delta <sha>
    [BIN=B ...]` (positional; `docs/plan-text-fit.md` T80.0); M82
    cites it and adds no interface. Below, `<parent>` is the task's
    parent commit, and with no `BIN=B` the bound is K4's default,
    `firmware_interactive=1000`; `print` never fails. It prints `.text` and `.rodata` per binary and each delta;
    this task records `firmware_interactive`'s headroom to 81,920 from
    it.
  - **Pins**, which assert today's gap so that the owning task's
    inversion is the red run:
    - `crates/lumen-static-shell/tests/contrast_ground.rs`: text over a
      `Cmd::Gradient` lands in `Report::unmeasured` with `"gradient"`
      (`contrast.rs:121`). T82.2 inverts it.
    - `tests/glass_cmd.rs` gains a `GradientPayload` mirror, printing
      56 beside the 56 B bound. T82.1 and T82.2 keep it at 56.
    - `tests/overlay.rs`'s `deferring_forwards_every_command` prints its
      command count, 6 after M80/T80.1 (its `text_spaced` call).
  - **Gates:**
    - The contrast pin asserts its setup landed: exactly 1 text run,
      over exactly 1 gradient.
    - (The comparator's own injection and accept runs are T80.0's.)
  - Docs in the same commit: `docs/results-paint-vocabulary.md`
    created with its threshold table (the kill rules below, written
    before anything is built), and its `scripts/doc-paths.allow`
    entry (committed by M80/T80.0) removed.
- **T82.1 — `Stops` offsets in 1/255.**
  - `Stops::at: [u8; 4]`. `Stops::two` and `sample` convert.
    `indicator.rs:375` and `wire.rs` are updated, and the wire `VERSION`
    goes up by one from the current version (expected 2 → 3, after
    M80's bump), because a field changed meaning.
  - **`svg.rs:83-89` writes `stops.at[k] as f32 / 255.0`.** It formats
    the offset with `{:.3}`, and precision is ignored for integers, so
    the type change compiles there and would write `offset="51"` for
    0.2. Every other consumer fails to compile (`retain.rs:1011`,
    `display.rs:711`'s `to_bits`, wgpu's `grad_of` at `lib.rs:1695`).
    The task also greps for silent consumers of `at` (format strings,
    `as` casts) and lists them in the commit body.
  - **Gates:**
    - **Size.** `tests/vector.rs:96` asserts
      `size_of::<Stops>() == 21`, printed. `glass_cmd.rs`'s mirror
      prints the unchanged `Gradient` payload of 56 beside 56, with
      `Cmd == 60`.
      - Injection: `at: [u16; 4]`. The mirror goes to 60 and the
        assertion fails, naming `Gradient`.
    - **Sampling.** `sample(t)` for 1,001 values of `t` against the f32
      implementation it replaces:
      - Offsets representable in 1/255 (0, 1, 51/255) give a max
        channel delta of **0**, printed.
      - An offset of 0.5 gives a delta of ≤ 1, printed with its count.
      - Injection: `at[k] as f32` without `/ 255.0`. Every interior stop
        is lost and the delta is printed at ≥ 100.
    - **Golden.** No existing golden holds a gradient (`golden.rs:26-32`
      renders a rect and a rounded rect; the other two are panic
      screens), so the old "every golden" check could not change. The
      task adds `tests/golden/gradient3.ppm`, a 3-stop linear with an
      interior stop at 0.2 (= 51/255, exact in both encodings),
      recorded at the parent commit and committed with it. After the
      change it is byte-identical (max delta printed, bound 0).
      - Injection: `at[k] as f32` without `/ 255.0`. The golden
        differs, delta printed at ≥ 100.
    - **SVG** (`crates/lumen-static-shell/tests/svg.rs`). A middle stop
      at 51/255 writes `offset="0.200"`, printed.
      - Injection: format the raw byte. It writes `offset="51"`.
    - **Wire.** A round trip of a 3-stop gradient at the new version is
      equal. `wire::VERSION` is asserted equal to a literal (expected
      3), printed.
      - The refusal fixture is a **committed byte literal** from the
        pre-change encoder, with a literal version byte (expected 2),
        holding a rect and no gradient, so its length decodes under the
        new layout and only the version check can refuse it. It decodes
        to `None`. (`wire.rs:354` builds its refusal case as
        `VERSION + 1`, relative to the constant, and an old gradient
        buffer is 12 B longer per gradient and refused by the
        trailing-bytes check either way, `wire.rs:344-359`; neither can
        see an unbumped version.)
      - Injection: leave `VERSION` unbumped. The fixture decodes to
        `Some`, and the literal assertion fails.
  - Firmware: `just firmware-delta <parent> print`. Expected about 0,
    since the firmware uses `gradient`'s default.
  - Docs in the same commit:
    - rustdoc on `Stops`, `MAX_STOPS` and `wire::VERSION`;
    - the `vector.rs` comment;
    - a results row.
- **T82.2 — radial and rounded gradients.**
  - `GradKind`, `GradShape`, the widened `Cmd::Gradient`, and
    `Painter::gradient(rect, stops, shape, corner)` at every call site.
  - Updated: `fill_gradient`, the shader and upload (`lib.rs:1205-1220`),
    `svg.rs:203` (`<radialGradient>` and `rx`), `wire.rs`, `hash_into`,
    `Recording::gradient` (fold `kind`, `geom` and `corner`), and
    `Scaled` (scale `corner` only).
  - **The wire `VERSION` goes up by one again** (expected 3 → 4),
    because tag 6's fields change meaning (`wire.rs:31-33`). Without
    it a T82.1 buffer would decode into wrong geometry, not `None`.
  - `benches/src/bin/vocabulary.rs` is **created** here, with `linear`,
    `radial` and `rounded-radial` arms in ns per Mpx. Each arm asserts
    its setup (exactly 1 `Gradient` of its kind in the list) before
    timing. T82.5 and T82.8 add arms to it. The same commit removes
    the `bin vocabulary` line from `scripts/doc-paths.allow` (M80/T80.0).
  - `contrast.rs` measures text over a gradient: the ground is sampled at
    the run's centre and both horizontal ends, and the minimum ratio is
    reported.
  - **Gates**
    (`crates/lumen-static-shell/tests/gradient_shapes.rs`; GPU in
    `lumen-static-wgpu/tests/parity.rs`, run by `just gpu`):
    - **Radial.** The centre pixel is stop 0 and the pixel at `radius`
      is the last stop, within 2/255, on both backends. The deltas are
      printed. Setup: the list holds exactly 1 `Gradient` of kind
      `Radial`.
      - Injection (software): `t` measured from the rect centre,
        ignoring `center`.
      - Injection (GPU): centre and edge swapped. Parity by region
        fails.
    - **Corner.** With `corner = 12`, the pixel at (1, 1) inside the
      rect has coverage ≤ 1/255 (printed), and the pixel at the rect
      centre is the stop colour, on both backends. Not (3, 3): its
      centre is 8.5·√2 = 12.02 px from the corner's centre, so
      `sd_round_box` ≈ 0.02 and coverage ≈ 0.48 on correct code
      (`software.rs:405-406`, `:2326-2350`). (1, 1) has sd ≈ 2.85.
      - Injection: the GPU ignores `params.x` for `KIND_GRADIENT`. The
        corner pixel is the stop colour (T15).
    - **Scaled.** At scale 2, the physical pixel (6, 6) is page colour.
      - Injection: `Scaled` forwards `corner` unscaled.
    - **Retention.** Moving a radial's `center` inside the same rect
      damages exactly the rect: area printed, equal to the rect's.
      - Injection: `Recording::gradient` omits `geom`. The damage is 0.
      - Accept: an unchanged gradient damages 0.
    - **Deferring.** The stream test passes a radial and stays at 6
      commands (the count at landing, printed; T80.1 made it 6).
      - Injection: `Deferring::gradient` forwards `Linear` whatever it
        was handed. The streams differ.
    - **Contrast.** The T82.0 pin inverts:
      - a run over a gradient whose second stop fails AA is measured, at
        a ratio below 4.5 (printed);
      - with both stops passing, the ratio is at least 4.5.
      - Injection: sample the centre only. A run whose end sits on the
        failing stop then passes.
    - `tests/svg.rs` finds `<radialGradient`.
    - **Wire.** As T82.1's: `wire::VERSION` equals a literal (expected
      4), and a committed literal buffer at the previous version
      (expected 3, a rect and no gradient) decodes to `None`.
      - Injection: leave `VERSION` unbumped. The fixture decodes to
        `Some`.
  - **Live:** none yet, because nothing puts a radial on screen before
    T82.3. What observes this task is `just gpu`'s headless parity.
    T82.3's Card action is its live check.
  - Firmware: `just firmware-delta <parent> firmware_interactive=400`.
  - Docs in the same commit:
    - handoff §12 item 8 records radial and rounded gradients as built
      (M82/T82.2), since they were already on its near side;
    - rustdoc on `Cmd::Gradient`, `Painter::gradient`, `contrast.rs`
      and `wire::VERSION`;
    - a results row with `benches --bin vocabulary`'s linear, radial and
      rounded-radial arms, in ns per Mpx.
- **T82.3 — `Style.background_fill` and the fill table.**
  - What changes:
    - `Style.background_fill` and `NodeState.fill`, stamped by
      `record_painted`;
    - `Fields::BACKGROUND_FILL` and `Patch::background_fill`;
    - `Fill` and `Fill::resolve`;
    - the required `Painter::fills` in every implementor listed above;
    - `DisplayList::set_fills`, and `ops::fill` at the 29 sites;
    - the masks of those kinds' `Seed`s;
    - `Program::fills`, `Host::set_fills` (compare, then `mark_all`
      on a change), `Host::fills`, and `set_palette` re-resolving the
      fills;
    - the fills applied to the frame list and the shadow scratch list;
    - the agent: `TreeRef::fills` and `with_fills`, set at
      `session.rs:939`; `measure_into` applying them; the journal's
      container signature and `paints[i]` counting the fill;
    - `Takes::Index` (lowering to `Val::U8`, no macro arm) and the
      `.lss` `background-fill` declaration,
      with its arm in the shell's `lss.rs` `set` (`lss.rs:90`; a
      pairing it lacks panics in `lss::load`) and a sheet using
      `background-fill` added to `lss_parity.rs`'s cases, so
      `include_lss!` and `lss::load` are compared on it;
    - `scripts/check_fill_paint.sh` in `just static`.
  - The showcase's Card demo gets a radial, rounded, role-named fill
    through `Program::fills`.
  - **Gates**
    (`crates/lumen-static-shell/tests/fill.rs` unless named):
    - **Sizes.** `tests/cursor.rs:32-33`, `tests/focus.rs:203-204` and
      `fixtures/embedded/src/lib.rs`'s `STYLE`, `NODE_STATE` and
      `PER_NODE_RAM` asserts are edited to the exact measured values.
      The commit prints both sizes and the RAM per 100 nodes against
      24 KB.
      - A used-bytes mirror beside the size (the `glass_cmd.rs`
        payload-mirror pattern): the sum of `Style`'s field sizes,
        printed beside the asserted value (expected 81 used of 84,
        `docs/plan-iced-roadmap.md` §2.1), and the same for
        `NodeState`.
      - Injection: a second `u8` added to `Style` beside the first. The
        used-bytes mirror fires (82); `size_of` alone would not, since
        82 still pads to 84 (`fixtures/embedded/src/lib.rs:410-414`
        records the same 4-byte slot).
      - Injection: a `u32` added to `Style`. The exact `size_of`
        assertion fires (88).
    - **Reaches paint.** Three sites: a derive container, a `Repeat`
      and a `Button`. A node with `background_fill(1)` emits exactly 1
      `Cmd::Gradient`, with `rect == rect_of(node)` and stops equal to
      the resolved entry.
      - Setup: `st[i].fill == 1`, and the rule's `Patch::fields`
        contains the bit. Both are printed.
      - Injection: `record_painted` does not copy the field. The
        gradient count is 0, and the test names the kind.
    - **Palette.** A role-named stop is `LIGHT.accent`, then
      `DARK.accent` after `Host::set_palette`. Retention damages
      exactly the node's rect: area printed, bound equal to the rect.
      - Injection: `set_palette` does not re-resolve the fills. The
        colour is unchanged and the damage is 0.
    - **`set_fills` marks.** On a `Boundaries<256>` host, a second
      `Host::set_fills` with a changed entry repaints the node: 1
      `Gradient` with the new stops in the next frame, printed. The
      same table again marks nothing (damage 0).
      - Injection: `set_fills` omits `mark_all`. The frame keeps the
        old stops.
    - **Journal** (`crates/lumen-static-agent/tests/`). A derive
      container with a transparent `bg` and a role-named fill:
      `set_palette` gives `ui.lastDamage` at 1.00× its box, printed.
      A `Button` with a fill reports the same.
      - Injection 1: the journal's signature omits `st.fill`. The
        container reports 0.
      - Injection 2: `TreeRef` passes `&[]` for fills. The `Button`
        reports 0.
    - **Index 0 is today.** A `Host` with a non-empty `Program::fills`
      and every node at `fill == 0` emits 0 `Cmd::Gradient` (count and
      node count printed). The goldens hold no `Host` and no fill
      table, so they cannot see this.
      - Injection: `ops::fill` paints entry 0 when `fill == 0`. The
        count prints > 0.
    - **W1010** (`tests/sheet_lint.rs`). A `background_fill` rule on a
      kind that does not read the bit is reported. On `Card`, which
      does, the finding count is 0.
      - Injection: `Patch::background_fill` does not record its bit.
        No finding (G11).
    - **Deferring** (`tests/overlay.rs`). A deferred node with a fill
      paints its gradient in phase 2 (count 1).
      - Injection: `Deferring::fills` answers `&[]`. The count is 0.
    - **CullTo**, if M85/T85.8 has landed. A filled node inside a
      `ScrollView` paints its gradient through `CullTo` (count 1,
      printed).
      - Injection: `CullTo::fills` answers `&[]`. The count is 0.
    - **Shadow oracle** (`tests/shadow.rs`, `shadow-oracle`). A fill
      tree reports no difference.
      - Injection: the scratch list is not given fills. The oracle fires
        on the lists.
    - **`check_fill_paint.sh`**, per call site, not per file, both
      directions (tests excluded):
      - every `border(` call in a paint body, qualified
        (`ops::border(`, `::lumen_static::ops::border(`) or bare through
        an import, must be preceded by `fill(` in the same `fn`, unless
        listed with a reason. The script reports the enclosing
        `impl … for` type. A per-file rule could not fire: `Label` and
        `Button` share `widgets/src/ops.rs`, so `Label`'s call would
        keep the file green (likewise `data.rs`'s 3 sites and
        `trim.rs`'s 4);
      - a kind whose paint calls `fill(` must name `BACKGROUND_FILL` in
        its seed mask.
      - Injection: remove `fill(` from `Button::paint`
        (`widgets/src/ops.rs:497`). The script names `Button`, with
        `Label` still filled.
      - Accept: the tree prints `sites N, filled N, allowed 0`, with N
        counted at landing (29 in 18 files at `553fa5e`: derive 1,
        `Repeat` 1, widgets 27).
    - **`.lss`.** `check_lss_setters.sh` prints its setter count, one
      higher. In the lss tests, `background-fill: 1` lowers to
      `Patch::background_fill(1)` and `background-fill: 1px` is refused
      with `Takes::Index`'s description.
      - Injection: delete the `PROPERTIES` line. The script names the
        setter.
      - Injection: declare it `Takes::Byte`. `background-fill: 1` is
        refused.
      - `lss_parity.rs`: the new sheet lowers to the same rules both
        ways. Injection: delete the `set` arm. `lss::load` panics on
        `background_fill`, by name.
    - **Disabled.** A disabled node's stops are at 40% alpha.
      - Injection: `ops::fill` ignores `st[i].disabled`.
    - **Live**, under both strategies
      (`scripts/drive/widgets/surface.py`, `CARD`). A static action,
      "the card's fill is a rounded radial":
      - the card's centre pixel and its edge pixels differ by ≥ 16
        levels;
      - its corner pixel is page colour;
      - both are printed, and M57's D6 scale-2 pass runs too.
      - Injection: remove `ops::fill` from `Card`'s paint. The pixels are
        equal.
      - Also: the existing Card actions stay at 1.00× damage, M57's D5 parity
        covers the entry live against headless, and M57's D4 test on the
        showcase judges the card's text on the fill in both palettes.
  - Firmware: `just firmware-delta <parent>` (default
    `firmware_interactive=1000`), with the headline rewritten.
  - Docs in the same commit:
    - handoff: a DECIDED entry, "a gradient in `Style` is a table index
      resolved before paint";
    - a `results-embedded.md` re-budget row, attributed;
    - `book/src/styling.md` gets `background-fill` and a tested snippet;
    - rustdoc on each new item;
    - the `tests/cursor.rs` comment.
- **T82.4 — M82a closes.** The headline of `results-paint-vocabulary.md`
  and a "What building found" section. The AGENT.md queue entry cites
  `just ci-verify <sha>` green (S5). No other change.

*M82b — paths and vector text.*

- **T82.5 — `Painter::path` and `Cmd::Path`, on the software canvas.**
  - What changes:
    - `PathEl`, `Affine` and `PathPaint`;
    - `path` in every implementor, `DisplayList::paths`, and
      `hash_into`/`hash_from`;
    - winit's `hot::Barred::new` (`lumen-static-winit/src/lib.rs:1709`)
      copies the session's list by named-field literal. If M83/T83.3
      has not landed, T82.5 adopts its design: `DisplayList` gains
      `#[derive(Clone)]` and `Barred` copies with `src.clone()`, so
      `paths` (and any later store) travels with the commands;
      otherwise `Barred` already clones and needs no change. Adding
      `paths: src.paths.clone()` to the literal or
      `..Default::default()` are not taken: the second compiles and
      leaves the store empty while `Cmd::Path` offsets index into it,
      and the loader has no `catch_unwind`;
    - `Recording::path`, with ink from `xf`, and `Scaled::path`;
    - `rasterize_path` in `software.rs`, and the `Canvas` arm;
    - `svg.rs` (`<path d=>`), `wire.rs` (the next free tag at landing,
      printed; expected 11, since tags 0–10 are taken and M83's
      `Shader`/`ShaderEnd` take the two after it; the elements after
      the text), and the `tests/svg.rs` arm;
    - `contrast.rs`: `Path` is an `Unknown("path")` ground, and its fill
      is measured as non-text ink at 3:1 in a new `Report::shapes`;
    - the GPU's `Cmd::Path` arm is **empty, with a pin test asserting
      so**, which T82.6 inverts;
    - the exhaustive matches listed in the Decisions, each with a
      `Path` arm: `clip_test.rs`'s `render_cmds`, `overlay.rs`'s `cmds`,
      `glass_cmd.rs`, `retain.rs`'s `ink`, and `parity.rs`'s `kind`,
      whose `Path` row names its pin test as the place it is compared
      (`Some("tests/parity.rs: path pin, until T82.6")`), with `KINDS`
      up by one (printed; 11 → 12 at `553fa5e`, more if M83's
      variants landed first). T82.6 draws `Path` in the frame and sets
      the row's third field to `None`;
    - `benches --bin vocabulary` gains path arms (the star filled, the
      circle stroked), each asserting its command count and element
      count > 0 before timing.
  - The showcase's `PaintBox` callback (`catalog.rs:135`) also fills a
    star and strokes an arc.
  - **Gates**
    (`crates/lumen-static-shell/tests/path_coverage.rs` and
    `path_damage.rs`):
    - **Coverage, on edge pixels only** (G12). The reference is 32×32
      supersampled non-zero coverage. The population is the pixels with
      0 < ref < 1, counted, printed and asserted at ≥ 200 per shape.
      - Star (lines): mean |err| ≤ 1/255 and max ≤ 8/255.
      - Circle (cubics): mean ≤ 3/255 and max ≤ 26/255, which is the
        0.1 px flattening.
      - Interior pixels are exact, and exterior pixels untouched, with
        both counts printed.
      - Injection: round the coverage to 0 or 1, the stencil-without-MSAA
        shape. The mean on edge pixels goes to about 64/255.
      - Accept: the unmodified rasteriser.
    - **Stroke join.** The pixel on a 90° join's outer bisector at
      `w / 4` from the vertex, outside both segment quads and inside
      the join's disc, has coverage above 0.5 (printed). Not the
      geometric outer corner: it lies `w / √2` from the vertex,
      outside a round join, and is uncovered on correct code.
      - Injection: omit the join polygons. The coverage prints ≈ 0.
    - **No phantom damage** (S2, T5). Path A, then a text run, then path
      B. A changes its element count. Retention damages A's ink only
      (area printed against A's ink area), and B's journal signature is
      unchanged.
      - Injection 1: `hash_into` folds `els.0`. B is reported changed.
      - Injection 2: `Recording::path` hashes only the count. A changed
        with the same count reports no damage.
    - **Transform.** A path rotated 30°, at scale 1 and at scale 2.
      Recording's ink equals the transformed control-point box, grown by
      `width / 2 + 1`; both are printed. The rotated tip pixel is
      inked.
      - Injection: `Scaled::path` forwards `xf` unscaled. At scale 2 the
        ink is half, and the gate is red.
    - **Deferring.** The stream test goes from 6 to 7 commands (the
      count at landing, printed).
      - Injection: `Deferring::path` has an empty body, and the test
        prints "7 in, 6 out" (A16, G7).
    - **CullTo**, if M85/T85.8 has landed. A path inside a
      `ScrollView` reaches the canvas through `CullTo` (commands in
      and out printed, equal).
      - Injection: `CullTo::path` has an empty body; the counts differ.
    - **Banded.** A path across a band boundary is byte-identical to a
      single band, asserting `Canvas::bands_for` ≥ 2 (`just threads`).
      - Injection: rows not offset by the band's `y0`.
    - **Wire.** The path round trip is equal.
      - Injection: the decoder drops the elements.
    - **Contrast.** A path at 2:1 is reported below 3.
      - Injection: the walk skips `Path`.
    - **Live**, under both strategies (`PAINTBOX` in
      `scripts/drive/widgets/rich.py`, a new static action):
      - the star's colour is counted inside the star's analytic area,
        at ≥ 90%, and 0 pixels of it fall outside the box;
      - M57's D6 scale-2 pass runs too.
      - Injection: `Canvas` skips `Cmd::Path`.
      - Also: M57's D5 parity covers the entry.
    - **Error bar** (`just hot-window-drive`'s failed-build step). With
      the PaintBox entry up, a region painted by a path keeps its
      `ui.regionHash` under the error bar, printed beside the hash
      before the failed build.
      - Injection: `Barred` rebuilds the literal without `paths`
        (`..Default::default()`) instead of `src.clone()`. The region's
        hash changes.
  - Firmware: `just firmware-delta <parent>` (K4's per-task cap on
    `firmware_interactive`) and K82b. Beyond 1,000 B, K82b fires.
  - Docs in the same commit:
    - handoff §12 item 8 marked **SUPERSEDED in part** with the reason
      above;
    - `book/src/gaps.md`'s "Arbitrary vector paths" paragraph rewritten;
    - `book/src/rasteriser.md`'s command list;
    - rustdoc, and a results row with `vocabulary`'s path arms.
- **T82.6 — GPU path masks.**
  - What changes:
    - `Atlas<GlyphId>` → `Atlas<MaskKey>`;
    - the `Cmd::Path` arm rasterises misses through `rasterize_path`,
      uploads R8, and draws `KIND_GLYPH` tiles;
    - the T82.5 pin inverts;
    - a `gpu_parity.sh` leg on `showcase --demo <PaintBox>`.
  - **Gates** (`lumen-static-wgpu/tests/parity.rs`, `just gpu`):
    - **Parity.** The same path scene on both backends: max channel
      delta ≤ 1 (blend rounding), with the pixel count printed. The
      scene includes a filled-and-stroked path, which rasterises 2
      masks (printed).
      - Injection: one key for both paints. The stroke draws in the
        fill's mask, and parity fails.
      - Injection: masks rasterised at the integer origin, ignoring the
        fractional offset.
      - Injection (the plan's): transform applied on software only.
    - **Cache.** An idle second frame rasterises 0 masks (counter
      printed), and a changed path rasterises exactly 1. The same path
      translated by an integer 1 px rasterises 0 (printed).
      - Injection: the key hashes absolute position. The 1 px case
        prints 1.
    - **Tiles.** A 2,000 × 100 px path draws with 0 atlas resets. The
      tile count is printed.
    - **Resume.** No new GPU resource exists: masks are atlas contents,
      and `just resume-drive`'s existing leg covers the reset. This is
      stated in the commit, with the resume leg's run cited.
    - **Live.** `just gpu-parity`'s new leg, where the worst channel
      delta is printed, and K82d's atlas share, printed.
  - Docs in the same commit: `docs/results-gpu.md` row, rustdoc on
    `MaskKey`.
- **T82.7 — `TextMeasure::outline` and the `vector-text` feature.**
  - What changes:
    - `PathSink`;
    - the provided `outline`, with rustdoc saying a wrapper must
      forward it;
    - `FontStack`/`FontMetrics::outline` behind `vector-text`, with the
      face bytes retained;
    - `ComplexText::outline`;
    - `lumen-static-winit` forwards `vector-text`;
    - the manifest comment records crates and size (K2);
    - a re-grep of `impl TextMeasure for`: any wrapper found gets a
      forwarding test (none expected; 28 leaves at `553fa5e`).
  - **How the gate file runs.** `vector_text.rs` is a `[[test]]` with
    `required-features = ["vector-text"]` in the shell's manifest, so
    the default selection does not build it and
    `check_test_counts.sh` never sees an empty binary. `just test`
    (and so `dev-shape`) gains
    `cargo test -p lumen-static-shell --features vector-text --test
    vector_text`, which T82.8's gates also ride. The clippy recipe
    gains `cargo clippy -p lumen-static-shell --features vector-text
    --all-targets -- -D warnings`, beside the `simd` and
    `threads,truetype` lines (`justfile:328`, `:338`), since the
    workspace line builds the shell without the feature.
  - **Gates**
    (`crates/lumen-static-shell/tests/vector_text.rs`, with
    `tests/fonts/DejaVuSans.ttf`):
    - **Outline.** `outline("AVWay", 32)` emits a count of elements that
      is printed and asserted above 0, which is the setup. Its bounding
      box equals the union of fontdue's glyph boxes within 1 px, both
      printed. The glyph ids from `place_run` equal ttf-parser's cmap
      ids, with the pairs printed.
      - Injection: pens ignored, so every glyph sits at x = 0. The
        width collapses.
    - **Pens.** Each glyph's outline xmin equals its pen (as
      `extent_by` computes it) plus ttf-parser's bbox xmin, within
      1/64 px, every pair printed; a space advances the next pen.
      - Injection: pens taken from `Placed.x`. The bearing is applied
        twice and the gate names the glyph.
      - Same gate against `ComplexText`. Injection: delete its override;
        it answers `false`.
    - **Symbols** (`scripts/release_window_shape.sh`, extended).
      `examples/minimal` in release links 0 outline symbols by default,
      and more than 0 with `lumen-static-winit/vector-text`, printed
      beside `.text`.
      - Injection: make `vector-text` a default feature.
    - **Crates.** `cargo tree` of `lumen-static-winit` with and without
      the feature: 0 new crates, printed. The stripped showcase's size
      delta is printed against 3 MB (B9).
    - **Memory.** The retained face bytes and the RSS delta are printed
      in the results doc.
    - Firmware: `just firmware-delta <parent>` (K4's default
      `firmware_interactive=1000`), for `BakedFace`'s slot. It is
      measured under K4 and reported, not accepted growth: it is in
      neither D3's nor D6's list, so the owner is told the number.
  - Docs in the same commit: rustdoc, the manifest comment, a results
    row.
- **T82.8 — `VectorText`.**
  - What changes:
    - the widget, in `lumen-static-widgets` (`alloc`), with `observe`;
    - a catalogue entry at 0°, 30° and 90°;
    - `VECTORTEXT` in `scripts/drive/widgets/rich.py`;
    - `roles.py` `"VectorText": "Label"`;
    - a `ROWS` entry in `crates/lumen-static-widgets/tests/a11y_roles.rs`
      (`:57`) and the catalogue field it reads: `every_node_impl_has_a_row`
      (`a11y_roles.rs:165-198`) scans `src/*.rs` for ` Node for ` and
      fails on an impl with no row;
    - `benches --bin vocabulary` gains the uncached 24-glyph, 32 px
      `VectorText` arm K82e reads, asserting its element count > 0
      before timing;
    - its line removed from `scripts/doc-identifiers.allow`.
  - **Gates**
    (`vector_text.rs` unless named):
    - **Rotated.** The ink box's w/h at 90° is the inverse of 0°'s
      (both printed). The ink stays inside the node's box. Setup:
      element count above 0, since otherwise the fallback text would
      pass vacuously.
      - Injection: paint ignores the angle.
      - Injection: the box is sized from the unrotated run, so the ink
        leaves the box.
    - **Memo.** Re-layout with nothing changed calls `outline` 0 times.
      A text change calls it once. Bumping the counting measurer's
      `generation()` with the text unchanged calls it once. All three
      are printed.
      - Injection: the key omits `generation()`. The generation case
        prints 0, a stale outline served (M45/T45.2's shape).
    - **Fallback.** With `BlockMetrics`: exactly 1 `Cmd::Text` and 0
      `Path`.
      - Injection: remove the fallback. Nothing is painted.
    - **Speaks.** `Role::Label` with the string as its value, and
      `~=` finds it. `check_observe.sh` and `check_drive_coverage.sh`
      print their counts.
      - Injection: remove `observe`. `check_observe.sh` names
        `VectorText`.
      - Injection: delete the `a11y_roles.rs` row. The test names
        `VectorText`.
    - **Live**, under both strategies plus M57's D6 scale-2 pass. The
      static action "drawn rotated": the 90° instance's ink box, read
      from the screenshot, has h ≥ 2·w, printed. M57's D5 parity covers the
      entry, and M57's D4 test on the showcase judges its ink at 3:1 in both
      palettes.
      - Injection: paint ignores the angle.
  - Firmware: none, since the widget is `alloc`-gated. The delta is
    printed at 0.
  - Docs in the same commit:
    - `book/src/gaps.md` and `book/src/text.md` (a tested snippet);
    - handoff: vectorial text DECIDED, with a pointer to this plan;
    - a results row with `vocabulary`'s VectorText arm.
- **T82.9 — M82b closes.** The results headline, "What building found",
  and the queue entry citing `just ci-verify <sha>`.

**Kill rules** (M82's own; the plan-wide K1–K3 still apply).

- **K82a — the gradient does not fit.** `glass_cmd.rs`'s
  `GradientPayload` mirror, printed beside 56 B. If it reads above 56
  on the chosen layout, `corner` leaves the command (payload 52) and
  rounded gradients are deferred. **Reopen** when a `Cmd` re-budget is
  accepted by the owner. It cannot fire as designed: 54 B, padded to
  56.
- **K82b — firmware headroom.** Measured by `just firmware-delta` and
  `check_text.sh`.
  - It fires when any M82 task leaves `firmware_interactive` with less
    than **1,024 B** under the 80 KB ceiling (81,920 B, D5). That is
    about one input task's worth: M55/T55.5 cost +880 B. It also fires
    when any binary exceeds the ceiling, and when one task's
    `firmware_interactive` delta exceeds K4's 1,000 B (T82.5's bound).
    The plans' stated bounds sum to 9,824 B
    (`docs/plan-iced-roadmap.md` §2.1 table) against 19,998 B, with
    10,174 B to spare, so the
    1,024 B floor is a check that is not expected to fire; the
    per-task cap is the half that can. Under the old 64 KB ceiling M82 was the first
    expected to trip; the owner's raise (2026-09-30) answered that.
    The per-task cap is the owner's to decide, with the symbol diff.
  - Then it goes to the owner with the symbol diff (K4), since D3 and
    D6 accepted this growth. If the owner moves it, the remedy is the
    one that compiles under workspace unification, not cfg'd
    *required* items: a required method behind a core feature breaks
    `Checksum` (`fixtures/embedded/src/lib.rs:247`), which builds
    `lumen-static` with `default-features = false`
    (`fixtures/embedded/Cargo.toml:18`) while the workspace unifies
    `alloc` on (`std = ["alloc"]`), and it cannot test the core's
    feature (`docs/plan-iced-roadmap.md` §2.2).
    - `Painter::path` (and `fills`, if the fill branch is what
      crossed) becomes **provided** under `cfg(feature = "alloc")`,
      with a documented leaf default (`path` draws nothing, `fills`
      answers `&[]`). The three wrappers (`Scaled`, `Deferring`,
      `Recording`), all in `lumen-static`, override it under the same
      `cfg`, as do any added since (M85's culling painter, if landed).
    - `PathEl`, `Cmd::Path` and `ops::fill` carry
      `cfg(feature = "alloc")` in the core. Every crate outside the
      core that matches `Cmd` exhaustively (shell, wgpu, benches and
      their tests) depends on `lumen-static` with `std`, so `alloc` is
      on wherever they compile and their arms need no cfg; the
      fixtures that match `Cmd` do so with `if let`. Were that ever
      false, the arms take M83's `links`/`DEP_LUMEN_STATIC_*` cfg.
    - Gate: two builds, `fixtures/embedded` alone (alloc off, 0 path
      symbols by `nm`, printed, bound 0) and the workspace (which
      compiles `Checksum` with the provided default).
  - Recorded with the number either way.
- **K82c — unused features must not cost the frame.** K3's arms
  (`docs/plan-iced-roadmap.md` §4): `benches-competitive/run.sh -p
  lumen-static-frame`'s idle and paint rows, `--bin textarea`'s
  `keystroke + layout` row and `--bin cascade`'s `cascade_deep` row,
  eight interleaved runs under `just bench-lock`, on a tree with no
  fill and no path.
  - It fires when any row leaves its spread.
  - Step 1: `ops::fill`'s body goes out of line (`#[cold]`).
  - Step 2: if a row is still outside, `background_fill` is honoured
    only by derive containers and `Card`. The other kinds drop the bit,
    and W1010 then says so.
- **K82d — path masks crowd out glyphs.** On the showcase's first GPU
  frame with the PaintBox and VectorText entries, mask texels are
  measured as a share of the glyph atlas's start size (printed). Above
  25%, paths get their own R8 atlas and binding. Decided on that
  number, not in advance.
- **K82e — software vector text is too slow.** `benches --bin
  vocabulary`, a 24-glyph `VectorText` at 32 px, uncached, under
  `just bench-lock`. Above **250 µs a frame**, the software canvas
  gets the GPU's `MaskKey` cache.
- **K2 for `vector-text`.** 0 new crates, and the stripped showcase's
  delta against 3 MB (B9). Above that, the feature stays default-off,
  as it is designed to, and the size is recorded.

**What building found (M82).**

- T82.1: **the payload mirror had to mirror T82.2's variant, not
  today's.** T82.0's `GradientPayload` mirrored the variant as it was
  (`rect`, `stops`, `angle`: 56 B). T82.1's byte offsets take that
  variant to 44 B, so "keep it at 56" could not hold, and the planned
  injection (`at: [u16; 4]`) reads 48 on it and stays green: the
  "goes to 60" arithmetic is T82.2's layout. So since T82.1 the mirror
  is the layout Decisions commits to (`rect`, `stops`, a `u8` kind,
  `[f32; 3]`, a corner) over the real `Stops`: 56 B green, 60 B under
  the injection, and today's variant (44) printed beside it. T82.2
  replaces the `u8` with `GradKind`.
- T82.1: offsets are read through a new `Stops::offset(k)`, so the
  conversion is written once; the plan's "`sample` converts" is that
  call. The injection (no `/ 255.0`) is in `offset`, which `sample`,
  `svg.rs` and wgpu's `grad_of` all read.
- T82.2: **the GPU had only ever drawn the first gradient in a frame.**
  `shader.wgsl`'s `Grad` ended in `_pad: vec3<u32>`, which aligns to 16,
  so the WGSL struct was 112 B against the Rust `Grad`'s 96 and every
  index past 0 read a zero length and drew nothing. No parity frame held
  two gradients until this task's GPU gate, which draws the radial and
  the rounded gradient in one list (the rounded one second, on purpose).
  Fixed here as three `u32`s; with the old pad restored the gate's GPU
  rounded gradient is the page, 4,708 px over 8.
- T82.2: **the K82a mirror's injection cannot be seen by `Cmd`'s own
  size.** With `GradKind` as `repr(u32)` the payload reads 60 B and the
  mirror fails, but `size_of::<Cmd>()` still reads 60: rustc puts the
  tag into the slack, the same effect Decisions records for `[u16; 4]`.
  The mirror is now the variant's own fields (bound by a match on
  `Cmd::Gradient`) over the real `Stops` and `GradKind`, not a layout
  ahead of it.
- T82.2: **`GradAxis`, one parameter formula, behind `std`.** The
  decisions put `t` in four places (software, shader upload, SVG,
  contrast); a struct resolved once per command and read by all of them
  keeps them from drifting, and the GPU uploads its `uv()`. It needs
  `cos`, `sin` and an exact square root, which `core` lacks and
  `crate::num` does not supply, and no firmware painter interpolates, so
  it is `cfg(feature = "std")`.
- T82.2: **the contrast check samples a run's ends clamped into the
  gradient's rect**, and a translucent sample leaves the run unmeasured
  as `"translucent gradient"`. The centre walk found the gradient; the
  ground past its edge is some other fill the walk did not reach, so
  sampling the gradient there would measure the wrong ground. The
  three-point sampling applies only when the centre's ground is a
  gradient; an opaque fill is still measured at the centre alone, so no
  existing contrast gate moved.
- T82.2: **T82.1's wire test asserted `VERSION == 3`**; it now asserts
  `VERSION > 2` (a `const` assert) and keeps its version-2 literal, and
  the literal pin moved to T82.2's test (`== 4`, a version-3 literal from
  the encoder at `dfb72c5`, regenerated there rather than edited). That
  test also refuses an unknown kind byte.
- T82.2: **a radial is written to SVG in user space.** In
  object-bounding-box units SVG scales `r` by the box's normalised
  diagonal, which draws an ellipse on a non-square rect where both
  rasterisers draw a circle; `cx`, `cy` and `r` come from `GradAxis` in
  px.
- T82.2: the software corner multiplies every pixel by `sd_round_box`'s
  coverage, not only the corner squares', which is the +8–13% the
  `rounded-radial` arm prints; at `corner == 0` there is no coverage term,
  and `gradient3.ppm` stays byte-identical.

- T82.3: **the journal records from `TreeMut`, not from the dispatch
  view the plan named.** `session.rs`'s `with_sheet` call builds the
  `TreeRef` that answers RPCs; `Agent::frame_done` records the journal from
  `TreeMut::as_ref()`, which carried no fills, so `TreeRef::with_fills`
  alone would have left a leaf's fill out of every signature. `TreeMut`
  gained `fills` and `with_fills` too, `as_ref` forwards it, and the
  session's `agent_frame_done` sets it from the frame list
  (`h.list.fills`, since `h.app` is borrowed mutably there); the dispatch
  view gets `with_fills` as planned. The journal gate's second injection
  is therefore `TreeMut::as_ref` handing `&[]` (both nodes 0.00×).
- T82.3: **`Host::show` replaced the frame list wholesale.** The panic
  screen (M36) assigns a new `DisplayList`, which would have dropped the
  fill table, so every frame after a retry painted no fill. It keeps the
  table now. `hot::Barred::new` (winit, `reload`) builds a list by literal
  and copies `fills` too, so it compiles; it never paints a fill.
- T82.3: **`check_fill_paint.sh`'s second direction is per file, and the
  per-kind direction is measured.** A seed is spelled half a dozen ways
  (`Self::KIND` in `SEEDS`, a `*Table` const, `"appbar"`, a part's
  `"picker-trigger"`), so the script asks only that a file whose paint
  calls `fill(` names `BACKGROUND_FILL`. The per-kind claim is
  `lumen-static-widgets/tests/style_reads.rs`, which renders every kind
  with and without `background_fill(1)` over a one-entry table: it found
  **16** kinds reading the bit before their masks named it. `Image` is
  listed in the script: its layout never calls `record_painted`, so its
  `bg` (and now its fill) branch is dead — pre-existing, not fixed here,
  and its mask matches the measurement (it reads neither).
- T82.3: **where the call goes.** "Before the flat `bg`" at each site,
  which is the line before the node's `rounded_rect`; where that is
  guarded by `bg.3 > 0` (`Image`, the charts, `RichText`, `PaintBox`) the
  fill goes above the guard, so a fill under a transparent background
  still paints; `DataGrid`'s border is a hundred lines below its ground,
  and the fill is before the ground. The `Picker` paints its trigger
  part's fill at `i + TRIGGER`, as it does its border.
- T82.3: **the D4 contrast test judged text on the stage behind a fill.**
  `every_demo_text_reads_in_both_palettes` composited flat fills only, so
  the Card's title on its gradient was measured against the stage card,
  and a fill whose edge stop failed AA (1.63:1) passed at 15.93:1. It now
  samples a gradient ground at a run's centre and both ends (T82.2's rule)
  and asserts it met the Card's runs.
- T82.3: the showcase's Card has no drop shadow, so its rounded corner
  shows the stage the live check compares it with; `Stops::two` became a
  `const fn` so a table entry can use it. The palette gate replaces the
  `Button`'s palette-role colours with literals: they follow the palette
  too, and the swap damaged the button (952 px² over the bound) until
  only the fill could move.
- T82.3: the plan's figures held exactly: bit **19**, 29 sites in 18
  files, `Style` 84 (81 used), `NodeState` 84/84.
- T82.4 (M82a's close): **`ops::fill`'s body and its call sites are in
  neither D3's nor D6's list.** T82.3's symbol diff put `ops::fill` at
  +312 B and each call site at about +32–40; D6 accepts the `fills`
  getter, which is 10 B of it, and Decisions ("Firmware") had said the
  branch would be measured and reported. It is recorded as part of the
  accepted `fills` feature — the getter is useless without the code that
  reads it — by the coordinator, unless the owner says otherwise
  (`docs/results-paint-vocabulary.md`, "M82a's kill rules, at its
  close"). Every M82a kill rule held; K82a held with no byte to spare
  (56 of 56), as designed.

- T82.5: **the circle's coverage bounds were not the 0.1 px flattening.**
  The plan wrote mean ≤ 3/255 and max ≤ 26/255 and called them that
  tolerance's consequence; a chord at sagitta `s` sits on average `2s/3`
  inside its arc, so 0.1 px gives a mean near 17/255, and a pixel the chord
  crosses diagonally reaches `√2 · 0.1`, 36/255. 3/255 is a tolerance of
  about 0.02 px. The tolerance is the decision (it is what T82.6's masks
  inherit), so the gate's bounds are now computed from
  `FLATTEN_TOLERANCE` in the test: measured 10.3 and 26.5 against 17.0 and
  36.1. The rounding injection reads about 50/255, not "about 64".
- T82.5: **the star is a ten-vertex outline, not a pentagram.** Signed-area
  accumulation is exact where a pixel's winding is 0 or 1; where it holds
  winding 2 the clamp counts that area twice. A pentagram's inner vertices
  are exactly such pixels (about 0.3 over at a vertex), so "max ≤ 8/255"
  on lines holds only for a non-self-intersecting star, which is what a UI
  draws. The stroke has the same over-count on the edge pixels where its
  quads and discs overlap; the join gate asks a pixel inside the disc.
- T82.5: **a banded frame differed from a whole one** on the first build:
  the mask subtracted its origin from every y, and `y - y0` rounds
  differently for each band's `y0`, so 8 of `tests/banded.rs`' 10
  checksums moved under `threads`. y stays absolute now (only x is moved
  into the mask's columns), and the canvas takes the mask's x origin from
  the ink on the surface, never from the damage, so a partial redraw sums
  each row in the same order as a full one.
- T82.5: **the wire elements are written only when the list has any**, so
  "tag 11 is appended and bumps nothing" stays true: a list with no path
  is byte-for-byte the version-4 encoding, and a path's range is checked
  against the decoded store.
- T82.5: **`Checksum::path` linked 956 B** at first, for a method no
  firmware widget calls: every field folded through inlined per-byte
  `fold`s (a ten-value `f32s` was 964 B, unrolled). One call to the
  out-of-line `r`, shaped like `shadow`'s, is 66 B
  (`docs/results-embedded.md` M82/T82.5).
- T82.5: a stroke's `width` is not transformed by `xf` (only `Scaled`
  scales it), so a path built in a unit frame and placed by its transform
  keeps the stroke it asked for; the showcase's star and arc are built that
  way. `contrast.rs` measures a path's fill, or its stroke when the fill is
  transparent, and keeps `Report::unmeasured_shapes` beside `shapes`.
- T82.5: `fixtures/hot-window` gained `#star` (a paint box filling a path,
  first in the column) and grew from 160 to 400 px tall: the error bar is
  up to ten lines, 216 px at scale 1, and at 160 px it covered every region
  the error-bar gate could compare.
- T82.5: the `CullTo` case is not built: M85/T85.8 has not landed.
- T82.5: **K82c fired as written, on the fast side.** On a quiet machine
  the work tree's `+render (damage)` median sat 0.05 µs under the parent's
  eight-run minimum, and two of four runs taken beside a concurrent
  `just ci` left the spread the same way (`+render`, `+paint`); one left it
  slower on two arms T82.5 does not touch (`textarea`, `cascade_deep`).
  K82c says "unused features must not cost the frame" and "fires when any
  row leaves its spread", and K3 reads two-sided; its responses answer a
  cost. The coordinator ruled it not a firing (2026-10-05): a faster
  median is not a cost, and the slower rows were on arms T82.5 does not
  run (`docs/results-paint-vocabulary.md` T82.5).

- T82.6: **the parity bound is one blend's rounding per paint, not per
  frame.** The masks are the software canvas's bytes, but where a
  translucent fill and a stroke over it both cover a pixel there are two
  blends, each free to round by one: llvmpipe reads 2 on 31 px of the
  circle's stroke edge (the RTX 4070 reads 1). The gate holds each paint
  alone over the opaque page to ≤ 1 (all four read 1 on both adapters)
  and the whole scene to ≤ 2; both injections are red on the per-paint
  half (230 and up to 230).
- T82.6: **the plan's third parity injection ("transform applied on
  software only") has no site.** Since T82.5 the transform is applied in
  `DisplayList::path`, before either backend, and a renderer never sees an
  `Affine`. The rotated bar is in the parity scene, so a GPU that drew
  its mask from anything but the list's elements would fail the per-paint
  bound; the two injections that break the GPU's own mechanism (one key
  for both paints, the mask without its fractional offset) are run
  instead.
- T82.6: **the key carries a tile index**, `MaskKey::Path { hash, paint,
  frac, tile }`: a `PATH_TILE` tile is its own atlas entry, and the
  plan's key had no field to tell two tiles of one path apart. Tiles are
  rasterised one at a time (`rasterize_path` with the tile's origin), so a
  path wider than the window costs one tile's buffer, not the whole mask.
- T82.6: **a tile's hard clip is the clipped ink snapped out to whole
  pixels**, not the fractional ink: the shader clips at pixel centres,
  and a pixel the ink's edge only grazes holds coverage the software
  canvas blends (it writes from the floor to the ceiling of the clipped
  ink).
- T82.6: **K82d is printed by the window**, not by an agent method: the
  GPU backend logs one line on its first frame holding a path mask, and
  `just gpu-parity`'s PaintBox leg requires it (1.49% on the PaintBox
  entry). `AtlasStats` carries `path_masks` and `path_texels` for the
  headless gates.

- T82.7: **`PathSink` announces each glyph.** The plan named the trait and
  not its shape; it takes `el(PathEl)` and a provided `glyph(index)`
  called before each glyph's elements, which is what lets the pens gate
  hold each glyph's outline to its own pen and the outline gate check the
  ids it drew against the cmap's. A wrapping sink forwards it
  (`FontMetrics`'s scale-dividing `LogicalPath` does).
- T82.7: **the outline carries the cap shift.** `place_run_into` moves
  every glyph by the run's cap shift, so an outline placed on the bare
  baseline would sit up to a pixel off the glyphs the same run paints; it
  sits where they do, and the outline gate's 1 px comparison is against
  `place_run`'s boxes, shift included. At scale 2 the shift is rounded at
  physical px, so the logical outline's y moves 0.5 px from scale 1's.
- T82.7: **29 implementors, not 28, and no production wrapper.** The
  re-grep at `8db6ba5` finds 25 test stand-ins and the four real
  measurers. Several stand-ins delegate `measure` to an inner measurer to
  count calls (`measurer_generation.rs`, `text_memo.rs`,
  `text_input_b15.rs`, `measure_scale_widgets.rs`): test fixtures that
  never ask for an outline, so no forwarding test; T82.8's counting
  measurer, which does, forwards it.
- T82.7: **`ComplexText`'s gate runs in `vector_text.rs` under `shaping`
  too**, a second `just` line (`--features vector-text,shaping`): the
  plan puts the gate beside `FontMetrics`'s, and the shell's tests reach
  `lumen-static-text` only through `shaping`. **The memory gate is
  in `font_heap.rs`**, beside the counting allocator it needs; its bound
  is one copy of each file + 64 KiB, and its RSS is printed in-process.
- T82.7: under `shaping` and `vector-text` together a `Complex` stack holds
  a face's bytes twice (the shaper's interned copy and this feature's);
  recorded in `docs/results-paint-vocabulary.md` T82.7, not deduplicated.
- T82.8: **the D4 walk, taught to read path ink, found T82.5's star at
  2.67:1** in the dark palette (`#c2410c` on the dark PaintBox ground; 5.18
  in light). Nothing had judged path ink before. The star is `#e14d0a`
  now (3.99 / 3.47), in the catalogue and `rich.py`'s `STAR_INK` together,
  and `scripts/widget_drive.sh` ran PaintBox beside VectorText to hold it.
- T82.8: **`VectorText`'s text is a plain field keyed by its hash**, as
  `Button`'s label is (`FoldedText`'s rule), not a `Dirty` the way
  `Label`'s is: a `#[bind]` and a reload both write it directly. It
  overrides `Node::probe` under `observe` so the agent's audit, whose
  measurer has another generation, does not throw the frame's outline away
  twice a frame. Its trigonometry is its own (`sin_cos_deg`), exact at
  every quarter turn: the `alloc` tier's `core` has none.
- T82.8: **"`~=` finds it" is a second static action of the live drive**
  ("it speaks its text, and ~= finds it"), beside "drawn rotated": the
  selector grammar is the agent's, and the drive is where it is asked.
- T82.8: the showcase's `every_entry_paints_a_demo` exempts `VectorText`
  as it does `Label`, `RichText` and `Selectable`: it pumps `BlockMetrics`,
  so the demo paints its level fallback there. `VECTORTEXT` stays in
  `scripts/doc-identifiers.allow` (a Python name the scan does not read,
  as `PAINTBOX` does); `VectorText` left it. `just gpu-parity`'s path leg
  loops over PaintBox and VectorText, so K82d reads both entries' first
  frames. K82e's arm needs `benches --features vector-text`, so K3's arms
  build without the feature as before.
- T82.9 (M82b's close): **every M82b kill rule held, one by ruling.**
  K82c read literally fired at T82.5, on the fast side; the coordinator
  ruled it not a firing (2026-10-05): a faster median is not a cost, and
  every response K82c names answers one, so it stays two-sided as a
  measurement and is read one-sided as a stop (T82.5's entry above).
  **T82.6 deviated from the plan's parity bound**: ≤ 1 is held per paint
  over the page, and the whole scene is held to ≤ 2, one rounding per
  blend, because a translucent fill and the stroke over it are two
  blends (T82.6's entry above). **The owner is told the `outline` slot**:
  T82.7's provided `TextMeasure::outline` in `BakedFace`'s vtable is
  +8 B `.text` on all four firmware binaries, in neither D3's nor D6's
  list, within K4, reported and not accepted growth. The numbers are
  `docs/results-paint-vocabulary.md`, "M82b's kill rules, at its close".

**What the review found (M82).**

- **C1** → adopted, changed. `Stops` offsets become u8 (36 → 21 B), so
  `kind`, `geom` and `corner` fit in 56 B and `Cmd` stays 60. K82a
  prints the mirror, and its fallback (drop `corner`, 52 B) fits.
- **C2** → adopted. M82 needs exactly 1 B of `Style` and 1 B of
  `NodeState`. The exact sizes are asserted at commit; expected 84 and
  84 (M80 left `NodeState` one byte), which is 16.4 KB per 100 nodes.
- **C3** → adopted. `NodeState.fill` is stamped by `record_painted`,
  and paint reaches the table through `Painter::fills` and `ops::fill`.
- **C32** → adopted. ttf-parser comes through fontdue with the same two
  features: 0 new crates.
- **G11** → adopted. The injection is the `Patch` setter dropping its
  bit. The W1010 gate runs both directions, and the size assertion is
  exact.
- **G12** → adopted. Coverage is measured on edge pixels only, with
  their count printed. The GPU is byte-parity with software, because it
  uses the same rasteriser.
- **G27** → adopted. T82.0 holds only gates that compile today: pins,
  green, asserting today's gap. `firmware_delta.sh` is M80/T80.0's.
  New-API gates land in their own task.
- **G28** → adopted. Every gate names an injection that breaks the
  mechanism.
- **G29** → adopted. There are live static actions for Card, PaintBox
  and VectorText under both strategies, and T82.1–T82.2's lack of a live
  user is stated.
- **G32** → adopted. `firmware_delta.sh` (M80/T80.0) is an exact diff
  against the parent build, with a bound; M82 cites it, invoked
  positionally as `just firmware-delta <parent> [BIN=B ...]` (second
  review B5/B16).
- **B1** → adopted, as C1. The arithmetic was measured with rustc
  1.94.0. The u16 alternative was rejected because it fits only by the
  tag sitting in slack.
- **B2** → adopted. `fixtures/embedded/src/lib.rs`'s `STYLE`,
  `NODE_STATE` and `PER_NODE_RAM` are named as sites.
- **B4** → changed. It is not a Painter method that takes the index,
  which would carry the index into retention. It is a required
  `fills()` getter plus `ops::fill`, which paints inline content at
  every `border` call site (29 at `553fa5e`, counted per site; the
  second review's C2/T3/R7 corrected the first draft's 28). It is gated
  on a container, a `Repeat` and a leaf.
- **B6** → adopted. Per-task firmware deltas, `Checksum` in every list,
  the headroom stated (19,998 B under 80 KB since D5, 2026-09-30;
  3,614 B under 64 KB when reviewed), and K82b.
- **B7** → adopted. The firmware clause for `vector-text` is dropped,
  replaced by a two-way symbol count in `release_window_shape.sh`.
- **B8** → adopted for M82. Exact per-struct sizes are asserted, with
  `Cmd == 60` and `Event` untouched. RAM per 100 nodes is judged only
  for `Style` and `NodeState`.
- **A5** → adopted. Fills are host-side through `Program::host()` and
  re-asked on every swap. M82 adds no window-side setting.
- **A8** → adopted. Paths follow `Cmd::Text`: the offset is never
  hashed, the content is, and there is a shifted-index gate.
- **A9** → adopted, as C1.
- **A10** → adopted, as C2 and C3.
- **A16** → changed. The transform is a per-path argument, so there is
  no push/pop and no balancing in `Deferring`. The required-method
  injections are an empty body or `&[]`. `Checksum`, `Sink` and six
  test painters are listed.
- **A24** → adopted. `VectorText` publishes `Label` with its string, has
  a drive inventory and a role row, and `contrast.rs` measures path ink
  at 3:1.
- **A25** → rejected. `FontStack` never crosses the seam: the library
  builds its own at `session.rs:387` and the loader its own at
  `winit/src/lib.rs:677`. What does cross is source-hashed (M49).
  **Reopen** if a `FontStack` ever crosses.
- **T12** → adopted, as C2.
- **T13** → adopted, as C1. The inscribed-ellipse idea was not needed.
- **T14** → adopted. `ops::fill` goes at every `border` site (29, not
  28: second review C2), `check_fill_paint.sh` checks them per call
  site, and a palette gate covers re-resolution.
- **T15** → adopted. Corner coverage is new code on both backends, with
  a GPU-only injection.
- **T16** → changed. No stencil: the GPU draws masks from the software
  rasteriser through the glyph atlas, so there is no new attachment
  lifecycle.
- **T17** → changed. The transform is per path. The `Scaled`
  conjugation is gated, hit testing is stated as axis-aligned, and
  glyph runs are never transformed.
- **T18** → adopted. The `TextMeasure::outline` seam, byte retention
  under the feature, and the crate `VectorText` lives in are all named.
- **T21** → adopted, per D3 (paths, gradient) and D6 (`fills`,
  2026-10-02). Measured growth per task, and the headline re-budgeted.
- **M1** → adopted, as C2.
- **M2** → adopted, as B6.
- **M3** → adopted. There is no docs task. T82.4 and T82.9 carry only
  the results summary and the queue entry.
- **M4** → adopted. `VectorText`'s obligations are in T82.8. M82 adds no
  other widget.
- **M10** → adopted. M82 does not depend on M81. M81 builds no glyph
  command; text under a transform is `VectorText`.
- **M24** → adopted. `background-fill` is its own declaration, taking
  a new unitless `Takes::Index` (second review T11/M19: `Takes::Byte`
  is px). M61 is merged at `553fa5e`, so it is unconditional.
- **M26** → adopted. Split into M82a and M82b.
- **X2** → noted for M85. M82's two users keep their ink inside their
  box. A custom widget that draws a path outside its rect is M85's to
  cull by ink.
- **Drafter's own finding** → handoff §12 item 8's DECIDED "vector
  half not in `Cmd`" is superseded in part by T82.5, with its reason,
  in that commit (the first draft cited §8, which is "Configuration";
  second review C13/M20).
- **Cross-milestone findings applied here:**
  - C24: Painter wrappers are separated from `Cmd` consumers.
  - G7: `overlay.rs`'s stream test is extended per command.
  - B9: 3 MB, not 4.12 MiB.
  - T5: see A8.

**What the second review found (M82).** Ids are the second review's.

- **C2, T3, R7** → changed: the count is 29 production sites in 18
  files (derive 1, `Repeat` 1, widgets 27, two of them `Label`'s and
  `Button`'s bare `border(` in `widgets/src/ops.rs`; the test
  `border_pixels.rs` excluded). `check_fill_paint.sh` is per call
  site, qualified or bare, reports the enclosing type, and prints N
  at landing; the `Button` injection is kept and now fires.
- **C3** → adopted: the journal's container signature folds `st.fill`
  and the resolved entry, `paints[i]` counts a fill, the agent gets
  fills through `TreeRef::with_fills`; a journal gate at 1.00× with two
  injections.
- **T10** → adopted for the journal half (the journal list is the
  agent's, reached through `TreeRef`, not stored by `Host`); the
  shadow-oracle half rejected: `sign_leaves` paints into the same
  scratch list (shell `lib.rs:514`, `:578`), so there is no second list.
- **C7** → adopted: T82.2 bumps `VERSION` again (3 → 4) with its own
  literal-fixture gate; M84's expectation becomes 4 → 5 (request).
- **C11** → adopted: 24 test stand-ins and four real measurers, named
  with their intended answers.
- **C13, M20** → adopted: "handoff §12 item 8" at all four places.
- **C19** → adopted: the exhaustive `Cmd` matches are listed, and
  `parity.rs`'s `Path` row names its pin until T82.6, with `KINDS` +1.
- **G5** → adopted: a used-bytes mirror catches the second `u8`; a
  `u32` injection exercises the exact `size_of`.
- **G6** → adopted: T82.1 adds a gradient golden recorded at the
  parent; T82.3's "index 0" gate is a `Host` with a non-empty table and
  0 `Gradient`s.
- **G7, T15** → adopted: the refusal fixture is a committed literal at
  the previous version with no gradient, and `VERSION` is asserted
  against a literal.
- **G8** → adopted: the corner probe is (1, 1), coverage ≤ 1/255
  printed; (6, 6) at scale 2 kept.
- **G9** → adopted: an integer 1 px translate rasterises 0 masks.
- **G10** → adopted: a generation bump with the same text calls
  `outline` once.
- **G11, B3, A12, T7, R9** → changed: K82b goes to the owner first
  (D3/D6 accepted the growth); if moved, `path`/`fills` become
  provided under `cfg(feature = "alloc")` with wrapper overrides, and
  `Cmd::Path` is alloc-gated in the core only, since every outside
  matcher builds with `std`. Two-build gate. Recorded in the index
  §2.2 (request).
- **G33** → adopted: the join probe is on the outer bisector at `w / 4`.
- **B2** → adopted: T82.2 creates `benches/src/bin/vocabulary.rs`;
  T82.5 and T82.8 add arms, each asserting its setup.
- **A15** → adopted for `VectorText`: an `a11y_roles.rs` row and its
  injection in T82.8; S4 gains the obligation (request).
- **T11, M19** → adopted: a unitless `Takes::Index`, with `1` lowering
  and `1px` refused.
- **T12, R8** → adopted: `svg.rs`'s stop offset divides by 255, with a
  `tests/svg.rs` gate.
- **T16** → adopted: pens as `extent_by` computes them, not
  `Placed.x`; a per-glyph pen gate.
- **T17** → adopted: no wrapper is planned; T82.7 re-greps.
- **T22** → adopted: masks keyed by paint, one per non-transparent
  paint, and the parity scene fills and strokes one path.
- **T23** → adopted: `set_fills` compares and `mark_all`s; gated on a
  `Boundaries` host.
- **M10** → adopted for `vector_text.rs`: `required-features` plus a
  `just test` line.
- **D6** (owner, 2026-10-02) → applied: `fills` is accepted growth by
  D6, not D3; `outline`'s slot and `ops::fill`'s branch are in neither
  list and reported to the owner.
- **B6, C9/B14, M7, M3, X7, R6** (from another plan's editor) →
  adopted: K82b cites the §2.1 table's 9,824 B (consistency-3 item 4;
  it said 9,792 B), not "about 7,700 B",
  and the 1,024 B floor is a check not expected to fire; T82.3's
  `focus.rs` site is `:203-204` (both asserts), and it names the
  shell `lss.rs` `set` arm and an `lss_parity.rs` sheet; T82.7's
  `outline` slot is measured under K4 and reported, with a clippy
  line for `--features vector-text`; T82.8 names the `ROWS` entry
  and catalogue field.
- **G3/R5** (from another plan's editor) → adopted: the `Deferring` stream
  test reads 6 after M80/T80.1's `text_spaced`, so T82.5 takes it
  6 → 7 and its injection prints "7 in, 6 out".
- **B5/B16** (from another plan's editor) → adopted: every invocation is
  positional, `just firmware-delta <parent> [BIN=B ...]`; T82.1 uses
  `print`, T82.2 `firmware_interactive=400`, T82.3/T82.5/T82.7 the
  default 1,000 B. `fills` was already recorded under D6, not D3.
- **A13** (from another plan's editor) →
  adopted: T82.5 lists `hot::Barred::new` (`lib.rs:1709`) as a
  `DisplayList` consumer that must copy `paths`, gated on
  `just hot-window-drive`'s failed-build step by `ui.regionHash`,
  with `Barred` omitting `paths` as the injection.
- **D7, X5** → not applicable: M82 uses no clipboard and no
  `rustybuzz` advisory text.
- consistency-3 item 3 → T82.2's deferring stream test stays at 6
  commands (the count at landing, printed), not 5; T80.1 adds one.
- consistency-3 item 4 → K82b's sum is §2.1's 9,824 B with 10,174 B
  spare, not 9,792 B.
- consistency-3 item 11 → T82.5 adopts M83's design if T83.3 has not
  landed (`DisplayList` derives `Clone`, `Barred` uses `src.clone()`;
  injection: the literal without `paths`), else `Barred` already
  clones.
- consistency-3 item 19 → M57's gates read "M57's D4/D5/D6" (eight
  sites), not the owner labels.
- consistency-3 item 22 → `Takes::Index` lowers to the existing
  `Val::U8` (checked: `lower.rs:63`, macro arm `lss.rs:229`); no macro
  arm or `Val` variant, only the shell `set` pairing.
- consistency-3 item 23 → T82.3 and T82.5 gain a `CullTo` case (fills,
  path) if M85/T85.8 has landed, each with an empty-body injection.
- consistency-3 item 24 → T82.0 removes the results doc's
  `doc-paths.allow` entry and T82.2 the `bin vocabulary` line.
