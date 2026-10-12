# M57 — fix what the widget audit found

Written 2026-09-25 from `docs/widget-audit.md`, a 52-widget audit of the
showcase. Five agents wrote a spec for each widget, then checked it against
the live window, headless renders in both palettes, and the source. The
audit found **12 widgets broken, 35 with flaws, and 5 working**. Most of the
defects trace back to about a dozen framework causes.

The rule for this plan: **a framework defect is fixed in the framework.**
Nothing here works around a framework defect inside `examples/showcase`.
The showcase changes only where the defect *is* the showcase, meaning its
own stylesheet or its own wiring. Where a widget's behaviour follows from a
missing framework capability (keyboard focus, disabled state, palette-aware
defaults), the capability is built once in the framework. Widgets do not
each grow their own.

Numbered M57, because M39–M55 are proposals already taken by the unmerged
`m38-deep-review` branch.

Every entry below has three parts:

- **Cause**: where the defect lives, with the file and line the audit
  measured or read.
- **Fix**: what changes.
- **Gate**: the test that proves it. Per AGENT.md, each gate is shown red
  under an injection that restores the defect, and the injection's result
  goes in the commit body.

"Read" marks a cause inferred from code rather than measured.

**Status: DONE 2026-09-27**, on branch `m56-showcase-design`. Every A, B,
C and D item landed; `docs/widget-audit.md` now carries a *fixed in* and a
*checked by* column, computed by `scripts/audit_fixed_in.py` from the
commit log and the drives rather than written. **What building it found**,
including where this plan was wrong, is the last section below.

---

## Phase A — framework defects

These come first because most per-widget symptoms disappear once they
land. Phase B must not start on a widget until the Phase A items it
depends on are in; the dependencies are named in each B entry.

### A1. Focus is published before it moves (the reported button bug)

- **Symptom.** Clicking button B lights up A's focus ring. The ring and
  caret lag one event (Button, Slider, TextInput).
- **Cause.**
  - `pass::dispatch` writes `st[i].focused` at the start of the event
    (`crates/fulgurite/src/pass.rs:497-499`). The press, Tab or arrow
    then moves `*focus`, and nothing writes it again.
  - The host then zeroes the ring on `input.focus`, which is the *new*
    node (`crates/fulgurite-shell/src/lib.rs:1235`).
  - So the old node keeps `focused = true` with its ring intact, and
    paints it.
- **Fix.** Publish focus **after** dispatch has finished moving it, once,
  at the end of `dispatch` (and of the overlay path at `pass.rs:269`). Mark
  the old and new focus nodes dirty, so `Boundaries` repaints both.
- **Gate.** A new `tests/focus_publish.rs`:
  - Press A, then press B, then read `st[..].focused` after the frame:
    exactly B is focused.
  - Tab once: the ring is on the first stop in *that* frame.
  - Drive the same sequence in `just widget-drive` by pixels, under both
    strategies.
  - Injection: move the publish loop back to the top of `dispatch`.

### A2. Damage in logical pixels on a physical surface (R1-F1, scale ≠ 1)

- **Symptom.** At display scale 2, the screen showed `presses: 0` while
  the store held 3 (reproduced with XTest clicks).
- **Cause.** Damage is recorded inside `Scaled`, so it is in logical
  pixels, but applied to the physical surface. Recorded as R1-F1 in
  `results-deep-review.md` (on that branch) on `m38-deep-review`.
- **Fix.** Use the M40 fix from `plan-m38-fixes.md` (on `m38-deep-review`): record damage
  in physical space, by wrapping `Scaled` outside `Recording`. Snap each
  region outward to whole physical pixels. If M40 lands first, this entry
  closes on it; do not implement it twice.
- **Gate.** A headless host at scale 2:
  - Click, then compare a partial repaint against a full repaint of the
    same frame, byte for byte.
  - Repeat at scale 1.5 to cover fractional snapping.
  - Injection: put `Recording` back outside `Scaled`.

### A3. The agent's audit leaves estimated layout behind (X1)

- **Symptom.** With `FULGURITE_AGENT` set:
  - RichText, Markdown and the editor preview collapse and split words.
  - Click-to-caret is dead in TextInput and RichTextEditor.
  - Tab, BottomNav and Rail labels wrap mid-word.
  - `W1005` reports false findings.
- **Cause.**
  - `audit.rs:525` calls `Node::probe` on every leaf, inside 1e6 and 2e6
    px boxes. `Node::probe`'s default is `self.layout(..)`
    (`widget.rs:259-268`).
  - The measure is `BlockMetrics`, because nothing calls
    `Agent::set_text_measure`.
  - Widgets that cache layout results keep the probe's results: `placed`
    in RichText, `clusters` in TextArea and TextInput, `widths` in
    `Strip`, `track_w` in Slider. Paint then reads them on every frame
    that does not re-lay the widget out.
  - This is `Repeat`'s T27.10 defect again. That time it was fixed by one
    override, which left the shape in place for the next widget.
- **Fix.** Two parts, both in the framework.
  1. **The audit restores what it disturbs.** After probing a leaf, the
     audit lays the leaf out again under its real `st[i].constraints`,
     with the real measure. Any widget whose cached state is a function
     of its limits is then left as the frame found it, with no
     per-widget override. `Repeat`'s override stays, because its probe
     must not clamp scroll.
  2. **The audit measures with the real face.**
     `fulgurite-winit`'s agent setup, and `AgentHost`, install the
     host's own `TextMeasure` through `set_text_measure`. `W1005` then
     judges what is drawn.

  Update the rustdoc on `Node::probe` to name the rule:
  **a probe may not change what the next paint draws.**
- **Gate.** An agent-enabled headless `AgentHost` with RichText,
  TextInput and a `Strip`:
  - Run a frame with the audit, then paint.
  - Compare against a frame from a host without the agent: byte-identical.
  - Also check that TextInput's `offset_at` of a mid-text point is less
    than the text's length.
  - Injection: remove the restoring layout.
  - Second gate: `W1005` count on the showcase's blurb with the real
    measure is 0. The injection is `BlockMetrics`.

### A4. A bubbled press reaches the container in the child's coordinates (X2)

- **Symptom.** These containers act on presses meant for their children:
  - a press on a Modal, Sheet or Drawer panel asks to dismiss it;
  - a click in a SplitPane or PaneGrid pane moves a divider;
  - a click on an open Accordion section's body collapses it.
- **Cause.**
  - Dispatch pass 1 walks from the hit node to each ancestor
    (`pass.rs:731-751`) and never re-derives `ctx.local`. The comment at
    `pass.rs:728` says only the hit knows it.
  - Each container then treats an unhandled press from a child as a
    press on its own chrome:
    - `Veil::on_event`: "anything landing here missed it".
    - `SplitPane`: `split.rs:245`.
    - `Collapsible`: `split.rs:547`, where `y ≤ header_h`.
    - `PaneGrid`: `divider_at(..).unwrap_or(0)`.
- **Fix.**
  1. **Framework.**
     - `dispatch` recomputes `ctx.local = pos − st[i].pos` for every node
       it visits. `st` is in scope; line 595 already does this for the
       hit.
     - `EventCtx` gains `target: u32`, the node actually hit, so a
       container can ask "was this press on me?" (`ctx.target ==
       ctx.node`).
     - Pass 2 does the same.
  2. **Each container claims only its own area.**
     - `Veil` dismisses only when the local point is outside the panel's
       rect.
     - `SplitPane` claims only presses inside its divider's grab zone.
     - `Collapsible` claims only presses inside its header's rect.
     - `PaneGrid` declines when `divider_at` is `None`, instead of
       dragging divider 0.
- **Gate.** One test per container in its crate's tests:
  - Press a non-handling Label inside the panel, pane or body.
  - `pump` answers `handled == false`, and the state is unchanged
    (dismissal count, split fraction, open flag).
  - Assert that the press landed on the label (`ui.explain` target), so
    the setup cannot be vacuous.
  - Other direction: press the scrim, divider or header, and it acts.
  - Injection: drop the `ctx.local` recompute. All four fire.

### A5. Rows in a `Repeat` skip layout on the main table's dirty set (X3)

- **Symptom.** In DataGrid, "10 KB" to "14 KB" stay in the 37 px boxes
  laid out for "1 KB", so the Size column garbles after scrolling.
- **Cause (read).**
  - The derive's clean-subtree skip
    (`fulgurite-macros/src/lib.rs:953-958`) asks
    `cx.changed.is_clean(i..i+N)`.
  - `Repeat::layout` lays each row out at local index 0
    (`repeat.rs:448`), so the question is asked about the main table's
    first nodes, which are almost always clean.
  - A row whose bind rewrote its text keeps last frame's size.
- **Fix.**
  - `Repeat::layout` lays its window out under a `LayoutCx` whose
    `changed` is `Changed::ALL`, the value the shadow oracle already
    uses. The row index space is not the table's, so no set indexed by
    the table can answer for it.
  - The cost is bounded by the window: about 9 rows here.
  - Record the cost on the `grid` bench arm in
    `docs/results-damage-precision.md`, beside T29.5's number.
- **Gate.** A new `tests/repeat_rebind.rs`:
  - A repeater whose bind writes texts of different lengths per row.
  - Scroll one row, then assert that each row's label width equals its
    text's measured width.
  - Injection: pass the parent's `changed` through.
  - Also add an arena-row pass to `ui.lint`, so `W1005` can see arena
    rows. The audit found the lint blind to them.

### A6. The focus ring is a filled underlay in the text colour (X4)

- **Symptom.**
  - A focused Tabs strip becomes a solid dark block.
  - Slider and RangeSlider become solid blue bars.
  - Primary's ring is white on a white card.
  - A checked toggle's ring is the same blue as its body.
- **Cause.** `ops::focus_ring` (`crates/fulgurite/src/ops.rs:59`)
  fills a rect 2 px larger than the node, in `s.fg`, under the widget. It
  relies on every widget painting an opaque full-box body over it. The
  palette's `focus` role exists and is never used.
- **Fix.**
  - **The host draws the ring, once, after the tree.** There is one
    focus, so one ring: a `stroked_rect` 2 px wide, offset 2 px outside
    the node with a transparent gap, rounded to `radius + 4`, in
    `palette.focus`.
  - It is drawn on the deferred (overlay) layer, so a neighbour cannot
    cover it.
  - The host adds the old and new ring rects to damage when focus or
    `focus_visible` changes.
  - Widgets stop calling `ops::focus_ring`.
  - `Style::focus_ring` stays as the per-widget width, where 0 opts out.
    This also removes A1's zeroing hack in `Host::frame`, since the host
    reads `input.focus` directly.
  - The ring must still be reported to the agent, so tests can see it.
- **Gate.** In `tests/focus_ring.rs`, for a Button, a Slider, a checked
  CheckBox, a Tabs strip and a transparent-bodied widget:
  - Tab-focus each one, render, and assert the ring's pixels have ≥ 3:1
    contrast against both the control and the page, in both palettes.
  - Assert that the pixel inside the control is unchanged from the
    unfocused render (not filled).
  - Injection: restore the fill in `s.fg`.

### A7. A choice or dismissal reopens the list in the same layout (X5)

- **Symptom.** After a choice or Escape, PickList and Combobox stay open.
- **Cause.**
  - `Chooser::layout` (`crates/fulgurite-widgets/src/choose.rs`
    ~505–572) first applies `take_picked()` or `dismiss()`, which closes
    the list. It then runs
    `if st[trigger].focused { list.open = true }`.
  - `focused` is the pre-event value (A1's defect), so the list reopens.
  - Opening from focus in *layout* at all is the underlying mistake:
    opening is an event, not a state to re-derive every frame.
- **Fix.**
  - Open on the transition only. The `Chooser` receives focus-gained as
    an event: `Event::Focus { gained: bool }`, dispatched by the pass
    when focus moves. That is a framework addition, and A1 is where
    focus moves are known.
  - Delete the per-frame `focused` check from `layout`.
  - This also gives PickList its documented "Tab opens it" behaviour
    (B20) and Tooltip its "opens on focus" behaviour (B35).
- **Gate.** Through `pump`, not `on_event`:
  - Open, pick, then frame: the list is closed and stays closed after
    two more idle frames.
  - Same with Escape, and with a press outside.
  - Assert that the pick landed (the value changed) before asserting
    that the list closed.
  - Injection: restore the `focused` reopen.
  - Delete `escape_closes_an_open_chooser`'s direct `on_event` form, or
    keep it beside the pumped one with a comment saying why it is not
    enough.

### A8. Demo content ignores the palette (X6)

- **Symptom.** These keep light colours in dark mode:
  - Label (1.30:1) and veil titles (1.3:1), which become unreadable.
  - Card, Chip, Divider, PaintBox, OptionList, NumberInput, SearchField,
    ProgressBar, Skeleton, pickers, BottomNav/Rail, Tree, DataGrid,
    Pagination and Accordion headers stay white or light, and glare.
  - Tabs text (1.41:1) and SearchField's icon (1.51:1) nearly vanish.
- **Cause.**
  - `type_defaults` are `const fn` colours. The palette reaches a node
    only through a stylesheet `color_role`/`background_role` rule
    (M25/T25.11).
  - The widgets' own rustdoc admits the palette cannot reach `const fn`
    defaults. So every application has to restyle every widget kind, and
    the showcase did so for four.
  - `chrome_text_meets_contrast` checks only the chrome.
- **Fix.**
  - **Palette roles in type defaults.** `Style` already resolves
    `*_role` fields at cascade (`style.rs:809-821`). `type_defaults`
    sets them the same way a rule does:
    - `color_role: Some(Role::Text)`
    - `background_role: Some(Role::Surface)`
    - `border_color_role: Some(Role::Muted)`
  - The seed mask carries them, so an application's rule still overrides
    them. A default is then palette-aware with no sheet rule.
  - Add the roles the audit found missing:
    - `Track` (an unfilled slider or progress track)
    - `Scrim`
    - SurfaceRaised (a card or panel on a surface)
    - `Placeholder` (skeleton)
    - `Danger` (badge)
    - OnDanger
  - Give them values in `Palette::light` and `Palette::dark`, chosen
    against WCAG:
    - text ≥ 4.5:1;
    - non-text UI ≥ 3:1 (WCAG 1.4.11), which also fixes the unchecked
      box, the off track and the unselected ring at 1.75:1 (B11–B13),
      the field border at 2.2:1 (B15), and the light tracks at
      1.23–1.38:1 (B14, B25, B27);
    - AppBar 4.499:1 and Pagination 4.12:1 raised to ≥ 4.5:1.
  - Every widget in `fulgurite-widgets` moves its hard-coded colours
    to roles. Colours that are *data* stay literal: chart series and
    colour-picker swatches.
- **Gate.**
  1. A framework test, `tests/palette_defaults.rs`:
     - For every widget kind, cascade with an empty sheet under both
       palettes.
     - Every text/fill pair each kind paints (read from `Cmd`s) meets
       its bound: 4.5:1 for text, 3:1 for UI.
     - Print each ratio beside its bound.
     - Injection: one kind's default back to a literal. The test names
       it.
  2. The showcase's contrast gate is widened from the chrome to **every
     demo**, in both palettes, reading the painted `Cmd`s.
     - Injection: a black label rule.

### A9. A release activates whatever it lands on (X7)

- **Symptom.**
  - Pressing Primary and releasing on Default activates Default.
  - Pressing the card and releasing on a button activates the button.
- **Cause.** Pass 2 (`pass.rs:774-806`) runs the handler of whatever the
  `PointerUp` hit. It never checks that the press started there.
- **Fix.**
  - `InputState` records the node a `PointerDown` hit.
  - Pass 2 activates on `PointerUp` only if the up's hit is that node or
    one of its descendants (the handler walk already climbs to it).
  - A drag off and back on still activates, as on every desktop
    toolkit.
- **Gate.**
  - Press A, release B: `handled == false` and the count is unchanged.
  - Press A, release A: activates.
  - Press A, drag out and back, release A: activates.
  - Injection: remove the check.

### A10. Key activation on key-down repeats (X7, rest)

- **Symptom.**
  - Holding Enter or Space re-fires activation.
  - The comment at `pass.rs:766-773` still says Space does not activate.
    It does, so the audit's item (c) is stale.
- **Cause.** Activation matches `Key { down: true }`, and auto-repeat
  sends repeated downs.
- **Fix.**
  - Space activates on key-up, the web convention.
  - Enter activates on key-down but ignores repeats. This needs a
    `repeat: bool` on `Event::Key`, set by winit from `KeyEvent::repeat`.
  - Rewrite the stale comment.
- **Gate.**
  - A repeated Enter down fires once.
  - Space fires on up, not down.
  - Injection: drop the `repeat` check.

### A11. Arrow-key focus grouping runs before widgets see the key (X11)

- **Symptom (latent).** A TextInput or Slider in a row with a sibling
  control loses Left and Right to focus movement.
- **Cause.** Arrow grouping (`pass.rs:528-556`) runs before pass 1
  whenever the parent has more than one focusable child.
- **Fix.**
  - Offer the arrow to the focused node first (pass 1).
  - Group-move only if it declines.
  - A Button declines arrows; a TextInput, Slider or Tabs consumes them.
- **Gate.**
  - A row of [TextInput, Button]: Left inside the input moves the caret,
    and focus stays put.
  - A row of two Buttons: Right moves focus.
  - Injection: grouping first.

### A12. Many containers cannot take keyboard focus (X8)

- **Symptom.**
  - SplitPane, PaneGrid, Collapsible, Tree, ColorPicker's pad and
    Select's rows cannot be reached by Tab.
  - Their keyboard code (`split.rs:254`, Collapsible's Enter arm) is
    dead.
- **Cause.** Focusability is a `NodeDesc` flag (`FOCUSABLE`). These
  widgets are declared `CONTAINER` with no flag, so `dispatch` never
  stops on them.
- **Fix.**
  - Let a container be focusable: `NodeDesc::CONTAINER | FOCUSABLE`.
    Check that Tab order, A11 and A6 handle a focused container.
  - Each widget sets the flag on the node that owns its keys:
    - SplitPane: the divider.
    - Collapsible: the header.
    - Tree: the tree.
    - PaneGrid: each divider.
    - Picker: the pad.
  - Popup contents get **focus-in on open** (the framework moves focus to
    the popup's first focusable) and **restore on close**. This covers
    Select, the Choosers, Pickers, Menu and Veil (B41).
- **Gate.** A `widget-drive` keyboard pass per widget, driven by Tab and
  keys only (no pointer), which must reach every action in the widget's
  inventory that has a keyboard form. Coverage is asserted as data, as
  M27 did for pointer actions. Injection: clear one widget's flag.

### A13. A veil's Escape needs focus inside it

- **Symptom.** On a freshly opened Modal, Sheet or Drawer, Escape returns
  `handled: false`.
- **Cause.** The overlay Escape path (`pass.rs:269`) asks `Node::dismiss`
  (`widget.rs:196`), and `Veil` does not override it.
- **Fix.**
  - Implement `Veil::dismiss` to raise its dismissal request.
  - Audit every overlay widget for the same override: Tooltip (B35),
    Picker, Chooser and Select.
  - A12's focus-in on open makes Escape land either way.
- **Gate.** Through `pump`: open, Escape, and the dismissal count is +1
  with nothing focused beforehand. Injection: remove the override.

### A14. Damage misses a popup's shadow

- **Symptom.** A shrinking Combobox list leaves 1–3 px ghost shadow rows.
- **Cause.** Damage covers a node's box, but `shadow` paints up to
  `shadow` px outside it.
- **Fix.**
  - The journal's box for a node with `shadow > 0` is its paint bounds:
    the box inflated by the shadow.
  - The same inflation applies wherever `ui.lastDamage` and the
    renderer's damage are computed.
- **Gate.**
  - Shrink a shadowed popup, then compare the partial repaint against a
    full repaint: byte-identical.
  - Injection: the plain box.

### A15. Nothing snaps positions to whole pixels (X10)

- **Symptom.**
  - The 1 px Divider draws 2 px wide.
  - Default's border smears to 2 px at 1.21:1.
  - The Stop icon renders 13×12.
- **Cause.** Centring produces `.5` origins, and the software renderer
  rounds rect edges outward (`software.rs:163-164`).
- **Fix.**
  - `place` rounds each node's origin to the device pixel grid (logical
    × scale, rounded, ÷ scale). Sizes are left alone, so layout is
    unchanged.
  - Check GPU parity (`just gpu-parity`), because both backends must see
    the same snapped origins.
- **Gate.**
  - A centred 1 px Divider renders exactly one row at scale 1 and two at
    scale 2.
  - Injection: remove the snap.
  - Record the golden-image churn in the commit.

### A16. No disabled state (X12)

- **Symptom.** No widget can be disabled.
- **Cause.** There is no field or flag.
- **Fix.**
  - `Style.enabled: bool`, default true, is cascaded and published to
    `NodeState` as one bit. Check it against `NodeState`'s size budget
    (M15 policy); if there is no spare bit, measure the cost.
  - `dispatch` skips disabled nodes for hit, focus and activation.
  - `ops` gives a common disabled look: `Role::Muted` text and
    `Role::Control` body.
  - The agent publishes `disabled` through AccessKit.
- **Gate.**
  - A disabled Button: a press returns `handled == false`, Tab skips it,
    and the agent reports it as disabled.
  - Injection: ignore the bit in hit testing.

### A17. Accessibility values missing across widgets (X9)

- **Symptom.** These publish `GenericContainer` with no value: Badge,
  Chip, Avatar, Icon, Toast, the three charts, ProgressBar, Spinner and
  Skeleton.
- **Cause.** None of them implements `observe`.
- **Fix.**
  - Every widget in `fulgurite-widgets` implements `observe`.
  - Add a static check to `just static`: a `Widget` impl in the crate
    without `observe` fails. This is the same shape as M35's required
    table-driven walk, so the walk is extended rather than a check
    duplicated.
  - Roles:

    | Widget | Role and value |
    |---|---|
    | Badge, Chip, Avatar | `Label`, with the text as value |
    | Icon | `Image`, with the symbol name |
    | Toast | `Status` (a live region) |
    | Charts | `Figure`, value summarised: "10 bars, max 10" |
    | ProgressBar | `ProgressIndicator`, numeric |
    | Spinner | `ProgressIndicator`, busy, no value |
    | Skeleton | `busy`, hidden from reading |
    | Menu | `Menu` / `MenuItem` (B23) |
    | Tree | tree items (B50) |
    | RangeSlider | both bounds (B25) |

- **Gate.** One test walks every showcase entry through `ui.getTree` and
  asserts no leaf reports `GenericContainer` with no value. It prints
  the offenders. Injection: remove one `observe`.

### A18. Popups are not kept on screen

- **Symptom.**
  - A Tooltip or popup near the window's right or bottom edge goes
    off-screen.
  - Long option lists have no cap. FilePicker grows by 26 px per row, to
    64 rows.
- **Cause.** `Popup::place` (`popup.rs:150-175`) always places `Below`,
  with `max_h = INFINITY`.
- **Fix.**
  - Place against the window, flipping and then clamping:
    - `Below` flips to `Above` when below does not fit.
    - The x position is clamped into the window.
  - `max_h` is the larger of the room above and below. Over that, the
    content scrolls (OptionList moves to `Repeat` or gains wheel
    scroll).
- **Gate.**
  - A popup anchored 20 px from the bottom opens above.
  - One anchored at the right edge is clamped inside the window.
  - A 40-option list fits in the window and scrolls with the wheel.
  - Injection: always `Below`.

### A19. The Grid lowering does not share column x (the Grid defect)

- **Symptom.** With unequal cells, the columns are jagged.
- **Cause.**
  - A grid is the wrap lowering with a break every N children
    (`fulgurite-macros/src/lib.rs:403-406`).
  - `place` advances `cur += s.w + spacing` separately on each line
    (`lib.rs:689-715`).
- **Fix.**
  - A grid lays its columns out as columns. Each column's width is the
    max over its cells, or `container_w / N` for FILL cells.
  - Every cell is placed at its column's x.
  - `row_gap` and `column_gap` become separate axis parameters (the
    `spacing` default sets both).
  - The wrap axis gains `align = start | center | end` for its lines,
    the Wrap entry's off-centre complaint.
- **Gate.** A new `tests/flow.rs` case:
  - A 3-column grid with cells of widths 20, 60 and 40 in each row.
  - Every cell in column k shares an x.
  - Injection: the wrap `place` path.
  - This test is the one the audit said `flow.rs` lacked: its cells were
    all equal.

---

## Phase B — per-widget defects

Each entry names the Phase A items it depends on. Where an A item fixes
the whole symptom, the entry says only "closes on A*n*", and its gate is a
render check of that widget.

### Text

- **B0 Label.** Unreadable in dark mode. Closes on A8.
- **B1 Badge.**
  - **Cause:** the rustdoc claims "a one- or two-character count comes
    out circular", but nothing sets `min_w = h` (`trim.rs`).
  - **Fix:** `min_w = height` in layout, which makes the claim true.
  - Accessibility closes on A17.
  - **Gate:** "3" and "12" give w ≥ h, and "3" gives w == h.
- **B2 Chip.**
  - **Cause:** the rustdoc advertises selected and removable, but the
    widget is `nohit` with no handlers.
  - **Fix:**
    - `selected: bool`, a controlled field following `Toggle`'s
      precedent.
    - An optional remove affordance, a trailing × part.
    - Chip becomes hittable, activates like a Button, and reports
      remove through a second handler selector (the `inner` part).
  - Palette closes on A8; accessibility on A17.
  - **Gate:** `widget-drive` inventory rows for select and remove.
- **B3 Avatar (the reported bug).**
  - **Cause:** `trim.rs:419-432` passes the full box to `text_weighted`,
    which draws from the left. `align: Center` is set in `type_defaults`
    but is not in the seed mask, and `paint` never reads it.
  - **Fix:**
    - Measure the initials in layout and cache them (like `StatusChip`).
    - Paint them centred at `(box − measured) / 2` on both axes.
    - Clip to the circle.
    - Delete the dead `align` default.
  - Framework follow-up: add a rustdoc line on `Painter::text_weighted`
    saying that the rect is a *box drawn from its origin*, not an
    alignment frame. That comment is what misled this widget.
  - **Gate:** a render test with the ink box's centre within 1 px of the
    circle's centre for "K", "JD" and "ÅB". Injection: the old full-box
    call.
- **B4 Icon.**
  - The Play triangle is striped: it is filled with vertical lines.
    **Fix:** fill it as a polygon, or with lines on a 1 px grid, after
    A15.
  - The blurb says eight marks and there are nine: add Search to the
    demo.
  - Accessibility: A17.
- **B5 Divider.** Closes on A15 (the 2 px line) and A8 (too bright in
  dark). Correct the rustdoc's "antialiased 1 logical pixel" claim to
  what A15 makes true.
- **B6 RichText.**
  - Live breakage closes on A3.
  - **Cause:** the accessible text joins segments with spaces, ignoring
    `span.space` (`rich.rs` ~290).
  - **Fix:** insert a space only where `span.space` is set.
  - **Gate:** the observed value is "bold, code." for the demo's
    markdown.
  - Also cache word measurements keyed on (text, size, weight), like
    `FoldedText`, since every layout re-measures every word.
- **B7 Markdown.**
  - Live breakage closes on A3.
  - The bullet dot sits 2.5 px high. **Fix:** centre it on the line's
    x-height from the face metrics, not the box.
  - Wrapped bullets have no hanging indent. **Fix:** continuation lines
    start at the bullet's text x.
  - **Gate:** a render check of both.
- **B8 RichTextEditor.**
  - Live breakage closes on A3.
  - No field outline: give TextArea's `type_defaults` a border and
    surface through A8's roles, and a focus ring through A6.
  - No formatting UI:
    - Ctrl+B and Ctrl+E (`Event::Key` with `mods`, already tracked by
      `dispatch`) toggle `**` and `` ` `` around the selection.
    - The agent's `input.key` gains a `mods` field so this can be
      driven.
  - Handle End, which is currently unhandled.
- **B9 PaintBox.** The white default closes on A8. The clip is
  rectangular while the background is rounded: clip to the rounded rect,
  or document that the clip is the box.

### Input

- **B10 Button.**
  - The late ring closes on A1 and A6; release-anywhere on A9; key
    repeat on A10; disabled on A16.
  - No hover is **the showcase's stylesheet** (C1).
  - `hover_dim` defaults to 0.07, which the audit found weak. Raise the
    default to a contrast-based shade: at least 1.2:1 against rest.
    Judge it on the same gate as the pressed shade, which already
    measures 1.39–1.53:1.
- **B11 CheckBox, B12 Switch.**
  - The drawn state never changes. This is **showcase wiring** (C2): the
    widget is controlled and the demo never binds `on`.
  - Framework parts:
    - Clicking the label toggles it: the toggle's hit box includes its
      label run.
    - Hover and press feedback when on: `record_painted` dims `fg` too
      for a toggle's on body.
    - Contrast closes on A8's `Control` role at 3:1; the focus ring on
      A6.
  - Switch: centre the label on the track by the same measurement as B3.
  - **Gate:** a `widget-drive` pixel assertion that the box/knob region
    changes on toggle. The drives currently read only the store.
- **B13 Radio.**
  - **Cause:** there is no group concept (the `toggle.rs` doc defers it
    to the application), and the showcase binds three radios to one
    bool (`catalog.rs:317`).
  - **Fix (framework):**
    - `Radio<T>` is controlled by `selected == value`, a pattern shared
      with iced's `radio(label, value, selected, on_select)`.
    - Arrow keys inside a run of sibling radios move the selection, not
      just focus, per A11.
    - The showcase binds it properly (C2).
  - **Gate:** click Blue and Blue is selected; Down moves the selection;
    the dots repaint.
- **B14 Slider.**
  - The ring closes on A6, and the latent `track_w` probe clobber on A3.
  - Add Up/Down (±step) and PageUp/PageDown (±10 steps). The agent's
    key list gains PageUp and PageDown.
  - Keep the grab offset when pressing the knob: no jump on press.
  - Map the pointer over `[knob/2, w − knob/2]` so the ends are
    reachable.
  - Change notification: `Slider::on_change` as a handler selector, like
    Button's, so applications stop polling in `after_update`.
- **B15 TextInput.**
  - Click-to-caret closes on A3, the late caret on A1, and border
    contrast on A8.
  - Add a placeholder (`placeholder: &'static str`, drawn in
    `Role::Muted` while empty).
  - A submit handler on Enter.
  - The shape key includes `max_w`, so every probe re-shapes: after A3
    the probe restores, but key on the content width only.
  - The end-of-text caret: `x_of`/`caret_x` return the *start* of the
    last cluster at `len` (`input.rs:361-380`). **Fix:** at `len`,
    return the end of the last cluster.
  - Word movement (Ctrl+Left/Right) and double-click select word.
  - **Gate:** a caret at the end sits right of the last glyph, and a
    placeholder shows only while empty.
- **B16–B19 Pickers (shared `Picker<KIND>`).**
  - Keyboard: A12 (a focusable pad) and A7 (open on focus-gain; Space
    opens too).
  - Focus ring: A6, since `Picker::paint` never called `focus_ring`.
  - Palette: A8. Popup placement: A18.
  - **Cause:** a press on the pad's own padding or weekday strip closes
    the popup. The unhandled `PointerUp` falls through to
    `Picker::on_event`, which toggles `open` (`pick.rs:797-800`).
  - **Fix:** only the trigger toggles, checked with A4's `ctx.target`.
  - The trigger gains `HOVER_DIM`, `CURSOR` and a chevron.
  - The selected-swatch ring uses `Role::Focus`-contrast, not `s.fg`.
- **B17 DatePicker.**
  - **Cause:** paging commits a new date. `shift_month` mutates the
    committed `year/month/day` (`pick.rs:268-273`).
  - **Fix:** a separate `view_year/view_month` for the shown month,
    committed only by a day press.
  - **Cause:** a press on the header title pages the month (half-width
    split, `pick.rs:519-523`).
  - **Fix:** explicit ‹ › button rects; the title does nothing.
  - Centre the day numbers in their cells.
  - **Gate:** page forward and the value is unchanged; pick and it
    commits. Injection: the old `shift_month`.
- **B18 TimePicker.**
  - Picking the hour closes the popup. **Fix:** the hour press moves to
    the minute column, and only the minute press commits and closes.
  - Cell label padding: centre the label.
- **B19 FilePicker.** The long list closes on A18 (cap and scroll).
  Centre the trigger text vertically (B3's measurement).

### Choice and fields

- **B20 PickList.**
  - The list staying open closes on A7.
  - **Cause:** the trigger is a `TextInput`, so the "picker" is a
    free-text field (`Applexy`), and tests/choose.rs:221 claims
    otherwise.
  - **Fix:**
    - PickList's trigger becomes a read-only label with a chevron.
      Combobox keeps the TextInput.
    - Keyboard: Down/Up move a highlight, Enter picks, and type-ahead
      jumps to the first match.
    - Mark the current option (tint plus check) when reopened.
    - Palette: A8. Placement and scroll: A18.
  - **Gate:** typing into a PickList changes nothing; Down, Down, Enter
    picks the third option.
- **B21 Select.**
  - No row hover is the showcase's `hover_dim(0.0)` (C1).
  - Framework parts:
    - Mark the current value when reopened.
    - Keyboard: rows reachable (A12's focus-in), Down/Up/Enter.
  - The panel's doubled bottom edge (a hard-cut shadow): the shadow
    must fade over its whole extent. Check the software shadow's falloff
    at the panel's bottom.
- **B22 Combobox.**
  - Staying open closes on A7; ghost shadow on A14.
  - Arrow and Enter pick the highlighted or first match.
  - A "No matches" row in place of the 12 px empty panel.
  - An abandoned query reverts to the selected option's text on blur.
- **B23 Menu.**
  - **Cause:** Menu is a `Chooser` kind, so choosing stores a selection.
  - **Fix:**
    - Choosing fires an action (a handler per item index through `ctx.row`)
      and does not store a value.
    - Role `Menu` with items `MenuItem` (A17).
    - Keyboard as in B20.
  - The demo gets a trigger button to reopen it (C3).
- **B24 NumberInput.**
  - **Cause:** Up after typing discards the typed value. `nudge` adds to
    the committed `self.value` (field.rs).
  - **Fix:** `nudge` first parses and commits the current text, then
    steps.
  - Snap to step on commit (2.25 with step 0.5 becomes 2.5), or document
    that it doesn't.
  - Wheel stepping while focused.
  - The readout never updates: showcase wiring (C4).
  - **Gate:** type 3, press Up, and get 3.50. Injection: the old
    `nudge`.
- **B25 RangeSlider.**
  - **Cause:** a collapsed range cannot reopen to the left. `nearest()`
    always picks the high handle.
  - **Fix:** when collapsed, pick by the press side: left of the handles
    takes low, right takes high.
  - **Cause:** the Tab arm is dead, because `dispatch` consumes Tab.
  - **Fix:**
    - Make the two handles two focusable parts, so Tab moves between
      them.
    - Draw the active handle with a ring or a distinct shade.
    - Remove the dead arm.
  - The ring closes on A6, and the track contrast on A8.
  - Publish both bounds (A17).
  - **Gate:** collapse, drag left, and low decreases; Tab, Right moves
    the high handle.
- **B26 SearchField.**
  - The box-in-box is the showcase's `Kind("textinput")` rule reaching
    the inner field (C5). Framework part: SearchField's inner field
    resets border, width and radius in its own seed, so a sheet's
    textinput rule must name it to reach it.
  - Escape clears the query, and a × clear part appears when the field
    is non-empty (wiring `clear()`, which exists and has no caller).
  - A submit handler on Enter.
  - The icon colour closes on A8; the clipped long text and mid-word
    caret on B15's end-of-text fix.
  - **Gate:** a long query plus End shows the last word, with the caret
    after it.
- **B27 ProgressBar.** Accessibility closes on A17, and the track on A8.
- **B28 Spinner.** Accessibility closes on A17.
- **B29 Skeleton.**
  - The dark slabs close on A8 (`Placeholder`).
  - The shimmer is a hard-edged band. **Fix:** a horizontal gradient
    band. `Painter` has gradients (the `Deferring` note in AGENT.md
    confirms it).
  - One sweep across a group: the phase is offset by x.

### Charts

- **B30 BarChart, B31 LineChart.**
  - **Fix:**
    - Optional axes: a baseline rule, y ticks with labels, and x labels
      from a `labels: &[&str]`.
    - Hover inspection: a highlight on hover plus a value tooltip via
      the `Hover` event (T27.11).
  - Accessibility: A17.
  - LineChart only:
    - Joins: draw the polyline as one path with round joins, or add
      round caps at each vertex.
    - Inset the plot by half the stroke width.
  - A series with no positive values draws an empty frame.
- **B32 PieChart.**
  - **Cause:** a 1.35% gap at 12 o'clock.
    `to = ((at + frac) * STEPS) as usize` truncates, and the f32 running
    sum ends below 1.0, so the last segment is never drawn. Each wedge is
    also snapped to 3.75° and drawn from its segment start.
  - **Fix:**
    - Compute wedge edges from exact cumulative fractions.
    - The last wedge ends at exactly 1.0.
    - Draw by angle rather than by 96 fixed steps: a triangle fan per
      wedge, which also fixes the scalloped edge.
  - A legend option. The default series colours are chosen at ≥ 3:1
    against `Surface`.
  - **Gate:** sampling angles, the unpainted fraction is 0 and each
    wedge is within 0.5% of its share. Injection: truncation.

### Surface and navigation

- **B33 Card.**
  - Dark mode closes on A8 (SurfaceRaised, plus a dark-palette shadow
    or border so it separates).
  - Elevation levels: `elevation: 0..=3` mapping to shadow and border.
- **B34 Toast.**
  - Accessibility closes on A17 (`Status`).
  - Placement: a toast shown through `Popup` with anchor `Window` at
    bottom-centre, so it overlays rather than flows.
  - Auto-dismiss via a `Task` timer (M28): `Toast::show(ms)` returns a
    job, and the rustdoc's "no per-node clock" is answered by M28.
  - A close part.
- **B35 Tooltip.**
  - Opens on focus (A7's focus event), stays on screen (A18), and
    Escape closes it (A13).
  - A show delay of 500 ms through an M28 timer, and a hide delay of 0.
  - **Cause:** the rustdoc promises a press closes the tip, but `layout`
    re-opens it every hovered frame.
  - **Fix:** a press sets `suppressed` until the pointer leaves.
  - Centre the tip under the control.
  - **Gate:** hover opens only after the delay; a press closes it and it
    stays closed while hovered.
- **B36 Tabs.**
  - The solid block closes on A6, dark text on A8, and live wrapping on
    A3.
  - Hover and press feedback: `Strip::paint` reads `hovered` and
    `pressed` for the hovered cell (via `Hover`).
  - The underline and the selected label in `Role::Accent`, with
    unselected labels `Role::Muted`.
- **B37 BottomNav, B38 NavigationRail.**
  - Wrapping closes on A3, and dark mode on A8.
  - Hover and press as in B36.
  - Rail: fixed-height items stacked from the top (Material), not
    stretched over the rail.
- **B39 AppBar.**
  - Contrast closes on A8 (the accent is chosen so on-accent text is
    ≥ 4.5:1).
  - Leading and action slots: `AppBar<L, A>` with two optional child
    slots.
  - Test the long-title claim (the audit could not): a title wider than
    the bar is truncated with an ellipsis inside the bar.
- **B40 PullToRefresh.**
  - **Cause:** a wheel pull never springs back, because release happens
    only on `PointerUp`.
  - **Fix:** a wheel gesture ends after 150 ms with no wheel (an M28
    timer), and then releases.
  - **Cause:** dragging does nothing, because there is no
    `PointerMove`/drag arm.
  - **Fix:** a press captures, a move pulls, and a release releases.
  - **Cause:** it pulls at any scroll offset.
  - **Fix:** pull only when the child reports it is at its top. That
    needs a framework query: `Node::scroll_offset() -> Option<f32>`,
    which `ScrollView` and `Repeat` answer.
  - Resistance: pull = raw × 0.5, capped at 2× the threshold.
  - Clip the content to the widget's box.
  - A refreshing state: `refreshing: bool`, controlled, showing a
    spinner until the application clears it.
  - **Gate:**
    - Wheel to armed, idle 200 ms, and it is idle and released.
    - A drag pulls.
    - A scrolled child does not pull.

### Overlay, layout and data

- **B41 Modal, B42 Sheet, B43 Drawer (`Veil`).**
  - Panel-press dismissal closes on A4, Escape on A13, focus-in on A12,
    and the dark title and scrim on A8 (`Scrim`, SurfaceRaised).
  - Shift+Tab: the Tab trap (`pass.rs:315`) moves backwards on
    Shift+Tab (`mods`).
  - Modal is "sized to its content": the body column is `Shrink`, with a
    max of 80% of the veil.
  - Sheet: only the top corners are rounded. This needs per-corner
    radius, a `Style.radius` → `[f32; 4]` framework change, measured
    against `NodeState`'s budget. Its shadow is clipped to the veil.
  - Drawer: `edge = Left | Right`.
  - The showcase's shared dismissal/confirm readout: C6.
- **B44 Wrap.** Line alignment closes on A19. `W1005` on `#d_wrap_f`:
  re-check after A3's real measure. It is expected to vanish; if it does
  not, it is a real clip.
- **B45 Grid.** Closes on A19.
- **B46 SplitPane.**
  - Pane clicks close on A4, and the keyboard on A12.
  - The divider drift:
    - Place the divider at `split·(w − d)` and set
      `split = (x − grab)/(w − d)`, where `d` is the divider width.
    - Keep the grab offset on press.
  - A minimum pane size in px (`min_px`), not a fraction.
  - A vertical orientation, sharing the code by axis.
  - The divider colour via A8's `Muted` at ≥ 3:1.
- **B47 Accordion (`Collapsible`).**
  - Body clicks close on A4, and the keyboard on A12 (Enter and Space on
    the header).
  - Bottom padding on the open body.
  - Opening moves the header under the pointer: a showcase layout matter
    (C7), since the stage centres vertically.
  - The one-open rule and the catalog comment: C7.
- **B48 PaneGrid.**
  - Pane clicks close on A4. Keyboard via A12: each divider focusable,
    with arrows along its own axis.
  - The resize cursor via `CURSOR` in its seed.
  - A minimum px as in B46.
- **B49 Pagination.**
  - **Cause:** the window moves under the pointer, and the rustdoc
    claims it doesn't.
  - **Fix:**
    - Slide the window only when the chosen page is at the window's
      edge, so a press on a visible number leaves the numbers in place.
    - First and last pages always shown, with "…" gaps.
  - Contrast: A8.
  - Two-digit wrapping (seen once): after A3 and A5, it is expected to
    vanish. Keep a render check of pages 9–13.
  - Hover highlight per page.
  - **Gate:** pressing a visible page number leaves the number under the
    pointer unchanged.
- **B50 Tree.**
  - **Cause:** the hover sticks, because `hot` clears only on a `Hover`
    inside the tree.
  - **Fix:** the pass sends `Hover` with `None` (pointer left) to the
    last hovered node when the hover target changes. This is a
    framework fix in `dispatch`'s hover synthesis (T27.11), so every
    `Hover` user gets it.
  - The keyboard closes on A12: Up/Down move, Left/Right collapse and
    expand, Enter selects.
  - Selection: `selected: Option<u32>`, controlled. Hover becomes a
    subtle tint, and selection the strong fill.
  - Accessibility tree items: A17. Palette: A8.
  - **Gate:** hover a row, leave, and there is no highlight.
- **B51 DataGrid.**
  - The garbled Size column closes on A5.
  - Row hover and selection as in B50.
  - A press on a row: a row handler via `ctx.row`.
  - The wheel over the header scrolls the body.
  - A scrollbar indicator. `Repeat` gains an optional thumb, drawn when
    content exceeds the viewport.
  - Per-column alignment (right for numbers).
  - Row separators. Palette: A8.

---

## Phase C — the showcase's own defects

These are the only changes made in `examples/showcase`. Each one is the
showcase's wiring or stylesheet, not a framework gap.

- **C1. `hover_dim(0.0)` leaks from Select's row rule onto every
  button.**
  - **Cause:** `Selector::Kind("button")` at
    `examples/showcase/src/main.rs:1240-1250` was meant for Select's
    rows.
  - **Fix:** scope it with the descendant step (`#demo_select > Kind`),
    or style the rows through `SelectMenu`'s own part ids.
  - **Gate:** the existing `every_rule_matches_a_node` lint plus a new
    showcase test that a demo Button's painted background changes under
    hover.
- **C2. The toggle demos never bind `on`.**
  - **Cause:** `toggle()` in `catalog.rs:241` flips `Store.flag`, and
    `refresh` (main.rs:300-395) has no toggle arm.
  - **Fix:**
    - Bind `on` from the store, as M33 made possible with `#[bind]`'s
      value form.
    - Give Radio its own `Store.choice` (B13).
    - Drop the hard-coded `toggle(false)` on Blue.
- **C3. The Menu demo cannot reopen.** Add a trigger button.
- **C4. The NumberInput readout never updates.** `refresh` reads Slider
  only (main.rs ~350). Add NumberInput, and RangeSlider's bounds.
- **C5. SearchField box-in-box.** The `Kind("textinput")` rule at
  main.rs:1122-1133 gives every TextInput a 220 px bordered box. Scope it
  to the TextInput demo's id. Once B26 lands, that rule should no longer
  reach the inner field anyway.
- **C6. Modal's readout counts Confirm presses as dismissals.** Split
  `presses` into two store fields.
- **C7. Accordion demo.**
  - `section()`'s comment says the first starts open, and none does.
    Open the first.
  - Enforce the one-open rule in the application, the "three lines" the
    rustdoc promises.
  - Top-align the demo stage for this entry, so opening does not move
    the header.
- **C8. Stage padding on PullToRefresh's demo** (label jammed into the
  corner).
- **C9. The Wrap demo is off-centre.** Closes on A19's line alignment,
  using `align = center`.

---

## Phase D — the gates the audit showed were missing

Each is a standing check, so this class of defect cannot pass again.

- **D1. The drives assert pixels, not only the store.**
  `scripts/drive/harness.py` gains `region_changed(sel)`, which captures
  a node's bounds before and after an action and diffs them. Every
  action row in every widget's inventory declares whether its
  consequence is visible. If it is, the drive asserts the pixels
  changed.
  - This is what would have caught C2: CheckBox passed while never
    repainting.
  - Injection: C2's unbound `on`.
- **D2. Unit tests of pass behaviour go through `pump`.** A lint in
  `just static` flags a test in `fulgurite-widgets/tests/` that calls
  `on_event` with a `PointerDown`, `PointerUp` or `Key` and never calls
  `pump`/`dispatch` in the same file. Such a test is allowed with an
  `// on_event only:` comment saying why.
  - This is what hid A4, A7, A13 and the first `Hover` test.
- **D3. A keyboard-only drive** (A12's gate), under both strategies.
- **D4. The contrast gate covers every demo** in both palettes (A8's
  gate 2).
- **D5. The drive runs with the agent attached** (it always does) **and
  compares against a headless frame without it** for every entry: a
  pixel diff over the demo's box. This is A3's end-to-end form, and the
  check that would have caught X1 on day one.
- **D6. The drive runs at scale 2** (`WINIT_X11_SCALE_FACTOR=2`) as a
  third pass after `Boundaries` and `Everything`, comparing the partial
  frame against a forced full repaint. This is A2's end-to-end form.
- **D7. The accessibility sweep** (A17's gate) runs in `just
  widget-drive`.

---

## Order

1. **A1, A6** first: the reported bug, and the ring every later render
   check sees.
2. **A3, A5, A4, A7, A13**: the framework defects that make widgets
   broken.
3. **A2**, landing M40 if it is ready, together with **D6**.
4. **A8** (a large but mechanical change across every widget), then
   **D4**.
5. **A9–A12, A14–A19**.
6. **Phase C**, alongside the A items it follows. C1 and C2 can land any
   time; they are independent.
7. **Phase B**, widget by widget, in catalogue order. Each B entry is one
   commit with its gate.
8. **D1–D3, D5, D7** as soon as their A item exists, so each later B
   entry is checked by them.

Each A item updates `docs/00-handoff-static-tree.md` where it changes a
decided behaviour. The focus ring moves from the widget to the host, and
activation from key-down to key-up for Space. The closing commit updates
`docs/widget-audit.md` with a "fixed in" column, and checks each of the 52
rows mechanically against the drive rather than restating them.

## What this plan does not do

- **It does not make the renderer anti-alias rect edges.** A15 snaps
  origins instead, which makes the question moot for axis-aligned
  geometry.
- **Italic in Markdown stays out**, for the reason its rustdoc gives:
  `Painter` has no slant axis.
- **PaneGrid stays a fixed 2×2.** Rearranging panes as a tree is the
  topology-as-data question its rustdoc already names.

## What building it found

Kept here rather than folded into the entries above, so the plan still
reads as it was written and the corrections are in one place.

### Where the plan was wrong

- **B15's end-of-text caret.** The plan read `x_of` answering the last
  cluster's start at `len`. The shell's shaper emits an insertion point
  after the last character, so the live caret was already right, and
  restoring the old fallback left the test green. The fallback matters
  when that cluster is missing; with the shaper's end cluster removed it
  put the caret inside the last glyph (32 px against 42).
- **B7's bullet.** The audit's "2.5 px high" was right; the first test of
  the fix said the opposite, because a headless `Host` has no face and
  every glyph is a solid box. The test now lays out with the system face
  and asserts its `x` is not a box.
- **B9's "the sparkline draws 500 px past the edge".** Nothing did. What
  the clip keeps in is the edge dots' 3 px, and the drive now says so.
- **B26** asked for a clear button and a submit; building its gate found a
  **layout defect in four wrappers** the plan never mentioned: `SearchField`,
  `Card`, `Glass` and `ScrollView` laid their content out across what they
  were *offered*, not their own `Fixed` size -- M56's defect in the derive,
  never fixed in hand-written wrappers (`tests/wrapper_own_size.rs`).
- **B34**'s timer is on the application's clock (`Toast::tick`), not an M28
  job: a job needs the application's timer set and a message type for one
  widget's countdown. Using the floating toast found that a window-anchored
  popup was dismissed by the very press that opened it; it now has no
  "outside".
- **B41–B43** did not need per-corner radius: a sheet or drawer is laid out
  past the window's edge by its radius and painted clipped to the veil.
- **Not built, and why**: `Radio<T>` controlled by `selected == value`
  (arrows did not need a new type); `Slider::on_change` (a handler runs on
  activation, which a drag never is); rounded clips for `PaintBox` (its
  rustdoc now says the clip is the box).

### Framework defects the fixes uncovered

- An arrow group was its parent's children, so a radio beside its label
  had no group (B13); grouping now looks through single-control wrappers,
  asking each first -- the first version stole Down from a pick list.
- A table's row was not a target when its cells declined a hit (B51).
- Layout leaves published themselves as nameless containers, which the
  live accessibility sweep (D7) found in the grid's rows.
- **The software canvas repainted a mid-pixel damage edge wrong** (D6).
  `fill` rounds a rect outward and the glyph blit truncates it, so a
  region ending at 608.4 px -- a popup edge at 304.2 logical, scale 2 --
  cleared row 608 and never re-inked it: one stale row under every
  picker's trigger text. The canvas now snaps the damage outward, as the
  GPU backend's `snap_out` already did. It showed only at the window size
  the window manager actually granted (1920x1008, not the 2240x1520
  asked for): at the requested size every edge was whole, and a headless
  reproduction stayed green until it used the real one.
- **That edge was inside the trigger to begin with.** A picker hangs its
  pad 4 px under its trigger and told the popup to flip by the trigger's
  height alone, so a pad opened above ended 4 px *inside* the trigger
  (A18's flip, one gap short; `Tooltip` had counted both). Only the
  scale-2 window was short enough to flip it. The drive also learned
  that an action which opens a popup is bounded against the widget and
  that popup (`Action.opens`): a flipped pad's shadow falls on its
  trigger, and the old bound read that as 9.14x the trigger's box.
- DataGrid's rows could not be reached by keyboard at all (D3); B51 did
  not ask for it. Fixed after closing, below, when the grid became a sheet.

### Reported after closing (2026-09-27)

Three defects found by using the showcase, each fixed where it lives:

- **A filtered combobox chose by the unfiltered position.** Typing "ap"
  left Apple and apricot; pressing the second chose Banana. The press
  cleared focus (the list is not focusable), B22's "an abandoned query
  reverts" arm fired on the field's focus loss and cleared the filter,
  and the release chose by position in the whole list. A press on a
  field's own list now leaves focus where it is (`NodeDesc::KEEPS_FOCUS`,
  honoured by the pass, set on the chooser's list). Every drive action
  either filtered or chose, never both; the combobox drive now does both.
  Building that action exposed a second, older false positive: every
  *closed* chooser's list was `W1001` all the time, because a closed
  popup zeroes its subtree exactly as a squeeze does. A popup now says
  its content is absent on purpose (`Node::content_absent`), as a stack
  says its children overlap for `W1004`.
- **A search field showed a slice of its text.** The pill gives its
  field 22 px; `TextInput` kept its own 6 + 6 padding and clipped its run
  to the 10 px left of a 19 px line. A single-line field now centres its
  line and clips only across to its padding, down to its box.
- **The DataGrid read as a list.** It is a sheet now: rules on every
  column boundary and between rows, a tinted header band and row-number
  gutter, and an active cell a press or the arrows move (Home/End,
  Ctrl+Home/End, PageUp/PageDown), scrolled into view and reported as a
  press is. The grid is focusable, which closes D3's finding. Cells pad
  their own text by `DataGrid::CELL_INSET`, since the rules sit on the
  column boundaries of a row the grid does not own.

The second round, the same day, from using the first round's fixes:

- **After a choice, the combobox was stuck.** The caret followed the
  pointer, a press on the field did nothing, and typing opened no list.
  Three causes. The chooser handed *every* event that bubbled to it to
  its field, so a press on the list reached the field as a click, was
  consumed, and gave the chooser the pointer capture. The release was
  consumed by the list on the overlay path, which returned before
  capture was consulted, so the capture outlived the gesture; captured
  moves and releases now go to the captured node first on that path too.
  And with the field still focused (`KEEPS_FOCUS`), opening on the focus
  transition had nothing left to fire on. It now also reopens on a press
  of the field, counted by `TextInput::presses` because a pressed bit
  does not survive a press and release in one frame (the live drive's
  click), and on a changed query.
- **A full search field's view jumped on every click.** `TextInput`
  decided its scroll in `layout`, which also runs as a measure. Laid out
  wide, by a measuring parent or by the agent's audit under a
  million-pixel box, the run "fitted" and the scroll clamped to 0. The
  scroll is decided in `place` now, against the final box, which is
  T27.10's answer for `Repeat`.
- **A full search field went blank on the next letter** (reported with
  a screenshot, the same evening). The run's rect is the width its glyphs
  are laid out in, and the field passed its visible width. With the
  caret at the end, that equals the run's width to the float, so on every
  other keystroke a word with no spaces came out wider than its rect by a
  fraction of a pixel and wrapped whole onto a line below the clip. This
  is also the likelier cause of the first round's "visible on every
  second click". The rect now holds the whole run.
- **The DataGrid's scrollbar took no press.** It has its own strip, a
  thumb that drags, and a track that jumps the thumb to the press.

Two things about the scale-2 pass this round showed, not fixed:

- **Its sensitivity depends on the window it is granted.** Since the
  drive waits for the window to settle, the size it runs at is the one
  the window manager chose, and that varies: 1920x1008 on some runs,
  1920x1048 on others. At 1048 the flipped pad's edge lands on a whole
  pixel, and with D6's damage snap removed again the pass stays green.
  The headless test at 1920x1008
  (`a_popup_repaints_partially_to_the_same_pixels_at_scale_2`) is the
  gate that does not depend on it.
- **Whole-window damage lands in a random action, now and then.** Its
  `ui.lastDamage` is the whole 960x504 window where the same action
  otherwise reports 1.00x its box: one Combobox scale-2 run in four,
  each time on a different action, and never headless. The window is
  full-repainting for a reason of its own, and a late resize from the
  window manager is the unconfirmed candidate. The checks are left as
  they are, and it is recorded here.
- **The evening's full live drives ran on a desktop in use.** The drive
  opens a new window for each pass, and the user closed them, which
  showed in the drive as "agent endpoint closed the connection". Real
  pointer and keyboard input on those windows is the likeliest single
  explanation for the one-off failures above. Each failed action was
  rerun alone, three or four times, and passed every time. Live drives
  want an idle display.
- **One intermittent failure is not attributed.** DatePicker's "a press
  opens the popup" failed the full-repaint check twice, in about seven
  runs at 1008. The damaged frame showed the trigger hovered, and the
  full repaint showed it not hovered. Three instrumented runs read the
  trigger hovered both before and after the forced repaint, and passed.
  One unconfirmed explanation is a real pointer event from the machine
  landing between the two reads. A retry keyed to the frame counter was
  tried and removed: with the stale-row defect injected it passed on its
  third attempt, because the first forced repaint had already cleaned the
  frame it was comparing.

Two of this round's tests were vacuous on their first run, and both were
caught by injection. A capture test pressed the field of an open list:
that press dismisses the list, so nothing was ever captured. The
drag-release test moved with nothing pressed, and a stale drag only
shows once the grid holds the pointer again.

### Tests that proved nothing until fixed

Each was caught by an injection that left it green, or by a pass that
removed the incidental difference it had leaned on:

- the Play icon's stripes were **tonal**, not gaps, so a 50% threshold
  saw none (B4);
- a split's drift test used numbers where the broken and fixed mappings
  agreed within its tolerance (B46);
- a toast's "no × means no press" pressed outside the band a × would own
  (B34), and an app bar's trim was never exercised by its title (B39);
- **the Switch's "track changes colour" drive action** compared a colour
  an on switch does not paint with, and passed on a hover shade -- found
  when D1 hovered first and took that difference away;
- several drive actions **asserted the defect**: the TimePicker's hour
  closing the clock, a tree leaf declining a press, the pagination
  window, a SearchField isolate that only passed while the field overran
  the pill, an accordion press landing on the header only while the body
  was short.

### Standing gates this plan added

D1 (pixels, `visible` declared for every action), D2 (a direct `on_event`
test goes through the pass or says why), D3 (keyboard-only), D4 (contrast
across every demo, both palettes), D5 (agent versus headless, byte for
byte), D6 (scale 2 against a full repaint), D7 (every leaf speaks or is
hidden). Two intermittent full-run failures (SplitPane's drag, Combobox's
clear) did not reproduce and read like input landing before a demo
settled; they are recorded in their commits.

