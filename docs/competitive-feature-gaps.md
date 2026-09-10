# What the others have that lumen-static does not

*Surveyed 2026-09-06, from the sources on this machine — not from
memory. Qt 6.4.2 and GTK 4.14.5 headers as installed, `iced_widget`
0.14, `masonry` 0.4, `i-slint-core` 1.17.1 from the cargo registry, and
the Lumen tree at `/mnt/Utvikling-linux/Lumen`. Companion to
`docs/results-competitive.md`, which measures the same six.*

> This is a **capability** list, not a defect list. A third of it is
> deliberate scope recorded elsewhere — `docs/plan-widgets.md` T11.5 for
> overlays, `docs/text-render-adoption-decision.md` for shaping, and the
> whole of `docs/plan-embedded.md` for why the draw vocabulary is six
> commands. Where a gap is a decision, it says so.

## The baseline it is measured against

**Widgets (14):** `Label`, `Button`, `Fixed`, `Spacer`, `TextInput`,
`TextArea`, `ScrollView`, `Slider`, `Toggle` (checkbox/radio/switch),
`List`, `Repeat`, `Either`, `Dyn`, `Assembled`.

**Draw vocabulary (6 commands):** `Rect`, `RoundedRect`, `Text`,
`Shadow`, `PushClip`, `PopClip`.

**Layout:** row and column with `Length::{Fixed, Fill, Shrink}`,
padding, spacing, cross-axis `Align`. One axis per container.

**Styling:** a `Rule`/`Selector`/`Patch` cascade over 15 resolved
properties, set in Rust.

**Input:** pointer (3 buttons, click counts, capture), keys (17
variants, two of them parameterised — `Char(char)` and `F(u8)` — so the
addressable key space is larger than the count), modifiers, text, IME composition,
scroll.

---

## Common to all six — the gaps that are not about any one framework

These are absent from lumen-static and present in **every** other arm.
If the list is read for priorities, it should be read here.

| capability | what is missing |
|---|---|
| **Images** | No `Image` widget and no `Cmd` that carries pixels. No PNG/JPEG/WebP decode, no nine-patch, no icons. |
| **Vector drawing** | No paths, strokes, gradients, or line primitives. `Rect`/`RoundedRect`/`Shadow` is the whole geometry vocabulary. No SVG. |
| **Animation** | No timers, no clock, no easing, no transitions, no spring physics. Nothing in the framework can change a value over time; an application must drive every frame itself. |
| **Overlays and z-order** | No popup, dropdown, menu, tooltip, modal or toast. Deliberate — `docs/plan-widgets.md` T11.5 records this as a *layout* milestone (out-of-flow positioning, paint-after/hit-first, click-outside dismissal), not three widgets. |
| **Menus** | No menu bar, no context menu. Follows from overlays and from having no right-click routing above the event vocabulary. |
| **Multi-window** | `Host` is one tree, one surface. No second window, no dialog, no popup surface. |
| **Drag and drop** | Neither within an application nor across the platform boundary. Pointer capture (T11.2) is the substrate; nothing is built on it. |
| **Touch and gestures** | The event vocabulary has no touch, pinch, rotate, swipe or long-press. Pointer only. |
| **Complex-script text** | No bidi, no Arabic/Indic shaping, no grapheme-correct segmentation. Deliberate and recorded twice in `docs/text-render-adoption-decision.md` — fontdue does not do it, and the baked embedded face does not either. |
| **Localisation** | No translation catalogue, no locale-aware formatting, no RTL layout mirroring. |
| **Focus ring / keyboard nav beyond Tab** | Tab traversal exists; arrow-key navigation within a group, focus scopes, and a visible focus indicator do not. |
| **File / colour / font dialogs** | None. These are platform-owned in Qt, GTK and Slint. |
| **GPU rendering** | Software rasteriser only. `Renderer` is the seam a GPU backend would plug into (M10a) and nothing has. |

---

## Qt 6.4.2

The widest gap, because Qt is an application platform rather than a UI
library. Of the modules installed here — `QtCore`, `QtGui`,
`QtWidgets`, `QtNetwork`, `QtSql`, `QtDBus`, `QtConcurrent`,
`QtPrintSupport`, `QtOpenGL`, `QtOpenGLWidgets`, `QtTest`, `QtXml`, plus
seven embedded/EGLFS integration modules:

- **Non-UI subsystems entirely absent here**: an SQL layer with drivers
  and models, an HTTP/TCP/TLS stack, D-Bus IPC, a thread-pool and
  future/promise library, XML parsing, and printing.
- **Model/view framework** — `QAbstractItemModel` with proxy models,
  sorting, filtering and selection, shared by list, table and tree
  views. lumen-static has `List` and `Repeat` and no model layer at all.
- **Item views**: tree view, table view, header views, editable
  delegates.
- **Embedded/EGLFS backends** — direct KMS/DRM, framebuffer and input
  integration for devices with no window system. Notable because M13
  targets that class of hardware and lumen-static has no display driver
  of any kind (`docs/results-embedded.md` says so explicitly: the
  fixture computes a checksum, it does not talk to hardware).
- **Style engine with platform themes**, so an application inherits the
  desktop's look.
- **`QtTest`** — a GUI test framework with synthetic input and widget
  introspection. (Nearest equivalent here is the agent protocol, which
  is arguably better at this specific job; see the end.)
- Internationalisation: `QTranslator`, locale-aware number/date
  formatting, RTL layout mirroring.

## GTK 4.14.5

The largest *widget* vocabulary of the six. Present in GTK and absent
here, from the installed headers:

- **Virtualized, model-backed views**: `GtkListView`, `GtkColumnView`,
  `GtkGridView` over a `GListModel`, with `GtkSorter`, `GtkFilter`,
  `GtkSelectionModel` and `GtkTreeListModel`/`GtkTreeExpander` for
  trees. lumen-static's `Repeat` virtualizes; it has no model, sorting,
  filtering or selection layer.
- **Rich text**: `GtkTextView` with `GtkTextBuffer`, tags, marks and
  iterators. `TextArea` here is a flat string with a caret.
- **Layout managers as objects**: `GtkConstraintLayout` (a real
  constraint solver), `GtkGridLayout`, `GtkFixedLayout`,
  `GtkOverlayLayout`, `GtkCenterLayout`, plus `GtkSizeGroup` for
  cross-container alignment. lumen-static has one axis per container and
  no constraint solving.
- **Containers with behaviour**: `GtkPaned`, `GtkNotebook`,
  `GtkExpander`, `GtkRevealer`, `GtkStack` + `GtkStackSwitcher`,
  `GtkFlowBox`, `GtkAspectFrame`, `GtkHeaderBar`, `GtkActionBar`.
- **Gesture recognizers as first-class objects**: click, drag,
  long-press, pan, rotate, stylus, swipe, zoom — plus scroll, motion,
  key and focus event controllers.
- **Input methods**: `GtkIMContext`, `GtkIMMulticontext` — a pluggable
  IME layer. lumen-static handles the winit `Ime` events (T14.8) and has
  no equivalent abstraction.
- **Media**: `GtkVideo`, `GtkMediaStream`, `GtkMediaControls`.
- **Dialogs and pickers**: file, colour, font, print, alert, about,
  emoji chooser.
- **`GtkCssProvider`** — a real CSS engine, loadable at runtime.
  lumen-static's cascade is Rust values with no parser or stylesheet
  file.
- **Accessibility as a widget property tree** (`GtkAccessible`,
  `GtkAccessibleText`, `GtkAccessibleRange`) rather than a snapshot.
- Drawing: `GtkDrawingArea`/`GtkSnapshot` gives an application arbitrary
  Cairo/GSK drawing. There is no escape hatch here for custom painting
  beyond implementing `Node::paint`.
- Shortcuts: `GtkShortcutController`, `GtkShortcutsWindow` — declarative
  accelerators with a generated help window.

## iced 0.14

Closest in spirit — Rust, CPU-rasterisable, declarative. Its widget set
is roughly 3× lumen-static's. Present in `iced_widget` and absent here:

- `canvas` — arbitrary 2D drawing with paths, strokes, fills, gradients.
- `shader` — a wgpu escape hatch for custom GPU rendering.
- `image`, `svg`, `qr_code`, `markdown` — content widgets.
- `text_editor` — a multi-line editor with its own content model
  (lumen-static's `TextArea` is a string and a caret).
- `pane_grid` — draggable, splittable, resizable panes.
- `table`, `grid` — tabular layout.
- `combo_box`, `pick_list`, `tooltip`, `overlay` — all overlay-dependent.
- `progress_bar`, `rule`, `toggler`, `float`, `pin`, `stack`.
- `scrollable` with programmatic scroll-to and scrollbar styling.
- `lazy`, `keyed`, `responsive`, `themer` — view-tree machinery with no
  analogue in a static tree (and mostly not wanted: see below).
- `mouse_area`, `sensor` — hit-region and visibility observers.
- A **theme system** with light/dark palettes and per-widget style
  functions.

## Xilem / masonry 0.4

Measured through masonry, Xilem's widget and layout layer. Present there
and absent here:

- `virtual_scroll` — virtualized scrolling as a container, and
  `scroll_bar` as a widget.
- `portal` — content rendered outside its parent's bounds; the
  substrate overlays need.
- `zstack`, `indexed_stack` — z-ordered and switched stacking.
- `split` — draggable splitter.
- `grid`, `align`, `sized_box` — layout containers.
- `prose`, `variable_label` — rich and animated text.
- `image`, `progress_bar`, `spinner`.
- **Xilem's own layer**, above masonry: a reactive view tree with diffing
  and stable element identity. Deliberately not wanted here —
  `docs/plan-lumen-parity.md` lists the `Element`/rebuild machinery
  under "what we should *not* copy", and `docs/results-competitive.md`
  §1 is what having no view tree buys.

## Slint 1.17

The most different: a language and compiler, not a Rust API.

- **A declarative DSL (`.slint`) with a compiler** — types, expressions,
  bindings and layouts in a separate language, compiled to Rust. The
  nearest thing here is `#[derive(StaticTree)]`, which encodes topology
  but not values, bindings or layout.
- **Reactive properties with two-way binding and change tracking**
  (`properties.rs`, `two_way_binding.rs`, `change_tracker.rs`).
  lumen-static's `#[bind]` is one-directional and recomputed per frame.
- **Animations and easing as language constructs**, plus
  `physics_simulation.rs`.
- **A model/repeater abstraction** with observable collections.
- `popup.rs`, `menus.rs` — overlays and menus.
- `drag_n_drop.rs`, `data_transfer/` — drag and drop and clipboard as
  core concerns.
- `system_tray/` — tray icons.
- `translations.rs` — i18n in the language.
- `flickable.rs` — kinetic/inertial scrolling. lumen-static's scroll is
  positional with no momentum.
- `partial_renderer.rs` — its own damage-tracking renderer (the one
  thing on this list lumen-static has an equal of).
- **Backends**: winit, Qt, and **linuxkms** — direct KMS/DRM for
  embedded devices with no window system. Plus wgpu integration.
- `bitmapfont.rs` — a baked font path, which lumen-static also has
  (T13.7), and Slint additionally has a full shaping stack beside it.
- `accessibility.rs`, `date_time.rs`, `timers.rs`, `styled_text.rs`.

## Lumen

The sibling project, and the closest architectural comparison. Its
widget layer is ~17.5k lines against lumen-static's core; the survey in
`docs/plan-lumen-parity.md` predates M11–M14 and its widget counts are
stale, but the *shape* of the gap is unchanged. Present in
`lumen-widgets` and absent here:

- **Content widgets**: `charts`, `markdown`, `richdoc`, `color_picker`,
  `file_picker`, `combobox`, `pick_list`, `search_field`,
  `range_slider`, `progress_bar`, `accordion`, `card`, `panes`, `sheet`,
  `popover`, `overlay`, `nav`/`nav_chrome`, `grid`, `lists`, `forms`.
- **`undo.rs`** — an undo/redo stack as framework machinery.
- **`motion.rs`** — animation and transitions.
- **`i18n.rs`** and **`wcag.rs`** — localisation, and accessibility
  *conformance checking* beyond a snapshot.
- **`theme.rs` / `design.rs`** — design tokens and a theme system.
- **`shader.rs`** — a GPU escape hatch.
- **A style language.** `lumen-style` (4.3k lines) is a `.lss`
  lexer/parser/AST. lumen-static's cascade has no parser and no file
  format.
- **Platform shells**: `lumen-shell-web`, `lumen-shell-android`,
  `lumen-shell-ios`. lumen-static is desktop-only, plus a `no_std` core
  with no driver.
- **`lumen-cli`** — a project CLI and dev server.
- **`lumen-test`** — locators, snapshots and traces as a library.
  lumen-static has the agent protocol and session replay (M12), which
  covers much of the same ground from the other side.
- **A real text stack** (`lumen-text`, parley/swash): complex scripts,
  bidi, font fallback chains.
- **A renderer with a full 2D vocabulary** (`lumen-render`), including
  paths, gradients and images.

---

## How much of this is a decision rather than a hole

Worth separating, because the list above reads as 60 missing things and
is not that.

**Decided against, with the reasoning recorded:** the reactive view tree
(`plan-lumen-parity.md`, "what we should not copy"), complex-script
shaping and a heavyweight text stack
(`text-render-adoption-decision.md`), a GPU backend (M10a defines the
seam and defers the backend), and adopting Lumen's render/text/style
crates (measured at 74–84 transitive dependencies and declined).

**Scheduled and specified but not built:** overlays, and everything that
depends on them — dropdown, tooltip, modal, context menu
(`plan-widgets.md` T11.5, gated on a stack/absolute layout mode).

**Genuinely absent with no decision either way:** images, vector
drawing, animation, multi-window, drag and drop, touch, a model layer,
rich text, a style file format, and localisation. These are the ones a
reader should treat as a backlog.

**And the two things that run the other way**, since a gap list read
alone is misleading: no framework here has a **no-allocator profile**
(`docs/results-embedded.md`: 22 458 bytes of `.text` at 2026-09-10,
zero `.bss`), and
none has **source provenance on every node** exposed through an agent
protocol. Both are consequences of the static tree, and both are why
several rows above are hard rather than merely unwritten.
