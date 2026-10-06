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

These were absent from lumen-static and present in **every** other arm
when this survey was taken (2026-09-06). **Most have shipped since**;
re-checked against `553fa5e` on 2026-10-04 (M80/T80.0), each row now
says what exists and which plan owns what is left. The per-framework
sections below are still the 2026-09-06 survey.

| capability | status at `553fa5e` |
|---|---|
| **Images** | Shipped: `Cmd::Image`, the `Image` widget, `ImageSource`; PNG and GIF decoding behind the shell's `image` (M84/T84.1) and animated images — a `Playback` writing an `Image`'s `frame`, the window woken by `App::wake_in_ms` (M84/T84.3, `docs/results-images.md`). Video: a controlled `Video` widget over a `VideoFeed` subscription, with `GifFeed` as the feed that ships (M84/T84.8, `docs/results-images.md`); a real codec — the GStreamer crate `lumen-static-video` — is deferred on K84b: the reference machine lacks GStreamer's development libraries (M84/T84.10, `docs/results-images.md`, with the reopen condition). |
| **Vector drawing** | Lines and linear gradients shipped (`Cmd::Line`, `Cmd::Gradient`). Paths, radial and style gradients, vector text: `docs/plan-paint-vocabulary.md` (M82). |
| **Animation** | Shipped: `Animated<T>`, `Easing`, `App::tick`, timers (M28). Built-in widget transitions: `docs/plan-transitions.md` (M86). |
| **Overlays and z-order** | Shipped: popups, menus, tooltips, modals, toasts, nested overlays (M15, M27). Arbitrary z-order stays rejected (`docs/plan-capabilities.md`). |
| **Menus** | `Menu` shipped (M27). Right-click context menus, a menu bar and a tray: `docs/plan-desktop-integration.md` (M90). |
| **Multi-window** | Not in a turnkey window (M54/T54.7 un-marked T15.10). `docs/plan-windows.md` (M89). |
| **Drag and drop** | Within an application shipped (`Drag`); the platform half compiles behind `dnd` and is not routed by `run` (M54/T54.7). |
| **Touch and gestures** | Shipped: touch events and recognisers (`gesture.rs`, M15; M44/T44.4). |
| **Complex-script text** | Measured and edited (`lumen-static-text`, M45), not painted. `docs/plan-shaped-paint.md` (M81). |
| **Localisation** | RTL layout mirroring exists (T15.9); no catalogue or locale formatting. Not planned. |
| **Focus ring / keyboard nav beyond Tab** | Shipped: arrow groups, focus traps, a keyboard-only focus ring (M15/T15.6, M44). |
| **File / colour / font dialogs** | File dialogs behind `dialogs` (T15.11); colour, date and file pickers as widgets (M27/T27.11). |
| **GPU rendering** | Shipped: `lumen-static-wgpu`, behind `gpu` (T15.12, M51). Custom shaders and backends: `docs/plan-gpu-extension.md` (M83). |

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
- **A style language — closed in part by M61 (2026-09-28).**
  `lumen-style` (4.3k lines) is a `.lss` lexer/parser/AST and a resolver.
  lumen-static now reads `.lss` too — Lumen's lexer and parser, trimmed
  to one simple selector per rule, `@tokens`, palette roles and `@sheet`
  blocks, parsed at compile time by `include_lss!` into the same `const`
  rules as hand-written Rust, and re-read at run time in debug windows
  (`docs/plan-lss.md`, `docs/results-lss.md`). What is still Lumen's
  alone: compound selectors, combinators, `:state`, `@media`,
  `@keyframes`, `@theme` and run-time classes — each a named compile
  error here, because the cascade has no machinery for them.
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
