# Text: what the measurer, the painter and the editors agree on

M45's results (`docs/plan-text-correctness.md`). Every figure a text
gate prints is recorded here once and cited from the rustdoc, the plan
and the commits.

## T45.0 — the baseline, at `826c7c3` (2026-09-26)

Each gate was written before its fix and committed `#[ignore = "red
until T45.k"]`. The quantity it prints, beside its bound:

| item | gate | finding | at `826c7c3` | bound |
|---|---|---|---|---|
| 1 | `lumen-static-shell/tests/text_containment.rs` | R10-F1 | **600 / 1200** runs re-wrapped, worst overhang 93 px (y) | 0 wrapped, 1.0 px |
| 2 | `lumen-static-text/tests/paint_shaped.rs` | R10-F2 | painted `[1390, 1366, 1375, 1365, 1371]` against shaped `[5256, 5260, 5277, 5288, 5341]`; unwrapped paint **equals the isolated cmap forms** | painted = shaped, ink in box |
| 3 | `lumen-static-text/tests/shaping.rs` | R10-F11 | green: RTL x falls 24.07 → 0.0; joined width 37 against 47.47 summed | x falls; width < summed |
| 4 | `lumen-static-widgets/tests/measure_scale_widgets.rs` | R2-F6 | `TextInput` served 129 (width and caret) against fresh 130; `TextArea` caret 129 against 130 | served = fresh |
| 5 | `lumen-static/tests/measure_key.rs` | R2-F10 | **5** measurer calls for five NaN layouts | 1 |
| 6 | `lumen-static-shell/tests/font_fallback.rs` | R10-F12 | `你好` and `שלום` laid out as glyph 34 (`?`) at every character, pixels identical to `??`/`????` | 0 substituted |
| 7 | `lumen-static-shell/tests/paste.rs`, `clipboard.rs` | R8-F4, R8-F5, R10-F5, R10-F6 | 6 mismatches (newlines lost, count 8 for 6 kept); a 256 KB paste **4,096 pumps** | 0; 1 pump |
| 8 | `lumen-static-shell/tests/graphemes.rs` | R10-F7 | **0 / 40** whole graphemes, five widgets | 40 / 40 |
| 9 | `lumen-static-shell/tests/text_motion.rs` | R10-F14 | **12** motions wrong of 12 (0 since T45.6) | 0 |
| 10 | `lumen-static-winit/tests/key_text.rs` | R10-F8 | **0 / 4** multi-character key texts whole (`"^a"` → `"^"`) | 4 / 4 |
| 11 | `lumen-static-winit/tests/clipboard_shortcut.rs`, `clipboard.rs` | R10-F13 | **6 / 12** decisions wrong; `NumberInput` copy `None` | 0; its text |
| 12 | `lumen-static-shell/tests/compose_marks.rs` | R10-F15 | `NumberInput` refused `"12"` and `"ab"` alike | accept `"12"`, refuse `"ab"` |
| 13 | `lumen-static-shell/tests/textarea_scale.rs` | R16-F7 | plain 20→200 KB ratio **59.5**; substituted 2→20 KB ratio **93.2**; self-ratio 1.00 | 25 |
| 14 | `lumen-static-shell/tests/textarea_visible.rs` | R8-F3 | **53,595** glyphs rasterised for one Left in 64 KB | 330 (6 lines × 55 × 1 region) |
| 15 | `fixtures/embedded/tests/wrap.rs` | R10-F10 | closed early by T45.12 (plan header) | — |

Item 13's timings were taken under load (load average 10 on the build
machine; the self-ratio beside them is the check that load did not
spoil the ratio).

## T45.1 — the measurer and the painter agree on the box (R10-F1)

`FontStack::extent` measures each line from where fontdue's pen ends
(`ceil(advance)` per glyph from the line's start) and where its ink ends;
`place_run` lays a run out in the rect's width plus half a pixel.
`tests/text_containment.rs`, 1,200 cases, labels at four logical x
offsets (0, 0.3, 7.7, 13.1):

| build | wrapped | worst overhang right/y | leading bearing (not bounded) |
|---|---|---|---|
| `826c7c3` (ink-origin width, x = 0 only) | 600 / 1200 | 0 / 93 px | — (clipped by the canvas edge) |
| T45.1 | **0 / 1200** | 0.45 / 0 px | 2.4 px |
| T45.1 with the ink-origin width restored | 600 / 1200 | 0.45 / 93 px | 2.4 px |
| T45.1 with the painter's slack removed | 92 / 1200 | 0.45 / 76 px | 2.4 px |

The slack case is the fractional-scale round-down: a rect whose two
edges are scaled separately, so its physical width comes out a hair
under the whole-pixel run measured for it. It needs a label off the
pixel grid; at x = 0 the product was exact for
every case and deleting the slack left the gate green.

A `TextInput` scrolled part-way handed the painter `inner + scroll`,
narrower than its run: 327 px for a 384 px run, and the word at the
right edge painted on a second line under the clip. It paints in its
run's width now (`a_part_scrolled_field_paints_its_run_on_one_line`).
`just gpu-parity` after the change: worst channel delta 1, 0 px over 8
(its baseline, `docs/results-gpu.md`).

## T45.2 — one key for every text memo (R2-F6, R2-F10)

`MeasureKey::new(cx, limits, style)` is the constructor every text memo
uses: limits, size, weight and scale as bit patterns, and the measurer's
`TextMeasure::generation`. `TextInput` and `TextArea` build their shaped
keys from it plus the text length; `TextInput`'s has no width.

| gate | before | after | bound |
|---|---|---|---|
| `measure_scale_widgets`, `TextInput` (width, caret) | 129 served, 130 fresh | 130 / 130 | equal |
| `measure_scale_widgets`, `TextArea` caret | 129 served, 130 fresh | 130 / 130 | equal |
| `measure_key`, five NaN layouts | 5 calls | **1** | 1 |
| `measurer_generation`, `set_stack` between layouts | — | 1 call, then 2 | more than 1 |
| an unchanged scale, three layouts | 1 call | 1 call | 1 |

Injections: the scale dropped from the key fires both widget legs and
`measure_scale.rs`; the limits stored by value (`Constraints`, `f32`
equality) fires the NaN gate at 5 calls; the generation dropped fires
`measurer_generation` at 1 call after the bump.

RAM: `Ui` on the target 1,168 → 1,176 B, the generation's cost
(`docs/results-embedded.md`).

### The memo key's generation — a regression, found and fixed 2026-09-27

**Found by the post-M55 regression run.** An interleaved A/B of every
common bench, `1717808` against the finished branch (8 runs a side,
under the bench lock, load 0.6–2.2), flagged memo hits and text-heavy
layout as slower. Then:

- A sweep across all 24 milestone merge points showed every flagged
  metric flat through M47 and stepping up at M45.
- Measuring each of M45's commits pinned the step to this task's
  `c9aff22`. Before it, grid's `mark_all` layout was 22.80 µs and a
  memo hit 5.19 ns; at it, 28.66 µs and 10.38 ns. No later commit moved
  either.

**Cause.** `MeasureKey::new` read `cx.text.generation()` through
`&mut dyn TextMeasure`, which is a virtual call for every text node on
every layout, in the hottest loop the framework has.

**Fix.** `LayoutCx` reads the generation once, when it is built. A
measurer's generation changes only through `&mut self` on the concrete
type (`FontMetrics::set_stack`, or a constructor), and the context holds
the measurer exclusively borrowed for the whole pass, so the cached value
is the one the call would have returned.

Measured against the unpatched tip, 8 interleaved runs a side, load 1.1–1.4:

| metric | tip, unpatched | fixed | before M45 |
|---|---|---|---|
| memo hit, `short, memo warm (hit)` (min) | 10.20 ns | **6.63 ns** | 5.19 ns |
| grid `mark_all` layout (median) | 28.66 µs | **18.73 µs** | 22.80 µs |
| grid `mark_all` whole pump (median) | 82.13 µs | **72.93 µs** | — |
| fusion `1k/fused` (min) | 12.08 µs | **10.23 µs** | 10.79 µs |
| crossover `1k/static` (min) | 11.70 µs | **10.37 µs** | 10.97 µs |

The ~1.4 ns left on a memo hit is the wider key's comparison: bit
patterns plus a generation, which is what makes the key correct.
Grid's layout now beats its pre-M45 figure. M50's glyph cache and
layout scratch had been paying off all along, hidden behind this call.

**The key still guards.** With `LayoutCx::generation` forced to 0,
`measurer_generation.rs`'s `a_new_generation_reshapes_a_warm_field`
fails. Restored, it passes. The test needs `--features truetype`: run
without it, it lists zero tests and proves nothing.

## T45.3 — a font stack with fallback (R10-F12)

`FontStack::system` attaches a list of fallback faces, each read the first
time a run needs a character no earlier face has; a run that needs several
faces is appended to fontdue's layout as one segment per face. On the
build machine (`tests/font_fallback.rs`):

| text | faces per character | glyphs | width vs tofu |
|---|---|---|---|
| `你好` | fallback 6 (DroidSansFallbackFull), both | 7416, 10005 | 48 vs 22 |
| `שלום` | fallback 0 (DejaVuSans), all four | 1344, 1331, 1324, 1332 | 55 vs 44 |

A mixed run's carets are the caller's offsets: `ab你好cd` reports
`[0, 1, 2, 5, 8, 9, 10]`, x rising across the faces.

**Latin is unchanged**: the twelve containment strings at 11, 16 and
24 px through the system stack hash `0xf688fc6aefbb610b` before and
after.

**Kill rule (§5)**, `--bin bindings`, the real-face "memo cold (miss)"
arms, eight runs a side interleaved under the bench lock (LOADED: load
average 3–7):

| build | short label, ns (median of each run) | 60-char wrapped, ns |
|---|---|---|
| before (T45.2) | 435.4–489.9 | 2,326.6–2,415.2 |
| first build: glyph `user_data` for segment offsets | 452.3–467.8 against 435.9–450.8 | 2,376.4–2,407.5 against 2,311.1–2,355.4 |
| shipped: no user data, `Faces::Many` boxed | 436.6–445.8, one run 493.5 | 2,313.3–2,373.6 |

The first build fired the rule, **+3.4% and +2.4%**, on a path Latin
never takes: fontdue's `user_data` made every glyph of every run eight
bytes wider to serve the multi-face path. The shipped build recovers
multi-face offsets from the laid-out text instead, and returns the one-
face `Faces` in 16 bytes rather than 48. Its runs sit inside the before
build's own spread on both arms.

## T45.4 — complex text says what it does (R10-F2, R10-F11)

Documentation, and two pins in `lumen-static-text/tests/paint_shaped.rs`:
the claim T15.8 made, `#[should_panic]` on "the painter drew other glyphs
than the shaper produced", and the limitation as it stands — painted
`[1390, 1375, 1371, 1366, 1365]`, the isolated letters in logical order.
The smallest possible shaped paint, injected (the test word's joined
presentation forms in visual order, unwrapped), turns both red: painted
`[5256, 5260, 5277, 5288, 5341]` equals the shaper's, and the claim then
fails on its next clause instead (ink to y 49 in a box ending at 47.25).
T45.0 item 3's assertions stay green: RTL x falls, the Arabic word joins.

## T45.5 — edit by grapheme (R10-F7)

`crate::edit::{prev_boundary, next_boundary}` in `lumen-static-widgets`,
`GraphemeCursor` over the whole buffer, behind `alloc`; both editors and
so all five text widgets step through them.

| gate | before | after | bound |
|---|---|---|---|
| `graphemes.rs`, five widgets × four clusters × {Backspace, Right+Delete} | 0 / 40 | **40 / 40** | 40 |
| with `prev_boundary` reverted | — | 20 Backspace cases wrong, 0 Right+Delete | — |
| with `next_boundary` reverted | — | 0 Backspace, 20 Right+Delete wrong | — |
| `handled_narrowing.rs`, Backspace over a flag under `Boundaries<16>` | — | same frame, one range marked | equal |

Leaf `.text` unchanged at 27,964 B; the alloc tier's host cost is in
`docs/results-embedded.md`.

## T45.6 — words, clicks and line ends (R10-F14)

`crate::edit::{next_word, prev_word, word_at, line_at}` in
`lumen-static-widgets`, over `unicode-segmentation`'s word bounds behind
`alloc` (runs of letters, digits and `_` without it); both editors move
and select through them. Built on M44/T44.1's `owns_arrows`.

| gate | before | after | bound |
|---|---|---|---|
| `text_motion.rs`, `words_clicks_and_line_ends` (12 motions) | 12 wrong | **0 wrong** | 0 |
| — with `EventCtx.clicks` ignored | — | 4 wrong (both double and both triple clicks) | — |
| — with the word branch dropped | — | 4 wrong (word-Left moves 17 → 16, word-Right 0 → 1) | — |
| — with the jump branch dropped | — | 3 wrong (Ctrl+End 18 for 23, Ctrl+Home 8 for 0) | — |
| `text_motion.rs`, `home_and_end_keep_to_a_wrapped_line` (3) | — | **0 wrong** | 0 |
| — with the visual line replaced by the `\n`-line | — | 2 wrong (Home 0 for 10, End 19 for 9) | — |
| `handled_narrowing.rs`, word-Left, double and triple click, End beside a button, `Boundaries<16>` | — | same frames, one range marked each | equal |
| — with a collapsing End answering "unchanged" | — | 0 ranges marked | ≥ 1 |
| `agent_motion.rs`: `input.key` `mods`, `input.click` `clicks` | — | caret 4, then 3; selection `0..7` | — |
| `just widget-drive TextInput TextArea`, both strategies and the oracle | 4 + 3 actions | **7 + 8 actions green**, damage 1.00× each, the shadow oracle diverged on no frame | — |

The live drive's first run was vacuous twice, and both are fixed in the
inventory: an isolate typed `Z` over a word that was already `Z`, which
"replaced a word" with the value unchanged; and Ctrl+End was pressed on
the last line, where the line's end is the text's end, so a plain End
passed. Leaf `.text` unchanged at 27,968 B (measured with and without
the change); the 0.85% against `docs/results-embedded.md`'s headline is
older than this task.

## T45.7 — a paste is one edit (R8-F4, R8-F5, R10-F5, R10-F6)

`Node::paste` takes the whole string; `Host::paste_with` asks the focused
node, marks its subtree, and pumps once. A node without one gets typing,
dispatched at once, each line ending as Enter.

| gate | before | after | bound |
|---|---|---|---|
| `paste.rs`, `"ab\ncd\r\nef"` into a `TextArea` / `TextInput`, both strategies | `"abcdef"` / count 8 for 6 kept; 6 mismatches | `"ab\ncd\nef"` (8) / `"abcdef"` (6); 0 mismatches | 0 |
| `paste.rs`, 256 KB into a `TextArea` | 4,096 pumps, 258,048 of 262,144 bytes | **1 pump**, whole | 1 |
| `paste_widgets.rs`, seven widgets (six with a paste, one without) | — | 0 wrong, frames equal under both strategies | 0 |
| `clipboard.rs`, `"a\nb\tc"` into a `TextArea` | `"abc"` | `"a\nbc"`, 4 | — |
| `handled_narrowing.rs`, a paste into a split application's field | — | same frame, one range marked | equal |
| `agent_paste.rs`, `input.paste` through a `Local` session | — | kept 5 of 7, one pump | — |
| `just widget-drive TextArea`, both strategies | not driven | 3 actions green, damage 1.00× | 4.0× |

Injections: the typing fallback mapping `'\n'` to `Event::Text` (the
widget with no paste reads `"abcd\nef"`); `TextArea::paste` dropping
newlines (`clipboard.rs` and the live drive, "kept 6 (want 7)"); the
chunk-and-pump loop restored (4,096 pumps against 1); `TextInput`
counting what was offered (9 against 6); `NumberInput`'s forward removed
(count 5 against 4); the paste left unmarked (all six hook widgets'
frames differ under `Boundaries`, and `handled_narrowing.rs` fires).

## T45.8 — key text arrives whole (R10-F8)

`translate::key_text` sorts a key press's text into none, one character
(an `Event::Text`, unchanged) or `Whole`, which `keys::deliver_key`
commits to the focused node through `Session::compose`.
`lumen-static-winit/tests/key_text.rs`, a `Local` session under its
default `Boundaries`:

| key text | before | after (width 16 → ) |
|---|---|---|
| `"^a"` | `"^"` | `"^a"`, 35 |
| `"e\u{301}"` | `"e"` | whole, 26 |
| a flag | half | whole, 30 |
| `"क्ष"` | `"क"` | whole, 39 |

`whole 4 / 4` (was 0 / 4). A single `a` is still `[Key(Char('a')),
Text('a')]`. Injection: `key_text` answering the first character for
several: `whole 0 / 4`, `"^a"` → `"^"`.

## T45.9 — shortcuts route on; `NumberInput` forwards (R10-F13, R10-F15)

`keys::clipboard_decision` takes the platform's modifier and answers
`None` — route the key on — when nothing is focused, nothing is selected
for `c`/`x`, or the clipboard is empty for `v`; `Session::has_focus`
tells the window which. `NumberInput` forwards `selection`, and filters
`compose` through its number rule.

| gate | before | after |
|---|---|---|
| `clipboard_shortcut.rs`, twelve rows | 6 wrong | **0 wrong** |
| `NumberInput`, Ctrl+A then copy | `None` | `Some("0.00")` |
| `NumberInput`, commit `"12"` / `"ab"` | refused / refused | accepted (`"0.0012"`) / refused |
| `SearchField`, Ctrl+A then copy (accept) | — | `Some("needle")` |

Injections: copy whenever the modifier is held (the nothing-focused and
no-selection rows fire, 2 wrong); the `selection` forward removed (copy
`None`); the `compose` override removed (`"12"` refused); the filter
removed (`"ab"` accepted, `"0.0012ab"`).

## T45.10 — shaping without the quadratics (R16-F7)

Two changes in `FontStack::shape_into`, measured separately: a line
cursor that advances with the glyph index instead of a search per glyph,
and caller offsets walked in step with the glyphs instead of a
`position` and an `nth` per glyph. Clusters for seven texts (empty
lines, a trailing newline, wrapped prose, substitution, a multi-face
run) are byte-identical before and after both.

`tests/textarea_scale.rs`, ratio of minima of five keystroke+layout
timings (bound 25; linear ≈ 10):

| build | plain 20 → 200 KB | substituted 2 → 20 KB |
|---|---|---|
| before | 59.5 | 93.2 |
| line cursor only | **11.1** | 94.2 |
| both | **10.1** | **10.1** |
| both, the line search restored | 53.6 | 18.2 |
| both, the per-glyph remap restored | 10.3 | 92.4 |

`--bin textarea`, keystroke plus layout with the system face, the median
of each run's median, eight runs a side interleaved under the bench lock
(LOADED, load average 6–8):

| size | before | after |
|---|---|---|
| 1 KB | 0.059 ms | 0.058 ms |
| 100 KB | 34.8 ms | **5.4 ms** |
| 1 MB | not run (minutes) | 82.2 ms |

§5's kill rule: a 100 KB keystroke is inside a 16.7 ms frame, so no
paragraph cache.

## T45.11 — a `TextArea` paints what is visible (R8-F3, part a)

One text command per visible line, from the clusters' line starts and
the scroll offset; the whole value as one run only when a measurer
reports no clusters (the stand-in). `tests/textarea_visible.rs`, one
Left and a damage-driven render:

| area | before | after | bound |
|---|---|---|---|
| 64 KB, glyphs rasterised | 53,595 | **225** | 330 (6 lines × 55 × 1 region) |
| 64 KB, allocations during the render | 107,235 (with the whole run restored) | 530 | 660 |
| five lines (accept) | 225 glyphs | 225 glyphs, 472–530 allocations | 330 / 660 |

Injection: the whole value painted as one run again — 53,595 glyphs and
107,235 allocations. `audit_is_inert.rs` read the area's scroll from its
whole-value run; it reads the first painted line now, and still fires
when `TextArea`'s probe override is removed (the Boundaries arm paints
the unwrapped paragraph at the padding edge). `RichText` and `Markdown`
already paint one run per span, a word, and are unchanged.

Part (b) — rasterising only the glyphs of a run that meet the clip —
needed no task: M50/T50.2's glyph cache closes it
(`docs/plan-text-correctness.md` §4). And the gate above counts glyphs
**placed** since T50.2, not rasterised: a warm cache rasterises nothing
for a caret move whether the area paints one run per line or one run in
all, so the rasterised count stopped being able to see this defect.
Placed, the same move is 180 glyphs against the bound of 660, and the
whole value as one run again is 107,190.

## M80/T80.1 — lines at a pitch: the paint seam (2026-10-04)

`Painter::text_spaced` (required; `text_weighted` now a provided forward
to it), `Cmd::Text.line_height` (sixteenths of the size, `0` = the face's
own spacing), and `FontStack::place_run_into` re-placing fontdue's lines
at that pitch with half-leading. Nothing sets the byte from a `Style`
until T80.2, so every default tree paints the same pixels; the gates
drive the placer, the canvas, `Recording`, the wire and the wrappers
directly. Measured on the pinned face (`DejaVuSans.ttf`) at 16 px, two
lines `"Ågjy\nÅgjy"`, whose ceiled new-line size `nls` is **19 px**
(1.19 em):

| gate | quantity | measured | bound | injection → measured |
|---|---|---|---|---|
| placement, `tests/line_height.rs` | line 2 − line 1, at 16/24/32 | 16 / 24 / 32 px | pitch ± 0.5 | re-placement skipped: **19** px at 16 (= `nls`) |
| | box height, at 16/24/32 | 35 / 48 / 64 | `2 × pitch + max(0, nls − pitch)` = 35 / 48 / 64 | re-placement skipped: 38 at 16 |
| | at 0, glyphs off fontdue's own layout | 0 of 8 | 0 | 0 treated as 16 (1.0×): **4 of 8** |
| ink at 1.0× (half-leading −1.5) | ink rows above / below the rect | 0 / 0 (35 rows inked, 35 tall) | 0 / 0 | `max(0, nls − pitch)` dropped: **1** above |
| | changed pixels outside `Recording`'s damage, text change | 0 of 611 | 0 | `max(0, nls − pitch)` dropped: **5** of 611 |
| canvas paints the byte | line 2 − line 1 ink rows at 24 / 32 | 24 / 32 | pitch ± 1 | re-placement skipped: one ink band, setup refuses |
| GPU passes the byte, `lumen-static-wgpu/tests/parity.rs` (RTX 4070) | px over 8, a pitched run at 24 | 0 (28 rows each side) | 2 (the text region's) | GPU passes 0: **366** |
| retention, `tests/retain_align.rs` | damage, only the byte changed | 9,600 px² | the rect's 9,600 | `Recording` skips the byte: **0** |
| wire, `wire.rs` | line_height after a round trip | 24 (`VERSION` 2) | 24 | decode skips the byte: **0** |
| forwarding, `tests/overlay.rs` | `Deferring` commands in / out | 6 / 6 | equal | empty body: **6 in, 5 out**; via `inner.text_weighted`: differs at command 5, `line_height` 24 → 0 |
| | `Scaled` at 2.0, `Recording`: forwarded `line_height` | 24, 24 | 24 | either forwards 0: **0** |

`Cmd` stays **60 B** (`tests/glass_cmd.rs`; `Text`'s payload 36 → 37 B).
Firmware, `just firmware-delta 845de0c firmware_interactive=1000`: **+56 B**
`.text` on `firmware`, `firmware_bind` and `firmware_cascade`, **+52 B** on
`firmware_interactive` (bound 1,000), `.rodata` +4 B each
(`docs/results-embedded.md`, the four-binary table).

The placer's rule, `pitch < nls` included, is `FontStack::place_run_into`'s
rustdoc. **The face's spacing is 19/16 here, not the ≈22/16 the plan
assumed**, so iced's 1.3 (21/16) is *above* it on this face and only 1.0×
to 1.1875× exercise the deficit (`docs/plan-text-fit.md`, "What building
found").

## M80/T80.2 — `Style.line_height`, measured and painted (2026-10-04)

`Style.line_height` (sixteenths of the font size, `0` = the face's
spacing), `Patch::line_height(ratio)` rounded to sixteenths by the
cascade, `.lss` `line-height`, `Fields::LINE_HEIGHT` (bit 17; `EACH` 17 →
18), `NodeState.line_height` stamped by `record_painted`, `MeasureKey`'s
`flow` word, and `LineCount`. `Label` measures `lines × pitch`
(`cx::measure_spaced`) and paints through `text_spaced`; `TextArea`'s row
is the pitch; `RichText` uses the field in place of its 1.45. Measured on
the pinned face at 16 px unless a row says otherwise (`nls` 19 px):

| gate | quantity | measured | bound | injection → measured |
|---|---|---|---|---|
| `Label`, `shell/tests/line_height.rs`, 3 lines at 24 | content height, scale 1 / 2 | 72 / 72 | exactly `3 × 24/16 × 16` = 72 | `Label` measures with `measure_weighted`: **57** |
| | line 3 − line 1 first ink row, scale 1 / 2 | 48 / 96 px | `2 × pitch × scale` ± 1 | `record_painted` does not stamp the byte: **38** (2 × 19) |
| | at 0, content height | 57 | `measure_weighted`'s 57 | — (accept) |
| `TextArea` at 32, caret on line 3 | caret top − content top | 64 px | `2 × pitch` = 64 ± 1 | `line_h` stays the face's: **37** |
| | line 3's ink rows / the caret's rows | 80..92 / 70..102 | inside ± 1 | (same) caret 43..62 against ink 53..65 |
| memo, `lumen-static/tests/measure_key.rs` | warm at 0, cold at 24; height | warm / cold; 32 → 48 | cold, heights differ | `flow` dropped: still warm, **32** served |
| lint, `lumen-static/tests/sheet_lint.rs` | findings: `line_height` on `#go` (Button), on `#title` (Label) | 1 (`Unread(LINE_HEIGHT)` on the button) | the button only | `Label`'s seed omits the bit: **2** |
| masks, `widgets/tests/style_reads.rs` | kinds measured reading `line_height` | `label`, `textarea`, `richtext` (33 kinds) | exactly those | `TextArea`'s seed omits the bit: "textarea: omits read [line_height]" |
| runtime applier, `shell/tests/lss_parity.rs` | `pass_accept.lss` rules, both lowerings | 6 / 6 (loud 7 / 7), equal | equal | `set` arm deleted: panics "no setter `line_height` taking F32(1.5)"; `check_lss_setters.sh` stays green (23 setters, 23 declared) |
| sizes, `lumen-static/tests/cursor.rs` | `Style`, `NodeState`, per node, per 100 nodes | 80, 84, 164 B, 16.0 KB | 80, 84 | — (K80a, unmoved) |
| live, `just widget-drive Label`, four legs | label height after the switch, scale 1 / 2 | 88 → **96** (4 × 24) / 66 → **72** (3 × 24), Noto Sans `nls` 22 | `lines × pitch` ± 0.5, grown by `lines × (pitch − before)` | `measure_weighted`: **88 → 88** on every leg |

The live drive's other checks on the same action: the label's pixels
change (`ui.regionHash`, 27,544 px at scale 1), the damage region over it
is **1.00×** its own box (30,048 px² at scale 1, 22,932 at scale 2; the
switch is the action's `subject`, since the stage re-centres the demo),
the press outside the switch changes nothing (96 → 96), the keyboard pass
reaches the switch in one Tab, and the shadow oracle compared 310 frames
with none diverging. The headless showcase test
`the_label_demos_switch_sets_a_line_height` is the same press without a
window.

Embedded `Ui` 1,192 → **1,216 B**, all of it the `flow` word through
alignment (`docs/results-embedded.md` M80/T80.2). Firmware, `just
firmware-delta 076ae16`: **+630 B** `.text` on `firmware`, **+802 B** on
`firmware_interactive` (bound 1,000), +678 / +730 on `firmware_bind` /
`firmware_cascade`; M80a's K80b total so far is 686 of 1,024 B and 854 of
1,536 B (the four-binary table there).

K80c, read here because `flow` is what the memo-warm compare pays (eight
invocations a side under `just bench-lock`, base `076ae16`): `grid`'s
`mark_all` frame 97.94 µs (89.16–98.66) → **95.17**; `retain --text`'s
build + `Incremental` 19,250.6 ns (18,759.9–19,746.8) → **18,867.5**;
`bindings`' "layout, memo warm" 7.54 ns (6.79–13.02) → **7.71**. All
three inside the parent's spread; the rule did not fire.

## M80/T80.3 — `TextOverflow::Clip` and `Ellipsis` (2026-10-04)

`Style.overflow: TextOverflow { Wrap, Clip, Ellipsis }` (`Style`'s last
spare byte, so 80 B), `Patch::overflow`, `Fields::OVERFLOW` (bit 18;
`EACH` 18 → 19), `.lss` `text-overflow` (`Takes::Overflow`,
`lower::Val::TextOverflow`, the shell's `set` arm and the macro's quote
arm), the overflow byte in `MeasureKey`'s `flow`, `edit::floor_boundary`,
and `fit::cut`, the grapheme-snapped binary search `Label` and `AppBar`
share. `Label` keeps its cut as `fit: (u32, f32)` from inside the memo's
miss and paints the prefix and `…` as two runs; it measures and cuts
against its own width less its padding (T80.2's carried defect). Without
an allocator `Label` reads every mode as `Wrap` (K80b's fallback,
`docs/results-embedded.md` M80/T80.3). Pinned face at 16 px unless a row
says otherwise:

| gate | quantity | measured | bound | injection → measured |
|---|---|---|---|---|
| corpus, `shell/tests/ellipsis_graphemes.rs`, `AppBar` and an `Ellipsis` `Label` | cuts inside the ZWJ family / flag / skin tone / accent | 0 / 0 / 0 / 0 for both (20 / 10 / 10 / 0 at `553fa5e`); near 542 / 352 / 352 / 138 (bar), 383 / 214 / 214 / 122 (label) | 0 inside, > 0 near | `floor_boundary` snaps to `char`: bar **20 / 10 / 10**, label **18 / 9 / 9**; the accent stays 0 (advance 0, T80.0) |
| | the old pins against the new code | — | — | `[0, 20, 10, 10]` asserted, `[0, 0, 0, 0]` measured: red, as the flip should be |
| `.lss`, `shell/tests/lss_parity.rs` | `pass_accept.lss` rules both ways (it gained `text-overflow: ellipsis`) | 6 / 6, loud 7 / 7, equal | equal | `set` arm deleted: panics "no setter `overflow` taking TextOverflow(Ellipsis)"; the macro maps `ellipsis` to `Clip`: "rule 1 (Kind(\"label\")) differs" with both patches printed |
| masks, `widgets/tests/style_reads.rs` (34 kinds; `AppBar` added) | kinds reading `overflow` | `label`, `appbar` | exactly those | probe texts left short: "label: declares unread [overflow]", "appbar: declares unread [overflow]" |
| | setup: the probe text under `Wrap` at 200 / 400 px | 4 / 2 lines | > 1 | — |
| fits its box, `shell/tests/text_containment.rs`, 46 widths × 5 texts | truncated cases; worst ink overhang right/top/bottom | **201** of 230; **0 px** | ≥ 30; 0 | `fit::cut` forgets the mark's width: **14 px** |
| | uncut cases byte-identical to `Wrap` | 29 of 29 | all | — (accept) |
| the carried defect, same file: `Fixed(90)`, padding 8, offered 480 | lines measured; ink overhang | 4 lines; 0 px | > 1; 0 | measured against the offer: **1 line, 54 px** |
| `Clip` damage, same file (the host's retained damage) | damage / the label's box | 1,520 / 1,520 px² = **1.00×** | 1.00× | whole text in a 10,000 px rect under a clip: **6.00×** (window-clipped) |
| S2, `agent/tests/damage_shape.rs`, two cut labels in a column | the journal's changed ids after the first cut moves (`"a first"` → `"abcdef"`) | `["first"]` | second absent | `Cmd::hash_into` folds `text.0`: `["first", "second"]` |
| memo, `lumen-static/tests/measure_key.rs`, `"one\ntwo"` | warm under `Wrap`, cold under `Ellipsis`; height | warm / cold; 32 → 16, shown `"one"` | cold, heights differ | overflow dropped from `flow`: still warm, **32** served |
| cost (K80d), `shell/tests/fit_cost.rs`, 1,568 cuts over 7 titles | runs measured per cut, worst, by title length | 8 (49–64 B), 11 (352–408 B), 6 (13 B) | `⌈log2 len⌉ + 3` = 9 / 12 / 7 | linear search: **50–65, 135–147, 14** |
| sizes, `lumen-static/tests/cursor.rs` | `Style`, `NodeState`, per node, per 100 nodes | 80, 84, 164 B, 16.0 KB | 80, 84 | — (K80a holds) |

`size_of` on the host (alloc tier): `Label<&str>` 72 → **80 B**,
`Label<String>` 80 → 88 B (the `fit` pair); `AppBar` 72 → **104 B**, its
hand-rolled `(fold, room, size, weight)` key replaced by a `FoldedText`
under `MeasureKey::new`, which adds the measurer's generation and the
overflow byte the old key never had. The embedded `Ui` is unchanged at
1,216 B because the leaf tier has no cut.

**Live**, `just widget-drive Label`, all four legs green (`Everything`,
`Boundaries`, the shadow oracle with 375 frames compared and none
diverging, and scale 2): the ellipsis switch takes the paragraph from
**88 → 22 px** (4 × 22, Noto Sans; 66 → 22 at scale 2, 3 × 22),
`getAppliedStyles` reports `overflow: Ellipsis`, the agent still publishes
all 113 characters, and `~changes.` (the last word, which the cut drops)
still resolves to the label; its pixels change (`ui.regionHash`, 27,544 px
at scale 1); the press outside changes nothing (22 → 22); a second press
wraps it back to 88. Under the injection the inventory names (`Label`
reads every mode as `Wrap`, the leaf tier's rule, on the host) the action
fails on all four legs, "height 88.0 -> 88.0" (66 → 66 at scale 2).
Damage over the label is **1.05×** (cut) and
**1.47×** (re-wrapped) of its own box at scale 1, **2.09×** at scale 2,
under the journal's bound of 4.0: the label's box changes height and the
stage re-centres the demo, so the region over it holds its old and new
rects, where T80.2's line height grew the box in place and read 1.00×.
`just widget-drive AppBar` (inventory unchanged) is green on all four
legs, the oracle comparing 411 frames with none diverging. The headless
twin is `the_label_demos_ellipsis_switch_cuts_to_one_line`.

**K80c** (eight invocations a side under `just bench-lock`, base
`3b99585`) **fired on the first build** and was answered: `bindings`'
"layout, memo warm" read **9.81 ns** against the parent's 7.13–7.92,
because `Label` now computes the room it measures against on every layout,
memo hit included, and called `resolve_self` for it. Attributed by putting
the room back at the offer (the rest of T80.3 kept): about 8.3 ns against
the base's 7.77, so the call was about 1.1 ns. With the width half of
`resolve_self` written out (`text_room` in `ops.rs`): `grid`'s `mark_all`
frame 96.29 µs (93.11–100.50) → **96.73**; `retain --text`'s build +
`Incremental` 18,712.2 ns (18,585.8–19,849.5) → **18,989.6**; "layout,
memo warm" 7.82 ns (7.33–13.49) → **7.88**. All three inside; the rule
does not fire on the committed code.

Firmware, `just firmware-delta 3b99585`: **+224 B** `.text` on
`firmware`, **+296 B** on `firmware_interactive` (bound 1,000), +248 /
+236 on `firmware_bind` / `firmware_cascade`, `.rodata` +0; K80b, `just
firmware-delta 845de0c firmware=1024 firmware_interactive=1536`: **910 of
1,024 B** and **1,150 of 1,536 B**, after the fallback (2,710 B on
`firmware` without it).


## M80/T80.4 — `Selectable<S>` and `app.copy` (2026-10-04)

`Selectable<S: TextStorage>` (`lumen-static-widgets`, `selectable.rs`,
behind `alloc`): one `FOCUSABLE` table entry that owns a `Label<S>`, lays
it out and paints it at the same index, shapes the shown text into a
cluster list keyed exactly as the label's memo is, and answers
`Node::selection` itself, so `Host::copy` (which asks the focused index)
gets the selection. Press, drag under capture, double press (`word_at`),
triple press and Ctrl+A (all), Escape and a lost focus (clear);
`Backspace` and `Delete` are not handled, so the window's Cut copies and
deletes nothing. `TextTarget` clamps the selection when a bind replaces
the text. `Label`'s paint is split `pub(crate)` into `paint_ground` and
`paint_run` so the tint goes between them. The window answers `app.copy`
(`{copied, clipboard, set, error}`). Pinned face at 16 px:

| gate | quantity | measured | bound | injection → measured |
|---|---|---|---|---|
| copy, `widgets/tests/selectable.rs`, a drag over two lines | `Host::copy()` after a press at "quick" and a drag to "jumps" on line 2 | `"quick brown fox\nLid jumps"`, 25 B | exactly the dragged substring | `Selectable`'s `selection` override removed: **`None`**, press and drag still handled |
| | setup: press / drag handled; the focused index | true / true; the Selectable's (1) | both; its own | — |
| | a double press; a triple press | `"over"`; all 52 B | one word; the text | — |
| | accept: a press on a plain `Label` beside it | not handled, `copy()` `None` | false, `None` | — |
| rows, same file, 8 identical lines | each line's (ink top − tint top) against line 1's, at `line_height` 0 and 24 | 0.00 px on every line, both (rows 19 px at 0, 24 at 24) | ≤ 1 px | rows at `TextMeasure::line_height` (18.6 px at 0): line 5 **2.00 px**; at 24, line 2 **5.00 px** |
| the bind seam, same file | `copy()` after a whole-text selection and a bind writing `"Hold"` | `"Hold"` | the new text | no clamp in `TextTarget::set_str`: **`None`** |
| idle, `widgets/tests/selectable_idle.rs` (alone in its binary) | runs measured or shaped over two forced-layout idle frames | **0** | 0 | reshape on every layout: **2** |
| | setup: both frames rewrote the node's zeroed box; the first frames shaped | 2 of 2; 2 runs | 2; > 0 | — |
| Ctrl+X, `winit/tests/clipboard_shortcut.rs` | the Cut's `Backspace`: handled; text; `copy()` | false; unchanged; `"this"` | false; unchanged; the word | `Selectable` clears its selection on `Backspace`: **handled true, `copy()` `None`** |
| | setup: the drag handled; `keys::clipboard_decision` for that state | true / true; `Cut` | `Cut` | — |
| sizes, `selectable.rs` | `Label<&str>`, `Label<String>` on the host | 80, 88 B | T80.3's 80, 88 | — |
| role, `widgets/tests/a11y_roles.rs` | rows / `Node` impls covered | 64 / 48 of 48 | all | the row deleted: "Node impls with no row in the table: [\"Selectable\"]" |
| contrast (M57's D4), showcase `every_demo_text_reads_in_both_palettes` | the demo's text over its own tint, light / dark | **8.66 / 5.58:1** at alpha 40 | 4.5 | alpha 64 (`input::selection_tint`'s): dark **3.62:1** |

**The highlight is lighter than a field's, by measurement.** The plan
reused `input::selection_tint` (the text colour at alpha 64). The demo
opens with its paragraph selected so the contrast gates see text over the
tint, and at 64 the dark palette read 3.62:1, under AA. `Selectable`'s
default is the same colour at alpha 40 (`selectable::tint_of`), which
passes both palettes. A `TextInput`'s or `TextArea`'s selected text sits
on the alpha-64 tint and is not measured by any gate, because no demo
opens with a field's text selected; that is recorded here, not changed by
this task.

**Live**, `just widget-drive Selectable`, all four legs green
(`Everything`, `Boundaries`, the shadow oracle with 322 frames compared and
none diverging, scale 2 with the full-repaint check): a drag along the
first line then `app.copy` copied **`"Drag across these words to"`**
(26 chars, 5 words; 27 at scale 2), `clipboard` equal and `error` null,
both inputs `handled`; a double click copied one word (`"across"`;
`"these"` at scale 2); the same drag above the widget, and a press just
outside, left `copied: null`. Each action changed the widget's pixels
(`ui.regionHash`, 26,576 px) and damaged **1.00×** its own box (26,576 px²;
20,922 at scale 2). The keyboard pass reached it in one Tab; Escape
cleared the selection and Ctrl+A selected all 117 characters again (the
readout). Under the inventory's injection (no `selection` override) the
drag's inputs are both `handled` and `app.copy` answers `copied: null`:
the action fails on all four legs.

Firmware, `just firmware-delta 1cd123e firmware=0 firmware_interactive=0
firmware_bind=0 firmware_cascade=0`: **+0 B** `.text` and `.rodata` on all
four binaries — `Selectable` is `alloc`-tier and the paint split compiles
to the same leaf code. With `paint_run` marked `#[inline(never)]`: +28 /
+332 / +40 / +28 B `.text` (+16 `.rodata` each), failing all four by name.

## M80/T80.5 — `caret_rect` and the IME area (2026-10-04)

`Node::caret_rect(&self, st, i)` (provided `None`) answers the caret `paint`
draws, in logical px, scroll included: `TextInput` and `TextArea` override
it, `NumberInput` forwards it (it shares its field's index), and a
read-only field answers `None`. `Host::caret_rect()` asks the focused index
and scales to physical px. `Session::caret_rect()` is required (the
compiler is the `Hot` forward's gate) and `Local`'s body runs through the
host's guard. The window passes the rect to `set_ime_cursor_area` after
each frame in which it changed (its own frames and an agent request's),
keeps it, and answers `app.imeArea` (`{area}`). Pinned face at 16 px:

| gate | quantity | measured | bound | injection → measured |
|---|---|---|---|---|
| painted caret, `shell/tests/ime_area.rs` (`TextInput` empty, mid-text, filled past its width; `TextArea` line 3; `NumberInput`; scale 1 and 2) | \|`Host::caret_rect()` centre − the caret column's centre found in the software canvas\| | worst **0.36 px** over 10 cases (0.00 on seven) | ≤ 1 physical px | `TextInput::caret_rect` leaves out its scroll: the scrolled case **356.14 / 671.28 px**; `Host` returns the logical rect: every scale-2 case off, worst **232.50 px** |
| | setup: the press focused the field; the caret column found (rows); `TextArea`'s caret line | all 10; 18–19 px of an 18.6 px line (37 of 37.2 at 2); line 3 | focused; ≥ 70% of a line; 3 | — |
| every composing node, `widgets/tests/caret_rect.rs` (the catalogue) | composed with a box / failed | **6** of 7 composed (35 focusable; the menu's boxless trigger counted, not judged) / **0** | ≥ 5 / 0 | `NumberInput` does not forward: **1** failed, `numberinput #NumberInput: composed and answered None` |
| live, `widget_drive.sh TextInput,TextArea,NumberInput,SearchField`, four legs | \|`app.imeArea` centre − the caret column in `ui.screenshot`\| after every action leaving a field focused | **96** checks (24 a leg), worst **0.50 px**; the oracle 1,842 frames compared, none diverged | ≤ 1 physical px | the window never calls `set_ime_cursor_area` (Boundaries): `app.imeArea` **null** on the first focusing action; it passes the logical rect (scale 2): **556.04 px** |
| across a swap, `just hot-window-drive` | after four swaps, `#field` focused: `app.imeArea` against the field's physical box | `[8, 84, 9, 105.8]` inside `[0, 78, 16, 111.8]` | inside | `Hot::caret_rect` returns `None`: **null**, step 8 fails |
| panic guard, `just panic-drive`, both legs | sites caught / panics logged; `app.imeArea` while each screen is up | **8 / 9** (site 8, a focused field's `caret_rect`, at `lib.rs:130`); null at every site | 8 / 9; null | `Local::caret_rect` unguarded: sites 1–7 pass, the window **dies at site 8** (exit 101, 8 panics logged of 9) |

**Two defects the gates found first.** A full `TextInput`'s end caret was
painted nowhere: the view kept the caret's left edge in the padded box and
the clip ended there, so the one-pixel caret lay wholly outside it (the
scrolled case found a 12 px glyph stem instead of a 19 px caret, and at
scale 2 a shaper's unrounded end pen, 580.14 against a 580 width, cut
0.78 px off it). The clip now runs `CARET_W` into the right padding, `place`
reserves only what that padding cannot hold, and the scroll's end is the
further of the run's width and the caret's x. And a read-only field
(a pick list's trigger) took an input method's preedit and a paste into its
shown option, against M57/B20's rule; it refuses both now. The catalogue
gate's first run named it: `textinput #chooser-trigger: composed and
answered None`.

Firmware, `just firmware-delta cf1f623` (K4's default, bound 1,000 B on
`firmware_interactive`): `.text` **+60 / +116 / +56 / +60 B** (`firmware`,
`firmware_interactive`, `firmware_bind`, `firmware_cascade`), `.rodata`
+16 / +44 / +16 / +16 — the `Node` vtable slot and its shared default on
every binary, and `TextInput`'s `caret_rect`, clip and scroll on the
interactive one. `docs/results-embedded.md` T80.5 has the table.

## M81 baselines — T81.0, at `d4859b4` (2026-10-04)

Recorded before M81 (`docs/plan-shaped-paint.md`) changes anything, so
every later M81 task compares against a number taken on the same machine
from the same tree. Two pins land with them; both compile today and hold
today's behaviour.

| what | gate or command | at `d4859b4` | bound / later use |
|---|---|---|---|
| Hebrew paint order | `lumen-static-text/tests/paint_shaped.rs` `hebrew_paints_right_to_left` (`should_panic`; moved to the shell's tests by T81.3) | `שלום` painted as glyphs `[1344, 1331, 1324, 1332]` at x **1, 24, 44, 53** (logical order, rising); every letter has a cmap glyph (setup) | x strictly falling; T81.3 drops the `should_panic` |
| — accept direction | the same assertion fed `ComplexText::shaped_glyphs`' x per glyph | x **48.14, 29.95, 21.23, 0** — falling; the harness reports "did not panic" | — |
| every `Basic` placement | `lumen-static-shell/tests/basic_placements.rs` | `BASIC_CHECKSUM` **0x866444d1064ccda8**; 6,600 glyphs (2,200 per scale at 1, 1.25, 2), 240 narrowed runs wrapped per scale, **0** glyphs at a fractional x or y | equal, with `shaping` compiled in (T81.3) and phases built (T81.8) |
| — injection | `cap_shift` floors instead of rounding | checksum 0x550bf3328192bd02; scale 1 named first (0xe46a0473ae03abd5 against 0x5ed8be5616d27ffd) | — |
| firmware, paint-only | `just firmware-size` | `.text` **29,586 B**, `.rodata` 13,276 B, `.bss` 4 B | K4: 81,920 B |
| the four firmwares | `just firmware` | `.text` / `.rodata`: `firmware` **29,586** / 13,276; `firmware_interactive` **63,188** / 21,806; `firmware_bind` **33,046** / 13,880; `firmware_cascade` **31,726** / 14,052 | T81.1 Δ 0 on all four; T81.2 ≤ 64 B each |
| face heap, pinned face (759,720 B file) | `lumen-static-shell/tests/font_heap.rs` | one parsed face **19,011,952 B**; a stack with a bold face built 19,771,672 B; peak through a regular frame 22,045,057 B; one bold run grew the heap 18,251,760 B | K81c (T81.3): a `Complex` stack ≤ `Basic` + file size + 64 KiB |

**Every `Basic` glyph sits on whole pixels** (the fractional count above):
fontdue lays glyphs out at whole pixels and `cap_shift` rounds its shift, so the
plan's first injection, flooring one glyph's `y` in `place_run_into`, was
a no-op and left the gate green. The injection that breaks the mechanism
is the rounding that makes `y` whole: `cap_shift` floors instead of
rounding, and the checksum moves at every scale
(`docs/plan-shaped-paint.md`, "What building found").

## M81/T81.1 — the dependency turned, and `GlyphKey` (2026-10-04)

`lumen-static-text` depends on the core alone; the shell is its
dev-dependency (for `tests/paint_shaped.rs`), and the shell reaches the
text crate behind its default-off `shaping` feature, forwarded by
`lumen-static-winit`, `lumen-static-wgpu`, `benches` and the frame arm.
`shaping` joins `abi_fields` and `process_features`. The core gains
`text::GlyphKey`; the shell's `GlyphId` is a newtype over it. No
behaviour changes: `BASIC_CHECKSUM` is unchanged (0x866444d1064ccda8).

| gate | quantity | measured | bound | injection → measured |
|---|---|---|---|---|
| `shell/tests/basic_placements.rs` sizes | `size_of` `GlyphKey`, `GlyphId` | **8**, **8** | 8, 8 | — (accept direction at HEAD) |
| `shell/tests/basic_placements.rs` round trip | corpus ids read back as a `GlyphKey` field for field; distinct ids / keys | **3,300** ids (1,650 regular, 1,650 bold); 1,300 / 1,300 | all fields equal; equal counts | the conversion drops `face`: `GlyphKey.face is 0, expected 1`, first id `index: 55, face: 1` (`Taj`, 10 px, 700) |
| `just release-window-shape` | `rustybuzz::`/`unicode_bidi::` symbols, default release `examples/minimal` | **0** | 0 | — (the symbol positive control is T81.3's; the pattern finds 1,082 in `paint_shaped`'s test binary) |
| — graph | `cargo tree -p lumen-static-shell -e normal`, unique packages, `truetype` → `shaping` | 16 → **27** (+11) | > 0 | `shaping` without `dep:lumen-static-text`: 16 → 16, **0**, the script fails |
| `just abi-stamp` | a `shaping` library under a loader without / both with | refused, naming `lumen-static-shell features` (`reload,truetype` against `reload,truetype,shaping`) / accepted, 20 fields | refuse / accept | `shaping` left out of `abi_fields`: the mismatched pair is **accepted** and the host panics "accepted a library that should have been refused" |
| `just fonts`' new lines | `clippy -p lumen-static-shell --features shaping --all-targets`, `cargo doc` with it | clean | clean | an unused binding under `cfg(feature = "shaping")`: the shaping clippy line fails, the `truetype` line stays clean |
| `just firmware-delta faeffe7`, all four bounded at 0 | `.text`/`.rodata` Δ | **+0 / +0** on all four | 0 | — |

Crate counts by K2's method (`cargo tree -e normal --prefix none`, `(*)`
stripped, deduplicated): the shell 8 default, 16 `truetype`, 27
`shaping`; `lumen-static-winit` **96 → 103** with `shaping` (the text
crate, rustybuzz and five unicode tables; ttf-parser is already there);
the text crate 21 (27 while the shell was a normal dependency). The
stripped showcase delta is K81e's, measured in T81.3 when the feature
has code.

## M81/T81.2 — `Shaper`, `FaceSource`, and clusters in source order (2026-10-04)

`lumen-static-text` gains `Shaper` over a `FaceSource` (bidi levels only
when a character can be right to left, runs by direction and face,
rustybuzz per run in logical order, greedy UAX #14 breaking, each line
reordered visually) and its output `ShapedRun` (`ShapedGlyph`s keyed by
the core's `GlyphKey`, `Cluster`s, `ShapedLine`s). `ComplexText` is a
single-face `FaceSource` over it: it now wraps to `max_w` and reports
clusters in source order with leading-edge `x`. The core's `Cluster`
gains `rtl`, set by every constructor. Nothing paints from the shaper
yet (T81.3). Gates in `crates/lumen-static-text/tests/shaper.rs`, pinned
face, 16 px:

| gate | quantity | measured | bound | injection → measured |
|---|---|---|---|---|
| Hebrew source order | `שלום`'s clusters (byte, leading x, rtl) | (0, **35.41**), (2, 24.07), (4, 14.98), (6, 10.62), all `rtl`; the first equals the run's width | byte rises, x strictly falls, `rtl` set | lines laid out in logical order (an RTL run's clusters not reversed): x **11.34, 20.43, 24.79, 35.41**, "Hebrew leading edges do not fall" |
| — accept, same test | `Latin` | x 0, 8.91, 18.72, 24.99, 29.44, 39.58, `rtl` clear | x strictly rises | — |
| wrap, 80..400 px step 8 | the containment corpus's twelve strings as one paragraph | 8 lines at 80 px (setup > 1); worst overshoot **−0.578 px** (at 88); 101 breaks at an opportunity, 0 inside a word | ≤ 0.5 px; every break at a UAX #14 opportunity or inside a word wider than `max_w` | the breaker ignores `max_w`: **1 line** at 80 px, overshoot **446.0 px** |
| — in-word breaks, 40 px | the same paragraph | 15 lines, worst −0.430 px; 9 at an opportunity, **5** inside `Settings` (65.6 px), `jejunj`, `POSITIVE`, `gypsy`, `Mahal` | the same; setup in-word > 0 | — |
| weight and fallback | faces per char, a source with face 0 regular, 1 bold, 2 Hebrew | `a`, `b` → 0; `ש ל ו ם` → **2**; `ab` at 700 → **1, 1** | Hebrew 2, Latin 0, bold 1 | the shaper ignores the source's face: "Hebrew 'ש' (U+05E9) was keyed face 0" |
| `size_of::<Cluster>()` | bytes | **12** | 12 | — (accept direction at HEAD) |
| firmware | `.text` Δ against `cdaf0ec`, four binaries | **−220 / −240 / −220 / −220 B** | 64 B each | `docs/results-embedded.md` M81/T81.2 |

Crate counts by K2's method after `unicode-linebreak` (one crate, no
dependencies): the text crate **22**, the shell with `shaping` **28**,
`lumen-static-winit` with `shaping` **104** (96 default, unchanged).

## M81/T81.3 — `Shaping` on the stack: both backends paint shaped (2026-10-04)

`FontStack` gains a mode, `Shaping { Basic, Complex }` (`Complex` with the
shell's `shaping` feature), set by `with_shaping`. Under `Complex`,
`place_run_into` (both backends), `measure_run` and `shape_into` lay a run
out with `lumen-static-text`'s `Shaper` over the stack's own `FaceSource`,
and put each glyph on fontdue's baseline, the `Basic` cap shift and M80's
pitch. Face bytes come back from where the stack read them (`from_files`,
`system`) or the caller's `'static` bytes (`from_static`), interned once
per process; `from_bytes` keeps none, and lays out as `Basic`. A cut
`Label` puts its `…` at the left of a right-to-left prefix (`fit::rtl`).
**K81a fired, and its response is built**: a run with nothing to shape
(`text::needs_shaping`) takes fontdue's path under `Complex` too.

### K81a — `Complex` on Latin

`cargo run --release -p benches --features shaping --bin textarea --
--shaping basic|complex 1 100`, the new "keystroke + layout + paint
[mode]" rows, eight invocations a side under `just bench-lock` (all
sixteen quiet each time), medians of eight:

| build | 1 KB basic / complex | ratio | 100 KB basic / complex | ratio | bound |
|---|---|---:|---|---:|---:|
| as first built: every `Complex` run shaped | 0.155 / 0.337 ms | **2.17×** | 5.338 / 19.376 ms | **3.63×** | 1.5× — **fired** |
| K81a's response: a run `needs_shaping` refuses takes fontdue | 0.158 / 0.153 ms | **0.97×** | 5.342 / 5.277 ms | **0.99×** | 1.5× — holds |

Attribution of the first row: on 100 KB of Latin, `Shaper::shape` took
12.7 ms with a one-face source or the stack's (the face lookup costs
nothing measurable), **rustybuzz alone 10.2 ms**, `unicode_linebreak`
0.21 ms, against fontdue's whole `Basic` measure at 3.8 ms. The shaper is
the cost, not its bookkeeping. Setup, after the response: the `complex`
rows report 0 runs through the shaper (the document is Latin) and 84 runs
painted. Injection: `needs_shaping` always answers yes; one quiet
invocation a side prints 0.158 / 0.346 ms and 5.599 / 20.077 ms, with 105
runs through the shaper — the ratio back at 2.2× and 3.6×. K3's
"keystroke + layout" row, computed first and unchanged: 0.068 / 5.213 ms
(median of 16), inside its spread. `Complex` stays opt-in; per-widget
shaping is reopened (`docs/plan-shaped-paint.md`).

### The gates

| gate | quantity | measured | bound | injection → measured |
|---|---|---|---|---|
| `shell/tests/paint_shaped.rs` Arabic pin (`Complex`) | painted ids left to right; ink against the box the stack measured | `[5256, 5260, 5277, 5288, 5341]` = the shaper's; box [10, 10, 84, 48], ink 13..83 × 16..49 | equal; inside ±1 px | `place_run_into` ignores the mode: painted `[1390, 1366, 1375, 1365, 1371]`, "the painter drew other glyphs than the shaper produced" |
| — Hebrew pin | `שלום`'s x per letter, logical order | **49.5, 31.3, 24.1, 2.9** | strictly falling | the same injection: 1, 24, 44, 53, "the painter drew Hebrew in logical order" |
| — `Basic` pin | painted ids on a `Basic` stack | `[1390, 1375, 1371, 1366, 1365]` = cmap, logical order | equal | — (the accept direction of both pins) |
| `basic_placements.rs`, `shaping` build | `BASIC_CHECKSUM` | **0x866444d1064ccda8** | equal | `Complex` branch whenever compiled: 0xa2f9db2d8e3371bd, scale 1 named first, 2,188 of 2,200 glyphs fractional |
| — on a `Complex` stack (K81a's response) | the corpus's checksum; runs through the shaper; a mixed paragraph's runs | **0x866444d1064ccda8**, **0** runs; `Taj Mahal, ` and ` quay j` identical to `Basic`, `שלום עולם` 1 run through the shaper and placed otherwise | equal; 0; identical / shaped | `needs_shaping` always yes: scale 1 0xa5087129ecf88821 against 0x5ed8be5616d27ffd |
| `glyph_placement.rs` on `Complex` | centring offset; ink rows against `Basic`'s at 16–64 px | −0.5 px; rows equal at all five sizes | ≤ 1 px; equal | cap shift dropped: rows one off at 32 px (107..130 against 108..131) and 64 px (113..160 against 114..161) |
| `text_containment.rs` on `Complex` | 18 strings (the corpus + 6 RTL, joined, mixed and a paragraph) × 10 sizes × 2 weights × scales 1, 1.25, 1.5, 2, `Host` label, measured with one stack and painted with a clone | 1,440 cases, **0** wrapped, worst overhang **1** px; mixed runs report both directions; the paragraph wraps in 80 of 80 (setup) | 0; ≤ 1 px | the `Complex` measure ignores `max_w`: **80** wrapped, worst overhang 286 px, every paragraph case named |
| — the right-to-left cut | the `…`'s ink x against the prefix's, three 120 px `Ellipsis` labels | Hebrew `…` 5..19, prefix 20.7..111.4; LTR-base mixed `…` 100..114 after 5.0..98.4; RTL-base mixed `…` 5..19, prefix 21.5..109.1; overhang 0 | leftmost / rightmost by base; 0 | `fit::rtl` answers false: both RTL `…` at x 97 and 95, "not the leftmost ink" |
| `line_height.rs`, `line_height` 12 `Label` on `Complex` | measured content height; painted line-start y | 43 (want 3 × 12 + (19 − 12)); 3, 15, 27 (scale 1) and 7, 31, 55 (scale 2) | ±1; pitch × scale ± 1 | the `Complex` branch ignores `line_height`: line starts 3, 22, 41, "line 2 painted 19 px below line 1, want 12" |
| `font_heap.rs` K81c | resident heap after one frame, kept stack + canvas | `Complex` **803,992 B** above `Basic`; a second `Complex` stack 44,112 B; `Basic` from the file = from bytes (+0) | file 759,720 + 64 KiB = 825,256; `Basic` ≤ from bytes + 64 KiB | the bytes kept under `Basic` too: `Basic` from the file 759,366 B over from bytes, the `Basic` bound fires; the older peak bound did not (22,045,185 under 23,966,104) |
| `alloc_gate.rs` `mod complex` | allocations per frame, after warm-up | combining-mark label **0** (2 shaped runs a frame, 0 bidi); plain Latin **0** (0 shaped); still RTL screen **0**; changing RTL label **960** = 80 bidi runs × 12 | 0 / 0 / 0 / bidi runs × `unicode_bidi`'s own count | a shape plan per call: the combining-mark arm prints 650 allocations in 50 frames |
| `lumen-static-wgpu/tests/parity.rs`, `just gpu` | `שלום world 123` on a `Complex` stack, px over 8 | **0** (worst 1), ink 355 both, 2 bidi runs (RTX 4070) | 2 (the text region's) | the GPU shapes with a `Basic` clone: **885** px, "the backends disagree on a shaped run" |
| `just release-window-shape` | `rustybuzz::`/`unicode_bidi::` symbols, release `minimal` | default **0**; with `lumen-static-winit/shaping` **310** | 0; > 0 | `shaping` a default shell feature: default 310, "a default release window links 310 shaper symbols" |
| `--bin grid --shaping basic\|complex` | the truetype pass's glyph cache, runs painted / through the shaper | identical under both: 28 misses, 0 resets, 909 runs painted, **0** through the shaper | — (K81b is T81.8's) | — |

### K2 and K81e

By K2's method: the shell 8 default, 16 `truetype`, **28** `shaping`;
`lumen-static-winit` 96 default, **104** with `shaping` — unchanged from
T81.2, since T81.3 adds no crate (`unicode-bidi` joins the shell as a
dev-dependency, the version the text crate already pulls). Stripped
release `examples/showcase`: 9,679,336 B → 10,428,296 B with
`lumen-static-winit/shaping`, **+748,960 B** (0.71 MiB) against K81e's
3 MB — holds.

## M81/T81.5 — caret, selection and hit test over bidi runs (2026-10-04)

`TextInput`, `TextArea` and `Selectable` place their caret, answer
`caret_rect`, hit-test a press and tint a selection through one module,
`lumen-static-widgets`' `caret`: an offset is the **last** cluster with
its byte (`caret::at`, which `TextInput::x_of` and `TextArea::cluster_at`
read), a press the nearest leading edge, a selection one rect per visually
contiguous span (`caret::spans`). `lumen-static-text`'s `Shaper` reports
a direction change as **two insertion points with one byte** — the ending
run's trailing edge, then the next leading edge — because no leading edge
is that boundary (`docs/plan-shaped-paint.md`, "What building found").
The tint colours are unchanged: `Selectable` keeps M80/T80.4's alpha 40,
the fields their 64, which is still unmeasured under text.

Gates in `crates/lumen-static-shell/tests/bidi_caret.rs` (`just fonts`):
pinned face, 16 px, a `Complex` stack from `from_static`, each on an
`Everything` and a `Boundaries<256>` host with identical output, every
input asserted handled:

| gate | quantity | measured | bound | injection → measured |
|---|---|---|---|---|
| typing `שלום 123` into an empty `TextInput` | caret x from the run's right edge, per keystroke | −12, −21, −26, −36, −41, −52, −62, −72 | strictly falling | `find(byte >= at)` in `TextInput::x_of`: −12, −21, −26, −36, −41, **−41.82, −41.64, −41.46** |
| — into an empty `TextArea` | the same | the same eight | strictly falling | `find(byte >= at)` in `TextArea::cluster_at`: the same three rising values |
| a `TextInput` over `abc שלום`, bytes 2..8 selected by keys | selected letters' ink columns covered; ink columns of unselected letters inside the tint; tint rects | **23/23**, **0**, **2** | all, 0 (±1 px), 2 | one rect from `x_of(start)` to `x_of(end)`: 7/23, **10** stray (columns 43..50, 54..55: `ם`, `ו`), 1 rect |
| — a `Selectable<String>`, the same bytes by a drag | the same | **23/23**, **0**, **2** | the same | the same one-rect injection in `tint_rows`: 7/23, **10** stray, 1 rect |
| — either, the shaper's pair removed | the same | — | — | `place_line` pushes no closing point: 23/23 but **10** stray in both (the space's span ends at the word's right end), 2 rects in the field and 1 in the `Selectable` |
| a press at the visual left end of `שלום` (line 1 of a two-line right-to-left `TextArea`) | the caret's offset | **8**, the `\n` (accept: its right end → 0) | 8; 0 | `offset_at` measures each cluster at its trailing edge: **6** |
| `caret_rect` at scale 2 against the painted caret column | \|centre − column\|, physical px: `TextInput` at byte 4 of `abc שלום` (a direction change), at the end of `שלום`, `TextArea` at line 1's end | **0.50**, 0.00, 0.00 (columns 37 px of a 37.2 px line) | ≤ 1 | `TextInput::caret_rect` with `find(byte >= at)` while paint looks up exactly: **70.31** at byte 4 (the Hebrew word's width), 0 elsewhere |
| `crates/lumen-static-text/tests/shaper.rs` mixed line | pairs (byte, trailing edge, leading edge) in `abc שלום def` | (4, **33.84**, 69.25), (12, **33.84**, 33.84) | bytes 4 and 12; both trailing edges equal, between the space's leading edge (28.76) and the word's (69.25) | no closing point pushed: **[]** |
| firmware | `.text` Δ against `3f5c7e3`, four binaries | **+860 / +844 / +860 / +860 B** | K4: 1,000 B on `firmware_interactive` | `docs/results-embedded.md` M81/T81.5 |

