# M87 — Markdown you can style, link and illustrate

> **Written 2026-09-28 as a section of `docs/plan-iced-roadmap.md`;
> revised 2026-09-29 after that plan's seven-lens review, and moved to
> its own file. Not started.** The shared byte inventory, seams (S1,
> S2), test and doc discipline, and plan-wide kill rules K1–K4 are in
> `docs/plan-iced-roadmap.md` §2–§4 and bind this plan; this file does
> not restate them.

*Size M: eight commits, one widget file
(`lumen-static-widgets/src/rich.rs`)
plus its drive, showcase and bench, and one signing rule in
`crates/lumen-static-agent/src/journal.rs` (T87.2; the agent is not
linked into firmware). **No file under
`crates/lumen-static/src` changes**, so `Style`, `NodeState`, `Cmd`
(60 B) and the four firmware binaries' `.text` cannot move from their
values at landing (`docs/plan-iced-roadmap.md` §2.1 owns them); each
task prints `just firmware-delta <parent> firmware=0
firmware_interactive=0 firmware_bind=0 firmware_cascade=0` anyway (Δ 0
on all four; without the four bounds the default checks only
`firmware_interactive`). Closes: customizable Markdown
rendering and images; incremental parsing (decided by K87, either
way). Italic closes only if M81 lands a per-run face (T87.5). Depends
on: nothing to start. **T87.4 lands after M84/T84.3**: its demo names
the id the showcase's `Image` entry registers, and that entry (a
decoded GIF in an `ImageStore` handed to `Program::images`) is what
T84.3 adds; the showcase registers no image today. The rest of M87
does not wait: an image here changes by id, never in place
(Decisions), so M84's `frame` field is not on this path beyond
T87.4 passing 0 to M84/T84.2's signature. M84 makes
images *useful* (decoding); it does not make them *correct*.
Checked against `553fa5e`, whose `rich.rs`, `pass.rs` and `observe.rs`
are identical to `92a4033`'s.*

**Decisions.**

- **The span store grows; the 192-word ceiling goes.** Today
  `RichText` holds `[Span; MAX_SPANS]` plus two parallel arrays
  (`rich.rs:42`, `:74`), and `push` refuses silently at 192
  (`rich.rs:157-163`). A 64 KB document lays out its first 192 words,
  so any probe of it times a truncated parse. `rich.rs` is behind
  `alloc` (`lib.rs:128-129`) and the leaf firmware links none of it,
  so the three arrays become `Vec`s sized once per `set_str`/`new`,
  never per layout. Cost: `size_of::<RichText>()` falls from about
  7.7 KB inline (192 × 40 B of arrays) to three `Vec` headers; heap is
  40 B per word, 44 B once `Span` grows (below). `MAX_SPANS` is
  removed from the export list; `docs/plan-widget-parity.md:1179`
  still names it historically, so it moves to
  `scripts/doc-identifiers.allow`. Crate: `lumen-static-widgets`.
- **`set_str` walks once.** `line_start` (`rich.rs:576`) re-scans from
  byte 0 for every line, so the parse is lines × bytes. The line
  offset is carried instead. This is T45.10's quadratic again, found
  by reading, and T87.0 gate 2 is what shows it.
- **Per-element style is a value plus two style parts.**
  - `MarkdownTheme { h1, h2, h3, body, code, link, quote }`, each a
    `(scale, weight)`, is a plain `Copy` field `theme` on the widget
    (56 B per `Markdown`), not a `const`: a bind can set it.
    `MarkdownTheme::DEFAULT` is today's `heading()` values
    (1.5/1.25/1.1 at 700), so the default display list is unchanged,
    and T87.0 gate 5 pins that before anything moves.
  - **Colour goes through the cascade, not the widget.** A `Span` has
    no colour and paint draws every span in `s.fg` (`rich.rs:367-373`);
    `LayoutCx` and `Painter` carry no palette. So `Markdown`'s table
    gains two zero-size **part nodes** with their own seeds: kind
    `markdown-link`, role `Accent`, and kind `markdown-quote`, role
    `Muted`. `layout` calls `record_painted` on each with
    `Size::default()`, which writes the cascade's `color` into that
    part's `NodeState::fg` (`ops.rs:360-384`); paint reads
    `st[i + 1].fg` for a link and `st[i + 2].fg` for a quote. Palette
    switches re-resolve them through `Host::set_palette` (M25/T25.11)
    like any node, and a stylesheet can restyle them by kind
    (`markdown-link { color: … }`). Kind selectors, not author ids,
    so two `Markdown`s in one owner never raise `W1011`.
  - Cost: `NODE_COUNT` 1 → 3 for every `Markdown`, which is 2 ×
    (`Style` + `NodeState`) of style and state per `Markdown` (328 B at
    today's 80 + 84; printed at landing), plus two
    `NodeDesc`s in the const table. No per-node field grows anywhere
    else. `RichTextEditor`'s hand-written table (`rich.rs` editor
    `ARRAY`, `MAX_EDITOR_NODES = 3`) splices the preview's three nodes
    and becomes 5.
  - A zero-size part is hidden to a reader by `emit_node`'s rule
    (`observe.rs`, "a node with no area"), so M57's D7 sees nothing new.
  - **The parts make `Markdown` a container, and the journal stops
    signing its text.** Today it is a leaf (`rich.rs:205`,
    `NODE_COUNT = 1`), and the journal paints and hashes leaves only
    (`journal.rs:196-205`). A container is painted only when
    `owns_a_visible_part` finds a stand-in child with
    `size.w > 0 && size.h > 0` (`journal.rs:255-270`); otherwise it is
    signed from radius, shadow, bg and border (`journal.rs:237-249`),
    which a `set_str` or an image resolve never changes. These parts
    are zero-size by design, so without a change every text edit is a
    frame with no damage. T87.2 extends the rule in the agent crate:
    a live container **all of whose direct children are stand-ins**
    (`live` false) is signed by painting it, whatever their area. An
    enum never matches (its active variant is live), a hidden branch
    never matches (its root is not live), and the `Picker` trigger
    case is unchanged. `RichTextEditor`'s spliced preview becomes such
    a container and is covered by the same rule.
- **Blockquote joins the subset** (`> ` at line start), because the
  theme names `quote` and a role nothing draws is the ghost-button
  shape. Drawn as its words in the quote colour with a 2 px bar in the
  same colour, indented like a bullet.
- **Links are an ordinary activation handler, not a `Msg`.** Widget
  handlers are `fn(&mut S, &EventCtx)` (`event.rs:853-855`,
  `toggle.rs:66`), no widget names `App::Msg`, and the inbox carries
  results of work the application started (`docs/plan-tasks.md` §3).
  So:
  - `Markdown<S: 'static = (), const LINKS: bool = false>`, with
    `LinkedMarkdown<S> = Markdown<S, true>`. `Markdown` alone is
    today's type, and all 15 existing uses compile unchanged —
    provided `new` stays on `impl Markdown<(), false>`: a default
    type parameter does not drive inference in expression position, so
    `Markdown::new(DOC)` must name a concrete impl.
    `LinkedMarkdown::new(src, on_link)` lives on
    `impl<S> Markdown<S, true>`.
    `impl<S> Handlers<S> for Markdown<(), false>` answers `None` as
    today; `impl<S> Handlers<S> for Markdown<S, true>` answers
    `on_link: fn(&mut S, &EventCtx)` when `sel.inner` is `Some`. The
    two impls cannot overlap: the const differs.
  - **Which link is positional detail in `ctx`, rule 2.** `hit` over a
    link span returns `Hit { inner: Some(Inner { row: n, node: 0 }) }`,
    so the pass sets `ctx.row = Some(n)` (`pass.rs:1264-1290`), and the
    press latch already refuses a press on link 0 released on link 1
    (`press_is_within`). A click on plain text returns `inner: None`,
    the handler lookup answers `None`, and the press bubbles as today.
  - **The target is read from the application's own source.** A
    handler receives `A::State`; when the app named a `#[state]`
    field "the tree is not reachable from here at all" (`pass.rs`,
    the `state_mut()` comment). So `link_target(src, n) -> Option<&str>`
    is a pure function over the source the state already holds, and
    `Markdown::link(n)` answers the same for tests and `after_update`.
    One parser, so the two cannot disagree.
  - The pointer shape comes from `Node::cursor_at(local)` (M57/B48),
    `Pointer` over a link span. Keyboard: `LINKS` sets `FOCUSABLE` on
    node 0; Up/Down move an active link and `active_row()` answers it,
    so Enter activates through M44/T44.10's existing path. The active
    link is drawn with a 2 px underline, others 1 px.
  - A link is published as an extra child: `Role::Link`, id
    `instance_id(i, n, 0)` (`observe.rs:404`), the link text as its
    label, bounds of its first span, `Action::Click`. A platform click
    lands through `click_for_platform` at those bounds
    (`session.rs:807`). This is what makes `~=` able to address a link
    at all: today the whole document is one `Paragraph` value
    (`rich.rs:381-406`) and nothing is pushed to `extra`.
  - `Span` gains `link: u16` (`u16::MAX` for none) and `kind: u8`
    (text, quote, image): 20 B → 24 B (printed by T87.3's gate).
    A `links: Vec<Link>` holds each link's span range and target byte
    range.
- **Images are data the application binds, and change by id.**
  `![alt](name)` names an entry of `Markdown::set_images(&[ImageRef])`,
  `ImageRef { name: &'static str, id: ImageId, size: Size }`, bound
  with `with =` as `examples/form`'s `set_on` is. A resolved image is
  a block at its size, scaled down to the inner width with its aspect
  kept, painted with `Painter::image` (no new command). An unresolved
  one draws its alt words in the quote colour. Alt text is the
  `Role::Image` child's label.
  - **Why by id:** `Cmd::Image` hashes `(rect, id, tint)` only
    (`retain.rs:1016-1023`), so pixels replaced under an id already
    painted produce no damage and no frame (M84's P1). A Markdown image
    "loads" when its binding names the id the pixels were registered
    under; the command changes, and damage is the image rect. M84 adds
    `frame: u32` to `Cmd::Image` (and rejects a store-side
    generation). T87.4 owns `Markdown`'s one `image(..)` call and
    writes `Painter::image(rect, id, tint, 0)`, a still: M84/T84.2
    lands first (§5) and changes the signature, so the call is written
    against it.
- **Italic waits for a face selector that reaches paint.** `Cmd::Text`
  and `text_weighted` carry size and weight only (`display.rs:159`,
  `:807`), so a `FontStack` slot alone can never be chosen per span.
  M81 builds no glyph runs: its shaped run (`ShapedGlyph`) keeps
  `GlyphKey { face: u8, .. }` (D1c) inside the shell, and no milestone
  exposes a face to `Painter`, so T87.5 is expected to record. It is
  a probe that builds only if both an italic system face is found and
  a face reaches a widget's paint; otherwise the record in
  `rich.rs`'s header stands. **No new `Cmd` or `Painter` method in
  M87**, so no wrapper list and no `Deferring` injection is owed here.
- **Incremental parsing is decided by a number (K87).** The probe
  splits the keystroke into parse, measure and place, because the
  measure memo keys the *whole text* (`fold`, `rich.rs:103-122`), so an
  append re-measures every word through `FontStack`. Which part
  dominates decides what, if anything, is built (the M29/T29.5 lesson:
  a real number attributed to the nearest plausible mechanism).

**Tasks.**

- **T87.0 — gates that compile today.** One file,
  `crates/lumen-static-widgets/tests/markdown_m69.rs`, headless with
  `BlockMetrics` as `tests/rich.rs` is. Every gate is committed
  **green** (§3.1): gates 1–4 pin today's gap, asserting the current
  value and printing it, and the task that closes each flips its
  assertion to the bound below; gate 5 pins the default.
  1. `every_word_is_kept`: a 1,000-word document;
     `rich().spans().len()` printed beside the word count. **Pinned
     today at 192** (asserted `== 192`); T87.1 flips it to the bound:
     equal.
  2. `set_str_is_linear`: the minimum of five `set_str` timings at
     6.4 KB and 64 KB of 40-byte lines, ratio printed with the small
     size's self-ratio (bound 25, `textarea_scale.rs`'s: linear ≈ 10).
     Predicted above 25 from `line_start`, so it pins "ratio > 25" and
     T87.1 flips it; if the measured ratio is already ≤ 25, the commit
     records it, pins the bound as is, and T87.1's second change
     carries no gate claim.
  3. `link_markup_is_not_drawn`: `see [the docs](https://a.example) now`;
     the joined `span_str`s printed. **Pinned today**: they contain
     `](`; T87.3 flips it to the bound: no `[`, `](`, or target bytes.
  4. `image_markup_is_not_drawn`: the same for `![a chart](chart)`.
     **Pinned today**; T87.4 flips it.
  5. `default_theme_is_todays`: showcase's `DOC` (`catalog.rs:131`)
     laid out at 320 px and painted into a `DisplayList`; every `Cmd`
     folded with `DisplayList::hash_from(0)`, which reads the text
     bytes (`Cmd::hash_into` folds a run's length only,
     `display.rs:661-675`, so a same-length change would pass); the
     fold printed and pinned. Setup
     landed: at least 20 `Cmd::Text` (count printed). Injection: `## `
     scale 1.25 → 1.3 turns it red, naming both folds. Accept: the
     untouched tree is green.
  - **Pins watched red at T.0** (index §3.1, second review M14):
    each pin names a temporary perturbation, run and reverted before
    the commit, whose red run (printed value) goes in the commit body.
    Gate 1: `MAX_SPANS` 192 → 256 (`rich.rs:42`; red, 256 printed).
    Gate 2: carry the line offset instead of `line_start(&text, ln)`
    per line (`rich.rs:456`; red, ratio ≤ 25 printed; if gate 2 is
    pinned at the bound instead, the perturbation is the reverse: call
    `line_start` per word). Gates 3 and 4: skip pushing a word that
    contains `](` (red, joined text printed without it). Gate 5: its
    injection above.
  - Doc obligations: none; nothing public changes.
- **T87.1 — the span store grows, `set_str` walks once, and the arm.**
  `Vec` stores, the carried line offset, and
  `benches/src/bin/markdown.rs` (`--bin markdown`), modelled on
  `--bin textarea`: a one-character append to a document of a size
  given in KB, then `set_str` and a layout under `FontStack`, printing
  median and minimum per size and the three split quantities (parse,
  measure, place). It asserts, before timing, that the span count
  equals the word count (printed): the setup-did-not-happen check.
  - Same commit, K87c's arm (it must exist on T87.2's parent):
    `--bin cascade` gains a row `cascade_deep (markdown x16)` over a
    derived column of sixteen `Markdown` fields, one Kind rule per
    part kind, and asserts the tree's `NODE_COUNT` before timing
    (printed; 17 here, 49 after T87.2). `Ten10`, which the existing
    rows time, holds no `Markdown`, so no existing row can see a part.
  - Same commit: `rich.rs` header ("What it costs is a fixed
    ceiling") rewritten; `MAX_SPANS` out of `lib.rs:161` and into
    `scripts/doc-identifiers.allow`; `docs/results-markdown.md`
    created with the arm's first row. Both its `scripts/doc-paths.allow`
    entries (M80/T80.0), the results doc and `bin markdown`, are
    removed.
  - **Gates:** T87.0 gates 1 and 2 flipped to their bounds (values
    printed). New arm
    in `crates/lumen-static-shell/tests/alloc_gate.rs`: a tree with a
    `Markdown`, ten hover frames, allocations per frame printed (bound
    0). `size_of::<RichText>()` printed before and after.
  - **Injections:** restore the 192 refusal in `push` (gate 1 red,
    count printed); restore `line_start` per line (gate 2 red, ratio
    printed); `Vec::new()` for `placed` inside `layout` (alloc arm red
    at one allocation per frame, printed). Accept: each restored gate
    green in the same session.
- **T87.2 — `MarkdownTheme`, the two style parts, and blockquote.**
  - Same commit: rustdoc on `Markdown` and `MarkdownTheme`;
    `MarkdownTheme` removed from `scripts/doc-identifiers.allow` (its
    `MarkdownTheme  # proposed by M87` entry); `MarkdownTheme | n/a |
    data (a Markdown's per-element scale and weight)` added to
    `scripts/drive/undriven.allow`, the way `Span` is, because
    `check_drive_coverage.sh` counts every exported `pub use` name
    (`:20-26`); the journal's all-stand-ins signing rule
    (Decisions) and its comment in `journal.rs`;
    `results-markdown.md` rows for the part-node cost (328 B per
    `Markdown`, printed) and for K87c; a `.lss` example
    of `markdown-quote` in the rustdoc, which `check_lss_*` do not
    need because no setter is added. The showcase `Markdown` demo
    gains a quote line; `scripts/drive/expected.json` updated.
  - **Gates:**
    - T87.0 gate 5 still green: the default display list is unchanged.
    - A theme with `h1 = (2.0, 700.0)`: the heading span's line height
      over the body's printed (bound 2.0 ± 0.05 × `LINE`). Injection:
      `heading()` keeps its literals and ignores `theme` (ratio 1.5,
      red).
    - Palette: a headless `Host` with a quote; `set_palette(DARK)`;
      the quote span's `Cmd::Text` colour printed beside
      `Palette::DARK`'s `Muted` (bound: equal), and a body word's
      beside `Text`. Setup landed: `Palette::LIGHT`'s `Muted` differs
      from `DARK`'s (printed). Injection: paint a quote span with
      `st[i].fg` instead of the part's (red: prints the Text colour).
    - Table: `Markdown::NODE_COUNT == 3`, the parts' `size` is zero,
      and the agent tree publishes them hidden (count printed).
      Injection: give the parts the root's box (M57's D7 red: two nameless
      nodes).
    - Signing: a `Boundaries<256>` host with an agent; `set_str` on
      the `Markdown` to a same-length text; `ui.lastDamage`'s region
      set printed beside the node's box (bound: ratio 1.00, not
      `None`). Setup landed: `NODE_COUNT == 3` and both parts
      zero-size (printed). Injection: the journal's part rule
      unchanged, `owns_a_visible_part` only (red: damage `None`,
      printed). Accept: a hidden enum branch and the `Picker` trigger
      tests stay green.
    - K87c: `--bin cascade`'s `cascade_deep (markdown x16)` row, eight
      runs a side against T87.1's commit, spread printed. Arm
      sensitivity: the same row with four extra part nodes per
      `Markdown` must leave the parent's spread (printed); if even
      that stays inside, the arm is blind and is fixed before K87c
      is judged.
    - Live: `just widget-drive Markdown` under both strategies, the
      existing action plus "a quote is drawn in its own colour":
      `ui.contrast` over the demo (harness bound 3.0) in both
      palettes, the quote text named in the printed minimum, **and**
      the quote's colour itself: `ui.getAppliedStyles` on the
      `markdown-quote` part, resolved `color` printed beside the
      palette's `Muted` and `Text` (bound: equals `Muted`) in each
      palette. If the agent cannot address a hidden part, the quote
      line's ink is sampled from `ui.screenshot` instead (nearest of
      `Muted`/`Text`, both distances printed). Contrast alone cannot
      see a missing role: the fallback is the cascade's `Text`, which
      passes 3.0. Injection: seed `markdown-quote` with
      `SeedRoles::NONE` (red on the colour check: `Text` printed where
      `Muted` was expected; the contrast line may stay green).
    - `RichTextEditor`: `just widget-drive RichTextEditor` stays green
      with the spliced table (its two actions print as today).
- **T87.3 — links.**
  - Same commit: `LinkedMarkdown` and `link_target` exported; a drive
    inventory `LINKED_MARKDOWN` in `scripts/drive/widgets/rich.py`;
    `"LinkedMarkdown": "Paragraph"` in `scripts/drive/roles.py` (the
    link children are `Link`); a `KEYS` entry in
    `scripts/drive/keys.py` (Down, Enter); an `observe` impl, so no
    `check_observe.sh` line; a showcase demo `#d_mdlinks_a` whose
    handler writes `link: <target>` to the readout; rustdoc on hit,
    `cursor_at`, `active_row`; `book/src/gaps.md`'s Markdown line
    corrected.
  - **Gates** (headless, through the pass: `Host::click` then
    `Host::pump`):
    - A click at a link span's centre (from `placed`, printed):
      `pump` returns `handled == true`, the state's `last_link ==
      Some(1)`, and `link_target(DOC, 1)` printed beside the expected
      URL. The accept-and-refuse pair: a click 4 px past the link's
      last span returns `handled == false` and the state is unchanged.
      Injections: `hit` returns `inner: None` (red: `handled` false);
      `hit` returns the previous link's index (red: `Some(0)` printed).
    - Press on link 0, release on link 1: nothing runs (count 0).
      Injection: `hit` reports row 0 for every link (red: count 1).
    - Cursor: `Host::cursor()` after a hover over the link is
      `Pointer`, and 4 px past it `Default`. Injection: `cursor_at`
      answers `None` (red). **Not live:** the agent has no cursor
      method (`ui.*` list in `lumen-static-agent/src/lib.rs`), so the
      headless `Host::cursor` is the gate, as it was for M44/T44.8.
    - Palette: the link span's colour equals `DARK`'s `Accent` after
      `set_palette(DARK)`. Injection: paint with `st[i].fg` (red).
    - `size_of::<Span>()` printed (bound 24).
    - Live, `just widget-drive LinkedMarkdown`, both strategies:
      click the link by its text with `~=`; the readout reads
      `link: https://…` (`visible=True`, region `#readout`); damage
      ratio printed by the harness. `isolate`: a click on a plain word
      leaves the readout unchanged. M57's D3 keyboard pass: Tab reaches
      `#d_mdlinks_a`, Down then Enter changes the readout. Injection:
      push no link children (red: `~=` matches no node, printed by the
      harness).
- **T87.4 — images.**
  - Same commit: `ImageRef`, `set_images` exported and documented;
    `ImageRef` added to `scripts/drive/undriven.allow` as data, the way
    `Span` is (line 47); a `Role::Image` child per image;
    `Markdown`'s paint calls `Painter::image(rect, id, tint, 0)` with
    M84/T84.2's frame argument (Decisions; M84 R20); the
    `#d_mdlinks_a` demo gains an image line naming the id the `Image`
    demo already registers; `results-markdown.md` row.
  - **Gates:**
    - An image span with `size = 120 × 80` in a 320 px box: its
      placed rect printed (bound ±0.5 px), and the next word's `y`
      printed beside the image's bottom (bound ≥). Injection: lay the
      image out as an inline word (red: the word's `y` is above).
    - In a 60 px box: width 60 − padding and height by the aspect
      (printed, ±0.5 px). Injection: clamp width only (red: aspect).
    - **Loading is the image rect.** `Boundaries<256>` host; the ref
      names an id the `ImageSource` lacks (draws nothing), then is
      rebound to a registered one of the same size. `ui.lastDamage`'s
      region set printed beside the image rect (bound: equal, ratio
      1.00). Setup landed: the second frame's list holds one
      `Cmd::Image` with the new id (printed). Injection: resolve refs
      once in `set_str` and never in `set_images` (red: damage `None`,
      printed). Accept: an unresolved → resolved change with a new
      size damages from the image's top to the node's bottom (printed,
      bound ≤ the node's rect).
    - Live, `just widget-drive LinkedMarkdown` gains "the image is
      drawn where its markup is" (`visible=True`, region: the image
      child's bounds; `ui.regionHash` differs from the page fill) and
      its alt text is its child's label. Injection: skip the
      `Painter::image` call (red: region hash equals the fill).
- **T87.5 — the italic probe, then the build or the record.**
  Probe: does the pinned fallback list find an italic face for the
  body face (face name printed), and does M81 give a widget's paint a
  per-run face (name the API, or "absent")? Both → build: `*x*` sets
  a `Span` italic bit painted through that face; gate: the italic
  run's `ui.regionHash` differs from the same word roman, and
  `tests/svg.rs` names the face; injection: ignore the bit at paint
  (red: hashes equal). Either absent → record in
  `results-markdown.md` with the probe's output; the `rich.rs` header
  and `book/src/gaps.md` stay, with the reopen condition "M81 exposes
  a per-run face to `Painter`".
- **T87.6 — the incremental-parse probe (K87), then the build or the
  record.** Eight runs a side of `--bin markdown -- 64` under
  `just bench-lock`, printed beside K87's bound. Under: the row is
  the verdict, "decided against" with the number. Over: build the
  dominant part only (block-level re-parse from the last complete
  block, or width reuse keyed by word content, not whole text), and
  re-run. Gate for the build: `markdown_m69.rs` gains
  `append_matches_full_parse` (spans after an incremental append equal
  a full parse's, byte for byte, over the 64 KB fixture plus ten
  edits; count of compared spans printed). Injection: re-parse from
  the append's own line, not the last block boundary (red on the
  fixture's edit inside a bullet run).
- **T87.7 — close.** `results-markdown.md` summary; the AGENT.md queue
  entry cites `just ci-verify <sha>` green (S5).

Appendix rows for the assembler:

| task | gate | injection |
|---|---|---|
| T87.0 | 1,000 spans; ratio ≤ 25; no link/image markup; pinned fold | `## ` scale 1.3 |
| T87.1 | spans = words; ratio ≤ 25; 0 allocs/frame; K87c arm's `NODE_COUNT` | 192 refusal; per-line `line_start`; `Vec::new` in layout |
| T87.2 | fold unchanged; h1 ratio; quote = `DARK` `Muted`; parts hidden; `set_str` damage 1.00; K87c row | ignore `theme`; root's `fg`; parts with a box; part rule unchanged; four extra parts; `SeedRoles::NONE` |
| T87.3 | `handled` + `Some(1)`; latch; `Pointer`; `~=` drive | `inner: None`; wrong index; `cursor_at` `None`; no children |
| T87.4 | at size; flows below; damage = image rect | inline layout; resolve once; skip `image` |
| T87.5 | italic hash differs (build only) | ignore the bit |
| T87.6 | incremental = full parse (build only) | re-parse from the append's line |

**Kill rules.**

- **K87 — incremental parsing.** `--bin markdown -- 64`: median of
  eight runs of parse + measure + place for a one-character append to
  64 KB, printed beside **1.5 ms**: half of T8b.3's 3 ms input →
  presented p95 budget (`docs/results-frame-cost.md`, M8b), the other
  half being about the 1.42 ms p95 the tally already spends there.
  Under: decided against, with the number. Over: T87.6 builds the
  dominant split quantity only. The arm asserts spans = words first,
  or it does not print a verdict.
- **K87a — the store must not allocate on a frame.** The T87.1
  `alloc_gate.rs` arm above 0 allocations per still frame: `Vec`
  stores are withdrawn, `MAX_SPANS` is raised to fit the 64 KB fixture
  measured in words, `push`'s refusal becomes a `debug_assert`, and
  K87's fixture is re-sized to what fits, recorded.
- **K87b — a link as `Inner`.** If reporting a link index as
  `Hit::inner` on a non-virtualized node turns any existing test or
  drive red (per-row addressing `%k`, `hover_inner`, the press latch),
  T87.3 stops and links use the take-once shape `Menu` uses
  (`Chooser::take_chosen`, M57/B23), read in `after_update`, recorded
  with the red test's name.
- **K87c — parts cost.** If the two part nodes move `--bin cascade`'s
  `cascade_deep (markdown x16)` row (added by T87.1, measured by
  T87.2) outside the parent's spread (eight runs a side), colours
  fall back to fixed `Color` fields on the theme that do not follow
  the palette, stated as such in the rustdoc, and M57's D4
  dark-palette result is recorded.
- K1–K3 apply as stated in §4; with no core change, each task prints
  `just firmware-delta <parent> firmware=0 firmware_interactive=0
  firmware_bind=0 firmware_cascade=0` (`scripts/firmware_delta.sh`,
  added in T80.0): bound Δ 0 on all four binaries, printed
  (`docs/plan-iced-roadmap.md` §2.1, second review B4), instead of
  re-measuring K3's arms. `just firmware-size` prints only the
  paint-only `firmware`, so it cannot bound the other three.

**What building found (M87).**

- **T87.0 — gate 5's injection could not fire on `DOC`.** The showcase's
  `DOC` (`catalog.rs:131`) has one `# ` heading and no `## ` or `### `,
  so the plan's injection (`## ` scale 1.25 → 1.3) left a `DOC`-only fold
  green: the gate pinned two of the three heading levels `MarkdownTheme`
  will own by name and could not see the third change. The gate now
  folds `DOC` **and** `EVERY` (`# `, `## `, `### `, body, `**bold**`,
  `` `code` ``, a bullet: every element the theme names that today's
  parser has), each pinned; the same injection turns `EVERY`'s fold red
  and leaves `DOC`'s where it was. Rule 2 of AGENT.md's "Tests must be
  shown to fail": the injection was right, the fixture could not exhibit
  it.
- **T87.0 — gate 2 measured 33–51 under a debug `cargo test` and 51 in
  release** (pinned "> 25"); the carried offset takes it to 6.9 in
  debug, so it is pinned above the bound and T87.1's second change does
  carry a gate claim.

- **T87.1 — `push` no longer answers a `bool`.** It answered whether the
  193rd word was refused; with nothing to refuse it returns `()`, and its
  only callers were `rich.rs`'s own parsers. **`probe` stopped copying
  the stores**: it copied three inline arrays to restore them, which a
  `Vec` would allocate for, so `layout` and `probe` share a private
  `lay(.., keep)` and the probe never writes `placed`.
- **T87.1 — how `--bin markdown` splits the append.** *Parse* is
  `set_str`; *place* is a second layout of the same text with the memo
  warm (the breaker plus the whole-text fold that finds it warm);
  *measure* is the first layout less that. The first single run puts the
  64 KB append at 3.10 ms, 2.73 of it measure
  (`docs/results-markdown.md` T87.1), above K87's 1.5 ms; K87 is judged
  by T87.6's eight runs under `just bench-lock`, not by this row.

- **T87.2 — K87c fired, and the parts are withdrawn.** Built as the
  Decisions say — two zero-size parts, `record_painted` writing each
  one's resolved colour into its `fg`, the journal's all-stand-ins rule,
  and the plan's palette, table and signing gates, each green and each
  watched red — the `cascade_deep (markdown x16)` row went from
  274–294 ns to 943–1,005 ns, eight runs a side (3.4x;
  `docs/results-markdown.md` K87c). A cascade costs per node and the parts
  tripled a `Markdown`'s nodes, so no part node could have stayed inside
  the parent's spread. K87c's response is taken: a `Markdown` is one node
  again, the editor's table is unchanged, and the journal gets no new
  rule (a `Markdown` is still a leaf, which the journal paints and hashes
  already). The table, signing and `getAppliedStyles` gates have nothing
  left to check and are not committed; the `.lss` example of
  `markdown-quote` goes with the kind.
- **T87.2 — the fallback's colours are an override, with a derived
  default.** K87c says "fixed `Color` fields on the theme that do not
  follow the palette". A fixed *default* cannot pass M57's D4 gate
  (`every_demo_text_reads_in_both_palettes`, AA 4.5:1) in both of the
  showcase's palettes: against its white surface a colour needs relative
  luminance ≤ 0.183 and against its `#1c212b` surface ≥ 0.245. So
  `quote_color` and `link_color` are `Option<Color>`: `Some` is the rule's
  fixed colour, stated in the rustdoc as not following the palette, and
  `None` (the default) derives from the node's own colour, which does —
  a quote at 70% alpha, a link mixed halfway toward a blue. That needs no
  node, so K87c's cost stays at zero. **T87.3's palette gate changes with
  it**: the link is checked against `MarkdownTheme::link_ink` of the
  palette's `Text` under both palettes, not against `DARK`'s `Accent`.
- **T87.2 — the live colour check reads the contrast minimum, not a
  part's applied style.** With no part, `ui.getAppliedStyles` has nothing
  to address. The drive asserts instead that the demo's least contrasting
  run is a quote word, at AA, in both palettes, and that its colour differs
  between them — which a quote drawn in the body colour fails, because the
  minimum is then the first run on the stage.
- **T87.2 — `ui.contrast` was blind at scale 2.** The live quote check
  asked for the least contrasting run and got none on the scale-2 leg:
  the session filtered the physical display list by a selector's logical
  bounds. Fixed in `lumen-static-shell`'s session (the rect is scaled),
  with `tests/contrast_scale.rs`; the harness's after-every-action
  contrast check had been passing on "no text in the widget's box" for
  every widget on that leg.
- **T87.2 — `Span` gained `kind` here, not in T87.3**: a quote word is
  `Span::QUOTE`, so `Span` is 24 B already; T87.3's `link: u16` fits in
  its padding (printed by T87.3's gate). A quote line's first word sets
  `bullet` for the indent and hang, and draws the bar instead of the dot.
- **T87.2 — the theme is set by `set_theme`, not written as a field.** A
  span's scale and weight are fixed at parse time, so a `pub theme` that
  only took effect at the next `set_str` would be a write that did
  nothing visible; `set_theme` re-parses when the theme changed.

- **T87.3 — the linked constructor is `linked`, not a second `new`.**
  The Decisions put `Markdown::new` on `impl Markdown<(), false>` and
  `LinkedMarkdown::new(src, on_link)` on `impl<S> Markdown<S, true>`; with
  both, every `Markdown::new(DOC)` is E0034 ("multiple applicable items"),
  because the path in expression position carries inference variables
  for both parameters and each impl could apply. Checked with a
  standalone file before anything was written. So it is
  `LinkedMarkdown::linked(src, on_link)`, and `Markdown::new` keeps its
  fifteen callers unchanged.
- **T87.3 — a plain `Markdown` parses links too, and does nothing with
  them.** Gate 3 is a plain `Markdown`, and markup that is drawn is the
  defect it pins, so the parser strips link markup for both forms and
  draws link words in the link colour, underlined. Only `LinkedMarkdown`
  reports a link as `Hit::inner`, sets `FOCUSABLE`, walks links with the
  arrows, shows `Pointer` over one and publishes `Link` children — a
  plain one publishing a `Click` that runs nothing would be the
  finished-but-unreachable shape.
- **T87.3 — the palette gate is `link_ink`, not `DARK`'s `Accent`**, by
  T87.2's K87c fallback: the link is the palette's `Text` mixed halfway
  toward `LINK_TINT`, checked under both palettes beside the body.
- **T87.3 — K87b not reached.** Reporting a link as `Hit::inner` on a
  node that is not virtualized turned nothing red: `just test` and the
  three-widget drive (`LinkedMarkdown`, `Markdown`, `RichTextEditor`)
  green on all six legs (`docs/results-markdown.md` T87.3).
- **T87.3 — the live action's `visible` region is the window.** Its
  consequence is the readout, whose chip is empty — no area to hash —
  before the first click.

**What the review found (M87).**

- C17 → adopted: the store grows (T87.1), the arm asserts spans =
  words before timing, K87's bound is 1.5 ms with its source.
- C27 → adopted: `on_link: fn(&mut S, &EventCtx)`, the index in
  `ctx.row` via `Hit::inner`.
- G22 → adopted: each link is a `Role::Link` extra child with bounds;
  the drive asserts `handled` and the readout; injection: no children.
- G29 → adopted for M87: `LinkedMarkdown` gets an inventory, roles and
  keys entry, observe impl, and showcase demo in T87.3; `ImageRef` is
  allowed as data in T87.4.
- B15 → adopted: `--bin markdown`, eight runs a side, numeric bound;
  "layout budget" dropped.
- A2 → adopted: no `Msg`; handler shape; the target is read by
  `link_target` over the application's own source.
- P1 → changed: the fix belongs to M84. M87 avoids it by rule: a
  Markdown image changes by id, and T87.4's gate uses that path. T87.4
  still lands after M84/T84.3, whose showcase `Image` entry registers
  the id its demo names.
- P12 → changed: handler shape adopted, but `markdown.link_at(ctx)`
  is not callable from a handler (the tree is out of reach under
  M33, `pass.rs`); replaced by the pure `link_target(src, n)`.
- P13 → adopted: capacity decided before the probe (growable under
  `alloc`); link and image spans share it with no cap.
- P14 → changed: no `Cmd`/`Painter` change in M87; italic is T87.5's
  probe. `GlyphKey::face` stays inside the shell's paint path (M81), so
  no widget can choose a face and no milestone provides a selector
  that reaches paint; the probe records that unless one appears.
- M3 → adopted: every task carries its docs; T87.7 is the results
  summary and ci-verify SHA only.
- M19 → adopted: same as B15.
- M20 → adopted: T87.1–T87.6 each name mechanism injections; appendix
  rows supplied.
- X7 → adopted, first option refined: no per-node colour table in
  `NodeState`; two zero-size part nodes whose own seeds carry roles, so
  the cascade and `set_palette` resolve them with no new field. The
  second review (C4) found the parts make `Markdown` a container the
  journal would no longer sign by its text; T87.2 adds an agent-side
  signing rule and a damage gate, and K87c gets an arm that holds
  parts (X4).

**What the second review found (M87).**

- C4 → changed: kept the parts and extended the journal (agent crate,
  not core): a live container whose direct children are all stand-ins
  is signed by painting it. T87.2 gate: same-length `set_str` gives
  `ui.lastDamage` 1.00 of the box; injection: the part rule unchanged.
- C20 → adopted in part: `MarkdownTheme`'s allowlist entry is cited
  by its text, not line 136 (it is at 191). `session.rs:807` kept: it
  is a real `click_for_platform` call site (the verifier's partial
  refutation).
- G28 → adopted: the live action asserts the quote's resolved colour
  equals `Muted` in each palette (`ui.getAppliedStyles`, or sampled
  ink), printed beside `Text`; the `SeedRoles::NONE` injection is red
  on that check, not on contrast.
- G29 → adopted: T87.0 gate 5 folds `DisplayList::hash_from(0)`,
  which reads text bytes; `Cmd::hash_into` folds lengths only.
- M29 → adopted: T87.2 adds `MarkdownTheme` to
  `scripts/drive/undriven.allow` as data and cites the identifier
  allowlist entry by name.
- X4 → adopted: T87.1 adds `cascade_deep (markdown x16)` to
  `--bin cascade` with a `NODE_COUNT` setup assertion; T87.2 measures
  it against its parent, eight runs a side, with a four-extra-parts
  sensitivity injection; K87c names that row.
- D6/D7/X5 → no change: M87 adds no `Painter` method, needs no
  clipboard tool, and does not touch shaping or `deny.toml`.
- B4, M14 (from `plan-iced-roadmap.md`) → adopted: the per-task
  firmware bound is `scripts/firmware_delta.sh <parent>`, Δ 0 on all
  four binaries, not `just firmware-size`'s one; T87.0 gates 1–4 each
  name a temporary perturbation watched red at T.0, recorded in the
  commit body (index §3.1).
- R20 (from `plan-images-in-motion.md`) → adopted: T87.4 owns
  `Markdown`'s `image(..)` call, written against M84/T84.2's
  signature with frame 0 (Decisions, T87.4's Changes); the old "in
  M84's consumer list, not an M87 task" claim is withdrawn.
- consistency-3 item 8 → the header and §4's per-task firmware line
  are `just firmware-delta <parent>` with all four bounds 0.
- consistency-3 item 18 → M81 builds no glyph runs: `ShapedGlyph`
  keeps `GlyphKey` in the shell and no milestone exposes a face to
  `Painter`, so T87.5 is expected to record.
- consistency-3 item 19 → M57's gates read "M57's D3/D4/D7" (four
  sites).
- consistency-3 item 24 → T87.1 removes both `doc-paths.allow`
  entries (`docs/results-markdown.md`, `bin markdown`).
