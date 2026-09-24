# M56 — the widget showcase, redesigned

> **Written 2026-09-24, not started.** Asked for on 2026-09-24: a more
> modern design that follows good design practice, and without the
> explanatory note under each widget. Two decisions came from the ask:
> keep the one-line blurb as the page subtitle, and replace the dropdown
> picker with a grouped sidebar. Numbered M56 because M39–M55 are
> proposals already taken by the unmerged `m38-deep-review` branch.

## Context

`examples/showcase` is the flagship example. It also serves as the live
window every drive script tests against (`just widget-drive`, `just
agent-drive`). Its chrome dates from M23/M24:

- a single column with a 300 px dropdown picker;
- a one-line blurb and a live readout in the header;
- a hairline rule;
- the demo, centred on a bare page, with a paragraph of implementation
  commentary (`Entry::note`) under it.

The notes read as noise next to the widget, and the dropdown hides the
catalogue: 51 widgets behind one closed menu. The group headings also
repeat, because `CATALOG` revisits Layout, Text and Input out of order.

The goal is a modern component-gallery layout that follows ordinary
design practice:

- a persistent grouped sidebar;
- clear type hierarchy on an 8 px grid;
- the demo on a card;
- a status chip;
- contrast checked;
- dark mode at parity.

**The note under each widget is removed.** Decided by the user: **keep
the blurb** as the page subtitle, and **navigate with a sidebar**, which
replaces the dropdown.

Numbered M56, because M39–M55 are proposals already taken by the
unmerged `m38-deep-review` branch.

## The design

The window grows from 960×720 to 1120×760, so the sidebar fits without
squeezing the stage.

```
┌──────────────┬──────────────────────────────────────────────┐
│ lumen-static │  Input / Button                 ● presses: 2 │  ← breadcrumb 12/600 muted; chip
│ WIDGETS      │  Button                                      │  ← 24/700 text
│              │  Three of them, all wired to one counter.    │  ← blurb 14/400 muted
│ TEXT         │ ┌──────────────────────────────────────────┐ │
│  Label       │ │                                          │ │
│  RichText    │ │          [Primary] [Default] [Ghost]     │ │  ← canvas card: surface,
│ INPUT        │ │                                          │ │    radius 16, 1px hairline,
│ ▌Button  ◀── │ │                                          │ │    shadow 12, padding 32,
│  CheckBox    │ └──────────────────────────────────────────┘ │    demo centred
└──────────────┴──────────────────────────────────────────────┘
 sidebar 248 px, surface, scrolls (Repeat wheel, T27.4)
```

**Scales.**
- Spacing is multiples of 4 and 8: 4, 8, 12, 16, 24, 32.
- Type is 12, 14, 16 and 24, in weights 400, 600 and 700.
- Radii are 8 for controls and rows, 12 for popups and 16 for the canvas.

**Sidebar.**
- The brand line is at the top.
- Group headings are 11/700, muted, in upper case. Tracking is not
  expressible, so upper case alone carries it.
- Rows are 32 px high, as a 14 px pill inset 8 px.
- Hovered rows use `hover_dim`.
- The selected row has an accent-tinted fill, the accent text colour,
  and weight 600.

**Header.** It sits on the page, with no band:
- the breadcrumb (`Group / Name`);
- the name as the page title;
- the blurb in muted text;
- the readout as a pill chip on the right, 12/600 in the accent colour
  on an accent tint.

**Canvas.** A surface card fills the rest of the window. The existing
FILL spacers keep centring the demo inside it.

**Palette.** The role-less chrome colours (a hairline, the chip's
ground, the ghost tint) get a light and a dark value each, picked with
the palette, so dark mode is at parity. *As built (T56.3):* one `Chrome`
struct, `CHROME_LIGHT` and `CHROME_DARK`, which replaced the single-valued
hairline and ghost constants. The selected row needed no constant: `NavRow`
derives its tint from the accent role.

**Within today's `Patch`.** Everything above uses existing setters only:
radius, border, background, weight, `shadow` (spread only), and
`hover_dim`. There is no framework change. The sidebar's right edge is a
1 px FILL-height `Divider` column, because borders are per node and not
per side.

## Tasks

Each task is one commit, with `just test` green, docs in the same
commit, and every new gate shown to fail (AGENT.md).

### T56.0 — baseline

Record `--shot` and `--demo` frames of every entry, in light and dark,
into `target/showcase-before/`. They are for side-by-side review only,
not goldens: the demo's backdrop changes from page to surface, so a
byte-identical comparison is impossible by design.

### T56.1 — the note goes, and the catalogue is grouped once

> **DONE 2026-09-24.** Two things were built differently from this
> section.
>
> - **The catalogue stays a flat `&[Entry]`**, reordered so each group
>   is contiguous. Nested `Group` slices would have churned every
>   `CATALOG[i]` index that the tests, the drives and `--shot N` use.
>   The property that mattered, each group named once, is now a test:
>   `each_group_appears_once` prints the 11 groups in order.
> - **The fixture is the opening 32 characters of each former note**,
>   not the whole note, which is enough to recognise one.
>   `every_entry_paints_a_demo` asserts that no text on any entry's
>   frame starts with one.
>
> **The rustdoc check.** 42 of the 51 notes were already stated in
> rustdoc, and 9 in plain comments, which the project's rule accepts.
> Five were stated nowhere and moved into rustdoc: `Label`, `LINE`,
> `item_at`, `TimePicker` and `FilePicker`. `LineChart`'s note was also
> slightly wrong: one bad sample in the middle costs two segments, not
> one, and the rustdoc says "a gap". **`W1005` left with the note.**
> `choose.py`'s allowance for it is gone, and the PickList drive is
> green under both strategies without it.

`catalog.rs`:
- Drop `Entry::note` and its `entry!` argument.
- Restructure as `CATALOG: &[Group { name, entries: &[Entry] }]`, in a
  teaching order: Text, Input, Choice, Fields, Progress, Charts,
  Surface, Navigation, Overlay, Layout, Data. Each group appears
  **once**; today Layout, Text and Input recur.
- Add `fn nth(i) -> (&Group, &Entry)` and `fn rows()`. Both are
  allocation-free walks over the static slices.

**Before deleting any note**, check that its claim lives in the widget's
rustdoc. If it does not, move it there, under the doc-currency rule: a
rationale lives in a doc or a comment. Most notes restate rustdoc
already. The commit body lists the ones that moved.

`main.rs`:
- Remove `Stage.note` (`#note`) and the note write in `rebuild`.
- Delete `choose.py`'s `W1005` expectation, which exists only because
  `#note` wrapped.

**Gate.** `every_entry_paints_a_demo` gains an assertion: no text
command anywhere on the screen equals any former note. The former notes
are copied into the test as a fixture before deletion. *Injection:* put
the `#note` label back, and the test fires naming the entry.

### T56.2 — the sidebar replaces the picker

> **DONE 2026-09-25.** `nav.rs`'s `NavRow` sits in one `Repeat` whose
> bind writes only `&'static str`s and flags. The selection is a
> thread-local the bind reads: a bind cannot capture, and the tests run
> one `Show` per thread. Its selected tint and bar come from the style's
> `border_color`, so a palette swap re-themes it. Building it found
> more than this section expected:
>
> - **There was no `Select` entry.** The picker *was* `Select`'s only
>   appearance, so removing it would have taken `Select` off screen and
>   taken `profile_popup` (M26/T26.0's popup arm) with it. `Select` is a
>   demo now (52 entries), controlled through `Store`.
> - **Two framework defects, each with its own gate and commit.** A
>   `Fixed` container laid its content out across its *offer*
>   (`crates/lumen-static/tests/fixed_container.rs`: two halves of 250
>   px inside a 100 px box). A scrolled `Repeat` asked the wrong row for
>   its handler (`tests/repeat_handler.rs`: 10 of 11 rows).
> - **Centring.** `#demo` was only centred by being one `FILL` third of
>   the stage row: 320 px at the old window, which fit three buttons by
>   luck, and "Gh / ost" on M56's card. Now every demo struct is
>   `Shrink`. The list of their kinds comes from the same macro that
>   declares `Demo`, so it cannot drift. Demos whose widget fills its
>   box get widths of their own. **Wrap reflows onto a second line now**,
>   which its blurb always claimed and which the old 248 px offer never
>   let it do.
> - **Two tests were passing on the chrome.** "A widget that draws only
>   text" counted everything below the stage's top edge. The header's
>   hairline sat exactly there, so RichText and PullToRefresh were each
>   credited with it; a worktree run at T56.1 printed the divider as
>   RichText's shape. The filter now takes only what is strictly inside
>   the stage card. RichText is exempt, as `Label` is, since it is text
>   by nature. PullToRefresh gets a ground, because a pull area that
>   reads as a line of text is the finding.
> - **Clicking the sidebar from a demo with an open popup** (DatePicker,
>   Menu) first closes the popup and is consumed. That is the documented
>   dropdown behaviour, kept. The walk clicks again only when a popup was
>   open and the press closed it, reading the new `Host::open_overlays`.
>
> **Gate:** `every_sidebar_row_picks_its_own_entry`. Real wheel events
> and real clicks cover all 63 rows (52 entries and 11 headings, 2
> presses first closing a popup). It checks the store, the header's
> title and the row's own highlight, and that a heading press answers
> `handled == false`. *Injections:* an entry row showing the next
> entry's name ("row 1 was clicked for \"Label\" and now reads
> \"Badge\"" — the message was sharpened after this run); headings
> given the item handler ("a press on the \"Text\" heading (row 0) was
> handled").

**The row widget.** A `NavRow` in a new `examples/showcase/src/nav.rs`
has:
- `label: &'static str`;
- `kind: Heading | Item`;
- `selected: bool`;
- `on_click`.

Its paint draws the selected pill, and headings ignore presses. Its
colour fields follow M25/T25.10, because a `Repeat` row cannot take a
per-row stylesheet id.

**The list.** `Sidebar` is a column holding the brand label and a
`Repeat<NavRow>` sized to `rows()`. The `Repeat` scrolls by wheel
(T27.4). Its **bind writes only `&'static str` and a bool**: no
`format!`, which is the T29.5 lesson. `alloc_gate.rs`'s writing-bind arm
is the check.

**Screen.** `Screen` becomes a row: the sidebar, a 1 px divider, and
`Main`. `Main` is a column of header and canvas.

**The picker.** `Header.picker`, `menu_open`, the `#[bind(... menu.open
...)]`, `bind_row`, `PANEL_H` and the `select-*` sheet rules all go. The
`Select` widget keeps its own catalogue entry and demo, so it is still
shown and driven; check that entry exists.

**Selection.** Picking an entry keeps `choose` and `needs_rebuild`
exactly as today, called from `NavRow`'s click.

**Tests** to update in `main.rs`:
- `the_menu_has_a_row_for_every_widget` becomes "the sidebar has a row
  for every entry and a heading per group, each group once".
- `picking_a_row_swaps_the_demo` clicks a sidebar row.
- `the_dropdown_is_absent_when_closed` is deleted, since there is no
  dropdown.
- The hard-coded stage node index (a constant 8, in two places) is replaced by resolving `#demo`'s
  index through `wid_index!`, not a new hard-coded number.
- The `pos.y > 200` and `width ≤ 300` geometry heuristics in the press
  tests are reworked to find nodes by id.

**Gate.** Every row click selects its own entry: the headless test walks
all 51. *Injection:* the bind puts row *i*'s label on row *i+1*, and the
test names the first mismatch. A heading press changes nothing,
asserted through `pump`'s `handled == false`. Its injection gives
headings the item handler.

### T56.3 — chrome restyle

> **DONE 2026-09-25.**
>
> - **The chip.** The readout is a `StatusChip` (`chip.rs`), which is
>   absent while it has nothing to say. A `Label` painted an empty
>   20 × 8 px lozenge beside every stateless entry.
> - **Chrome colours.** They are the palette-picked `Chrome` values. The
>   card's shadow went from 12 to 6. "Default" has an outline, having
>   been white on the white card.
> - **Contrast.** The gate `chrome_text_meets_contrast` checks every
>   text/ground pair the chrome paints, from the values it paints with,
>   **at AA's 4.5:1**. The plan's "3:1 for muted" was too lenient: the
>   blurb and breadcrumb are content. Its first run measured three
>   pairs under — light muted on the page at 4.33, and the chip's accent
>   on its tint at 3.90 (light) and 4.44 (dark) — plus the ghost demo
>   button at 3.91 and 4.19. The fixes were a darker light muted
>   (`#636c80`, 4.87) and one `accent_text` per palette for accent text
>   on a tint (5.88–7.15). *Injection:* light muted set to the page
>   colour reports "blurb and breadcrumb on the page is 1.00:1" and "brand
>   subtitle on the sidebar is 1.08:1".
> - **Visual review.** All 52 entries were reviewed in both palettes
>   against T56.0's baseline, and it found dark-mode defects. My own
>   `Kind("label")` → Text rule turned DataGrid's and PaneGrid's labels
>   light-on-light, so it was withdrawn. Icon, RichText, Markdown and
>   RichTextEditor were near-invisible on dark **before M56 too**; they
>   now take the Text role, the editor through its `textarea`, because
>   the sheet lint refused a rule on the editor itself, which reads no
>   `color`. The pull area and the split pane sit on a light panel in
>   both palettes, because their content is a bare dark `Label`.
> - **The veils.** Their scrim now covers the stage card rather than
>   the window, a side effect of each demo having its own box. The
>   sidebar stays reachable while a Modal, a Sheet or a Drawer is open.

- The header becomes breadcrumb, title, blurb and chip.
- The canvas becomes a card.
- Palette constants for both modes.
- Rewrite `stylesheet()`'s chrome rules. The demo rules are untouched,
  apart from any whose backdrop assumption changes.

The existing lints must stay green: `every_stylesheet_rule_changes_something`,
`every_rule_matches_a_node` and `every_role_rule_follows_the_palette`.

**New gate:** `chrome_text_meets_contrast`. For every chrome text/fill
pair, in both palettes:
- body text must be ≥ 4.5:1;
- the chip and the selected row must be ≥ 4.5:1;
- headings and muted text must be ≥ 3:1 (large or secondary).

The test prints each measured ratio beside its bound. Reuse the WCAG
luminance code from `crates/lumen-static-shell/tests/failure_screen.rs`
(T36.2), lifted into a small shared helper if both need it.
*Injection:* set muted text to the page colour, and the gate names the
pair.

**Visual review.** Compare `--shot` of every entry, light and dark,
against the T56.0 baseline by eye, using the render-first habit from
M24/M27. Anything clipped or low-contrast gets fixed in this task.

### T56.4 — the drives follow the new chrome

> **DONE 2026-09-25.** `just widget-drive` is green on 39 widgets and 94
> actions under both strategies, and `just agent-drive` is green under
> both, with the shadow oracle quiet over 47 frames.
>
> - **The harness picks through the sidebar.** `_pick` scrolls it to
>   the top, then down until the row whose label is exactly the name is
>   wholly in view, skipping `Heading`-role rows. It confirms the page's
>   title.
> - **`agent_drive.sh` works the dropdown as a demo.** It picks Switch
>   by `~=`, then scrolls to Select, opens it, and chooses "Cherry" by
>   its text; the trigger reads Cherry, the chip reads `selected:
>   Cherry`, and the menu closes. The palette check reads `#sidebar`.
>
> **What the drive found.**
>
> - **A defect in the agent.** `ui.lastDamage` counted nodes that draw
>   nothing, so the stage's spacers and transparent row, which move when
>   a demo grows, merged into the card's whole inner box: 7.47x the
>   accordion's own box, on a frame the renderer redrew at 1.00x. Fixed
>   with its own gate (`crates/lumen-static-agent/tests/damage_paints.rs`:
>   1,280 px² against 84,800 without the fix) and recorded as the fourth
>   amplification in `docs/damage-amplification-decision.md`.
> - **A measurement artefact in the PaintBox action.** A box centred in
>   an odd remainder starts on a half pixel (x = 524.5), and the counter
>   called the straddling pixels "outside". A pixel is outside now only
>   when it is wholly outside. *Injection:* the widget's clip removed
>   leaves 16 px outside, and the action fails under both strategies.
> - **An inconsistency in the audit, recorded and not changed.** Closing
>   an accordion section now draws `W1001` ("laid out 0x0") where the
>   old 248 px share drew `W1005` ("squeezed"). A collapsed
>   `Collapsible` body is audited because its addresses still answer
>   with the body, whereas an absent `Option` branch answers with its
>   container and is exempt. The action accepts both codes.

`scripts/drive/harness.py`:
- `_pick` clicks the sidebar row by the exact-text selector `~=Name`
  (T27.12), then asserts the title shows the name.
- `_menu_row` is deleted.
- `readout()` keeps `#readout`, which is now the chip.
- The per-widget modules keep parsing the readout formats unchanged.

`scripts/agent_drive.sh`:
- The Label→Switch pick goes through the sidebar.
- The dropdown section retargets to the `Select` **demo** (its own
  trigger and menu ids inside `#demo`).
- The palette check reads the sidebar's background, since the header
  band no longer exists. It must still change light→dark.

**Gates.** `just widget-drive` (94 actions) and `just agent-drive` stay
green under **both** strategies, with the shadow oracle quiet. Adding a
sidebar row is a new hover target, so the oracle is the check that
hover marks stay precise.

### T56.5 — docs

- Rewrite `docs/plan-widgets-gallery.md` §3 for the new layout.
- `docs/plan-showcase-design.md` is written first, as T56.0's commit,
  holding this plan and a "what building it found" section.
- Update the showcase mentions in `book/src/styling.md` (palette) and
  `book/src/agent.md`.
- Mark the queue entry done in AGENT.md.

## Critical files

- `examples/showcase/src/main.rs`: tree, `Store`, `build`, `rebuild`,
  `refresh`, `stylesheet()`, palettes, tests.
- `examples/showcase/src/catalog.rs`: `Entry`, `entry!`, `CATALOG`.
- `examples/showcase/src/nav.rs`: new.
- `examples/showcase/src/stage.rs`: untouched, except if a demo rule
  needs its backdrop fixed.
- `scripts/drive/harness.py`, `scripts/drive/widgets/choose.py`,
  `scripts/agent_drive.sh`.

**Reused:**
- `Repeat` with wheel scroll (`lumen_static::repeat`);
- `wid_index!` (T26.14);
- the `~=` selector (T27.12);
- `Divider`, `Label`, and the M25 palette roles;
- the existing FILL-spacer centring.

## Verification

1. Run `just test`. It includes the showcase unit tests, the new
   sidebar, note and contrast gates, and the sheet lints.
2. Run `just widget-drive` and `just agent-drive`: both strategies
   green, shadow oracle 0 divergences.
3. Run `cargo run -p showcase` and click through groups. Check wheel
   scrolling of the sidebar, the selected-row highlight, the chip
   updating, and `LUMEN_PALETTE=dark`.
4. Take `--shot` of all 51 entries in both palettes and review them
   against the T56.0 baseline.
5. Run each new gate's injection once, and record the ratios or
   mismatches in the commit bodies.
