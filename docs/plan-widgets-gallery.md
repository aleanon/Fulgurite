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
showcase is as much a demonstration of the answer as of the widgets.
Three ways out, and the third is used:

1. Every demo in the tree, all but one hidden — what `Either` does for
   two. For eighteen it is seventeen nested `Either`s, and the merged
   const table is bounded by `MAX_EITHER_NODES` (64) per instantiation.
2. A stylesheet rebuilt per selection, flipping `Style::visible`. The
   cascade can say it, but every demo still pays layout every frame.
3. **`Dyn`** — a boxed `dyn Node` island with `replace`. One node in the
   parent's table, content swapped on selection. Precisely what
   M13/T13.4 put the hatch behind `alloc` *for*.

Its cost is real and is recorded on `Entry::live`: a `Dyn` has no typed
handle, so a widget that owns its own value — a `Slider`, a
`TextInput` — cannot be read back out. Those entries print no readout.

**The menu is a field per entry, not a `Repeat`.** Both off-table
containers were tried first; neither painted a row, which is a seam to
understand before leaning on it for a menu. A catalogue's length is
known when the binary is, so the static answer is the honest one — and
`ROWS` is asserted against `CATALOG.len()` so the two cannot drift.

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
