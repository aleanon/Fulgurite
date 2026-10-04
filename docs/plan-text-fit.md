# M80 — text that fits: line height, ellipsis, read-only selection, IME placement

> **Written 2026-09-28 as a section of `docs/plan-iced-roadmap.md`;
> revised 2026-09-29 after that plan's seven-lens review, and moved to
> its own file. Not started.** The shared byte inventory, seams (S1,
> S2), test and doc discipline, and plan-wide kill rules K1–K4 are in
> `docs/plan-iced-roadmap.md` §2–§4 and bind this plan; this file does
> not restate them.

*Size L, split in two so each half closes on its own `just ci-verify`:
**M80a** (T80.0–T80.3, text flow: line height and overflow) and
**M80b** (T80.4–T80.5, selection and IME placement). Closes iced's
configurable line height, ellipsis, read-only text selection and IME
candidate placement. Depends on nothing; M61 has landed on this tree
(`553fa5e`, which merges `92a4033` with `main`), so every new `Patch`
setter adds its `.lss` declaration in its own commit. **M81 depends on
all of M80** (see "Cross-milestone" at the end). The first draft sized
this M; it touches `Painter`, `Cmd`, `Style`, `NodeState`, `MeasureKey`,
both renderers, the winit window, the agent and the reload wrapper,
which is not M.*

**Decisions.**

- **The byte inventory, first.** `Style` is 78 B of fields in 80
  (2 × `Length` 16, `Edges` 16, ten 4-byte fields 40, six 1-byte fields
  6: `style.rs`), so **2 bytes are spare**. `NodeState` is 82 in 84
  (`desc.rs`), **2 spare**. M80 takes **both of `Style`'s** and **one of
  `NodeState`'s**, and neither struct grows: `size_of::<Style>() == 80`,
  `size_of::<NodeState>() == 84`, 164 B/node, **16.0 KB per 100 nodes
  unchanged** (`tests/cursor.rs`, `tests/focus.rs`,
  `fixtures/embedded/src/lib.rs` `sizes`). **The next `Style` field in
  this plan (M82's `background_fill`) takes `Style` to 84 B**; M80
  leaves it no padding. `max_lines` is not built (below).
- **Line height is a ratio in sixteenths of the font size.**
  `Style.line_height: u8`, `0` = the measurer's own spacing (today's
  behaviour, so every default tree paints the same pixels). 16 = 1.0×,
  24 = 1.5×, max 255 = 15.9×. Sixteenths, not the first draft's eighths:
  iced's default 1.3 is 1.25 in eighths and 1.3125 in sixteenths. A
  ratio rather than a length because it is scale-invariant: `Scaled`
  forwards it untouched and the renderer multiplies by the *physical*
  size. Absolute line heights (iced's `LineHeight::Absolute`) are not
  built. `Patch::line_height(ratio: f32)` stores the ratio and the
  cascade rounds it to sixteenths; `.lss` `line-height: 1.5`
  (`Takes::Number`). Crate: `lumen-static`. Bytes: 1 of `Style`'s 2.
- **Its semantics are CSS's and iced's, not fontdue's.** A run of *n*
  lines measures exactly `n × pitch`, `pitch = line_height/16 × px`,
  **a single line included**. Glyphs sit at half-leading: line *k* moves
  down by `k × pitch + (pitch − nls_k)/2 − top_k`, where `nls_k` is that
  line's `max_new_line_size` and `top_k` its top as fontdue laid it out.
  **fontdue's `LayoutSettings.line_height` is not used**: it multiplies
  the face's ceil'd new-line size (≈1.36 em for the test faces, fontdue
  `layout.rs:430`) and applies only between lines (`layout.rs:476`,
  `height()` at `:395`), so passing a ratio through would paint 12/8 as
  about 2.04 em and leave single lines unscaled. The placer re-places
  lines itself from `layout.lines()`, allocation-free, with the cursor
  walk `shape_into` already uses.
- **A pitch below the face's spacing keeps its ink in the rect.**
  Half-leading is negative when `pitch < nls`: 16 (1.0×) and iced's
  1.3 (21/16) both sit under the test faces' ≈1.36 em, so line 0's
  ascenders would rise above the run's rect and the last line's
  descenders fall below it. Nothing clips a run (`software.rs`
  only culls against a reach padded by the font size), and
  `Recording` damages the run's rect (`retain.rs`, `text_weighted`),
  so a text change would leave the overflow stale. Rule: a run of *n*
  lines measures `n × pitch + max(0, nls − pitch)`, and the placer
  shifts every line down by `max(0, nls − pitch)/2`, so line 0's top
  and the last line's bottom land on the rect's edges. At or above
  the face's spacing (≥ 22/16 for the test faces) this is exactly
  `n × pitch`, CSS's and iced's figure; below it the box grows by the
  deficit rather than let ink leave it. `Label` measures with the
  measurer's `line_height(px)` as `nls`, rounded up as fontdue rounds
  `new_line_size`, so the box and the placer agree (both printed in
  T80.1's gate). `FontStack::extent_by`'s max-with-ink stays, and the
  gate prints the ink term so a face whose glyphs exceed `nls` shows.
- **Measurement needs no `TextMeasure` change.** The first draft's
  "line height keys `MeasureKey`" had no path into the measured box,
  because `measure`/`measure_weighted`/`shape` take no line height
  (`cx.rs`). But `shape` already reports every cluster's `line`
  (`Cluster::line`, honoured by `FontMetrics`, `BakedFace` and
  `ComplexText`), so a widget with `line_height != 0` shapes into a
  counting sink, `LineCount`, and takes `lines × pitch`. A measurer that
  reports no clusters (`BlockMetrics`, the test stand-ins) keeps its own
  height, which is the existing contract of `TextMeasure::shape` ("a
  caller that gets no clusters falls back to the run's size"). No
  signature changes across the 28 `TextMeasure` implementors.
  Crate: `lumen-static` (`LineCount` beside `NoSink` in `cx.rs`).
- **Paint gets it through a new required `Painter` method.**
  `Painter::text_spaced(rect, s, size, weight, line_height: u8, color)`
  is **required**. `text_weighted` becomes **provided** and forwards to
  `text_spaced(.., 0, ..)`, exactly as `text` already forwards to
  `text_weighted`. That is not the provided-method trap: the provided
  body calls a required method *on the same painter*, which a wrapper
  implements by forwarding, so nothing is dropped. The 52 existing
  `text_weighted` call sites do not change. `Cmd::Text` gains
  `line_height: u8` (payload 36 → 37 B; `Cmd` stays 60, asserted in
  `tests/glass_cmd.rs`). `Cmd::hash_into` and `Recording` fold it, so a
  line height change in a fixed-height box is damage. `wire.rs` encodes
  it and bumps `VERSION` by one from the current version (expected
  1 → 2; M82 and M84 bump it after). `svg.rs` names the field and ignores it
  (an SVG `<text>` is one line; recorded in its rustdoc). Every
  implementor is listed in T80.1. Crate: `lumen-static`, with
  `FontStack::place_run_into` (`lumen-static-shell`) taking the byte.
  **The wrappers lose their `text_weighted` overrides**: `Scaled`
  (`display.rs`, whose origin rounding, M57/A15, moves into
  `Scaled::text_spaced`), `Deferring` (`overlay.rs`) and `Recording`
  (`retain.rs`) implement `text_spaced` only, so no wrapper holds two
  bodies that must agree and every `Label` run, which goes through
  `text_spaced` after T80.2, is rounded by the one body.
  Firmware: one vtable slot on every painter plus `Checksum`'s body,
  **accepted growth (D6, owner, 2026-10-02)**: the required
  `text_spaced` here and M82's `fills` are accepted alongside paths
  and the widened gradient. D3 is the shader gate and says nothing
  about `text_spaced`; K4's 1,000 B per-task cap still applies.
- **Paint reads it from `NodeState`.** `NodeState.line_height: u8`,
  stamped by `record_painted` beside `font_size`, because `paint` has no
  `&[Style]` (`ops.rs`, `record_painted`'s rustdoc). Bytes: 1 of
  `NodeState`'s 2; one left.
- **Which widgets read it**, declared by their `Seed` masks so the
  `W1010` sheet lint reports a rule that sets it on anything else:
  `Label` (the run is painted by the renderer at the pitch), `TextArea`
  (already paints one run per line at `line × line_h`, so `line_h`
  becomes the pitch and its caret, selection and scroll follow), and
  `RichText` (its `LINE = 1.45` stays the default when the field is 0).
  `TextInput`, `Button` and the rest do not read it in M80, and the lint
  says so rather than a silent no-op. New bit `Fields::LINE_HEIGHT`:
  the next free bit at landing, printed (expected bit 17, with
  `Fields::OVERFLOW` at 18; `Fields::EACH` 17 → 19 with both). M82's
  `BACKGROUND_FILL` and M86's `TRANSITION` take the bits after these.
- **Overflow is a `Style` enum**, `TextOverflow { Wrap, Clip, Ellipsis }`
  (`#[repr(u8)]`, default `Wrap`), named so it cannot be confused with
  `lumen-static-wgpu`'s private `Overflow`. Bytes: 2 of `Style`'s 2. New
  bit `Fields::OVERFLOW` (the next free bit at landing, printed;
  expected 18); read by `Label` and `AppBar`. `.lss`
  `text-overflow: ellipsis`, with a `Takes::Overflow` keyword mirror,
  and `scripts/check_lss_setters.sh`'s enum list gains it. Crate:
  `lumen-static`.
- **A `.lss` property is five sites, and the check sees two.**
  `scripts/check_lss_setters.sh` compares `style.rs`'s `impl Patch`
  with `lower.rs`'s `PROPERTIES` (and the `Align`/`Cursor`/`Role`
  mirrors) only. The others: the shell's runtime applier
  (`crates/lumen-static-shell/src/lss.rs`, `fn set`, one arm per
  `(setter, Val)` pair, panicking "no setter `{m}` taking {v:?}" on an
  unknown one); for a new value kind, a `lower::Val` variant
  (`lower.rs` has `F32`, `U8`, `Bool`, `Color`, `Role`, `Length`,
  `Edges`, `Align`, `Cursor`, no overflow enum) with its shell mapping;
  and its quote arm in `crates/lumen-static-macros/src/lss.rs`'s `val`.
  `line-height` (`Takes::Number` → `Val::F32`) needs only the
  `set` arm, since the macro quotes `.#method(#value)` generically;
  `text-overflow` needs all of them. Each lands with a rule in a sheet
  `crates/lumen-static-shell/tests/lss_parity.rs` compares, which is
  the gate for the arms the check cannot see.
- **`Clip` and `Ellipsis` are one line, cut in layout, by grapheme.**
  The first draft said it generalised a grapheme search; `fit_title`
  searches **char** boundaries (`nav.rs`, `t.char_indices().nth(k)`) and
  so already splits a combining sequence or a ZWJ cluster. The shared
  helper is new: `fit::cut` in `lumen-static-widgets` (new module
  `fit.rs`), a binary search over **byte offsets** snapped down to a
  grapheme boundary with `edit::floor_boundary` (new, beside
  `prev_boundary`), measuring prefixes with `measure_weighted` and the
  ellipsis once. The leaf profile has no `unicode-segmentation`, so
  there it snaps to `char` boundaries, which is `edit.rs`'s existing,
  documented tier rule. A hard line break ends the visible line and
  counts as overflow. `Clip` is the same cut with no mark, **not** a
  run clipped mid-glyph: `Recording` inks a run's whole rect, so a run
  wider than its node would damage outside the node (the M27/T27.3
  1.00× rule), and a glyph cut in half says nothing a reader can use.
  `AppBar::fit_title` moves onto the helper, and its key moves onto
  `MeasureKey::new`, which adds the measurer's generation that
  `fit_key` never had. RTL: the cut is logical; M81 owns its visual side.
- **Where the cut lives, and what it costs.** On `Label`, as
  `fit: (u32, f32)`: the prefix's end byte (`u32::MAX` = paint as
  today) and its width. **+8 B per `Label`**, the firmware's included,
  printed as `Label`'s new size and in the embedded `Ui` re-budget. It
  is computed inside `Measured::get_or`'s miss, so it is keyed exactly
  as the box is and a still frame cuts nothing. Paint is two
  `text_spaced` calls, prefix then `…`, as `AppBar` paints today.
- **`MeasureKey` gains one word**, `flow: u32` =
  `line_height | overflow << 8`, filled by `MeasureKey::of` from the
  `Style` it already takes, so every text memo keys on both without a
  call-site change. 32 → 36 B, which grows every memo-holding widget
  (**`Measured` +4 B, `FoldedText` +4 or +8** by alignment; printed on
  the target). Without it a `Wrap` → `Ellipsis` change serves the
  stale multi-line box (the M45/T45.2 shape).
- **`max_lines` is not built.** It needs a third `Style` byte, which
  would take `Style` to 84 B for a field nothing in this repository
  asks for. Reopen when an application asks, and do it in the commit
  that already grows `Style` (M82).
- **Read-only selection is `Selectable<S>`, one table entry sharing its
  label's index.** The first draft's "makes its child focusable" and
  "`Selectable<W>` over any `W`" both had no mechanism: focusability is
  the const `NodeDesc::FOCUSABLE` flag in a `SUB_TABLE`, and no `Node`
  method exposes a child's text (`Label::as_str` is inherent). So
  `Selectable<S: TextStorage>` owns a `pub label: Label<S>`, has
  `NODE_COUNT = 1` and a `FOCUSABLE` entry, lays out and paints through
  the label at the same index, and **answers `Node::selection` itself**,
  as `NumberInput` does for its field (`field.rs`). `Host::copy` asks
  the focused index (`lib.rs`, `node(i).selection()`), so the answer is
  Selectable's. It holds `clusters: Vec<Cluster>`, `anchor`, `caret` and
  its row pitch, and reuses `edit::word_at`, the grapheme steps and
  `input::selection_tint`. **`alloc`-tier only**, beside `Select` and
  `RichText`, so the leaf firmware does not link it. Crate:
  `lumen-static-widgets` (new module `selectable.rs`). `Label` gains
  only a `pub(crate)` split of its paint (background, then run), so the
  tint can go between them. **Not over `RichText` or `Markdown`**: they
  paint word by word, so one cluster list does not describe them.
  Deferred, and no milestone in this plan owns it (M87 does not);
  reopen when an application asks to select rich or Markdown text, with
  a cluster list per word run.
- **Ctrl+X on a `Selectable` is a copy.** `Selectable` answers
  `selection`, so `keys::clipboard_decision` returns `Clip::Cut` for it
  (`keys.rs`, `Key::Char('x') if s.selection`), and
  `Win::clipboard_shortcut` sets the clipboard and then posts a
  `Backspace` (`lumen-static-winit/src/lib.rs`). Decided: that is
  correct for read-only text as long as the `Backspace` does nothing,
  so the text reaches the clipboard and nothing changes, which is a
  copy. `Selectable::on_event` does not handle `Backspace` or `Delete`
  (and leaves the selection as it was), and its rustdoc says Ctrl+X
  degrades to copy. No `keys.rs` or `Session` change: an "editable"
  input would need a new required `Session` method and a `Node` slot
  for a case one widget's `on_event` already decides.
- **IME placement is `Node::caret_rect`, asked of the focused index.**
  Provided `None` on `Node` (`lumen-static`), overridden by `TextInput`
  and `TextArea`, and **forwarded by `NumberInput`**, the one widget
  that shares its editable child's index (`SearchField`, `Chooser` and
  `RichTextEditor` give theirs a node of its own, so they need nothing;
  `widget.rs`'s `paste` rustdoc names the same set). No general wrapper
  forwards it, because no wrapper forwards `selection`, `compose` or
  `paste` either: the first draft's premise was wrong. Firmware: one
  slot on every `Node` vtable plus the shared default body, printed.
- **`Host::caret_rect()` is physical** (logical × `Host::scale`, M40),
  `None` without focus. **`Session::caret_rect()` is required**, as
  every `Session` method is (`session.rs`): a missing forward in `impl
  Session for Hot` is E0046, so **the compiler is the forwarding gate**
  and no stamp test is written. The ABI stamp covers the vtable through
  the M49/T49.1 source hash of the whole crate, not through
  `abi_fields` (which lists the version, the features and
  `size_of::<Box<dyn Session>>()`), as `docs/plan-iced-roadmap.md` §2.3
  states. **`Local::caret_rect` is guarded**: it asks a widget's
  `Node::caret_rect`, which is application code, so it goes through
  `self.caught(None, ..)` exactly as `copy` does (`session.rs`, whose
  module rustdoc forbids a panic unwinding across into the loader). A
  panicking `caret_rect` answers `None` and raises M36's screen; `Win`
  then passes no IME area.
- **`Win` calls `window.set_ime_cursor_area` after each frame** in which
  the rect changed, beside the title and cursor updates in `redraw`, as
  `PhysicalPosition`/`PhysicalSize` (winit 0.30.13). It keeps the rect
  it last passed in `ime_area: Option<[f32; 4]>` (20 B per window) and
  answers **`app.imeArea`** with it, a method `Win` answers itself as it
  does `app.presents`, because the call happens outside the session the
  agent talks to. From M89 the field lives on each `WindowSlot`, and
  `app.imeArea` answers the window that the request's `window` member
  names (`docs/plan-windows.md`, second review G24). This closes
  `docs/plan-desktop.md`'s "host can produce but does not expose".
- **`app.copy`, for the drives.** Agent input goes to `Session::agent`,
  and Ctrl+C is handled only by `Win::clipboard_shortcut` on a real
  `KeyboardInput`, so a drive's `input.key` Ctrl+C never copies. `Win`
  answers `app.copy` itself: `session.copy()`, then `clipboard.set`,
  then `clipboard.get`, and replies `{"copied", "clipboard"}` so a drive
  sees what reached the system clipboard. Crate: `lumen-static-winit`.
  M88's rich clipboard may extend it; it does not depend on S1's
  `Request` queue, because the user, not a handler, starts a copy.
- **S1 and S2.** M80 adds no platform request, so no `Request` variant.
  It adds no side store: `line_height` is inline in `Cmd::Text`, and the
  `…` run is an ordinary `Cmd::Text`, whose hash folds the run's
  **length**, not its buffer offset (`display.rs`, M27/T27.3). T80.3's
  S2 gate re-checks that with a second run in play.

**Tasks.**

*Every task below, in its own commit (S4): the rustdoc on what it
changed; its `docs/results-text.md` section (and
`docs/results-embedded.md` rows where it prints a firmware or `Ui`
figure); the book's `book/src/text.md` by named anchor where the
behaviour is reader-facing; its `scripts/doc-identifiers.allow` lines
removed as it builds them. `docs/00-handoff-static-tree.md` has no text
section and no OPEN marker for text flow (checked); a task adds one
line under §12 only if it settles a question listed there. Every gate
prints its quantity beside its bound, asserts its setup landed, is run
red under its injection with the output in the commit body, and green
in the accept direction in the same session.*

- **T80.0 — gates that compile today; the plan, the queue and the
  80 KB firmware ceiling.**
  Commits **all twelve plan files together** (`docs/plan-iced-roadmap.md`
  and the eleven milestone files it cites, so `check_doc_paths.sh`
  finds every one), both pre-seeded allowlist blocks the assembler
  generated (`scripts/doc-identifiers.allow`, M80's names among them;
  `scripts/doc-paths.allow`'s M80–M90 block: the eight
  `docs/results-*.md` files and `bin vocabulary`, `bin scroll_cull`,
  `bin markdown`, each removed by the task that creates it), and an
  AGENT.md queue entry for M80–M90 in §5's order (M28). Accept:
  `just static` green on the commit.
  Replaces the stale rows of `docs/competitive-feature-gaps.md` (its
  "common to all six" table: images, vector drawing, animation,
  overlays, multi-window, complex-script text) with pointers;
  `book/src/gaps.md` already flags that table as stale and is not
  touched. Re-measures the winit manifest's three crate deltas with
  K2's exact command, `cargo tree -p lumen-static-winit -e normal
  --prefix none [--features F] | sed 's/ (\*)$//' | sort -u | wc -l`:
  default, `--no-default-features` with and without `clipboard`,
  `dialogs`, `a11y` (at `553fa5e`: 96 default; clipboard +5, 91 → 96;
  dialogs +10, 96 → 106; a11y +71, 96 → 167 — re-measured at landing
  and printed). Records them in `docs/results-a11y-platform.md` and
  rewrites `crates/lumen-static-winit/Cargo.toml`:47 (clipboard "11"),
  :90–91 (dialogs "79"; "clipboard's 11 and a11y's 62") and :115
  (dialogs "79") with the method named and the doc cited. **Gates:**
  - `crates/lumen-static-shell/tests/ellipsis_graphemes.rs`: for each
    of the four `graphemes.rs` clusters, titles whose prefix length
    puts the cluster at every candidate cut, swept over room widths
    in 1 px steps through `AppBar` under `FontMetrics`. Prints, per
    cluster, cuts that landed inside it (bound 0) and cuts made within
    one cluster of it (setup, bound > 0 each, so the sweep cannot pass
    by never cutting there). **It pins today's gap, green, per
    cluster at the value measured at HEAD**: `fit_title` cuts on
    chars, so the ZWJ family (between the ZWJ and the next person),
    the flag and the skin tone are pinned at > 0 inside, each measured
    and printed; any that measures 0 at HEAD is pinned at 0 with the
    reason printed. The combining accent is pinned at **0**: in
    `tests/fonts/DejaVuSans.ttf` U+0301 has advance 0 (glyph 690,
    `hmtx`), so `w("…e") == w("…e\u{301}")` and a search for the
    largest fitting char prefix always lands after the accent. Only a
    cutter that does not maximise width could split it, so its 0 is a
    property of the search, not of the snap. T80.3 flips the others to
    0. No `#[ignore]` (§3.1). Accept direction: an all-ASCII corpus
    reports 0 inside and > 0 near.
  - `scripts/firmware_delta.sh BASE [BIN=B ...] [print]` (new),
    wrapped by the recipe `firmware-delta base *bounds:` and invoked
    positionally, `just firmware-delta <sha> firmware=0
    firmware_interactive=256` (`just` reads `NAME=VALUE` as an
    override only *before* the recipe name, and every parameterised
    recipe in the `justfile` is invoked positionally, e.g.
    `idle-count`, so the arguments are plain `BIN=B` words the script
    parses, never `--` flags). It builds the four
    `scripts/lib/firmware.sh` binaries at `BASE` (a worktree) and at
    the work tree, prints `.text`/`.rodata` for each side and the
    delta, and fails a `.text` delta above `B` on the binary each
    `BIN=B` names, by name. **Bounds are per binary**: with no `BIN=B`
    the default is K4's cap, `firmware_interactive=1000`, the other
    three printed and unbounded; `print` never fails (a task that only
    records its growth). Every citing task writes its invocation out in
    full. This is the one interface; M82, M84 and M90 cite it and add
    none. `just firmware-size` only prints and `scripts/check_text.sh`
    allows 1% against the headline, about 280 B of the paint-only
    firmware, so neither can say "unchanged". Accept: T80.0 itself
    prints 0 on all four under `firmware=0 firmware_interactive=0
    firmware_bind=0 firmware_cascade=0`. Injections: a
    `#[inline(never)]` counter added to `Label::paint` prints a
    non-zero `.text` delta and fails that invocation by name; the same
    tree under `print` exits 0 with the delta printed; and
    `firmware=0 firmware_interactive=100000` fails on `firmware` only
    (the per-binary bound is read per binary, not as one maximum).
  - **The firmware ceiling goes to 80 KB** (owner decision D5,
    2026-09-30, `docs/plan-iced-roadmap.md` header and K4). Changes,
    found by grepping `scripts/`, `justfile`, `AGENT.md`, `book/src`
    and `docs/` for `65536`, `65,536` and `64 KB`:
    - `scripts/check_text.sh`: `CEILING=${LUMEN_TEXT_CEILING:-65536}`
      becomes `81920`. Its header ("under an eighth of flash", "The
      interactive one keeps the same 64 KB"), the comment above
      that line ("An eighth of the T13.0 device's 512 KB flash") and
      the failure message (`check_text.sh:195`, "An eighth of the
      device's flash is the budget") are rewritten to 80 KB, 15.6% of
      the part, citing D5 and the results doc.
    - `scripts/check_text.sh` **asserts its default agrees with the
      budget row**: it reads the `Framework .text` row of
      `docs/results-embedded.md` (`≤ 80 KB` → 81,920) and fails by name
      when the unset default differs, so the script and the doc stop
      being two copies of one number. `LUMEN_TEXT_CEILING` still
      overrides the comparison and skips the agreement check, which is
      about the default.
    - `docs/results-embedded.md`: the budget row "Framework `.text` ≤
      64 KB" becomes ≤ 80 KB, its "An eighth of flash" rationale
      restated with the owner decision, the date and the argument:
      15.6% of the 512 KB part, under the row's own "more than a fifth
      of the part" line. The current-figure statements are restated
      against 81,920 B: the headline row (`.text` ≤ 64 KB, "43.7% of
      budget" → 34.9%) and the M56-merge paragraph
      (`firmware_interactive` "94.5% of 64 KB" → 75.6% of 80 KB,
      "3,614 B left" → 19,998 B). Dated tables and paragraphs that
      state "of 64 KB" or "of 65,536" for an earlier commit (M52,
      M55/T55.5, M45/T45.12, the 2026-09-10 enforcement) stay as
      measured, with one line saying the ceiling was 64 KB then. The
      present-tense statement at `results-embedded.md:153` ("against
      the same 64 KB ceiling") is restated at 80 KB. The newest
      four-binary table is M52's (2026-09-27); the M56-merge figures
      exist only as prose, so T80.0 **writes a dated four-binary table
      at `553fa5e`** (`.text`, % of 81,920, `.rodata`, per binary),
      which is the table K4's "rewrite the four-binary table" rewrites
      from then on.
    - `docs/plan-embedded-footprint.md` §2.5 ("The interactive firmware
      keeps the 64 KB ceiling") gains a SUPERSEDED line pointing at D5;
      its argument (one ceiling for every binary) still holds.
    - `scripts/selftest/plants/build/text-float-fmt.sh`'s comment
      ("whose .text stays under 64 KB") is updated; the plant keys on
      the float-formatting count, not the ceiling, so it still fires.
    - Found and left alone: `justfile` and `AGENT.md` hard-code no
      ceiling; `docs/00-handoff-static-tree.md` states none (checked);
      `docs/results-widget-crate.md`, `docs/results-frame-and-types.md`
      and `docs/results-deep-review.md` state 64 KB or 65,536 in dated
      measurements. The other hits are not the ceiling:
      `fixtures/embedded/src/bin/fwq.rs` (a stack reading),
      `docs/results-text.md` (a 64 KB `TextArea` value),
      `docs/plan-hash-identity.md` (65,536 instances).
  - **Gate: the raised ceiling fires and accepts.** `just firmware`
    (`scripts/check_text.sh`) prints each binary's `.text` "of 81920".
    - Injection: `LUMEN_TEXT_CEILING=60000 ./scripts/check_text.sh`
      fails by name, `firmware_interactive: .text is 61922 B, over the
      60000 B ceiling by 1922 B` (byte counts as measured that day).
    - Accept: the unset default (81,920) passes all four, each printed
      beside the ceiling, in the same session.
    - Setup landed: the printed ceiling reads 81920, not 65536.
    - The injection above exercises only the comparator, which T80.0
      does not change. What it changes, the default and the row, gets
      a selftest plant, `scripts/selftest/plants/build/text-ceiling-row.sh`:
      it edits the budget row back to `≤ 64 KB` and expects the
      agreement check to fail by name (both numbers printed, 81920 and
      65536). Accept, same session: the unplanted tree passes the
      check with both printed equal.
- **T80.1 — the paint seam: `Painter::text_spaced`,
  `Cmd::Text.line_height`, lines at a pitch.** Nothing sets it yet;
  this commit makes the byte travel and be placed. Changes:
  `Painter::text_spaced` (required), `text_weighted` provided;
  implemented by `Scaled` (origin rounding moved in from its
  `text_weighted`), `DisplayList`, `Deferring` (`overlay.rs`),
  `Recording` (`retain.rs`, folds the byte), `Checksum`
  (`fixtures/embedded`, folds it), `benches/src/bin/retain.rs`'s
  `Sink`, and the test painters in `tests/painter.rs`,
  `tests/overlay.rs`, `tests/glass_cmd.rs`, `tests/vector.rs`,
  `tests/container_fill.rs`, `tests/retain_align.rs`. The `text_weighted`
  overrides on `Scaled`, `Deferring` and `Recording` are deleted
  (Decisions). The `Cmd::Text`
  field, `Cmd::hash_into`, `wire.rs` (`VERSION` + 1, expected 2),
  `svg.rs`, and every full-field `Cmd::Text` pattern or construction:
  the exhaustive match and the construction at
  `crates/lumen-static-shell/tests/svg.rs:78`,
  `crates/lumen-static-wgpu/tests/parity.rs:282`,
  `crates/lumen-static-shell/tests/clip_test.rs:262`, the showcase's
  destructuring in `examples/showcase/src/main.rs`, and any other an
  `rg 'Cmd::Text \{'` at landing finds (count printed; the compiler
  finds the rest, the list is what the commit body checks).
  `FontStack::place_run_into` gains `line_height: u8` and the
  re-placement; `place_run` passes 0. `software.rs` and
  `lumen-static-wgpu`'s run path pass the command's byte. **Gates:**
  - placement, `crates/lumen-static-shell/tests/line_height.rs` (new):
    a two-line run through `place_run_into` at `line_height` 16, 24
    and 32. Prints the top of line 2's first glyph minus line 1's,
    against `pitch` ± 0.5 px, and the extent's height against
    `2 × pitch + max(0, nls − pitch)` (`nls` and the ink term
    printed; at 24 and 32 the deficit is 0 for the test faces, so the
    bound is exactly `2 × pitch`). Setup: the run really broke into
    two lines (count printed, must be 2). Accept: at 0, placement is
    byte-identical to today's `place_run` (every `Placed` equal);
    this accept direction is also the observer for "treat 0 as
    `pitch = px`" (1.0×), which moves every `Placed`. Injection: skip
    the re-placement when `line_height != 0` (lines at the face's
    spacing; the delta prints `nls`, not `pitch`).
  - ink stays in the rect below the face's spacing, same file: at
    `line_height` 16 (1.0×, under the test faces' ≈1.36 em) on a
    two-line run with ascenders and descenders (`"Ågjy\nÅgjy"` or
    the corpus's equivalent), the painted ink rows on the software
    canvas above the run's rect and below it, printed, bound 0 each;
    and the `Recording` damage of a text change over that run covers
    every changed pixel (changed pixels outside the damage printed,
    bound 0). Setup: half-leading printed and negative. Injection:
    drop the `max(0, nls − pitch)` term from both the measure and the
    placer shift; the ascenders' rows above the rect print > 0.
  - each backend passes the byte: a `line_height = 24` case in
    `crates/lumen-static-wgpu/tests/parity.rs` (`just gpu`). Parity
    cannot see a placer bug, because both backends call the same
    `place_run_into` (`text.rs`'s own rustdoc says parity is about
    rasterisation); it **can** see a backend that drops the byte.
    Injection: the GPU path passes 0; the case's differing-pixel count
    is printed against the file's existing bound. The software half is
    the pixel rows of the placement gate's run on the canvas.
  - retention, `crates/lumen-static/tests/retain_align.rs` (which
    already drives `Recording` directly; `damage_regions.rs` is a
    pure `Regions`/`Damage` unit file): the same rect, text, size,
    weight and colour recorded twice with only the byte changed is
    damage, printed as the damaged area against the rect's (bound:
    equal). Setup: the two rects are printed and equal, so the byte
    is the only difference. Injection: `Recording` leaves the byte out
    of its hash; damage prints 0.
  - forwarding (`docs/plan-iced-roadmap.md` §2.2), `tests/overlay.rs`'s
    `deferring_forwards_every_command`: its `paint` gains
    `p.text_spaced(.., 24, ..)`, so the command count goes **5 → 6**
    (printed), and the whole-`Cmd` comparison sees the forwarded
    `line_height`. Injections: `Deferring::text_spaced` has an empty
    body (the test prints "6 in, 5 out" by name); it forwards through
    `inner.text_weighted` (the plausible wrong forward, which drops the
    byte: the streams differ at the text command, by name). The same
    shape for `Scaled` (a `Scaled` over a `DisplayList` at scale 2:
    the forwarded `Cmd::Text.line_height` prints 24; injection:
    `Scaled::text_spaced` forwards 0) and for `Recording` (its inner
    list receives the byte; injection: forwards 0). M82's and M83's
    counts follow from 6, printed at landing.
  - **No golden gate**: `crates/lumen-static-shell/tests/golden.rs`
    holds one rect and one rounded rect plus the panic-screen images,
    built with `BlockMetrics`, and the `justfile` runs it with
    `truetype` off, where `software.rs` draws block glyphs and never
    calls `place_run_into`. No golden can change under a placer
    injection; the placement gate's accept direction is the observer.
  - `tests/glass_cmd.rs` still asserts `Cmd` at 60 B (printed).
  - the wire, `wire.rs`'s tests: a `Cmd::Text` with `line_height = 24`
    round-trips through `encode`/`decode` with the byte intact, and a
    buffer of the previous version decodes to `None` (both versions
    printed). Injection: `encode` writes the field and `decode` skips
    it; the round-trip prints 0.
  - firmware: `just firmware-delta <T80.0 sha>
    firmware_interactive=1000`, printed per binary. Accepted growth
    (D6, owner, 2026-10-02), bounded by K4's per-task cap on
    `firmware_interactive`. Expected on the paint-only `firmware`:
    about +676 B, T27.1's paint-only figure for a required method plus
    a field (`docs/results-embedded.md`), printed beside the measured
    delta and not a bound. If the cap fires: the task stops, the delta
    is attributed by a symbol diff, and the owner decides (K4).
  - live: none; nothing sets the byte until T80.2, whose drive covers
    this path.
- **T80.2 — `Style.line_height`, measured and painted.** `Style` field,
  `Patch::line_height`, `Fields::LINE_HEIGHT`, the `.lss` `line-height`
  declaration (its `PROPERTIES` row, `Takes::Number`; the shell
  `lss.rs` `set` arm `("line_height", Val::F32(x))`; a `line-height`
  rule in a sheet `lss_parity.rs` compares),
  `crates/lumen-static-widgets/tests/style_reads.rs`'s `patch_for`
  arm for `line_height` (it panics "no probe value for field" on any
  new `Fields::EACH` name), `NodeState.line_height` stamped by `record_painted`,
  `MeasureKey`'s `flow` word, `LineCount`. `Label` measures
  `lines × pitch` when the field is set and paints through
  `text_spaced`; `TextArea`'s `line_h` becomes the pitch (caret,
  selection, scroll and `paint_lines` follow) and it passes the byte;
  `RichText` uses the field in place of `LINE` when it is set. The
  three seeds declare the bit. Re-budgets the embedded `Ui` assertion
  (1,192 B today) with the `flow` word's delta attributed by removing
  it. Showcase: the `Label` demo gains a "1.5× line height" switch,
  restyled through `Program::style_by`. Drive: `Label`'s
  `scripts/drive/undriven.allow` line ("takes no input") is replaced by
  an inventory whose action is that switch, with a `"Label": "Label"`
  row in `scripts/drive/roles.py` (`run.py --manifest-only` refuses an
  entry with none); manifest counts updated.
  **Gates:**
  - measured and painted agree, `crates/lumen-static-shell/tests/line_height.rs`:
    a `Label` that wraps to 3 lines at `line_height = 24`, 16 px, laid
    out through a `Host` with `FontMetrics` and painted on the software
    canvas at scale 1.0 and 2.0. Prints content height against exactly
    `3 × 24/16 × 16`, and line 3's first ink row minus line 1's against
    `2 × pitch × scale` ± 1 px. Setup: line count 3 (printed).
    Accept: at 0 the box equals today's `measure_weighted` height.
    Injections: `Label` measures with `measure_weighted` (height prints
    the face's, not `3 × pitch`); `record_painted` does not stamp the
    byte (ink rows print the face's spacing).
  - `TextArea`, same file: after typing to line 3 at `line_height =
    32`, the caret's top minus the content top prints against
    `2 × pitch` and against line 3's ink rows ± 1 px. Injection:
    `TextArea` keeps `cx.text.line_height(px)`; the drift prints.
  - the memo, `crates/lumen-static/tests/measure_key.rs`: a `Label`
    laid out, then only `line_height` changed, is re-measured (warm
    before, cold after, heights printed and different); the same for
    only `overflow` (for T80.3). Injection: drop `flow` from
    `MeasureKey::of`; the stale height is served and printed.
  - the lint, `crates/lumen-static/tests/sheet_lint.rs`: a rule setting
    `line_height` on a `Button` reports `W1010`, and on a `Label` does
    not. Injection: `Label`'s seed omits the bit; the `Label` rule is
    reported.
  - sizes (owner D4; M80 grows neither): `Style` 80 and `NodeState`
    84, printed with 164 B/node and 16.0 KB per 100 nodes
    (`tests/cursor.rs`,
    `tests/focus.rs`, `fixtures/embedded`); the `Ui` figure printed old
    and new with the attribution. `.lss`: `scripts/check_lss_setters.sh`
    green with the new count printed.
  - the runtime applier, `lss_parity.rs`: the sheet with `line-height`
    lowers the same through `include_lss!` and `lss::load` (rule count
    printed). Injection: delete the shell `set` arm;
    `check_lss_setters.sh` stays green (it cannot see `set`) and the
    parity test panics "no setter `line_height` taking F32".
  - the masks, `style_reads.rs`: `Label`, `TextArea` and `RichText`
    measure as reading `line_height` and nothing else does (the
    measured set per kind printed). Probe `p.line_height(1.5)`, which
    changes even a single line's height (Decisions, "a single line
    included").
    Injection: `TextArea`'s seed omits the bit; "a field measured and
    not declared" fires for `TextArea` by name.
  - live, both strategies (`just widget-drive Label`, which runs
    `Everything`, `Boundaries`, the oracle leg and the scale-2 leg):
    pressing the switch changes the label's height by `lines × (pitch −
    before)` from `ui.getLayout` (printed), its `ui.regionHash` changes
    (M57's D1), and damage is 1.00× the switch's and the label's boxes; the
    same press just outside the switch changes neither. Setup: the
    press was `handled`.
  - firmware: `just firmware-delta <T80.1 sha>`, printed, bound K4's
    default (`firmware_interactive=1000`).
- **T80.3 — `TextOverflow::Clip` and `Ellipsis`.** `Style.overflow`,
  `Patch::overflow`, `Fields::OVERFLOW`, `.lss` `text-overflow` and its
  keyword mirror (the `PROPERTIES` row with `Takes::Overflow`, a
  `lower::Val::TextOverflow` variant, the shell `set` arm and its mapping
  function, the quote arm in `crates/lumen-static-macros/src/lss.rs`'s
  `val`, a `text-overflow` rule in a sheet `lss_parity.rs` compares),
  `style_reads.rs`'s `overflow` probe (the `Label` and `AppBar`
  probes' text lengthened so it wraps at `BASE`'s 200 px; today's
  "probe text" fits, so `overflow` would measure as unread),
  `fit::cut`, `edit::floor_boundary`, `Label.fit`,
  `AppBar` onto the helper (its `fit`/`fit_key` replaced; `AppBar`'s
  size printed before and after). Flips T80.0's corpus gate from
  "> 0 inside" to "0 inside".
  Re-budgets the embedded `Ui` for `Label`'s 8 B, attributed. Showcase:
  the `Label` demo gains an "ellipsis" switch over a long label in a
  fixed-width box, added to the T80.2 inventory. **Gates:**
  - T80.0's corpus, its assertion flipped: 0 cuts inside every
    cluster, > 0 near each (printed per cluster), for `AppBar` **and**
    `Label` with `Ellipsis`. Injection: `fit::cut` snaps to `char`
    boundaries, as `fit_title` did; the corpus names the cluster it
    split, chosen per cluster: the ZWJ family, the flag and the skin
    tone (whichever T80.0 pinned > 0) go red. The combining accent
    stays 0 under that injection by the search's own property (T80.0);
    it is printed and not counted as the injection's witness.
  - the `.lss` value kind, `lss_parity.rs`: `text-overflow: ellipsis`
    lowers to the same `Rule` both ways. Injections: delete the shell
    `set` arm (panics "no setter `overflow` taking TextOverflow"); map
    `ellipsis` to `Clip` in the macros quote arm (the comparison
    names the rule and selector).
  - the masks, `style_reads.rs`: `Label` and `AppBar` measure as
    reading `overflow` (setup: the probe text's line count under
    `Wrap` printed, > 1). Injection: the probe text left short; both
    print as declared and not measured, which is the setup check
    firing.
  - fits its box, `crates/lumen-static-shell/tests/text_containment.rs`:
    widths 40–400 px in 8 px steps over five label texts; for each
    truncated case the **painted ink bounding box** on the software
    canvas is inside the content box (overhang printed, bound 0 px) and
    the list ends in a `…` run. Prints how many cases truncated (bound
    ≥ 30 of 230, so the sweep cannot pass by fitting). Accept: an
    untruncated `Ellipsis` label's display list is byte-identical to
    the same label under `Wrap`. Injection: `fit::cut` forgets the
    ellipsis's width; the ink overhang prints > 0.
  - the S2 shape, `crates/lumen-static-agent/tests/damage_shape.rs`,
    beside `a_text_length_change_does_not_report_later_nodes` (the
    existing gate for this defect; `Cmd::hash_into` is read by the
    agent journal, not by `Recording`, which hashes the `&str` itself,
    so a `lumen-static` test cannot see it): two truncated labels in a
    column; changing the first's text so its cut moves reports the
    first and **not** the second in the journal's changed ids (both
    printed; bound: second absent). Setup: the first label is
    reported. Injection: `Cmd::hash_into` folds `text.0` (the offset)
    for `Cmd::Text`, which is M27/T27.3's defect; the second label is
    reported.
  - `Clip` never paints outside its node: per-widget damage of a
    clipped label whose text changes is 1.00× its box (printed).
    Injection: paint the whole text in a rect of its natural width
    under a clip; damage prints > 1.00×.
  - the key: T80.2's `overflow`-only memo case is green here.
  - cost (K80d), in its own test binary
    `crates/lumen-static-shell/tests/fit_cost.rs` with a single
    `#[test]`: `text_counters()` is process-wide and asks for "one
    test at a time" (`text.rs`), and cargo runs a binary's tests in
    parallel. `measured` read before and after each cut, the
    difference printed per cut against `⌈log2(len)⌉ + 3`. Injection:
    a linear search over prefixes; the long titles print over the
    bound.
  - live, both strategies (`just widget-drive Label`): the ellipsis
    switch takes the label from its wrapped height to one line
    (`ui.getLayout`, printed), `ui.regionHash` changes, damage 1.00×;
    the `~text` selector still matches the **whole** label text
    (`observe` publishes it, M57's D7). The `AppBar` inventory
    (`scripts/drive/widgets/nav.py`) is re-run unchanged.
  - firmware: `just firmware-delta <T80.2 sha>` (K4's default bound),
    and `just firmware-delta <T80.0 sha> firmware=1024
    firmware_interactive=1536` for K80b.
- **M80a closes**: K80a–K80c read, `just ci-verify <sha>` green, the
  SHA cited in AGENT.md's queue entry (S5).
- **T80.4 — `Selectable<S>` and `app.copy`.** The widget, its
  `on_event` (press sets the caret, drag extends under capture, double
  press selects `word_at`, triple selects all, Ctrl+A selects all,
  `Focus { gained: false }` clears), `Node::selection`, the tint behind
  the run, `TextTarget` forwarding that clamps the selection when a
  bind replaces the text. `Label`'s `pub(crate)` paint split. `Win`'s
  `app.copy`. Same commit (S4): an inventory in
  `scripts/drive/widgets/text_and_slider.py` with a `"Selectable":
  "Label"` row in `scripts/drive/roles.py`, a `scripts/drive/keys.py`
  `KEYS` entry (Tab reaches it, Ctrl+A selects all), an `observe` impl
  (`Role::Label`, the whole text as the value), a row in
  `crates/lumen-static-widgets/tests/a11y_roles.rs`'s table, a field in
  `tests/catalogue/mod.rs` so the role and contrast harnesses see it, a
  showcase entry, `app.copy` in the skill's method list and
  `book/src/agent.md` (`scripts/check_agent_methods.sh`). **Gates:**
  - `crates/lumen-static-widgets/tests/selectable.rs` (new), through a
    `Host` with `FontMetrics`: a press and a drag over a two-line
    label; `Host::copy()` is exactly the dragged substring (length
    printed). Setup: both pumps `handled`, and the focused index is the
    Selectable's. A double press copies one word, a triple the whole
    text. Accept: a press on a plain `Label` beside it is not handled
    and `copy()` is `None`. Injection: remove Selectable's `selection`
    override; `copy()` is `None` with the press still `handled`.
  - rows: the tint's top on line 2 minus the painted ink rows of line 2,
    printed, bound 1 px, at `line_height` 0 and 24. The row pitch with
    the field at 0 is the content height over the line count, because
    the painter advances by the ceil'd new-line size and
    `TextMeasure::line_height` does not round. Injection: rows at
    `cx.text.line_height(px)`; the drift at line 2 prints.
  - a still frame reshapes nothing: `text_counters().measured` across
    two idle frames, printed, bound 0. Injection: reshape on every
    layout.
  - `size_of::<Label>()` equals T80.3's (printed; baseline named), and
    `just firmware-delta <T80.3 sha> firmware=0
    firmware_interactive=0 firmware_bind=0 firmware_cascade=0` prints
    **0 B** `.text` and `.rodata` on all four binaries: the widget is
    `alloc`-tier and the split must compile to the same leaf code.
    Injection: mark `Label`'s `pub(crate)` paint split
    `#[inline(never)]`, which compiles in the leaf tier and breaks
    exactly what this gate protects; the delta prints non-zero and
    fails by name. (Exporting `Selectable` outside the `alloc` gate
    cannot be the injection: its `Vec<Cluster>` does not compile in
    the leaf `no_std` build, so it is a build error, not a red delta.)
  - Ctrl+X degrades to copy, `selectable.rs`: after a drag selects a
    word, a `Backspace` posted to the `Host` (what
    `Win::clipboard_shortcut` posts after a Cut) is not handled, the
    label's text is unchanged and `Host::copy()` still returns the
    word (both printed). Setup: the drag's pumps were `handled`, and
    `keys::clipboard_decision` returns `Clip::Cut` for that state
    (asserted, so the case is the one the window produces).
    Injection: `Selectable` handles `Backspace` by clearing its
    selection, as an editor's delete would; `copy()` prints `None`.
  - M57's D3/D4 gates: `Selectable` is reached by Tab in the
    keyboard-only pass, and its tint passes the contrast gate in both
    palettes.
  - live, both strategies (`just widget-drive Selectable`): drag across
    three words, then `app.copy` returns them (`copied` and `clipboard`
    both equal, length printed); a double click copies a word; a press
    just outside clears it (`copied` is `null`). Damage per action is
    1.00× the Selectable's box, and `ui.regionHash` changes. Setup:
    every input reply `handled`. Injection: as the headless one; the
    drive prints `copied: null` after a handled drag. The system
    clipboard is `arboard`'s X11 backend, which needs no external tool,
    so on the reference machine the drive **never reports skipped**: a
    `clipboard` that fails to read back fails the drive with the
    clipboard error printed. (`xclip`, owner decision D7, is the
    independent reader M88's primary-selection leg needs; this drive
    reads back through
    `Win`'s own clipboard and does not use it.) Elsewhere a skipped
    drive does not close M80b (`docs/plan-iced-roadmap.md` §3.1).
- **T80.5 — `caret_rect`, `Session::caret_rect`,
  `set_ime_cursor_area`, `app.imeArea`.** `Node::caret_rect` (provided
  `None`), `TextInput`/`TextArea` overrides (scroll offset included),
  `NumberInput`'s forward, `Host::caret_rect`, the required
  `Session::caret_rect` with `Local`'s body and `Hot`'s forward
  (`crates/lumen-static-reload/src/lib.rs`), `Win`'s call and
  `app.imeArea` (skill and book lists). `fixtures/hot-window` gains a
  `TextInput` and `scripts/hot_window_drive.py` a step. The rustdoc on
  `widget.rs`'s `paste` and on `caret_rect` name the same
  index-sharing rule. **Gates:**
  - against the painted caret, `crates/lumen-static-shell/tests/ime_area.rs`
    (new): for `TextInput` empty, mid-text and filled past its width
    (the scrolled case), `TextArea` on line 3, and `NumberInput`, at
    scale 1.0 and 2.0: `Host::caret_rect()`'s x against the caret
    column found in the software canvas's pixels, printed, bound 1
    physical px. The reference is the painted pixels, not the same
    `caret_rect` twice. Setup: the field is focused (`has_focus`) and
    the caret column was found. Injections: `TextInput` leaves out its
    scroll offset (the scrolled case prints the offset); `Host`
    returns the logical rect (scale 2.0 prints half).
  - every composing node answers,
    `crates/lumen-static-widgets/tests/caret_rect.rs` (new): each
    focusable node of the catalogue that accepts a `Preedit` answers
    `caret_rect` with a rect inside its box. Prints how many composed
    (bound ≥ 5: `TextInput`, `TextArea`, `NumberInput`, the search and
    combobox fields) and how many failed (bound 0). Injection:
    `NumberInput` does not forward; one failure, named.
  - live, both strategies and the scale-2 leg (`just widget-drive`
    over the text fields' inventories in
    `scripts/drive/widgets/text_and_slider.py` and `fields.py`): after
    each focusing action, `app.imeArea` is non-null and its x is within
    1 physical px of the caret column in `ui.screenshot` (both
    printed). Setup: the focus action was `handled`. Injections: `Win`
    never calls `set_ime_cursor_area` (`app.imeArea` is `null`); `Win`
    passes the logical rect (the scale-2 leg prints half the x).
  - across a swap (`just hot-window-drive`): after the swap, focusing
    the fixture's field gives a non-null `app.imeArea` inside the
    field's physical box. The compiler already refuses a `Hot` with no
    forward; the runtime injection is `Hot::caret_rect` returning
    `None`, which compiles and turns this step red.
  - panic guard (`just panic-drive`): `fixtures/panic-app` gains a
    site, a focused field whose `caret_rect` panics; the site count
    (seven today, `scripts/panic_drive.py`) and `--expect-panics` go up
    by one, printed. The recovering leg asserts M36's screen, the
    location, refused input and a clean retry, as for the other sites,
    and `app.imeArea` reads `null` while the screen is up. Injection:
    `Local::caret_rect` calls `self.host.caret_rect()` outside
    `caught`; the panic escapes the session, the site never reports
    through `app.failure`, and the leg fails on that site by name (in
    a hot library the same unwind would cross into the loader, which
    `session.rs`'s module rustdoc forbids).
  - firmware: `just firmware-delta <T80.4 sha>` prints the `Node`
    slot's cost on each binary, bounded by K4's default
    (`firmware_interactive=1000`), and reported to the owner with the
    number (no decision accepts it). This is not D6 growth: D6 names
    `text_spaced` and `fills` only.
- **M80b closes**: `just ci-verify <sha>` green, cited in AGENT.md's
  queue entry.

**Kill rules.**

- **K80a — the bytes.** After T80.3, `size_of::<Style>()` is 80 and
  `size_of::<NodeState>()` is 84 (`tests/cursor.rs`, `tests/focus.rs`,
  the `fixtures/embedded` `sizes` asserts, printed with RAM per 100
  nodes). The decision computes both fits; if the compiler's layout
  disagrees, the task stops, the measured sizes go in
  `docs/results-embedded.md`, and the growth is re-planned with M82,
  which is the milestone that grows `Style`, rather than paid silently
  here.
- **K80b — firmware.** M80a's cumulative `.text` delta from T80.0,
  printed by `just firmware-delta <T80.0 sha> firmware=1024
  firmware_interactive=1536`, stays ≤ 1,024 B on `firmware`
  and ≤ 1,536 B on `firmware_interactive`. These were set as M80's
  share of the 3,614 B the interactive build had under 64 KB; the
  owner raised the ceiling to 80 KB (D5, 2026-09-30), which leaves
  19,998 B, and the caps stay as M80's own budget rather than a share
  of the headroom. If it fires, the leaf tier (no `alloc`) reads
  `TextOverflow` as `Wrap`, the rustdoc says so, and the delta is
  re-measured; if it still fires, M80a stops and records the number.
- **K80c — frames with the features unused.** Under `just bench-lock`,
  eight runs a side, every style at its default: `benches --bin grid`
  (`just grid`, the 2,227-node spreadsheet of `Label`s), `--bin
  retain --text` (the text-heavy hash arm, which pays `Recording`'s
  extra fold) and `--bin bindings`' "layout, memo warm" arm (which pays
  the wider key compare) stay inside their spreads. If one does not,
  the task stops, the cost is attributed by removing the field, and it
  is recorded before anything else is tried.
- **K80d — the cut's shaping.** `fit::cut` measures at most
  `⌈log2(len)⌉ + 3` prefixes per cut across T80.3's corpus, counted by
  `text_counters().measured` (a count, so it answers the same under
  load). If it fires, the search is replaced by one `shape` into a
  sink that keeps the last cluster whose x fits, confirmed by one
  measure.

**What building found (M80).**

- T80.1: **the test face's spacing is 19/16 em, not ≈1.36 em.** The
  pinned `DejaVuSans.ttf` at 16 px has a ceiled `new_line_size` of 19 px
  (`tests/line_height.rs` prints it), so the deficit `max(0, nls −
  pitch)` is non-zero only below 19/16, and iced's 1.3 (21/16) sits
  *above* the face's spacing, not under it as Decisions says. The rule
  is unchanged; the gates use 16 (1.0×, deficit 3 px) for the
  below-spacing case and 24/32 for the exact `n × pitch` case, as the
  plan's gate text already did. Recorded in `docs/results-text.md`
  M80/T80.1.
- T80.1: **the accept direction at 0 is compared against fontdue, not
  `place_run`.** `place_run` now calls `place_run_into(.., 0, ..)`, so
  "every `Placed` equal to `place_run`'s" compares the function with
  itself and cannot see the "treat 0 as `pitch = px`" injection. The
  gate lays the run out with fontdue directly and asserts every glyph
  keeps fontdue's `x` and differs in `y` by one constant (the cap
  shift); the injection moves 4 of 8.
- T80.1: **the firmware cost is +52–56 B, not ≈ +676 B.** T27.1's figure
  was a new `Cmd` variant painted by every painter; T80.1 adds a vtable
  slot beside one that became a provided forward, and a byte `Checksum`
  folds. The re-placement lives in `lumen-static-shell`, which the
  firmware does not link (`docs/results-embedded.md`).

- T80.2: **`RichText`'s mask had never been measured.** `style_reads.rs`
  listed 32 kinds and not `RichText`, so the gate that names it beside
  `Label` and `TextArea` could not run until it was added. Its first
  measurement disagreed with its seed on three fields besides
  `line_height`: it declared `font_weight`, which nothing reads (a span
  carries its own weight), and omitted `cursor`, `pressed_dim` and
  `hover_dim`, which the hit and `record_painted`'s shade make take
  effect. The seed now says what the output does (33 kinds, all equal).
- T80.2: **a fixed-width `Label` measures at its offer, not its width.**
  `Label::layout` hands the measurer `limits.max_w` (and ignores its own
  padding), so `width: Fixed(90)` inside a 200 px parent measured two
  lines and painted three. Not this task's mechanism and not fixed here:
  the line-height gate sizes the window to the label's width instead,
  and the showcase constrains the demo struct (`DLabel` 320 px), which a
  derived container offers to its children since M56. Owed to whichever
  task next touches `Label`'s layout (T80.3 measures the cut there).
  **Fixed by T80.3** (below).
- T80.2: **the live gate reads the height from `ui.resolve`'s bounds**,
  the same `NodeState` box `ui.getLayout` reports, and the agent's
  `ui.getAppliedStyles` now reports `line_height`, so the drive asserts
  the byte reached the node (24) beside the box it produced.
- T80.2: `MeasureKey`'s `flow` holds the line height only; the overflow
  byte is T80.3's, as the gate list says.

- T80.3: **K80b's fallback was needed, and most of the first build's cost
  was not the cut.** With the cut in every tier the paint-only firmware
  grew **+5,622 B**: `&text[..end]` indexing linked `str`'s slice-error
  path (its `Debug` escaping and the Unicode printable tables) for a panic
  that cannot happen. With every slice a `get` it was still **+2,024 B**,
  2,710 of K80b's 1,024 cumulative, so the leaf tier reads `TextOverflow`
  as `Wrap`: `Label`'s cut, its `fit` field and its seed's `OVERFLOW` bit
  are behind `alloc`. Measured after: **+224 B** (`firmware`), **+296 B**
  (`firmware_interactive`); M80a's total 910 of 1,024 and 1,150 of 1,536.
  The embedded `Ui` therefore stays 1,216 B (1,224 with the field), not
  the "+8 B, the firmware's included" Decisions planned.
- T80.3: **the carried T80.2 defect is fixed here**, because the cut needs
  it: a `Label` measures (and cuts) against the width it occupies, its
  `width` resolved under the offer less its padding, and keys its memo on
  that room so a padding or width restyle under the same offer
  re-measures. Its own gate is in `shell/tests/text_containment.rs`
  (`Fixed(90)` with 8 px padding offered 480 px: 4 lines, ink inside;
  measured against the offer, 1 line and 54 px of ink past the box). The
  T80.2 line-height gate's window-width workaround is no longer needed and
  its comment says so.
- T80.3: **K80c fired once, on that fix, and the fix was rewritten.**
  Calling `resolve_self` for the room on every layout cost `--bin
  bindings`' "layout, memo warm" arm 9.81 ns (median of eight) against the
  parent's 7.13–7.92; with the room left at the offer the same build read
  about 8.3 ns and the base 7.77, so about 1.1 ns was the call. Writing
  out the width half (`Fixed` clamps, everything else is the offer) reads
  7.88 against 7.33–13.49, and `grid` and `retain --text` were inside
  throughout (`docs/results-text.md` M80/T80.3).
- T80.3: **`AppBar`'s mask had never been measured either**, and
  `style_reads.rs` had no probe for it, so "the `Label` and `AppBar`
  probes' text lengthened" had one probe to lengthen. Added; its first
  measurement found the seed omitting `radius` (the bar's ground and
  shadow are rounded by it) and declaring `shadow` as unread only because
  the `shadow` probe's 6.0 equalled `AppBar`'s own default. The probe is
  7.0 now and the seed declares `radius`.
- T80.3: **the `Clip` gate cannot be the agent's.** The plan put it beside
  the S2 gate in `damage_shape.rs`, but the agent's journal signs nodes,
  not runs, so a label painting its whole text in a rect of its natural
  width under a clip still reported 1.00× there (the injection stayed
  green). `Recording` damages a run's whole rect whatever clip is pushed,
  so the gate asks the host's retained damage
  (`shell/tests/text_containment.rs`): 1.00× green, **6.00×** under the
  injection.
- T80.3: **the drive's `~` check names the paragraph's last word**, which
  the cut drops: a selector term cannot hold a space, so `~=` cannot name
  a paragraph, and the last word matches only if the whole text is still
  published.
- T80.3: `AppBar` reads `Wrap` as `Ellipsis` (a bar is one line) and its
  type default is `Ellipsis`, so the `overflow` probe is `Clip`, which
  differs from both kinds' defaults.

- T80.4: **the plan's tint failed its own contrast gate.** `Selectable`
  was to reuse `input::selection_tint` (the text colour at alpha 64). The
  showcase demo opens with its paragraph selected, so M57's D4 gate reads
  the text over the tint, and the dark palette measured **3.62:1**, under
  AA's 4.5. `Selectable`'s default is the same colour at alpha 40
  (`selectable::tint_of`): 8.66 light, 5.58 dark. A field's selected text
  sits on the alpha-64 tint unmeasured, because no demo opens with a
  field's selection; recorded in `docs/results-text.md` M80/T80.4, not
  changed here (it is `TextInput`'s and `TextArea`'s look, and the
  scrollbar track's).
- T80.4: **the Ctrl+X gate lives in `lumen-static-winit/tests/clipboard_shortcut.rs`**,
  not `widgets/tests/selectable.rs`: it asserts `keys::clipboard_decision`
  for the state the drag left, and the widgets crate cannot name the
  window's crate. Everything else in it is the plan's.
- T80.4: **the rows gate checks every row of eight lines, not only line
  2.** At `line_height` 0 the named injection (rows at
  `TextMeasure::line_height`, 18.6 px for the pinned face against the
  painter's ceil'd 19) drifts 0.4 px a row, so line 2 cannot see it; line
  5 is 2 px off. At 24 line 2 already reads 5 px. The bound is still 1 px,
  measured as line k's (ink top − tint top) against line 1's, because a
  glyph's first ink row is not its row's top.
- T80.4: **the widgets crate's shell dev-dependency takes `truetype`**, for
  `tests/selectable.rs` and `tests/selectable_idle.rs`; `--workspace` had
  unified it on for every test there already. The idle gate is its own
  binary (the text counters are process-wide) and **forces its idle frames
  to lay out**, with a zeroed box as the evidence: a pump with nothing
  marked skips layout and would read 0 whatever a layout did.
- T80.4: **the keyboard entry is two actions.** The demo opens selected
  (for the contrast gate), so Ctrl+A alone changed nothing the pass could
  see; Escape clears it and Ctrl+A selects all again, each moving the
  readout (the selected character count).
- T80.4: **adding a catalogue entry broke a showcase test that had nothing
  to do with it.** `every_sidebar_row_picks_its_own_entry` chose its
  scroll direction by `r < first` alone, so a row first in the window but
  cut by its top edge was scrolled *away from*, and the walk oscillated
  between two offsets 32 px apart. Latent while the list's reveal offset
  was a multiple of the step; one more entry moved it off. It scrolls up
  for a row materialized above the viewport now.

- T80.5: **`Local::caret_rect` is guarded by the host's guard, not by
  `caught`, and `Session::caret_rect` takes `&mut self`.** Decisions said
  `self.caught(None, ..)` "exactly as `copy` does", and that a panicking
  `caret_rect` "answers `None` and raises M36's screen". `caught` cannot do
  the second: it is a `&self` `catch_unwind` that answers its fallback and
  records nothing (`session.rs`), so a panicking caret would have been a
  silent `None` and the panic-app site could never report through
  `app.failure`. `guarded` runs `Host::guard`, which records the report and
  answers `None` without asking while a screen is up (so `app.imeArea` is
  `null` there with no check of its own). The window holds the session
  `&mut` already. The panic-drive injection is unchanged (the call outside
  the guard), and kills the window at site 8. M84's `Local::images`
  cites this decision's `caught` wording for the same claim
  (`docs/plan-images-in-motion.md`, second review A3), and has the same
  gap; it is that milestone's to settle, so it is reported, not edited
  here.
- T80.5: **a full `TextInput` painted no caret at its end.** The view kept
  the caret's *left edge* inside the padded box and the clip ended at that
  box, so a caret at the end of a full field was drawn one pixel wide
  wholly outside the clip. `shell/tests/ime_area.rs` found it on its first
  run: the scrolled case's "caret column" was a 12 px glyph stem (bound 70%
  of an 18.6 px line), and at scale 2 a shaper's unrounded end pen (580.14
  against a 580 width) still cut 0.78 px off it after the first fix. Fixed
  in `TextInput`: the clip runs `CARET_W` into the right padding, `place`
  reserves only what the padding cannot hold, and the scroll's end is the
  further of the run's width and the caret's x. Reserving the caret's
  width outright was tried first and broke `text_input_b15`'s end caret (a
  field sized to its text scrolled a pixel), which is why the clip moved
  instead.
- T80.5: **a read-only field took an input method's text, and a paste.**
  M57/B20 made a pick list's trigger read-only so it "declines text and
  deletion", and `TextInput::compose` and `TextInput::paste` never checked
  the flag: the catalogue gate's first run reported `textinput
  #chooser-trigger: composed and answered None`. Both refuse now. A menu's
  trigger (a menu has none, M57/B23, and lays its out with no box) still
  composes; the gate counts it as boxless and does not judge it, since
  nothing presses it or shows its caret.
- T80.5: **the live check is the harness's, after every action that
  leaves a text field focused**, not new inventory actions:
  `scripts/drive/harness.py`'s `_ime_check` runs for any focused node of
  the widget's subtree whose role is `TextInput`, `MultilineTextInput` or
  `SpinButton`, so `expected.json` does not change. Two of its own details
  were wrong on the first live run and are fixed in it: an empty field's
  padded box is the caret's width, so its commonest colour was the caret's
  (the ground is now read from the whole box), and a glyph stem beside a
  triple-clicked line's end caret pulled the centre 1.28 px (neighbours
  must be 80% as tall to join, in the headless gate too). `run.py` takes a
  comma list, so the four fields run in one window per leg.
- T80.5: **the window places the IME area after an agent request too.**
  An `input.*` request pumps a frame of its own and the window's redraw
  comes after the reply, so a drive's `app.imeArea` raced it. The area is
  placed before the reply is sent.
- T80.5: **panic-app's job-panic trigger moved from 8 to 10**
  (`panic_app::JOB_PANIC`), so site 8 is the numbered site the drive arms
  and the arm button's handler cannot spawn the spawner's panic instead.
  Site 8 is a focusable fixture leaf, `Fuse`, whose `caret_rect` panics
  once; its arm is a `Cell`, because `caret_rect` takes `&self` and an
  `update` between the arm and the focus would otherwise disarm it.
- T80.5 (follow-up): **a full-catalogue drive failed where the
  single-widget drive was green.** `just ci d4859b4` reported `FAIL
  PickList [Everything]: PickList/releasing over a row chooses it: node 172
  (TextInput) is focused and app.imeArea is null`. A pick list's trigger is
  a *read-only* `TextInput` (M57/B20), and T80.5 itself made a read-only
  field answer `caret_rect` = `None` -- so the window correctly placed no
  area, and `_ime_check` demanded one. T80.5's live drive named only the
  four editable fields, so no drive met a read-only trigger until the
  whole catalogue ran: AGENT.md's assembly lesson again, each piece right
  in isolation. Fixed in both directions rather than by skipping the node:
  a read-only `TextInput` publishes AccessKit's read-only state, the
  agent's node JSON carries `readOnly: true` (`observe::state_json`, only
  when set), and `_ime_check` asserts `app.imeArea` is **null** for a
  focused read-only node (a read-only field must not open a candidate
  window) and keeps the caret-column check for an editable one. The only
  catalogue widget embedding a read-only `TextInput` is `PickList`
  (`choose.rs`, `KIND == PICKLIST`); `docs/results-text.md` T80.5 has the
  drive's numbers and both injections.

**What the review found (M80).**

- C2 → adopted: the inventory is in Decisions; M80 takes both of
  `Style`'s spare bytes and says M82 grows it to 84 B.
- C3 → adopted for M80: `record_painted` copies `line_height` into
  `NodeState` (1 of 2 spare bytes); M82's and M86's halves are theirs.
- C4 → adopted: `Session::caret_rect` is required, the compiler is the
  gate, no stamp test; the runtime injection is `Hot` returning `None`
  in `just hot-window-drive`.
- C8 → changed: no `TextMeasure` change; the line count comes from the
  existing `shape` clusters through `LineCount`, and the injections are
  "`Label` measures with `measure_weighted`" and "the placer skips the
  re-placement".
- C9 → adopted: the helper is new and grapheme-based; the injection is
  "cut on chars, as `fit_title` does".
- C10 → adopted: `Selectable` is its own focusable entry sharing its
  label's index and answers `selection`; `caret_rect` is asked of the
  focused index and forwarded only by `NumberInput`.
- C20 → adopted: T80.0 corrects `docs/competitive-feature-gaps.md`
  and (second review B9/M17/R13; consistency-3 item 1) the winit
  manifest's three crate deltas.
- G1 → adopted, with `TextInput` (scroll) and `NumberInput` (forward)
  injections for T80.5's own mechanism.
- G3 → changed: the shared placer is gated measured-against-painted in
  one CPU test; parity keeps a `line_height` case because it can catch
  a backend that drops the byte, which is the only thing it can see.
- G4 → adopted (with C9); a byte cut would panic, so it is not used.
- G5 → adopted: `app.copy`, answered by `Win`, returning both the
  session's text and the system clipboard's.
- G6 → adopted: the reference is the caret column in pixels, and
  `app.imeArea` reports what `Win` passed.
- G27 → adopted: T80.0 commits only gates that compile today (one over
  existing `AppBar`, green, pinning today's gap; T80.3 flips it); no
  `#[ignore]`; every new-API gate lands in its task.
- G28 → adopted for M80: every gate above names an injection.
- G29 → adopted for M80: T80.2, T80.3, T80.4 and T80.5 each have a live
  drive under both strategies; T80.1 says why it has none.
- G32 → adopted: `scripts/firmware_delta.sh`, exact, per binary, with
  T80.4's bound at 0 B; since the second review (B5) the bounds are
  per binary (`BIN=B`), K4's 1,000 B on `firmware_interactive` is the
  default, and `print` never fails.
- G34 → adopted: painted ink box against the content box, and the
  number of truncating widths printed with a minimum.
- B2 → adopted: `max_lines` is not built; the cumulative result is
  stated (Style 80 after M80, 84 at M82).
- B3 → adopted, with `text_spaced` required and `text_weighted`
  provided so the 52 call sites stay; every implementor is listed in
  T80.1, `Checksum` included, and (second review A20) the wrappers'
  `text_weighted` overrides are deleted.
- B5 → adopted: `flow` in the key, `Ui` re-budgets attributed in T80.2
  and T80.3, `Label`'s +8 B stated, T80.4's size baseline named.
- B6 → adopted for M80: a printed firmware delta per task and K80b's
  cumulative bound; `Checksum` is in T80.1's list.
- B8 → adopted for M80: K80a can fire on a measured size, and `Cmd` is
  judged against its own 60 B assert, not RAM per node.
- A10 → partly rejected: `Style` has 2 spare bytes (78 in 80, counted
  field by field; `disabled` took one of `border_width`'s slot), not
  none. The `NodeState` half is adopted.
- A15 → adopted for M80 (T81.4's half is M81's).
- A17 → adopted: `NumberInput` is gated and injected in T80.5.
- A24 → adopted for `Selectable`: observe, role, inventory, catalogue.
- A30 → adopted: T80.1 lists the signature, the wrappers, `Recording`,
  `hash_into`, `wire.rs` and `svg.rs`, with the fixed-rect damage gate.
- T9 → adopted (with C4).
- T10 → changed: fontdue's multiplier is not used at all; the placer
  sets each line's position, so the pitch is exact and a single line
  grows, as in CSS and iced. Second review T1: below the face's
  spacing the box grows by `max(0, nls − pitch)` so ink stays in the
  rect `Recording` damages.
- T11 → (1) changed as C8; (2) adopted: `TextArea`'s rows use the pitch,
  with a caret gate; (3) and (4) adopted.
- T12 → adopted (with C2).
- T21 → adopted for M80: per-task printed deltas; `text_spaced` is
  accepted growth under D6 (owner, 2026-10-02), not D3, which the
  first resolution cited without the owner's say-so.
- T22 → adopted: new helper, memo on `Label` (+8 B), `char` fallback in
  the leaf tier.
- T23 → adopted (with G3).
- T24 → changed: no `TextSource` trait; `Selectable<S>` owns a
  `Label<S>`, because `Label` is the one widget whose text is one run
  in its padding box. `RichText`/`Markdown` selection is deferred, with
  its reopen condition in Decisions (not to M87, which has no selection
  work).
- M1 → adopted (with C2).
- M3 → adopted: no trailing docs task; each commit carries its docs.
- M4 → adopted for `Selectable`, in T80.4's commit.
- M8 → adopted (with C4).
- M9 → adopted (with C10).
- M11 → adopted as a cross-milestone requirement (below); M80's side is
  the rustdoc saying the cut is logical.
- M28 → adopted: T80.0 commits all twelve plan files, both allowlist
  blocks (`doc-identifiers.allow`, `doc-paths.allow`) and the queue
  entry (widened by the second review's B1/M18/R17).
- X8 → adopted: the corpus is built per cluster and per cut position,
  and prints how many cuts landed near each cluster; since the second
  review (M2) each cluster is pinned at its value measured at HEAD,
  the combining accent at 0.

**Cross-milestone (for the assembler).**

- M82 is the milestone that takes `Style` to 84 B; M80 leaves no
  padding, and `NodeState` one byte.
- M81 must add gates that `line_height` measures and paints the same
  pitch under `Shaping::Complex`, that a truncated RTL label shows `…`
  on its visual start (and a mixed-direction one is checked), and that
  `Selectable` selects visually spanned clusters over a shaped run.
  T81.5's caret reuses `Node::caret_rect`.
- `scripts/firmware_delta.sh` (T80.0) is the comparator every later
  milestone's K4 print uses, invoked positionally with per-binary
  `BIN=B` bounds (`just firmware-delta <sha> firmware=0
  firmware_interactive=256`) or `print`; the `base=<sha> bound=<B>`
  form cited elsewhere cannot work under `just`.
- M81's subpixel premise ("`Scaled::text_weighted` rounds the run
  origin") moves to `Scaled::text_spaced` at T80.1, which deletes the
  wrappers' `text_weighted` overrides.
- `deferring_forwards_every_command` counts 6 after T80.1; M82's and
  M83's stream counts start there, printed at landing.
- `Session::caret_rect` goes through `Local::caught`; M84's and M90's
  new session-seam entries that run application code need the same.

**What the second review found (M80).**

- C6 → adopted, with the verifier's correction: `line-height` needs
  the `PROPERTIES` row and the shell `set` arm only (the macro quote
  is generic); `text-overflow` also needs `lower::Val::TextOverflow`,
  its mapping and a macros quote arm. Decisions name the five sites;
  T80.2/T80.3 list them with an `lss_parity.rs` gate each.
- C18 → adopted: `parity.rs:282`, `clip_test.rs:262`, `svg.rs:78`
  added, plus "every full-field `Cmd::Text` by `rg` at landing, count
  printed".
- G1 → adopted: the golden claim is dropped; the placement gate's
  accept direction observes "treat 0 as 1.0×".
- G2 → adopted: the S2 gate moves to
  `crates/lumen-static-agent/tests/damage_shape.rs` and asserts on the
  journal's changed ids. T80.1's retention gate moves from
  `damage_regions.rs` to `retain_align.rs`, which drives `Recording`.
- G3 → adopted (with A5, M1, R5): T80.1 extends
  `deferring_forwards_every_command` 5 → 6 with empty-body and
  forwards-through-`text_weighted` injections, and the same shape for
  `Scaled` and `Recording`; M82/M83 counts renumbered (request).
- G4 → adopted: the injection is `Label`'s paint split marked
  `#[inline(never)]`.
- B1 → adopted (with M18, R17): T80.0 commits all twelve plan files and
  both allowlist blocks, each entry removed by the task that creates
  its target; accept is `just static` green.
- B5 → adopted: per-binary `BIN=B` bounds, default K4's
  `firmware_interactive=1000`, a `print` mode, and every invocation in
  this file written out in full.
- B10 → adopted: `check_text.sh:195`, `results-embedded.md:153`, and a
  dated four-binary table at `553fa5e` written by T80.0.
- B11 → adopted: `check_text.sh` asserts its default agrees with the
  budget row, with a selftest plant (`text-ceiling-row.sh`) and its
  accept run.
- B12 → adopted: bound at K4's 1,000 B on `firmware_interactive`, the
  +676 B paint-only figure printed as an expectation, and the K4 stop
  on firing.
- B16 → adopted: the recipe is positional (`firmware-delta base
  *bounds`); `base=<sha> bound=<B>` is gone from this file (other files
  by request).
- B17 → adopted: K80d's count runs alone in `tests/fit_cost.rs`, one
  `#[test]`, counters read around each cut.
- A3 → adopted for M80: `Local::caret_rect` goes through `caught(None,
  ..)`, with a `panic-app` site and an unguarded-call injection.
- A5 → adopted (with G3).
- A20 → adopted: T80.1 deletes the wrappers' `text_weighted` overrides
  and moves `Scaled`'s origin rounding into `text_spaced`; M81's
  premise re-pointed by request.
- T1 → changed: no clip and no `Recording` inflation (the core does not
  know `nls`); the box grows by `max(0, nls − pitch)` and the placer
  shifts by half of it, so ink stays in the rect at 1.0×. Gated at
  `line_height` 16 on ascenders and descenders, with ink-outside and
  undamaged-change counts at 0.
- T4 → adopted: `style_reads.rs` gains `line_height` (T80.2) and
  `overflow` (T80.3) probes, with a probe text that wraps.
- M1 → adopted (with G3).
- M2 → adopted: per-cluster pins at HEAD's measured value, the
  combining accent at 0 (U+0301 advance 0 in DejaVuSans), and T80.3's
  injection witnessed by the other clusters.
- M18 → adopted (with B1); the dangling "list below" is gone.
- R5 → adopted (with G3).
- R17 → adopted (with B1); T90.0's wording is M90's (request).
- R18 → adopted (with B10).
- X9 → changed: Ctrl+X on a `Selectable` degrades to copy because
  `Selectable` ignores `Backspace`, not by a `keys.rs` "editable"
  input, which would need a new required `Session` method and a `Node`
  slot; gated in `selectable.rs` with a handles-`Backspace` injection.
- D6 → applied: `text_spaced` is accepted growth under D6 (owner,
  2026-10-02), not D3; T80.5's `Node` slot is K4-bounded, not D6.
- D7 → applied: T80.4's clipboard drive uses `arboard`'s X11 backend,
  needs no `xclip`, and never reports skipped on the reference machine;
  `xclip`'s install step is M88's.
- B9/M17/R13 (from plan-iced-roadmap.md) → adopted: T80.0 rewrites
  `lumen-static-winit/Cargo.toml:91`'s a11y count 62 → 71 (95 → 166,
  `docs/results-a11y-platform.md`, K2's method); C20 updated.
  Superseded by consistency-3 item 1.
- M7, M3, R6 (from plan-iced-roadmap.md) → adopted in part: the `.lss`
  sites and the delete-the-`set`-arm injections were already in
  T80.2/T80.3 (C6); added the `roles.py` row for `Label` (T80.2) and
  the `roles.py` row and `keys.py` entry for `Selectable` (T80.4),
  neither present in `scripts/drive/` at HEAD. "(D3, printed and
  recorded)" was already gone; T80.5's slot now says it is reported to
  the owner with the number, no decision accepting it.
- G24 (from plan-windows.md) → adopted: from M89 `ime_area` lives on
  each `WindowSlot` and `app.imeArea` answers the request's `window`.
- Owner/M57 labels: "D1", "D3/D4" and "D7" in gates that meant M57's
  Phase D gates are now marked "M57's", since owner decisions share
  those names.
- consistency-3 item 1 → T80.0 re-measures all three manifest deltas
  (clipboard :47, dialogs :90/:115, a11y :91) with K2's exact command
  (96 default; +5 / +10 / +71 at `553fa5e`), records them in
  `docs/results-a11y-platform.md`, and rewrites all three comments.
