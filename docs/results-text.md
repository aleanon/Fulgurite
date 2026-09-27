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
