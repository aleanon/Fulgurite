# Results: the embedded profile

## The thresholds — written 2026-09-05, **before** the measurement

M13/T13.9's gate is that these are recorded first, so the gate can say
no. A profile with no measured footprint is a claim; a footprint
measured against a threshold invented afterwards is worse, because it
can only pass.

The device is T13.0's: Cortex-M4F, `thumbv7em-none-eabihf`, **128 KB
RAM, 512 KB flash**, a 320×240 SPI panel with its own GRAM and no
framebuffer.

| | Threshold | Why that number |
|---|---:|---|
| Framework `.text` | **≤ 80 KB** | 15.6% of flash. **Raised from 64 KB (an eighth) by the owner on 2026-09-30** (D5, `docs/plan-iced-roadmap.md`): the interactive firmware stood at 94.5% of 64 KB with 3,614 B left, against roughly 9.8 KB of planned growth (M80–M90). 80 KB is still under this row's own line: the app, its face and its data need the rest, and a GUI framework that takes more than a fifth of the part is not an embedded GUI framework. `scripts/check_text.sh` reads this row and fails when its default disagrees. |
| Framework `.bss` + `.data` | **zero symbols**, exemptions named | The framework's *own* statics. **Tightened from "≤ 2 KB" on 2026-09-07**: a bound nothing had approached told nobody anything, and what the number was standing in for is a *shape* — no allocator arena, no lazily-initialised registry, no static caches, nothing claiming RAM before `main`. `scripts/check_bss.sh` checks that, and `scripts/bss.allow` names what is allowed and why. |
| RAM for a 100-node tree | **≤ 24 KB** | T13.0's budget said ~19 KB for state + style + widget structs. 24 KB leaves headroom and still fits with the 8 KB stack in well under half of RAM. |
| Stack, actual depth | **≤ 4 KB** | T13.0 measured 256–272 B per level and a real screen at 4–5 deep (~1.6 KB). 4 KB is generous for that and half the 8 KB budgeted. |
| `f64` in the frame path | **zero** | The FPU is single-precision, so any `f64` becomes soft-float calls. A symbol count in `scripts/check_text.sh` since M52/T52.2, not prose (R11-F2). |
| The fixture builds and runs | **yes** | The binary answer to "zero allocation, no allocator required". |

Recorded threats to these numbers, so a pass cannot be read as more than
it is: `cargo check` proves none of them — it type-checks and stops, with
no codegen, no link, no `.text`. Section sizes below come from a real
`cargo build --release` for the target.

## The headline — re-measured 2026-09-27 (M52/T52.0)

**This section is the current measurement, and M52's gates cite it
rather than restating it** (`docs/plan-embedded-footprint.md` §0 rule 3).
`scripts/check_text.sh` holds the paint-only `.text` row below to the
linked firmware: it fails when the build differs from it by more than
1%. This document was caught three times with a headline a milestone
behind the binary (the history is below, dated); the ceiling was gated
and the number beside it was a habit. When a change moves `.text` past
the tolerance, the gate asks for this table to be re-measured and
rewritten in the same commit.

`just firmware-size`: a **linked** `thumbv7em-none-eabihf` binary
(`fixtures/embedded/src/bin/firmware.rs`), release, one frame of the
fixture with a baked face. Not a board bring-up — no memory map, no
vector table, no peripheral — but a real link, which is the only thing
that produces real sections.

| | Threshold | Measured | |
|---|---:|---:|---|
| `.text` | ≤ 80 KB | **30,226 B** (29.5 KB) | **pass**, at 36.9% of budget; re-measured at M81/T81.5 (2026-10-04; the four-binary table below, +860 B), against the 80 KB ceiling since M80/T80.0 |
| `.bss` + `.data` | zero framework symbols | **1 symbol, 4 B, exempted** | **pass** — the fixture's own `SINK` (`scripts/bss.allow`) |
| `.data` | 0 B | **0 B** in all four binaries | **pass**; asserted by `scripts/check_bss.sh` since M52/T52.3 — the interactive and bind-only builds had 4 B, one binding's cache (F-NEW-1) |
| `.rodata` | — | 13,316 B | of which the baked face is most; 9,010 B at T13.7; re-measured at M81/T81.5, +40 B (panic locations of the `caret` module) |
| Total (`size -A`) | ≤ 512 KB flash | **43,759 B** (42.7 KB) | 8.3% of the part; re-measured at M81/T81.5 |
| RAM, 100 nodes | ≤ 24 KB | **16.0 KB** state + style | **pass**; measured at M27/T27.1 (2026-09-18), below |
| Stack, release | ≤ 4 KB | **1,192 B** a cold frame (model 1,328) | **pass**, measured under qemu by `just stack` since M52/T52.6; dev 6,996 B, recorded not gated — below |
| `f64` in the frame path | zero | **0 soft-f64 symbols**, and 0 float-formatting | in all four binaries, counted by `scripts/check_text.sh` (M52/T52.2); the only `f64` is `observe`'s accesskit call, which needs `std` |
| Fixture builds and runs | yes | **yes** | links for the target; runs on the host with an aborting allocator |

The 2026-09-26 headline read 27,732 B `.text` and 12,860 B `.rodata`; the
build had moved +236 B (+0.85%) since, inside the gate's 1%, so nothing
asked for it to be rewritten. M52/T52.0's rule is that the budgets are
re-measured before any of its tasks moves them, whatever the tolerance.

### The binaries a device actually is — M52/T52.0

**The paint-only firmware above never dispatches, binds or cascades**
(R11-F4, `docs/plan-embedded-footprint.md` §1.1), so the gates reading it
could not see M29 and M33 grow binding and dispatch. Three more binaries
link what a device does, over the same tree (`fixtures/embedded/src/lib.rs`
writes it once, for `Panel` and for `Device`, whose tree adds one bound
label). Same target, same release profile, measured 2026-09-27 at the
commit that adds them, before any M52 task changed the core:

| binary | what it runs | `.text` | of 64 KB | `.rodata` | `.data` | total (`size -A`) |
|---|---|---:|---:|---:|---:|---:|
| `firmware` | one frame | 27,968 | 42.7% | 12,864 | 0 | 41,049 |
| `firmware_interactive` | cascade, press + release, a typed character, one `#[bind]`, `Boundaries<16>` | **52,814** | **80.6%** | 19,410 | **4** | 72,445 |
| `firmware_bind` | one `#[bind]` update, no dispatch | 46,610 | 71.1% | 18,042 | **4** | 64,873 |
| `firmware_cascade` | `cascade_deep_with`, no dispatch | 43,886 | 67.0% | 17,926 | 0 | 62,029 |

**Re-measured after rebasing M52 onto M48 and M51** (2026-09-27, before
T52.4): the paint-only figures above did not move; the other three each
grew by ~3.7 KB of `.text` and ~2.2 KB of `.rodata`:

| binary | `.text` | of 64 KB | `.rodata` | total (`size -A`) |
|---|---:|---:|---:|---:|
| `firmware_interactive` | **56,486** | **86.2%** | 21,598 | 78,305 |
| `firmware_bind` | 50,282 | 76.7% | 20,230 | 70,733 |
| `firmware_cascade` | 47,618 | 72.7% | 20,114 | 67,949 |

Every new named symbol is `TextInput`'s word movement
(`move_word_left`/`move_word_right`, `edit::next_word`/`prev_word`/`word_at`)
and the Unicode `alphabetic` and `n` tables its word test reads. The
paint-only firmware links none of it, because nothing there reaches
`TextInput::on_event`. The interactive build pays it as the real price of
a keystroke; **the bind-only and cascade-only builds pay it through the
vtable alone** — R12-F4 and F-NEW-2 made visible a second time, by code
neither binary can call.

**At the end of M52** (2026-09-27, T52.3–T52.5 and T52.7 landed, on
M48 and M51) — the figures the gates held then; M55/T55.5 has since moved
the cascade and interactive rows (its section below):

| binary | `.text` | of 64 KB | `.rodata` | `.data` | total (`size -A`) | forbidden symbols |
|---|---:|---:|---:|---:|---:|---|
| `firmware` | **27,964** | 42.7% | 12,864 | 0 | 41,045 | none |
| `firmware_interactive` | 55,922 | 85.3% | 21,194 | 0 | 77,337 | (dispatch is its price) |
| `firmware_bind` | 31,512 | 48.1% | 13,428 | 0 | 45,161 | 0 `hit`, 0 `on_event` |
| `firmware_cascade` | 29,304 | 44.7% | 13,828 | 0 | 43,349 | 0 `hit`, 0 `on_event` |

The paint-only `.text` moved −4 B over the milestone (alignment; no
symbol it links changed). The `alloc`-tier cascade record now prints 10
`on_event` symbols where T52.4's commit printed 17: identical-code folding
merges more of the default methods once the bind walk stopped
instantiating them, and the record is not a gate.

What each links that it should not, counted with `nm -C` (at the commit
that added them; the counts did not change with the rebase):

| binary | `__ln_site` in `.data` (F-NEW-1) | `Node>::hit` (F-NEW-2) | `on_event` (R12-F4) | float formatting / soft `f64` |
|---|---:|---:|---:|---:|
| `firmware` | 0 | 0 | 0 | 0 / 0 |
| `firmware_interactive` | 1 (4 B) | 10 | 17 | 0 / 0 |
| `firmware_bind` | 1 (4 B) | **10** — it hit-tests nothing | 17 | 0 / 0 |
| `firmware_cascade` | 0 | 10 | **17** — it dispatches nothing | 0 / 0 |

The interactive build is **80.6%** of the ceiling, not the 88.6% the
re-triage measured: its probe was not committed, and these binaries are
rebuilt from the plan's description, so the two are different programs.
The *shape* reproduces exactly — each finding shows in the binary that
should exhibit it, and in no other.

**The release window** (F-NEW-3): `examples/minimal`, `cargo build
--release`, the window crate's default features, no `reload`, no agent.
`.text` 2,743,702 B, of which **14 `lumen_static_shell::recover::`
symbols, 13,964 B**, where rule 3 of `docs/plan-panic-recovery.md` says
there should be none — the re-triage's count and byte total, to the
byte. With `--features lumen-static-winit/reload` the count is the same
14, which is the finding: the `cfg` decides behaviour and not the link.
`just release-window-shape` holds both, marked expected red until T52.7.

**Closed by T52.7** (on the tree rebased onto M48 and M51): the policy is
a type, `session::RecoverPolicy`, and a release window without `reload`
links **0** symbols defined in `recover::` — **.text 2,714,086 B**,
against 2,740,854 B with the recovering marker forced into the same
build: **26,768 B** of `.text`, twice the recover module's own 13.3 KB,
because the guarded arms' `catch_unwind` paths go with it. With `reload`
the count is 8 symbols, 13,328 B, as it must be. What remains is the drop
glue of `Host`'s always-`None` failure field (3 × 118 B), which is data
every host carries under `panic = "unwind"`; the gate counts symbols
*defined* in the module and prints the glue beside the count.

**The gates read all four since T52.1** (R11-F4). `scripts/check_text.sh`
prints each binary's `.text` against the same ceiling (64 KB until
2026-09-30, 80 KB since, D5) — the interactive build keeps it, because an application that handles input
is the ordinary case and not a larger product
(`docs/plan-embedded-footprint.md` §2.5); needing more is a re-budget
argued here. Before reading a number, each binary asserts the code it
exists to price is linked: the interactive one counts `pass::dispatch`,
`on_event`, `cascade_range` and `BindSite`, and a build with its
dispatch calls deleted fails with `0 pass::dispatch symbols linked`.
`scripts/check_bss.sh` runs the zero-framework-statics check on each and
names a static by its **full** demangled name; it used to cut the name at
its first space (`<embedded::Device`), so no exemption could have matched
the binding cache. The three findings in the second table above are
committed as expected red, each naming the task that turns it green.

### A binding's cache claims no flash — M52/T52.3

The `4` in the `.data` column above was F-NEW-1: the derive's per-binding
`BindSite` static began at `usize::MAX`, so it was initialised data —
stored in flash, copied to RAM before `main`. `BindSite` now stores the
index plus two, with `0` for "not looked up" and `1` for "nowhere", so a
new site is all-zero and lands in `.bss`: **`.data` is 0 B in all four
binaries**, and the interactive and bind-only builds each carry one 4 B
binding cache in `.bss`. It is still a framework static, the only one
this profile has, so `scripts/bss.allow` exempts it **by pattern**
(`glob: *::update_marked::__ln_site`) with the argument — bounded by the
source text, one per `#[bind]`, 4 B of zeroed RAM each — and
`scripts/check_bss.sh` prints `1 binding cache(s) (4 B, exempt by
pattern)` per binary so the exemption is visible. Putting "unknown" back
at `usize::MAX` fails the `.data` assertion at 4 B on both binaries;
removing the pattern lists the static by its full name.

### A stylesheet costs the cascade, not a vtable — M52/T52.4

`cascade_deep_with`, `cascade_arenas_deep` and the scoped cascade's
per-root loop each called `tree.node_mut(i)` for every node and handed the
`&mut dyn Node` to `cascade_arenas`, whose body was compiled out without
`alloc` — so the loop did nothing and the coercion linked the whole
`Node` vtable (R12-F4). The three loops, and `cascade_arenas` itself, are
gated on `alloc` now. `scripts/check_text.sh` gates the leaf tier and
records the `alloc` tier:

| cascade-only firmware | `.text` | paint-only | delta (bound 3,072) | `on_event` / `Node>::hit` |
|---|---:|---:|---:|---:|
| leaf, before (after the M48/M51 rebase) | 47,618 | 27,968 | 19,650 | 17 / 10 |
| **leaf, after** | **29,308** | 27,968 | **1,340** | **0 / 0** |
| `alloc` tier (record, not gated) | 57,378 | 27,964 | 29,414 | 17 / — |

**The `alloc` tier still pays**, as §2.4 of
`docs/plan-embedded-footprint.md` says it would: there, arenas exist and
the loop must ask every node whether it owns one, through the vtable,
until a derive-computed "has arenas" constant lets it skip the types that
cannot (M26 Phase D finding 9, promoted and not built). Putting the loop
in `cascade_deep_with` back un-gated returns the leaf build to 47,618 B,
17 `on_event` and 10 `hit`, and the gate fires on all three.

### The cascade repairs lengths — M55/T55.5 (2026-09-27)

The cascade now asks each rule whether it carries a length that cannot
be laid out, and runs a cold repair pass when one does
(`docs/results-derive-dx.md` T55.5). The firmware links both for every
build that cascades; the paint-only headline does not move. Leaf tier,
`scripts/check_text.sh`:

| binary | `.text` before | after | delta |
|---|---:|---:|---:|
| `firmware` (headline) | 27,964 | 27,964 | 0 |
| `firmware_cascade` | 29,304 | **30,184** | +880 (delta over paint-only 1,340 → **2,220**, bound 3,072) |
| `firmware_interactive` | 55,922 | **56,802** | +880 (86.7% of 64 KB) |

**Three shapes were built to get there**, each measured on this table: a
repaired copy of every applied patch (+1,504 B on both), the same with
`apply_to` out of line (+960), and a pass against each kind's own
defaults (+964, the defaults are a `fn` call per node). The shipped pass
compares against `Style::default()`'s constants, and the per-sheet
question (`Patch::needs_repair`, 484 B) is written apart from the lint's
field-naming `Patch::repairs`, which firmware does not link.

### A binding links no vtable — M52/T52.5

`bind::node_of` coerced every table node to `&dyn Node` to read its
address, size and kind (F-NEW-2). It asks `Widget::bind_walk` now, which
the derive answers on concrete types. The bind-only firmware, before and
after (leaf tier, after the M48/M51 rebase and T52.4):

| bind-only firmware | `.text` | `Node>::hit` / `on_event` | binding's own symbols (bound 1,024) |
|---|---:|---:|---:|
| before | 50,282 | 10 / 17 | — |
| **after** | **31,512** | **0 / 0** | **778 B** |

**The plan's bound was the wrong quantity.** It asked for the bind-only
binary within 1,024 B of the paint-only one; it is 3,548 B over, and the
symbol diff says why: the bind-only binary holds a *different tree* —
`Device`'s, one more label, whose layout, place and paint are ~2.2 KB
larger than `Panel`'s — and runs a second frame. What a binding itself
links (`bind::format_into`, `BindSite::get`, `update_marked`,
`TextTarget::set_str`) is **778 B**, and that is what `check_text.sh`
bounds at 1,024, printing the whole-binary delta beside it as a record.
The `u32` formatting a `"{}"` binding needs (`core::fmt::write` and
friends, ~0.7 KB) is outside both: it is the format string's price, and
a device that formats anything pays it. Routing the derived walk back
through `&dyn Node` puts 13 `hit` and 17 `on_event` symbols back and the
binary at 50,338 B; the gate fires on all three counts.

### The stack, measured — M52/T52.6

**The stack row reported a model as a measurement** (R11-F3): it was
`stack_bytes(DEPTH)`, depth × 272 B, and the fixture asserted that model
at compile time. `fixtures/embedded/src/bin/fwq.rs` runs under qemu
(`-M mps2-an386`, a Cortex-M4F), paints the stack before each call and
scans for the deepest word the call overwrote; `just stack`
(`scripts/check_stack.sh`, in `check-extra`) holds the figures. Measured
2026-09-27 on the core T52.0 measured (T52.1 and T52.2 changed scripts
and the fixture only), `Device`'s tree, depth 3:

| call | release | dev (recorded, not gated) | model before | model now |
|---|---:|---:|---:|---:|
| calibration: a known 2,000 B local | **2,032** (bound 2,000–2,100) | 2,204 | | |
| cold frame | **1,192** | 6,996 | 816 | 1,328 |
| warm frame | 1,072 | 6,724 | 816 | 1,328 |
| a typed character (`pass::dispatch`) | 556 | 3,332 | | |
| a bound update (`update_marked`) | 552 | 984 | | |

The depth sweep — `Panel`'s `Ui` wrapped in one, two and three more
columns, a cold frame each, release:

| depth | 3 | 4 | 5 | 6 |
|---|---:|---:|---:|---:|
| measured | 1,184 | 1,384 | 1,600 | 1,816 |
| `depth × 272` (the old model) | 816 | 1,088 | 1,360 | 1,632 |
| `512 + depth × 272` (the model now) | 1,328 | 1,600 | 1,872 | 2,144 |

A least-squares line through the sweep is **211 B per level plus
546 B**: the slope is under the 272 B the model already used, and the
old model had **no intercept**, so it was below the measurement at every
depth — by 368 B at depth 3. `desc::STACK_BASE_BYTES` is 512: the largest
residual over the 272 B slope (376 B, the device's cold frame) rounded up
for toolchain headroom. `just stack` fails if any release frame stops
fitting under the model, so the constant cannot drift back into a claim.

**What the gate catches that the assert cannot.** The review's `937885b`,
a 4 KB local in `TextInput::layout`: the cold frame measures **5,096 B**
(1,000 over the budget; the review measured 5,016 on its probe), `just
stack` fails naming `cold frame` and `warm frame`, and the compile-time
assert stays green because `DEPTH` did not change. Skipping the paint
makes the calibration read 61,688 B and fail before any figure is read.

**The budget is a release budget.** The dev profile's frame is almost six
times the release one, over 4 KB at every depth; a dev firmware for this
part needs `opt-level = 1` for the `embedded` package
(`docs/plan-embedded-footprint.md` §2.6). The qemu figures are a model of
a Cortex-M4F, not a board: the exception frame and lazy FPU stacking a
real interrupt adds are not in them, and a board run is owed, not
claimed.

`Boundaries<256>` does not reach this profile: it is only `Host`'s
default, and `Host` is `std`.

**`#[bind]` is exercised without `std`** (M43/T43.8):
`fixtures/embedded/tests/bind_nostd.rs`, run by `just thumb` with the
fixture built alone, stores exactly 256 bytes of a 1,000-byte bound value
and cuts at a `char` boundary (255 bytes when the 256th is inside an
`é`). It is a test, not part of the fixture's library, so the firmware
above does not change. Until then no gate ran core's `no_std` bind branch
at all (R12-F2).

## The first measurement — 2026-09-05 (history)

The table as first recorded. Its "now" annotations were dropped when the
headline above replaced them; the rows in between live in the dated
sections below.

| | Threshold | Measured 2026-09-05 | |
|---|---:|---:|---|
| `.text` | ≤ 64 KB | 20,524 B (20.0 KB) | pass, at 31% of budget |
| `.bss` + `.data` | zero framework symbols | 1 symbol, 4 B, exempted | pass — the only static is the fixture's own harness |
| `.rodata` | — | 12,940 B | of which 9,010 B is the font face (T13.7) |
| Flash, total | ≤ 512 KB | 33,677 B (32.9 KB) | 6.4% of the part |
| RAM, 100 nodes | ≤ 24 KB | 14.5 KB state + style | pass; widget structs are the app's, below |
| Stack, actual depth | ≤ 4 KB | 816 B at depth 3 | pass, and asserted at compile time since 2026-09-07 |
| `f64` in the frame path | zero | zero | the only `f64` is `observe`'s accesskit call, which needs `std` |
| Fixture builds and runs | yes | yes | links for the target; runs on the host with an aborting allocator |

Every threshold met. Three things worth more than the pass.

### One `f32::clamp` cost 26.5 KB — 44% of the firmware

The first link was **43,224 B of `.text`**, and a third of it was
`core::num::flt2dec` — the float *formatting* machinery, in a GUI that
formats no floats. The panic string in the binary named the culprit:
`f32::clamp` panics when `lo > hi` or a bound is NaN, and its message
formats a float.

`lumen_static::num::clamp` replaces it at all nine call sites — one of
them `Constraints::clamp`, which runs for every node of every frame:

| | before | after | |
|---|---:|---:|---|
| `.text` | 43,224 B | **20,524 B** | −52.5% |
| `.rodata` | 16,736 B | **12,940 B** | −22.7% |
| total | 60,173 B | **33,677 B** | **−26,496 B** |

The replacement matches `f32::clamp` everywhere `f32::clamp` returns,
NaN included; where `f32::clamp` panics it is total instead, which is
the better failure for a frame loop. **This is the finding that
justifies the task**: it is invisible on a desktop, where 26 KB is
nothing and nobody links for a 512 KB part, and no amount of
`cargo check` would have shown it.

**Gated by a count since M52/T52.2** (R11-F1). Until then the only thing
that could catch a return of this was the `.text` ceiling, and a
paint-only firmware stays under it with the 26.5 KB back in (the review
measured 48,720 of 65,536, exit 0). `scripts/check_text.sh` counts
`core::num::flt2dec` and `core::fmt::float` symbols, bound 0, in every
binary: the same `f32::clamp` edit gives **18** in each, and the
cascade-only binary's `.text` (64,840 B) is still under the ceiling
while its count is red. Formatting one `f32` with `Display` gives 17.
Soft `f64` is a separate count (the `__aeabi_d*`, `__*df*` and
`__aeabi_*2d` names, demangled `compiler_builtins` paths included): an
`f64` round-trip in `Constraints::clamp` gives **6**. Both are planted
for re-runs (`scripts/selftest/plants/build/text-float-fmt.sh`,
`text-soft-f64.sh`).

### Re-measured 2026-09-10 — and the ceiling is a gate now

**The 2026-09-05 figure above drifted 9.4% and nothing noticed**, because
the `≤ 64 KB` threshold was stated in this document and enforced by
nothing. `scripts/check_bss.sh` has held `.bss` since 2026-09-07; there
was no equivalent for `.text`. There is now —
`scripts/check_text.sh`, in `just thumb` beside the `.bss` gate — and it
prints the measurement next to the ceiling rather than only "pass", so a
closing margin is visible in the output.

Found while measuring something else: whether growing `Style` by 4 bytes
would cost the embedded profile anything (`docs/plan-glass.md`'s T18.7
question). It does not — but the baseline it was measured against turned
out not to match this file.

| | `.text` | Δ | of the 64 KB ceiling |
|---|---:|---:|---:|
| 2026-09-05, T13.9 | 20,524 | — | 31.3% |
| end of M15 (`8109ad2`) | 21,970 | **+1,446** | 33.5% |
| end of M16 (`959cf2c`) | 22,458 | **+488** | 34.3% |
| end of M17 (`59a45ef`) | 22,458 | **0** | 34.3% |
| 2026-09-10 | 22,458 | | 34.3% |
| M18 + M19 (`ee220cb`…`f02c5cb`) | **22,606** | **+148** | **34.5%** |

`.rodata` moved 12,940 → 13,016 (+76) and flash total 33,677 → 35,691
over the same span. `.bss` is unchanged at one exempted 4-byte symbol,
and the per-node RAM figures below are unchanged — `NodeState` is still
76 B and `Style` still 72 B, both still `const`-asserted.

**M18 and M19 together cost 148 bytes** — `Damage`'s region set, the
`Cmd::Glass` arm in `hash_into`, and `display::glass_source`. The blur
itself is not there at all: it lives in `lumen-static-shell`, which an
embedded profile does not link, so a part that cannot render a backdrop
filter pays for the command's *identity* and nothing else. `.rodata`
moved 13,016 → 13,032 and `.bss` is unchanged at one exempted 4-byte
symbol. That is T19.6, and the claim it was there to check.

**M17 cost the firmware exactly nothing**, which is the tiering claim
holding up under measurement rather than in argument: T17.0–T17.5 landed
entirely in `lumen-static-shell`, which an embedded profile does not
link. M14 and M15 together account for three quarters of the growth
(+1,446) and M16 for the rest (+488) — the core gained `Cmd::Line`,
`Cmd::Gradient`, `Cmd::Image`, overlays, popups, the focus ring and the
cursor in M15, and invalidation and paint-side culling in M16. Not
attributed below the milestone; the per-milestone split is what the gate
needs and what a reader can act on.

**What growing `Style` would cost, since it was measured here** (probe
applied and reverted, 2026-09-10):

| `Style` | `.text` | Δ flash | RAM/node | 100 nodes |
|---|---:|---:|---:|---:|
| 72 B (today) | 22,458 | — | 148 B | 14.5 KB |
| 76 B (+1 `f32`) | 22,494 | +36 | 152 B | 15.2 KB |
| 92 B (+20 B, a full glass parameter set) | 22,694 | +236 | 168 B | 16.4 KB |

Frame time at 10 000 nodes was unaffected in either direction — four
runs each, min-of-150, distributions fully overlapping (rebuild
5,202–5,442 µs baseline against 5,169–5,460 µs at 76 B). The cost of a
per-node field on this profile is RAM per node, not flash and not
frame time.

### Re-measured 2026-09-15 — the fixture holds toggles now

**The baseline had drifted again**: the linked `.text` was **21,278 B**,
not the 22,606 recorded above, and `.rodata` 12,704. Not attributed —
M20 to M24 landed between the two measurements.

Then M25/T25.1 added one `CheckBox`, one `Radio` and one `Switch` to the
fixture's tree, because M25's pilot migrates `Toggle` and stops on a
flash threshold, and the firmware linked **zero** Toggle symbols — so
that threshold would have measured a binary the pilot's code could not
reach (`docs/plan-appearance.md` §5):

| | `.text` | `.rodata` | total | Toggle symbols |
|---|---:|---:|---:|---:|
| before, 8 nodes | 21,278 | 12,704 | 34,199 | 0 |
| with three toggles, 11 nodes | **26,252** | 12,752 | 39,221 | 3 |

**Three toggle shapes cost 4,974 B of `.text`**, 7.6% of the ceiling.
Each `SHAPE` is its own instantiation (`docs/results-monomorphization.md`).
`scripts/check_text.sh` now fails when no Toggle is linked, verified by
removing the three fields: the fixture still compiled and the script
exited 1 naming the missing code. The fixture's `Ui` is 1,104 B on the
target and asserted there; M25's own figures are in
`docs/results-appearance.md`.

### Target sizes, asserted rather than remembered

Measured on `thumbv7em-none-eabihf` and held by `const` assertions in
`fixtures/embedded/src/lib.rs`, so the build fails if a field is added
rather than the budget quietly drifting — which is what happened to
these figures once already (see the 72/64 correction below).

- `NodeState` **76 B**, `Style` **72 B** → **148 B per node** of RAM.
  100 nodes = 14.5 KB.
- `NodeDesc` **32 B on the target**, against 48 on a 64-bit host, as
  this file predicted. It is `const` data in flash, not RAM.
- The fixture's `Ui` is **1,064 B** for 8 nodes — but **768 B of that is
  one `InlineClusters<64>`**, at 12 B per cluster. The
  `TextBuffer::Clusters` pairing takes one cluster per byte, which is
  the correct worst case and an expensive one: an embedded app with a
  text field should size `InlineText<N>` to the field, not to a round
  number.

### `.bss` is zero symbols now, not two kilobytes

The old threshold was `≤ 2 KB` and measured 0 — **a bound nothing had
approached, which tells a reader nothing**. What it was standing in for
is a shape rather than a size: this framework has no hidden global
state, no allocator arena, no lazily-initialised registry, no static
caches. The day that stops being true is the day the number matters,
and 2 KB of headroom would have hidden it.

So it is **zero framework symbols**, with `scripts/bss.allow` naming
what is allowed and arguing for it. `scripts/check_bss.sh` enforces it
against a linked binary in `just check`, and was verified by injecting
a framework static the frame path writes — it names the offender and
exits 1.

**Making the check honest exposed a defect in the fixture.** `SINK`, the
static `firmware.rs` parks the frame checksum in "so the whole frame
cannot be optimized away as dead", **had no symbol in the linked
binary at all**: as a private static never read, LLVM removed the store.
The frame survived on panic paths instead — true by accident, which is
the worst way for a published number to be true. It is `#[no_mangle]
pub` now, the guard does what its comment says, and it is the one
exemption on the list.

Confirmed separately that the measured `.text` is not dead code:
removing the `frame()` call collapses it from 21,946 B to **1,438 B**.

### The stack budget is enforced now, not just recorded

Until 2026-09-07 this row was a threshold nobody checked: the number was
written down, the depth was measured once by hand, and the two were
never connected. `desc::stack_bytes(depth)` connects them, and
`fixtures/embedded` asserts it **at compile time** against the tree that
actually has to fit — 816 B at depth 3, verified by re-injection
(lowering the bound to 512 fails the build).

**A depth backstop since M52/T52.6, and not the measurement.** The 816 B
was the model's output, reported here as measured; the real release frame
was 1,192 B, and a widget whose own frame grows leaves this assert green.
`just stack` measures under qemu, and the model gained an intercept
fitted from its sweep ("The stack, measured", above).

Against a tree's **actual** `DEPTH`, not `MAX_DEPTH`. Those are
different numbers, and conflating them is the mistake the next section
describes.

### `MAX_DEPTH` stays 64, and the reason is a measurement

T13.0 said the embedded profile "lowers the cap 64 → 24". It does not,
because `MAX_DEPTH` is a **compile-time assertion with no runtime
cost** — the derive checks `DEPTH <= MAX_DEPTH` and nothing allocates
against it. Lowering it would only reject trees, and would reject them
on desktop too. What is true is the budget: at T13.0's 256–272 B per
level, 64 levels would be ~17 KB against an 8 KB stack, so an embedded
app should stay under ~30 deep. Every real tree here is 4–5.

## What is already known, 2026-09-04

Not results, just the starting position, measured while planning M10:

- The core is **9 `std::` references** from building without `std`.
- **`NodeState` is 76 B, `Style` 72 B, `NodeDesc` 48 B** — measured
  here with `size_of` on x86-64, default features, 2026-09-04. A
  200-node tree is therefore **29.6 KB** of state and style, before any
  display list or the widget structs themselves.

  Corrected the same day: this file first said 72/64/48 and cited
  `docs/results-frame-cost.md`, **which contains neither figure**. The
  numbers pre-dated C6, which added `pressed`, `hovered` and `focused`
  to `NodeState`. `scripts/check_doc_paths.sh` passed throughout,
  because it verifies a cited file *exists* and not that it says the
  thing — which is the gap in AGENT.md's numbers-stated-once rule that
  this correction exists to record.

- **`NodeDesc` is 48 B only on a 64-bit host, and only without
  `observe`.** It holds a `&'static str` and a `&'static [&'static str]`,
  so it is ~32 B on a 32-bit target; `observe` grows it to 72 B
  (`crates/lumen-static-agent/Cargo.toml`). The profile this document
  is about is the 32-bit, no-`observe` one, so **sizes must be
  re-measured on the target** rather than carried over from here.

- **The widget structs are a third resident array** and no earlier
  version of this file counted them. They are not free: a `Label` is
  larger than a `Fixed`, and the per-node budget is state + style +
  struct, not state + style.
- The alloc-free path **half** exists. `pass::layout`/`paint`/`hit_test`
  take slices, and the tests own `[NodeState; N]` arrays directly. But
  there are **zero** `[Style; N]` array literals in the workspace: all
  84 sites call `pass::default_styles`, which returns a `Vec`. There is
  no non-allocating way to construct a style array today, and adding
  one is part of the milestone rather than a starting position.

~~Until the first three bullets have numbers, the claim "zero
allocation, the microcontroller profile" remains **unproven** and should
be written as an intention wherever it appears.~~ **Measured 2026-09-05,
above.** The claim is now a number and may be written as one.

One correction to the starting position, since it was wrong in a way
worth recording: it said "there are **zero** `[Style; N]` array literals
in the workspace: all 84 sites call `pass::default_styles`". True when
written; `pass::default_styles_into` (T13.3) is what changed it, and the
embedded fixture is the first caller to own its style array.

One figure that is not a measurement of this framework but sets every
budget here: a 320×240 16 bpp **framebuffer is 150 KB**, and a full
frame over a 40 MHz SPI bus takes **31 ms**. The first is why the
profile keeps no framebuffer; the second is why damage matters more on
this device than on a desktop (T13.8).

## M27/T27.1 — what the border cost, measured 2026-09-18

The border is a `Style` property (`docs/plan-widget-parity.md` T27.1
settles why). Twelve of M27's thirty-three widgets are bordered controls
and the framework could not draw one: no field, and no command.

**Measured before and after on the same day and toolchain**, by stashing
the change rather than trusting the figure this document already carried:

| | `.text` | `.rodata` | total |
|---|---:|---:|---:|
| before (`5c7f6db`) | 26,396 | 12,784 | 39,397 |
| **after T27.1** | **27,072** | **12,804** | **40,093** |
| Δ | **+676** | **+20** | **+696** |

### M27/T27.11 — `Fields` widened to a `u32`, measured 2026-09-19

Zero. `.text` 27,072, `.rodata` 12,804, total 40,093 — byte-identical
before and after, `git stash` on either side of the same link.

`Fields` grew from `u16` to `u32` to hold a seventeenth bit
(`Fields::BORDER`; the first sixteen were taken). It is a field of
[`Seed`](../crates/lumen-static/src/style.rs), which also holds a
`&'static str` and a `fn` pointer, so the extra two bytes fall inside
padding the struct already had. `Event::Hover` cost nothing either:
`Event` is still 16 bytes, asserted by `tests/gesture.rs`.

Recorded because "it was free" is a measurement, and the alternative is
the next reader re-deriving it — or assuming it was not.

`.text` is now **41.3% of the 64 KB ceiling**, from 40.3%. `.bss` is
unchanged at one exempted 4-byte symbol.

**The baseline in the table above had drifted again, and by more than
last time.** That table's last row is 22,606 for M18+M19 and the prose
below it says 22,458; the actual linked `.text` at `5c7f6db` is
**26,396** — a **16.8%** understatement, accumulated across M22, M24,
M25 and M26, none of which re-measured. This is the third time this
document has been caught with a stale baseline, and the pattern is the
same each time: `scripts/check_text.sh` gates the *ceiling*, so drift
below it is invisible. The ceiling is the gate; a per-milestone row is a
habit, and the habit lapsed.

### RAM, which is the real cost

| | before | after | Δ |
|---|---:|---:|---:|
| `Style` | 72 B | **80 B** | +8 |
| `NodeState` | 76 B | **84 B** | +8 |
| **per node** | 148 B | **164 B** | **+16 (+10.8%)** |
| 100 nodes | 14.5 KB | **16.0 KB** | +1.5 KB |

**Eight bytes twice, and neither was avoidable at this design.** Both
structs' tails were *exactly* full: `Style`'s `align`/`focus_ring`/
`cursor`/`visible` and `NodeState`'s three `bool`s plus `focus_ring`
each fill one 4-byte slot with nothing spare, so `border_width: u8` cost
four bytes in each and `border_color: Color` four more. The `u8` is kept
for its semantics — a border wider than 255 px is not a border — not for
a size it does not buy.

`NodeState` grew because `Node::paint` has no `&[Style]` and must not
re-cascade: the border travels to paint the way `bg` and `radius` do.

**Two ways to get half of it back, both declined here and both
measurable:**

- **Pack `NodeState`'s three `bool`s into a flags byte.** That frees
  three bytes, `border_width` fits, and `NodeState` stays 80 rather than
  84. It changes `st[i].pressed` at every read site.
- **Move `align` out of `Style`.** Only containers read it — every
  reader is in the derive's container layout. But `i18n::mirror_styles`
  rewrites it in one flat pass over the style array, and per-widget
  alignment makes RTL mirroring a tree walk through a trait method every
  custom container must remember to implement.

Both are packing exercises with real API cost, and pairing either with a
widget milestone would hide one behind the other. They are named here so
the next reader of this budget knows the 164 is a choice.


## M28 and the embedded profile (2026-09-20)

**Zero deferred-work symbols link into a leaf profile**, and that is the
measurement M28/T28.7 owes rather than a `.text` delta.

`.text` is deliberately *not* the measure. `lumen_static::inbox` and
`lumen_static::spawn` are behind the `std` feature and `fixtures/embedded`
does not enable it, so that number cannot move whatever M28 does — a
bound computed from the wrong quantity, which is the shape `AGENT.md`
records twice. The falsifiable form of the same claim is a symbol count,
and zero is a bound an addition can actually cross.

`scripts/check_text.sh` now prints it beside the Toggle count:

```
text: .text 27036 B of 65536 (41.3% of budget), 3 Toggle symbols linked, 0 task symbols (bound 0)
```

**The gate is shown able to fire**, because a gate that refuses
everything and a gate that refuses nothing look identical at zero. With
the pattern pointed at a module that *is* linked
(`lumen_static::ops::`), it reports `11 deferred-work symbols linked …
expected 0`, prints the offenders and exits non-zero. That injection
proves the counting, comparison and reporting work; the emptiness of the
real pattern is the property being asserted, and the two claims are
separate.

### `Ui` re-budgeted 1,152 → 1,168 B on the target

Not M28's cost. **M26/T26.17 finding 12**: `MeasureKey` gained the
display scale, because the measurer's answer depends on it and the memo
did not key on it — so an ordinary monitor-to-monitor drag served a box
measured for the other display (`docs/results-frame-and-types.md`).

Four bytes on each of the four memo-holding widgets in the eleven-node
fixture. **Attributed rather than assumed**: removing the field returns
the number to exactly 1,152.

It is paid because the alternative is a text box that is wrong on one of
two monitors. The `const` assert in `fixtures/embedded/src/lib.rs` moves
with the argument, which is what a re-budget means here.

**Found late, and that is worth recording.** The assert is a `const` on
the `thumbv7em-none-eabihf` target, which only `just check-extra` and
`just firmware-size` build. The per-commit path — `just static`,
`cargo test --workspace`, `cargo clippy` — never targets it, so the
regression survived four green commits. `just static` is pure bash by
design and compiles nothing; the cross build is where this class of
gate lives, and it has to be run deliberately.

### `Ui` re-budgeted 1,168 → 1,176 B on the target — M45/T45.2

`MeasureKey` gained the measurer's generation (`TextMeasure::generation`),
so a box measured by one font stack is not served to a frame measured by
another — the key half of R2-F1 (`docs/plan-text-correctness.md` §2.3).
**Attributed**: with the field removed and everything else in T45.2 kept
(the limits as bit patterns, `TextInput`'s key built on `MeasureKey` with
a `u32` length), the number is exactly 1,168. A `u64` length, which the
old tuple had, made it 1,192; the length is a `u32` because no field
holds 4 GB. `.text` after it: 27,964 B, inside the headline's 1%.

### `Ui` re-budgeted 1,176 → 1,192 B on the target — the M56 merge

**Attributed by type**, each measured on `thumbv7em-none-eabihf` in both
trees: `Label` 56, `Button` 72, the three toggles 12 each, `Fixed` 8 and
the derived `Controls` 144 are unchanged; `TextInput<InlineText<64>>`
went 932 → 948 B. The sixteen bytes are M57's fields on it —
`read_only` and `submitted` (`bool`), `presses` (`u32`, the press edge
the combobox reopens on) and `placeholder_w` (`f32`). The branch asserted
1,168 throughout M57 and the assertion never ran: `fixtures/embedded` did
not build for the target on the branch (`alloc` and `f32::ceil` in
`no_std` code), which the merge fixed. So the cost was paid in M57 and
first measured here.

### `Ui` re-budgeted 1,192 → 1,216 B on the target — M80/T80.2

`MeasureKey` gained `flow`, the node's `Style::line_height`, so a restyle
that changes only the pitch is not served the box measured at the old one
(`crates/lumen-static/tests/measure_key.rs`). Four bytes of key, 32 → 36,
on each of the four memo-holding widgets in the eleven-node fixture,
which alignment makes **24**: the title `Label`'s `Measured` +4, the field
`TextInput`'s shaped key +4, and each of the two `Button`s' `FoldedText`
+8, because its `u64` fold rounds the tuple up to a multiple of eight.
**Attributed** by removing the field and everything else in T80.2 kept:
1,192 exactly. `Style` stays **80 B** and `NodeState` **84 B** (T80.2 took
one spare tail byte of each, `line_height`), so `PER_NODE_RAM` is still
164 B, **16.0 KB per 100 nodes** (`tests/cursor.rs` prints it). T80.3's
`Label.fit` was to be the next `Ui` move; it is not on the target (below).

### `Ui` stays 1,216 B on the target — M80/T80.3

T80.3 gave `Label` its cut, `fit: (u32, f32)`, **+8 B per `Label`** where
there is an allocator (`Label<&str>` 72 → 80 B and `Label<String>` 80 → 88
on the host; `AppBar` 72 → 104, the memo moved onto a `FoldedText` keyed by
`MeasureKey`). With the field on the leaf tier the fixture's `Ui` measured
**1,224 B** on `thumbv7em-none-eabihf` (one title `Label`, +8). K80b's
fallback (next section) takes the cut out of the leaf tier, and the field
with it, so the asserted figure is unchanged at **1,216 B**; the
`alloc-tier` record build keeps the cut and asserts **1,224 B**. `Style` stays
**80 B**, using its last spare tail byte for `overflow`, and `NodeState`
**84 B**, untouched (the cut is made in layout and paint needs no copy):
`PER_NODE_RAM` is 164 B, **16.0 KB per 100 nodes** (K80a holds;
`tests/cursor.rs` prints it).

### K80b's fallback: the leaf tier reads `TextOverflow` as `Wrap` — M80/T80.3

`just firmware-delta 3b99585 print` on the first build, with the cut in
every tier: `firmware` **+5,622 B** `.text` and **+3,078 B** `.rodata`. Most
of that was not the cut: `&text[..end]` indexing linked `str`'s slice-error
path (`slice_error_fail_rt` 1,096 B, `char::escape_debug_ext` 490,
`unicode::printable::is_printable` 720, the `grapheme_extend` offsets table 767,
`Formatter::pad` 470) for a panic the code cannot reach, since every
offset is a boundary by construction. With every slice a `get` the cut
still cost **+2,024 B** on `firmware` and +1,844 B on
`firmware_interactive` (`fit::cut` 744 B, `measure_label` 388,
`Label::layout`/`paint` growth), which would have put M80a at 2,710 of
K80b's 1,024 B. So the plan's own fallback applies: without `alloc`,
`Label` measures every mode as `Wrap`, its `fit` field is compiled out, and
its seed does not declare `Fields::OVERFLOW` (so the sheet lint reports an
`overflow` rule there as the no-op it is). `AppBar` keeps cutting in every
tier through `fit::cut`, by `char` where there is no allocator, and is not
in any firmware.

### `Selectable` costs the firmware nothing — M80/T80.4

`just firmware-delta 1cd123e firmware=0 firmware_interactive=0
firmware_bind=0 firmware_cascade=0`: **+0 B** `.text` and `.rodata` on all
four binaries, so the headline and the four-binary table above stand as
T80.3 measured them (until T80.5 moved them, below). `Selectable` is behind `alloc`, which the leaf tier
does not take, and `Label`'s paint split (`paint_ground`, `paint_run`,
`pub(crate)` and `#[inline]`) compiles to the same code. The gate's
injection, `paint_run` marked `#[inline(never)]`, prints +28 / +332 / +40 /
+28 B `.text` and fails all four by name. `Label` stays 80 B on the host
and the embedded `Ui` 1,216 B; `Style` and `NodeState` are untouched.

### The `caret_rect` slot — M80/T80.5

`just firmware-delta cf1f623` (K4's default: 1,000 B on
`firmware_interactive`, the others printed):

| binary | `.text` | `.rodata` |
|---|---|---|
| `firmware` | 29,526 → 29,586 (**+60**) | +16 |
| `firmware_interactive` | 63,072 → 63,188 (**+116**, bound 1,000) | +44 |
| `firmware_bind` | 32,990 → 33,046 (**+56**) | +16 |
| `firmware_cascade` | 31,666 → 31,726 (**+60**) | +16 |

One slot on every `Node` vtable and the shared `None` default body, on
every binary; on the interactive one also `TextInput`'s override and its
clip and scroll fix (`docs/results-text.md` M80/T80.5). Not D6 growth (D6
names `text_spaced` and `fills`); reported to the owner with the number.
`Style`, `NodeState` and the embedded `Ui` are untouched.

### `GlyphKey` costs the firmware nothing — M81/T81.1

`just firmware-delta faeffe7 firmware=0 firmware_interactive=0
firmware_bind=0 firmware_cascade=0` (K4's comparator, Δ 0 on all four as
`docs/plan-shaped-paint.md` bounds it): **+0 B** `.text` and `.rodata`
on every binary. The core gained `text::GlyphKey`, a plain 8 B struct no
firmware names, and the shell, which the firmware does not link, wraps
it in `GlyphId`.

### `Cluster::rtl` — M81/T81.2

`just firmware-delta cdaf0ec firmware=64 firmware_interactive=64
firmware_bind=64 firmware_cascade=64` (K4's comparator at the plan's
tighter 64 B per binary): **−220 / −240 / −220 / −220 B** `.text`,
`.rodata` +0 on all four. `size_of::<Cluster>()` stays **12 B**
(`crates/lumen-static-text/tests/shaper.rs` prints it): `rtl` takes a
byte of `line`'s padding.

**What the field costs, from a symbol diff of the two builds**:
`BakedFace::shape` **+8 B** (the `rtl: false` stores in its cluster
constructors), the one growth on every binary. The rest is codegen the
field moved: `TextInput::paint` −42, `place` −8, `caret_rect` −8 and
`on_event` −16 on the interactive binary, and `Panel::new` /
`Device::new` **−176**, the fixture's zeroing of the default tree.

**The first build was over the bound**: +150 / +134 / +154 / +154 B. With
the literal `Cluster { .., rtl: false }` repeated in
`InlineClusters::default`, `TextInput::default` came out of line at
**602 B** where `Panel::new` had cleared the buffer with
`__aeabi_memclr8`. Written through a `const` element the default is a
clear again, and `Panel::new` comes out 176 B smaller than at the parent
— so this task's net is negative, and part of it is a saving the parent
could have had.

**That saving hides the plan's injection from the parent comparison**,
which is a finding about the gate, not a pass: `BakedFace` computing
`rtl` through an `#[inline(never)]` range-check chain over the Hebrew and
Arabic blocks, called per cluster, prints −36 / −48 / −32 / −28 B against
`cdaf0ec` and passes. Against a snapshot of this task's tree (a dangling
commit of the work tree, so the base is T81.2 without the injection) the
same injection prints **+184 / +192 / +188 / +192 B** and fails all four
by name; restored, the snapshot comparison prints +0 on all four.

### `TextInput`'s caret and tint over bidi runs — M81/T81.5

`just firmware-delta 3f5c7e3` (K4's default, `firmware_interactive=1000`):
**+860 / +844 / +860 / +860 B** `.text`, `.rodata` **+40** on all four
(the new module's panic locations). Every binary links `TextInput`, so
every binary pays: from a symbol diff of the two builds, `caret::spans`
**+708**, `caret::at` **+242**, `TextInput::spans`' closure and its
`&mut dyn FnMut` shim **+144 + 144**, against `TextInput::paint` **−310**
(its two inline selection and composing rects), `place` −72, and on the
interactive binary `caret_rect` −72 and `on_event` +60. `Style`,
`NodeState` and the embedded `Ui` are untouched.

**The first build was over K4's bound**: +1,464 B on
`firmware_interactive` (+1,472 on the others), with `caret::spans`
generic over its closures, so `TextInput`'s selection and its composing
underline each had a copy. Taking the callback as `&mut dyn FnMut` and
the line ends as a slice leaves one copy for all three widgets.

### The end wins a click's tie — M81/T81.6

`just firmware-delta 46072b7` (K4's default): **+0 / −708 / +0 / +0 B**
`.text`, `.rodata` +0. Only `firmware_interactive` dispatches a press, so
only it links `TextInput::offset_at`, which inlines into
`TextInput::on_event`: a symbol diff of the two builds puts all −708 B in
`on_event`, and no other symbol moves. A shrink for an added comparison
is an inlining decision, not a saving the change earns; recorded as
measured. The paint-only headline above is unchanged.

### Subpixel pens cost the firmware nothing — M81/T81.8

`just firmware-delta 3b3760d` (K4's default): **+0 B** `.text` and
`.rodata` on all four binaries. The phase is decided and rasterised in
the shell (`text::pen_of`, `FontStack::rasterize`), which no firmware
links; the core changed only `GlyphKey::phase`'s rustdoc.

### The headline re-measured at the M56 merge — 2026-09-29

`scripts/check_text.sh` refused the merge: **28,616 B** linked against the
27,964 B headline, 2.33% over its 1% tolerance. `main` at `1a59224` links
**28,020 B** on the same toolchain, so the merge adds **596 B**, and the
branch's M56/M57 work is all of it — like `Ui` above, the branch's
fixture never linked for the target. Netted by owner from a symbol diff
of the two binaries (inlining moves code between symbols, so single
symbols overstate): `Toggle` layout and paint +508, `SeedRoles::apply`
+314 (M57/A8's palette roles), `TextInput::paint` +298 (the centred,
clipped line), `ops::shaded` +240, `ops::record_painted` +124 (M56's
`paints` flag); `Button` and the derived `Ui` net about zero or below.

The other three binaries, same build: `firmware_interactive` **61,922 B,
75.6% of 80 KB** (94.5% of the 64 KB ceiling then; 56,802 in the
M55/T55.5 table below — the dispatch path carries M57's input work: the
press latch, focus moves, the combobox's edges), `firmware_bind` 32,008,
`firmware_cascade` 30,644 (delta over paint-only 2,028, bound 3,072).
Every binary is gated on the ceiling, and the interactive one had
**3,614 B** left under 64 KB, which is why the owner raised it to 80 KB
(D5): **19,998 B** left at the merge. The table below is the current four-binary
record.

### All four, against 80 KB — M80/T80.2 (2026-10-04)

`scripts/check_text.sh` and `scripts/firmware_delta.sh`, release,
`thumbv7em-none-eabihf`. **This table is the one K4
(`docs/plan-iced-roadmap.md`) rewrites** when a task moves a binary; the
dated tables above and below were measured against the 64 KB ceiling
that held then, and stay as measured. First written by M80/T80.0 at
`553fa5e` (the M56/M61 merge; the "at `553fa5e`" column), rewritten by
T80.1 (`Painter::text_spaced` required, `text_weighted` provided, the
`Cmd::Text.line_height` byte folded by `Checksum`), T80.2
(`Style.line_height` measured by `Label` and painted through
`text_spaced`), T80.3 (`Style.overflow`, with the leaf tier reading it
as `Wrap`, and `Label` measuring against its own width less its padding)
and T80.5 (the `Node::caret_rect` slot; T80.4 moved nothing),
re-measured by M81/T81.1 (the core's `GlyphKey`, which no firmware
names: Δ 0 on all four), rewritten by M81/T81.2 (`Cluster::rtl`, and
`InlineClusters::default` through a `const` element; the section below),
re-measured by M81/T81.3 (`Label` placing a right-to-left cut's `…`
at the left, behind `alloc`, which the leaf tier drops: Δ 0 on all four,
`just firmware-delta 4ccde3b`), and rewritten by M81/T81.5 (`TextInput`'s
caret and selection through the `caret` module; the section below);
rewritten by M81/T81.8 to carry M81/T81.6's −708 B on
`firmware_interactive` (its own section above), which T81.6 had recorded
there and not here, and re-measured by T81.8 itself: Δ 0 on all four
(`just firmware-delta 3b3760d`).
M80a's column is T80.0–T80.3, as K80b bounds it.

| binary | `.text` | of 81,920 | headroom | `.rodata` | `.text` at `553fa5e` | T80.1 Δ | T80.2 Δ | T80.3 Δ | T80.5 Δ | M80a (K80b) | T81.1 Δ | T81.2 Δ | T81.3 Δ | T81.5 Δ | T81.6 Δ |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `firmware` | 30,226 | 36.9% | 51,694 | 13,316 | 28,616 | +56 | +630 | +224 | +60 | **910 of 1,024** | +0 | −220 | +0 | +860 | +0 |
| `firmware_interactive` | **63,084** | **77.0%** | **18,836** | 21,846 | 61,922 | **+52** | **+802** | **+296** | **+116** | **1,150 of 1,536** | +0 | **−240** | +0 | **+844** | **−708** |
| `firmware_bind` | 33,686 | 41.1% | 48,234 | 13,920 | 32,008 | +56 | +678 | +248 | +56 | — | +0 | −220 | +0 | +860 | +0 |
| `firmware_cascade` | 32,366 | 39.5% | 49,554 | 14,092 | 30,644 | +56 | +730 | +236 | +60 | — | +0 | −220 | +0 | +860 | +0 |

**T80.3's cost, from a symbol diff of the two builds**: `text_limits` (the
room `Label` now measures against, its own width less its padding:
M80/T80.2's carried defect, fixed here) about **+120**, `measure_label`
+66 and the `Label`/`Button`/`TextInput` layouts a few bytes each (the key's
`flow` word now carries the overflow byte), every `type_defaults` about +4
(the `overflow` byte in each default `Style`), and on the two cascading
binaries `Patch::apply_to`'s new branch. `.rodata` +0. Printed by `just
firmware-delta 3b99585` (K4's default, `firmware_interactive=1000`), and
M80a's total by `just firmware-delta 845de0c firmware=1024
firmware_interactive=1536` (K80b): **+910 B** and **+1,150 B**, both under
their bounds, after the fallback above; the same invocation without it
would have printed +2,710 B on `firmware`.

**T80.2's cost, from a symbol diff of the two builds** (inlining moves
code between symbols, so these are attributions, not a partition):
`cx::measure_spaced` **+332** (the line count, the pitch and the ceiled
new-line size, linked by `Label::layout`), `__aeabi_memclr8` **+174**
(newly linked, with one caller: `Panel::new`, zeroing the default tree,
whose layout the wider memo structs changed),
`LineCount`'s `cluster` +18 and `GlyphSink::is_full` +8 (its vtable), and
on the two binaries that cascade a sheet `cascade_range` **+102**
(`Patch::apply_to`'s new branch and the ratio's round to sixteenths); the
rest is a few bytes across `Label::layout`/`paint`, `Button::layout` and
`record_painted`. `.rodata` +20–44 B is the wider `Patch` in the
embedded sheet and the vtable. Printed by `just firmware-delta 076ae16`
(K4's default, `firmware_interactive=1000`): under the per-task cap.

**T80.1's +52–56 B is a twelfth of what the plan expected.** The plan
cited T27.1's +676 B paint-only cost for a required method plus a field
(M27, a border: a new `Cmd` variant, its painter body and its fold in
every painter). T80.1 adds a vtable slot (`text_spaced`) beside one that
became a provided forward (`text_weighted`), and one byte folded by
`Checksum::text_spaced`; nothing in the firmware paints a second line, so
the re-placement code is in `lumen-static-shell`, which the firmware does
not link. Printed by `just firmware-delta 845de0c
firmware_interactive=1000` (K4's per-task cap, 1,000 B, on
`firmware_interactive`), `.rodata` +4 B on each.

A task's cost is measured by `just firmware-delta <parent>`, which builds
the four at the parent commit and at the work tree and fails a per-binary
bound by name; `check_text.sh`'s 1% headline tolerance is about 280 B of
the paint-only binary and cannot say "unchanged".

### Grapheme editing is behind `alloc` — M45/T45.5

The editors step by extended grapheme cluster through
`unicode-segmentation`, an optional dependency of `lumen-static-widgets`
enabled by its `alloc` feature. **The leaf firmware's `.text` did not
change**: 27,964 B before and after (`scripts/check_text.sh`), because
`fixtures/embedded` takes neither `std` nor `alloc` and its `TextInput`
keeps stepping by `char`. The alloc tier has no linked binary in this
repository to measure; on the host, `examples/counter-window` (which has
a `TextInput`) links **6,962 B** of `unicode_segmentation` symbols —
tables and cursor — in release, x86-64. That is the tier's price for
Backspace removing a whole flag.

## M45/T45.12 — the baked face wraps at the space, measured 2026-09-26

`BakedFace::shape` moved a word that would overflow whole to the next
line, instead of breaking it mid-word (R10-F10). Measured with `just
firmware-size`, before and after on the same toolchain, by swapping
`font.rs` on either side of the same link:

| | `.text` | `.rodata` |
|---|---:|---:|
| before (`0091266`) | 27,024 | 12,860 |
| **after T45.12** | **27,332** | **12,860** |
| Δ | **+308** | **0** |
| *the first version, rejected* | *31,590 (+4,566)* | *15,634 (+2,774)* |

The first version found the word's end with `text[byte..].split(' ')`.
A slice and a `split` link `str`'s pattern machinery and the panicking
char-boundary check, neither of which the firmware had; the kept version
clones the `char_indices` iterator and sums advances to the next space.
**The plan left the choice (a buffer or a first pass) to `.text`, and
the choice of how to write the first pass was fifteen times the cost of
the pass itself.** `scripts/check_text.sh`: 27,332 B of 65,536, 41.7%.

## M46/T46.1 — the text hash, measured 2026-09-26

`.text` **+400 B**, measured on M46's own base: 27,024 → 27,424. On the
merged tree, after T45.12's 27,332, it is **27,732 B**, the headline
above (+308 + 400 = +708 over 27,024, exactly). All of it is
`Button::layout` (848 → 1,248 B): the fixture's `Button` keys its
`FoldedText` memo on `display::hash_bytes`, inlined, where it used to
fold byte by byte. The fixture links no `Recording`. RAM is unchanged —
the memo's key is a `u64` either way. The kill rule that judged it, K3,
and the rest of the measurement are in `docs/results-retention.md`,
"What a text hash guarantees".

**The plan predicted this could not move, and was wrong.** `nm` on the
firmware showed no `FoldedText` symbol, which read as "not linked"; it
was inlined into `Button::layout`. A symbol search is evidence of
absence only for a function that is not inlined, which is the same
lesson as the `#[no_mangle]` static that the linker dropped (M15): the
quantity has to be *measured*, not looked up.

## M45 — the headline re-measured, 2026-09-27

After all of M45 had merged (T45.0–T45.12), `just firmware-size` gave
`.text` **27,732 → 27,968 B (+236)**, `.rodata` 12,860 → 12,880 (+20), and
flash 41,065 B. That drift sat 0.85% inside check_text.sh's 1% tolerance,
so no gate asked for it. It was re-measured when M45 closed, because a
headline left near its tolerance hands the next branch a failure that
branch did not cause. T45.6, the last task, moved `.text` by 0 B
(measured with and without it). The +236 belongs to M45's earlier
tasks, taken together; no per-task split was measured.

