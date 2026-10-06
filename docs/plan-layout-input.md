# M85 — layout and input: `Flexible`, `pin`, `pop`, culling, capture

> **Written 2026-09-28 as a section of `docs/plan-iced-roadmap.md`;
> revised 2026-09-29 after that plan's seven-lens review, and moved to
> its own file; revised 2026-10-02 after the second review (list at
> the end). T85.0–T85.5 landed 2026-10-06, and M85a closed on its kill rules: K85a and K85d held, K85e decided the capture phase against (`docs/results-layout-input.md`, "M85a closes"). T85.6 landed 2026-10-06.** The shared byte inventory, seams (S1,
> S2), test and doc discipline, and plan-wide kill rules K1–K4 are in
> `docs/plan-iced-roadmap.md` §2–§4 and bind this plan; this file does
> not restate them.

*Split in two, because the review showed culling is a retention change
and not a paint change (X1). **M85a** (size M, six tasks): `Flexible`,
`Pin`, `Pop` and the capture-phase survey. **M85b** (size M, three
tasks, highest risk in M85): container culling, which ships only if its
probe beats the paint arm's spread **and** retention stops charging a
shifted command count as damage. Closes: the `Flexible` length, `pin`,
`pop`, primitive culling in containers. **Smart event propagation is
narrowed to a survey** (T85.5): if it finds two real cases, the build
is its own plan, because a required `Node` method touches every hand-
written `Node` impl (47 in `lumen-static-widgets/src` alone, counted
with `grep 'impl.*Node for'`); that plan is unwritten, and K85e
names when it is written. Depends on nothing. `Pop` could serve
lazily loaded images (a possibility; M84 does not use it).
`NodeDesc::INKS_OUTSIDE` is a general mechanism for any widget whose ink
leaves its box; M82's two path users do not need it, because M82 keeps
their ink inside their box (see Decisions, and the note for the
assembler).
Read against `553fa5e`, which is `92a4033` plus M61 (`.lss`).*

**Decisions.**

- **`Length::Flexible(u16)`** — `lumen-static` (`style.rs`). Laid out
  under a main-axis maximum of its fill share, so it ends at
  `min(content, share)`. **What it leaves goes to the `FillPortion`
  children, never to another `Flexible`.** In the derive's row/column
  body (`lumen-static-macros/src/lib.rs`, scans 1 and 2 at ~540–600)
  scan 2 splits into 2a (`Flexible` children, each under
  `loose_child(axis, inner, share, align)`) and 2b (fills, dividing
  `free - flex_used` among the fill portions). 2a is skipped when the
  flexible portion sum is 0, so a row without `Flexible` runs today's
  arithmetic. **Every child is laid out exactly once per container
  layout, with or without `Flexible`** — there is no second pass to
  gate, which is what G19 found the first draft's rule could not
  deliver. The price is stated, not hidden: two `Flexible`s never share
  each other's leftover (CSS §9.7's freeze loop), so
  `[Flexible 80 px, Flexible 400 px]` in 300 px reads 80/150, not
  80/220. Reopen: an application that needs it, with the layout arm's
  cost of a second round measured. On the cross axis, and in a stack,
  wrap or grid (which divide nothing), `Flexible` resolves as `Shrink`;
  `resolve_self` (`ops.rs:276–303`) gets the arm. **Budget:** `Length`
  stays 8 B (`Fixed(f32)` is the widest variant), so `Style` and
  `NodeState` are unchanged from the parent commit's sizes, printed
  (D4: no growth here; M82/M86 may land first and take them to 84/88,
  so no literal is asserted). Firmware `.text` grows by the derive
  branch in every derived row/column; printed against K4's per-task
  1,000 B and the 81,920 B ceiling (D5). It is not a `Painter` method,
  so neither D3 nor D6 covers it.
- **`.lss` spells it `flexible` and `flexible(n)`** — M61 is on this
  tree (`lumen-static-lss`, `check_lss_setters.sh` in `just static`),
  so this is not conditional. Four exhaustive sites:
  `lumen-static-lss/src/lower.rs` (`Length` and `length()`),
  `lumen-static-shell/src/lss.rs` `length()`, and
  `lumen-static-macros/src/lss.rs`. No new `Patch` setter: it is a
  value of `width`/`height`.
- **`Pin<W>`** — `lumen-static-widgets` (`surface.rs`, beside `Card`,
  the same one-child splice). A single child laid out `Shrink` inside
  the pin's box less its offset, and placed at `pos + (x, y)`. The pin
  is `FILL` by default, pushes a clip of its own rect in `paint` and
  gates `hit` on it, and answers `clips_children()` — so paint and hit
  agree (A1). Not out-of-flow positioning, which
  `docs/plan-capabilities.md` rejects. **Budget:** two `f32` in the
  application's struct; 0 B `Style`/`NodeState`; firmware 0 B (the
  fixture names no `Pin`), printed.
- **`Pop<S, W>` and `Event::Visibility { visible: bool }`** — the
  first draft had `place()` deliver to itself; `place` has no clip, no
  context and no `&mut S` (`widget.rs:61`), and `pass::deliver` never
  runs an application handler (`pass.rs:580–603`). The mechanism is
  host-side, built from pieces that exist:
  - **Which nodes.** `NodeDesc::WATCHES_VIEW` (flag bit 4; bits 0–3
    are taken, `desc.rs:53–67`), set by `Pop` on its own table entry.
    `Host` scans `SUB_TABLE` **once at construction** into
    `view_watch: Vec<u32>`; a tree with no `Pop` keeps it empty and
    pays one empty-slice check per frame. No per-node virtual call, so
    the `Node::tick` objection (`event.rs:1193–1198`) does not apply.
  - **When.** After `tree.place` in `Host::frame`
    (`lumen-static-shell/src/lib.rs:2020–2023`). `place` is a
    whole-tree walk under both strategies ("`place` takes no
    `Changed`", same site), so positions are fresh under
    `Boundaries<256>` as well as `Everything`.
  - **Against what.** A node is visible when it is not `gone` (the C5
    predicate, `pass.rs:1502`, made `pub`) and its rect intersected
    with the window and with the rect of **every ancestor answering
    `clips_children()`** (M55: `ScrollView`, `Repeat`, `List`, `Dyn`)
    has positive area. Ancestors come from the table's `parent` chain:
    one virtual call per ancestor per watched node per frame.
  - **How the app hears it.** A change is queued in
    `view_pending: Vec<(u32, bool)>` (data, no `fn` pointer across a
    swap) against `view_seen` (starts `false`, so a node visible on
    frame 1 gets `true` and one never shown gets nothing).
    `next_deadline_ms` (`lib.rs:2254`) answers `Some(0.0)` while it is
    non-empty — the existing `Session::next_deadline_ms` carries it, so
    **no new `Session` method**. The next `pump_with` drains it before
    its `update_marked` (`lib.rs:1888`): `pass::deliver` offers the
    event to the `Pop` (which records it and consumes), then
    `pass::tell_visibility` runs the `Pop`'s own handler, asked
    through `Handlers::handler` with `NodeSel::visibility(i)` — a
    sentinel `inner`, the pattern `NodeSel::dismissed` and `tell_owner`
    already use (`event.rs` ~838, `pass.rs` ~1461). No activation ever
    sends that sentinel, so a click inside a `Pop<Image>` cannot run
    its load handler, and its handler never answers a click. The
    handler reads `ctx.event`. The write reaches the screen **one frame
    after** the frame that placed the node in view, printed by the gate.
  - **Marking.** A delivery that ran a handler is marked through
    `mark_handled(&consumed)` (`lib.rs:1657–1671`), as a click is, not
    as `ScrollEnd` is (`lib.rs:2203–2210`, which marks only the
    consumed subtrees because `pass::deliver` runs no app handler). An
    unsplit application's handler may write any widget (the demo's
    readout is outside the `Pop`), so it marks the whole tree unless
    `A::STATE_IS_SPLIT`; a split one marks the consumed subtree (A11).
  - **Across a swap.** `reasked` (`lib.rs:742`, reached by `adopt`,
    `:725`, and so by `Host::reload`) sets every `view_seen` to
    `false` and clears `view_pending`, so an adopted application hears
    `visible: true` from each `Pop` already in view on its first frame;
    last frame's visibility describes the old application (X8).
  - **Inside an arena it is refused** (X4). A `Repeat` materializes the
    visible window and recycles instances across logical rows
    (`repeat.rs:1–2, 46`); a row leaving is never placed out of view,
    and arena nodes are not in `SUB_TABLE`, so the host would never see
    them. `Repeat::new`, `List`'s constructors and `Dyn::new` assert
    `!widget::table_has_flag(T::SUB_TABLE, NodeDesc::WATCHES_VIEW)`
    with a sentence naming the alternative: in a repeater the
    visibility signal is `first_row()..first_row() + window_len()`
    (`repeat.rs:272–280`), which the application reads. The assertion
    reads a `const` table and folds away when false.
    `table_has_flag` does not exist yet (no `table_has`/`has_flag` in
    `crates/`): T85.4 adds `pub const fn table_has_flag(t: &[NodeDesc],
    f: u8) -> bool` beside `table_is_well_formed` (`widget.rs:727`),
    and T85.8 reuses it (C15, P7).
  - **Budget.** `Event` stays 16 B (`Touch` is the widest). 0 B
    `Style`/`NodeState`; `NodeDesc` is flash, not RAM
    (`fixtures/embedded/src/lib.rs:395–396`). Host heap: 4 B per watched
    node plus a `bool`. Firmware: one `Event` variant and a flag
    constant; `.text` printed (expected ≈ 0 B, no firmware code matches
    it).
- **Culling (M85b)** — `List` already culls on `clip_bounds()`
  (T16.12, `hatch.rs:636–650`; 62.8 → 7.5 µs paint and 10,002 → 39
  commands at 10,000 rows, `docs/results-competitive.md` §"What M16 did
  buy"). The first draft's "M50's +20 µs at 10k rows" exists in no doc
  and is dropped. Three things the draft missed are decided here:
  - **The bound.** `clip_bounds()` is the surface, deliberately not the
    clip stack (`display.rs:878–898`), so a column culled against it in
    a 400 px `ScrollView` keeps every row inside the window. The
    `ScrollView` paints its content through `CullTo`, a `Painter`
    wrapper in `lumen-static` (`display.rs`, beside `Scaled`) that
    answers `clip_bounds() = inner ∩ viewport` and forwards **every**
    other method that exists when T85.8 lands, provided ones included:
    today's `text`, `glass`, `gradient`, `defer`, `line`, `image`,
    `shadow`, `stroked_rect`; `text_spaced` (M80), `path` and `fills`
    (M82) if landed (required, so compile-caught); and, if M83 has
    landed, `shader`/`shader_end` under `#[cfg(feature = "shader")]`
    (D3). The last two are **provided** with a leaf default of
    `push_clip`/`pop_clip`, so a `CullTo` that inherits them compiles
    and turns every shader bracket inside a `ScrollView` into a plain
    clip that draws only the fallback — the `Deferring` rule, and the
    reason `CullTo` is a fourth wrapper beside `Scaled`, `Deferring`
    and `Recording` in §2.2's list (A6, P6, R4). No new `Painter`
    method, so no firmware vtable growth and no `Checksum` change.
  - **The ink, not the rect** (X2). A derived container culls child
    `k` only when `ops::cull_child(out, &st[at])` says its ink —
    `rect_of` grown by `display::shadow_ink(rect, st.shadow)` and by the
    focus ring's `FOCUS_GAP + focus_ring` — misses the bound. **A child
    whose subtree carries `NodeDesc::INKS_OUTSIDE` (bit 5) is never
    culled**, decided at compile time per field
    (`widget::table_has_flag(<F as Widget>::SUB_TABLE, INKS_OUTSIDE)`,
    the const fn T85.4 adds): `Popup`
    and `Veil` set it, because they register for the overlay pass from
    inside their own paint (`popup.rs:253–262`, `veil.rs:324`), and any
    widget whose ink leaves its box sets it. M82's path users
    (`PaintBox`'s paths, M82/T82.5; `VectorText`, M82/T82.8) keep their
    ink inside their box by M82's decision and do not set it.
    `ops::cull_child`'s body is
    `false` without core's `alloc` feature, so the `no_std` firmwares
    (which take neither `std` nor `alloc`) link **0 B** of it; printed.
  - **The retention it breaks** (X1). `Incremental` compares command
    *n* with slot *n* (`retain.rs:380–381, 549–566`). A culled range
    changes the count as rows enter and leave, so every command painted
    after the container is compared with another command's slot and its
    ink is charged as damage on every scroll frame. `List`'s cull
    already does this. **Fix: `Retention::scope_end(key)`, required**
    (four impls: `Rebuild`, `Incremental`, `Streaming<N>`, and
    `tests/container_fill.rs`'s `Counted`), called by `Recording::pop_clip`
    (`retain.rs:1068`) with a key hashed from the per-depth pop ordinals
    (a row's own inner clip pops at a deeper depth, so culling inside a
    scope never shifts the keys at or above it). `Incremental` keeps
    last frame's slots in a second `Vec` plus `(key, slot index)` for
    each scope end; when a key's index moved, the unmatched old and new
    inks inside the scope are unioned into damage (sound: they lie
    inside the scope's clip) and the read cursor jumps to last frame's
    index. `Streaming<N>` keeps positional matching and says so: it
    over-reports, never under-reports, and no firmware culls
    (`Checksum` answers `None`). Reopen: an allocator-free host with a
    surface. **Budget:** +24 B per command of host heap (`(u64, Rect)`)
    for the second buffer, std only. S2 does not apply: no side store,
    and the key is ordinal-derived, not a store index.
  - **Unbracketed culling is recorded, not fixed.** A derived column
    that culls against the surface with no enclosing clip still shifts
    what follows (the host's focus ring, overlays). Every change that
    moves such a column moves what follows it anyway; the gate prints
    this case beside the bracketed one.
- **The capture phase is a survey** (T85.5). Candidates, each to be
  confirmed or refused with `file:line`: `PullToRefresh` taking a drag
  its content declined and leaving a button's press alone, through
  `EventCtx::target_is_control` (`event.rs:427–433`, `nav.rs`), and
  `ScrollView` stealing a drag from a child after a threshold. A "real
  case" is shipped code or a `docs/widget-audit.md` row whose
  workaround exists **because** events only bubble. Fewer than two
  closes the item as decided against (K85e). Two or more opens its own
  plan, with `Node::on_capture` **required** (P8): `leaf_common!` and
  the derive emit the leaf's "pass", every other hand-written impl
  writes it, and the wrappers that hold an off-table widget (the survey
  lists them from `grep`, `NumberInput` and `Markdown` at least, per
  `Node::probe`'s rustdoc) forward it, with an injection that makes one
  of them answer "pass".

**Tasks — M85a.**

- **T85.0 — the gates that compile today.**
  - `crates/lumen-static/tests/flexible.rs`: a test leaf `CountingLeaf`
    (`leaf_common!`, fixed content width, a `Cell` counter in `layout`)
    in a derived 300 px row `[CountingLeaf(Shrink, 80 px), CountingLeaf(FILL)]`,
    laid out once under `LayoutCx` as `tests/fixed_container.rs` does.
    Prints each child's layout count and size. Bound: count == 1 each;
    80/220. **Setup landed:** the row's own width is 300 (printed).
    **Injection:** emit scan 2's `d.layout` twice in the derive → the
    fill reads 2, by name. **Accept:** three `Shrink` children read
    1, 1, 1.
  - `size_of::<Length>() == 8`, beside `tests/cursor.rs:32`'s `Style`
    assertion, printed. **Injection:** `FillPortion(u64)` → 16.
  - `crates/lumen-static-shell/tests/pop_visibility.rs`, setup half: a
    `ScrollView` whose viewport (printed) is strictly shorter than the
    window (printed), a target leaf below its fold, wheeled 20 px at a
    time. Asserts each wheel is `handled`, the offset moved (printed),
    and that **some step leaves the target inside the window and
    outside the viewport** (printed) — the setup the T85.3 injection
    needs, which G20 found the first draft never required. Runs as
    `Host<_, Incremental, Everything>` and the default
    `Boundaries<256>`. **Injection:** a viewport as tall as the window
    → the "inside window, outside viewport" assertion fires.
  - Record `just firmware` (`scripts/check_text.sh`, four binaries) at
    this commit as M85's baseline. Every later M85 task measures its
    own delta with `just firmware-delta <parent>` (default bound
    `firmware_interactive=1000`, K4), and K85d's cumulative check
    compares against this commit; all print against the 80 KB
    ceiling, 81,920 B (D5, raised by
    M80/T80.0; headline 28,616 B; `firmware_interactive` 61,922 B with
    19,998 B left,
    `docs/results-embedded.md`, "The headline re-measured at the M56
    merge").
  - *Same commit:* `docs/results-layout-input.md` is created here, not
    in T85.5 (M15), with the firmware baseline above and the K85a–K85e
    threshold table written before anything is built. Every later M85
    task records its numbers (layout counts, firmware deltas, K-rule
    rows) there in its own commit; T85.5 adds the survey section. Its
    `scripts/doc-paths.allow` entry (M80/T80.0) is removed here.
- **T85.1 — `Flexible`.** Variant, `is_flexible()`, the derive's 2a/2b
  split, `resolve_self`'s arm, the stack/flow arms, the four `.lss`
  sites and a `lumen-static-lss/src/tests.rs` case beside the
  `fill(3)` one.
  *Same commit:* rustdoc on `Length`; `docs/layout-and-storage-decisions.md`
  §2's table gains `Flexible(k)` and a sentence that distribution stays
  single-round because leftover goes only to fills;
  `docs/00-handoff-static-tree.md` §12 gets a DECIDED line pointing
  there; `book/src/layout.md` gets a runnable, asserting snippet;
  `book/src/styling.md`'s length list (`fixed(12px)`, `fill`,
  `fill(2)`, `shrink`) gains `flexible(n)`; the showcase gets a
  `Layout` entry (`examples/showcase/src/catalog.rs` `entry!`, a `Demo`
  variant in `stage.rs`); `scripts/drive/widgets/layout.py` gets a
  static geometry inventory entry like `Wrap`'s, with a `roles.py` row.
  **Gates** (`flexible.rs`):
  - `[CountingLeaf(Flexible(1), 80 px), CountingLeaf(FILL)]` in 300 px → 80/220;
  - at 400 px of content → 150/150;
  - `[Flexible 80, Flexible 400]` → 80/150, pinning the stated limit;
  - layout count == 1 for every child in every case above (printed);
  - `Length` asserted at 8 B; `Style` and `NodeState` asserted equal
    to the parent commit's sizes (no literal: M82/M86 may land first,
    R16), all three printed, and RAM per 100 nodes printed against
    24 KB and against the parent commit's figure (unchanged).
  - live: `just widget-drive Layout` (both strategies; `Layout` is the
    inventory entry this task adds to `layout.py`, which
    `scripts/widget_drive.sh` passes to `scripts/drive/run.py` by name;
    `Flexible` is a `Length` variant, not an entry, G35) asserts the
    showcase row's two boxes against the same 80/220 rule at the
    stage's width, the entry's row count and the quantities printed.

  **Injections:** treat `Flexible` as `FillPortion` in scan 2 → the
  80 px case reads 150/150; run scan 2a after 2b → the fill reads 150
  and the 80/220 case fires; lay a `Flexible` child out a second time
  under its measured size → count 2. **Accept:** every row without
  `Flexible` in `tests/` renders byte-identically (`just test`).
  Firmware: `just firmware-delta <parent>` (default
  `firmware_interactive=1000`), four binaries printed.
- **T85.2 — `Pin<W>`.** *Same commit:* `lumen-static-widgets` export;
  `scripts/check_observe.sh` ALLOW line ("a wrapper: its content
  speaks", as `Card`); `roles.py` row `GenericContainer`; a
  `#[widget_id("Pin")]` field in
  `crates/lumen-static-widgets/tests/catalogue/mod.rs`'s `Ui`, a
  `row("Pin", "Pin", Role::GenericContainer, false)` in
  `tests/a11y_roles.rs`'s `ROWS` and `"Pin"` in its `GENERIC_BOUND`
  (else `every_node_impl_has_a_row` and the explicit-list check fail
  by name, second review M3); an inventory
  in `scripts/drive/widgets/surface.py`; a showcase entry; rustdoc;
  handoff §12 DECIDED line; `docs/competitive-feature-gaps.md` row.
  `Pin` is not in `scripts/doc-identifiers.allow` (checked).
  **Gates** (`crates/lumen-static-widgets/tests/pin.rs`, and live):
  - child `pos` == pin `pos` + `(x, y)` exactly, printed;
  - a press/release at the child's centre is `handled` and the hit
    node is the child's index (printed), not the pin's;
  - a press inside the pin but outside the child is not the child's;
  - `just widget-drive Pin`, both strategies: a click on the pinned
    button changes the readout, damage 1.00× of its box.

  **Injections:** `place` ignores the offset → the child sits at the
  pin's origin and the centre press hits the pin, by name; drop the
  `hit` gate on the pin's rect → a press outside the pin reaches a
  child placed past its edge. Firmware: `just firmware-delta <parent>
  firmware=0 firmware_interactive=0 firmware_bind=0
  firmware_cascade=0`, printed.
- **T85.3 — the visibility pass: `Event::Visibility`,
  `NodeDesc::WATCHES_VIEW`, `NodeSel::visibility`,
  `pass::tell_visibility`, `Host`'s `view_*` fields.** Tested with a
  test-only flagged leaf, so the mechanism lands before the widget.
  *Same commit:* rustdoc on each; handoff §6/§12 (the pass, and why it
  is a host pass and not `Node::tick`); `book/src/events.md` anchor.
  **Gates** (`pop_visibility.rs`, both strategies):
  - wheeling the flagged leaf in and out delivers exactly one `true`
    and one `false`, and **the test app's counter moves 0 → 1**
    (setup landed: each wheel `handled`, offset printed);
  - `true` arrives **exactly at the first wheel step whose rect
    intersects the viewport**, that step printed beside T85.0's
    inside-window-outside-viewport step (G18: the counts alone pass
    under a window-only intersection);
  - while `view_pending` is non-empty, `next_deadline_ms()` answers
    `Some(0.0)` (printed each step), and a modelled loop that pumps
    only when the deadline is due paints the app's write within one
    frame of the frame that placed the leaf in view (frames counted
    and printed, bound 1). A test that pumps every step cannot see the
    deadline, because the next `pump_with` drains `view_pending`
    whatever it answers (G17);
  - 50 frames of still visibility deliver nothing (printed, bound 0);
  - a click on the flagged leaf does not run its visibility handler
    (counter unchanged, click `handled` by what it hit);
  - an **unsplit** app under `Boundaries<256>` whose visibility handler
    writes a sibling label outside the leaf: the label's new text is
    in the next frame (pixels and `ui.lastDamage` printed), matching
    `Everything` (A11);
  - `Host::adopt` of a fresh app while the leaf is in view: the new
    app's handler count goes 0 → 1 after one pump, printed (X8);
  - `size_of::<Event>() == 16`, printed.

  **Injections:** intersect with the window only → `true` arrives at
  the step T85.0 printed as inside-window-outside-viewport, and the
  arrival-step gate fires with both steps printed; drop the
  `next_deadline_ms` term → the deadline gate reads `None` (or a timer)
  with `view_pending` non-empty, and the modelled loop's one-frame
  bound fires; let activation answer the sentinel (a `Handlers` impl
  that ignores `sel`) → the click gate fires; mark the visibility
  delivery as `ScrollEnd` does → the sibling label is stale under
  `Boundaries<256>` while `Everything` passes; leave `view_seen`
  untouched in `reasked` → the adopted app's count stays 0.
  **Accept:** a tree with no flagged node leaves `view_watch` empty
  (printed) and its frames are byte-identical; a split app's
  visibility frame marks only the consumed subtree (marked count
  printed). Firmware: `just firmware-delta <parent>` (default
  `firmware_interactive=1000`), printed.
- **T85.4 — `Pop<S, W>`, and the arena refusal.** `Pop` in
  `lumen-static-widgets` (`surface.rs`), `on_view: fn(&mut S,
  &EventCtx)`, `visible()`; the new `pub const fn
  widget::table_has_flag(t: &[NodeDesc], f: u8) -> bool` beside
  `table_is_well_formed` (`widget.rs:727`), with rustdoc and a `const`
  unit test (a table with the flag on a nested entry answers `true`,
  one without answers `false`); the `table_has_flag` assertion in
  `Repeat::new`, `List` and `Dyn::new`. *Same commit:* export;
  `check_observe.sh` ALLOW line; `roles.py` row `GenericContainer`; a
  `#[widget_id("Pop")]` field in `tests/catalogue/mod.rs`'s `Ui` (a
  `#[tree]` struct, not an arena, so the refusal does not apply), a
  `ROWS` row and a `GENERIC_BOUND` entry in `tests/a11y_roles.rs`, as
  T85.2 (second review M3); an
  inventory in `surface.py` whose demo is a `ScrollView` with a
  `Pop<Image>`-shaped item below the fold (reusing the `ScrollView`
  demo's shape in `select_scroll.py`); showcase entry; rustdoc on
  `Repeat::bind` naming the window range as the in-repeater signal;
  handoff §6 (repeaters) line; `docs/competitive-feature-gaps.md` row.
  **Gates:**
  - `just widget-drive Pop`, both strategies: PageDown until the item
    is in view; the readout says loaded (the app's state, not a
    delivery count), damage 1.00× of the item's box; the demo's
    viewport height and the window's are printed, viewport < window;
  - `#[should_panic(expected = …)]` on `Repeat::new::<Row>` where `Row`
    contains a `Pop`, and on `List`; **accept:** the same `Row` without
    the `Pop` constructs.

  **Injections:** delete the `Repeat::new` assertion → the should-panic
  test fails by name; make `table_has_flag` always answer `false` → the
  should-panic tests and the unit test go red by name (green before,
  red under it); have `Pop` leave `WATCHES_VIEW` off its entry →
  the drive's readout never changes and the action fails its
  consequence. Firmware: `just firmware-delta <parent> firmware=0
  firmware_interactive=0 firmware_bind=0 firmware_cascade=0`, printed.
- **T85.5 — the capture-phase survey.** Its evidence goes in a new
  survey section of `docs/results-layout-input.md` (created by T85.0),
  with each candidate's `file:line` and verdict, and the handoff §12
  line. No code. K85e decides.
- **M85a closes** after T85.5, on its own: `just ci-verify <sha>`
  green, cited in AGENT.md's queue entry, and
  `docs/results-layout-input.md` records K85a, K85d (M85a's share) and
  K85e's numbers, fired or not (index §3.3, M12).

**Tasks — M85b.**

- **T85.6 — the culling gates that compile today.**
  - `benches/src/bin/scroll_cull.rs` (and its `bin scroll_cull` line
    removed from `scripts/doc-paths.allow`): a derived 1,000-row column in a
    400 px `ScrollView` in an 800 px window, painted into a
    `DisplayList` with `set_bounds(Some(surface))` as `Host::frame`
    does, and again with `None`. Prints paint µs and `list.cmds.len()`
    for both, eight runs a side under `just bench-lock`. Today the two
    command counts are equal (printed): **this is the setup check** —
    an arm that culls nothing shows it (B13).
  - `crates/lumen-static-shell/tests/cull_damage.rs`: a `List` (which
    culls today) inside a `ScrollView`, a label painted after it,
    `Host<_, Incremental, _>` under both strategies; ten wheel steps.
    Prints per step: damage px inside the scroller's rect, outside it,
    and the list's command count (setup landed: the count changes
    between steps, each wheel `handled`). Then the same with `bounds`
    forced `None`. **Pinned green today** (X1, §3.1: no `#[ignore]`):
    the gate asserts the current gap, damage outside the scroller with
    culling > without, printed. T85.7 flips it to the bound: with
    culling ≤ without.
  - `crates/lumen-static-shell/tests/cull_ink.rs`: a shadowed card
    whose rect is just below the surface while its `shadow_ink` is on
    it, an open `Select` whose anchor is off-surface, each printed
    (rect, ink, surface; `content_absent()` false). Render with bounds
    `Some` against `None`: byte-identical. Green today for derived
    containers (they do not cull); **the `List` variant pins today's
    gap green** (it culls by `rect_of`, so it asserts the two renders
    differ, differing pixels printed), and T85.8 flips it.
- **T85.7 — `Retention::scope_end`, and `Incremental` re-syncs.**
  *Same commit:* rustdoc on `Retention`, `Recording::pop_clip` and
  `Incremental`; `docs/damage-amplification-decision.md` gets the
  scope-end rule; handoff line.
  **Gates:** `cull_damage.rs` flipped and green (quantities
  printed); `tests/container_fill.rs` and every damage test unchanged;
  `just widget-drive ScrollView` 1.00× both ways.
  **Injections:** make `Incremental::scope_end` a no-op → `cull_damage`
  fires with the label's ink printed as damage; key by global pop
  ordinal instead of per-depth → a row with its own clip shifts every
  key and the same gate fires. **Accept:** a frame whose scopes all
  keep their counts reports the same damage as before, byte for byte
  (`benches --bin retain` rows unchanged within spread). Firmware:
  `just firmware-delta <parent>` (default `firmware_interactive=1000`),
  printed.
- **T85.8 — `CullTo` and derived-container culling, behind K85b.**
  `CullTo` (overriding every provided `Painter` method that exists at
  landing, `shader`/`shader_end` under `cfg(feature = "shader")` if
  M83 is in), `ops::cull_child`, `NodeDesc::INKS_OUTSIDE` on `Popup`
  and `Veil`, the derive's per-field `const` (through T85.4's
  `widget::table_has_flag`) and the paint-loop test;
  `List` moves to `cull_child` too. *Same commit:* rustdoc (including
  `clip_bounds`'s, which gains "a wrapper may narrow it");
  `docs/results-layout-input.md` K85b row; `results-competitive.md`
  gains the scrolled row beside the T16.12 one, apart from the
  cross-arm table as that one is; handoff §12 DECIDED.
  **Gates:**
  - `scroll_cull`: culled commands printed against the uncull count,
    and paint µs against the arm's spread (K85b);
  - `cull_ink.rs`'s `List` variant flipped: byte-identical, with rects
    printed;
  - `cull_damage.rs` stays green;
  - every golden byte-identical, and at least one golden has content
    past a scroller's viewport but inside the window, with the culled
    child count printed so the case is known to have culled (A22);
  - `just widget-drive ScrollView` and `Select` under both strategies;
  - `deferring_forwards_every_command` (`tests/overlay.rs:435`) gains a
    `CullTo` case: every call in, every command out, count printed;
  - if M83 has landed, `deferring_forwards_the_bracket`
    (`tests/shader_cmd.rs`, `--features shader`) gains a `CullTo`
    case; if M83 lands after T85.8, M83 adds it (A6, R4: whichever
    lands second).

  **Injections:** cull by `rect_of` → the card's shadow and the open
  menu vanish and `cull_ink` names them; let `CullTo` inherit `defer`
  → the open `Select` inside the scroller paints inline and
  `tests/overlay.rs` fires; let `CullTo` inherit `shader` (delete its
  override) → `PushClip` appears where `Shader` was and the bracket
  test fires by name; `CullTo::text_spaced` does nothing → the
  forwarding count drops and `deferring_forwards_every_command` fires;
  answer the surface instead of the viewport
  in `CullTo` → the command count printed by `scroll_cull` stays at the
  window's rows, and its "fewer than the window's rows" bound fires.
  Firmware: `just firmware-delta <parent>` (default
  `firmware_interactive=1000`), printed (expected 0 B).
- **M85b closes** after T85.8 (or after K85b/K85c stop it), on its own:
  `just ci-verify <sha>` green, cited in AGENT.md's queue entry, and
  `docs/results-layout-input.md` records K85b, K85c and K85d's numbers,
  fired or not (index §3.3, M12).

**Kill rules (M85).**

- **K85a — one layout per child.** `flexible.rs` prints a count above 1
  for any child, or the frame arm's `ALL rows: layout` row
  (`benches-competitive/run.sh -p lumen-static-frame`, under
  `just bench-lock`, eight runs a side) leaves its spread with
  `Flexible` unused. Fires → `Flexible` is not shipped; the count and
  the row go in `docs/results-layout-input.md`.
- **K85b — culling beats the arm.** `benches --bin scroll_cull`,
  culled median against the uncull arm's spread, eight runs a side.
  Not below the spread's minimum → T85.8 is not shipped; the number is
  recorded.
- **K85c — re-sync is free when nothing shifts.** `benches --bin
  retain` and the frame arm's `idle (floor)` and `+render (damage)`
  rows, eight runs a side. Outside spread → T85.7 is reverted, and
  culling is decided against **on X1's ground**, not only on paint
  time.
- **K85d — firmware.** M85's cumulative `.text` on
  `firmware_interactive` above 1,024 B (5.1% of its 19,998 B headroom
  under 80 KB, which M80–M90 share; it was 28% of the 3,614 B under
  64 KB, and the owner kept the cap when raising the ceiling, D5,
  2026-09-30) → the task stops and the owner re-budgets or defers
  `Flexible`'s derive branch. Measured as `just firmware-delta
  <T85.0 sha> firmware_interactive=1024` at each task; every task also
  prints the four binaries against its parent (`just firmware-delta
  <parent>`) and 81,920 B.
- **K85e — capture.** Fewer than two real cases in T85.5's survey →
  decided against, the survey is the evidence. **Reopen condition**
  (index §6, second review M27): a later shipped widget or
  `docs/widget-audit.md` row that is a real case by T85.5's definition
  and brings the count to two. Whether reached by T85.5 or by a
  reopen, two cases do not count as planned (index §1.2) until the
  build's plan has an M-number, a file and a §5 entry; T85.5's survey
  section records the count and this condition either way.

**Note for the assembler.** No change to S1/S2. `CullTo` is a fourth
`lumen-static` wrapper (`display.rs`): §2.2's implementor table and
its "three wrappers" sentence, and M83's override list, name it
(second review A6, P6, R4). No cross-milestone
obligation on M82: `NodeDesc::INKS_OUTSIDE` is for any widget whose ink
leaves its box, and M82's two path users (T82.5, T82.8) keep theirs
inside, so they do not set it. T85.8's `cull_ink.rs` should gain a
rotated `VectorText` case once both exist, asserting it is culled only
by its box. The plan's §1.2 culling row becomes "renderers and `List` cull
(T16.12); derived containers and `ScrollView` do not". §2's "Every task
that adds a `Session` method…" does not touch M85: none is added.

**What building found (M85).**

- **T85.1: scan 2 is one body run in two rounds, not two scans.** The
  Decisions' shape — 2a and 2b emitted per field, as scan 1 is — put a
  third inlined call of every child's `layout` in every derived row and
  column, and the embedded fixture's two rows cost **+1,616 B** of
  `firmware_interactive` `.text` (K4: 1,000). Two cheaper spellings did
  not help: a `for fills in [false, true]` loop over one body was unrolled
  back into two (+1,628 B, and +2,040 with the array `black_box`ed, since
  the trip count was still known), and a per-field body with `Style`
  borrowed instead of copied still cost +1,580 B. Looping over the child
  roots through `Node::node_mut` linked the whole `dyn Node` machinery
  into the paint-only firmware (+20,026 B). What fits is one body per
  field inside a `while` loop whose round count is `black_box(2)`, with
  every difference between the rounds decided out of line in
  `ops::Round::limits`, so the emitted body has no branch on the round
  for the optimiser to split it by: **+436 B** on `firmware_interactive`
  (`docs/results-layout-input.md` T85.1). The behaviour is the
  Decisions' exactly — 2a before 2b, the shares over `Flexible` and fill
  portions together, the fills' share computed as `free * k / portions`
  as it always was, one layout per child — so this is recorded and not
  re-decided.
- **T85.1: "run scan 2a after 2b" reads 300 for the fill, not 150.** The
  plan predicted the fill would read 150; run first, the fills divide all
  of `free` among their own portions, so `[Flexible 80, FILL]` reads
  80/300. The gate fires by name either way.
- **T85.2: the past-the-edge hit gate needs a negative offset.** A child
  is laid out inside the pin's box less its offset, so a positive offset
  can never put it past the pin's right or bottom edge; the gate pins its
  child at `(-30, 20)`, the one way a child leaves the box it is laid out
  in, and the pin's own `hit` is what keeps that part unpressable.
- **T85.3: the adopted application hears `true` after two pumps, not
  one.** The Decisions deliver a change on the pump *after* the `place`
  that found it, and an adopted application has been placed by nobody:
  its first pump places it and queues `true`, and the pump its deadline
  asks for delivers it. The gate reads 0 → 0 → 1 and bounds the count at
  2; "0 → 1 after one pump" contradicted the one-frame-later rule.
- **T85.3: the A11 gate's label has to be in a container of its own.**
  Written with the readout label as a bare leaf beside the scroller, the
  `ScrollEnd`-style marking injection left it green: the derive's skip is
  per container, a leaf is laid out whenever its parent is, and the root
  is dirty whenever the watched leaf below it is marked. The label now
  sits in a row of its own, which `Boundaries` skips, and the injection
  goes red with damage `None` (`docs/results-layout-input.md` T85.3).

- **T85.4: the live drive cannot see the load's frame on its own, so its
  1.00× is gated headlessly.** The window's input RPC pumps a frame the
  agent's journal does not record — the one whose `place` queues the
  change — and the redraw after it delivers and draws the load, so the
  journal's frame holds the scroll and the load together and the region
  over the view is the scroll's (`docs/results-layout-input.md` T85.4).
  The drive asserts the application's state (readout `loaded`) and the
  placeholder's text, and the harness bounds that combined frame against
  the placeholder's box and where it was (bound 4.0); the load's own
  frame — inside the pop's box, at most 1.00× of it — is
  `crates/lumen-static-widgets/tests/pop.rs`, where the host pumps the
  delivering frame by itself.
- **T85.4: the drive's inventory node is the `Pop`, not the demo's
  `ScrollView`.** `roles.py`'s row (`GenericContainer`) is checked
  against the node the inventory names; naming the scroller read
  `ScrollView` and failed by name. The demo is still the `ScrollView`
  shape the plan names, and the placeholder sits just below its fold
  (four rows) so one PageDown brings it in and the harness's bound on the
  combined frame has margin.
- **T85.4: the refusal covers `List::default` and `Dyn::new` too**, and
  is one `#[track_caller]` helper beside `table_has_flag`
  (`widget::refuse_watched_in_arena`) so the three sentences cannot
  drift; `Repeat::default` reaches it through `Repeat::new`.

- **T85.6: a `ScrollView` holds at most 255 nodes, so `scroll_cull`'s
  column is 220 rows, not 1,000.** Its content is spliced into its own
  table of `MAX_SCROLL_NODES` (256) entries, and a derived column of a
  thousand leaves is a const-evaluation error inside one. Twenty-two
  derived columns of ten `Fixed` rows (243 nodes) is the largest round
  column that fits; the window holds 34 of its rows and the viewport 17,
  which is the shape K85b judges (`docs/results-layout-input.md` T85.6).
- **T85.6: X1's mechanism is `Streaming`'s, not `Incremental`'s.** Since
  M26/T26.11 `Incremental` compares two frames of different lengths by
  their longest common prefix and suffix, not slot *n* with slot *n*, so
  a shifted count alone charges nothing after the culled range: the
  unchanged tail is the common suffix. The damage X1 names appears when
  something *after* the range also changes, which makes everything
  between the two the middle — so `cull_damage.rs` writes a readout after
  its label every step, and the label is the quantity: **1,792 px** on
  each step whose count changed, **0** with culling off. The outside
  total moves too (416,640 against 407,680 px) and is asserted as the
  plan writes it, but it is not what an injection reliably moves: under
  a slot-from-the-end comparison it *rose* while the label went to 0.
- **T85.6: the `None` arm is a mirror of the host's frame.** `Host::frame`
  always sets the surface as the bound and has no switch, so after each
  pump the gate repaints the host's laid-out tree through
  `Scaled(Recording(list))` into its own `Incremental`, once with `None`
  and once with the surface, and asserts the surface twin's damage and
  count equal the host's (banked with `take_damage_for_age(1)`, since one
  pump may paint twice) at every step. Without `Scaled` the twin differed
  from the host by the scrollbar thumb, which `Scaled` snaps even at 1.0.
- **T85.6: the damage outside a scroller is dominated by clipped-out
  ink.** A command's ink is its own rect, not its rect within the clip
  stack, so rows painted below a 200 px viewport (inside the window,
  clipped away) are damage on every scroll, and so is the `List`'s own
  `PushClip`, whose rect is the list's whole 5,280 px. Recorded, not
  changed: it is the same with culling and without.
- **T85.6: a `Popup` inside a `List` draws nothing, culled or not — a
  defect, recorded and not fixed here.** A popup defers its own index,
  and inside an arena that index is the item's, not the main table's;
  the overlay pass repaints the main-table node at that index
  (`pass::paint_overlaid`), so a `Select` in a `List` re-paints whatever
  main-table node shares the number — here the list's parent — and its
  menu lands inside the list's own clip, off the surface. So
  `cull_ink.rs` has no `List` variant of the menu case: both renders
  would agree for the wrong reason. T85.8's `INKS_OUTSIDE` on `Popup`
  cannot be gated inside a `List` until an arena-local deferral is
  translated to something phase 2 can repaint. Also seen while building
  it: a `Kind` rule reaches a `Repeat` inside a `List` item but not the
  `Repeat`'s rows, which is why the gate styles the menu panel and not
  its rows.

**What the review found (M85).**

- C19 → adopted: `List` culls already (T16.12, cited); `ScrollView`
  narrows through `CullTo`; the "+20 µs" figure dropped. `CullTo`'s
  forward list now covers M80/M82's required and M83's cfg-gated
  provided methods (second review A6, P6, R4).
- G19 → changed: no second pass at all — leftover goes only to fills,
  one layout per child; the redistribution limit is stated and gated.
- G20 → adopted: host-side pass after `place`, against every
  `clips_children()` ancestor; T85.0 asserts viewport < window and the
  inside-window-outside-viewport step; both strategies.
- G28 → adopted: every M85 task names a mechanism-breaking injection.
- G29 → adopted: `Flexible`, `Pin`, `Pop`, `ScrollView`/`Select`
  culling each have a live `just widget-drive` action under both
  strategies; T85.5 has no behaviour.
- B13 → adopted: `scroll_cull` sets `bounds` as `Host` does and prints
  commands beside time; `cascade_deep` dropped.
- B14 → adopted: T85.0 builds the count (test leaf, per child, baseline
  1) and shows it red by a doubled scan-2 call.
- A21 → adopted: `WATCHES_VIEW` flag, sentinel-handler path, deadline
  0 for the follow-up frame, one-frame paint gate; the gate asserts the
  deadline itself and models the loop (second review G17).
- A22 → adopted: viewport ∩ surface; a golden past the viewport inside
  the window, culled count printed.
- A24 → adopted: `Pin`/`Pop` ALLOW lines (wrappers), `roles.py`
  rows, and `a11y_roles.rs` rows with catalogue fields (second review
  M3); the other widgets are other milestones'.
- P6 → adopted: host pass like `ScrollEnd`; the gate asserts the app's
  counter, not a delivery count. Marking is `mark_handled`'s, not
  `ScrollEnd`'s, because a handler runs (second review A11).
- P7 → adopted: as C19/A22.
- P8 → changed: `on_capture` is required if built; the build moves to
  its own plan because it touches every `Node` impl.
- P30 → adopted: T85.1's injections listed; appendix row replaced.
- M3 → adopted: no trailing docs task; each task carries its docs.
- M4 → adopted: `Pin` and `Pop` live in `lumen-static-widgets`; each
  export carries inventory, ALLOW, role and showcase in its commit.
- M20 → adopted: T85.1, T85.2, T85.4, T85.7, T85.8 have injections.
- M24 → changed: M61 is on this tree, so `.lss` `flexible(n)` is
  unconditional and the four sites are named.
- X1 → adopted: `Retention::scope_end` with `Incremental` re-sync
  (T85.7), its gate pinned green on today's `List` gap and flipped by
  T85.7, and K85c decides culling on it.
- X2 → adopted: cull by ink (shadow, focus ring), never a subtree with
  `INKS_OUTSIDE` (`Popup`, `Veil`, any widget whose ink leaves its box;
  M82's path users keep theirs inside and do not need it);
  `cull_ink.rs`.
- X4 → changed: `Pop` in an arena is refused at construction; the
  repeater's window range is the documented signal. The refusal's
  `table_has_flag` is a new const fn, added by T85.4 (second review
  C15, P7).

**What the second review found (M85).**

- C15 → adopted: `widget::table_has_flag` is a new `pub const fn`
  in T85.4 beside `table_is_well_formed`, with rustdoc and a const
  unit test; T85.8 reuses it.
- G17 → adopted (verifier's fix): T85.3 asserts
  `next_deadline_ms() == Some(0.0)` while `view_pending` is non-empty
  and models the loop, pumping only when the deadline is due.
- G18 → adopted: `true` must arrive at the first step whose rect
  meets the viewport, printed beside T85.0's step.
- G35 → adopted: `just widget-drive Layout`, the entry T85.1 adds,
  with its row count printed.
- A6 → adopted: `CullTo` forwards `shader`/`shader_end` under
  `cfg(feature = "shader")`; the bracket test gains a `CullTo` case
  in whichever of M83/M85 lands second; injection "inherit `shader`".
  §2.2 and M83 changes requested of their files.
- A11 → adopted: a visibility delivery that ran a handler is marked by
  `mark_handled`; an unsplit `Boundaries<256>` gate writes a sibling
  label, with a `ScrollEnd`-style marking injection.
- P6 → adopted: as A6, plus `text_spaced` (and M82's `path`/`fills`)
  named in the forward list; `deferring_forwards_every_command` gains
  a `CullTo` case.
- P7 → adopted: as C15, with the "always `false`" injection in T85.4.
- M12 → adopted: "M85a closes" after T85.5 and "M85b closes" after
  T85.8, each on `just ci-verify <sha>` cited in AGENT.md with every
  kill rule's number.
- M15 → adopted: `docs/results-layout-input.md` is created by T85.0
  with the firmware baseline and K85a–K85e table; T85.5 adds the
  survey section.
- R4 → adopted: as A6; the Note for the assembler names `CullTo` as
  the fourth wrapper.
- R16 → adopted: `Style`/`NodeState` asserted equal to the parent
  commit's sizes and printed, no literals.
- X8 → adopted: `reasked` clears `view_seen` and `view_pending`; T85.3
  gates an adopt with the `Pop` in view (0 → 1), injection "leave
  `view_seen` untouched".
- D6 → adopted: the `Flexible` derive branch's `.text` was attributed
  to D3; it is now printed against K4 and D5 (not a `Painter` method,
  so D3/D6 do not cover it). M85 adds no `Painter` method.
- D7, X5 → no change: M85 touches neither xclip nor `deny.toml`.
- M3 (from plan-layout-input.md) → adopted: T85.2 and T85.4 add a
  `catalogue/mod.rs` field, an `a11y_roles.rs` `ROWS` row and a
  `GENERIC_BOUND` entry for `Pin` and `Pop` (`GENERIC_BOUND` added
  beyond the request: a `GenericContainer` row outside it fails,
  `a11y_roles.rs:245`).
- M27 (from plan-layout-input.md) → adopted: K85e names the reopen
  condition for the unplanned capture build; the header points to it.
- consistency-3 item 7 → each task names its comparator: `just
  firmware-delta <parent>` (default `firmware_interactive=1000`), all
  four bounds 0 for T85.2 (`Pin`) and T85.4; K85d is `just
  firmware-delta <T85.0 sha> firmware_interactive=1024`; T85.7 gains a
  firmware line.
- consistency-3 item 24 → T85.0 removes the results doc's and T85.6
  the `bin scroll_cull` `doc-paths.allow` entry.
