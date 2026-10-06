# M80–M90 — what iced's roadmap to 1.0 has that this framework does not

> **Written 2026-09-28; revised 2026-09-29 after a seven-lens review
> (202 findings, 199 survived verification: 19 blocking, 118 major, 62
> minor; 3 refuted; plus 9 from a completeness critic).** Not started.
> Read against `92a4033`, the merge of `m56-showcase-design` into
> `main`, and re-checked on 2026-09-29 against `553fa5e`, which merges
> M61 (`.lss`, `just ci-verify 0793dd5` green) into that candidate.
> **Renumbered 2026-10-04 from M62–M72 to M80–M90** (tasks, kill rules
> and the review lists with them), because an unmerged branch,
> `m62-m64-plans`, had claimed M62–M64 for `.lss` selector and style
> work first. The review findings' own ids (C1, G27, …) are unchanged.
> Every "exists" and "missing" below was checked against the tree, not
> against a gap list. Of the two gap lists, only
> `docs/competitive-feature-gaps.md` still carries stale rows;
> `book/src/gaps.md` already says its survey table is stale and that
> lines and gradients shipped (review C20).
>
> **Source.** iced's public roadmap board
> (`whimsical.com/roadmap-iced-7vhq6R35Lp3TmYH4WeYwLM`), read on
> 2026-09-28 from the board's own data rather than a screenshot: eight
> milestone columns from "Advanced Text" to "Endgame" (→ 1.0), their
> bonus lanes, and each item's status (Finished, In Progress, Pending,
> Canceled). §1 maps every item to this repository.
>
> **Excluded at the owner's request:** font hinting (already declined in
> `docs/text-render-adoption-decision.md`), time-travel debugging, and
> the Comet debugger.
>
> **Owner decisions, binding on every section (2026-09-29):**
> - **D1** — no new crate. Four moves break the crate-boundary blockers
>   instead (§2.7), and a `lumen-static-core` crate was considered and
>   not chosen (see "Considered and not chosen").
> - **D2** — on Linux the framework draws its own menu bar from the same
>   `MenuEntry` data; native `muda` only on macOS and Windows. **M90
>   defers the native half** (no macOS or Windows target is installed
>   on the reference machine, and a skipped gate means not done; it
>   reopens with a macOS or Windows leg in `scripts/ci.sh`), so the
>   drawn bar is the menu bar everywhere until then. The Linux tray uses
>   `ksni` over D-Bus, which needs no GTK thread.
> - **D3** — `shader` (`Cmd::Shader`, `Painter::shader`) is behind a
>   default-off cargo feature. **The variants are gated too, at every
>   use site (owner, 2026-09-30).** `Cmd::Shader`/`Cmd::ShaderEnd` and
>   `Painter::shader`/`shader_end` exist only under
>   `cfg(feature = "shader")` in `lumen-static`, and every match arm or
>   method that names them carries that `cfg` in the core and
>   `cfg(lumen_shader)` in every other crate, a cfg the core's build
>   script hands its direct dependents (M83, "Feature placement"). So
>   each match shows the variant is behind a feature, and K83b's +0 B
>   holds by construction. Paths and the widened gradient are accepted
>   firmware growth, measured per task (K4); `text_spaced` and `fills`
>   are accepted under D6 (2026-10-02), not D3. **Two allowances go
>   unused**: M81 shapes inside
>   `FontStack` and adds no glyph command, and M82 carries its
>   transform on each path, so no `glyphs` or `push_transform` /
>   `pop_transform` method is built.
> - **D4** — `Style` and `NodeState` grow as needed, and every task
>   that grows either prints the new size and the RAM per 100 nodes
>   against the 24 KB ceiling (K1).
>
> **Owner decisions, 2026-09-30:**
> - **D5** — the firmware `.text` ceiling is **80 KB (81,920 B)** for
>   every firmware binary, raised from 64 KB (65,536 B). 80 KB is 15.6%
>   of the 512 KB part, still under the budget's own line that "a GUI
>   framework that takes more than a fifth of the part is not an
>   embedded GUI framework" (`docs/results-embedded.md`, budget row).
>   `firmware_interactive` then has **19,998 B** of headroom. K4's
>   per-task cap (1,000 B) and per-task printed delta are unchanged.
>   T80.0 performs the raise (§2.1, K4).
> - **D3's variants** — gated at every use site (D3 above; M83). This
>   reverses M83's first draft, which left the variants ungated and
>   gated only the methods and the table.
>
> **Owner decisions, 2026-10-02:**
> - **D6** — the required `Painter` methods `text_spaced` (M80) and
>   `fills` (M82) are **accepted firmware growth**, beside D3's paths
>   and widened gradient. D3 alone never accepted them (second review
>   R6). Each is still measured per task by `scripts/firmware_delta.sh`
>   and bounded by K4's 1,000 B. Nothing else is accepted: the `Node`
>   slot `caret_rect` (M80) and the `TextMeasure::outline` slot (M82)
>   are measured under K4 and reported to the owner with the number.
> - **D7** — **`xclip` is installed on the reference machine** before
>   M88/T88.4, as an explicit prerequisite (§3.4). It is absent today
>   (`command -v xclip` finds nothing, 2026-10-02). The primary-selection
>   leg of T88.4's drive then always runs there: a missing `xclip`
>   fails by name with the install instruction, and never reports
>   *skipped*.
>
> **Gate discipline, doc currency and closing rules are in §3.** The old
> header's italics were replaced by it, because two of their rules (gates
> committed red and ignored first; docs in a trailing task) contradicted
> the compiler and AGENT.md respectively.

## 0. The claim, and what would falsify it

**The claim:** every roadmap item iced has finished or is building
toward 1.0 that is meaningful for a static-tree, `no_std`-core framework
can be built here **without giving up the four goals** (M38): peak
performance, only-what-is-used compiled in, the agent, and hot reload.
Concretely, each capability below is:

- **default-off** where it costs a dependency, behind a feature
  measured in crates and bytes, following `lumen-static-winit`'s
  manifest. Its figures are cited, not restated: `a11y`'s from
  `docs/results-a11y-platform.md` (95 → 166 crates, +4.12 MiB stripped),
  the GPU's
  from `docs/results-gpu.md` §5–§6. The manifest's three crate deltas
  (clipboard "11", 207 → 218; a11y "62"; dialogs "79", 218 → 297;
  `crates/lumen-static-winit/Cargo.toml`:47, :90–91, :115) were
  counted by another method than K2's, so T80.0 re-measures all three
  by K2's command, records them in `docs/results-a11y-platform.md`,
  and rewrites the three comments with the method named. At `553fa5e`
  (2026-10-02, second review B8; re-run for consistency-3 item 1): 96
  default, 91 with `--no-default-features` (clipboard +5, not 11), 106
  with `dialogs` (+10, not 79), 167 with `a11y` (+71, matching 95 →
  166);
- **paid for in the firmware only where the owner accepted it (D3,
  D6), and elsewhere only within K4's per-task cap.**
  This is narrower than the first draft's "free for a tree that does not
  use it", which could not hold: the firmware paints through
  `&mut dyn Painter` (`Checksum`, `fixtures/embedded/src/lib.rs:247`),
  so every required `Painter` method is linked through the vtable
  whether a tree calls it or not. T27.1's one method and one `Style`
  field cost +676 B of `.text` on the paint-only `firmware`, before the
  four-binary gates existed (`docs/results-embedded.md`). So:
  - `path` and the widened `Cmd::Gradient` (M82) are accepted growth
    by D3, and the required `text_spaced` (M80) and `fills` (M82) by
    D6. Each task prints its
    measured `.text`/`.rodata` delta for all four firmware binaries
    against the 80 KB ceiling (D5, K4);
  - every other firmware-linked addition, the `caret_rect` and
    `outline` vtable slots included, is accepted by no decision: it is
    measured under K4's 1,000 B and reported to the owner with the
    number (second review R6);
  - `shader` is **behind a default-off core feature**, its `Cmd`
    variants included, and the firmware links none of it (K4, bound
    0 B);
  - features of the shell, text and window crates cannot reach the
    firmware at all (`fixtures/embedded` depends on `lumen-static` and
    `lumen-static-widgets` only), so they are judged by K2 and never by
    a firmware delta (review B7);
- **per-node state and style grow only where §2.1 assigns them (D4).**
  Widget structs, the embedded budget's third resident array, grow
  too (M80's `Label` and `MeasureKey`), and §2.1 holds that figure
  separately (second review B7). `Style` goes
  80 → 84 B (M82) and `NodeState` 84 → 88 B (M86's `pointer_mix`). Every task that grows
  either prints the new size and RAM per 100 nodes (16.0 KB today
  against 24 KB). `Cmd` (60 B) and `Event` (16 B) do not grow at all
  (K1);
- **driven end to end**: every capability with a user-visible behaviour
  gets a live drive under both `Everything` and `Boundaries<256>`, or
  its task says precisely why no live drive can observe it and names
  what does (§3.1). That is the method that found the last twenty
  defects (AGENT.md: "an end-to-end assembly is the only thing that can
  find a capability which is present, correct in isolation, and
  unreachable").

**What would falsify it:** a plan-wide kill rule in §4 firing. In
particular: a struct growing past §2.1's end-of-plan maximum (K1); a
default feature set that gains a crate or a symbol (K2); a frame arm
outside its spread with the feature unused (K3); or a firmware binary
crossing 80 KB (D5), or one task costing more than 1,000 B of
`firmware_interactive`'s `.text` (K4).

## 1. The roadmap, item by item

Status letters: **F** finished, **P** in progress, **—** pending,
**X** canceled (iced's own board). "Here" is this tree at `553fa5e`.

### 1.1 Already here, and not re-planned

| iced item | iced | here |
|---|---|---|
| Software renderer; incremental rendering for it | F | `Canvas`, `render_within(list, damage)` |
| Rendering parallelization | P | banded raster (M17, `docs/results-threading.md`) |
| Damage tracking / incremental presentation | P | region-set damage (M18), physical damage (M40), GPU `LoadOp::Load` |
| Reactive rendering | F | `ControlFlow::Wait`, `Boundaries<256>` default (M34) |
| Gaussian blur | P | `Glass` (M19, M22) |
| Shadows for quads | F | `Cmd::Shadow`, `Style.shadow` |
| Quad pixel snapping | F | `Scaled::snapped` (M40/T40.3, M57/A15) |
| Off-screen rendering / screenshots | F | headless `Canvas`, `ui.screenshot`, `LUMEN_SHOT` |
| Headless mode | F | `Program::host()` |
| Explicit text caching | F | `Measured`, `FoldedText`, `MeasureKey::new` (M45/T45.2) |
| Text shaping and font fallback | F | fallback faces (M45/T45.3); `lumen-static-text` shapes for measuring and editing; shaped *paint* is M81 |
| Dynamically-sized glyph cache | F | GPU atlas grows (M51/T51.1); software `GlyphCache` (M50/T50.2) |
| Cursor availability | F | `Style.cursor`, 8 shapes |
| Nested overlays; overlays for tooltips | F | `overlay.rs` `MAX_ROUNDS = 4`; `Tooltip` |
| Theming reloaded | F | `Palette`, `Role`, `Host::set_palette` (M25) |
| `Application` API | F | `application(boot)` (M32) |
| `Task` API / chaining | F | `Job`/`Descriptor`/`Subs`/`Timers` (M28) |
| Animation API | F | `Animated<T>`, `Easing` (built-in widget transitions are M86) |
| Hot reloading | F | M37, in a live window |
| Input method editors | F | preedit and commit (`docs/plan-desktop.md` T14.8); the focused subtree re-marked on compose (M41/T41.1); candidate placement is M80 |
| Centralized focus; keyboard navigation | P | Tab order, groups, modal trap (M44) |
| Accessibility / AccessKit | P | M54: the adapter, roles, actions; `a11y` default-off by its kill rule |
| Stack, Markdown, rich text, table, `ComboBox`, `Modal`, text editor, `List` | mixed | `#[widget(stack)]`, `Markdown`, `RichText`, `DataGrid`, `Combobox`, `Modal`, `TextArea`, `Repeat` |
| Image support for `Canvas` | F | `PaintBox` + `Painter::image` |
| Benchmarking & profiling | P | `benches/`, `benches-competitive/`, `app.perf` |
| Custom renderer injection (the trait half) | F | `Painter`/`Renderer` public; `Svg` (`svg.rs`) is a third `Renderer`. The window half is M83 |
| Testing library; test recorder and player | F | `TestApp` (M12); the agent's `session::Recording` (`replay`, `export_test`), not `retain::Recording` |
| Official guidebook | P | `book/` (M20) |

### 1.2 Planned here

| iced item | iced | here today | milestone |
|---|---|---|---|
| Configurable line height | F | missing: no `Style` field | **M80** |
| Ellipsis | F | one widget: `AppBar::fit_title`, which cuts on `char` boundaries, not graphemes (`nav.rs`) | **M80** |
| Text selection (read-only text) | — | inputs only | **M80** |
| IME candidate window placement | (part of IME, F) | `set_ime_cursor_area` is never called | **M80** |
| RTL text (painted) | — | measured and edited, not painted | **M81** |
| Shaping strategy selection | F | no seam on `Program`; `lumen-static-text` depends on the shell, so the shell cannot name its measurer (D1a) | **M81** |
| Subpixel glyph positioning | F | missing by design: `GlyphId` has no phase, and nothing rasterises at a fractional pen | **M81** (M81b, T81.7–T81.8: fontdue's public `rasterize_indexed_subpixel`, with `ab_glyph` as K81d's fallback; no M82 dependency) |
| Background gradients (radial, in `Style`) | F | linear `Cmd` only; `Style.background` is a `Color`; `Gradient`'s 56 B payload already sets `Cmd`'s 60 | **M82** |
| Vectorial text; paths | F | no path or transform command | **M82** |
| Custom shaders | F | missing | **M83** (feature `shader`, D3) |
| Custom renderer injection (window) | F | `pick_backend` is private, and `Backend` lives in the winit crate, which `Program`'s crate cannot name (D1b) | **M83** |
| Animated images | — | no decoding at all (T54.9); `Cmd::Image`'s hash folds rect, id and tint only, so new pixels under one id damage nothing | **M84** |
| `Video` widget | — | missing | **M84** (widget + the `VideoFeed` producer; decoder crate on K84b) |
| `pin` widget | F | missing | **M85** |
| `pop` widget | F | missing | **M85** |
| `Flexible` variant for `Length` | F | missing (`FillPortion` exists) | **M85** (T85.1; K85a held, `docs/results-layout-input.md` "M85a closes") |
| Primitive culling (containers) | — | renderers cull; `List` culls on `clip_bounds()` (T16.12, `docs/results-competitive.md`); derived containers and `ScrollView` do not, and `clip_bounds` is the surface, not the clip | **M85b**: **decided against under K85c on 2026-10-06** — retention's re-sync cost +4.3% to +11.6% on `--bin retain` with no shift (`docs/results-layout-input.md` T85.7, "M85b closes"); so T85.8 (`CullTo`) is not shipped and the `CullTo` cases §2.2 names do not arise; reopens on a scope mechanism free when no frame shifts |
| Smart event propagation | — | bubble only, no capture phase | **M85/T85.5 survey only**: **decided against under K85e on 2026-10-06** — one real case (`PullToRefresh`), two required (`docs/results-layout-input.md` T85.5); reopens on a second real case (§6) |
| Widget animations | P | only `Glass`'s press | **M86** |
| Customizable Markdown rendering and images | F | fixed subset, no images, no links | **M87** |
| Incremental Markdown parsing | F | whole document re-parsed on `set_str` | **M87** (probe first) |
| `Sipper` abstraction | F | deferred (`docs/plan-tasks.md` §4) | **M88** |
| Screenshot as a task | F | agent and `LUMEN_SHOT` only; a `Job` runs on a worker and nothing lets the window answer it | **M88** (as a `Request`, §2.8) |
| Rich clipboard | F | text only, through `Session::copy`/`paste`; the application cannot reach the clipboard | **M88** (as a `Request`) |
| Multi-window | F | `Pane`/`Panes` (`windows.rs`) hold a loader-side `Host<A>` and cannot be a hot window's state; M89 deletes them | **M89** |
| `Daemon` API | F | missing; `KeyOutcome::Quit` and `Session::wants_quit` end the whole loop | **M89** |
| Context menus | — | `Menu` exists; a right *release* activates (pass 2 matches any button), no widget `on_event` arm checks the button, and the agent can only click Left | **M90** |
| System menus | P | missing; native on macOS and Windows, framework-drawn on Linux (D2) | **M90** |
| Tray icon | — | missing; on Linux it is `ksni` over D-Bus, with no GTK thread (D2) | **M90** |

### 1.3 Not planned, with the reason

| iced item | reason |
|---|---|
| Font hinting | excluded at the owner's request; `docs/text-render-adoption-decision.md` already declines it |
| Time-travel debugging | excluded at the owner's request |
| The Comet debugger | excluded at the owner's request (the agent's `app.perf`/`app.logs`/journal is the substitute) |
| Test *emulator* | the headless `Host` is the emulator; a separate one would be a second construction path, which M31/M32 exist to prevent |
| Primitive layering (arbitrary z-order) | **rejected on the record**: `overlay.rs` and `docs/plan-capabilities.md` — paint order *is* tree order, and retention, the journal and clip balance rely on it. The overlay phase stays the one extra layer |
| Community OpenGL renderer; `vello`; `femtovg`; `glazier` | iced-specific renderer choices (two of them canceled by iced). M83's backend seam is what makes a third-party renderer possible here |
| Texture storage for gradient data; incremental rendering for `iced_wgpu` | iced-internal; this GPU backend already damages incrementally, and M82 keeps gradients in the storage buffer |
| Arena allocator exploration | the tree is a compile-time table; `arena.rs` already serves the off-table constructs |
| `winit` fork / upstream; `wgpu`, `softbuffer` updates; `tiny-skia` clipping; presentation optimisation | iced's dependencies, not features |
| The helper crates (`anywho`, `function`, `decoder`, `iced_palace`), legacy web colours, core consolidation, pocket guide | iced's own codebase |
| Remaining a11y work (incremental `TreeUpdate`, text `SetValue`, iOS) | owned by `docs/plan-a11y-platform.md` §4, with its own reopen conditions |
| A native menu bar on Linux | D2: `muda` attaches only to a GTK window, and a winit window is not one; the framework draws the bar there |

## 2. Where things stand: the budgets and seams every milestone shares

Every figure below was read from `553fa5e`. Each milestone section
cites this section rather than restating it.

### 2.1 Byte inventory

**`Style`: 80 B, 78 used, 2 spare** (`crates/lumen-static/src/style.rs`).

| fields | bytes |
|---|---:|
| `width`, `height` (`Length`) | 16 |
| `padding` (`Edges`) | 16 |
| `spacing`, `radius`, `font_size`, `font_weight`, `shadow`, `pressed_dim`, `hover_dim` (`f32`) | 28 |
| `color`, `background`, `border_color` (`Color`) | 12 |
| `align`, `focus_ring`, `cursor`, `border_width` (`u8`) | 4 |
| `visible`, `disabled` (`bool`) | 2 |
| **used / size** | **78 / 80** |

**`NodeState`: 84 B, 82 used, 2 spare** (`crates/lumen-static/src/desc.rs`).

| fields | bytes |
|---|---:|
| `constraints` | 16 |
| `size`, `pos` | 16 |
| `fg`, `bg`, `border_color` (`Color`) | 12 |
| `radius`, `font_size`, `font_weight`, `shadow` (`f32`) | 16 |
| `pad` | 16 |
| `pressed`, `hovered`, `focused`, `focus_ring`, `border_width`, `disabled` | 6 |
| **used / size** | **82 / 84** |

Paint has no `&[Style]`. It reads the cascade's answer from
`NodeState`, which `ops::record_painted` fills at layout. So **a style
field that paint must see costs a byte in both structs** (review A10,
C3).

**The plan's assignments.** A milestone that lands out of this order
takes the next free byte and prints the size either way:

| task | field | `Style` | `NodeState` |
|---|---|---|---|
| today | — | 78/80 | 82/84 |
| M80, line height | `line_height: u8` | 79/80 | 83/84 (paint copy) |
| M80, overflow | `overflow: TextOverflow`, the whole `u8` (`max_lines` is not built) | 80/80 | — (the cut is made in layout) |
| M82, style gradient | `background_fill: u8` | **81/84, re-budget** | 84/84 (paint copy) |
| M86, transitions | `transition: u8` (tens of ms; set by `Patch::transition_ms`) | 82/84 | **86/88**: `pointer_mix: u16`, the level `shaded` reads |
| **end-of-plan maximum** | | **84** | **88** |

**RAM per 100 nodes** is `PER_NODE_RAM` (`NodeState` + `Style`,
`fixtures/embedded/src/lib.rs`): **164 B/node, 16.0 KB today; 168 B,
16.4 KB after M82; 172 B, 16.8 KB at most**, against a **24 KB**
ceiling (`docs/results-embedded.md`). A task that moves either size
edits every assertion of it in the same commit (second review C9,
B14), and every milestone that moves either size cites this list:
- `crates/lumen-static/tests/cursor.rs:32` (`Style == 80`) and `:33`
  (`NodeState == 84`);
- `crates/lumen-static/tests/focus.rs:203` (`Style == 80`) and `:204`
  (`NodeState == 84`);
- `fixtures/embedded/src/lib.rs:409`/`:415`/`:416` (`NODE_STATE == 84`,
  `STYLE == 80`, `PER_NODE_RAM == 164`).

It also re-budgets `docs/results-embedded.md` with the attribution.
(`benches/src/bin/appearance.rs:220-221` prints both sizes and
asserts neither.)

**Widget structs, the third resident array** (second review B7). The
embedded budget is state + style + widget structs
(`docs/results-embedded.md`), and `PER_NODE_RAM` counts only the
first two. The third is held by `assert!(UI == 1192)`
(`fixtures/embedded/src/lib.rs:449`, 32-bit target only). M80 moves
it: `Label` gains 8 B for `fit`, and `MeasureKey` goes 32 → 36 B,
4 B on each of the eleven-node tree's four memo-holding widgets
(`plan-text-fit.md`). T80.2 and T80.3 re-budget the assertion
attributed, and **K1 requires every task that moves `Ui` to print
old and new `Ui` sizes** beside `Style` and `NodeState`.

**`Cmd`: 60 B, `Copy`; `Gradient`'s 56 B payload sets it**
(`crates/lumen-static/tests/glass_cmd.rs:15`, which prints each other
payload against the 56). `Cmd` is not per node: it is per display-list
command. **This plan does not grow it** (K1). A new variant's payload
is printed against 56 B the way `Glass`'s is. What does not fit goes
through a side store under §2.9. The widened gradient has **zero
headroom** today (inline `Stops`, `MAX_STOPS = 4`), so M82 chooses a
layout that fits (for example `u8` stop offsets, or stops out of line
under §2.9) and prints the arithmetic.

**`Event`: 16 B** (`crates/lumen-static/tests/gesture.rs:21`,
`crates/lumen-static/tests/modifiers.rs:101`). It is per `InputQueue`
slot (`CAP * size_of::<Event>()`), not per node. New variants (M85's
visibility, M90's context request and `Event::Menu { id: u32 }`, whose
16 B fit M90/T90.4 asserts) fit in 16 B or the task stops (K1).

**`GlyphId`: 8 B** (`u32` px bits + `u16` index + `u8` face, padded;
`crates/lumen-static-shell/src/text.rs`). It is a shell type with a
private constructor, which is why the core cannot hold one (§2.7c).

**Firmware** (`docs/results-embedded.md`, re-measured at the M56 merge,
2026-09-29). Every binary is gated by `scripts/check_text.sh` at
**81,920 B** from T80.0 (D5, owner, 2026-09-30); it was 65,536 B, and
`firmware_interactive` had 3,614 B left under that:

| binary | `.text` | of 80 KB | headroom |
|---|---:|---:|---:|
| `firmware` (headline) | 28,616 | 34.9% | 53,304 |
| `firmware_interactive` | **61,922** | **75.6%** | **19,998** |
| `firmware_bind` | 32,008 | 39.1% | 49,912 |
| `firmware_cascade` | 30,644 | 37.4% | 51,276 |

`check_text.sh` compares the headline with a **1% tolerance**, so a
few hundred leaked bytes pass it, and `just firmware-size` prints only
the paint-only `firmware` (`justfile`, `scripts/lib/firmware.sh`).
Neither is an "unchanged" comparator (review G32; second review B4);
K4 defines one. **The binding constraint is
`firmware_interactive`'s 19,998 B.** It links dispatch, so it pays for
M90's button filter, M85's layout and dispatch additions (`Flexible`,
`Pop`'s visibility delivery) and every new `Event` arm, as well as the
accepted D3 and D6 methods. It does **not** pay for a capture phase (M85
builds none; T85.5 is a survey) or for M86's blend, which sits behind
the core `transitions` feature that the firmware does not enable, with
its entries started in the shell's `pump_with`.

**The programme tally (firmware).** The bounds the plans state on
`firmware_interactive`, task by task (second review B6; the first
draft's "about 7,700 B" had no table and undercounted):

| task | bound (B) | source |
|---|---:|---|
| M80a, T80.0–T80.3 cumulative | 1,536 | K80b |
| T80.5, the `caret_rect` slot | 1,000 | K4 |
| T81.2, `Cluster::rtl` | 1,000 | K4 as upper bound; T81.2 bounds it at 64 B per binary |
| T81.5, `TextInput` over bidi runs | 1,000 | K4 |
| T82.2, radial and rounded gradients | 400 | T82.2 |
| T82.3, `background_fill` | 1,000 | T82.3 |
| T82.5, `Painter::path` | 1,000 | T82.5, K82b |
| T82.7, the `outline` slot on `BakedFace` | 1,000 | K4 |
| M85, T85.1–T85.4 cumulative | 1,024 | K85d |
| T86.1, the level in `NodeState` and `Patch::apply_to`'s branch | 96 | K86a |
| T90.2–T90.4, 3 × 256 | 768 | K90a |
| **sum** | **9,824** | |

Bounded at **0 B** and not summed: T81.1 (D1a/c), every M83 task
(K83b), T84.2 (no firmware paints an image), M87, and M88/M89, whose
`Request` sits behind `std` (an `nm` count under K4). T82.1 is
printed and expected at about 0. The sum fits the **19,998 B** of
headroom under 80 KB with **10,174 B** to spare. A middle estimate of
what will actually be linked, about **3,450 B** (range 2,300–5,000),
prices each required method at T27.1's 676 B, which is a paint-only
`firmware` figure for a method plus a `Style` field
(`docs/results-embedded.md`), so it is an order of magnitude for
`firmware_interactive`, not a measurement of it. Against the old
64 KB ceiling's 3,614 B the bounds did not fit and M82 (K82b) was
the first expected to trip; under 80 KB K82b's 1,024 B floor is a
check that is not expected to fire, and its per-task half is the one
that can. **The ceiling decision is DECIDED 2026-09-30
(owner): 80 KB (D5).** What can still stop a task is K4's per-task
cap of 1,000 B, which the owner kept.

### 2.2 `Painter`: implementors and consumers

`Painter` (`crates/lumen-static/src/display.rs:805`) has seven required
methods (`rect`, `text_weighted`, `rounded_rect`, `shadow`,
`stroked_rect`, `push_clip`, `pop_clip`) and provided `line`,
`gradient`, `image`, `glass`, `text`, `clip_bounds`, `defer`. Every
`impl Painter` in `crates/`, `fixtures/`, `examples/` and `benches/`,
found by grep on `553fa5e`:

| implementor | where | kind |
|---|---|---|
| `DisplayList` | `lumen-static/src/display.rs:1358` | leaf collector |
| `Scaled` | `lumen-static/src/display.rs:1220` | **wrapper** |
| `Deferring` | `lumen-static/src/overlay.rs:190` | **wrapper** |
| `retain::Recording` | `lumen-static/src/retain.rs:923` | **wrapper** (streams `(hash, ink)`, holds no list) |
| `CullTo` (M85/T85.8) | `lumen-static/src/display.rs` | **wrapper** (narrows `clip_bounds` to the viewport) |
| `Checksum` | `fixtures/embedded/src/lib.rs:247` | leaf, the firmware's |
| `Sink` | `benches/src/bin/retain.rs:215` | leaf |
| `Plain`, `Null`, `Echo`, `Counting`, `Flat`, `Surfaced` | `lumen-static/tests/{glass_cmd,retain_align,painter,container_fill,vector,overlay}.rs` | test leaves |

**Not implementors**, and the first draft's list confused them (review
C24): `Svg` (`svg.rs:113`), `Canvas` (`software.rs:1842`) and `Gpu`
(`lumen-static-wgpu`) are `Renderer`s, and consume `Cmd`. `wire.rs`,
`contrast.rs`, `Cmd::hash_into`, `DisplayList::hash_from` and
`benches/src/bin/retain.rs` match on `Cmd` too. The hatches
(`hatch.rs`) and clip helpers call a painter; none implements one.
`examples/` has no implementor.

**Rules for this plan:**

- **A new `Painter` method is required**: `text_spaced` (M80, with
  `text_weighted` becoming a provided method that forwards to it; a
  wrapper overrides `text_spaced` and does not override
  `text_weighted` (T80.1 deletes the existing overrides on `Scaled`,
  `Deferring` and `Recording`, moving `Scaled`'s origin rounding into
  `text_spaced`), because the provided body calls the required method
  on the same painter, M80), `path` and `fills` (M82). No `glyphs`
  or transform-stack method is built (header, D3). The compiler then
  gates its
  *existence* in every implementor above. The gate for *forwarding it
  correctly* is `deferring_forwards_every_command`
  (`crates/lumen-static/tests/overlay.rs:435`). Each task extends it
  with the new call, updates its asserted count (5 today, 6 after T80.1's
  `text_spaced`) and prints
  it. For a required method, "`Deferring` inherits a default" does not
  compile (review A16). The injection is **`Deferring`'s body for the
  new method does nothing**, which compiles and turns the test red by
  name. `Scaled` and `retain::Recording` get the same shape of check in
  the same task, and so does `CullTo` once M85b has landed (each of
  `text_spaced`, `path` and `fills` gets a `CullTo` case).
- **`Painter::shader`/`shader_end` are the one exception: provided,
  behind `feature = "shader"`**, with a documented leaf default
  (`push_clip(rect)`/`pop_clip()`, which draws the fallback). M83 owns
  the mechanics (`docs/plan-gpu-extension.md`). A required method
  behind a feature cannot work here: `fixtures/embedded` and `benches`
  are workspace members, so a unified build that turns
  `lumen-static/shader` on would make every leaf in a crate that does
  not forward the feature fail to compile. A provided method is right
  for a leaf and wrong for a wrapper, and the four wrappers
  (`Scaled`, `Deferring`, `Recording` and, once M85b lands, `CullTo`)
  live in `lumen-static`, so they all override both under the same
  `cfg`. M82's K82b fallback, if the owner ever moves paths out of the
  firmware, uses the same shape under `cfg(feature = "alloc")`:
  provided `path`/`fills` with leaf defaults, overridden by the
  wrappers; `Cmd::Path` needs no `links` cfg because every crate
  outside the core that matches `Cmd` builds with `std`
  (`docs/plan-paint-vocabulary.md`, K82b). **`Cmd::Shader`/`Cmd::ShaderEnd`
  are gated too, at every use site** (D3, owner, 2026-09-30):
  - in `lumen-static` every arm or method that names them carries
    `#[cfg(feature = "shader")]` (`display.rs`: the enum, `DisplayList`,
    `Scaled`, `CullTo`, `hash_into`, `hash_from`; `wire.rs`; `retain.rs`'s
    `Recording`; `overlay.rs`'s `Deferring`; the core's tests);
  - a crate outside the core cannot test the core's feature: its own
    `cfg(feature = "shader")` tests *its* feature, so under unification
    it would drop its arm while the variant exists and fail E0004. So
    `lumen-static` gains `links = "lumen_static"` and a `build.rs` that
    prints `cargo:shader=1` when `CARGO_FEATURE_SHADER` is set, and
    each crate that matches `Cmd` reads `DEP_LUMEN_STATIC_SHADER` in
    its own `build.rs` and emits `cargo:rustc-cfg=lumen_shader`. Its
    arms are `#[cfg(lumen_shader)] Cmd::Shader { .. } => …`. M83 lists
    the crates and sites (`lumen-static-shell`, `lumen-static-wgpu`,
    `benches`);
  - the gate is two workspace builds, feature off and
    `--features lumen-static/shader`, so exhaustiveness is
    compiler-checked both ways (M83/T83.3). The firmware links none of
    it by construction (K83b, 0 B).

  The forwarding gate is `deferring_forwards_the_bracket`
  (`crates/lumen-static/tests/shader_cmd.rs`), whose injection deletes
  `Deferring`'s override. `dev-shape` gains the core's and the shell's
  `--features shader` test lines, so `--workspace` unification does not
  decide whether they run.
- **`crates/lumen-static-shell/tests/svg.rs`'s exhaustive `match` over
  `Cmd` names every new kind.**
- **`Deferring` repaints deferred subtrees outside every clip scope**
  (`overlay.rs`, module doc). That does not touch transforms: M82
  builds no transform stack, and each path's `xf` travels with its
  `Cmd::Path`, so a path deferred anywhere keeps its transform in
  phase 2 with nothing re-pushed.

### 2.3 `Session` and the ABI stamp

- **Every `Session` method is required.** All 29 (30 since M80/T80.5's
  `caret_rect`, 31 since M81/T81.4's `shaping`) at
  `crates/lumen-static-shell/src/session.rs:148` end in `;`, and
  `impl Session for Hot` (`crates/lumen-static-reload/src/lib.rs:833`)
  writes each one out. **A missing forward is a compile error (E0046),
  so the compiler is the Hot-forward gate.** Every new method in this
  plan stays required. No task writes a "drop the Hot forward"
  injection or a per-method stamp-change test (reviews C4, G1, A15, M8,
  T9). A runtime gate on the seam uses an injection that compiles, for
  example `Hot` forwarding to the wrong argument or returning `None`,
  and it is observed through `just hot-window-drive` after a real
  swap.
- **The stamp is not a method list.** `abi_fields()`
  (`session.rs:1123`) holds the shell version, the shell's feature list
  and `size_of::<Box<dyn Session>>()`. The vtable is covered by the
  source hash in `crates/lumen-static-reload/build.rs`, which hashes
  `src/` and the manifests of `lumen-static`, `lumen-static-shell` and
  the reload crate. Any edit changes it, so a per-method stamp test
  cannot tell one change from another.
- **The feature list is load-bearing** (M37: sizes alone missed an
  `observe` build). **Every shell feature this plan adds joins
  `abi_fields`' list in the task that adds it**, starting with
  `shaping` (§2.7a) and `shader`. `shader` changes `Cmd`'s variant
  set, and the core's feature can be on while the shell's is off, so
  shader has **two entries**: `lumen_shader` reads `cfg!(lumen_shader)`
  (the core's variant set), and `shader` reads the shell's own
  `cfg!(feature = "shader")` (`ShaderTable`'s layout, which crosses
  `Session::shaders` by value). Both are added in M83/T83.3, and `just
  abi-stamp` tests both pairs (second review A7). The list's gate is `just abi-stamp`: a
  library built with the feature is refused by a loader built without
  it (review A25). **Two recorded exceptions**, each argued in its
  milestone: M82's `vector-text` (no `FontStack` crosses the seam; the
  library and the loader each build their own, and what does cross is
  source-hashed, M49), and M84's `image` (the store crosses only as
  `&dyn ImageSource`, a core trait under the source hash, so an entry
  would refuse a swap that is safe; T84.6 checks the accept
  direction).
- `lumen-static-text` joins `build.rs`'s hash list only if one of its
  types crosses the seam. In this design none does, because the
  measurer lives in the library's `Local`.

### 2.4 `.lss` (M61, shipped)

M61 is on `553fa5e` (`just ci-verify 0793dd5`, `docs/results-lss.md`).
So **a new `Patch` setter lands with its `.lss` declaration in the same
commit**, or `scripts/check_lss_setters.sh` fails `just static`. That
script checks both directions and also mirrors `Align` and `Cursor`, so
M80's new `TextOverflow` enum is mirrored there too. `docs/plan-lss.md` §2.1
allows **one declaration per setter, taking the setter's own value**.
`background_fill` therefore gets its own name (`background-fill`),
never an overload of `background: <color>` (review M24). The first
draft's conditional "if M61 has landed" wording is gone.

`check_lss_setters.sh` compares only `Patch` setters with the
`PROPERTIES` table. Two lowering sites have one explicit arm per
pairing and are not checked by it (second review M7):
- the runtime lowering, `set` in `crates/lumen-static-shell/src/lss.rs`,
  one arm per (setter, `Val`), ending in `panic!("… no setter …")`.
  A sheet loaded through `lss::load` (hot reload, `showcase.lss`) that
  uses a setter with no arm panics;
- the macro lowering, `val` in `crates/lumen-static-macros/src/lss.rs`,
  one arm per `Val` variant.

So a new setter lands, in one commit, with its `PROPERTIES` row, its
arm in the shell's `set`, and, for a new value kind, the `Val`/`Takes`
variant in `crates/lumen-static-lss/src/lower.rs` plus the macro's
`val` arm, with a sheet in `crates/lumen-static-shell/tests/lss_parity.rs`
that uses it (so the runtime arm is exercised, not only declared).
The sites are named in T80.2 (`line_height`), T80.3 (`overflow`, a
new keyword kind), T82.3 (`background_fill`) and T86.1
(`transition_ms`, a new `Takes::Millis`); M85 already names them for
`Length`. **Injection** for each: delete the new `set` arm, and the
`lss_parity.rs` sheet panics by the setter's name.

### 2.5 Widgets

Every task that exports a widget meets §3.2's list in its own commit.

### 2.6 Frame arms

K3 names the arms. Nothing else in this plan invents one for K3; a
milestone-local row a kill rule needs (K87c's `cascade_deep (markdown
x16)`, T87.1) is added by its own task and is not a K3 arm.

### 2.7 The crate moves of D1

| move | what | performed in |
|---|---|---|
| **a** | `lumen-static-text`'s `[dependencies]` entry on `lumen-static-shell` becomes `[dev-dependencies]` (with `features = ["truetype"]`). Only `tests/paint_shaped.rs` uses it (2 lines). The shell then depends on `lumen-static-text` behind a **default-off `shaping` feature**, which joins `abi_fields`. The crate count and stripped size with and without it are recorded under K2, and the default window build links zero `rustybuzz` symbols. `deny.toml`'s `RUSTSEC-2026-0206` acceptance, which calls rustybuzz "lumen-static-text's shaper (T15.8), a default-off crate", is reworded in the same commit: rustybuzz is then reached through the shell's default-off `shaping` feature (second review X5) | **M81** (T81.1) |
| **b** | `Backend` (`lumen-static-winit/src/backend.rs:43`) and `Frame` (`surface.rs:25`) move into a new `lumen_static_shell::backend` module beside `Screen`, behind the shell's `truetype` (as `session` and `text` are; `raw-window-handle` is optional under it, and `Program::backend_with` is gated the same way), and `fetch_rgba`, `set_images` and `frame_rgba` become required in the move (M83/T83.1), with `BackendTarget` and `WindowHandles`. None of them names a winit type. `AsScreen`, `frame_times`, `Software`, `GpuBackend` and `pick_backend` stay in the winit crate (M83 owns the list). **`Backend` and `Screen` stay separate traits**: `Screen` has 9 implementors (`Offscreen` at `session.rs:109`, winit's `AsScreen` at `backend.rs:166`, whose receiver M88/T88.2 changes, and 7 test `Blind`s in `lumen-static-shell/tests/`; second review C10, M25) with no `resize`, `take_images` or `name`, and merging would give each four stubs. `Frame` is not merged into `FrameTimes`: `Frame` is the window's non-optional record (with `dropped`), `FrameTimes` the agent's optional view, and the conversion (`frame_times`) stays in the winit crate. `Software` stays there too, because it holds softbuffer and the window. The factory takes the window as `raw-window-handle` (`HasWindowHandle + HasDisplayHandle`, 0.6.2, already in `Cargo.lock`), plus `accept_cpu`, and returns `Option` so the software fallback still applies. It can be declared on `Program` now that it names no winit type. It is a boxed `FnMut` (M83/T83.2), **taken once by `run` into the window's `Config` at start, called once per window and per `resumed`** (a factory backend makes its own device; M89b/T89.10 shares the built-in backend's device through `Weak<GpuContext>`, not through the factory), **and never reloadable**: a swap re-runs `program()` but never re-reads `Config`, and a hot library's factory is ignored | **M83** (T83.1 the move, T83.2 the factory) |
| **c** | a core **`GlyphKey`** in `lumen-static`: `px_bits: u32`, `index: u16`, `face: u8`, `phase: u8` (M81's order), 8 B. The shell's `GlyphId` converts to and from it. M81's shaped run (`ShapedGlyph { key: GlyphKey, .. }`) carries it inside the shell's paint path and no core store holds one, so no widget can choose a face through it (M87's italic probe records that) | **M81** (T81.1) |
| **d** | programs and windows are named by **data**. A program is a `u16` index into a table the hot library exports, resolved by the current build on every swap: `export_program!`'s one boot symbol (`lumen_boot` today) gains the index. A window is a `u32` key. Neither is ever a `fn` pointer or a foreign type (rule 3, `docs/events-decision.md`) | **M89** (the `u32` window key and the program table; M88's `Request` carries no window key) |

### 2.8 Platform requests are data (S1, normative)

A `Job` is a `Copy` descriptor and `fn` pointers, and `Host::spawn`
hands every job to a `Spawner` that runs it on a worker
(`crates/lumen-static/src/spawn.rs`). Nothing lets a window answer one,
a handler cannot build one, and a clipboard image is not `Copy`. So
platform work is **not a `Job`** (reviews C14, A14, M12, P17). M88
builds the mechanism (T88.3 in its section, which is authoritative for
the details), and M89 and M90 extend it:

- **`Request`**, a `Copy` enum in a new `lumen-static/src/request.rs`
  behind `std`, as `Outbox` is, so the firmware links none of it. M88's
  variants are `Screenshot { tag }`, `ClipboardWrite { tag, kind }` and
  `ClipboardRead { tag, kind, format }`. **`tag` is a `u64`**, chosen
  by the application. Variants are append-only. M88 asserts the size at
  16 B. **M89 appends** `OpenWindow { program: u16, tag }`,
  `CloseWindow { window: u32 }`, `Close`, `Post` and `Quit`. `Post`
  carries a 24 B `Descriptor`, so M89's task re-asserts `Request` at
  32 B and prints it; `Requests` holds 8, so the cost is 32 × 8 =
  256 B plus the queue header per application, not per node. **M89 reserves `u64::MAX`** as the
  unsolicited tag (`Posted`, `WindowClosed`).
- **`Requests`**, a fixed-capacity queue (`CAPACITY = 8`) that refuses
  and counts, a copy of `Outbox`'s shape. It is reached through a
  provided `App::requests(&mut self) -> Option<&mut Requests>`, which
  mirrors `App::outbox` (`event.rs:1040`).
- **What the application writes** comes from a provided
  `App::clip_for(&mut self, tag) -> Option<Clip<'_>>`, asked
  synchronously while the host drains a `ClipboardWrite`. The bytes
  never live in a request.
- **`Session::take_requests(&mut self) -> Vec<Outgoing>`** (required).
  `Outgoing { request, payload: Payload }` and the owned `Payload` are
  shell types. `String` and `Vec<u8>` already cross the seam in `copy`
  and `save`.
- **`Session::answer(&mut self, tag: u64, a: Answer<'_>)`** (required).
  The host converts the answer with a provided
  **`App::answered(tag, Answer<'_>) -> Option<Self::Msg>`**, code in the
  *current* build, so no `fn` pointer is held across a swap (rule 3).
  `Host::answer` **applies** the message inside the call (M88, second
  review A1) — not on a queue for the next pump, which a swap landing
  before that pump would drop with a v1 `Msg` that `hot::save` cannot
  carry — and not into the inbox, because a refused post at the
  inbox's capacity would drop an answer the platform already acted
  on. A change it reports sets `Host::answer_changed`, which the next
  pump ORs into `applied_changed` (A2) and `more_deferred` reports.
- **Who answers.** `Win` drains in **three** places and answers
  before it returns: in `redraw` on every frame after the
  present-or-skip decision (a no-damage frame presents nothing,
  M53/T53.4), after the `failed()` return so it is skipped while the
  panic screen is up; after every
  agent call, and (M89) **after every windowless frame** of the
  daemon's key-0 session and after any post or answer routed into it.
  The windowless session presents nothing and is pumped from
  `new_events`/`Wake(0)` through `frame(dt, 1, 1)`, so without the
  third site its `OpenWindow`, `CloseWindow` and `Quit` never leave
  the queue (second review A9). A `Screenshot` drained from key 0 is
  answered `Refused(Unsupported)`. **Injection** (M89/T89.6): skip the
  windowless drain, and `app.windows` stays `[0]` after tick 10. An
  answer routed into another window's session asks that window for a
  redraw (A10).
  So no M88 request is pending at a swap, and M89 decides that no
  `OpenWindow` is either (`docs/plan-windows.md`, second review A19,
  P10): `Win` creates the window and answers `WindowOpened` (or
  `NoSuchProgram`) to the opener inside the same drain that took the
  request, and the swap step runs only at the start of a frame, never
  inside a drain. Gated in M89/T89.3 gate 2. The issuing window is the session that drained
  the request. `Quit` is the process; closing one window is `Close` or
  `CloseWindow` (review X5).
- **A screenshot answers from `frame_rgba`** (what the renderer drew).
  `fetch_rgba` stays "what the display server shows" and stays `None`
  on the GPU (review A19). M88/T88.2 changes `frame_rgba`'s receiver to
  `&mut self`, on `Backend` wherever M83 has left it.
- **Other `Session` methods this plan adds**, all required, so §2.3's
  compiler gate covers `Hot`: `caret_rect()` (M80), `shaping()` (M81),
  `shaders()` (M83), `tray_model()` and `menu_state()` (M90). M84 adds
  the required `FrameSource::images`; `FrameSource` is `Session`'s
  supertrait, so `Hot` forwards it under the same gate. `Backend` gains the required
  `set_font` (M81) and `set_shaders` (M83). One existing method changes
  signature: M90/T90.5 makes `Session::paste` answer `Option<usize>`
  (`None`: the focused node did not take it, so the key goes on), and
  `Hot` follows by compile error.

### 2.9 Per-frame side stores never fold an index (S2, normative)

A glyph run, a path, a shader's uniforms or out-of-line gradient data
lives in a buffer the display list owns for one frame. Any index into
that buffer shifts when an earlier node's entry changes. When
`Cmd::Text`'s hash folded its buffer offset, one byte of text reported
**61,056 px² of phantom damage** (M27/T27.3,
`crates/lumen-static/src/display.rs`, `Cmd::hash_into`). So, for every
store:

- the `Painter` method takes the **content by reference** (`&[..]`), so
  `retain::Recording`, which streams and holds no list, and `Checksum`
  can hash it;
- `Cmd::hash_into` folds the entry's **length, never its start**.
  `DisplayList::hash_from`, the only holder of the buffer, folds the
  **content**. A content-derived key in the command is equivalent;
- **the gate** is two entries in one list, modelled on
  `a_text_length_change_does_not_report_later_nodes`
  (`crates/lumen-static-agent/tests/damage_shape.rs:261`). When the
  first entry changes content and length, the second reports no damage,
  and the count is printed. **Injection:** fold the start index, and
  the second entry reports damage. **Accept direction:** change the
  second entry, and it reports damage at 1.00× its own box.

## 3. Test and doc discipline for this plan

These rules are normative and replace the first draft's header italics.
Each task is one commit and ends with `just test` green.

### 3.1 Gates (S3)

- **No gate is committed ignored first.** An `#[ignore]` test still
  compiles, so a gate that names an API which does not exist yet cannot
  be committed at all (review G27). **A milestone's T.0 commits only
  gates that compile today.** They pin the current gap **green**: each
  asserts today's behaviour and prints it, and the task that closes the
  gap flips the assertion. No gate is committed red. **A pin is still
  watched red before it is committed** (AGENT.md, "Before a new test
  or gate is committed, break the thing it protects and watch it go
  red"; second review M14): each T.0 pin names a temporary
  perturbation of the pinned mechanism, for example pass 2 ignoring a
  non-`Left` `PointerUp`, that turns it red at T.0, and the T.0
  commit's body records it. The flip's own red run, the old assertion
  against the new code, is a second red run, not the first. "No
  injection can be written against a pin" is false: a pin fails the
  moment the pinned behaviour changes. **Every gate for a new API
  lands in its own task's commit**, and that commit's body records the
  red run under the injection.
- **Every gate names an injection that breaks the mechanism**, not a
  value (AGENT.md, "Tests must be shown to fail"). "—" is not an
  injection (reviews G28, P30, M20). Each gate names its injection
  inline, beside the gate, which replaces the first draft's appendix;
  only M87 also tabulates them (second review R14).
- **Every new cargo feature is linted and documented with the feature
  on** (second review X7). The workspace `clippy` and `cargo doc`
  lines (`justfile`) build default features only, so a default-off
  feature's code would never be linted and its rustdoc links never
  resolved. The task that adds a feature adds, in its own commit, a
  `just` line `cargo clippy -p <crate> --features <f> --all-targets --
  -D warnings` (the precedent is `--features simd`,
  `threads,truetype`, `gpu`), and `RUSTDOCFLAGS="-D warnings" cargo doc
  -p <crate> --no-deps --features <f>` where the feature adds public
  items (precedent: `lumen-static --features a11y`). For `shader`, the
  clippy line is a workspace one with `--features lumen-static/shader`,
  so the `cfg(lumen_shader)` arms are linted. This covers `shaping`
  (T81.1), `vector-text` (T82.7), `shader` (M83), `image` (T84.1),
  `transitions` (M86), `clipboard-image` (T88.5) and `tray` (T90.7).
  **Injection:** add an unused binding inside the feature's code; the
  new line fails and the workspace line stays green.
- **Every gate asserts that its setup landed**: `handled`, a count, or
  a hit on the intended node. It observes through a channel the
  harness can actually read. Examples: a drive cannot send Ctrl+C to
  `Win::clipboard_shortcut` through `input.key` (review G5), and it
  cannot right-click through `input.click` until that takes a button
  (review G25). A new agent method is added to the set
  `scripts/check_agent_methods.sh` compares, in the same commit.
- **Every gate prints its measured quantity beside its bound**, and is
  checked in the accept direction in the same session.
- **Live drives run under both `Everything` and `Boundaries<256>`.**
  This applies to every capability with a user-visible behaviour, and
  uses the real recipes: `just widget-drive <name>`,
  `just agent-drive`, `just tasks-drive`, `just hot-window-drive`,
  `just gpu-parity`, `just resume-drive`, `just panic-drive`. Otherwise
  the task states why no live drive can observe the behaviour and what
  does. A drive that cannot run reports *skipped*, and a milestone with
  a skipped gate is not done (M43's R17-F6 rule). A tool listed in §3.4
  is a prerequisite, not a capability: its absence **fails** the gate
  by name with the install instruction, and never reports *skipped*
  (D7).

### 3.2 Doc currency and widget obligations (S4)

- **There is no trailing "docs" task** (review M3). Each task's commit
  carries its own doc updates:
  - the handoff's OPEN/DECIDED marker (`docs/00-handoff-static-tree.md`);
  - the results-doc row that owns its number;
  - the rustdoc on the code it changed;
  - the book chapter, included by named anchor;
  - the deletion, from `scripts/doc-identifiers.allow` and
    `scripts/doc-paths.allow`, of every pre-listed entry the task
    builds (second review M6: `scripts/check_doc_identifiers.sh` fails
    on a built name left there, and M83 alone builds eleven non-widget
    names, `BackendTarget` to `ShaderView`).
- **A task that exports a widget does all of this in the same commit:**
  - adds its drive inventory under `scripts/drive/widgets/`, or a
    `scripts/drive/undriven.allow` line with an owner and a reason
    (`scripts/check_drive_coverage.sh`);
  - implements `observe`, or adds the reason to
    `scripts/check_observe.sh`'s list;
  - gives each new **drive inventory name** a row in
    `scripts/drive/roles.py` (M54/T54.3; `scripts/drive/run.py` refuses
    an inventory with no row and a row with no inventory);
  - gives each new **`impl Node for`** in `lumen-static-widgets/src` a
    `ROWS` entry and a catalogue field in
    `crates/lumen-static-widgets/tests/a11y_roles.rs`
    (`every_node_impl_has_a_row` scans `src/` and fails by name on a
    missing row; injection: delete the row). This is a different table
    from `roles.py`, keyed by type, not by inventory (second review M3).
    A feature-gated widget (M83's `ShaderView`) needs an allow line or
    a `--features a11y,<feature>` run in `just test`, because the scan
    reads source text (second review A15);
  - adds every new inventory and action to
    `scripts/drive/expected.json`, with its `counts` (`run.py` fails
    on an action or entry the manifest does not name, and on a count
    that differs);
  - adds a `scripts/drive/keys.py` entry for every demo with a
    focusable node (`keys.py`: a demo with a focusable node and no
    entry fails the run);
  - adds a showcase entry (`examples/showcase`);
  The tasks that export or change a widget name each missing item
  themselves: T80.2 (`Label` inventory: `roles.py`), T80.4
  (`Selectable`: `roles.py`, `keys.py`), T81.6 (`#d_textinput_rtl`:
  `keys.py`, `expected.json`), T82.8 (`VectorText`: `a11y_roles.rs`),
  T84.3 (`Image` inventory: `roles.py`), T84.8 (`Video`: `roles.py`,
  `a11y_roles.rs`), T85.1 (`Layout` inventory: `roles.py`), T85.2 and
  T85.4 (`Pin`, `Pop`: `a11y_roles.rs`), T90.4 (`ContextMenu`: all
  four; `CommandList`: `a11y_roles.rs` only, driven as `ContextMenu`
  under `undriven.allow`), T90.5 (`MenuBar`: `a11y_roles.rs` `ROWS`,
  `roles.py`, `expected.json`, `keys.py`).
- **A new `Patch` setter lands with its `.lss` declaration, its
  runtime `set` arm and, for a new value kind, its `Val` arms**
  (§2.4).
- T80.0 commits all twelve plan files together, both pre-seeded
  allowlist blocks (`doc-identifiers.allow`; `doc-paths.allow`'s
  M80–M90 block of eight results docs and `bin vocabulary`/
  `scroll_cull`/`markdown`, each entry removed by the task that
  creates its target), and an AGENT.md queue entry in §5's order
  (review M28), and
  raises the firmware ceiling to 81,920 B (D5).

### 3.3 Closing (S5)

A milestone closes only with **`just ci-verify <sha>` green, cited in
AGENT.md's queue entry.** The results doc records every kill rule's
number, whether or not it fired.

### 3.4 Reference-machine prerequisites (D7)

The reference machine's tools are recorded by the capability probe,
`scripts/probe.sh`, which `scripts/ci.sh` runs first (`probe.sh thumb
qemu miri`) and whose findings it writes into `target/ci/<sha>.json`.
Its capabilities are optional: an absent one exits 77, a skip, which
`just ci` counts as a failure unless `LUMEN_ALLOW_SKIP=1`. A
**prerequisite** is a tool the plan has decided the machine has; its
absence is a failure, never a skip.

- **`xclip`** (D7, owner, 2026-10-02), for M88/T88.4's independent
  read of the primary selection (`xclip -o -selection primary`).
  Absent on 2026-10-02. The step, done once before T88.4:
  1. install it on the reference machine: `sudo apt install xclip`;
  2. T88.4's commit adds an `xclip` case to `scripts/probe.sh` that
     prints `probe: xclip present (<path>, <version>)` or `probe:
     xclip absent -- required (D7): sudo apt install xclip` and exits
     **1, not 77**, the probe's first required case; it adds `xclip`
     to `scripts/ci.sh`'s probe line, so every `just ci` record names
     it;
  3. `just tasks-drive`'s primary leg calls `scripts/probe.sh xclip`
     first and propagates its status, so a missing `xclip` fails the
     drive with the install line and never prints *skipped*.

  **Both directions in one session** (AGENT.md rule 3): with `xclip`
  removed from `PATH`, the leg exits non-zero and non-77 and prints
  the install line; with it present, the leg reads `B`.
- The tools the earlier plans already rely on (`cargo-deny`,
  `qemu-system-arm`, the thumb target; `docs/plan-gate-hygiene.md`)
  stay probe capabilities and are not re-classified here.

## 4. Kill rules

Written before anything is built. A kill rule that fires stops its task.
The number goes in the milestone's "What building found" section and in
the results doc that owns it.

- **K1 — the struct budget.**
  - `size_of::<Cmd>() == 60` and `size_of::<Event>() == 16` exactly.
    Neither grows in this plan. A variant that does not fit stops, and
    its payload moves to a side store under §2.9.
  - `size_of::<Style>() ≤ 84` and `size_of::<NodeState>() ≤ 88`, which
    is `PER_NODE_RAM ≤ 172 B` (16.8 KB per 100 nodes against 24 KB).
  - A task that would exceed either maximum stops. It packs its field
    into existing bytes, or into a `u8` index into a per-sheet table.
    The owner's re-budget is the only other way out.
  - Every task that moves a size prints old and new sizes and the RAM
    per 100 nodes, and every task that moves the embedded `Ui` (the
    widget-struct array, §2.1) prints its old and new size beside
    them (second review B7).
  - Unlike the first draft's "above 24 KB", this can fire. That bound
    had 80 B/node of headroom against a plan that adds at most 8
    (review B8).
- **K2 — default-off stays default-off.**
  - The default `lumen-static-winit` crate count, measured as
    `docs/results-a11y-platform.md` measures it (`cargo tree -e
    normal`, deduplicated; 95 on 2026-09-27, 96 at `553fa5e`), does not
    move. Bound 0. The command is `cargo tree -p lumen-static-winit -e
    normal --prefix none [--features F] | sed 's/ (\*)$//' | sort -u |
    wc -l`; without the `sed` step a `(*)` repeat counts twice (117
    default), which is not K2's count (second review B8).
  - A default release window build links **zero symbols** from each
    new feature's module (`nm`), for every feature whose code a
    default build could reach (a vtable slot, a runtime branch, a
    provided method). The red run enables the feature.
  - **Recorded exception: M84's `image`** (second review R12). The
    linker drops an uncalled decoder whether the feature is on or off,
    so enabling it still links 0 symbols and the red run cannot fire
    (`plan-images-in-motion.md`, Decisions). Its falsifiable half is
    the crate count, T84.0(c).
  - Each feature's own crate count and stripped delta go in its
    manifest comment, citing the results doc.
  - A feature proposed as default-on must keep the stripped release
    `examples/showcase` delta within **3 MB**, the bound `a11y` failed
    (`docs/results-a11y-platform.md`: +4.12 MiB, "Bound: 3 MB.
    Fired."). It must also meet K3. Otherwise it ships default-off, as
    `a11y` did. The first draft's 4.12 MiB was the measurement that
    failed that bound, not the bound (reviews C23, B9).
  - This rule says nothing about firmware (K4 does).
- **K3 — the frame arms.** With the feature unused, each arm's median
  stays inside the eight-run spread (min–max) of the parent commit.
  Both sides run under `just bench-lock`, eight invocations a side,
  with the spread printed. `benches-competitive/run.sh` already keeps
  the minimum over `R` runs (default 5), so it is invoked with `R=1`
  and the eight invocations are the spread (second review B15). The
  arms are:
  - `R=1 benches-competitive/run.sh -p lumen-static-frame`, rows
    `idle (floor)`, `ALL rows: +paint` and `+render (damage)`. The
    rows that change one row are the printed `layout`, `+paint` and
    `+render (damage)`; no row is called "one-row-change", and a
    milestone names the printed labels;
  - `cargo run --release -p benches --bin textarea -- 100`, row
    `keystroke + layout, 100 KB` (the default prints 1, 100 and
    1024 KB, the last from 5 keystrokes). This replaces the first
    draft's "typing arm", which does not exist (review B10);
  - `cargo run --release -p benches --bin cascade`, row
    `cascade_deep (whole table)`.
- **K4 — firmware `.text` (D3, D5, D6).**
  - All four binaries stay at or under **81,920 B**
    (`scripts/check_text.sh`, raised from 65,536 B by T80.0; DECIDED
    2026-09-30, owner).
  - **One task may add at most 1,000 B to `firmware_interactive`**
    (19,998 B of headroom under 80 KB; T27.1's command cost 676 B on the
    paint-only `firmware`). The
    owner kept this cap when raising the ceiling. A task
    over that stops and attributes the delta by a symbol diff, as the
    M56 merge did in `docs/results-embedded.md`. The owner then
    decides: accept it, or move that surface behind a default-off core
    feature as `shader` is. §2.1's programme tally fits the ceiling,
    so this cap, not the ceiling, is what a task is expected to meet.
  - Every task that touches `lumen-static` or `lumen-static-widgets`
    prints the exact `.text`/`.rodata` delta for all four binaries
    against the parent commit's build. The comparator is
    `scripts/firmware_delta.sh BASE [BIN=B ...] [print]`, wrapped by
    `just firmware-delta <sha> [BIN=B ...]` (positional; `just` reads
    NAME=VALUE only before the recipe name), added in T80.0; every
    later milestone cites it and adds no interface. Bounds are per
    binary; with none the default is `firmware_interactive=1000` (this
    cap), the other three printed; `print` never fails. T80.0 writes
    the first dated four-binary table at `553fa5e` (the newest today is
    M52/T52.0's; the M56-merge figures are prose only,
    `docs/results-embedded.md`), and every later task rewrites the
    headline and that table in the same commit, whatever the 1%
    tolerance says (M52/T52.0's rule).
  - **Every per-task bound names its binaries** (second review B4).
    `just firmware-size` prints only the paint-only `firmware`, and
    `check_text.sh`'s 1% of the headline lets about 286 B through
    unrecorded and bounds nothing on the other three, so neither
    measures a task. The milestones' bounds are, all by
    `just firmware-delta <parent> ...` with each bound written out:
    T81.1, T83.0, K83b and M87, Δ 0 on all four (`firmware=0
    firmware_interactive=0 firmware_bind=0 firmware_cascade=0`, since
    no `BIN=B` word bounds only `firmware_interactive` at 1,000); T81.2, 64 B per binary, with the headline rewritten;
    K86a, 64 B per binary plus 32 B for
    `Patch::apply_to` on `firmware_cascade` and
    `firmware_interactive`.
  - **A change that sits entirely behind `cfg(feature = "std")`, or in
    a crate the firmware does not link,** may replace the delta with
    an `nm` count of its symbols over each `scripts/lib/firmware.sh`
    binary, bound 0, with a positive control (the same count above 0
    on a binary that links it), as M88/T88.3 does for `Request`
    (second review M26). M89, which appends `Request` variants, prints
    that count or a 0 B delta; "no firmware row" is not an option.
  - `shader`'s delta is **0 B** in all four, by construction now that
    its variants are gated too (D3); it stays a check. The red run
    removes the cfg from `ShaderId` together with
    `Painter::shader`/`shader_end` (M83/T83.3), and `just
    firmware-delta` prints Δ > 0 on every binary.

Milestone-specific kill rules live in their sections:

| rule | fires when (one line) | file |
|---|---|---|
| K80a | `Style` or `NodeState` is not 80/84 after T80.3 (M80's bytes do not fit) | `plan-text-fit.md` |
| K80b | M80a's cumulative `.text` > 1,024 B on `firmware` or > 1,536 B on `firmware_interactive` | `plan-text-fit.md` |
| K80c | `grid`, `retain --text` or `bindings`' memo-warm arm leaves its spread with the features unused | `plan-text-fit.md` |
| K80d | `fit::cut` measures more than `⌈log2(len)⌉ + 3` prefixes per cut | `plan-text-fit.md` |
| K81a | `Complex` on Latin costs > 1.5× `Basic` on `--bin textarea` | `plan-shaped-paint.md` |
| K81b | phases reset the glyph cache or triple its misses on `just grid` | `plan-shaped-paint.md` |
| K81c | kept face bytes exceed file size + 64 KiB per face | `plan-shaped-paint.md` |
| K81d | fontdue's subpixel raster misses k/3 ± 0.05 px; then `ab_glyph`, else M81b deferred | `plan-shaped-paint.md` |
| K81e | the `shaping` feature's stripped showcase delta exceeds 3 MB | `plan-shaped-paint.md` |
| K82a | the widened gradient's payload mirror reads above 56 B | `plan-paint-vocabulary.md` |
| K82b | one M82 task > 1,000 B on `firmware_interactive`; its < 1,024 B headroom floor is a check not expected to fire (§2.1) | `plan-paint-vocabulary.md` |
| K82c | with no fill and no path, a K3 row leaves its spread | `plan-paint-vocabulary.md` |
| K82d | path masks take > 25% of the glyph atlas's start size | `plan-paint-vocabulary.md` |
| K82e | software `VectorText` above 250 µs a frame uncached | `plan-paint-vocabulary.md` |
| K83a | a trivial shader pass costs > 50 µs per node | `plan-gpu-extension.md` |
| K83b | `shader` off, any firmware delta ≠ 0 B (true by construction; kept as a check) | `plan-gpu-extension.md` |
| K83c | with shaders unused, a K3 row leaves its spread (no fallback: the variants are already gated) | `plan-gpu-extension.md` |
| K83d | the default graph gains a crate, or the showcase > 3 MB | `plan-gpu-extension.md` |
| K84a | `image`'s own crates exceed 15 | `plan-images-in-motion.md` |
| K84b | the GStreamer crate's set exceeds 150 crates, needs `unsafe`, or can only skip | `plan-images-in-motion.md` |
| K84c | the whole-tree mark per animated frame exceeds 100 µs; then `App::tick_marked` | `plan-images-in-motion.md` |
| K84d | the atlas resets under animation despite the in-place write | `plan-images-in-motion.md` |
| K84e | 720p video on software presents fewer than 28 of 30 frames/s | `plan-images-in-motion.md` |
| K85a | a `Flexible` child is laid out more than once, or the layout row moves unused | `plan-layout-input.md` |
| K85b | culling does not beat the uncull arm's spread | `plan-layout-input.md` |
| K85c | the retention re-sync moves `retain` or the frame rows | `plan-layout-input.md` |
| K85d | M85's cumulative `firmware_interactive` `.text` > 1,024 B | `plan-layout-input.md` |
| K85e | T85.5's survey finds fewer than two real capture cases | `plan-layout-input.md` |
| K86a | T86.1 grows a firmware's `.text` past its per-binary bound (≤ 64 B of struct Δ; +≤ 32 B for `Patch::apply_to`'s branch on `firmware_cascade`/`firmware_interactive`) | `plan-transitions.md` |
| K86b | with every `transition == 0`, `appearance` or the idle row leaves its spread | `plan-transitions.md` |
| K86c | an active transition costs > 1.10 × a hover | `plan-transitions.md` |
| K86d | after T86.1, `Style` > 84 B, `NodeState` > 88 B or `PER_NODE_RAM` > 172 (K1's end-of-plan maxima) | `plan-transitions.md` |
| K86e | a live action's `ui.waitSettled` times out with no transition running | `plan-transitions.md` |
| K87 | a one-character append to 64 KB costs more than 1.5 ms (then T87.6 builds) | `plan-markdown.md` |
| K87a | the span store allocates on a still frame | `plan-markdown.md` |
| K87b | a link reported as `Hit::inner` turns an existing test or drive red | `plan-markdown.md` |
| K87c | the two part nodes move `cascade`'s `cascade_deep (markdown x16)` row (added by T87.1; a milestone-local arm, not a K3 arm) outside its parent's spread | `plan-markdown.md` |
| K88a | `spawn`'s pool arm leaves its spread after T88.1 | `plan-platform-requests.md` |
| K88b | the idle floor leaves its spread after T88.1 or T88.3 | `plan-platform-requests.md` |
| K88c | a 1080p GPU readback costs more than 100 ms | `plan-platform-requests.md` |
| K88d | `arboard/image-data` fails `just deny` | `plan-platform-requests.md` |
| K89a | the shared device loses a window's pixels or panics across a resume (review G35) | `plan-windows.md` |
| K89b | a shared-device second window's RSS is ≥ 50% of a separate context's | `plan-windows.md` |
| K89c | one window reads more idle frames or presents than T89.0's baseline | `plan-windows.md` |
| K89d | an idle daemon wakes without a deadline | `plan-windows.md` |
| K90a | the leaf firmware grows at all, or `firmware_interactive` > 256 B per task or > 81,920 B | `plan-desktop-integration.md` |
| K90b | with no menus, `dispatch` or the idle row leaves its spread | `plan-desktop-integration.md` |
| K90c | `tray` adds more crates than `a11y`'s +71 (K2's method), > 3 MB, or any `unsafe` | `plan-desktop-integration.md` |

## 5. Dependency order

- **M80 first.** It is small and touches only text. T80.0 corrects
  `docs/competitive-feature-gaps.md` and the manifest's three crate
  deltas (`lumen-static-winit/Cargo.toml`'s clipboard "11" → 5,
  `a11y` "62" → 71 and `dialogs` "79" → 10, all re-measured by K2's
  command, §0;
  M80's own T80.0 list must carry it, second review B8, B9, M17,
  R13), commits the queue entry, and raises the firmware ceiling to
  80 KB (D5) before any task spends against it.
- **M81 after all of M80**, not only T80.5 (review M11):
  - the caret, selection tint and `caret_rect` have to be right for RTL
    runs;
  - `line_height` has to reach the shaped path as well as `Cmd::Text`;
  - the layout-side ellipsis has to be checked on a mixed-direction
    run;
  - `Selectable`'s clusters have to come from the shaped run.

  M81 gates all four under `Complex`. M81 also performs D1a and D1c.
- **M81b (subpixel, T81.7–T81.8) does not wait for M82.** It
  rasterises through fontdue's public `rasterize_indexed_subpixel`,
  probed first (T81.7); if the probe misses its bound, K81d moves the
  `Complex` raster to `ab_glyph` or defers M81b (review T1).
- **M82 does not need M81.** Vector text goes through `Cmd::Path`.
  M82's transform applies to paths only, and rotated text is
  `VectorText` (outlines as paths), so the two do not meet (review
  M10). M82 performs §2.1's first `Style`
  re-budget.
- **M83 before M89b only** (second review M16). M83 performs D1b.
  M89a needs nothing from M83. M89b's shared device
  (T89.10) does not go through M83's factory: factory backends make
  their own device, and the built-in GPU backend shares one through
  `Weak<GpuContext>` (review M13).
- **M84/T84.7 and M88/T88.1, either order; the later extends the
  other.** Both change `inbox::JobTrack` (`stop` vs the
  `latest`/`fresh` progress slot). The later task makes the slot drain
  honour `stop` and `settled` treat a stopped job as settled, and
  carries the progress-after-stop gate (bound 0).
- **M84/T84.3 before M87/T87.4.** T87.4's demo names the id the
  showcase's `Image` entry registers, which T84.3 adds. The rest of M87
  does not wait: its images change by id, not by `frame`.
- **M85 before M90** (M90a states it as an ordering dependency).
  - `ContextMenu` is a deferring overlay, and M85's culling must never
    cull a node that may defer (review X2).
  - T90.2's button filter and M85's `Pop` visibility delivery both
    edit dispatch, so M90 rebases on M85's gates. (M85 builds no
    capture phase: T85.5 is a survey.)
  - M86 starts its entries in `Host::pump_with`, beside dispatch.
    Whichever of M85, M86 and M90 lands later re-runs the others'
    dispatch gates.
- **M86 depends on nothing** (second review M16). §2.1's byte table is
  an order of assignment, not a dependency: if M80 and M82 have not
  landed, M86 takes the next free bytes and its byte arithmetic is
  computed from the sizes it finds, printed, not from §2.1's row.
- **M87's italic is T87.5's probe.** Building it would need a face
  selector that reaches a widget's paint: `Cmd::Text` carries only size
  and weight, and `GlyphKey::face` stays inside the shell's paint path
  (M81), so no widget can choose a face. No milestone provides that
  selector, and the probe records so (review P14).
- **M88 before M89 and M90, or M89/T89.3 lands S1's core itself.** M88
  builds §2.8's `Request`/`Answer` and the two `Session` methods
  (`take_requests`, `answer`), which M89's windows and M90's tray and
  menu use. It adds no window key. If M89 is scheduled first, T89.3
  lands that core under S1's names and M88 adds only its own variants.
- **M89a after M88, or with S1's core (above); M89b after M89a and
  M83/T83.2.** M89 performs D1d (the `u32` window key and the program
  table) and deletes `Pane`/`Panes`.
- **M90a after M85** (a rebase, not a build dependency); **M90b after
  M89a/T89.6.** M90a depends on nothing in M89.
- **M88/T88.4 after `xclip` is on the reference machine** (D7, §3.4).
- **M61 is done** (§2.4). There is no ordering left to resolve.

## 6. Risks

- **`firmware_interactive` meets its ceiling.** It has 19,998 B left
  under 80 KB (D5). The accepted methods (`path` by D3, `text_spaced`
  and `fills` by D6, each priced at T27.1's 676 B, a paint-only
  figure) plus M85's layout and dispatch additions and M90's button
  filter come to about 3,450 B by §2.1's middle estimate, and the
  plans' stated bounds to 9,824 B (§2.1's table), so neither reaches
  it; M86's blend is
  behind the `transitions` feature, which the firmware does not
  enable. The residual risk is one task over K4's 1,000 B cap.
  **Revealed by** K4's per-task delta and `scripts/check_text.sh`.
  **Answer:** DECIDED 2026-09-30 (owner): the ceiling is 80 KB, raised
  by T80.0; a task over the per-task cap still stops, and the owner
  decides it with a symbol diff in hand.
- **A required `Painter` method is forwarded wrongly.** A missing one
  does not compile. **Revealed by** `deferring_forwards_every_command`,
  extended per method, and `tests/svg.rs`'s exhaustive match. For
  `shader`, it is revealed by `deferring_forwards_the_bracket` under
  `--features shader`, run by `dev-shape` (M83).
- **A crate that matches `Cmd` tests the wrong feature.** A
  `cfg(feature = "shader")` outside the core names that crate's own
  feature, so a unified build with the core's on fails E0004 there.
  **Revealed by** M83/T83.3's feature-on workspace build, and avoided
  by the `lumen_shader` cfg from each such crate's `build.rs` (§2.2).
- **A per-frame store folds an index.** This is the T27.3 defect again.
  **Revealed by** §2.9's two-entry gate in each store's task.
- **Byte budget taken out of order.** **Revealed by** every size
  assertion §2.1 lists (four in the core's tests, three in
  `fixtures/embedded`, plus `UI == 1192`), which fail the build. The task that moves them
  prints the new RAM.
- **M81's shaped paint regresses M80's features under `Complex`.**
  **Revealed by** M81's four gates (§5).
- **The `Complex` path rests on an unmaintained shaper** (`rustybuzz`,
  RUSTSEC-2026-0206, accepted in `deny.toml`; M81/T81.1 rewrites the
  reason to the shell's `shaping` placement). **Revealed by** `just
  deny` (the advisory gate). **Answer:** a harfrust port as its own
  milestone; reopen when the advisory escalates to a vulnerability, or
  when harfrust reaches feature parity for the pinned face's scripts.
- **A `Session` forward that compiles and is wrong.** **Revealed by**
  `just hot-window-drive` after a real swap. The compiler catches only
  absence.
- **Platform crates pull in GTK or system libraries.** This applies to
  `muda` on macOS and Windows (deferred, D2), `ksni` on Linux (D-Bus,
  no GTK thread; `tray-icon` on a GTK thread is only K90c's fallback)
  and GStreamer. **Revealed by** `just ci-verify` on the reference machine.
  A missing system library reports *skipped* and never counts as a pass
  (M43's R17-F6 rule); a missing §3.4 prerequisite (`xclip`, D7)
  fails outright with its install line.
- **A capture phase is wanted and has no plan.** If T85.5's survey
  finds two real capture cases, K85e does not fire and "the build is
  its own plan" (`plan-layout-input.md`), which has no M-number, file
  or §5 entry (second review M27). **Revealed by** T85.5's survey.
  **Answer:** M85 records the cases and a reopen condition, and the
  plan is written and queued then; §1.2 does not count the item as
  planned until it is. **Outcome (2026-10-06):** the survey found one case, so K85e
  decided the capture phase against and no plan is owed; the reopen
  condition is recorded in `docs/results-layout-input.md` T85.5.
- **Size L milestones (M80, M81, M82, M83, M84, M89, M90, M90a) will
  each find a defect in existing machinery** (second review M28: the
  first list omitted M80, M83 and M90, which size themselves L; M90a
  is sized L by its own file). Every milestone since M24 has found one, and M56–M57
  found three with their tests green. **Revealed by** the end-to-end
  drive each one names. Budget a "What building found" section before
  the first commit, not after the last.
- **iced's roadmap will move.** This plan is against the board as read
  on 2026-09-28. Items added later are out of scope until it is re-read.

## 7. The milestone plans

Each milestone has its own plan file. Every one keeps a "What the review
found" list mapping its findings to their resolution.

| milestone | plan |
|---|---|
| **M80** — text that fits: line height, ellipsis, read-only selection, IME placement | `docs/plan-text-fit.md` |
| **M81** — shaped paint: RTL drawn, the measurer chosen, subpixel pens (includes D1 moves a and c) | `docs/plan-shaped-paint.md` |
| **M82** — the paint vocabulary: radial and style gradients, paths, vector text | `docs/plan-paint-vocabulary.md` |
| **M83** — a GPU you can extend: custom shaders, custom backends (includes D1 move b; shader feature per D3) | `docs/plan-gpu-extension.md` |
| **M84** — images in motion: decoding, animated images, video | `docs/plan-images-in-motion.md` |
| **M85** — layout and input: `Flexible`, `pin`, `pop`, culling, a capture survey | `docs/plan-layout-input.md` |
| **M86** — widgets that move: built-in transitions for hover, press and focus | `docs/plan-transitions.md` |
| **M87** — Markdown you can style, link and illustrate | `docs/plan-markdown.md` |
| **M88** — the runtime missing pieces: Sipper, screenshot requests, rich clipboard (defines S1) | `docs/plan-platform-requests.md` |
| **M89** — more than one window, and none at all (uses S1 and D1 move d) | `docs/plan-windows.md` |
| **M90** — desktop integration: context menus, the menu bar (D2), the tray | `docs/plan-desktop-integration.md` |

## Considered and not chosen

- **A new `lumen-static-core` crate** to hold the backend, glyph and
  request types (D1). `lumen-static` already *is* the `no_std` common
  crate. A `std` crate cannot serve the firmware. `Program<Renderer>`
  would make backend choice compile-time, pull wgpu into hot dylibs
  across M37's loader/library split, and spread into the `Session` ABI.
  A new crate would also have to join `build.rs`'s source-hash list. The
  four moves of §2.7 do the same work inside existing crates.
- **Platform work as `Job`s answered by the window**
  (`Job::screenshot`, `Job::clipboard_*`, `Job::open_window`). A `Job`
  runs on a worker and is `Copy`; nothing lets the window answer it; a
  clipboard image is not `Copy`. Replaced by §2.8 (reviews C14, A1,
  P17).
- **The fn-pointer `WindowSpec`**, one naming a program by `fn`
  pointer. The loader would hold code across a swap, which is rule 3's
  defect. Replaced by D1d's index (review A3). The name is reused by
  M89 for a different design: a `WindowSpec` of plain data (size,
  page, `Prefer`, images).
- **Constructing `ComplexText` from the shell without the feature and
  the dependency move** (the first draft's
  `Program::text(Shaping::{Basic, Complex})`). It either cycles through
  `lumen-static-text` or links rustybuzz into every shell user.
  Replaced by D1a (reviews A18, B18). A `Shaping` enum in the shell is
  not what was rejected: M81 has one, with `Complex` existing only
  under the default-off `shaping` feature after D1a's move.
- **The winit-typed `backend_with`**,
  `Program::backend_with(fn(Arc<Window>, ..))`. It names winit types
  in a crate that cannot. Replaced by D1b (reviews A4, B19, C5). The
  name is reused by M83 for a closure over the `raw-window-handle`
  `BackendTarget`, which names no winit type.
- **A native menu bar on Linux through `muda`** (D2, review P26).
- **GPU `fetch_rgba` via `Gpu::read_rgba`.** It would compare the
  renderer with itself (review A19).
- **Per-method ABI stamp tests and "drop the Hot forward" injections.**
  They are vacuous or cannot compile (§2.3).
- **Gates committed ignored first, and trailing docs tasks.** These are
  §3's two reversals.
- **Reusing `Pane`/`Panes` as per-window state.** They hold a
  loader-side `Host<A>` (review A27).

## What the review found (cross-cutting)

One line per finding in the cross-cutting digest, with its resolution.

- **C2** — `Style` has 2 spare bytes against 3–5 wanted → §2.1 assigns
  every byte; M82 takes `Style` to 84.
- **C4** — every `Session` method is required, and the stamp is a
  source hash → §2.3: the compiler is the forward gate; no stamp tests.
- **C14** — nothing lets `Win` answer a `Job` → §2.8: `Request` data,
  `take_requests`/`payload`/`answer`, `App::answered`.
- **C21** — IME preedit is T14.8, not M41/T41.1 → §1.1 row cites both.
- **C23** — K2 used a11y's measurement as the bound → K2 is 3 MB.
- **C24** — the wrapper list was wrong → §2.2 lists implementors and
  consumers separately.
- **G1** — the T80.5 injection cannot happen → §2.3; M80 names a
  compiling injection.
- **G5** — a drive cannot reach the clipboard shortcut → §3.1 "a
  channel the harness can read"; M80 adds the agent path.
- **G7** — no gate runs new commands through `Deferring` → §2.2:
  `deferring_forwards_every_command` extended per method, with
  `CullTo` cases once M85b lands (second review A6).
- **G10** — T81.4's injection had nothing to drop → §2.3; M81 targets
  the `Program` measurer seam.
- **G28** — tasks without injections → §3.1; each gate names its
  injection inline (only M87 also tabulates them; second review R14
  corrected the claim that every milestone carries a table).
- **G29** — §0 claimed live drives the tasks did not have → §0
  narrowed; §3.1 and §3.2 per widget.
- **G32** — "firmware unchanged" had no comparator → K4:
  `scripts/firmware_delta.sh`, exact delta, both numbers; per-binary
  `BIN=B` bounds, positional `just` arguments (second review B5/B16).
- **G35** — K89a's evidence could not exercise a shared device → K89a
  restated in M89 as a live two-window GPU resume drive.
- **B2** — cumulative `Style` growth unstated; the embedded assertions
  unnamed → §2.1 table and every assertion site.
- **B6** — firmware pays for every required method through the vtable
  → §0 narrowed; K4; `Checksum` in §2.2.
- **B7** — firmware clauses for shell and text features were vacuous →
  §0 and K2: `nm` symbol check instead, for features a default build
  could reach; M84's `image` is a recorded exception (second review
  R12).
- **B8** — K1 could not fire; `Cmd` and `Event` judged by per-node RAM
  → K1 has exact sizes and end-of-plan maxima.
- **B9** — K2 bound 4.12 MiB → 3 MB.
- **B10** — no typing arm; the idle floor is in another workspace → K3
  names `lumen-static-frame`, `textarea` and `cascade` rows (the `grid`
  row was dropped: neither M85 nor M86 uses it).
- **B23** — figures restated without their owner; 62 vs 71 crates → §0
  cites results docs; T80.0 fixes the manifest comment (second review
  B9/M17/R13: M80's T80.0 list did not carry it; §5 now says so and
  M80 is asked to add it; second review B8: all three manifest deltas
  are re-measured by K2's command, not "62" alone).
- **A10** — no padding left; paint reads `NodeState` → §2.1 budgets
  `NodeState` paint copies.
- **A14** — M88–M90 need `Session` traffic and name none → §2.8 names
  the methods; M90 adds its own, required.
- **A15** — same as C4 → §2.3.
- **A16** — "inherits a default" does not compile for a required
  method; `Checksum` and `Sink` missing; transform in phase 2 → §2.2.
- **A25** — new shell features must enter `abi_fields` → §2.3,
  `shaping` and `shader` first.
- **T5** — `RunId` over a per-frame store brings back T27.3 → §2.9.
- **T9** — same as C4, and `abi_fields` described wrongly → §2.3.
- **T12** — M80 uses both spare bytes → §2.1.
- **T21** — required methods grow firmware whatever the feature set →
  §0, K4.
- **P30** — T84.3 and T85.1 had "—" injections → §3.1; M84 and M85
  sections.
- **M1** — the 80 B presented as likely to hold → §2.1: 80 is a
  baseline; 84 is planned.
- **M2** — no M81–M83 task re-measures firmware → K4 per task.
- **M3** — trailing docs tasks → §3.2: none.
- **M8** — same as C4 → §2.3.
- **M10** — M82's transform named `Glyphs` before M81 exists → §5:
  M81 builds no glyph command and M82's transform is a per-path `xf`,
  so the two never meet; text under a transform is `VectorText`.
- **M11** — M80's work not carried into shaped paint → §5: M81 after
  all of M80, with four gates.
- **M12** — host-answered work ambiguous with two windows → §2.8: the
  issuing session's window answers; `CloseWindow` names one.
- **M20** — tasks with no injection or no gate → §3.1; injections
  inline, beside each gate (second review R14), and pins watched red
  at T.0 (second review M14).
- **M24** — M61 status, and `.lss` grammar for new fields → §2.4: M61
  shipped; one declaration per setter; `background-fill`.

## What the second review found (cross-cutting)

One line per finding in the second review's digest for this file, with
its resolution. Owner decisions D6 and D7 (2026-10-02) and X5 are
applied too.

- **C9** → adopted: §2.1 lists `cursor.rs:32-33`, `focus.rs:203-204`
  and `fixtures/embedded/src/lib.rs:409/415/416`; milestones cite it.
- **C10** → adopted: §2.7b names `Offscreen`, winit's `AsScreen` and 7
  test `Blind`s.
- **B4** → adopted: K4 rules out `just firmware-size` and
  `check_text.sh`'s 1% and states the per-binary bounds (T81.1, T83.0,
  K83b, M87 Δ 0; T81.2 1,000 B; K86a 64 B, plus 32 B for
  `Patch::apply_to` after B13); the milestones are asked to cite
  `firmware_delta.sh`.
- **B6** → adopted: §2.1 has a task → bound → source table, sum
  9,792 B (10,206 B spare; 9,824 B and 10,174 B after B13's K86a
  allowance); 676 B is called a paint-only figure;
  K82b's floor is a check not expected to fire.
- **B7** → adopted: §0 narrowed to state and style; §2.1 holds the
  embedded `Ui` (1,192 B, M80's growth); K1 prints old and new `Ui`.
- **B9** → changed: the index's claim is kept, §5 names the edit
  (`Cargo.toml`'s "62" → 71, K2's method), and M80 is asked to add it
  to T80.0 and fix its C20 line; widened by B8 to all three deltas.
- **B14** → adopted: same list as C9.
- **B15** → adopted: K3 fixes `R=1`, eight invocations, `--bin textarea
  -- 100` and the printed labels; "one-row-change" is retired.
- **A9** → adopted: §2.8 drains after every windowless frame; key-0
  `Screenshot` is `Refused(Unsupported)`; injection named.
- **A19** → changed: §2.8 no longer claims the gate exists; M89 either
  adds the pending-`OpenWindow`-across-swap gate to T89.7 or states
  that nothing is pending.
- **M3** → adopted: §3.2 adds `a11y_roles.rs` `ROWS`,
  `expected.json` and `keys.py`, separates `roles.py`, and names each
  task's missing item.
- **M7** → adopted: §2.4 names the shell's `set`, the macro's `val`,
  `lower.rs`'s `Val`/`Takes` and an `lss_parity.rs` sheet, with an
  injection, for T80.2, T80.3, T82.3 and T86.1.
- **M14** → adopted: §3.1 requires each T.0 pin to be watched red at
  T.0 under a named temporary perturbation.
- **M16** → adopted: §5 splits M89a/M89b and M90a/M90b; M86 depends on
  nothing and computes bytes from the sizes it finds.
- **M17** → adopted: as B9.
- **M25** → adopted: as C10.
- **M26** → adopted: K4 lets a change wholly behind `std` (or outside
  the firmware's crates) print a positive-controlled `nm` count
  instead; M89 must print one.
- **M27** → adopted: §1.2 calls the item a survey; §6 lists the
  unplanned capture build.
- **M28** → adopted: §6 lists every L milestone; M90a's sizing is
  returned to M90.
- **R6** → adopted, via D6: §0 and the header attribute `text_spaced`
  and `fills` to D6 and `path`/gradient to D3; `caret_rect` and
  `outline` slots are under K4 and reported.
- **R12** → adopted: K2's `nm` clause covers reachable features;
  `image` is a recorded exception, its crate count the falsifiable
  half.
- **R13** → adopted: as B9.
- **R14** → adopted: §3.1 and G28 say injections are inline.
- **X7** → adopted: §3.1 requires a `clippy -D warnings` line (and a
  `cargo doc` line for public items) per new feature, workspace-wide
  for `shader`.
- **D6** → applied: header, §0, §2.1, K4, §6.
- **D7** → applied: header, §3.1, new §3.4 (`xclip` via `apt`, a
  required `probe.sh` case exiting 1, `ci.sh`'s probe line), §5, §6.
- **X5** → applied: §2.7a's row says T81.1 rewords `deny.toml`'s
  rustybuzz acceptance.
- **A20/G3** (from `plan-text-fit.md`) → adopted: §2.2 says a wrapper
  does not override `text_weighted` (T80.1 deletes the three
  overrides, `Scaled`'s rounding moves into `text_spaced`); the
  asserted count reads 5 today, 6 after T80.1 (checked:
  `overlay.rs:468` asserts 5).
- **B5/B16** (from `plan-text-fit.md`) → adopted: K4's comparator is
  `firmware_delta.sh BASE [BIN=B ...] [print]`, positional `just`
  arguments, default `firmware_interactive=1000`; T80.0 writes the
  first dated four-binary table at `553fa5e` (checked: the M56-merge
  figures in `results-embedded.md` are prose only).
- **B1/M18/R17** (from `plan-text-fit.md`) → adopted: §3.2's T80.0
  bullet names all twelve files and both allowlist blocks (checked:
  `doc-paths.allow` carries the M80–M90 block and the three bins).
- **D6** (from `plan-text-fit.md`, owner record) → adopted in part: the
  header's D6, §0's "(D3, D6)" and split sub-bullet, §2.1 and §6 were
  already split; D3's paragraph now points `text_spaced`/`fills` to D6.
- **D6** (from `plan-paint-vocabulary.md`) → already satisfied: §0,
  §2.1 and §6 attribute `path`/gradient to D3 and `text_spaced`/
  `fills` to D6, and the header carries D6.
- **X5** (from `plan-shaped-paint.md`) → adopted: §6 lists the
  unmaintained shaper (`deny.toml`'s RUSTSEC-2026-0206), revealed by
  `just deny`, answered by a harfrust milestone.
- **A15** (from `plan-gpu-extension.md`) → changed: §3.2 already had
  the `a11y_roles.rs` row bullet; it gains the injection and the
  feature-gated-widget note instead of a duplicate bullet.
- **G11/B3/A12/T7/R9** (from `plan-paint-vocabulary.md`) → adopted:
  §2.2 states K82b's fallback as provided methods under
  `cfg(feature = "alloc")` with wrapper overrides.
- **A7** (from `plan-gpu-extension.md`) → adopted: §2.3 gives shader
  two stamp entries, `lumen_shader` and `shader`, both in T83.3.
- **M6** (from `plan-gpu-extension.md`) → adopted: the allowlist
  deletion is a per-task rule under "There is no trailing docs task",
  for both allowlists, not a widget-only item.
- **G13/T20** (from `plan-gpu-extension.md`) → adopted: K4's shader red
  run removes the cfg from `ShaderId` with the bracket methods.
- **T6, A16/M8/R3** (from `plan-gpu-extension.md`) → adopted: §2.7b
  gates the `backend` module behind the shell's `truetype` and makes
  `fetch_rgba`/`set_images`/`frame_rgba` required (checked: all three
  are provided at `backend.rs:57/68/94`).
- **M11** (from `plan-images-in-motion.md`) → adopted: §5 orders
  T84.7 and T88.1 either way, the later extending the other.
- **A6/P6/R4** (from `plan-layout-input.md`) → adopted: §2.2's table,
  the shader-override paragraph, the D3 site list and the forwarding
  gate name `CullTo` as a fourth wrapper once M85b lands.
- **B13** (from `plan-transitions.md`, K86a) → adopted: the table row
  is per binary with a 32 B `Patch::apply_to` allowance; K4's bounds
  list and §2.1's tally follow (T86.1 64 → 96 B, sum 9,792 → 9,824 B,
  spare 10,206 → 10,174 B).
- **B13** (from `plan-transitions.md`, K86d) → adopted: K86d is K1's
  end-of-plan maxima after T86.1.
- **X4** (from `plan-markdown.md`) → adopted: K87c names the
  `cascade_deep (markdown x16)` row; §2.6 says a milestone-local row
  is not a K3 arm.
- **C4** (from `plan-markdown.md`) → skipped: the index never says M87
  touches only `rich.rs` (its K4 "M87 Δ 0" holds, `journal.rs` is in
  the agent crate).
- **A1** (from `plan-platform-requests.md`) → adopted: §2.8's answer is
  applied inside `Host::answer`, with no host-side queue.
- **A18/P8/R21** (from `plan-platform-requests.md`) → adopted: §2.8's
  redraw drain runs on every frame after present-or-skip, after the
  `failed()` return.
- **P9** (from `plan-windows.md`) → adopted in part: §2.8 already
  drained the windowless session after every frame and every routed
  post or answer (A9); the A10 redraw sentence is added.
- **B8** (from `plan-desktop-integration.md`) → adopted: §0 and §5 have
  T80.0 re-measure all three manifest deltas by K2's command; K2 names
  the command (re-run at `553fa5e`: 96 / 106 / 167, as reported).
  The K90c row now bounds against `a11y`'s +71, as
  `plan-desktop-integration.md` does, since `dialogs`' 79 was the old
  method's count.
- **consistency-3 item 1** → §0 names `docs/results-a11y-platform.md`,
  :90–91, and clipboard's re-measured +5 (91 → 96); §5 adds clipboard
  "11" → 5. M80/T80.0 now carries all three deltas.
- **consistency-3 item 5** → K4's list says T81.2, 64 B per binary;
  the tally row stays 1,000 B as an upper bound citing T81.2.
- **consistency-3 item 9** → §2.8 states M89's decision: no
  `OpenWindow` is pending at a swap (`Win` answers inside the drain,
  T89.3 gate 2), replacing "M89's to state".
- **consistency-3 item 13** → §1.2's Video row names `VideoFeed`, not
  `FrameSource`.
- **consistency-3 item 14** → §6 lists M90a as L and drops the
  "sized M by its own file" sentence.
- **consistency-3 item 15** → §2.7 row b: M83 (T83.1 the move, T83.2
  the factory).
- **consistency-3 item 16** → §3.2's list adds T85.1 (`Layout`:
  `roles.py`) and T90.5 (`MenuBar`: four items), and splits T90.4
  (`ContextMenu` all four; `CommandList` `a11y_roles.rs` only).
- **consistency-3 item 17** → §2.8 adds M90/T90.5's `Session::paste`
  → `Option<usize>`; §2.1's `Event` paragraph adds `Event::Menu`.
- **consistency-3 item 21** → §2.7c lists `GlyphKey`'s fields in M81's
  order (`px_bits`, `index`, `face`, `phase`).
