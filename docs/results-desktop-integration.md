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
