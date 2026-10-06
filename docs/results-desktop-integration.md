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
