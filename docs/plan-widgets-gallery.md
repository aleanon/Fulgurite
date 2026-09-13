# M23 — the widget set, and the showcase that lists it

> **Started 2026-09-13.** The ask: Lumen's `examples/widget_showcase`,
> rebuilt here — a dropdown listing every built-in widget, each one
> demoed on a stage — and the widgets it needs, in their own crate.
>
> **Tranche 1 has landed**: `lumen-static-widgets` with eleven leaf
> widgets, and `examples/showcase` with the picker and eighteen entries.
> The rest of this document is the map.

## The scale, measured rather than guessed

Lumen's showcase lists **61 widgets in 8 groups** (counted from
`/mnt/Utvikling-linux/Lumen/examples/widget_showcase/src/catalog.rs`),
backed by a `lumen-widgets` crate of 17.5k lines. `lumen-static` had
**nine** of them. So parity is ~52 widgets, and the honest framing is
that this is a milestone in the same weight class as M22, not a commit.

**It is also harder here than there, for a reason worth stating.** In a
dynamic tree a widget is a function returning an `Element`; a container
takes `Vec<Element>` and nothing about its shape is known early. Here a
widget is a *type* with a const `NODE_COUNT`, `DEPTH` and `SUB_TABLE`,
and a container's children are its fields. That makes leaves **cheaper**
— no children to splice, no depth to carry — and composites dearer.
Tranche 1 is every leaf worth having, which is why it moved fast.

## Where the 61 stand

| group | Lumen | here |
|---|---|---|
| **Text & display** (15) | Label, RichText, Markdown, Icon, Image, Avatar, Badge, Card, Chip, Rule, Space, Skeleton, Spinner, Toast, Tooltip | **done:** Label, Image, Spacer (=Space), Icon, Avatar, Badge, Chip, Divider (=Rule), Skeleton, Spinner · **left:** RichText, Markdown, Card, Toast, Tooltip |
| **Buttons & input** (16) | Button, CheckBox, Switch, Radio, Slider, RangeSlider, Stepper, TextInput, TextField, SearchField, RichTextEditor, FindReplaceBar, ColorPicker, DatePicker, TimePicker, FilePicker | **done:** Button, CheckBox, Switch, Radio, Slider, TextInput, TextArea (≈TextField) · **left:** RangeSlider, Stepper, SearchField, RichTextEditor, FindReplaceBar, ColorPicker, DatePicker, TimePicker, FilePicker |
| **Choice** (4) | PickList, Combobox, Select, Menu | **left:** all four. The showcase's own picker is the prototype — §3. |
| **Layout** (8) | Container, AlignBox, Wrap, Grid, SplitPane, PaneGrid, Scrollable, Accordion | **done:** the derive's row/column/stack (≈Container/AlignBox), ScrollView (=Scrollable) · **left:** Wrap, Grid, SplitPane, PaneGrid, Accordion |
| **Lists & data** (4) | VirtualList, DataGrid, Tree, Pagination | **done:** Repeat (=VirtualList), List · **left:** DataGrid, Tree, Pagination |
| **Charts & progress** (5) | BarChart, LineChart, PieChart, ProgressBar, Canvas | **done:** all but Canvas |
| **Navigation** (5) | Tabs, AppBar, BottomNav, NavigationRail, PullToRefresh | **left:** all five |
| **Overlays** (4) | Modal, Popover, Sheet, Drawer | **done:** Popup (≈Popover) · **left:** Modal, Sheet, Drawer |

**Twenty-two of sixty-one**, counting the core's. Thirty-nine left.

## 1. The crate, and why it is a crate

`lumen-static-widgets`, and the argument is the one this workspace makes
everywhere: **the crate is the gate**. A widget set grows without limit,
and an embedded profile that links a pie chart it will never draw has
paid `.text` for nothing — the M15 gating policy. Nothing here is in the
core; an application opts in by naming the crate.

`leaf_common!` was exported from the core to make this possible
(M23/T23.0). `Node` has four methods every leaf answers identically, and
a widget crate outside the core had no way to reach them — so the first
out-of-core widget would have copied twenty lines, which is the shape
that let `Shadow` and `Image` hash as the same tag for a year.

## 2. What tranche 1 shipped

Eleven leaves: `Divider`, `Badge`, `Chip`, `Avatar`, `Icon` (+`Symbol`),
`ProgressBar`, `Spinner`, `Skeleton`, `BarChart`, `LineChart`,
`PieChart` (+`Series`).

Three decisions inside them worth keeping:

- **`Icon` is a closed enum drawn from primitives, not a font
  codepoint.** A glyph-based icon set needs the icon font present and
  correct at every scale on every platform, and the failure mode when it
  is not is the missing-glyph box — which is exactly how
  `examples/glasswall` lost twenty-two of its twenty-four icons.
- **`Series` is a fixed array**, like `Stops` and `Regions`, so a chart
  allocates nothing and is `Copy`. Over-long input is truncated, not
  refused: a chart is a read-only view of someone else's numbers.
- **`Divider`, where Lumen says `Rule`.** `lumen_static::Rule` is
  already a *stylesheet* rule and is in the prelude. A widget set may
  differ from its model where the model's name is taken.

## 3. The showcase, and the one hatch it needs

`examples/showcase`: a picker at the top, the chosen widget centred on a
stage, a live readout for the ones that carry state.

**"One of N" is the interesting problem in a static tree**, and the
showcase got it wrong first. Three candidates:

1. Every demo in the tree, all but one hidden — what `Either` does for
   two. For eighteen it is seventeen nested `Either`s, and the merged
   const table is bounded by `MAX_EITHER_NODES` (64) per instantiation.
2. A stylesheet rebuilt per selection, flipping `Style::visible`. The
   cascade can say it, but every demo pays layout every frame.
3. `Dyn` — a boxed `dyn Node` island with `replace`.

**Three was chosen and it was wrong — and then it was fixed in the
core.** `Dyn`'s `Handlers<S>` returned `None` unconditionally, because
the application type is exactly what the erasure erased. Every button,
checkbox, radio and text field in the showcase could be hit-tested,
hovered and pressed, and no handler ever ran. Nothing that stopped at
"the demo paints" could see it — and the first round of tests stopped
there.

**`Dyn` routes handlers now** (M23/T23.1b). The island boxes a
`dyn NodeFor<S>` — a supertrait of `Node` with a blanket impl for
anything that already answers `Handlers<S>` — instead of a bare
`dyn Node`, and `Dyn<S>` forwards to it. The cost is that an island in
an application's tree is `Dyn<TheApp>` rather than `Dyn`; `Dyn<()>`
remains for content that only paints.

The answer is **one above the list: every demo is a typed `Option`
field, and exactly one is `Some`.** `Option` is the core's own hide
(`layout` zeroes an absent branch every frame) and `Option<W>` forwards
`Handlers` to `W`. It also buys back what the island cost: a typed field
can be *read*, so a `Slider`'s value reaches the readout, where through a
`Dyn` it could not and `Entry` carried a flag apologising for it.

What it costs is that the tree carries all eighteen demos — space for
reach, which is the trade this architecture offers — and one new failure
mode: `Demos::clear` is a hand-written list of eighteen fields, and one
left out leaves the previous demo underneath the new one.
`Demos::shown()` is asserted to be exactly 1 after every selection.

**And the cascade runs once.** The second bug in the same file, and the
more surprising one: `set_stylesheet` resolves every node's style *once*
from `Node::defaults()`, and `Option<W>::node` returns the **`Option`**
when it is `None`. So every demo that was absent at that moment had its
style resolved from `Spacer`'s defaults — transparent background, no
size of its own — and painted as bare text. On screen, every widget
looked like a label.

The tree is therefore built **full**, cascaded, and reduced to one demo
on the first pump: `Demos::fill_all` exists for the cascade's benefit
and nothing else. The style array is indexed by node and hiding does not
touch it, so the resolved styles survive.

**Three lessons for T23.2 onward.** A widget's `defaults()` is a floor
that keeps it *visible*, not a look — a showcase supplies the style it
means to show, and `TextInput` and `Button` ship plain on purpose. A
subtree that is `None` when the stylesheet is set is a subtree styled as
a `Spacer`. And a widget test that asserts only "it drew something" will
pass on a widget nobody can use *and* on one that draws only its text.

**The menu is a field per entry, not a `Repeat`.** Both off-table
containers were tried; neither painted a row, which is a seam to
understand before leaning on it for a menu, and T23.8 owns it. A
catalogue's length is known when the binary is, so `ROWS` is asserted
against `CATALOG.len()`.

## 3b. What driving it found

Reading a showcase is not inspecting it. Five defects survived the first
two rounds of tests because every one of those tests asked "did it
paint", and all five painted.

| what | why nothing caught it |
|---|---|
| **The switch's knob inverted with its track** — an *off* switch drew a grey track with an **accent-coloured** knob, which reads as on | `Toggle` swaps `(body, mark)` on the state boundary, which is exactly right for a checkbox and a radio, where the mark exists only when on. A switch draws its knob in **both** states, so it is the one shape the two-colour scheme does not cover. Fixed in the core; `tests/switch_knob.rs` holds the knob lighter than its track in both states, the same colour in both, and the *track* different between them — the third is what stops the first two being satisfied by a switch that never changes. |
| **A stylesheet rule matching no node** — the `ProgressBar`'s field is `progress`, so its id is `d_progress_a`, and the rule said `d_progressbar_a` | A dead rule is silent: the cascade skips it and the widget keeps its bare `defaults()`, so the bar rendered full-window and flat. `tests::every_rule_matches_a_node` compares the sheet's ids against `SUB_TABLE`'s. |
| **Ghost buttons showed no press at all** | `pressed_dim` and `hover_dim` *scale* a background; they cannot invent one, and a fully transparent button has nothing to scale. A third of "clicking does nothing" was this. |
| **Animated demos opened at phase 0** — an empty bar, an unlit spinner | Indistinguishable from a broken widget in a still frame. The app opens mid-travel. |
| **An avatar's CJK initials drew as the missing-glyph box** | The same trap `examples/glasswall` paid for. A demo has to draw on the machine it ships to. |

Two tools now exist for this and should be used before any T23.x is
called done: `--demo <n>` renders one entry cropped to the laid-out
geometry of the demo itself, and `--seq` renders every frame of a click
— rest, held, released, and a second control held. **A press exists
between two events**, so a screenshot taken after the release shows the
resting frame, which is why "clicking does nothing" is a report a still
image can neither confirm nor deny.

One known cost, recorded rather than fixed: the showcase's frame never
settles, because `#[app(after_update = ...)]` reports nothing and the
derive therefore marks every pump changed (`plan-invalidation.md`
T16.10). It uses the hook to keep a `Slider`'s readout following a drag
— a slider owns its value and has no change callback to hang it on.
Damage itself is exact: a press reports the button's own rect and
nothing more.

## 4. What is next, in order

| id | task |
|---|---|
| **T23.2** | Card, Tooltip, Toast — leaves with a shadow and a tip. |
| **T23.3** | Stepper, RangeSlider, SearchField — composites over what exists. |
| **T23.4** | Tabs, AppBar, BottomNav, NavigationRail — the nav group, all one shape. |
| **T23.5** | Modal, Sheet, Drawer on the overlay layer M15 built. |
| **T23.6** | Grid, Wrap, SplitPane, Accordion — the layout group, which needs Widget-level work. |
| **T23.7** | PickList/Combobox/Select/Menu, generalising the showcase's own picker. |
| **T23.8** | DataGrid, Tree, Pagination, and the off-table paint seam §3 names. |
| **T23.9** | ColorPicker, DatePicker, TimePicker, FilePicker. |
| **T23.10** | RichText, Markdown, RichTextEditor, FindReplaceBar, Canvas. |

Each is one commit, each ends with `just check` green, and each names the
injection its tests must go red under — `AGENT.md`'s rule, which tranche
1 was held to: six injections against eleven widgets, and one of them
found a bound computed from the wrong quantity.
