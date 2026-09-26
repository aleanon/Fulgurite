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
| 9 | `lumen-static-shell/tests/text_motion.rs` | R10-F14 | **12** motions wrong of 12 | 0 |
| 10 | `lumen-static-winit/tests/key_text.rs` | R10-F8 | **0 / 4** multi-character key texts whole (`"^a"` → `"^"`) | 4 / 4 |
| 11 | `lumen-static-winit/tests/clipboard_shortcut.rs`, `clipboard.rs` | R10-F13 | **6 / 12** decisions wrong; `NumberInput` copy `None` | 0; its text |
| 12 | `lumen-static-shell/tests/compose_marks.rs` | R10-F15 | `NumberInput` refused `"12"` and `"ab"` alike | accept `"12"`, refuse `"ab"` |
| 13 | `lumen-static-shell/tests/textarea_scale.rs` | R16-F7 | plain 20→200 KB ratio **59.5**; substituted 2→20 KB ratio **93.2**; self-ratio 1.00 | 25 |
| 14 | `lumen-static-shell/tests/textarea_visible.rs` | R8-F3 | **53,595** glyphs rasterised for one Left in 64 KB | 330 (6 lines × 55 × 1 region) |
| 15 | `fixtures/embedded/tests/wrap.rs` | R10-F10 | closed early by T45.12 (plan header) | — |

Item 13's timings were taken under load (load average 10 on the build
machine; the self-ratio beside them is the check that load did not
spoil the ratio).
