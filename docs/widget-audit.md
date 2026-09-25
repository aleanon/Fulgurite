# Widget audit — 2026-09-25

For each of the showcase's 52 entries, we wrote down how the widget should
behave and then checked that against the live window, headless `--shot`
renders in both palettes, and the source. Nothing was fixed in this pass.
The evidence (renders, scripts, measurements) is in the session scratchpad
and is not committed. Every measurement below was taken at display scale
1.0, unless it says otherwise.

Verdicts: **broken** (the main job does not work), **flaws** (the main
job works but something a user would notice is wrong), **works**.

## The two reported bugs

**1. A button's border lights up one click late, on the previous button.**
Two causes contribute.

- **(a) Focus is published before it moves, at any scale.**
  - `pass::dispatch` writes `st.focused` at the start of the event, before
    the press, Tab or arrow changes the focus (`pass.rs:497-499`). Nothing
    writes it again afterwards.
  - After layout, the host zeroes the focus ring of the node that
    *currently* has focus (`input.focus`), unless focus is
    keyboard-visible (`lumen-static-shell/src/lib.rs:1235`).
  - So pressing B leaves A with `focused = true`, and A's ring is not
    zeroed: it paints on A.
  - The input audit saw the same one-event lag on Slider (Tab, then the
    ring is still absent while Right already moves the value) and on
    TextInput (the caret appears one event late).
- **(b) At display scale ≠ 1, damage is applied in logical pixels on a
  physical surface.** This is R1-F1, planned as M40 on `m38-deep-review`.
  Reproduced at `WINIT_X11_SCALE_FACTOR=2` with real XTest clicks: the
  store read `presses: 3` while the screen still showed `0`.

**Related:** the demo buttons show **no hover at all**. The showcase's
`Kind("button")` rule for Select's rows sets `.hover_dim(0.0)` on every
button (`examples/showcase/src/main.rs:1240-1250`). Primary's focus ring is
also white on a white card (see X4).

**2. Avatar initials are not centred.** `trim.rs:419-432` passes the full
box to `text_weighted`, which draws from the left edge. We measured the
initials 9.5–14 px left of the circle's centre, and the "J" hook reaches
outside the circle. `type_defaults` sets `align: Center`, but the seed has
no `ALIGN` field and `paint` never reads it.

## Defects that affect several widgets

| id | defect | widgets |
|---|---|---|
| X1 | The agent's audit calls `Node::probe`. Its default is `layout`, measured with `BlockMetrics`, and nothing calls `set_text_measure`. Widgets that store layout results keep the wrong ones, and paint uses them. | RichText, Markdown, RichTextEditor (paragraphs collapse, words split, click-to-caret dead), TextInput (caret always at end), Tabs/BottomNav/Rail (labels wrap mid-word); latent in a FILL Slider |
| X2 | A press a child does not handle bubbles to its container. Veil, SplitPane, Collapsible and PaneGrid treat it as a press on their own area, using the child's local coordinates (`pass.rs:731-751`). | Modal/Sheet/Drawer (panel press dismisses), SplitPane/PaneGrid (pane click moves divider), Accordion (body click collapses) |
| X3 | In a `Repeat`, the derived clean-subtree skip checks `cx.changed` with a row-local index, so rows keep their stale size. Read from the code; the garbling is measured. | DataGrid (Size column garbled after scrolling), any repeater whose rows change text length |
| X4 | `ops::focus_ring` fills a rect in `s.fg` under the widget. | Tabs (solid dark block), Slider/RangeSlider (solid blue bar), Primary button (white on white), checked toggles (no visible ring) |
| X5 | `Chooser::layout` reopens the list if the trigger was focused *before* the event, after a pick or dismiss has closed it. | PickList, Combobox stay open |
| X6 | Demo widgets have hard-coded light colours, and `chrome_text_meets_contrast` checks only the chrome. | Label (1.30:1 in dark), veil titles (1.3:1), Card, Chip, Divider, PaintBox, OptionList, NumberInput, SearchField, ProgressBar, Skeleton, pickers, BottomNav/Rail, Tree, DataGrid, Pagination, Accordion headers |
| X7 | A release activates whatever it lands on; pass 2 never checks that the press started on the same node (`pass.rs:774-806`). | every button-like widget |
| X8 | Many widgets have no keyboard path: they are not focusable, or have no arrow/Enter handling. | PickList, Combobox, Menu, Select rows, pickers, SplitPane, PaneGrid, Collapsible, Tree, Radio groups |
| X9 | Many widgets expose nothing to accessibility (`GenericContainer`, no value). | Badge, Chip, Avatar, Icon, Toast, charts, ProgressBar, Spinner, Skeleton |
| X10 | Nothing snaps positions to whole pixels, so 1 px lines render 2 px wide. | Divider, bordered buttons, icons |
| X11 | Arrow-key focus grouping runs before widgets see the keys (`pass.rs:528-556`). Latent: a TextInput next to a sibling control would lose Left/Right. | TextInput, Slider in rows |
| X12 | No widget has a disabled state. | all |

**Item (c) is closed.** Space does activate (`pass.rs:777`), but on
key-down, so auto-repeat fires again. The comment at `pass.rs:766-773` that
says otherwise is stale.

## Per widget

| # | widget | verdict | what is wrong |
|---|---|---|---|
| 0 | Label | flaws | 1.30:1 in dark (X6) |
| 1 | Badge | flaws | no accessible value; "circular" only by accident of glyph width |
| 2 | Chip | flaws | display-only, although the doc says selectable and removable; does not follow the palette |
| 3 | Avatar | **broken** | initials off-centre and spilling outside the circle; `align` is a dead setting |
| 4 | Icon | works | Play triangle striped; blurb says eight marks, and Search is missing |
| 5 | Divider | flaws | the 1 px hairline renders 2 px (X10); bright in dark |
| 6 | RichText | **broken live** | X1; the accessible text inserts spaces before punctuation |
| 7 | Markdown | **broken live** | X1; bullet dot 2.5 px high; no hanging indent |
| 8 | RichTextEditor | **broken live** | X1: caret always at the end, preview garbled; no field outline; no formatting UI |
| 9 | PaintBox | works | white in dark |
| 10 | Button | flaws | ring lags one event, on the wrong button (bug 1a); no hover (`hover_dim(0.0)` leak); Primary ring invisible; release-anywhere activates (X7); no disabled state |
| 11 | CheckBox | **broken in showcase** | the store flips but the drawn box never does: `on` is never bound or re-seeded (`catalog.rs:241`, `refresh`); label not clickable; unchecked box 1.75:1 |
| 12 | Switch | **broken in showcase** | same binding defect as CheckBox |
| 13 | Radio | **broken** | all three bound to one bool; "Blue" can never be selected; never repaints; no arrow-key group |
| 14 | Slider | flaws | focus ring fills the slider (X4); no Up/Down/PageUp/PageDown; no grab offset on the knob |
| 15 | TextInput | flaws | click-to-caret and drag-select dead when the agent is on (X1); no placeholder; no submit |
| 16 | ColorPicker | flaws | no keyboard, no focus ring, white in dark; a press on the pad's padding closes the picker |
| 17 | DatePicker | flaws | **paging the month changes the committed date** (`shift_month`); a press on the header title pages; day numbers left-aligned |
| 18 | TimePicker | flaws | picking the hour closes the popup, so a time needs two opens; no keyboard |
| 19 | FilePicker | flaws | no keyboard or focus ring; no scroll cap (latent at 64 rows) |
| 20 | PickList | **broken** | the trigger is editable free text; stays open (X5); no keys; Tab does not open it, although the doc says it does |
| 21 | Select | flaws | no row hover (showcase `hover_dim(0.0)`); current value not marked; keyboard cannot reach rows |
| 22 | Combobox | flaws | stays open (X5); no keyboard pick; empty sliver when nothing matches; the shrinking list leaves shadow pixels (damage excludes the shadow) |
| 23 | Menu | flaws | a command becomes a stored selection; role is ComboBox; no keys; white in dark |
| 24 | NumberInput | flaws | Up after typing discards the typed value; the showcase readout never updates |
| 25 | RangeSlider | **broken** | once collapsed, the low handle cannot move left again; the Tab arm is dead code; focus ring hides the control |
| 26 | SearchField | flaws | box-in-box from the showcase rule; icon 1.51:1 in dark; no Escape or clear; long text clipped with the caret mid-word |
| 27 | ProgressBar | flaws | no accessible value; track glares in dark |
| 28 | Spinner | works | no accessible busy role |
| 29 | Skeleton | flaws | bright slabs in dark; hard-edged shimmer |
| 30 | BarChart | flaws | exact values, but no axes, labels or accessibility |
| 31 | LineChart | flaws | accurate, but unlabelled; notched joins; stroke overhangs its box |
| 32 | PieChart | flaws | **1.35% gap at 12 o'clock** (the last segment is truncated); scalloped edge; no legend |
| 33 | Card | flaws | pure white in dark |
| 34 | Toast | flaws | no placement, timing or announcement; text not accessible |
| 35 | Tooltip | flaws | no delay; does not open on focus; not kept on screen; stays open on press, although the doc says it closes |
| 36 | Tabs | **broken** | focus fills the strip dark (X4); 1.41:1 in dark; labels wrap live (X1); no hover |
| 37 | BottomNav | flaws | labels wrap live (X1); light in dark |
| 38 | NavigationRail | flaws | labels wrap out of their pill live (X1); light in dark |
| 39 | AppBar | works | title contrast 4.499:1; no leading or action slots |
| 40 | PullToRefresh | **broken** | a wheel pull never springs back; drag does nothing; ignores whether content is at the top; draws outside its box |
| 41 | Modal | flaws | a press on the panel dismisses (X2); Escape needs focus inside (`Veil` lacks `Node::dismiss`); title 1.3:1 in dark |
| 42 | Sheet | flaws | as Modal; all four corners rounded; shadow spills |
| 43 | Drawer | flaws | as Modal; the scrim is invisible in dark |
| 44 | Wrap | works | lines are left-packed only |
| 45 | Grid | flaws, nearly broken | columns do not line up when cells differ in width; the demo hides this |
| 46 | SplitPane | flaws | a pane click jumps the divider (X2); not focusable; divider drifts up to 6 px from the pointer |
| 47 | Accordion | flaws | a body click collapses (X2); no keyboard; opening a section moves its header under the pointer |
| 48 | PaneGrid | flaws | a pane click moves the vertical divider (X2); keyboard reaches only one divider |
| 49 | Pagination | flaws | the number window moves under the pointer, contrary to its rustdoc; no first/last page with "…"; 4.12:1 |
| 50 | Tree | flaws | hover sticks after the pointer leaves; no keyboard or selection |
| 51 | DataGrid | **broken** | Size column garbled after scrolling (X3); no hover, selection or scrollbar |

**Totals:** 12 broken (3 of them only in the live window, 2 only in the showcase), 35 work
with flaws, 5 work.

## Gaps in the tests (why none of this was caught)

- **Drives read the store, not the pixels.**
  - CheckBox, Switch and Radio pass while never repainting.
  - Tabs passes on its `value` while its labels wrap.
- **Unit tests call `on_event` directly**, so they cannot see anything the
  pass does around it:
  - `escape_closes_an_open_chooser`;
  - `escape_requests_dismissal_too`;
  - X2, X5 and bug 1a.
- **The contrast gate covers only the chrome**, not demo content.
- **The drives never exercise these:**
  - pressing a panel's background or a pane's content;
  - keyboard focus rendering;
  - collapsing a range and dragging left;
  - stepping a NumberInput after typing into it;
  - scrolling a DataGrid past a text-length change.
