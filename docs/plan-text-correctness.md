# M45 — text that measures, paints and edits as one thing

> **T45.12 DONE 2026-09-26 (early landing).** `BakedFace` moves an
> overflowing word whole to the next line; `"aaa bbbb"` at 60 is two
> lines and 41 wide (was: a break at byte 6, 55 wide), 0 of 25 phrase ×
> width cases start mid-word (was 27 failures). `.text` +308 B
> (`docs/results-embedded.md`).
>
> **Written 2026-09-24.** Closes R10-F1, R10-F2, R10-F5,
> R10-F6, R10-F7, R10-F8, R10-F10, R10-F11, R10-F12, R10-F13, R10-F14,
> R10-F15, R16-F7, R2-F6, R2-F10, R8-F3, R8-F4 and R8-F5 from M38's
> deep review (`docs/results-deep-review.md`), as re-triaged against
> `main` at `1717808` on 2026-09-24. Where the review and the re-triage
> differ, the re-triage is what this plan follows.
>
> **Phase 3** of the fix queue, and **after M41** specifically. M41 owns
> three things this plan builds on and does not repeat: `Host::paste_with`,
> which makes a paste lay out with the window's real measurer (R10-F4,
> R2-F1; M41/T41.5); marking the focused node when `Host::compose`
> changes it (R10-F9; M41/T41.1); and an override of `Node::probe` in
> `TextInput` and `TextArea`, so the audit's probe leaves no trace (R1-F5;
> M41/T41.4). **Two things M41 hands to this plan instead** (M41 §4):
> dropping `limits.max_w` from `TextInput`'s shaped key, which is T45.2;
> and measurer identity in the memo keys. No plan in M39–M55 adds an
> `input.compose` agent method. It also runs after M42, for any
> live drive leg, and one task waits for M44's arrow fix (T45.6). T45.12
> may run earlier, and says why.
>
> **What the review found is one sentence repeated eighteen ways:** the
> text stack's parts each work, and they disagree with each other. The
> measurer and the painter disagree about a box by the last glyph's
> bearing. The memo and the display disagree about the scale. The shaper
> and the painter disagree about which glyphs Arabic is drawn with. The
> shell's paste and the widget disagree about what a newline is. Every
> part has a test, and none of the tests assembles two of them.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and prints the measured quantity beside its bound.*

---

## 0. The rules for this milestone

1. **Measurer and painter are checked together.** A text gate measures
   with the measurer a window uses (`FontMetrics`) and paints with the
   painter a window uses (`Canvas::with_font`), through a `Host`. A test
   that measures and paints with two stand-ins proves the stand-ins
   agree.
2. **Every text-writing path is checked under the default invalidation.**
   A paste, a composition and a key-text commit all change a widget
   outside an ordinary keystroke. Under `Boundaries<256>` a change that
   is not marked is not drawn. Each such gate runs through `Host<A>` and
   `Host<A, _, Everything>` and requires the same frame.
3. **A gate that needs a system face says so when it has none.** At
   `1717808`, `crates/lumen-static-text/tests/shaping.rs` returns
   silently when no face loads (`let Some(mut t) = face() else { return };`),
   which is a pass that proved nothing. This plan follows M43's policy
   for it (M43 §2.2 and T43.11): a gate that only needs *a* face shapes
   M43's committed test face, and a gate that needs a *system* face
   (the fallback faces of T45.3) prints its skip reason, and fails
   under the fonts requirement that `dev-shape` sets. The plan's own
   machine has the faces, so a skip there is itself a failure the
   commit must explain.
4. **Cost changes are attributed before they are fixed.** M29/T29.5's
   lesson: a real number blamed on the nearest plausible mechanism.
   T45.10 and T45.11 each measure their own mechanism separately.

## 1. What is wrong, at 1717808

### 1.1 The measured box is narrower than what the painter draws (R10-F1, R10-F2, R10-F11)

- **Pen, not ink origin (R10-F1).** `FontStack` computes a run's width
  as `max(g.x + max(advance, ink width))` in three places:
  `place_run`, `measure_run` and `shape_into`
  (`crates/lumen-static-shell/src/text.rs:182`, `:276`, `:336`).
  fontdue's `g.x` already includes the glyph's left bearing, so a run
  ending in a negative-bearing glyph (`j`, `J`) is measured short, and
  the painter, which wraps inside the measured box, moves the last glyph
  to a second line below the box (R10-F1). A second, smaller mechanism:
  a logical width times a fractional scale rounds down in f32 and wraps
  the same way. The comment at `text.rs:176-181` says this re-wrap was
  fixed; it was fixed for positive bearings. The GPU backend calls the
  same `place_run` with the rect width
  (`crates/lumen-static-wgpu/src/lib.rs:1009`).
- **Shaping measures and nothing paints it (R10-F2).**
  `lumen-static-text`'s `ComplexText` shapes, orders and reports
  clusters. The painter then draws the original string with fontdue:
  cmap glyphs in logical order. So Arabic is painted unjoined and left
  to right, a bidi line is painted in logical order, and because
  `ComplexText`'s box is narrower than fontdue's own width for *every*
  string (the verifier's correction: plain ASCII wraps too), the painter
  wraps inside it (R10-F2). `TextInput::x_of`
  (`crates/lumen-static-widgets/src/input.rs:371`) assumes cluster
  bytes rise with x, which a right-to-left run breaks. And no window
  can install `ComplexText` at all: the winit `Session` builds a
  `FontMetrics` and `Program` has no measurer parameter. Yet
  `docs/plan-capabilities.md` T15.8 is marked DONE on the gate "an
  Arabic string measures and paints right-to-left", and the crate's
  rustdoc says it "changes *where the glyphs go*"
  (`crates/lumen-static-text/src/lib.rs:25-26`).
- **The test named for this cannot see it (R10-F11).**
  `shaping.rs`'s `a_right_to_left_run_is_laid_out_and_stays_on_boundaries`
  (`crates/lumen-static-text/tests/shaping.rs:75-87`) asserts a positive
  width, more than one cluster, and offsets on character boundaries.
  `BlockMetrics` satisfies all three, and the review's injection that
  shapes RTL runs left to right left it green (R10-F11).

### 1.2 The memo keys omit what the box depends on (R2-F6, R2-F10, R10-F12)

- **Scale (R2-F6).** `TextInput`'s shaped key is
  `(text length, font size, weight, limits.max_w)`
  (`input.rs:474-479`) and `TextArea`'s is the same with the inner width
  (`crates/lumen-static-widgets/src/textarea.rs:463-468`). Neither
  carries the display scale, so M26/T26.17's monitor-drag defect,
  fixed for `Label` through `MeasureKey`, survives in the two editable
  widgets (R2-F6). The caret positions come from the cached clusters,
  so after a drag the caret is placed from the old scale's shaping.
- **NaN limits (R2-F10).** `MeasureKey` stores its `Constraints` by
  value (`crates/lumen-static/src/text.rs:110-111`), compared with f32
  `PartialEq`, so a NaN limit never hits. The rustdoc's "bit patterns,
  not values" (`:128`) covers the other fields. Nothing in the tree
  produces NaN limits; it is a memo miss, not a stale box.
- **The measurer's identity.** No key carries which measurer produced
  it. M41 fixes the paste that exposed this (R2-F1) and names the key
  half as this milestone's (R2-F1's "defence in depth").
- **The font stack is one face (R10-F12).** `FontStack::system` takes
  the first candidate pair that loads (`text.rs:33`, `:70-71`), and
  `prepare` substitutes `?` for every character that face lacks
  (`:90-132`). CJK, Arabic, Hebrew and emoji render as `?` even where
  installed faces cover them, and once M41 fixes R10-F9 this is what a
  CJK IME user will see (R10-F12). A fallback face changes measurement
  for text it covers, so the stack's contents are part of the key too.

### 1.3 Paste is per-character, loses newlines and is quadratic (R8-F4, R8-F5, R10-F5, R10-F6)

`Host::paste` (`crates/lumen-static-shell/src/lib.rs:1422-1433`) posts
one `Event::Text` per character, keeping `'\n'` in its filter. A
`TextArea` accepts text only `if !ch.is_control()`
(`textarea.rs:566`) and takes a newline only as `Key::Enter`, so every
pasted newline is dropped while `paste` counts it as delivered (R8-F4,
R10-F5; two reviewers found it). It pumps once per 64 characters, and
each pump is linear in the text length, so a large paste is quadratic
and blocks the UI (R8-F5; the verifier split the attribution and a
single edit costs a small fraction of it). The window now reaches this
through `Session::paste` (`crates/lumen-static-shell/src/session.rs:294-297`),
which discards the count. The test named for newlines,
`paste_skips_control_characters_but_keeps_newlines`
(`crates/lumen-static-shell/tests/clipboard.rs:99-107`), pastes no
newline into a single-line field (R10-F6).

### 1.4 Editing steps by the wrong unit (R10-F7, R10-F14, R10-F8)

- **Codepoints, not graphemes (R10-F7).** `prev_boundary` and
  `next_boundary` use `char_indices` in both editors
  (`input.rs:341-349`, `textarea.rs:266-273`). Backspace after a
  combining accent, a ZWJ sequence or a flag leaves half a character,
  in all five text widgets (R10-F7). `book/src/input-widgets.md:58-59`
  says every path "steps by clusters".
- **No word motion, no multi-click, no Home/End in `TextArea`
  (R10-F14).** No widget reads `EventCtx.clicks`, although the pass
  sets it and `docs/plan-desktop.md` T14.3 carries it on the event for
  exactly this. `TextArea::on_event` has no Home or End arm.
- **Key text past its first character is lost (R10-F8).**
  `translate::keyboard` emits `Event::Text` for the first non-control
  character only (`crates/lumen-static-winit/src/translate.rs:202-205`).
  The verifier's case: a dead key followed by a consonant it cannot
  combine with, on Windows's US-International layout, delivers two
  characters and loses the second.

### 1.5 The clipboard and composition reach less than they claim (R10-F13, R10-F15)

`clipboard_shortcut` (`crates/lumen-static-winit/src/lib.rs:401-434`)
returns `true`, which drops the event, for Ctrl or Cmd with `c`/`x`
when there is nothing to copy and for `v` always, so an application
never sees those keys, not even with nothing focused. It accepts Ctrl
or Meta on every OS, so Super+C copies on Linux. And `NumberInput`'s
`node_mut` returns itself (`crates/lumen-static-widgets/src/field.rs:275`)
and forwards neither `selection` nor `compose` to its inner field, so
it cannot copy (R10-F13) and refuses IME composition (R10-F15).

### 1.6 TextArea's cost grows faster than its text (R16-F7, R8-F3)

- **Two quadratics in shaping (R16-F7).** `shape_into`'s `line_of`
  closure searches every line for each glyph (`text.rs:322-330`), which
  is O(glyphs × lines) and was the verifier's attribution of the
  superlinear keystroke cost. **The re-triage found a second one:**
  when substitution ran (`prepare` returned an owned string), `remap`
  (`text.rs:311-319`) does a `position` and an `nth` over the character
  indices for every glyph, O(glyphs × chars), for any document
  containing one substituted glyph. Reshaping the whole document per
  edit is also real, and linear.
- **One run for the whole value (R8-F3).** `TextArea` paints its whole
  value as a single text command. The software backend culls per run
  and shapes and rasterises every glyph of a run before clipping
  (`crates/lumen-static-shell/src/software.rs:122`), so a caret move in
  a long document rasterises the document, about twice per frame since
  a caret move damages two small regions (R8-F3, with the verifier's
  correction).

### 1.7 The embedded face breaks words (R10-F10)

`BakedFace`'s wrap (`crates/lumen-static/src/font.rs:139-160`), on
overflow with a space behind it, rewinds only the widest-line counter
and resets x. The glyphs of the word already placed stay on the old
line, so it breaks mid-word and measures the wrong width (R10-F10).
Its rustdoc (`font.rs:116`) says it wraps at the last space that fits.

## 2. Decisions

### 2.1 ComplexText: a shaped painter, or an honest refusal (T45.4)

R10-F2 is S1 because T15.8 is marked DONE on a claim that is false.

- **A. Paint the shaper's output.** A display-list command carrying
  glyph ids and positions, produced by the measurer, rasterised by id in
  `Canvas` and in the GPU backend. Caret mapping by cluster lookup
  rather than byte order. A measurer seam on `Program` and in the
  winit `Session` so a window can install `ComplexText`. It touches the
  display list, both renderers, the embedded budget (`Cmd` size) and the
  GPU parity gates. Size L at least.
- **B. Say what it does, and pin it.** T15.8 becomes "measurement
  only". The crate rustdoc, the book's capability table
  (`book/src/text.md:31`) and the handoff say so. A test asserts the
  limitation as it stands: painted Arabic equals its isolated forms. The
  test is marked expected-to-fail against the claim, so it goes red the
  day someone builds A, and forces the documentation to move with it.

**Recommended: B in this milestone, and A proposed as its own
milestone** (§4). A is the right end state, and it is a renderer
project, not a text-correctness fix. B makes every current claim true
now, in one small commit.

**B leaves one harm reachable.** A `Host` user who measures with
`ComplexText` and paints with `Canvas::with_font` gets every label
wrapped. T45.1's painter rule (a run measured as one line is never
re-wrapped) turns that into a horizontal overhang of a few pixels, with
no second line. T45.4's rustdoc on `ComplexText` says to pair it only
with a painter that consumes its clusters, and that none exists yet.

### 2.2 How a paste reaches a widget (T45.7)

Two reviewers proposed opposite fixes for the lost newline.

- **A. Map `'\n'` (and `"\r\n"`, and a lone `'\r'`) to `Key::Enter`**
  (R8-F4). One edit path, the same as typing. `TextInput` keeps
  refusing Enter. It leaves the per-character pumping, so it fixes
  R8-F4 and not R8-F5.
- **B. `TextArea` accepts `Event::Text { ch: '\n' }`** (R10-F5).
  `Event::Text` then carries a control character that every other
  widget must remember to refuse.
- **C. A paste hook on `Node`**: the focused node receives the whole
  string once, inserts it through its own `insert` (the one edit path
  that typing already uses), with its own newline policy, and reports
  how many characters it kept. `TextArea` normalises line endings and
  keeps newlines; `TextInput` strips them. A node that does not
  implement the hook gets today's per-character delivery with A's
  mapping, so a custom widget keeps working.

**Recommended: C.** It closes R8-F4, R8-F5 and R10-F5 in one mechanism,
and the count it returns is what was kept. A recorded session records
the paste string. **What it breaks:** the hook is a new provided method
on `Node`, and a wrapper that does not forward it is exactly the
`NumberInput` shape of R10-F13 and R10-F15. So the gate runs every text
widget, composites included, and the hook is asked of `node_mut(i)` at
the focused index, which resolves to the widget that owns it.

**Built on M41, not beside it.** M41's `Host::paste_with` stays the
entry point, and `Session::paste` keeps calling it with the window's
`FontMetrics`. C removes the chunk loop, so no pump happens mid-paste
at all. The one pump left is M41's final pump, with the real measurer,
which M41 §2.4 keeps because every headless caller reads the field
straight after `paste` returns. The paste changes a widget outside dispatch, so it marks the
focused node's subtree exactly as M41's R10-F9 fix makes
`Host::compose` do, and for a split application it counts as consumed.

### 2.3 One key for every text memo (T45.2)

Each memo builds its own key today: `MeasureKey::of(limits, style, scale)`
for `Label`, `Button` and the rest, a hand-written tuple for
`TextInput` and `TextArea`. **Recommended:** one constructor, taking the
`LayoutCx`, that folds limits as bit patterns (R2-F10), font size,
weight, scale (R2-F6) and the measurer's generation. `TextInput` and
`TextArea` build theirs from it plus the text length, so the next field
that matters goes into one place.

The generation is a provided method on `TextMeasure` that answers 0.
`FontMetrics` bumps it whenever its stack changes: a different face,
and, after T45.3, a fallback face loaded lazily. **Why the measurer
reports it and the host does not:** the host receives a
`&mut dyn TextMeasure` every frame and cannot tell a new measurer from
the old one except by address, and a lazily loaded fallback face
changes measurement with no change of measurer at all. There are 14
`TextMeasure` impls at `1717808` (4 in `src`, 10 in tests), and none
wraps another, so the provided default has no wrapper to be wrong for.
A future wrapper must forward it; T45.2's rustdoc says so.

### 2.4 Where fallback faces come from (T45.3)

- **A. Extend the built-in candidate list** (`CANDIDATES`) with known
  CJK and Arabic/Hebrew-capable faces, loaded lazily the first time a
  run needs a character the loaded faces lack.
- **B. Ask fontconfig.** Correct on Linux, and a new platform dependency
  in the shell, for a crate that has kept platforms out of itself.

**Recommended: A.** Colour emoji stays `?` either way: fontdue cannot
rasterise CBDT or COLR tables. The rustdoc says so, and a monochrome
emoji face is used when one is installed.

## 3. Tasks

### T45.0 — the gates first

Every finding becomes a check that is **red at `1717808`**, committed
marked expected-to-fail until its task lands, as M35/T35.0 did, with
the red output in the commit body. Where a finding cannot be red, the
commit says why and records the injection instead.

1. `crates/lumen-static-shell/tests/text_containment.rs` (R10-F1),
   feature `truetype`: 12 strings × 10 sizes × 2 weights × 5 scales,
   measured with `FontMetrics` and painted with `Canvas::with_font`
   through a `Host` label. Zero wrapped runs, zero ink rows more than
   1 px outside the box. Includes `Taj`, `Tj`, `fj`, and `Settings`
   bold at 13 px and scale 1.5. Prints `wrapped N / 1200, worst
   overhang x/y px (bound 1.0)`. **Red today.**
2. `crates/lumen-static-text/tests/paint_shaped.rs` (R10-F2): paint
   `مرحبا` through the window's painter at `ComplexText`'s box, and
   assert what T15.8 claims: painted glyph ids equal the shaper's, in
   visual order, and the ink stays in the box. **Red today**, with the
   hash equality to the isolated forms printed. This test stays
   expected-to-fail after T45.4 (§2.1 B), which is what makes it the
   pin.
3. In `shaping.rs` (R10-F11): for the pure RTL run, the cluster at byte
   0 has the largest x and x falls as the byte rises; for an Arabic
   word, the shaped glyph ids differ from the isolated forms' cmap ids
   and the width is less than their summed advances. **Green today,
   because the shaper is right**; its injection is the review's
   `ca8547c` (`let rtl = false && …`), which must turn it red, and a
   feature set without `init`/`medi`/`fina` must turn the joining half
   red. The commit records both.
4. In `crates/lumen-static-widgets/tests/text_memo.rs`, or a new
   `measure_scale_widgets.rs` (R2-F6): a `TextInput` and a `TextArea`
   laid out at scale 1, then 2 with the same limits, against fresh
   instances at 2. For `TextArea` the assertion is on the caret x, since
   its box does not move. Uses the non-linear `Hinted` measurer that M40
   shares from `crates/lumen-static/tests/measure_scale.rs`. **Red
   today** for `TextInput` (served ≠ fresh); the `TextArea` leg's red
   state is recorded if it has one, and if the caret does not move
   either, the commit says the leg is by reading.
5. In `crates/lumen-static/tests/measure_key.rs` (R2-F10): five layouts
   at NaN `max_w` make one measurer call. Prints calls beside `bound 1`.
   **Red today:** five.
6. `crates/lumen-static-shell/tests/font_fallback.rs` (R10-F12),
   skipping with a printed reason when the fallback faces are absent:
   `你好` and `שלום` measure and paint differently from `??` and
   `????`, with glyph ids that are not `?`'s. Prints the face index per
   character. **Red today:** identical pixels.
7. `crates/lumen-static-shell/tests/paste.rs` (R8-F4, R8-F5, R10-F5,
   R10-F6), through `Host` under both invalidations. Paste
   `"ab\ncd\r\nef"` into a `TextArea`: `"ab\ncd\nef"`, and the count is
   the characters kept. The same paste into a `TextInput`: `"abcdef"`.
   A 256 KB paste pumps once, printed beside `bound 1`. Typing
   `ab`, Enter, `cd` equals the paste. The focus is asserted by typing
   first. **Red today:** newlines lost, and one pump per 64 characters.
   `clipboard.rs`'s newline test is rewritten against a `TextArea`, and
   the review's injection `ec07f29` (the filter drops `'\n'`) must turn
   it red.
8. `crates/lumen-static-widgets/tests/graphemes.rs` (R10-F7): for each
   of `TextInput`, `TextArea`, `SearchField`, `Combobox` and
   `RichTextEditor`, through `Host` after Tab, Backspace after a
   combining accent, a ZWJ family, a flag and a skin-tone emoji leaves
   `x`; Right then Delete removes a whole cluster. Prints codepoints left
   beside the expected count. **Red today:** 0 of 15.
9. `crates/lumen-static-widgets/tests/text_motion.rs` (R10-F14), with a
   lone field so the arrow rule cannot mask it: Ctrl+Left moves by word,
   a double click selects a word, a triple click selects the line, and
   Home and End in a `TextArea` go to line bounds. **Red today.**
10. `crates/lumen-static-winit/tests/key_text.rs` (R10-F8), through the
    window's key-text routing factored out as a function: `"^a"`,
    `"e\u{301}"`, a flag and `"क्ष"` reach the focused field whole. **Red
    today:** first character only.
11. `crates/lumen-static-winit/tests/clipboard_shortcut.rs` (R10-F13),
    on the shortcut decision factored out as a pure function: nothing
    focused plus Ctrl+C routes on; a selection plus Ctrl+C copies;
    Super+C on Linux routes on; Cmd+C on macOS copies; Ctrl+V with an
    empty clipboard routes on. Prints the table. And `NumberInput`,
    Ctrl+A, copy gives its text. **Red today.**
12. In M41's `crates/lumen-static-shell/tests/compose_marks.rs`
    (R10-F15): Tab to a `NumberInput`, commit `"12"`: accepted, and the
    value contains it; commit `"ab"`: refused. **Red today:** refused
    both. This item lands with M41's file, or as its own file if M41
    has not landed.
13. `crates/lumen-static-shell/tests/textarea_scale.rs` (R16-F7),
    feature `truetype`: the minimum of five keystroke-plus-layout timings
    at 20 KB and 200 KB, ratio below 25 (linear is about 10, quadratic
    about 100), printed as `keystroke 20KB=a ms 200KB=b ms ratio=r
    (bound 25)` beside a 20 KB-against-20 KB self-ratio. A second arm
    with a substituted glyph, asserting that substitution ran by
    counting it. **Red today** on both arms.
14. A glyph-count gate for R8-F3: a 64 KB `TextArea`, one Left, a
    damage-driven render, and a test counter of glyphs rasterised,
    bounded by visible lines × longest line × damage regions and
    printed beside the bound. **Red today.**
15. `fixtures/embedded/tests/wrap.rs` (R10-F10): for a corpus of phrases
    and widths, no line starts mid-word and the measured width is at
    most the limit; `"aaa bbbb"` at 60 is two lines and 41 wide. **Red
    today:** a break at byte 6.

The numbers these print at `1717808` go into a new results doc,
`results-text.md` beside the others in `docs/`, created by this task,
so every later task has a baseline to cite.

**Effort:** L (fifteen gates). **Depends on:** M41 (items 7 and 12 need
its `paste_with` and compose marks), M40 (item 4's shared measurer).

*What building found (2026-09-26).* Every finding re-checked at
`826c7c3` still reproduced; `docs/results-text.md` has each red number.
Where the gates differ from the list above:

- **Items 8, 9 and 11's `NumberInput` half live in the shell's tests**,
  not the widgets crate's: they run through a `Host`, and the widgets
  crate has none.
- **Every editing gate puts its field alone in its tree.** Adding a
  `TextArea` beside `clipboard.rs`'s field turned that file's existing
  Shift+Right test red: with a focusable sibling, an arrow moves focus
  instead of the caret (M44's R3 finding). A gate that shares a tree
  with a sibling measures M44, not M45.
- **Item 1 does not exhibit the fractional-scale mechanism.** With the
  pinned DejaVu face, `Settings` bold at 13 px and 1.5 measures wide
  enough today (the review measured Noto); all 600 wrapped runs are the
  six strings ending in `j`. T45.1's slack injection has to find its
  own case.
- **Item 4's `TextArea` leg is red, not "by reading":** `Hinted` with
  clusters (`x = n × size + scale`) moves the caret, 129 served against
  130 fresh.
- **Item 13's substituted arm is at 2 and 20 KB**, a tenth of the
  plain arm: at 200 KB its quadratic runs for minutes, and a gate that
  takes an hour to go red is not run. The ratio of ten is the same.
- **Item 14's accept direction was wrong.** A 4 KB area is 74 lines and
  rasterised 3,330 glyphs against a bound of 330: it is a long document
  too. The accept arm is an area whose five lines all fit.
- **Items 10 and 11 needed functions to call.** `keys::deliver_key` and
  `keys::clipboard_decision` are the window's own code factored out,
  behaviour unchanged, so the gates drive what the window runs.
- **Item 2 needs the shaper's glyph ids**, which `ComplexText` did not
  expose: `ComplexText::shaped_glyphs`, and `GlyphId::index` for the
  painter's.

### T45.1 — the measurer and the painter agree on the box

**Closes:** R10-F1.

**Change.** In `place_run`, `measure_run` and `shape_into`, the width
is the larger of the pen's end (glyph origin minus its left bearing,
plus the advance) and the ink's right edge. The painter never re-wraps a
run that measured as one line: it lays it out with no width limit, or
the measured width plus half a pixel, which also absorbs the f32
round-down at fractional scales. The GPU backend's `place_run` call
takes the same rule.

**Gate.** T45.0 item 1 goes green: `wrapped 0 / 1200`, overhang within
1 px. `crates/lumen-static-shell/tests/glyph_placement.rs` and
`just gpu-parity` stay green.

**Injection.** Restore the ink-origin width: the gate reports the
wrapped runs and fires by name. Remove only the painter's slack: the
`Settings` case at 1.5 fires, which isolates the second mechanism.
**Accepts good input:** `Taj Mahal` and `W` stay on one line and inside
the box.

**Docs in the same commit.** `text.rs:174-181` and `:257-263`. The
results doc's containment table. `docs/text-render-adoption-decision.md`
if it states containment.

*What building found (2026-09-26).* **The slack injection left the gate
green at first**: every label sat at x = 0, where a rect's physical width
is `w × k` and exact for these scales. The round-down needs the two edges
scaled separately, so the corpus now starts labels at four logical
offsets, and removing the slack wraps 92 of 1,200. Moving labels off the
edge also exposed ink *left* of the box — a leading `j` or `T` bearing,
up to 2.4 px — which the canvas edge had clipped at x = 0; the gate
prints it and bounds only the right edge and the rows, since the box
starts at the pen as a CSS content box does. And **`TextInput` handed
the painter a rect narrower than its run** whenever it was scrolled
part-way; it paints in its run's width now.

**Effort:** M. **Depends on:** T45.0.

### T45.2 — one key for every text memo

**Closes:** R2-F6, R2-F10.

**Change.** Per §2.3. The single constructor on `MeasureKey`, taking the
`LayoutCx`; its callers in `crates/lumen-static-widgets/src/ops.rs` and
`trim.rs` move to it. Limits are stored as bit patterns. `TextMeasure`
gains the provided generation method, and `FontMetrics` bumps it when
its stack changes. `TextInput`'s and `TextArea`'s shaped keys are built
from the constructor plus the text length. `TextInput`'s key drops
`limits.max_w`, which M41 §4 hands to this task: a single-line field
shapes the same run at any width, so the width only makes the audit's
1e6 px probe miss. `TextArea` keeps its inner width, which wraps. This
also delivers the measurer-identity half of M41's R2-F1, as M41 asks.

**Gate.** T45.0 items 4 and 5 go green: served equals fresh after the
scale change, and one measurer call for five NaN layouts. A new case: a
`FontMetrics` whose generation is bumped between two layouts re-shapes
a warm `TextInput`, printed as calls before and after.
`crates/lumen-static/tests/text_memo.rs`'s "a still frame measures
nothing" stays green, so the memo still hits.

**Injection.** Remove the scale from the key: item 4 fires. Store the
limits by value: item 5 fires with five calls. Drop the generation:
the new case fires. **Accepts good input:** finite limits and an
unchanged scale still hit, with one call.

**Docs in the same commit.** `MeasureKey`'s rustdoc (the bit patterns,
and `measure_scale.rs` now covering three widgets). The key comments in
`input.rs` and `textarea.rs`. `docs/results-frame-and-types.md`
T26.17 #12 (the gate now covers the editable widgets). `TextMeasure`'s
rustdoc for the generation, including the forwarding rule.

**Effort:** S. **Depends on:** T45.0; M41/T41.4 (R1-F5's `probe`
override in the same widget; it leaves the key to this task); M40's
shared `Hinted` fixture (M40/T40.0).

### T45.3 — a font stack with fallback

**Closes:** R10-F12.

**Change.** Per §2.4 A. `FontStack` holds a list of faces: the primary
regular and bold, then the rest of `CANDIDATES` plus known CJK and
Arabic/Hebrew-capable faces, loaded lazily. `prepare` and the run code
split a run into segments by the first face that has each character,
and measure and paint each segment with its face. `?` remains only for
a character no face has. The generation from T45.2 bumps when a face
loads, so every memo reshapes.

**Gate.** T45.0 item 6 goes green, printing the face index per
character. Latin text uses face 0 for every character and is
pixel-identical to today: a hash of T45.0 item 1's corpus rendered
before and after.

**Injection.** Limit the stack to its first face: the CJK case fires,
pixel-identical to `??`. Skip the generation bump on a lazy load: a
field measured before the load keeps a `?`-sized box, and the memo case
fires. **Accepts good input:** the Latin hash, as above.

**Docs in the same commit.** `text.rs`'s rustdoc on the tofu rule
("any face in the stack") and on colour emoji.
`docs/00-handoff-static-tree.md` §11 ("What actually kills GUI framework
projects", the Text bullet): font fallback DECIDED, with a pointer.
`docs/text-render-adoption-decision.md`. `docs/plan-desktop.md` T14.8
(IME output is now drawable).

*What building found (2026-09-26).* **The generation does not bump on a
lazy load**, and the injection that skips it has nothing to skip. A face
is chosen by list order — the first face that has the character,
loaded or not — so what a run measures never depends on which faces an
earlier run happened to load, and no memo can hold a box the load
changed. The generation stays what T45.2 made it: the stack's identity,
new on `FontMetrics::set_stack`. **The kill rule fired on the first
build**, +3.4% on the short-label arm, from fontdue's per-glyph
`user_data` carrying segment offsets that only a multi-face run needs;
without it, and with the multi-face case boxed, the arms sit inside the
old build's spread (`docs/results-text.md` T45.3). The pixel-identity
check hashes the containment strings through the *system* stack, since
a stack built from bytes has no fallback faces to change anything.

**Effort:** M. **Depends on:** T45.1 (same run code), T45.2 (the
generation). §5's kill rule applies.

### T45.4 — complex text says what it does

**Closes:** R10-F2, R10-F11.

**Change.** Per §2.1 B. `docs/plan-capabilities.md` T15.8 becomes
"measurement only", with a pointer to the proposed shaped-paint
milestone. `lumen-static-text`'s crate rustdoc replaces "changes where
the glyphs go" with what it changes (the box and the clusters), and
says a window cannot install it. `ComplexText`'s rustdoc says to pair it
only with a painter that consumes its clusters. The book's capability
table says the same. T45.0 item 3's stronger assertions stay, because
what the shaper produces is still claimed.

**Gate.** T45.0 item 2 stays expected-to-fail with its message pointing
at T15.8's new status: the pin. T45.0 item 3 is green. The crate
rustdoc and T15.8's status are the doc-currency check.

**Injection.** For the pin: make the painter draw from the shaped glyph
ids for one test build (the smallest possible A): the expected-to-fail
test passes, so the harness reports it red. For item 3: `ca8547c`, and
the feature set without joining, each red by name. **Accepts good
input:** an LTR Latin run passes item 3 with rising x.

**Docs in the same commit.** As in Change, plus
`docs/00-handoff-static-tree.md` §11's Text bullet (shaped paint OPEN)
and `docs/text-render-adoption-decision.md`.

**Effort:** S. **Depends on:** T45.1 (the painter rule that bounds the
remaining harm).

### T45.5 — edit by grapheme

**Closes:** R10-F7.

**Change.** `prev_boundary` and `next_boundary` in `input.rs` and
`textarea.rs` step by extended grapheme cluster, using the
`unicode-segmentation` crate behind the widgets crate's `alloc` feature
(it is `no_std`). Backspace, Delete, Left, Right and Shift-selection all
go through them. Where `lumen-static-text`'s clusters exist they still
place the caret; graphemes decide editing.

**Gate.** T45.0 item 8 goes green: 15 of 15, with codepoints printed. A
`handled_narrowing.rs` step deletes a flag under `Boundaries`.

**Injection.** Revert `prev_boundary`: the Backspace cases fire. Revert
only `next_boundary`: the Right-then-Delete cases fire, which shows
both directions are covered. **Accepts good input:** ASCII and a
precomposed `é` delete one character.

**Docs in the same commit.** The boundary rustdoc in both editors.
`book/src/input-widgets.md:58-59`. `docs/plan-capabilities.md` T15.8's
scope note. `docs/results-embedded.md` if the alloc tier grows.

**Effort:** M. **Depends on:** T45.0. §5's kill rule applies.

### T45.6 — words, clicks and line ends

**Closes:** R10-F14.

**Change.** In `TextInput` and `TextArea`: Ctrl+Left and Ctrl+Right
(Alt on macOS) move by word boundary from `unicode-segmentation`, and
the Shift variants extend the selection. In `TextArea`, Home and End go
to the line's bounds and Ctrl+Home and Ctrl+End to the buffer's. A
press with `ctx.clicks == 2` selects the word under it; with 3, the line
(`TextArea`) or everything (`TextInput`).

**Gate.** T45.0 item 9 goes green, printing caret and selection beside
the expected ones. The same actions are added to both widgets'
inventories in `scripts/widget_drive.sh`, driven after M42, and to
M42/T42.5's manifest. `TextArea`'s inventory does not exist until T45.7
writes it, so this task lands after T45.7.

**Injection.** Ignore `ctx.clicks`: the double-click case fires with no
selection. Drop the Ctrl branch: Ctrl+Left moves one grapheme.
**Accepts good input:** a plain Left still moves one grapheme.

**Docs in the same commit.** The key tables in both editors' rustdoc.
`book/src/input-widgets.md`. `docs/plan-widget-parity.md`'s inventories
for `TextInput` and `TextArea`.

**Effort:** M. **Depends on:** T45.5; T45.7 (`TextArea`'s drive
inventory); **M44's T44.1** (an arrow must
reach a field with a sibling, or every word-motion gate in a real form
is masked).

### T45.7 — a paste is one edit

**Closes:** R8-F4, R8-F5, R10-F5, R10-F6.

**Change.** Per §2.2 C. The paste hook on `Node`, implemented by
`TextInput` and `TextArea` through their own `insert`, and forwarded by
`NumberInput`, `SearchField`, `Combobox` and `RichTextEditor` to their
inner fields. `Host::paste_with` (M41) asks the focused node's hook and
marks its subtree; with no hook it falls back to per-character events,
mapping line endings to `Key::Enter`. No pump happens mid-paste; M41's
final pump is the only one. It returns the characters kept. `clipboard.rs`'s newline test is
rewritten against a `TextArea`; tab and bell keep their own test.

**Gate.** T45.0 item 7 goes green: newlines kept in a `TextArea`,
stripped in a `TextInput`, the count equal to what was kept, one pump
for 256 KB, and the paste equal to the typed control, under both
invalidations. A `handled_narrowing.rs` step pastes into a split
application's field. Every text widget, composites included, is pasted
into.

**Injection.** Map `'\n'` to `Event::Text` again (the fallback path):
the `TextArea` arm reads `"abcdef"`. Restore the chunk-and-pump loop:
4,096 pumps against a bound of 1. Count what was offered rather than
kept: the `TextInput` count fires. Drop `NumberInput`'s forwarding: its
row fires. **Accepts good input:** a ten-character paste pumps once
and equals typing.

**Docs in the same commit.** `Host::paste` and `Host::paste_with`
rustdoc (the one-edit rule, and what the count means). `Session::paste`.
The hook's rustdoc on `Node`, with the forwarding rule for composites.
`docs/plan-events.md` E0 (a paste replays as a string).
`textarea.rs`'s `on_event` docs. `TextArea` has no drive inventory at
1717808, and M42 lists it as owed to this milestone for its paste
(M42 §4): this commit writes it, with a paste action, removes `TextArea`
from M42's undriven allow file, and adds it to M42/T42.5's manifest.

**Effort:** M. **Depends on:** T45.0; M41 (`paste_with`, and the marking
pattern from R10-F9); M42/T42.5 (the manifest the new inventory joins).

### T45.8 — key text arrives whole

**Closes:** R10-F8.

**Change.** When winit's `KeyboardInput` text has more than one
character, the window routes it as `Session::compose` with a commit of
the whole string, which `TextInput` and `TextArea` accept and which M41
makes mark. Single-character text keeps the `Event::Text` path, so
recorded sessions do not change. `translate` stays one-event-out; the
routing lives in the window loop, as a function the test can call.

**Gate.** T45.0 item 10 goes green: the field holds the whole string,
and the next frame's field width grew, under `Boundaries`. A single
`a` still goes through `Event::Text` (asserted on the event trace).

**Injection.** Route multi-character text back through `translate`'s
first-character path: `"^"` instead of `"^a"`. **Accepts good input:**
the single-character trace.

**Docs in the same commit.** `translate::keyboard`'s rustdoc (its
comment on multi-character labels, `translate.rs`). `docs/plan-desktop.md`
T14.8 (key text against IME).

**Effort:** S. **Depends on:** M41 (compose marks).

### T45.9 — shortcuts route on; NumberInput forwards

**Closes:** R10-F13, R10-F15.

**Change.** The shortcut decision becomes a pure function of the
modifiers, the key, whether anything is focused, the selection and the
OS. It returns no action (so the key routes on) when nothing is
focused, when there is nothing to copy for `c`/`x`, and when the
clipboard is empty for `v`. The modifier is Cmd on macOS and Ctrl
elsewhere. `NumberInput` forwards `selection` to its inner field, and
overrides `compose` to filter the text through its number rule and
forward what survives, refusing when nothing does.

**Gate.** T45.0 items 11 and 12 go green, printing the decision table
and the accepted flag and value.

**Injection.** Return copy whenever the modifier is held: the "nothing
focused" row fires. Remove the `selection` forward: copy is `None`.
Remove the `compose` override: the commit is refused. Remove the
filter: letters reach the value. **Accepts good input:** `SearchField`
still copies its text.

**Docs in the same commit.** `clipboard_shortcut`'s rustdoc.
`docs/plan-desktop.md` T14.5 (the modifier per OS). `NumberInput`'s
rustdoc.

**Effort:** S. **Depends on:** M41 (item 12's compose marks).

### T45.10 — shaping without the quadratics

**Closes:** R16-F7.

**Change.** Two changes, measured separately. `line_of` becomes a
cursor that advances with the glyph index, since lines and glyphs are
both in order. `remap` becomes a table built in one pass, only when
substitution produced an owned string, walking the laid-out and the
original character indices in step. Reshaping the whole document per
edit stays; §5 says when it would not.

**Gate.** T45.0 item 13 goes green on both arms, with the
substitution count printed so the second arm is shown to reach
`remap`. A bench arm at 1 KB, 100 KB and 1 MB, eight runs a side under
the bench lock, records keystroke plus layout in the results doc.

**Injection.** Restore the `position` search in `line_of`: the ratio
fires near 100. Restore the per-glyph `remap`: only the substituted
arm fires, which proves it reaches the fallback. **Accepts good
input:** the self-ratio sits near 1, which shows timing noise is well
under the bound.

**Docs in the same commit.** `textarea.rs`'s module rustdoc (the size
it is built for). `crates/lumen-static/src/cx.rs:93-99` (accurate; add
the per-edit cost). The results doc.

*What building found (2026-09-26).* The "table built in one pass" is
not needed: fontdue emits one glyph per character and substitution is
one character for one, so the caller's offsets are its own
`char_indices`, walked in step with the glyphs. The same walk also
serves T45.3's multi-face runs, whose glyphs fontdue counts from each
segment's start. The bench arm is `--bin textarea`.

**Effort:** S. **Depends on:** T45.1 (it edits `shape_into` first).

### T45.11 — a TextArea paints what is visible

**Closes:** R8-F3.

**Change.** `TextArea` emits one text command per visible line, from
the line starts and scroll offset it already keeps, so the backend's
per-run cull bounds a frame by what is on screen. `RichText` and
`Markdown` get the same treatment if they emit long runs; the commit
checks and says.

**Gate.** T45.0 item 14 goes green: the glyph count at 64 KB within the
visible-lines bound, printed beside it. `crates/lumen-static-shell/tests/alloc_gate.rs`
gains a `truetype` arm for the same frame, which is the load-
independent twin of the count.

**Injection.** Emit the whole value as one run again: the count returns
to the whole document. **Accepts good input:** a 4 KB `TextArea` is
under the same bound before and after.

**Docs in the same commit.** `TextArea`'s paint rustdoc. The Text-arm
cull comment in `software.rs` (a per-run cull does not bound a long
run). `docs/results-frame-and-types.md` Finding 15.

**Effort:** M. **Depends on:** T45.10.

### T45.12 — the embedded face wraps at the space

**Closes:** R10-F10.

**Change.** `BakedFace`'s wrap moves the current word's glyphs to the
new line when it breaks at a space: their x shifts by the space's
position and advance, their line increments, and the finished line's
width is the space's position. A word with no space still breaks at the
glyph. The API has no allocator, so the word is either buffered in a
small fixed array or measured in a first pass; the commit chooses by
`.text` cost.

**Gate.** T45.0 item 15 goes green, printing any offending byte and
line.

**Injection.** Restore the rewind-only wrap: `no line starts mid-word`
fires at byte 6. **Accepts good input:** a single unbreakable word still
breaks at glyphs without panicking.

**Docs in the same commit.** `font.rs`'s rustdoc (the claim becomes
true). `docs/results-embedded.md`'s `.text` figure if it moves.

*What building found (2026-09-26).* The first pass was the right
choice, and how it was written decided the cost: slicing the text and
calling `split` added 4,566 B of `.text` to the firmware, cloning the
`char_indices` iterator 308 B. A wrapped line's width is its last
word's end, not its trailing space; the last line keeps its trailing
spaces, as an unwrapped run always did, so a caret after `"abc "` is
unchanged. The gate's width clause never fired at `0091266` — its
failures were all mid-word breaks — so the width is held by the fixed
`"aaa bbbb"` case (55 then, 41 now), not by the corpus.

**Effort:** S. **Depends on:** nothing in this plan. **It may land
before Phase 3**: it is the embedded core's own measurer, used by no
window, touches no memo, no mark and no live gate, and its gate is a
fixture test. T45.0 as a whole waits for M40 and M41, so an early
T45.12 carries its own gate, T45.0 item 15, committed red first.

## 4. Boundaries of this plan

- **Shaped paint (§2.1 A) is proposed as its own milestone, not
  built here.** It needs a glyph command in the display list, glyph-id
  rasterisation in both renderers, a caret mapping by cluster, and a
  measurer seam in `Program` and the winit `Session`. T45.0 item 2 is
  its first gate, already written.
- **R8-F3's second half needs no task of its own.** Rasterising only
  the glyphs that meet the clip comes with M50's raster work: after
  M50/T50.2's cache a glyph is rasterised once, not per frame, and
  M50/T50.3's blit skips glyphs outside the rows being drawn. T45.11
  does the widget-local half and closes R8-F3; it does not wait for M50,
  which is Phase 4.
- **`Host::paste_with` is M41's** (R10-F4, R2-F1). T45.7 builds on it
  and does not re-specify it.
- **No paragraph cache for `TextArea`.** Linear reshaping per edit
  remains after T45.10. §5 says what would bring it back.
- **No colour emoji.** fontdue cannot rasterise it. §2.4.

## 5. Kill rules and risks

**Kill rules, written before the tasks they govern.**

- **T45.3 (fallback).** A Latin run must stay pixel-identical, and its
  layout cost in `--bin bindings`'s `Label` "layout, changed" arm may
  not rise by more than today's build measured against itself, eight
  runs a side under the bench lock. If per-character face lookup costs
  more than that, the lookup moves behind a "the primary face has every
  character" check done once per run, and the rule is re-run; if it
  still fires, fallback is built for measurement and paint only when
  `prepare` would have substituted.
- **T45.5 (graphemes).** The leaf `no_std` profile's `.text` must not
  change, because segmentation is behind `alloc`. The alloc tier's
  growth is recorded and re-budgeted in `docs/results-embedded.md`, as
  M27/T27.1 did for `Cmd::StrokedRect`. If it exceeds that doc's stated
  headroom, the tables are trimmed to extended grapheme clusters only,
  or the milestone stops at a documented limit.
- **T45.10 (quadratics).** If after both fixes a 100 KB keystroke still
  exceeds a 16.7 ms frame on the bench, the paragraph cache becomes a
  follow-up task with its own attribution first. It is not added to
  this milestone to make a number fit.

**What would stop the milestone.** T45.1 is the keystone: T45.3, T45.4
and T45.10 edit the same run code after it. If the pen-based width
changes layout for runs that are right today by more than a pixel (the
`just gpu-parity` and golden-image gates would show it), T45.1 stops
and re-attributes before anything builds on it.

**Risks, and what would reveal each.**

- *The paste hook is not forwarded by a composite.* That is the
  `NumberInput` shape found twice in this review. Revealed by T45.7's
  every-widget gate, which exists for this reason.
- *A text gate passes on a machine with no faces.* Revealed by §0 rule
  3's printed skip reason; the reference machine has NotoSans, DejaVu,
  NotoSansCJK and DroidSansFallback (per the review's `fc-list`).
- *The scaling test flakes under load.* It is a ratio of minima with a
  self-ratio printed beside it. If the self-ratio drifts from 1 by more
  than a factor of 2 on the reference machine, the timing moves to the
  bench arm and the test counts glyph-line comparisons instead.
- *A generation bump on every frame* would turn every memo into a miss.
  Revealed by `text_memo.rs`'s "a still frame measures nothing", which
  T45.2 and T45.3 both keep green.

## Appendix: finding → task

| ID | sev now | status | task |
|---|---|---|---|
| R10-F1 | S1 | OPEN | T45.1 |
| R10-F2 | S1 | OPEN | T45.4 |
| R8-F4 | S1 | OPEN | T45.7 |
| R10-F5 | S2 | OPEN | T45.7 |
| R10-F6 | S2 | OPEN | T45.7 |
| R8-F5 | S2 | OPEN | T45.7 |
| R10-F7 | S2 | OPEN | T45.5 |
| R10-F8 | S2 | OPEN | T45.8 |
| R10-F10 | S2 | OPEN | T45.12 |
| R10-F12 | S2 | OPEN | T45.3 |
| R10-F13 | S2 | OPEN | T45.9 |
| R16-F7 | S2 | OPEN | T45.10 |
| R2-F6 | S2 | OPEN | T45.2 |
| R8-F3 | S2 | OPEN | T45.11 |
| R10-F11 | S3 | OPEN | T45.4 |
| R10-F14 | S3 | OPEN | T45.6 |
| R10-F15 | S3 | OPEN | T45.9 |
| R2-F10 | S4 | OPEN | T45.2 |
