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

## T83.2 — `Program::backend_with`, at `44d5824` + T83.2 (2026-10-06)

`Program::backend_with` installs a `BackendFactory` (a boxed `FnMut(&BackendTarget)
-> Option<Box<dyn Backend>>`, behind the shell's `truetype`); `run_program`
moves it into `Config::backend_with` once, and `pick_backend` asks it on
every `resumed` in **both** `cfg` branches, through `choose_backend(prefer,
custom, gpu, software)`. `examples/glasswall --features custom-backend` is
the factory the live leg drives.

**Gates.**

| gate | measured (green) | bound | injection (red) |
|---|---|---|---|
| `cpu_adapter.rs`, the custom column (no window) | `(Auto, custom Some)` → `Custom`, calls (1, 0); `(Gpu, custom None)` → `Gpu { accept_cpu: true }`, (1, 1); `(Software, custom Some)` → `Software`, (0, 0); `(Auto, custom None, gpu None)` → `Software`, (1, 1); `(Auto, custom Some, gpu Absent)` → `Custom`, (1, 0) | each row as stated | `gpu` asked before `custom`: row 1 prints `Gpu { accept_cpu: false }; custom called 0, gpu called 1`, red ("made the wrong backend"); `custom` called under `Software`: row 3 prints `custom called 1`, red ("calls (custom, gpu)") |
| `just resume-drive`'s `custom-resume` leg (glasswall `agent,custom-backend`, no `gpu`, `LUMEN_BACKEND=auto`, `LUMEN_TEST_RESUME=4000`) | factory backends before the resume **1**; wallpaper pixel (112, 105) **(46, 115, 214)** against page (8, 10, 18); factory backends **2**, backend #2 `counting (software) #2`; 0 / 458,620 px differ | exactly 2; the pixel not the page colour; 0.1% | `Config::backend_with` `take()`n on first use: "factory backends 1 (bound: exactly 2); backend #2 is 'software (softbuffer)'"; the counter's `set_images` drops the source: pixel (8, 10, 18), "the first capture shows the page colour"; the counter's `frame_rgba` answers `None`: "ui.screenshot failed: this host does not render"; `pick_backend`'s `not(gpu)` branch builds `Software::new` directly: "factory backends before the resume 0" — each exit 1 |
| — the other legs (`just resume-drive`, full) | software-resume, software-still, gpu-resume: 0 px differ each | 0.1% | their own |
| firmware, `just firmware-delta 44d5824 firmware=0 firmware_interactive=0 firmware_bind=0 firmware_cascade=0` | Δ **0** on all four | 0 (K83b, K4) | — |

## M83a closes — K83d at T83.2 (2026-10-06)

| quantity | T83.0 | at T83.2 | bound | verdict |
|---|---:|---:|---|---|
| `lumen-static-winit`, default normal dependencies (K2's exact command) | 96 | **96** | +0 | holds |
| the stripped release showcase (`cargo build --release -p showcase`, GNU strip 2.42) | 9,719,656 B | **9,727,848 B** (+8,192 B; 13,055,688 B unstripped) | under +3 MB | holds |

`just ci-verify` on this half is the coordinator's, and is cited in
AGENT.md's queue entry.

## T83.3 — the core bracket, at `a4080ea` + T83.3 (2026-10-06)

`Cmd::Shader { rect, id: ShaderId, uniforms: (offset, len) }` … `Cmd::ShaderEnd`,
`Painter::shader`/`shader_end` (provided: `push_clip`/`pop_clip`, the leaf
contract), `DisplayList::uniforms`, and the overrides in `Scaled`,
`Deferring` and `Recording` -- all behind the core's default-off `shader`
(owner decision D3). Outside the core every arm is `cfg(lumen_shader)`:
`lumen-static` declares `links = "lumen_static"` and its `build.rs` prints
`cargo:shader=1`; `lumen-static-shell`, `lumen-static-wgpu` and `benches`
each turn `DEP_LUMEN_STATIC_SHADER` into that cfg. Wire tags **12** and
**13**, appended (`wire::VERSION` stays 4). The canvas, SVG and (until
T83.5) the GPU draw a bracket as a clip around its fallback.

**Budgets** (K1, K83b).

| quantity | parent `a4080ea` | T83.3, `shader` off | T83.3, `shader` on | bound |
|---|---:|---:|---:|---|
| `size_of::<Cmd>()` (`tests/glass_cmd.rs`) | 60 B | **60 B** | **60 B** | 60 |
| `Shader`'s payload, mirrored | — | — | **28 B** (26 B of fields, padded) | ≤ 56 (`Gradient`'s) |
| `Style` / `NodeState` (`tests/cursor.rs`) | 84 / 84 B | **84 / 84 B** | **84 / 84 B** | unchanged |
| firmware, `just firmware-delta a4080ea firmware=0 firmware_interactive=0 firmware_bind=0 firmware_cascade=0` | — | Δ **0** `.text`, **0** `.rodata` on all four (30,606 / 63,480 / 34,014 / 32,694 B `.text`) | — | 0 (K83b, K4) |
| clean build of `just firmware-size` (fresh target dir, three runs) | 3.00, 2.94, 3.04 s (re-measured today; T83.0's 3.04, 3.00, 3.08) | **3.10**, 2.99, 2.98 s | — | printed (Decisions, "Cost") |
| build scripts in the workspace | 2 (`lumen-static-reload`, `fixtures/embedded`) | **6** (+ the core, the shell, wgpu, `benches`); the firmware build runs the core's and the fixture's | — | the four the plan names |

**K83c, eight invocations under `just bench-lock`, `shader` off** (all 24
quiet, load1 ≤ 1.27 against 8).

| row | T83.0 spread | T83.3 median | T83.3 spread | verdict |
|---|---:|---:|---:|---|
| `idle (floor)` | 55.80–62.60 µs | **56.30 µs** | 53.20–59.70 | inside |
| `+render (damage)` | 72.90–77.20 µs | **73.20 µs** | 73.10–81.60 | inside |
| `keystroke + layout, 100 KB` | 5.22–5.50 ms | **5.22 ms** | 5.00–5.79 | inside |
| `cascade_deep (whole table)` | 15,854.81–15,928.86 ns | **15,152.42 ns** | 15,017.38–15,876.46 | **below the spread (faster): attributed to the machine, not T83.3** |

The cascade row is K83c's "alignment" outcome, settled the strong way: the
release `cascade` binaries built from `a4080ea` (a throwaway `git archive`
snapshot) and from this tree have **identical** `.text` (682,515 B each,
`size`) and an **empty** `nm -C -S` diff of their text symbols, and run
alternately under the lock, eight a side, they read base **15,155.69 ns**
(14,993.23–15,339.56) and work **15,109.34 ns** (15,069.95–15,245.01). The
parent itself sits below T83.0's spread today, so the shift is the
machine's; no shader symbol is in the default build (K83b reads Δ 0).

**Gates.**

| gate | measured (green) | bound | injection (red) |
|---|---|---|---|
| exhaustive both ways (`dev-shape`): `cargo check --workspace --all-targets`, off and `--features lumen-static/shader` | both compile, wgpu included | both green | `benches/src/bin/retain.rs`'s `cfg(lumen_shader)` `Shader` arm deleted: "on" fails **E0004** `&Cmd::Shader { .. }` not covered at `retain.rs:37`, "off" green; `cargo:rustc-cfg=lumen_shader` removed from the shell's `build.rs`: "on" fails E0004 at `software.rs:1327` and `svg.rs:168`; one shell arm written `#[cfg(feature = "shader")]` (`svg.rs`): "on" fails E0004 at `svg.rs:168` -- the unification defect the design exists to stop |
| — lint (`dev-shape`, `just gpu`): `cargo clippy --workspace --all-targets --exclude lumen-static-wgpu --features lumen-static/shader -- -D warnings`, and `-p lumen-static-wgpu --features lumen-static/shader` | clean | -D warnings | an unused binding in svg.rs's `ShaderEnd` arm: the "on" line fails `unused variable: injected`, the feature-off line green |
| shift (`tests/shader_cmd.rs`) | B's offset **16** → **32** (setup: differ); B's `hash_from` `0x15848bd8474ccf96` both | equal | `hash_into` folds `uniforms.0`: `0x15848bd8474ccf96` vs `0x82cfb1ea060355a6`, red by name |
| same slot, new bytes | `0x15848bd8474ccf96` vs `0x35920fb4916db1aa`, the command field-for-field equal | differ | `hash_from` skips the bytes: both `0xf7dec8b455c15b95`, red |
| retention (`Recording` → `Incremental`) | A's bytes change: damage **1.00×** A's rect, B not in it, 2 brackets recorded; accept: B's change damages **1.00×** B, A not in it | 1.00 | `Recording`'s two overrides removed: "damage **0.00×** A's rect, regions [], brackets in the recorded list 0 (setup 2)", red at the setup line |
| `deferring_forwards_the_bracket` | 4 in, 4 out; `Shader` 1 in each list | equal, count 1 | `Deferring`'s overrides removed: "Shader in the wrapped one 0", "first difference at Some(1), Shader … became PushClip", red |
| — `Scaled` | rect ×2 `[20, 20, 120, 80]`, bytes `[1..8]` untouched | as stated | overrides removed: `[PushClip, PopClip]`, bytes `[]`, red |
| — `Recording` forwards | `[Shader, ShaderEnd]` under it | as stated | (the retention injection): `[PushClip, PopClip]`, red |
| — a leaf inherits the clip | `["push", "rect", "pop"]` | as stated | — (the default itself) |
| wire | 137 B at version 4, 21 uniform bytes back of 21, equal; with a path too; a range past the bytes refused; an empty list is the version-4 layout | equal | decode drops the bytes: the range check refuses the buffer, `expect("decodes")` red -- a decoder that drops uniforms cannot also pass validation |
| software (`lumen-static-shell/tests/shader_software.rs`) | fallback pixels inside the rect **2,000** (setup), outside **100** (the unclipped corner rect's only), **0** px differ from `push_clip` + fill + `pop_clip` | 0 | `Canvas` ignores `Shader`/`ShaderEnd`: outside **2,300**, **2,200** px differ, red |
| contrast (`contrast_names_every_command`) | `kinds 14 built of 14`, probes 14 (measured 8, unmeasured 6); `Shader` and `ShaderEnd` `Unknown("shader")` (feature off: 12 of 12) | every row | `ground_at`'s `Shader` arm deleted: both rows `got Miss`, red; the fallback taken as ground: `ShaderEnd: got Ground(Color(255, 255, 255, 255))`, red |
| `tests/svg.rs`, the whole vocabulary | 14 kinds, 14 drawn (feature off: 12) | all | — (the arms' own E0004 above) |
| allocation (`alloc_gate.rs`, `shader::` arm, `--features truetype,shader`) | 20 frames with two brackets: **0** allocs, **0** frees; arms counted 1 | 0; ≥ 1 arm | `DisplayList::shader` copies through a fresh `Vec`: **40** allocs, 40 frees (2.00/frame), red |
| GPU parity, the clip scope inside a bracket (`lumen-static-wgpu/tests/parity.rs`, `--features lumen-static/shader`, RTX 4070) | clip scope **0** px over 8 (bound 2); `Shader`, `ShaderEnd` "in the frame" | 2 | wgpu's `Shader` arm pushes an unbounded clip: clip scope **700** px over 8, red |
| size | `Cmd` 60 B both builds; `Shader` payload 28 B | 60; ≤ 56 | (K1's own) |
| `just abi-stamp`, pair 1: core on, shell off, under a loader with neither | refused (first field `DisplayList` 120/8 vs 144/8); fields that differ `["DisplayList", "lumen-static-shell features"]`, `lumen_shader` in the library's list only | refused, by the entry | the entry reads the shell's `cfg!(feature = "shader")`: fields that differ `["DisplayList"]`, "`lumen-static-shell features` does not differ", red |
| — pair 2: core and shell on, under a loader with the core on and the shell off | refused: `lumen-static-shell features` `reload,truetype,lumen_shader` vs `…,lumen_shader,shader` | refused | the shell entry dropped: "accepted a library that should have been refused", red |
| — accept | `host-shader-core`/`shader-core.so` and `host-shader`/`shader.so` accepted (20 fields), and the plain shell pair | accepted | — |
| firmware (K83b) | Δ **0** on all four | 0 | `ShaderId` and both provided methods ungated: `.text` **+52** (`firmware`), **+24** (the other three), `.rodata` **+8** each, FAIL by name on all four |

## T83.4 — the shader table, host to backend, at `e339bc1` + T83.4 (2026-10-06)

`ShaderDesc` (a `const fn` constructor asserting `uniform_size <= 256`)
and `ShaderTable` (opaque, `Copy`, empty without the shell's `shader`) in
`lumen_static_shell::shader`; `Program::shaders`, installed by
`Program::host()` through `Host::set_shaders`; the required
`Session::shaders` (`Local` answers its host's table, `Hot` forwards) and
`Backend::set_shaders` (`Software`: a no-op refusing 0; `GpuBackend`: an
owned copy until T83.5; glasswall's `Counting` forwards). The window calls
it in `resume_backend`, on every backend it makes, and in `redraw`'s swap
branch after `swap` returns true. No backend acts on the table yet, so no
pixel and no frame changes; the live call sites are driven by T83.7
(resume) and T83.8 (swap).

| gate | measured (green) | bound | injection (red) |
|---|---|---|---|
| headless (`lumen-static-shell/tests/shader_table.rs`, `--features truetype,shader`) | `Program::shaders(&TWO).host()`: **2** descs `["solid", "tint"]`; `Local::from_program(..).shaders()`: **2** descs `["solid", "tint"]` | 2, by name | `host()` omits `set_shaders`: both print **0** descs `[]`, red at "`Program::host` must install the program's shader table"; `Local::shaders` answering `ShaderTable::EMPTY`: host 2, session **0**, red at "`Session::shaders` must answer the host's table" |
| — accept | a program with no table, and a bare `Host::new`, answer **0** | empty | — |
| winit unit (`a_second_resume_still_has_the_images`) | backends handed the shader table **`[1, 2]`** (of 2 made); images still `[1, 2]` | `[1, 2]` | a once-only send in `resume_backend` (the plan's "only when `made == 1`"; the count is the test's, see the plan's "What building found"): **`[1]`**, red |
| `Hot` forwards `Session::shaders` (S1) | compiles | — | forward deleted: **E0046** "missing: `shaders`" (`cargo check -p lumen-static-winit --features hot`); no runtime injection is claimed |
| — glasswall's `Counting` forwards `Backend::set_shaders` | compiles, `dev-shape`'s `custom-backend` clippy line clean | — | forward deleted: **E0046** "missing: `set_shaders`" |
| lint (`dev-shape`): `cargo clippy -p lumen-static-shell -p lumen-static-winit --all-targets --features lumen-static-shell/shader,lumen-static-shell/truetype,lumen-static-winit/shader` | clean | -D warnings | an unused binding in `Host::set_shaders`: the new line fails `unused variable: injected`, the workspace `--features lumen-static/shader` line stays green |
| firmware (K83b, K4): `just firmware-delta e339bc1 firmware=0 firmware_interactive=0 firmware_bind=0 firmware_cascade=0` | Δ **0** `.text`, **0** `.rodata` on all four (30,606 / 63,480 / 34,014 / 32,694 B `.text`) | 0 | — (nothing in the firmware's graph changed: the shell, winit and reload crates only) |

## T83.5 — the GPU split, at `4700b1d` + T83.5 (2026-10-06)

`lumen-static-wgpu`'s default-off `shader` (winit's `shader` now forwards
it in a `gpu` build): `Gpu::set_shaders` compiles each table entry as
`src/prelude.wgsl` + its WGSL (vertex `lumen_vs`, fragment `fs_main`)
with an explicit per-shader bind group layout — binding 0 the prelude's
32 B `LumenFrame`, binding 1 `min_binding_size = uniform_size` — inside a
validation error scope, keyed by a hash of the source and uniform size.
A known bracket with uniforms of its shader's length becomes a
`Split::Shader` in paint order beside the glass splits: the shapes
pipeline runs up to it, one `LoadOp::Load` pass draws a full-target
triangle under the scissor `snap_out(clipped(rect, clip stack))` (the
stack starts with the damage region), and the fallback is skipped to the
matching end. Frame and uniform slots are 256-byte aligned in one
per-frame buffer read at dynamic offsets; no buffer or pipeline exists
before the first entry that compiles. Everything else — unknown id,
refused entry, wrong uniform length (counted in `ShaderStats::mismatches`)
— draws the fallback clipped to the rect, as the canvas does.

**K83a — the pass split's cost** (`cargo run --release -p
lumen-static-wgpu --features shader --example shader_cost`, 1080p, 32
trivial 64 × 64 shaders less the same list drawing their fallbacks, best
of 50 with `Gpu::wait`; passes counted 32 per frame, 1,696 over the run).

| adapter | 32 shaders | their fallbacks | per shader node | bound | verdict |
|---|---:|---:|---:|---|---|
| RTX 4070 (Vulkan, `DiscreteGpu`), three runs | 333.4, 336.7, 331.0 µs | 131.8, 147.1, 131.3 µs | **6.30**, 5.93, 6.24 µs | 50 µs | **does not fire** |
| llvmpipe (Vulkan, `Cpu`), for the record | 4,455.3 µs | 901.7 µs | 111.05 µs | — | not the rule's adapter |

The bound is derived from the RTX 4070's glass pass (`docs/results-glass.md`
§0h), so the discrete row is the rule's. llvmpipe is a CPU rasteriser,
which M51 §0 rule 1 says is not evidence about a GPU and which
`Prefer::Auto` never opens (it keeps the software canvas); there a
scissored full-target triangle costs 18× the discrete figure, and a
window that asks for it with `LUMEN_BACKEND=gpu` pays that per shader.

**Allocation.** The plan's bound, "≤ the empty frame's floor", cannot
hold with nothing of this crate's in it: a bracket is a render pass, and
wgpu allocates per pass — the two-shader frame (three passes) reads **98**
against the one-pass floor's **61** on both adapters. The gate measures
the frame minus `Gpu::resubmit`, wgpu's own submission of the same
instances, splits, slots and passes (`submit_floor`'s M51/T51.4 method
for a multi-pass frame); see the plan's "What building found".

**Gates** (`crates/lumen-static-wgpu/tests/shader.rs` unless named;
headless `Gpu`, 300 × 300; green on the RTX 4070 and on llvmpipe with the
same numbers, `just gpu` exit 0).

| gate | measured (green) | bound | injection (red) |
|---|---|---|---|
| scissor | shader colour inside **10,000** (setup), fallback pixels **0** (setup), ink outside **0**, passes 1 | 0 | `set_scissor_rect` skipped: outside **80,000**, red |
| clip | inside the clip **5,000** (setup), outside **0** | 0 | scissor from `rect ∩ region` without the open clips: outside **5,000**, red |
| region | B inside the region **5,000** (setup), B outside **0**, A kept **5,000** | 0 | scissor ignores the region: B outside **5,000**, A kept 0, red |
| parity (`tests/parity.rs`, `shader` region `[8, 44, 62, 58]`, `--features shader`) | **0** px over 8 (bound `2 × 0 + 2` = **2**, baseline 0 on discrete and on cpu), worst 0; GPU shader passes **1** (setup ≥ 1); page outside every region 0 | 2 | `collect` skips the bracket and runs no pipeline: passes **0**, red at the setup line; with the setup assertion disabled the region reads **500** px over 8 (the 50 × 10 rect left page-coloured), red |
| refusal | `set_shaders` answers **2**; one line each, naming `"broken"` (naga: "expected global item … found \"this\"") and `"oversize"` ("Buffer structure size 32 … greater than the given `min_binding_size`, which is 16"); then fallback pixels **20,000** (setup) and **0** px differ from the canvas | 2; 0 | error scope removed: the test panics in wgpu's handler, "**wgpu error: Validation Error**", red; binding 1's `min_binding_size: None`: answers **1** (the oversize struct accepted), red. (Dropping the guard instead of removing the scope pops it on drop and discards the error: "answered 0", red, but that is not the plan's injection) |
| uniform length | 8 bytes to a 16-byte shader: fallback **10,000**, shader colour 0, `mismatches` **1**, passes 0; accept: 16 bytes draw the shader on **10,000** px | as stated | the length check removed: fallback **0**, passes **1**, red |
| content key | id 0 red source → `(220, 30, 40)`; id 0 blue source → `(30, 60, 220)` | the new colour | cache keyed on `ShaderId`: the second frame stays `(220, 30, 40)`, red |
| order | glass panel (half-white tint) over a shader: GPU centre `[131, 212, 154]`, canvas over the identical fallback `[131, 211, 154]`, distance **1** (bound 8); setup: canvas panel over the page `[252, 252, 252]`, distance 121 | 8 | shader splits run after every glass split: GPU centre `[20, 160, 60]` (the raw shader over the panel), distance **111**, red. LIQUID's own 18/255 tint left that injection at distance 9, one over the bound, so the panel carries a half-white tint |
| allocation (`tests/alloc.rs`, `--features shader`) | two-shader frame **98**, wgpu's submission of the same frame **98**, difference **0**; shader passes in the frame 2 (setup); one-pass floor 61 for the record | 0 | each bracket's uniforms staged through a fresh `Vec`: 100 vs 98, difference **2**, red |
| lint (`just gpu`): `cargo clippy -p lumen-static-wgpu --features shader --all-targets -- -D warnings`, and the feature's `cargo doc -D warnings` | clean | -D warnings | an unused binding in `Gpu::set_shaders`: the new line fails `unused variable: injected`, the `--features lumen-static/shader` line stays green (`custom.rs` is not compiled there) |
| firmware (K83b, K4): `just firmware-delta 4700b1d firmware=0 firmware_interactive=0 firmware_bind=0 firmware_cascade=0` | Δ **0** `.text`, **0** `.rodata` on all four (30,606 / 63,480 / 34,014 / 32,694 B `.text`) | 0 | — (nothing in the firmware's graph changed: the wgpu and winit crates only) |

## T83.6 — `ShaderView`, at `106d16e` + T83.6 (2026-10-06)

`lumen_static_widgets::ShaderView<const N: usize>` behind the widgets'
default-off `shader` (which turns the core's on): an id, `N` uniform bytes
(`N` ≤ `MAX_UNIFORMS`, a compile-time assertion), the length it sends, a
`fn` fallback and an intrinsic size. Its `paint` is the bracket — shader,
fallback, end — and nothing else. `set_uniforms` and `set_shader` compare
before they store and answer whether they changed anything; the value form
of `#[bind]` writes through `BindValue<[u8; N]>`, whose answer marks the
node. `observe`: `Role::Image`, named by the author's label. The showcase's
`shader` feature adds the demo (Text group, after VectorText) and
registers its one-entry table (`catalog::SHADERS`, a two-colour blend);
its fallback is the same blend as a gradient, which `--shot` and the
default software window draw.

**Size.** `size_of::<ShaderView<16>>()` = **56 B** with `a11y` on (the
label's 16 B included). It is the widget's own: `Style` **84 B** and
`NodeState` **84 B** are the parent's, printed in a `shader` build by
`a_shader_view_grows_no_per_node_table` (bound 84 each; D4 not triggered).

| gate | measured (green) | bound | injection (red) |
|---|---|---|---|
| uniforms (`lumen-static-agent/tests/damage_shape.rs`, `--features shader`): `a_uniform_change_damages_only_its_shader`, two 120 × 40 views one container down, a real `Host` under `Boundaries<256>`, the bytes bound by `#[bind]` | marks `{ all: false, ranges: 1 }`; damage `[[0, 0, 120, 40]]` (the first view's box; the second's `[0, 48, 120, 88]` absent); the host's first bracket carries `[20, 160, 60, 255]`; `ui.lastChange` `["first"]` | first damaged and reported, second neither | `set_uniforms` stores and answers `false`: marks `{ all: false, ranges: 0 }`, damage `[]`, the list still carries `[200, 30, 40, 255]`, red at "the render is stale". `ui.lastChange` still names `first`, because the journal re-signs every leaf from the tree (see the plan's "What building found") |
| shift: `a_uniform_shift_does_not_report_later_shaders`, the first view's block 16 → 32 B by `set_shader(ShaderId(1), 32)` | the second bracket's uniform offset 16 → 32 (setup: it moved); `ui.lastChange` `["first"]` | second absent | `Cmd::hash_into` folds `uniforms.0` (T83.3's offset fold): `["first", "second"]`, red |
| `check_drive_coverage.sh` (`just static`) | exported **79**, driven 46, allowed **33**, missing 0 | 0 | the `undriven.allow` line dropped: "ShaderView: exported by lumen-static-widgets with no inventory and no undriven entry", missing 1, red |
| `check_observe.sh` (`just static`) | "every widget speaks or is listed" | — | `observe` removed: "no observe: …/shader.rs: ShaderView", red |
| `a11y_roles` (`lumen-static-widgets`), both builds | `a11y`: 65 rows, 50 impls, 49 covered, `ShaderView` excused by `FEATURE_GATED`; `a11y,shader`: 66 rows, 50 / 50 covered, `ShaderView role=Image label=Some("A blue plasma")` | 0 missing | the cfg'd `ROWS` entry dropped, `a11y,shader` run: "Node impls with no row in the table: [\"ShaderView\"]", red; the `FEATURE_GATED` line dropped, `a11y` run: the same, red. Accept: both green, same session |
| showcase (`cargo test -p showcase --features shader`) | 32 passed; `every_entry_paints_a_demo` reads the demo's bracket and its gradient fallback | as before | — (the default build's tests are `just test`'s) |
| lint (`dev-shape`): `cargo clippy -p lumen-static-widgets -p lumen-static-agent -p showcase --all-targets --features lumen-static-widgets/shader,lumen-static-agent/shader,showcase/shader -- -D warnings`, and `cargo doc -D warnings` with the widgets' `shader` | clean | -D warnings | an unused binding in `ShaderView::set_uniforms`: the new line fails `unused variable: injected`; the workspace `--features lumen-static/shader` line stays green (`shader.rs` is not compiled there) |
| firmware (K83b, K4): `just firmware-delta 106d16e firmware=0 firmware_interactive=0 firmware_bind=0 firmware_cascade=0` | Δ **0** `.text`, **0** `.rodata` on all four (30,606 / 63,480 / 34,014 / 32,694 B `.text`) | 0 | — (the widgets' `shader` is off in every firmware build; no default-build item changed) |

## T83.7 — `examples/shader`, driven live, at `a2af86b` + T83.7 (2026-10-06)

`examples/shader`: an animated plasma (`SHADERS[0]`, 16 B of uniforms —
the time) in a 400 × 240 `ShaderView` behind a frosted `Glass` card that
covers its bottom 40 px and the page below it, 480 × 400 logical. The
clock is `#[state]`, advanced by `App::tick`; the value form of `#[bind]`
writes the bytes. The fallback is a dark field with four discs orbiting
on the view's edge, half of each outside the box, so only the bracket's
clip keeps them in. Every feature is default-off and the binary requires
`shader` (the manifest says why; `cargo tree --workspace -e features -i
lumen-static` counts `lumen-static feature "shader"` **0** times, and **2**
with the widgets' `shader` named unconditionally). `just shader-drive`
(`scripts/shader_drive.sh`/`.py`, through `scripts/live/launch.py`, in
`scripts/live.sh`'s list) drives five legs, debug builds, at scale 1, on
the RTX 4070 (Vulkan).

| gate | measured (green) | bound | injection (red) |
|---|---|---|---|
| software, `Boundaries<256>` (`ui.screenshot` twice, 0.4 s apart) | view `[40, 40, 440, 280]`, card `[64, 240, 416, 360]`; centre `(26, 29, 43)` = the field, distance **0**; inside the view **32,096** px changed (setup); outside view ∪ card **0** | 4; > 0; 0 | `Canvas` ignores `Shader`/`ShaderEnd` (both arms of the raster loop empty): outside **9,429** px, red ("the fallback was not clipped to its bracket") |
| — `ui.lastDamage` per animated frame, six frames | `[[40, 40, 440, 280]]` each: 96,000 px of view ∪ card's 124,160, ratio **0.77**, outside **0** px | 1.00; 0 | — (the journal's, unchanged by a canvas fault) |
| software, `Everything` | centre distance **0**; inside **31,790** (setup); outside **0**; damage ratio **0.77**, outside 0 | as above | the same injection: outside **8,834** px, red |
| GPU, `Boundaries<256>` (`LUMEN_BACKEND=gpu`, `xwd`) | centre `(28, 109, 246)` then `(233, 136, 13)`: distance from the field **203**, **207**; inside **95,271** (setup); outside **0** | > 48; > 0; 0 | `set_scissor_rect` skipped: outside **67,755** px, red ("not bounded by its scissor") |
| — `LUMEN_TEST_RESUME=10000` round trip | round trips 1, GPU backends made **2**; centre after `(174, 1, 207)`, distance **164** | exactly (1, 2); > 48 | a once-only send in `resume_backend` (T83.4's injection shape): centre after `(26, 29, 43)`, distance **0**, red ("the second backend was not handed the shader table"); the call deleted outright: red at the first read, centre distance 0 on both |
| GPU, `Everything` | distance **203**, **207**; inside 95,259; outside **0**; after the round trip distance **174** | as above | the scissor injection: outside **67,724** px, red |
| GPU with `LUMEN_GPU_BAD_SHADER=1` | after 3 s `app.diagnostics` answers; centre `(26, 29, 43)`, distance **0** (the fallback); one line `lumen-static: shader "plasma" refused; its brackets draw their fallback: Validation Error / …` | 4; exactly 1 | the error scope removed (push and pop): the child exits **101**, `wgpu error: Validation Error`, and `launch.py` prints "child exited 101 before answering app.diagnostics" with the log tail |
| `the_fallback_is_clipped_to_the_view` (`cargo test -p shader --features shader`, `dev-shape`) | centre the field; non-page pixels in the strips left of and above the view at t = 0 **0** | 0 | `Canvas` ignores the bracket: **5,436**, red |
| lint (`dev-shape`, `just gpu`): `cargo clippy -p shader --all-targets --features shader,agent,everything`, and `--features gpu,agent` | clean | -D warnings | an unused binding in `fallback`: the featured line fails `unused variable: injected`; `cargo clippy -p shader` without features stays green (the binary is skipped) |
| firmware (K83b, K4): `just firmware-delta a2af86b firmware=0 firmware_interactive=0 firmware_bind=0 firmware_cascade=0` | Δ **0** `.text`, **0** `.rodata` on all four (30,606 / 63,480 / 34,014 / 32,694 B `.text`) | 0 | — (nothing the firmware links changed: an example, scripts and the launcher) |

**Recorded, not gated (A12): the whole-tree mark's price.** `app.perf`
on the software `Boundaries<256>` leg, tree **10** table nodes:

| frames | `layout_ns` | n |
|---|---:|---:|
| animated (the tick answers `true`, the whole tree marked) | p50 **4,871 ns** | 128 (the ring) |
| still (paused by the card's button; hover frames over it) | **not run**: layout ran in 0 of 24 frames, which `app.perf` reports as `null` (M53/T53.3) | 24 |

A ticking shader re-lays the whole tree out every frame; a still window
lays nothing out. On ten nodes that is 4.9 µs; it grows with the tree,
which is why M84's `App::tick_marked` (K84c) is where it is reopened.

## T83.8 — a WGSL edit reaches a running window, at `6412378` + T83.8 (2026-10-06)

`fixtures/hot-window`'s second node, `#swatch`, is a 320 × 40
`ShaderView<16>` with the fixture's default-off `shader` (an empty spacer
in the same slot without it, so the tree has one shape in every build): a
flat colour on its own WGSL line times a uniform gain of ones, over a grey
fallback. `scripts/hot_window_drive.sh` gains two GPU legs (`gpu`,
`gpu-everything`; `--gpu` runs them alone), built `hot agent gpu shader`,
`LUMEN_BACKEND=gpu`, read back with `xwd`, skipping with 77 without `xwd`
or an adapter. RTX 4070 (Vulkan), scale 1, release builds.

| gate | measured (green) | bound | injection (red) |
|---|---|---|---|
| the WGSL edit (`EDIT colour`, red → blue), `Boundaries<256>` | before: `#swatch` centre `(219, 31, 41)`, distance from red **0** (from the fallback's grey 97); reloads **1 → 2** (setup: +1), swapped in 1.16 s; after: `(31, 76, 229)`, distance from blue **1** (from red 188) | 6; +1 | the post-swap `Backend::set_shaders` in `redraw`'s swap branch skipped: after the swap the centre stays `(219, 31, 41)`, distance from blue **189**, red ("the new build's shader table did not reach the backend") |
| — the swap frame's `damaged_px` | **128,000** px, the oldest entry of the new session's fresh ring (2 entries; `since: 0` less `since: 1`) | = the window's 320 × 400 = 128,000 | — (the premise `ShaderId` rests on, printed) |
| — `Everything` | distance from red 0 → from blue **1**; reloads 1 → 2; swap frame **128,000** px | as above | — (the same call site; the strategy cannot reach it) |
| the error bar keeps the shader (the drive's broken `EDIT handler`, both legs) | the bar is up: foot pixel `(255, 255, 255)` → `(102, 30, 34)` (setup); `#swatch` under the bar: **0** of 12,800 px differ, centre `(31, 76, 229)`; the fix-back clears it with no swap | 0 | `Barred::new` clears the clone's `uniforms` (the literal's shape): the frame panics, "range end index 16 out of range for slice of length 0" (`display.rs`, `uniforms_of`), the window exits **101** and `launch.py` fails the leg on the unexpected panic |
| the other legs (`hot`, `static`, `everything`), unchanged | PASS: format edit 0.85 s, `#star` under the bar unchanged, the panic shown and cleared, the IME area inside `#field`, the shaping swap's `#hebrew` = the `Complex` render (`946486508a17152d`) on all three | their own | their own (M37, M80, M81, M82) |
| lint (`dev-shape`, `just gpu`): `cargo clippy -p hot-window --all-targets --features hot,agent,shader`, and `hot,agent,gpu,shader` | clean | -D warnings | an unused binding in `swatch_fallback`: the `shader` line fails `unused variable: injected`; the same crate without `shader` stays green |
| firmware (K83b, K4): `just firmware-delta 6412378 firmware=0 firmware_interactive=0 firmware_bind=0 firmware_cascade=0` | Δ **0** `.text`, **0** `.rodata` on all four (30,606 / 63,480 / 34,014 / 32,694 B `.text`) | 0 | — (a fixture and scripts only) |

## M83b closes — the verdict table (2026-10-06)

Every kill rule's number, fired or not (S5).

| rule | quantity | bound | measured | verdict |
|---|---|---|---|---|
| K83a | the pass split, per shader node, `shader_cost` at 1080p on the rule's adapter | ≤ 50 µs | **6.30** µs (RTX 4070; llvmpipe 111.05 µs for the record, not the rule's adapter) | does not fire (T83.5) |
| K83b | firmware `.text`/`.rodata`, all four binaries, `shader` off, per task | Δ 0 | Δ **0** in T83.0–T83.8, every task | holds |
| K83c | frame `idle (floor)`, `+render (damage)`, `keystroke + layout, 100 KB`, `cascade_deep` against T83.0's spread | inside | three inside; `cascade_deep` below (faster), attributed to the machine by identical `.text` and an empty symbol diff | holds (alignment outcome, T83.3) |
| K83d | `lumen-static-winit` default normal dependencies; the stripped release showcase | +0; under +3 MB | **96** (T83.0 96); **9,728,104 B** stripped (T83.0 9,719,656 B: **+8,448 B**; 13,055,368 B unstripped) | holds |
| K1 | `size_of::<Cmd>()`; `Style` / `NodeState` | 60 B both builds; unchanged | **60 B** off and on; **84 / 84 B** | holds |

What M83b closes from the iced list (`docs/plan-iced-roadmap.md`): custom
shaders — registered as data, run by the GPU behind a scissor, a fallback
everywhere else, and re-sent across a hot swap. `just ci-verify` on this
half is the coordinator's, and is cited in AGENT.md's queue entry.
