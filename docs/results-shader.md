# A GPU you can extend: the backend seam and custom shaders

M83's results (`docs/plan-gpu-extension.md`). Every figure an M83 gate
prints is recorded here once and cited from the rustdoc, the plan and the
commits. The plan-wide budgets and kill rules K1–K4 are
`docs/plan-iced-roadmap.md` §2 and §4; the firmware's four-binary table
is `docs/results-embedded.md`'s.

## Thresholds, written before anything is built (T83.0, 2026-10-06)

M83's own kill rules and the bounds its tasks state, copied from the plan
so each later row has its threshold beside it. A row's verdict is filled
in by the task that measures it, never edited here.

| rule | quantity | threshold | measured by | if it fires |
|---|---|---|---|---|
| K83a | `examples/shader_cost` at 1080p: 32 trivial 64 × 64 shaders less the same frame drawing their fallbacks, ÷ 32 | ≤ **50 µs** per node (three times a glass pass, `docs/results-glass.md` §0h) | T83.5 | consecutive brackets sharing a pipeline batched into one pass and re-measured; then the per-node figure goes in the rustdoc and "many shader nodes per frame" is stated unsupported |
| K83b | the four firmware binaries' `.text` and `.rodata`, `shader` off, `just firmware-delta <parent> firmware=0 firmware_interactive=0 firmware_bind=0 firmware_cascade=0` | Δ **0 B** each, in every task that touches `lumen-static` | every such task; the record is the T83.0 row below | the task does not land until the leaked symbol is found by symbol diff and gated |
| K83c (K3) | frame `idle (floor)` and `+render (damage)`, `--bin cascade` `cascade_deep (whole table)`, `--bin textarea` `keystroke + layout, 100 KB`; eight runs a side under `just bench-lock`, `shader` off | inside the T83.0 spread below | T83.3 | attributed by a symbol diff of the release frame binary: alignment lands with both numbers recorded; a shader symbol in the default build is a missed `cfg` (K83b) and stops T83.3 |
| K83d | `lumen-static-winit`'s default normal-dependency count (K2's exact command); the stripped release showcase | **+0** crates; a showcase delta under **3 MB** | T83.1, T83.2 | T83.1 stops; `raw-window-handle` is pinned to `std` only |
| K1 | `size_of::<Cmd>()`, `Style`, `NodeState` | `Cmd` **60 B** with `shader` off and on; `Style` / `NodeState` unchanged from the parent commit | T83.3 | K1 |

## T83.0 — baselines and the gates that compile today, at `5b6da2b` (2026-10-06)

The numbers every kill rule above compares against, and two gates that
compile today: one pins the premise T83.1 relies on, the other forces
T83.3 to say what a shader is as a contrast ground.

**Baselines.** Timings under `just bench-lock` on the reference machine
(32 cores; every run quiet, load1 0.79–1.01 against the lock's bound of
8).

| quantity | how | at `5b6da2b` |
|---|---|---|
| firmware, all four binaries | `just firmware-delta 5b6da2b firmware=0 firmware_interactive=0 firmware_bind=0 firmware_cascade=0` | Δ **0** `.text`, **0** `.rodata` on all four: `firmware` 30,606 / 13,344 B, `firmware_interactive` **63,480** / 21,886 B (headroom 18,440 B to 81,920), `firmware_bind` 34,014 / 13,948 B, `firmware_cascade` 32,694 / 14,132 B |
| `lumen-static-winit`, default normal dependencies | `cargo tree -p lumen-static-winit -e normal --prefix none \| sed 's/ (\*)$//' \| sort -u \| wc -l` (K2's command) | **96** |
| `lumen-static-shell` alone | the same command on `-p lumen-static-shell`, default and `--features truetype` (`scripts/check_shell_headless.sh` prints both) | **8** default, **16** with `truetype` |
| the stripped release showcase | `cargo build --release -p showcase`, GNU strip 2.42 | **9,719,656 B** stripped (13,043,320 B unstripped) |
| `lumen-static-wgpu/tests/alloc.rs` empty-frame floor (RTX 4070, Vulkan) | `cargo test -p lumen-static-wgpu --test alloc -- --nocapture` | floor **61** allocations, wgpu's submission alone 61, difference **0**; the caret frame 61 (settled after `[64, 61, 61]`) |
| clean build of `just firmware-size` (T83.3's four build scripts' "before") | a fresh cargo target directory, three runs | **3.04**, 3.00, 3.08 s; build scripts in the core, shell, wgpu and benches today: **0** (the workspace has two, `lumen-static-reload`'s and `fixtures/embedded`'s) |

**K3's arms, eight invocations at `5b6da2b`** (`R=1
benches-competitive/run.sh -p lumen-static-frame`, `cargo run --release
-p benches --bin textarea -- 100`, `cargo run --release -p benches --bin
cascade`; one invocation of each per lock). K83c's rows are the first
four; the other frame rows are printed for the later tasks that name
them.

| row | median | spread (min–max) |
|---|---:|---:|
| `idle (floor)` | 56.35 µs | **55.80–62.60** |
| `+render (damage)` | 76.85 µs | **72.90–77.20** |
| `cascade_deep (whole table)` | 15,902.42 ns | **15,854.81–15,928.86** |
| `keystroke + layout, 100 KB` | 5.35 ms | **5.22–5.50** |
| `ALL rows: layout` | 1,162.95 µs | 1,160.50–1,165.80 |
| `ALL rows: +paint` | 1,287.55 µs | 1,284.30–1,290.80 |
| `layout` | 20.30 µs | 19.10–20.30 |
| `+paint` | 40.85 µs | 38.60–41.20 |
| `+render (full)` | 138.40 µs | 131.20–139.00 |
| `+paint (culled)` | 22.40 µs | 22.30–22.50 |

**Gates.**

| gate | measured (green) | bound | injection (red) |
|---|---|---|---|
| `scripts/check_shell_headless.sh` (`just static`, no compile): the shell's normal dependencies, default and `truetype` | default **8** crates, `truetype` **16**, none of winit/softbuffer/wgpu | none | `winit = "0.30"` in the shell's `[dependencies]`: before the lock is updated, "cargo tree failed for the default build -- not a pass", exit 1; after `cargo update --workspace --offline`, "the shell's default build depends on winit v0.30.13" and the same for `truetype`, exit 1 |
| — accept direction | `raw-window-handle` 0.6 (`std`, optional, enabled by `truetype`; T83.1's line) with its lock update: default **8**, `truetype` **17**, exit 0; `raw-window-handle v0.6.2` in the `truetype` run only | passes | — |
| `contrast_names_every_command` (`lumen-static-shell/tests/contrast_commands.rs`): each `Cmd` under a text run's centre, the expected ground from an exhaustive match | 12 kinds built of `KINDS` = 12; probes reported **12** (measured 8, unmeasured 4); `Rect`, `RoundedRect` and `Gradient` measured against their colour; `Text`, `StrokedRect`, `Line`, `PushClip`, `PopClip` against the page; `Shadow`, `Image`, `Glass`, `Path` unmeasured with their reasons | every row as stated; 12 reported | `ground_at`'s `Glass` arm deleted: `Glass: want Unknown("glass"), got Miss`, red by name; the `Image` sample dropped: "kinds 11 built of 12", red by name |

## T83.1 — `Backend`/`Frame` into the shell; `BackendTarget`, at `376301b` + T83.1 (2026-10-06)

`Backend`, `Frame`, `BackendTarget` and `WindowHandles` live in
`lumen_static_shell::backend`, behind the shell's `truetype`;
`lumen-static-winit` re-exports them. `fetch_rgba`, `set_images` and
`frame_rgba` are required; `GpuBackend` and winit's test `Fake` write the
`None` bodies they inherited. `Software::new` and `GpuBackend::new` take
`&BackendTarget`, which `resumed_impl` builds from the window once. No
behaviour changes.

**Crate counts** (K83d, K2's exact command).

| quantity | T83.0 | T83.1 | bound |
|---|---:|---:|---|
| `lumen-static-winit`, default normal dependencies | 96 | **96** | +0 (K83d) |
| `lumen-static-shell`, default | 8 | **8** | +0 (Decisions) |
| `lumen-static-shell`, `--features truetype` | 16 | **17** | +1: `raw-window-handle v0.6.2`, the only line in the diff of the two sorted lists |

**Gates.**

| gate | measured (green) | bound | injection (red) |
|---|---|---|---|
| `scripts/check_shell_headless.sh` | default **8**, `truetype` **17**, none of winit/softbuffer/wgpu | none of the three | (T83.0's) |
| `cargo check -p lumen-static-shell` (default features, `just test-crates`' build of the shell alone) | compiles | compiles | `#[cfg(feature = "truetype")]` dropped from `pub mod backend`: E0432 on `raw_window_handle`, `crate::session` (`FrameSource`) and `crate::text` (`FontStack`), 3 errors |
| M57's D5 parity, `scripts/widget_drive.sh Label` (a filtered run compares the entries it names, M81/T81.6) | 67,584 px, **0** bytes differ, on all three legs (`Everything`, `Boundaries<256>`, `Boundaries<256>`+oracle) | 0 | `Software::new` stores `None` for the font: "the live demo with the agent differs from the headless render over 67584 px … **30,453** bytes differ (bound 0)", on all three legs |
| `just resume-drive` | software-resume 0 / 458,620 px differ, backends 2; software-still 0, backends 1; gpu-resume 0 (`xwd`), backends 2 | 0.1% | its own M47/T47.7 injections stand; it gates presence and image retention, not constructor arithmetic |
| `just agent-drive` | green, both strategies and the oracle leg (92 frames compared, none diverged) | — | its own |
| firmware, `just firmware-delta 376301b firmware=0 firmware_interactive=0 firmware_bind=0 firmware_cascade=0` | Δ **0** `.text` and `.rodata` on all four | 0 (K83b, K4) | — (nothing the firmware links moved) |
