# Layout and input: `Flexible`, `pin`, `pop`, culling, capture

M85's results (`docs/plan-layout-input.md`). Every figure an M85 gate
prints is recorded here once and cited from the rustdoc, the plan and
the commits. The plan-wide budgets and kill rules K1–K4 are
`docs/plan-iced-roadmap.md` §2 and §4; the firmware's four-binary table
is `docs/results-embedded.md`'s.

## Thresholds, written before anything is built (T85.0, 2026-10-06)

M85's kill rules and the bounds its tasks state, copied from the plan so
each later row has its threshold beside it. A row's verdict is filled in
by the task that measures it, never edited here.

| rule | quantity | threshold | measured by | if it fires |
|---|---|---|---|---|
| K85a | layout calls per child in `crates/lumen-static/tests/flexible.rs`, every case; the frame arm's `ALL rows: layout` row (`R=1 benches-competitive/run.sh -p lumen-static-frame`, `just bench-lock`, eight runs a side) with `Flexible` unused | **1** per child; inside the parent's eight-run spread | T85.0 (the count), T85.1 | `Flexible` is not shipped; the count and the row are recorded here |
| K85b | `benches --bin scroll_cull`, the culled median against the uncull arm's eight-run spread | below the spread's minimum | T85.8 | T85.8 is not shipped; the number is recorded |
| K85c | `benches --bin retain` and the frame arm's `idle (floor)` and `+render (damage)` rows, eight runs a side | inside each row's spread | T85.7 | T85.7 is reverted, and culling is decided against on X1's ground |
| K85d | M85's cumulative `firmware_interactive` `.text`, `just firmware-delta <T85.0 sha> firmware_interactive=1024` at each task | ≤ **1,024 B** (5.1% of the 19,998 B headroom under 80 KB at the M56 merge) | T85.1–T85.4, T85.7, T85.8 | the task stops; the owner re-budgets or defers `Flexible`'s derive branch |
| K85e | real capture-phase cases in T85.5's survey (shipped code or a `docs/widget-audit.md` row whose workaround exists *because* events only bubble) | ≥ **2** opens its own plan | T85.5 | fewer than two: decided against, the survey is the evidence |
| K4 | one task's `firmware_interactive` `.text` Δ, `just firmware-delta <parent>` | ≤ **1,000 B** (T85.2 and T85.4: Δ **0** on all four); every binary ≤ 81,920 B | every task | the task stops and attributes the delta by a symbol diff |
| K1 | `size_of::<Length>()`; `Style` / `NodeState` | **8 B**; equal to the parent commit's (84 / 84 B at `2fcb000`, 16.4 KB per 100 nodes against 24 KB) | T85.0, T85.1 | the variant is packed or the owner re-budgets |

## T85.0 — the gates that compile today, at `2fcb000` (2026-10-06)

Gates that compile today and hold today's behaviour green; each names
the injection it was watched red under.

| gate | prints today | injection (red) | used by |
|---|---|---|---|
| `crates/lumen-static/tests/flexible.rs`: a derived 300 px row `[CountingLeaf(Shrink, 80 px), CountingLeaf(FILL)]`, laid out once | row **300** px; layouts **1, 1**; widths **80 / 220** | scan 2's `d.layout` emitted twice in the derive: the fill reads **2** layouts, fails by name | T85.1 adds the `Flexible` cases |
| the same file, accept: three `Shrink` children | layouts **1, 1, 1** (widths 40 / 50 / 60) — unchanged under the injection, which never reaches a row with no fill | — | — |
| `crates/lumen-static/tests/cursor.rs` `a_length_is_eight_bytes` | `Length` **8 B**, `Style` 84 B | `FillPortion(u64)`: `Length` **16 B**, `Style` 104 B, fails by name | T85.1 |
| `crates/lumen-static-shell/tests/pop_visibility.rs`, setup half, under `Everything` and `Boundaries<256>` | viewport **100** px in a **300** px window; 15 wheel steps of 20 px, each handled, offset 20 → 300; the target enters the window at step **3** (y 280..300) and the viewport at step **13** (y 80..100); identical under both strategies | a viewport as tall as the window: first in the window and in the viewport both step 3, "inside the window and outside the viewport" **none**, fails by name | T85.3's arrival-step gate |
| firmware, `just firmware-delta 2fcb000 firmware=0 firmware_interactive=0 firmware_bind=0 firmware_cascade=0` | Δ **0** on all four | — | — |

**M85's firmware baseline** (`just firmware`, `scripts/check_text.sh`,
at this commit; tests and docs only, so equal to `2fcb000`). K85d's
cumulative check compares every later M85 task with this commit.

| binary | `.text` | of 81,920 | headroom | `.rodata` |
|---|---:|---:|---:|---:|
| `firmware` | 30,606 | 37.4% | 51,314 | 13,344 |
| `firmware_interactive` | **63,480** | **77.5%** | **18,440** | 21,886 |
| `firmware_bind` | 34,014 | 41.5% | 47,906 | 13,948 |
| `firmware_cascade` | 32,694 | 39.9% | 49,226 | 14,132 |

## T85.1 — `Length::Flexible` (2026-10-06)

The variant, its two `.lss` spellings, and the derive's scan 2 as one
body run in two rounds (`ops::Round`; why not two bodies is
`docs/plan-layout-input.md`, "What building found").

| gate | prints | injection (red) |
|---|---|---|
| `flexible.rs`, `[Flexible(1) 80 px, FILL]` in 300 px | row **300**; layouts **1, 1**; **80 / 220** | Flexible treated as `FillPortion` in scan 2 (counted with the fills, accepted by the fills' round): **150 / 150**, fails by name |
| `flexible.rs`, `[Flexible(1) 400 px, FILL]` | layouts **1, 1**; **150 / 150** | the fills' round run first: the fill reads **300** (80 / 300 in the case above), fails by name |
| `flexible.rs`, `[Flexible 80, Flexible 400]` — the stated limit | layouts **1, 1**; **80 / 150** | under the first injection: **150 / 150**, fails by name |
| layout count, every case above (K85a) | **1** for every child | a `Flexible` child laid out a second time under its measured size: **2** in all three cases, fails by name |
| `flexible.rs` `flexible_moves_no_struct` | `Length` **8 B**; `Style` **84**, `NodeState` **84** (parent 84 / 84); **16.4 KB** per 100 nodes (parent 16.4 KB, bound 24 KB) | `Flexible(u64)`: `Length` **16 B**, `Style` 104, 18.4 KB, fails by name |
| `lumen-static-lss` `every_property_lowers_to_its_setter` | `width: flexible` → `Flexible(1)`, `height: flexible(2)` → `Flexible(2)` | the keyword lowered as `FillPortion(1)`: fails by name. Deleting the shell's or the macro's `Flexible` arm is a compile error (both matches are exhaustive) |
| live, `scripts/widget_drive.sh Layout` (Everything, Boundaries, Boundaries+oracle, scale 2, both `+shaping` legs) | row **300.0** px on **1** line; flexible **80.0** px (bound min(80, 150.0) = 80.0), fill **220.0** px (bound 220.0); D5 parity 1 of 1 byte-identical; oracle 596 frames, none diverged | the first injection: flexible **150.0**, fill 150.0, FAIL by name on every leg |
| firmware, `just firmware-delta a21135b` (K4) and `just firmware-delta a21135b firmware_interactive=1024` (K85d) | `firmware` **+484**, `firmware_interactive` **+436**, `firmware_bind` **+436**, `firmware_cascade` **+484**; `.rodata` +0; headroom **18,004 B** | — |

**The spellings that did not fit K4**, each `just firmware-delta a21135b
print` on `firmware_interactive`: 2a and 2b emitted per field **+1,616**;
a two-round `for` over one body **+1,628** (unrolled), **+2,040** with the
array `black_box`ed; the per-field 2a borrowing `Style` **+1,580**; a loop
over the child roots through `node_mut` **+1,396** here and **+20,026** on
the paint-only `firmware`, which never linked the `dyn Node` machinery;
the one body with its round's choices as data **+756**; and with them out
of line in `Round::limits`, what shipped, **+436**.

**K85a** — the frame arm, `a21135b` against this tree, eight interleaved
invocations a side under one `just bench-lock` (quiet; load1 2.22 before,
2.18 after, bound 8), `Flexible` unused:

| row | base median (spread) | T85.1 median | verdict |
|---|---:|---:|---|
| `ALL rows: layout` (K85a's row) | 1,159.70 µs (1,105.2–1,169.3) | 1,161.30 | inside |
| `idle (floor)` | 59.40 µs (53.2–62.5) | 59.05 | inside |
| `ALL rows: +paint` | 1,283.60 µs (1,220.2–1,291.8) | 1,285.15 | inside |
| `layout` | 20.20 µs (20.1–20.5) | 20.25 | inside |
| `+paint` | 40.95 µs (40.6–41.3) | 41.25 | inside |
| `+render (damage)` | 77.70 µs (77.3–77.9) | 77.10 | below the spread (faster) |

K85a holds: one layout per child in every case, and its row inside its
spread. `+render (damage)` reads 0.6 µs under its base's minimum; the
change touches layout only (no paint or render code moved), so it is
recorded and not attributed. K1 holds (`Length` 8 B, `Style` and
`NodeState` unchanged). K4 and K85d hold (436 of 1,000; 436 of M85's
cumulative 1,024).

## T85.2 — `Pin<W>` (2026-10-06)

One child at an offset inside the pin's own box, in flow, clipped to
the pin in `paint` and in `hit` (`crates/lumen-static-widgets/src/surface.rs`).
Gates in `crates/lumen-static-widgets/tests/pin.rs` and live.

| gate | prints | injection (red) |
|---|---|---|
| `the_child_sits_at_the_pins_position_plus_its_offset` | pin at (50, 50) 200×150; child at (**80, 90**) 60×30, offset (30, 40); a second pin's child at its position + (−30, 20) | `Pin::place` ignores the offset: child at (50, 50), fails by name |
| `a_press_at_the_childs_centre_lands_on_the_child` | press at the pin + offset + half the child, (110, 105): release handled, hit **node 2** (the child; the pin is 1), clicks 0 → **1** | under the same injection: release not handled, hit `None`, clicks 0, fails by name |
| `a_press_inside_the_pin_but_off_the_child_is_not_the_childs` | press inside the pin at (200, 170): hit `None`, clicks **0** | — (the accept half of the gate below) |
| `a_child_past_the_pins_edge_is_not_pressable_there` | pin x 50..150, child x 20..80 (offset −30); press at x 35: hit `None`, clicks **0**; accept: at x 60, hit node 4, clicks **1** | drop the `hit` gate on the pin's rect: hit **node 4**, clicks 1, "a press past the pin's edge reached its child" |
| `the_pin_clips_its_child_to_its_rect_in_paint` | `PushClip` at the pin's rect: **1** (bound 1) | drop the `push_clip`/`pop_clip` bracket: **0**, fails by name |
| live, `scripts/widget_drive.sh Pin` (Everything, Boundaries, Boundaries+oracle, scale 2, both `+shaping` legs) | button at (**112.0, 64.0**) from the pin's origin (bound (112, 64)); a click: presses 0 → 1, the damage over the pin **3,690 px²** against the button's 3,690 px² = **1.00×** (3,604 / 3,604 at scale 2); role `GenericContainer`; Enter reaches it in 1 Tab; D5 parity byte-identical; oracle 549 frames, none diverged | — (the unit gates' injections) |
| firmware, `just firmware-delta 0307c4c firmware=0 firmware_interactive=0 firmware_bind=0 firmware_cascade=0` | **+0** on all four `.text` and `.rodata` (`firmware_interactive` 63,916 B) | — |

`a11y_roles.rs`'s `every_node_impl_has_a_row` fails by name ("Node impls
with no row in the table: [\"Pin\"]") with the `Pin` row removed, and
`scripts/check_observe.sh` with its ALLOW line removed. K1 holds: two
`f32` in the application's struct, no `Style` or `NodeState` byte.

## T85.3 — the visibility pass (2026-10-06)

`NodeDesc::WATCHES_VIEW`, `Event::Visibility`, `NodeSel::visibility`,
`pass::in_view`, `pass::tell_visibility`, and `Host`'s `view_watch`,
`view_seen` and `view_pending`. Gated with a test-only flagged leaf in
`crates/lumen-static-shell/tests/pop_visibility.rs` (the T85.0 setup:
100 px viewport, 300 px window, 20 px wheel steps), both strategies
unless named.

| gate | prints | injection (red) |
|---|---|---|
| in and out | wheel down 15 steps, up 15, each handled; `true` **1**, `false` **1** (bound 1 / 1); the leaf heard `[true, false]`; counter 0 → **1** | — |
| arrival step (G18) | first inside the window and outside the viewport **3**; first in the viewport **13**; `true` at **13**. On the way up, first out of the viewport **3**, `false` at **3** | intersect with the window only (skip the clipping ancestors in `pass::in_view`): `true` at **3**, "`true` did not arrive at the first step whose rect meets the viewport (inside-window-outside-viewport step Some(3))" |
| deadline (G17) | each step with a change pending: deadline `Some(0.0)`; otherwise the wheel's scroll-end timer, `Some(150.0)` | drop `next_deadline_with`'s term: "step 13: 1 change(s) pending and the deadline is Some(150.0)" |
| one frame, modelled loop (pumps only while due) | frames from placement to the write: **1** (bound 1), readout `"seen 1"` | the same injection with the deadline assertion skipped: **0** frames in the modelled loop, the write a wheel later (step 14), fails by name |
| still | 50 still frames delivered **0** (bound 0) | — |
| a click on the leaf | hit node **4** (the leaf), handled, clicks **1**, visibility handler runs 1 → **1** (bound unchanged) | a `Handlers` impl that ignores `sel`: runs 1 → **2**, "a click ran the visibility handler" |
| unsplit, `Boundaries<256>` vs `Everything` (A11) | at step 13 the readout reads `"seen 1"` in both; the frame hashes equal at every step (Everything's moved at 13); damage two regions, the label `[0, 100, 48, 116]` and the label after it | mark the delivery as `ScrollEnd` does (consumed subtrees only): Boundaries' hash unchanged, damage `None`, "step 13: the label written by the visibility handler is stale under Boundaries" |
| split application (accept) | the delivering frame marked `Marks { all: false, ranges: 1 }`; `true` 1 | — |
| `Host::adopt` with the leaf in view (X8) | the fresh application's count 0 → 0 after one pump → **1** after **2** (the pump that places it, then the one its deadline asks for; bound 2) | leave `view_seen` untouched in `reasked`: 0 after 1, "the adopted application never heard `true` from a leaf in view" |
| `size_of::<Event>()` (K1) | **16** (bound 16) | — |
| a tree with no flagged node (accept) | `watched_views()` **0** (bound 0); every golden byte-identical (`just test`) | — |
| firmware, `just firmware-delta ad18045` (K4) and against `a21135b` (K85d) | `firmware_interactive` **−4** (`pass::dispatch` −6, its `Event` match), the other three +0, `.rodata` +0; K85d cumulative **+432** of 1,024 | — |

K1 holds: `Event` 16 B, no `Style` or `NodeState` byte; the host's heap
is 4 B per watched node plus a `bool`. K4 and K85d hold.

## T85.4 — `Pop<S, W>`, and the arena refusal (2026-10-06)

One child whose own table entry carries `NodeDesc::WATCHES_VIEW`, with
`visible()` recorded from `Event::Visibility` and an `on_view` handler
answered only for `NodeSel::visibility`
(`crates/lumen-static-widgets/src/surface.rs`); `widget::table_has_flag`,
a `const fn`, and the refusal in `Repeat::new`, `List::new`/`Default` and
`Dyn::new`. Gates in `crates/lumen-static-widgets/tests/pop.rs`,
`crates/lumen-static/tests/table_invariants.rs` and live.

| gate | prints | injection (red) |
|---|---|---|
| `table_has_flag_finds_a_flag_on_a_nested_entry` (const-evaluated) | nested **true** (bound true); plain **false** (bound false) | `table_has_flag` always answers `false`: nested false, "table_has_flag missed WATCHES_VIEW on the deepest entry" |
| `a_repeat_of_a_row_holding_a_pop_is_refused`, `a_list_…`, `a_default_list_…`, `a_dyn_island_…` (`should_panic`) | each panics with "a Repeat row / List item / Dyn island cannot hold a node that watches its visibility …", naming the window range (or "put the `Pop` around it") | delete the `Repeat::new` call: the Repeat test "did not panic as expected", the other three green; `table_has_flag` always `false`: all four "did not panic as expected" |
| `the_same_row_without_a_pop_constructs_in_every_arena` (accept) | Repeat count **10**, List len **2**, Dyn nodes **3** | — |
| `a_pop_scrolled_into_view_runs_on_view_once`, `Everything` and `Boundaries<256>` | the pop is node 4, flagged; host watches **1** (bound 1); in the viewport at wheel step **13**, deadline `Some(0.0)`, loads **0** before the delivering pump (bound 0) and **1** after (bound 1), `visible()` true; the delivering frame's damage over the view `[14, 89, 84, 105]`, inside the pop, **0.39×** its 2,856 px² (bound 1.00); a click on its content: handled, presses **1**, loads still 1; wheeled back out at offset 220, hides **1**, `visible()` false | `Pop` leaves `WATCHES_VIEW` off its entry: watches **0**, "the host does not watch the Pop"; a visibility delivery that resets retention (`self.retain.reset()` after `tell_visibility`): damage the whole window, **31.51×**, "the load's frame damaged 31.51x the pop's box over the view" |
| live, `scripts/widget_drive.sh Pop` (Everything, Boundaries, Boundaries+oracle, scale 2, both `+shaping` legs) | viewport **180** px in a **760** px window (bound viewport < window), the placeholder at y 531..651 below the view's 339..519, readout `waiting`; one PageDown: readout **`loaded`**, the placeholder at 399..519 reads `loaded`; the journal frame holding scroll and load **69,520 px²** over the view against the placeholder's 31,200 px² = **2.23×** (harness bound 4.0); role `GenericContainer`; PageDown from `#d_pop_first` (1 Tab) moves the readout; D5 parity byte-identical; oracle 566 frames, none diverged | `Pop` leaves `WATCHES_VIEW` off its entry: readout **`waiting`** after 5 PageDowns with the placeholder in the view and its text unchanged, FAIL by name on all six legs; the keyboard pass `waiting` → `waiting`, FAIL |
| firmware, `just firmware-delta 3c2dc3e firmware=0 firmware_interactive=0 firmware_bind=0 firmware_cascade=0` | **+0** on all four `.text` and `.rodata` (`firmware_interactive` 63,912 B); K85d cumulative **+432** of 1,024 against `a21135b` | — |

**Why the live frame is 2.23× and not 1.00×.** The window's input RPC
pumps a frame the agent's journal does not record — the one whose
`place` queues the change — and the redraw after it delivers and draws
the load, so the scroll and the load are one journal frame and the
region over the view is mostly the scroll's. The load's own frame is the
headless row above (0.39×, inside the pop); `docs/plan-layout-input.md`,
"What building found", T85.4.

`a11y_roles.rs`'s `every_node_impl_has_a_row` fails by name ("Node impls
with no row in the table: [\"Pop\"]") with the `Pop` row removed, and
`scripts/check_observe.sh` ("no observe: …surface.rs: Pop") with its
ALLOW line removed. K1 holds: `Event` 16 B, no `Style` or `NodeState`
byte; a `Pop` is a `fn` pointer and a `bool` in the application's
struct.

## T85.5 — the capture-phase survey (2026-10-06)

Evidence only; no code. The question (`docs/plan-layout-input.md`,
Decisions, "The capture phase is a survey"): does any shipped widget, or
any `docs/widget-audit.md` row, carry a workaround that exists
**because** events only bubble — one a capture phase (a parent seeing a
pointer event before the child it is aimed at) would remove? The bubble
model being read: a press is offered to the node it hit and then to each
ancestor until one consumes it; consuming a `PointerDown` captures the
pointer, after which every move and the release go to the captor and
**a captured release activates nothing**; a `PointerMove` reaches a
widget only under capture (`crates/lumen-static/src/event.rs:198`, the
`Event::Hover` rustdoc). Searched by `grep` for `target_is_control`,
`cx.target == cx.node` / `!= cx.node`, "bubbl", "steal", "intercept" over
`crates/*/src`, and the audit's rows.

| candidate | where | what the code does | verdict |
|---|---|---|---|
| `PullToRefresh` takes a drag its content declined, but not a control's | `crates/lumen-static-widgets/src/nav.rs:1010`; `EventCtx::target_is_control`, `crates/lumen-static/src/event.rs:441–449`; audit row 40 (B40, "drag does nothing", `docs/widget-audit.md:131`) | a press on a label or on its own surface starts a pull; a press on a focusable node is left alone, because consuming it would capture the pointer and the control's click would never activate. So a drag that **starts on a button never pulls** | **real case (1).** The flag is the workaround, and it exists because a parent cannot watch a gesture it has not taken: with no phase before the child, the only choices are to take every press (and break controls) or to forgo drags from controls. A capture phase with a threshold would let it watch and claim late |
| `ScrollView` steals a drag from a child after a threshold | `crates/lumen-static-widgets/src/scroll.rs:415–447` | the wheel and the keys (PageUp/PageDown/Home/End) bubble from the focused or hovered child and are declined at an end, which is scroll chaining; a press is taken only on the bar's track. **There is no content drag to pan at all** | **refused**: no shipped code steals or wants to; the missing feature is touch panning, not a workaround. It becomes a case if content panning ships and needs one |
| a press on a tab is consumed "or a scroller above would take it as a drag" | `crates/lumen-static-widgets/src/nav.rs:400–405` | `Strip` consumes a press on the selected tab, to keep it from bubbling to an ancestor that acts on declined presses | **refused**: the bubble phase working as designed — the child decides first and says so. A capture phase would make the parent decide first, which is the opposite of what this needs |
| containers acting only on their **own** chrome (`target == node`) | `Veil` `crates/lumen-static-widgets/src/veil.rs:363`; `SplitPane`/`PaneGrid` `split.rs:286, 1091`; `Collapsible` `split.rs:633, 636`; `Picker` `pick.rs:1036`; `Chooser` `choose.rs:911, 929, 946`; `DataGrid` `data.rs:1438`; `Video` `video.rs:600`; `RangeSlider` `field.rs:655`; `SearchField` `field.rs:983, 987`; audit X2 (`docs/widget-audit.md:54`) | a press a child declined reaches its container with `target` still naming the child, and the container ignores it (a click on a modal's panel no longer dismisses it, a click in a pane no longer moves the divider) | **refused**: these guards exist because events bubble, not because they *only* bubble. A capture phase adds a phase before the child and leaves the bubble phase, and these guards, exactly as they are |
| `Tooltip` sees its content's presses and focus without consuming them | `crates/lumen-static-widgets/src/surface.rs:1105–1109` | hover intent, focus and presses bubble up from the content, and the tooltip reacts while declining, so the content keeps its press | **refused**: works under bubbling today, because a press its content declines still reaches it. It would need a capture phase only for content that consumes `PointerDown`, and no shipped tooltip content does |
| a hovered row in `Tree`/`OptionList` | `crates/lumen-static/src/event.rs:195–216` (M27/T27.11) | a move reached a widget only under capture, so a row-painting widget could not learn the hovered row; fixed by synthesising `Event::Hover`, which bubbles | **refused**: closed in the bubble model by a second event, not by a phase; no workaround remains |

**Count: 1 real case** (`PullToRefresh`), against K85e's threshold of
**2**. **K85e: fewer than two — the capture phase is decided against, and
this survey is the evidence.** Nothing is built and no plan is opened.
So the wrapper list a build would have needed (the off-table holders
that must forward `Node::on_capture`, `NumberInput` and `Markdown` at
least, per `Node::probe`'s rustdoc) is not drawn up here.

**Reopen condition** (`docs/plan-iced-roadmap.md` §6, second review
M27): a later shipped widget or `docs/widget-audit.md` row that is a real
case by this section's definition — a workaround that exists because a
parent cannot see a pointer event before the child it is aimed at — and
brings the count to two. The likeliest is content panning in
`ScrollView` (touch), which would steal a drag from a child after a
threshold, `PullToRefresh`'s shape. Even then two cases do not count as
planned (index §1.2) until the build's plan has an M-number, a file and
a §5 entry, and that plan makes `Node::on_capture` **required** (P8).

## M85a closes (2026-10-06)

T85.0–T85.5 are in. M85a's kill rules, each with its number, fired or
not (index §3.3, M12):

| rule | threshold | measured | verdict |
|---|---|---|---|
| K85a — one layout per child | 1 per child in every `flexible.rs` case; `ALL rows: layout` inside its parent's eight-run spread with `Flexible` unused | **1** for every child in every case; `ALL rows: layout` **1,161.30 µs** against 1,105.2–1,169.3 (T85.1) | **held** — `Flexible` shipped |
| K85d — firmware, M85a's share | M85's cumulative `firmware_interactive` `.text` ≤ **1,024 B** against `a21135b` | T85.1 **+436**, T85.2 **+0**, T85.3 **−4**, T85.4 **+0**, T85.5 **+0** (docs only): cumulative **+432** of 1,024 (`firmware_interactive` 63,912 B of 81,920, headroom 18,008 B) | **held**; 592 B of the cap left for M85b |
| K85e — capture | ≥ 2 real cases opens its own plan | **1** (`PullToRefresh`) | **fired as written: decided against**, the survey above is the evidence, reopen condition stated |
| K4 (per task) | ≤ 1,000 B; T85.2 and T85.4 Δ 0 on all four | largest **+436** (T85.1); T85.2, T85.4 **0** on all four | held |
| K1 | `Length` 8 B; `Event` 16 B; `Style`/`NodeState` unchanged | 8 / 16 / 84 / 84 B | held |

The closing `just ci-verify <sha>` and its citation in AGENT.md's queue
entry are the coordinator's (index §3.3).

## T85.6 — the culling gates that compile today, at `f5fedd9` (2026-10-06)

M85b's first task: gates that compile today and hold today's behaviour
green, each watched red under a perturbation of what it pins. Where the
plan's shape could not be built as written, the reason is in
`docs/plan-layout-input.md`, "What building found", T85.6.

| gate | prints today | injection (red) | flipped by |
|---|---|---|---|
| `benches --bin scroll_cull` (`just bench-lock`, quiet, load1 0.76, bound 8): a derived **220**-row column (243 nodes, `MAX_SCROLL_NODES` 256) in a 400 px `ScrollView`, 800 × 800 window, offset 2,640; 8 runs a side, each the median of 4,000 paints | rows **34** in the window, **17** in the viewport; commands Some(surface) **224**, None **224** — equal, the setup check: nothing culls; paint **1.18 µs** (1.18–1.71) and **1.18 µs** (1.18–1.18) | — (a bench; its setup asserts 220 leaves, a 400 px viewport, the offset, and more rows in the window than in the viewport) | T85.8 (K85b) |
| `crates/lumen-static-shell/tests/cull_damage.rs`, `Everything` and `Boundaries<256>` (identical): a `List` of 220 one-row `List`s in a 240 × 200 `ScrollView`, a label and a readout beside it, 400 × 400 window, ten 7 px wheel steps, the readout written every step; the `None` arm a repaint of the host's frame whose surface twin equals the host's damage at every step | every wheel handled, offset 7 → 70; culled count **59 / 62** (changes on **5** of 9 steps), uncull **668** every step; over ten steps the label damaged **8,960 px** culled (1,792 on each step whose count changed) against **0** uncull, and the damage outside the scroller **416,640 px** culled against **407,680** uncull | `List` culls nothing: count changes on 0 of 9 steps, "the list's command count never changed"; `Incremental` comparing two frames of different lengths slot for slot from their ends: label **0 px** culled, "culling no longer charges the label (0 px) -- the gap this pins has closed" (the outside total *rose* to 447,680 under it, so the label is the assertion that sees it) | T85.7 |
| `crates/lumen-static-shell/tests/cull_ink.rs`, a derived column, a card whose rect `[0, 300, 300, 372]` starts at the surface's edge, its shadow 8 inking `[-8, 294.8, 308, 382.8]` | uncull render: **1,215** shadow pixels on the surface; Some vs None **0** pixels differ (bound 0) | `Card::paint` returns when its rect misses `clip_bounds`: **1,171** pixels differ, fails by name | stays green (T85.8) |
| the same file, a derived column, an open `Select` whose trigger `[0, 300, 28, 334]` is below the surface, its menu `[0, 200, 300, 300]` on it, `content_absent()` false | uncull render: **13,190** inked pixels in the menu's rect; Some vs None **0** differ (bound 0) | `Popup::paint` returns when its anchor misses `clip_bounds`: **14,270** pixels differ, fails by name | stays green (T85.8) |
| the same file, a `List` of the two cards (the plan's pin) | uncull: **1,215** shadow pixels; Some vs None **1,171** pixels differ — `List` culls by `rect_of`, so the shadow vanishes | `List` culls nothing: **0** differ, "a List no longer culls the card below the surface … flip the gate (T85.8)" | T85.8 |
| firmware, `just firmware-delta f5fedd9 firmware=0 firmware_interactive=0 firmware_bind=0 firmware_cascade=0` | **+0** on all four `.text` and `.rodata` (`firmware_interactive` 63,912 B); K85d cumulative **+432** of 1,024 against `a21135b` | — | — |

## T85.7 — `Retention::scope_end`: built, measured, K85c fired (2026-10-06)

Built as the plan wrote it: `Retention::scope_end(key)`, required, on
`Rebuild`, `Incremental`, `Streaming<N>` and the two test strategies
(`container_fill.rs`'s `Counted`, and `path_damage.rs`'s `Inks`, which
M82 added after the plan counted four); `Recording` keeping per-depth
ordinals (each scope's key its parent's folded with its ordinal among
its siblings, the children's count reset when it opens) and calling
`scope_end` after observing each pop; `Incremental` keeping each frame's
`(key, slot index)` scope ends beside its two slot buffers, comparing
slot for slot when the lengths and every scope end agree, and otherwise
re-synchronising at every key both frames have (looking at most 64
ends ahead), each segment compared as a whole frame was.

**It worked**, on the gate T85.6 pinned:

| gate | prints | injection (red) |
|---|---|---|
| `cull_damage.rs`, flipped (both strategies, identical) | at every step outside the scroller culled **≤** uncull (**407,680 = 407,680 px** over ten steps, 40,768 a step); the label **0 px** (bound 0) at every step; inside **40,000** a step where T85.6 read 41,600 on the shifted steps | the T85.6 pin against this code: "culling no longer charges the label (0 px)"; `Incremental::scope_end` a no-op: "step 2: damage outside the scroller with culling (42560 px) is above without (40768 px)"; keys from a global pop count: the same, step 2, 42,560 against 40,768 |

**K85c — re-sync is free when nothing shifts.** `9c73dc9` (T85.6)
against this tree, two release binaries each, eight interleaved runs a
side under one `just bench-lock` (quiet; load1 2.50 before, 2.27 after,
bound 8):

| row | base median (spread) | T85.7 median | verdict |
|---|---:|---:|---|
| `--bin retain` 2,000 cmds, `streaming: build + Incremental` | 18,229 ns (18,089–18,397) | **20,340** | **above** (+11.6%) |
| `--bin retain` 2,000 cmds, `streaming: build + Streaming<4096>` | 17,055 ns (16,795–17,418) | **18,241** | **above** (+7.0%) |
| `--bin retain` 2,000 cmds, `streaming: NO list + Streaming<4096>` | 10,978 ns (10,944–11,035) | **11,923** | **above** (+8.6%) |
| `--bin retain` 2,000 cmds, `wrapper only (Rebuild)` | 13,010 ns (12,949–13,282) | **13,568** | **above** (+4.3%) |
| `--bin retain` 34 cmds, `streaming: build + Incremental` | 295.2 ns (293.4–303.0) | **324.2** | **above** (+9.8%) |
| `--bin retain` 2,000 text cmds, `build + Incremental` (no clips) | 19,924 ns (19,623–20,192) | 19,678 | inside |
| frame arm `idle (floor)` | 56.35 µs (55.7–85.2) | 61.25 | inside |
| frame arm `+render (damage)` | 77.10 µs (73.4–86.3) | 76.75 | inside |
| frame arm `ALL rows: layout`, `+paint`, `+render (full)`, `+paint (culled)` | — | — | inside, every one |

**Attributed**, by a third binary with the keys not computed
(`Recording` passing `0`), eight runs a side under one lock (quiet): at
2,000 commands of which 500 are a clip push and pop (`--bin retain`'s
stream), computing the keys costs about **1,000 ns** on every strategy
(≈2 ns a scope: `Streaming<4096>` 16,359 → 17,464 ns), and
`Incremental`'s scope-end buffer and its equality check about **900 ns**
more (18,609 → 19,491). The rows with no clip in them do not move. A
per-scope cost is paid on every frame, shifted or not, so "free when
nothing shifts" cannot hold for any shape of this mechanism that records
scopes as they pass; the frame arm, whose tree has few clips, cannot see
it (inside every spread), and `--bin retain`'s quarter of clip commands
can.

**K85c fires.** Its written response: T85.7 is reverted (the code is not
in the tree; this section and the plan's "What building found" record
it), and container culling is decided against **on X1's ground** — so
T85.8 is not shipped, whatever K85b would have said. T85.6's gates stay
as pinned: `List`'s cull and its retention gap are today's behaviour,
and `cull_damage.rs` and `cull_ink.rs` keep holding them. Firmware: docs
and test comments only, `just firmware-delta 9c73dc9 firmware=0
firmware_interactive=0 firmware_bind=0 firmware_cascade=0`, **+0** on
all four.
