# Results: what appearance on the widget would cost

*M25, `docs/plan-appearance.md`. Owns every figure that plan cites.
Machine: i9-13900KF, governor `powersave`. Target: `thumbv7em-none-eabihf`.*

## The thresholds — written before the pilot is built

Recorded at T25.1, 2026-09-15, so T25.5's pilot can be told no.

| quantity | how | threshold | kill if |
|---|---|---|---|
| embedded `.text` | `just firmware-size`; `scripts/check_text.sh` refuses a binary with no Toggle linked | pilot ≤ **+300 B** over the T25.1 baseline, against M16's +488 and M18+M19's +148 | it exceeds that |
| RAM, table tree | state + style + widget struct on the target: `fixtures/embedded`'s `NODE_STATE`, `STYLE` and `UI`, all three asserted | must not grow | it grows |
| RAM, arena row | `NodeState` + one instance, host | reported, not gated — the desktop has no per-node budget | — |
| frame, table tree | `benches --bin appearance` (1 100 table nodes, 1 000 of them toggles), four runs | pilot's median shift smaller than the four runs' spread | it exceeds the spread |
| frame, arena | `just stages 10000 150 20`, the incremental layout and paint stages | same | same |
| cascade | `benches --bin cascade`, including a showcase-shaped sheet | reported with no predicted direction | — |

## T25.1 — the baseline

### Flash

| | `.text` | `.rodata` | total | Toggle symbols |
|---|---:|---:|---:|---:|
| fixture before T25.1, 8 nodes | 21,278 | 12,704 | 34,199 | 0 |
| **with a `CheckBox`, `Radio` and `Switch`, 11 nodes** | **26,252** | 12,752 | 39,221 | 3 |

The second row is the baseline the pilot is judged against. Three toggle
shapes cost 4,974 B; `docs/results-embedded.md` records the re-baseline.

### Bytes, measured by probe

A scratch crate over the real component types, with the six appearance
fields (`color`, `background`, `radius`, `shadow`, `pressed_dim`,
`hover_dim`) removed, and hypothetical widgets carrying a `fn` pointer
plus a stored resolved look. Same result on both targets for the shared
structs:

| | today | appearance removed |
|---|---:|---:|
| `Style` | 72 | **48** |
| `NodeState` | 76 | **60** |
| `Patch` | 108 | 68 |

Widgets, today → with a look pointer and a stored look:

| | thumbv7em | x86-64 |
|---|---:|---:|
| `Toggle` | 8 → **28** | 16 → 40 |
| `Button` | 64 → **88** | 72 → 96 |
| `Label` | 40 → **48** | 56 → 72 |
| `fn` pointer | 4 | 8 |

**A table tree still shrinks per node once the struct is counted** — a
thumb `Label` gives up 40 B of state and style and gains 8 — **and an
arena row does not**: a `List` or `Repeat` shares one style row, so a row
saves only `NodeState`'s 16 B and gains the look, roughly neutral for a
label and a net gain for a button. And under `docs/plan-appearance.md`
§2.3's default, box fills stay per node, so the real per-node saving is
smaller than the table's.

### RAM, the fixture as it stands

`NODE_STATE` 76 + `STYLE` 72 = 148 B per node, × 11 = 1,628 B, plus the
`Ui` struct's **1,104 B** — asserted on the target at T25.1 because
`PER_NODE_RAM` alone cannot see bytes that move onto widgets. **2,732 B
for 11 nodes, 248 B per node.** 768 B of `Ui` is the text field's
`InlineClusters<64>`.

### RAM, an arena row on the host

`NodeState` 76 plus one instance: a `Label<String>` row is **140 B**
(76 + 64), a `Button<(), String>` row **156 B** (76 + 80). With the look
the probe sizes, those become 60 + 72 = 132 B and 60 + 96 = 156 B — a
label row 8 B lighter, a button row unchanged. Printed by
`benches --bin appearance` beside its timing.

### Frame and cascade — four runs, medians

`cargo run --release -p benches --bin appearance`, `--bin cascade` and
`--bin crossover`, four consecutive runs on an idle machine, each figure
a median of blocks:

| | run 1 | run 2 | run 3 | run 4 | spread |
|---|---:|---:|---:|---:|---:|
| **1 000 toggles on the table** (1 100 nodes, layout + paint) | 13,510 ns | 13,449 | 13,784 | 13,743 | **335 ns, 2.5%** |
| `crossover` 1k/static (layout + paint) | 9,177 | 9,539 | 9,196 | 9,132 | 407 ns |
| `cascade_deep`, 2-rule sheet, 901 nodes | 15,459 | 15,607 | 15,383 | 15,478 | 224 ns |
| `cascade_subtree`, 90 of 901 | 1,433 | 1,449 | 1,431 | 1,480 | 49 ns |
| **`cascade_deep`, showcase-shaped sheet** (27 rules) | 66,552 | 66,600 | 66,594 | 66,842 | 290 ns |

The pilot passes the frame threshold if its median over four runs sits
within **335 ns** of these.

**The showcase-shaped sheet costs 4.3× the two-rule one**, and the
two-rule figure is the one `docs/results-cascade-cost.md` has quoted as
"a stylesheet swap". Its three `Kind` rules match 540 of the 901 nodes
and its 24 `Id` rules match none, so each node pays 27 selector tests ×
4 specificity passes plus up to one `apply_to`. The two-rule bench's
`Kind("Leaf")` names no kind in the tree, which is why that number was
low: **what a swap costs depends on the sheet, and the quoted figure was
measured on a sheet that barely does anything.** Not re-quoted as a
correction there — the scoping conclusion stands at either figure — but
recorded here, where M25's cascade row reads it.

### The arena frame — one run

`just stages 10000 150 20` with `LUMEN_NO_GPU=1`, min of 150 per stage,
`List<Label<String>>` at 10 000 rows, `incremental` retention:

| workload | layout | place | paint |
|---|---:|---:|---:|
| idle | 61.9 µs | 7.6 µs | 142.6 µs |
| one row changed | 57.9 µs | 7.7 µs | 129.9 µs |

**One run, so no spread** — a second is T25.5's to take, beside the
pilot's. These stages, not the `rebuild` frame, are what a per-node look
would show in: the rebuild's layout at `all_rows` is 4,879 µs of text
shaping.
