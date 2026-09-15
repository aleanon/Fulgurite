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
pilot's.

## T25.2 — what each kind reads, measured

`crates/lumen-static-widgets/tests/style_reads.rs` renders each of 32
kinds with and without each of the 16 `Style` fields — under a fixed and
a shrink-to-content base, in every pointer state the node can actually be
in, over the instance states that change what it paints — and requires
the declared `Seed::reads` to equal what changed. The masks live in the
code beside each kind; what the measurement showed that nobody had
written down:

- **An `Either`'s own node reads nothing**, `visible` included. It
  forwards layout to its showing branch, so every rule aimed at it is a
  no-op. `Style::visible`'s rustdoc said all nodes honour it; it now says
  which does not.
- **`select` and `popup` read only `visible`**: their appearance belongs
  to their parts, which have kinds of their own.
- **A pie chart ignores `color`** — its wedges are `WEDGES`, §0's
  hard-coded palette, visible here as a missing bit.
- **Badges, chips, avatars, icons, indicators and charts read no
  `hover_dim`, `pressed_dim` or `cursor`**: no hit lands on them, so the
  host never hovers or presses them. The code resolves the dims for them
  anyway; a rule setting one does nothing on screen.
- **A `Fixed` paints `color`, not `background`.**
- **No example sheet has a rule that does nothing**: showcase 29 rules,
  glass 26, glasswall 26, the counter and the form fixtures — all zero
  findings, and injecting `Kind("Label")` into the showcase's sheet fails
  its test naming rule 1.

## T25.4 — where box fills live, and who themes a kind

### Box fills: counted, and they stay per node

Every stylesheet rule in `examples/` that sets `background`, `radius` or
`shadow`, classified by what it targets:

| target | rules | examples |
|---|---:|---|
| a **derived container** | 7 | `Screen` ×3, `Page`, `FormUi`, `Header`, the counter's `Card` (the one with a shadow) |
| a `Glass` panel | 3 | `dock` ×2, `icon` (radius only) |
| a composite's part | 2 | `Select`'s trigger (a `Button`) and menu (a `Repeat`) |
| a leaf | 13 | buttons ×6 by id or kind, three toggles and a slider by kind, a text field, a divider, a pill `Label` |

And from T25.2's measurement, **about twenty kinds read those three
fields** — every leaf with a body, `Repeat`, `ScrollView`, `Glass` and the
derive. A fill that left `Style` would need a new home in all of them, and
the derive's container has no field to hold one (plan §2.3). **Decided:
shape 1 — `background`, `radius` and `shadow` stay per node.**

What that costs the byte argument, measured by the same probe, both
targets:

| | today | all six removed | **shape 1** (`color` and the dims only) |
|---|---:|---:|---:|
| `Style` | 72 | 48 | **60** |
| `NodeState` | 76 | 60 | **72** (only `fg` leaves) |
| per node | 148 | 108 | **132** |

**Sixteen bytes a node, not forty.** The RAM case for M25 is now small;
what is left to justify it is the status look and the extra colours a
widget can have, which is what T25.5's pilot has to show.

### Colour: every kind-wide rule is a palette role

Every rule in `examples/` that sets `color` or a dim by **kind** —
the only kind of colour rule that themes more than one node — uses a
colour the example already names for a role:

| rule | colour | role |
|---|---|---|
| form `Kind("label")`, `Kind("button")` | `INK` | `text` |
| form `Kind("checkbox")`, `Kind("switch")`, `Kind("slider")` | `ACCENT` | `accent` |
| showcase `Kind("button")` (the menu rows) | `INK` | `text` |

Each example also defines its own `PAGE`, `INK`, `MUTED`, `ACCENT` and (the
counter) `SURFACE` constants — a palette in all but name. **Decided:
kind-wide colour comes from the application's `Palette`; no per-kind look
table.** The remaining colour rules are by **id** — 20 of them: card text
on coloured glass (`CARD_INK`, `CARD_SUB`), the counter's sign-dependent
`value_fg`, a pill's text on its own fill — and those are what a
per-instance look is for.

## T25.5 — the `Toggle` pilot, against the thresholds

Measured 2026-09-15 on the pilot as built: `Toggle` with a `look`
function and a stored resolved `ToggleLook`, resolved in `layout` from
the palette and `Status`. Same machine and procedure as T25.1.

| quantity | T25.1 baseline | pilot | threshold | verdict |
|---|---:|---:|---|---|
| embedded `.text` | 26,252 B | **26,080 B** (−172) | ≤ +300 B | **pass** |
| the same, with `dim`'s early return | 26,252 B | **26,144 B** (−108) | ≤ +300 B | **pass** |
| RAM, table tree: `Ui` on the target | 1,104 B | **1,168 B** (+64) | must not grow | **trips** |
| 1 000 toggles, layout + paint, 4 medians | 13,449–13,784 ns | **18,069–18,248 ns** | within 335 ns of baseline | **trips** (+4.5 µs, +33%) |
| the same, `ops::dim` returning early at a zero shade | — | **17,392–17,480 ns**, one run 19,017 | within 335 ns | **trips** (+3.6 µs, +27%) |
| arena frame, 10 000 rows, one-row layout / paint | 57.9 / 129.9 µs | 57.9 / 130.7 µs | within spread | pass — no toggle in it |

**The RAM row trips by construction**, and that is a finding about the
threshold as much as about the pilot. At T25.5 nothing has left `Style`
yet — T25.8 removes the fields — so adding a look to three toggles can
only grow `state + style + struct`. Projected to T25.8, with shape 1's
16 B a node saved on 11 nodes (−176 B) against every widget in the
fixture gaining a look (≈ +64 B toggles, +48 B two buttons, +8 B a label,
≈ +24 B a field), the end state is **roughly neutral**. The threshold
should have been written against that projection; it would not have
passed by enough to matter.

**The frame row trips on its own**, and the early return in `dim` — work
the first version did for no result — recovered a fifth of it, not the
rest. What is left is a function-pointer call and a 16-byte copy per
toggle per layout, and a `Toggle` that grew from 16 to 40 B on the host:
about 3.6 ns a toggle. Small in absolute terms; ten times the
run-to-run spread the threshold was written against.

**What the pilot bought that nothing smaller would**: a pressed look
that changes hue (`crates/lumen-static-shell/tests/toggle_status.rs`).
The coloured tick, the palette-driven body, and the reload behaviour
are all within reach of `docs/plan-appearance.md` §2.6's (a) and (b).

## T25.10 — the fallback, against the same thresholds

The pilot's look reverted and `Toggle::mark` added as a plain `Color`
field — §2.6's (a) for the one widget the pilot touched. Same fixture,
same bench, same procedure:

| quantity | T25.1 baseline | pilot (T25.5) | **`mark` field** | threshold | verdict |
|---|---:|---:|---:|---|---|
| embedded `.text` | 26,252 B | 26,144 B | **26,280 B** (+28) | ≤ +300 B | **pass** |
| RAM, `Ui` on the target | 1,104 B | 1,168 B | **1,120 B** (+16) | must not grow | grows by the field, 4 B a toggle — the price (a) states |
| 1 000 toggles, 4 medians | 13,449–13,784 ns | 17,392–17,480 ns | **13,456–13,732 ns** | within 335 ns | **pass** |

**The colour the pilot existed to deliver costs a sixth of the pilot's
bytes and none of its frame time.** What it cannot do is change with the
pointer: a pressed toggle still darkens by `pressed_dim` and nothing more.

### The rest of (a): text fields, the scrollbar, the chart

`TextInput` and `TextArea` gained `caret_color` and `selection_color`,
`ScrollView` `track` and `thumb` — each an `Option<Color>` that falls back
to today's derivation from the text colour — and `Chart` a `series`
slice defaulting to its built-in six. `CARET` was deleted: it had no user,
and the caret already painted in the text colour.

| quantity | T25.1 baseline | `Toggle::mark` only | **all of (a)** | threshold | verdict |
|---|---:|---:|---:|---|---|
| embedded `.text` | 26,252 B | 26,280 B | **26,364 B** (+112) | ≤ +300 B | **pass** |
| RAM, `Ui` on the target | 1,104 B | 1,120 B | **1,136 B** (+32) | — | the fixture's one `TextInput` carries two `Option<Color>`s: +16 B, 1.7% of its 920 |

The frame bench holds toggles, not text fields, so it does not move with
this half; the toggle row above is (a)'s frame measurement.

**The probe was wrong three times before it was right**, each caught by
reading its output against the code rather than trusting a green: a
setup assertion that every kind reads `visible` (false for `Either`);
hovered and pressed bits set on nodes no hit reaches (so a badge
"read" `hover_dim`); and a fixed base size, under which `Button`'s
`padding` — which only changes intrinsic size — measured as unread. A
mask declared from any of those would have made the lint wrong in one
direction or the other. These stages, not the `rebuild` frame, are what a per-node look
would show in: the rebuild's layout at `all_rows` is 4,879 µs of text
shaping.

## T25.11 — palette roles in a stylesheet

`Patch::color_role` and `Patch::background_role` name a `style::Role`; the
cascade resolves them against the palette it is handed, and
`Host::set_palette` re-cascades the sheet the host kept. Measured against
the T25.10 commit (`58ce0f3`), built in a worktree beside this one.

| quantity | T25.10 | **roles** | threshold | verdict |
|---|---:|---:|---|---|
| embedded `.text` | 26,364 B | **26,364 B** (+0) | ≤ +300 B over T25.1 | **pass** — the fixture's firmware links no cascade, so this measures that nothing else grew, not what a cascade on the target would cost |
| RAM, `Ui` on the target | 1,136 B | **1,136 B** | must not grow | **pass** — no widget changed |
| `Patch`, host | 108 B | **112 B** (+4) | reported | two one-byte `Option<Role>`s, rounded to the struct's 4-byte alignment |
| `Rule`, host | 136 B | **136 B** | reported | the +4 fits in padding the selector already left |

The cascade, two alternating runs of `benches --bin cascade` each, medians:

| case | T25.10 | **roles** | verdict |
|---|---:|---:|---|
| whole table, 901 nodes | 17,848–17,910 ns | **17,905–18,179 ns** | overlaps the runs' own spread |
| subtree, 90 of 901 | 1,634–1,751 ns | **1,740–1,860 ns** | overlaps |
| showcase-shaped sheet, 27 rules | 76,927–78,224 ns | **63,764–64,254 ns** | **17% faster — not claimed.** The bench's sheet names no role, so the change adds a branch per colour field and removes nothing; a speed-up it cannot cause is code placement, recorded so a later regression back to ~77 µs is not mistaken for one |

A palette swap is one of these cascades plus the full relayout
`set_palette` already cost at T25.3, on an event a user makes.

**What the showcase exercises:** `every_role_rule_follows_the_palette`
checks 414 role-decided colour fields against both of the showcase's
palettes and 54 value-decided ones for staying put, over its 18 catalogue
entries — the main table and every arena under it, since most of the sheet
styles the demo and the menu rows, which live off the table.

## T25.12 — thresholds for the cached pilot, written before it is built

Recorded 2026-09-15, so the variant `docs/plan-appearance.md` T25.12
describes can be told no. **B** is `main`, **P** the pilot re-applied to
the same base, **C** the cached variant; all three measured in one
session, alternating, four runs each.

| quantity | how | threshold | kill if |
|---|---|---|---|
| embedded `.text` | `just firmware-size`, Toggle symbols asserted linked | C ≤ +300 B over T25.1's 26,252 B — of which T25.10 has spent 112, leaving 188 over today's `main` | it exceeds that |
| RAM, table tree | `fixtures/embedded`'s `UI`, `NODE_STATE` and `STYLE` on the target | trips by construction at this step, as T25.5's did; judged on T25.5's projection to T25.8, recomputed from target sizes **with a 4-byte stamp on every migrated leaf**, against T25.1's 1,104 B | the projected end state grows |
| frame, `still` | `benches --bin appearance`, 0 misses proven untimed | C − B within the spread of B's four runs, printed beside it | it exceeds the spread |
| frame, `hover` | the same, 2 misses of 1 000 proven | C − B within B's spread | it exceeds the spread |
| frame, `palette` | the same, 1 000 misses proven | C − P within B's spread — the cache must not make the all-miss frame worse than the pilot; C − B reported | C − P exceeds the spread |
| frame, arena | `just stages 10000 150 20` | reported — no toggle in it | — |
| `Toggle<()>`, host | the bench's size table | reported beside P's 40 B | — |

**What a `still` trip at zero calls means is decided here too:** the cost
left is the widget's size, not the call. The plan does not try a third
variant for it.
