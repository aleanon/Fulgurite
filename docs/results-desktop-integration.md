# Results: M90 — desktop integration

*M90, `docs/plan-desktop-integration.md`. Owns every figure that plan
cites. Machine: i9-13900KF. Created by T90.0 (2026-10-06).*

## T90.0 — the pins and the baselines (2026-10-06, at `6cd1e0d`)

What a build does with a secondary button, a long hold, the Menu key and
a screen reader's "show context menu" before M90 builds anything, and the
numbers the milestone's kill rules (K90a, K90b, K90c) are judged against.

### The pins (`crates/lumen-static-shell/tests/secondary_button.rs`, `crates/lumen-static-winit/tests/translate.rs`)

Each asserts today's behaviour green and names the task that flips it
(`docs/plan-iced-roadmap.md` §3.1). Identical under `Everything` and
`Boundaries<256>`.

| pin | today (printed) | setup (left button, same point) | flipped by |
|---|---|---|---|
| right press+release on `Button` | handled true, handler count 0 → 1 | handled true, 0 → 1 | T90.2 |
| … on `CheckBox` | handled true, toggles 0 → 1 | handled true, 0 → 1 | T90.2 |
| … on `Slider` (pressed at ¾) | handled true, value 0 → 0.763 | handled true, 0 → 0.763 | T90.2 |
| … on `Collapsible` (header) | handled true, open 0 → 1 | handled true, 0 → 1 | T90.2 |
| … on `Tabs` (second item) | handled true, selected 0 → 1 | handled true, 0 → 1 | T90.2 |
| … on `TextInput` | handled true, caret 0 → 11 | handled true, 0 → 11 | T90.2 |
| a right release mid left `Slider` drag | capture ended; value 0.500 → 0.500 on a move to 0.9 | the drag captured, 0.237 → 0.500 | T90.2 |
| a 600 ms primary-touch hold over `Button` | handler count 0 (held) → 1 (lifted) | — | T90.3 (to 0) |
| the same hold over `Collapsible`, `Tabs` | acted at `Begin`: 0 → 1 (held) → 1 | — | kept by T90.3 (P16) |
| `key_of(NamedKey::ContextMenu)` | `None` | `key_of` maps F1–F12 (existing test) | T90.3 |
| `action_of(ShowContextMenu)`, `a11y` | `None` | `Click` → `Some((3, Default))` | T90.3 |

The `TextInput` starts with its caret at 0 and a press puts it at the
end: a host with no font shapes no clusters, so the end of the run is the
only place a press can land (`docs/plan-desktop-integration.md`, "What
building found").

### Sizes (K1; `docs/plan-iced-roadmap.md` §2.1)

`secondary_button.rs`'s `the_sizes_m90_must_not_move` asserts each.

| type | bytes |
|---|---:|
| `Event` | 16 (bound 16) |
| `Action` | 8 |
| `Key` | 8 |
| `Style` | 84 |
| `NodeState` | 88 |
| RAM per node (`Style` + `NodeState`) | 172 (16.8 KB per 100 nodes) |

### Firmware (K90a's parent)

`just firmware-delta 6cd1e0d firmware=0 firmware_interactive=0
firmware_bind=0 firmware_cascade=0`, T90.0's tree against its parent:

| binary | `.text` | `.rodata` | Δ |
|---|---:|---:|---:|
| `firmware` | 31,306 | 13,348 | 0 |
| `firmware_interactive` | 64,160 | 21,890 | 0 |
| `firmware_bind` | 34,678 | 13,952 | 0 |
| `firmware_cascade` | 33,398 | 14,136 | 0 |

`firmware_interactive` has 17,760 B left under the 81,920 B ceiling; K90a
allows T90.2–T90.4 256 B each on it and 0 B on `firmware`.

### The `dispatch` rows K90b reads (`benches --bin invalidation`)

Eight runs, each under `scripts/bench_lock.sh` (all eight quiet, load1
0.6–1.2 against a bound of 8). The press rows are new in T90.0: the two
`PointerMove` rows cannot see a filter on a press's button or a
`take_menu` asked after a consumed event. The consumed-click row is
`Host::pump` (which is `pump_with` over the block measurer) on `Ten10`
with one `Button` at its head, 903 nodes, no menu hook; it prints the
clicks consumed beside the samples (2,000 of 2,000).

| row | min | median | max (µs) |
|---|---:|---:|---:|
| `dispatch, one PointerMove` | 0.030 | 0.040 | 0.040 |
| `dispatch, one PointerMove (tiny)` | 0.020 | 0.020 | 0.030 |
| `dispatch, left press+release` | 0.066 | 0.069 | 0.072 |
| `dispatch, right press+release` | 0.065 | 0.068 | 0.071 |
| `pump_with, one consumed click` | 10.190 | 10.575 | 11.090 |

The two `PointerMove` rows print two decimals (10 ns), so their spread is
one step of their own resolution; the press rows print three.

### Crate counts (K2's method, for K90c)

`cargo tree -p lumen-static-winit -e normal --prefix none [--features F]
| sed 's/ (\*)$//' | sort -u | wc -l`:

| build | crates | over default | without the `sed` |
|---|---:|---:|---:|
| default | 96 | — | 117 |
| `dialogs` | 106 | +10 | 133 |
| `a11y` | 167 | +71 | 212 |

The manifest's `dialogs` comment already gives +10 by this method
(`crates/lumen-static-winit/Cargo.toml`); its 79 (218 → 297) is the
whole-workspace graph. K90c bounds `tray` at `a11y`'s +71.

## T90.2 — secondary buttons stop acting, and `Event::ContextRequest` (2026-10-06, on `baa18ce`)

A `Right` or `Middle` press or release stops in dispatch before pass 1
(`pass::secondary`); a right press runs the outside-press dismissal and is
delivered as `Event::ContextRequest { x, y }`, bubbling from the node
under it.

### The flipped pins (`crates/lumen-static-shell/tests/secondary_button.rs`)

Identical under `Everything` and `Boundaries<256>`. For each widget and
each of `Right` and `Middle`: `handled`, the effect, and after the press
the pressed bit and the latched press; focus starts on another control.
The left button at the same point is the setup (accept direction).

| widget | right or middle press+release | left, same point |
|---|---|---|
| `Button` | handled false, count 0 → 0, no pressed bit, no latch, focus kept | handled true, 0 → 1 |
| `CheckBox` | handled false, toggles 0 → 0, … | handled true, 0 → 1 |
| `Slider` (¾) | handled false, value 0 → 0, … | handled true, 0 → 0.763 |
| `Collapsible` | handled false, open 0 → 0, … | handled true, 0 → 1 |
| `Tabs` (second) | handled false, selected 0 → 0, … | handled true, 0 → 1 |
| `TextInput` | handled false, caret 0 → 0, … | handled true, 0 → 11 |
| left `Slider` drag, a right press+release mid-drag | capture held; the next move to 0.9 reads 0.921 (from 0.500); the left release ends it | the drag captured, 0.237 → 0.500 |

`crates/lumen-static/tests/context_request.rs`, through
`dispatch_overlaid_into` with a real overlay list: a right press outside
an open `Select` list closes it (handled true, `consumed` `[3]`, the popup
node) and its release over a `Button` is handled false with 0 presses; a
probe that consumes the request receives it once at the pressed point,
`(50, 254)`, with `ctx.pos` equal, and a left click on it delivers none.
`crates/lumen-static-agent/tests/button.rs` (T90.1's gate, rewritten):
`button: "right"` arrives as one request at `(40, 20)` on `input.click`
and `input.pointerDown`, and `input.pointerUp` right is handled false;
`"middle"` is handled false on all three, echoed `"middle"`; omitted and
`"left"` still arrive as `Left` presses and releases.

### Injections (red, by name)

| injection | what went red |
|---|---|
| (1) delete the filter (both call sites) | all six widgets, both buttons; the drag (capture held false); both `context_request.rs` gates; the agent gate (Right and Middle handed to the probe) |
| (2) filter in pass 2 only (`activates` takes a left `PointerUp`) | all six by pressed bit, latch and focus; by effect the four pass-1 widgets, `Slider` 0 → 0.763, `Collapsible` 0 → 1, `Tabs` 0 → 1, `TextInput` 0 → 11 |
| (3) a secondary release goes through the capture arm | the drag: capture held false, 0.500 → 0.500 |
| (4) a right press skips the dismissal | the `Select` gate: the list stayed open, handled false |

### The live leg (`just widget-drive`)

Every action whose `do` sends a left `input.click` is replayed after its
own checks with `button: "right"`: `handled: false`, and the widget's
pixels unchanged (`ui.regionHash`, settled). With a popup open
(`app.diagnostics`' new `open_overlays` > 0) the click is not sent, since
a right press there dismisses it. Both lists are named in
`scripts/drive/expected.json`'s `secondary`, and a run compares them by
name with what it filed. Run on a private display, one window per leg:

| leg | checked (manifest) | not sent, popup open (manifest) | actions failed |
|---|---:|---:|---:|
| `Everything` | 85 (85) | 19 (19) | 0 of 166 |
| `Boundaries<256>` | 85 (85) | 19 (19) | 0 of 166 |

Red, with the filter deleted, on `Button,Tabs,Slider`: `Button`, `Tabs`
and `Slider` each failed by name (handled True), and the leg filed 0 of the
7 the manifest names for them.

### Firmware (K90a)

`just firmware-delta baa18ce firmware=0 firmware_interactive=256`:

| binary | `.text` | Δ | `.rodata` Δ |
|---|---:|---:|---:|
| `firmware` | 31,306 | +0 (bound 0) | +0 |
| `firmware_interactive` | 64,156 | **−4** (bound 256) | +0 |
| `firmware_bind` | 34,678 | +0 | +0 |
| `firmware_cascade` | 33,398 | +0 | +0 |

K90a does not fire. Sizes unchanged (`the_sizes_m90_must_not_move`):
`Event` 16, `Action` 8, `Key` 8, `Style` 84, `NodeState` 88.

### K90b (`benches --bin invalidation`, `benches-competitive/arms/frame`)

Eight runs a side under `scripts/bench_lock.sh`, the parent's and this
task's binaries interleaved (all quiet), min / median / max µs:

| row | parent `baa18ce` | T90.2 | reading |
|---|---|---|---|
| `dispatch, one PointerMove` | 0.040 / 0.040 / 0.050 | 0.030 / **0.030** / 0.040 | below the spread (faster) |
| `dispatch, one PointerMove (tiny)` | 0.020 / 0.020 / 0.030 | 0.020 / **0.020** / 0.030 | inside |
| `dispatch, left press+release` | 0.068 / 0.069 / 0.098 | 0.057 / **0.061** / 0.076 | below (faster) |
| `dispatch, right press+release` | 0.068 / 0.069 / 0.088 | 0.058 / **0.062** / 0.069 | below (faster: a right press now stops before pass 1) |
| `pump_with, one consumed click` | 10.600 / 11.065 / 11.230 | 10.280 / **10.825** / 11.430 | inside |
| `idle (floor)` | 57.300 / 60.700 / 63.700 | 57.000 / **63.800** / 64.200 | 0.1 µs over the max |
| `idle (floor)`, a second interleaved run | 57.400 / 60.650 / 64.100 | 54.000 / **60.250** / 61.100 | inside |

No row costs more than its parent: the idle row dispatches nothing, so
the filter cannot be in it, and the second interleaved run puts its median
back inside the parent's spread. K90b does not fire.

## T90.3 — keyboard, touch and assistive-technology routes (2026-10-06, on `cce0f65`)

Shift+F10 and `Key::Menu` ask at the focused node's bottom-left; the
host's `LongPress` on the primary touch asks at the touch point and spends
the touch (its `End` a `PointerCancel`); `Action::ContextMenu` asks at the
centre of its target. All three end in `Event::ContextRequest`.

### Gates (`crates/lumen-static-shell/tests/context_routes.rs` unless named)

Identical under `Everything` and `Boundaries<256>`.

| gate | measured | bound / accept |
|---|---|---|
| Shift+F10, focus on `#probe` `[0, 42, 120, 82]` | one request at `(0, 82)`, handled true; `on_key` 0 times | the bottom-left; consumed, so not a key |
| the Menu key, same | one request at `(0, 82)`; `on_key` 0 | same |
| either, focus on a `Button` (no consumer) | `on_key` saw it once: `(F(10), SHIFT)`, `(Menu, NONE)`; 0 clicks | accept: the key goes on as a key |
| a hold over `#probe` at `(60, 20)` | 0 requests at 499 ms, one at `(60, 20)` at 500 ms | `LongPress`'s 500 ms |
| a 500 ms hold over a `Button` | clicks 0 held, 0 lifted (a cancel) | — |
| a hold drifting 11 px (slop 10) | 0 requests; over the `Button` the lift is a tap, clicks 1 | accept |
| `next_deadline_ms` | no touch `None`; a hold pending `Some(500)`, 200 ms in `Some(300)`; lifted `None` | ≤ 500; unchanged with none |
| a 500 ms hold over `Collapsible`, `Tabs` (P16) | 0 → 1 (held) → 1 (lifted), captured while held, no capture or press after the lift | kept, as T90.0 pinned it |
| `Action::ContextMenu` on `#probe` | handled true, one request at `(60, 62)`, inside its rect | inside |
| `Action::ContextMenu` on a `Button` | handled false, 0 clicks | accept |
| `secondary_button.rs`, the 600 ms hold (T90.0's pin, flipped) | `Button` 0 → 0 → **0**; `Collapsible`, `Tabs` 0 → 1 → 1 | the release is a cancel |
| `crates/lumen-static-agent/tests/input.rs` | `"F1"`..`"F12"` reach the host as `F(1)`..`F(12)`; `"F10"` + shift as `(F(10), SHIFT)`; `"Menu"` as `Menu`; `"F0"`, `"F13"` refused, naming the key, with 28 known names | both directions |
| `crates/lumen-static-winit/tests/translate.rs` | `key_of(ContextMenu) = Some(Menu)`; with `a11y`, `action_of(ShowContextMenu) = Some((3, ContextMenu))` | T90.0's pins flipped |

The flip's red run (T90.0's pins against this code): "a 600 ms hold's End
is a release today ... handler count 0"; "the Menu key translates to
nothing today ... it answered Some(Menu)"; "ShowContextMenu is not routed
today ... it answered Some((3, ContextMenu))".

### Injections (red, by name)

| injection | what went red |
|---|---|
| (1) the spent touch's `End` stays a `PointerUp` | "the lift of a long hold activated the button" (handler count 1) |
| (2) the hold leaves `next_deadline_ms` | "a pending hold set no deadline: None" |
| (3) the keyboard request at `(0, 0)` | "want one request at (0.0, 82.0), got [(0.0, 0.0)] (on #probe's rect [0.0, 42.0, 120.0, 82.0]: false)" |
| (4) `parse_key` loses its `"F<n>"` arm | "unknown key `F1`" (the first of the twelve) |
| (5, added) the `Action::ContextMenu` arm in dispatch removed | "want one request inside [0.0, 42.0, 120.0, 82.0], got []" |

### Firmware (K90a)

`just firmware-delta 59cc73b firmware=0 firmware_interactive=0
firmware_bind=0 firmware_cascade=0` (the parent for firmware, since
`cce0f65` touches no firmware): +0 on all four binaries, `.text` and `.rodata` (the routes are behind `std`;
`Key::Menu` and `Action::ContextMenu` move no firmware byte). K90a does
not fire.

### K90b, and the layout the dispatch rows can see

The parent's binaries (`59cc73b`; `cce0f65` changed two scripts only)
against this task's, interleaved, eight runs a side under
`scripts/bench_lock.sh` (all quiet), min / median / max µs:

| row | parent | T90.3 | reading |
|---|---|---|---|
| `dispatch, one PointerMove` | 0.030 / 0.030 / 0.040 | 0.030 / **0.040** / 0.060 | inside (at the max; a 10 ns step) |
| `dispatch, one PointerMove (tiny)` | 0.020 / 0.020 / 0.020 | 0.020 / **0.020** / 0.040 | inside |
| `dispatch, left press+release` | 0.060 / 0.061 / 0.064 | 0.062 / **0.063** / 0.101 | inside |
| `dispatch, right press+release` | 0.061 / 0.062 / 0.064 | 0.062 / **0.063** / 0.104 | inside |
| `pump_with, one consumed click` | 10.790 / 10.815 / 11.340 | 10.500 / **10.520** / 23.000 | below (faster) |
| `idle (floor)` | 56.900 / 63.600 / 64.300 | 57.100 / **60.500** / 106.200 | inside |

**The first build fired K90b**, and why is worth keeping. With the
keyboard test ahead of the position branch, the left press+release median
read 0.064–0.065 against the parent's 0.060–0.063 in two interleaved
runs. The written response (gate the per-event work) was applied: the
test moved onto the key's own path, where no pointer event reaches it, and
`request_context` was marked `#[inline(never)]` — and the row read
**0.066–0.068**, slower. A bisect, sixteen interleaved runs per binary
under the lock, then showed the rows move with code a press never runs:

| build (left press+release, median µs) | |
|---|---:|
| the parent | 0.061 |
| the parent + `Key::Menu` and `Action::ContextMenu` only | 0.061 |
| the parent + `#[inline(never)]` on `request_context` only — a function only a right press calls | **0.068** |
| T90.3's first build: the key test ahead of the position branch, no attribute | 0.064–0.065 |
| the key test on the key path, no attribute | 0.067 |
| the same with the key and action arms compiled out (`if false`) | 0.069 |
| the key test on the key path, `request_context` `#[inline(never)]` | 0.066–0.068 |
| the same, `#[inline(always)]` | 0.063 |
| the same, `#[cold]` (shipped) | 0.063–0.064 |

So a few nanoseconds on these rows is the layout of `dispatch_at`, not
work on the row's path (M26's Phase D finding again: two arms that
disagree about one quantity are a fact about the arms). The shipped
build keeps the key test on the key path and marks `request_context`
`#[cold]`, which is what it is, and reads inside every spread. K90b does
not fire on it.

## T90.4 — `ContextMenu<W>`, `CommandList`, `Anchor::At` and `Program::on_menu` (2026-10-07, on `cad8edc`)

A menu is data (`lumen_static::menu`: `MenuEntry`, `Accel`,
`MenuSection`, `MenuMask`); a choice is its id, taken once from the node
that consumed the release or the Enter (`Node::take_menu`, asked only
with a hook installed) or posted natively as `Event::Menu { id }`, and
both end in `Program::on_menu`. `ContextMenu<W>` consumes the
`ContextRequest` that bubbles to it and opens its `CommandList` at the
point (`Anchor::At`).

### Gates (`crates/lumen-static-shell/tests/context_menu.rs` unless named)

Identical under `Everything` and `Boundaries<256>`. A 360 × 420 window; the
wrapper is `[0, 16, 360, 136]`, under a label.

| gate | measured | bound / accept |
|---|---|---|
| a right press at `(40, 46)` inside the wrapper | handled true, open, the list's top-left `(40, 46)` | the press point |
| a release on row 1 (Copy) | handled true, `on_menu` `[20]` once, closed | `ENTRIES[1].id`, once |
| a right press at `(350, 46)` | the list `[140, 46, 360, 178]` | inside 0..360 × 0..420; x = 360 − 220 |
| Down, Down, Enter | focus on the list (index 5) once open; `on_menu` `[20]` | the keyboard chooses |
| a release on the disabled row 2 (Paste) | handled true, `on_menu` `[]`, still open | 0 calls, open; accept: row 3 → `[40]` |
| no `on_menu`: a click the area consumes | `take_menu` asked **0** times | 0; accept: with the hook, asked 1 |
| `Event::Menu { id: 77 }` | handled true, `on_menu` `[77]`; with no hook handled false | the same hook |
| the frame that opens the menu | 1 region `[28, 108.2, 272, 264.2]` = the list's ink (`display::shadow_ink`); 38,064 px² against list ink + wrapper 74,481 px² = **0.51×**; outside both **0 px²** | ≤ 1.00×; 0 px² |
| `size_of::<Event>()` | 16 | 16 (K1) |
| `crates/lumen-static-widgets/tests/a11y_roles.rs` | `ContextMenu` `GenericContainer` publishing `ShowContextMenu`, handled 1 of 1; `CommandList` `Menu`; 72 rows over 56 impls (55 covered, `ShaderView` excused); all rows published 54, handled 54 | every published action handled |
| `crates/lumen-static-agent/tests/actions.rs` | `input.invokeAction` `contextMenu` on the wrapper: handled true, open false → true; `contextmenu` refused naming the list | both directions |

### Injections (red, by name)

| injection | what went red |
|---|---|
| (1) the host asks `take_menu` of the consumer's grandparent (the wrapper) | "on_menu did not run once with the row's id", `[]`; the disabled gate's accept `[]`; "with a hook the consuming node was not asked" |
| (2) `Popup::place` ignores `Anchor::At` (its parent's origin) | "list at (0, 16) ... the list's top-left is not the press point" |
| (3) `CommandList::choose` ignores the mask | "release on disabled row 2 (Paste): handled true, on_menu [30], open false ... a disabled entry ran the hook" |
| (4, added) the wrapper paints a window-sized scrim while open | "1 region [0, 0, 360, 420] ... 151200 px² ... = 2.03x, outside both 76719 px²" |
| the role rows deleted | "Node impls with no row in the table: [CommandList, ContextMenu]" |
| `ContextMenu::on_event` ignores the request (role table) | "ContextMenu: handled 0 / published 1 -- not handled: [ShowContextMenu]" |
| the agent's `contextMenu` arm dropped | handled `null`, "unknown action `contextMenu`" |

### The live leg (`just widget-drive ContextMenu`, `scripts/drive/widgets/menus.py`)

A private Xephyr display, all six legs (`Everything`, `Boundaries<256>`,
`+oracle`, `+scale2` with the full-repaint check, and both strategies with
`shaping`): each "1 widgets, 10 actions, 5 without an isolate, 0 failed";
parity 1 of 1 byte-identical with no agent; the oracle compared 813 frames,
none diverged; the keyboard pass reached `#d_context_a >
#context-menu-content` in 1 Tab and Shift+F10, Down, Enter chose Cut
(`chose: nothing yet` → `chose: Cut (id 1)`); app exit 0. The secondary
leg: `checked 2 (manifest 2)`, `overlay_open 1 (manifest 1)`, `opened 1
(manifest 1)` — the replayed right click on the button handled true, one
overlay open, 0 after Escape, its 5,600 px restored. From the `Everything`
leg:

| action | measured | bound |
|---|---|---|
| a right click opens the menu at the press | the list at `(638, 429)`, want `(638, 429)`; damage 29,040 px² = 1.00× the list | 1 px; 4.0× |
| Shift+F10 / the Menu key on the focused button | the list at `(615, 449)`, want `(614, 449)` (the button's bottom-left, fractional) | 1 px |
| a reader's `contextMenu` action | the list 132 px tall | open |
| a click on Cut | `chose: Cut (id 1)`, closed; damage 1.41× | 4.0× |
| Down, Down and Enter | `chose: Copy (id 2)`, closed | — |
| a click outside / Escape | closed (overlays 0) | — |

Live injections, the `Boundaries` leg alone: `ContextMenu::on_event`
ignoring the request — "a right click on a context-menu entry did not open
its menu and close on Escape -- handled False, overlays 0 -> 0", the
keyboard pass "readout 'chose: nothing yet' -> 'chose: nothing yet'",
`opened 0 (manifest 1)`, 5 failed; T90.3's injection (4), `parse_key`
without its `"F<n>"` arm — "input.key ... unknown key `F10`", 3 failed.

**No AT-SPI leg**: AccessKit's AT-SPI adapter (0.20.0) offers only
`click`, so nothing on the bus can ask for `ShowContextMenu`; the live
route is the agent's `contextMenu`, which enters where the adapter's
request would (`docs/plan-desktop-integration.md`, "What building found").

### Firmware (K90a)

`just firmware-delta cad8edc firmware=0 firmware_interactive=256`:
`firmware` +0, `firmware_interactive` **+12** (64,156 → 64,168),
`firmware_bind` +0, `firmware_cascade` +0, `.rodata` +0 on all four. The 12
B are `pass::dispatch` (5,396 → 5,408 by `nm`), its `match` over `Event`
re-laid for `Event::Menu`; no menu code links. K90a does not fire.

### K90b (`benches --bin invalidation`, `benches-competitive/arms/frame`)

The parent's binaries (`cad8edc`) against this task's, interleaved under
`scripts/bench_lock.sh`, after the reference machine's CI run had
finished, every run quiet. First set, eight runs a side, min / median /
max µs:

| row | parent | T90.4 | reading |
|---|---|---|---|
| `dispatch, one PointerMove` | 0.030 / 0.035 / 0.050 | 0.030 / **0.040** / 0.050 | inside |
| `dispatch, one PointerMove (tiny)` | 0.020 / 0.025 / 0.040 | 0.020 / **0.020** / 0.040 | inside |
| `dispatch, left press+release` | 0.061 / 0.065 / 0.084 | 0.058 / **0.064** / 0.090 | inside |
| `dispatch, right press+release` | 0.062 / 0.064 / 0.082 | 0.058 / **0.063** / 0.084 | inside |
| `pump_with, one consumed click` | 10.090 / 11.040 / 11.140 | 10.590 / **11.145** / 11.180 | **0.005 over the max** |
| `idle (floor)` | 57.1 / 63.3 / 64.6 | 53.9 / **63.2** / 65.1 | inside |

**The consumed-click row read 0.005 µs past the parent's max, so K90b's
written response was applied**: the per-event work that row can see is
the host's call into the menu take, which already returned at once without
a hook; the hook test moved to the call site (`took &&
self.on_menu.is_some()`), so a program with no menus makes no call at all.
Remeasured, twelve runs a side interleaved, all quiet:

| row | parent | T90.4 (gated at the call) | reading |
|---|---|---|---|
| `dispatch, one PointerMove` | 0.030 / 0.030 / 0.050 | 0.030 / **0.040** / 0.060 | inside |
| `dispatch, one PointerMove (tiny)` | 0.020 / 0.030 / 0.040 | 0.020 / **0.020** / 0.040 | inside |
| `dispatch, left press+release` | 0.062 / 0.064 / 0.095 | 0.061 / **0.064** / 0.106 | inside |
| `dispatch, right press+release` | 0.062 / 0.065 / 0.096 | 0.061 / **0.062** / 0.099 | inside |
| `pump_with, one consumed click` | 10.490 / 11.015 / 11.150 | 10.610 / **11.145** / 11.280 | inside (at the edge) |

K90b does not fire on the shipped build. The consumed-click median sits
about 0.13 µs (1.2%) above the parent's in both sets, inside its spread:
what is left on that path without a hook is one discriminant test
(`Event::Menu`) per drained event and the `on_menu` test per consumed one.

## T90.5 — `MenuBar`, `Program::menu` and accelerators (2026-10-07, on `1faf792`)

The menu bar is drawn by the framework from the program's `&'static
[MenuSection]` (owner decision D2), and `Program::menu(sections,
enabled)` installs the same data into the host through `Program::host()`:
a key press no widget consumed is offered to the enabled entries'
accelerators before `Program::on_key`. In a window Ctrl+V is consumed as a
paste only when the focused node took it (`Session::paste` answers
`Option<usize>`, `keys::clipboard_shortcut`).

### Gates (`crates/lumen-static-shell/tests/menu_bar.rs` unless named)

Identical under `Everything` and `Boundaries<256>`. A 480 × 360 window:
the bar (File: New, Save; Edit: Copy, Paste; View: Zoom) over a
`TextInput`.

| gate | measured | bound / accept |
|---|---|---|
| Ctrl+S, nothing focused | handled true, `on_menu` `[2]`, `on_key` **0** | once; 0 |
| Ctrl+S with Save disabled (`enabled` clears bit 1) | `on_menu` still `[2]`, `on_key` `[(Char('s'), CTRL)]` | the key falls through once |
| `s` typed into the focused field | text `"s"`, `on_menu` 1 (unchanged), `on_key` **0** | neither hook |
| `Program::menu` with 65 entries over 2 sections | panics: "a program's menu holds at most 64 entries (one bit each of its MenuMask); this one has 65 across 2 sections" | refused by name; accept: 64 accepted |
| a click on title 0 at `(30, 17)` | handled true, open `Some(0)`, `["New", "Save"]` at `(4, 34)`, focus moves into the list | open below its title |
| Right with the menu open | open `Some(1)`, entries `["Copy", "Paste"]`, the list at x 56 = title 1's | the next title's menu |
| Escape | open `None`, focus on the bar (index 1) | focus returns to the bar |
| hover title 2 with title 0 open | open `Some(2)` | switches |
| a release on File > Save | `on_menu` `[2]`, closed | once |
| Down then Enter on the focused bar | opened `Some(0)`, `on_menu` `[1]` (New) | the keyboard opens and chooses |
| `crates/lumen-static-winit/tests/clipboard_shortcut.rs` (X3): a `Button` focused, `"x"` on the clipboard, Edit > Paste on Ctrl+V | `keys::clipboard_shortcut` consumed **false**; the key then runs `on_menu` `[30]` | not consumed, once; accept: a focused `TextInput` — consumed true, text `"x"`, `on_menu` unchanged |
| `crates/lumen-static-shell/tests/agent_hooks.rs`: `input.key` chords | Ctrl+K (an accelerator) handled true, count 100; Ctrl+J handled **false** | the key's own answer |
| `crates/lumen-static-widgets/tests/a11y_roles.rs` | `MenuBar` `Role::MenuBar`, publishes `Focus`, handled 1 of 1; 73 rows over 57 impls (56 covered); all rows published 55, handled 55 | every published action handled |

### Injections (red, by name)

| injection | what went red |
|---|---|
| (1) `on_key` before the accelerators | "Ctrl+S did not run on_menu(SAVE) once": on_menu `[]`, on_key 1 |
| the accelerator ignores `enabled` | "a disabled Save ran on_menu": `[2, 2]`, on_key `[]` |
| (added) a typed character is not owned by a node with a caret | "typing `s` into a field reached on_key": 1 against 0 |
| `Program::menu`'s limit off by one | "Program::menu accepted 65 entries" |
| the bar's Left/Right move only the roving title while a menu is open | **green at first** — the gate read `open_title()`, the roving title; rewritten to read the list, it names "Right moved the title but the menu still shows another section" (`["New", "Save"]`) |
| the bar's own node not `FOCUSABLE` | "focus did not return to the bar": focus `None` |
| (3) `keys::clipboard_shortcut` consumes `Clip::Paste` whatever `paste` answers | "Ctrl+V on a Button was swallowed as a paste nothing took": consumed true, on_menu `[]` |
| the role row deleted | "Node impls with no row in the table: [\"MenuBar\"]" |
| the agent pumps `Modifiers` with the key (today's code) | "the modifier event's answer leaked into the key's": Ctrl+J handled true |

### The live legs

**`just widget-drive MenuBar`** (`scripts/drive/widgets/menus.py`), a
private Xephyr display, all six legs (`Everything`, `Boundaries<256>`,
`+oracle`, `+scale2` with the full-repaint check, and both strategies with
`shaping`): each "1 widgets, 8 actions, 3 without an isolate, 0 failed";
parity 1 of 1 byte-identical; the keyboard pass reached `#d_menubar_a` in
1 Tab and Right, Down, Enter chose Undo (`chose: nothing yet` →
`chose: Undo (id 21)`); the secondary leg `checked 1 (manifest 1)`,
`overlay_open 2 (manifest 2)`; the oracle compared 753 frames, none
diverged; app exit 0. After the agent's `input.key` change, the final tree
was driven again as `just widget-drive MenuBar,ContextMenu,Selectable`
(the entries whose drives send chords): all six legs "3 widgets, 20
actions, 0 failed", parity 3 of 3, keyboard 4 actions 0 failed, the oracle
1,965 frames none diverged. From the `Everything` leg:

| action | measured | bound |
|---|---|---|
| a click on a title | the menu shows `New,Open,Save`, at x 606 = File's; damage 1.00× | 1 px; 4.0× |
| a click on Save | `chose: Save (id 13)`, closed; damage 1.00× | — |
| hovering Edit with File open | the menu shows `Undo,Copy,Paste` at Edit's x; damage 1.22× | 1 px; 4.0× |
| Right | `Undo,Copy,Paste` at Edit's x | 1 px |
| Escape | closed, overlays 0, the bar focused | — |
| Down, Enter on the focused bar | `chose: New (id 11)` | — |
| Ctrl+O (`input.key` with `mods`) | `chose: Open (id 12)`, no menu open; isolate: plain `o` changes nothing | — |
| a release on the disabled Paste (static) | readout unchanged, the menu still open | — |

Live injections, the `Boundaries` leg alone: the bar's press arm opening
nothing — "a click on a title opens its menu below it ... the input did
not reach the widget", 4 failed; the host matching no accelerator — "Ctrl+O
runs Open by its accelerator ... readout unchanged", 2 failed. Injection
(1) is the headless gate's alone: the showcase installs no key hook, so
which of the two is asked first cannot show in its window.

**`just hot-window-drive`, the `hot` leg** (step 7b): before the edit
Ctrl+K handled **true**, Ctrl+J **false**; the edit to Ctrl+J swapped in
1.24 s, then Ctrl+J **true**, Ctrl+K **false**; PASS. Injection (2), as
`docs/plan-desktop-integration.md` "What building found" words it (the
first table installed in the process is the one every later host gets):
after the swap "Ctrl+J handled False (want True), Ctrl+K handled True",
"step 7b: after the swap Ctrl+J does not fire".

### Firmware

`just firmware-delta 1faf792 firmware=0 firmware_interactive=0
firmware_bind=0 firmware_cascade=0`: every binary +0 `.text` and +0
`.rodata` (`firmware` 31,306, `firmware_interactive` 64,168,
`firmware_bind` 34,678, `firmware_cascade` 33,398). No core code changed.

### K90b (`benches --bin invalidation`, `benches-competitive/arms/frame`)

The parent's binaries (`1faf792`) against this task's, interleaved under
`scripts/bench_lock.sh` after the reference machine's CI run at `1faf792`
had finished, eight runs a side, every run quiet. min / median / max µs:

| row | parent | T90.5 | reading |
|---|---|---|---|
| `dispatch, one PointerMove` | 0.03 / 0.040 / 0.05 | 0.03 / **0.030** / 0.05 | inside |
| `dispatch, one PointerMove (tiny)` | 0.02 / 0.020 / 0.04 | 0.02 / **0.020** / 0.03 | inside |
| `dispatch, left press+release` | 0.059 / 0.062 / 0.088 | 0.058 / **0.059** / 0.075 | inside |
| `dispatch, right press+release` | 0.059 / 0.062 / 0.089 | 0.060 / **0.061** / 0.075 | inside |
| `pump_with, one consumed click` | 10.59 / 10.935 / 14.53 | 10.22 / **11.020** / 11.33 | inside |
| `idle (floor)` | 56.9 / 57.45 / 63.9 | 56.8 / **63.40** / 63.7 | inside |

K90b does not fire: the accelerator test sits at its call site
(`self.menu.is_some()`), reached only by an event nothing consumed, and
none of these rows installs a menu.
