# M83 — a GPU you can extend: custom shaders, custom backends (includes D1 move b; shader feature per D3)

> **Written 2026-09-28 as a section of `docs/plan-iced-roadmap.md`;
> revised 2026-09-29 after that plan's seven-lens review, and moved to
> its own file. M83a building: T83.0 and T83.1 landed 2026-10-06
> (`docs/results-shader.md`).** The shared byte inventory, seams (S1,
> S2), test and doc discipline, and plan-wide kill rules K1–K4 are in
> `docs/plan-iced-roadmap.md` §2–§4 and bind this plan; this file does
> not restate them.

*Size L, split in two because the halves close different iced items
and the second needs the first's trait home. **M83a** (T83.0–T83.2,
size M): the backend seam. `Backend` and `Frame` move into
`lumen-static-shell` (D1 move b), a backend is built from a
raw-window-handle target, and `Program::backend_with` installs a
factory. It closes "custom renderer injection (window)". **M83b**
(T83.3–T83.8, size L): custom shaders. There is a `Cmd::Shader`
bracket whose software fallback is recorded in the same list, a
program-level `ShaderDesc` table delivered to the backend as data, a
GPU pass split with a scissor, and a `ShaderView` widget with an
example. It closes "custom shaders". M83b depends on M83a
(`Backend::set_shaders` lives on the moved trait). Neither depends on
another milestone in this plan. M82 and M89 interact with it (see the
last Decision). Each half closes with its own `just ci-verify <sha>`
(S5). Read against `553fa5e`, which is `92a4033` plus the M61 merge.*

**Decisions.**

- **`Backend` and `Frame` move to `lumen_static_shell::backend`; they
  are not merged with `Screen`** (D1 b; C5, B19, A4). The trait names no
  winit type. It needs `FrameSource`, `Color`, `Instant`,
  `ImageSource` and `Frame`, and all of them are shell or core types
  (`lumen-static-winit/src/backend.rs`, `surface.rs`'s `Frame`).
  `lumen-static-winit` re-exports both, so `lumen_static_winit::Backend`
  still resolves. What stays in winit is `Software` (softbuffer),
  `GpuBackend`, `AsScreen`, `frame_times` and `pick_backend`.
  - **Why not merged.** `Screen` is the narrow view that crosses the
    `Session` ABI (`Session::agent(&mut self, req, screen: &mut dyn
    Screen)`), and `Offscreen` implements it with no images, no resize
    and no name. Merging would change `Session::agent`'s parameter
    type and widen what the ABI stamp covers, for no user.
  - **Behind the shell's `truetype`** (second review T6). `FrameSource`
    and `Screen` live in `session.rs` and `FontStack` in `text.rs`, and
    both modules are `#[cfg(feature = "truetype")]` (shell `lib.rs`
    :42-47); the shell's `default = []`. So `pub mod backend` is
    `#[cfg(feature = "truetype")]` too, as are `Program::backend_with`
    and its stored field (`program.rs` is ungated). winit already
    names the shell with `features = ["truetype"]` (winit
    `Cargo.toml`:111), so nothing a window builds changes.
  - **Three provided methods become required** (second review A16,
    M8, R3). `fetch_rgba` (`None`), `set_images` (no-op) and
    `frame_rgba` (`None`) are provided today (winit `backend.rs`
    :57-59, :68-70, :94-96); only `take_images` is required. Once user
    code implements `Backend`, a forwarding wrapper is the likely
    implementor, and a wrapper that inherits `set_images` drops every
    image in silence: AGENT.md's provided-method trap. So all three
    are required in the move. `Software` already writes all three,
    `GpuBackend` writes `fetch_rgba`/`frame_rgba` as an explicit
    `None` ("does not render", as today), and winit's test `Fake`
    writes all three.
  - Cost: `lumen-static-shell` gains `raw-window-handle = { version =
    "0.6", features = ["std"], optional = true }`, enabled by the
    shell's `truetype` (`truetype = ["dep:fontdue",
    "dep:raw-window-handle"]`). That is 0.6.2, already in `Cargo.lock`
    through winit, softbuffer and wgpu. `std` (hence `alloc`) is named
    because the `Arc<H: ?Sized>` impls are behind `alloc`, and `just
    test-crates` builds the shell alone. The shell's default build
    gains +0 crates; with `truetype` it gains +1. The default
    `lumen-static-winit` graph gains +0 (measured in T83.1, kill rule
    K83d).
- **A backend is built from a `BackendTarget`, one path for built-in and
  custom backends.** It is `BackendTarget { window: Arc<dyn
  WindowHandles>, width: u32, height: u32, font: Option<FontStack>,
  accept_cpu: bool }`, in `lumen_static_shell::backend`.
  `WindowHandles: HasWindowHandle + HasDisplayHandle + Send + Sync`
  has a blanket impl. `softbuffer::Context::new`/`Surface::new` and
  `Gpu::for_window` (`W: wgpu::DisplayAndWindowHandle + 'static`) both
  accept `Arc<dyn WindowHandles>`, because rwh 0.6.2 implements both
  traits for `Arc<H: ?Sized>`. The size is carried because raw handles
  have none, and `Software::new`/`GpuBackend::new` read only
  `window.inner_size()` beyond the handles. Both constructors change
  to take `&BackendTarget`.
- **The factory: `Program::backend_with(impl FnMut(&BackendTarget) ->
  Option<Box<dyn Backend>> + 'static)`, stored as `BackendFactory =
  Box<dyn FnMut(&BackendTarget) -> Option<Box<dyn Backend>>>`.**
  - **Placement.** It is declarable on `Program` now that the trait
    lives in the shell. `run_program` moves it into a new
    `Config::backend_with` once at start, beside `images` (winit
    `lib.rs`, `run_program`). `pick_backend` calls it on every
    `resumed`, **in both of its `cfg` branches** (second review C14,
    T19). Today only the `#[cfg(feature = "gpu")]` branch calls
    `choose_backend`; the `not(gpu)` branch builds `Software::new`
    directly (winit `lib.rs`:280-305). After T83.2 the `not(gpu)`
    branch calls `choose_backend` too, with a `gpu` constructor that
    returns `None` (it keeps its "built without the `gpu` feature"
    line), so a factory is asked in a build without `gpu`, which is
    the build T83.2's live leg runs.
  - **Order.** `choose_backend` gains a `custom` slot, asked first
    with `accept_cpu = prefer == Prefer::Gpu`. `None` falls through to
    today's GPU-then-software decision, so the software fallback
    stands (C5). `Prefer::Software`, from the program or from
    `LUMEN_BACKEND=software`, skips the factory. The escape hatch for
    a driver bug keeps working however the application was written
    (`Prefer::from_env`'s rustdoc).
  - **Not reloadable, and why rule 3 does not apply** (A4, T20). The
    loader reads the static binary's `Program` once, and the hot boot
    never reads `backend`/`images` (winit `run_program`; nothing in
    `lumen-static-reload`). So the closure is loader code, held by the
    loader, and never a pointer into a dylib. A hot library's
    `backend_with` is ignored. The rustdoc and the handoff say so, and
    changing it needs a restart.
  - **A closure, not a `fn`** (M13). A custom backend that wants to
    share a device can capture one, so no later milestone needs a
    change to this signature. M89b (T89.10) does not route its shared
    device through the factory: factory backends make their own, and
    the built-in GPU backend shares one through `Weak<GpuContext>`.
- **Where each new program setting is consumed** (A5):

  | setting | side | read | across a swap |
  |---|---|---|---|
  | `Prefer` (existing) | window | once, into `Config` | loader's, unchanged |
  | `images` (existing) | window | once; handed back on suspend | loader's |
  | `backend_with` (shell `truetype`) | window | once; called per `resumed` | loader's; library's ignored |
  | `shaders` | host | `Program::host()`, every build | re-asked through `Session::shaders`, re-sent to the backend |

- **A shader is a bracket in the list, and its fallback is recorded
  inside it at paint time** (C18, A5, T20). A widget paints
  `p.shader(rect, id, &uniforms)`, then its fallback with ordinary
  `Painter` calls, then `p.shader_end()`. `DisplayList` records
  `Cmd::Shader { rect, id: ShaderId, uniforms: (u32, u32) }`, the
  fallback's commands, and `Cmd::ShaderEnd`.
  - **GPU.** On `Shader` it draws the pipeline and skips to the
    matching `ShaderEnd` (depth-counted).
  - **Software.** `Canvas` treats `Shader` as `PushClip(rect)` and
    `ShaderEnd` as `PopClip`, so the fallback draws clipped to the
    rect.
  - **One list for both backends.** The journal hash, `ui.lastDamage`
    and retention are backend-independent by construction.
  - **No fn pointer crosses the seam.** The fallback ran in the
    library, in the current build, when the list was recorded.
    `ui.screenshot` and `LUMEN_SHOT` show the fallback because that is
    what the software canvas drew.
  - **A broken shader shows its fallback.** Unknown id, failed
    compile or wrong uniform length: the GPU draws the bracket's
    contents like a clip (X6, below).
  - **No backdrop reads.** A user pipeline cannot sample the target
    (only `Glass` does), so a shader composes forward like a fill and
    needs no `observe_dep`.
- **Uniforms are content-hashed, never index-hashed** (S2; G8, B16,
  A8, T6).
  - **Storage.** `Painter::shader` receives the bytes (`&[u8]`, ≤ 256).
    `DisplayList` appends them to a per-frame `uniforms: Vec<u8>`
    (cleared with `text`) and stores `(offset, len)`, the `Cmd::Text`
    shape.
  - **Hashing.** `Cmd::hash_into` folds the **length**, never the
    offset. `DisplayList::hash_from` folds `hash_bytes` of the
    uniform bytes, as it does a text run's (`display.rs`, the
    M27/T27.3 comment on the `Text` arm). `Recording::shader` hashes
    the bytes it is handed, and it inks the rect. Wire tags are the next
    free tags at landing, printed (expected 12/13, after M82's `Path`
    at 11).
  - **`ShaderId` is not a per-frame store index.** It is a `u16` into
    the program's table, which is fixed for a build: the `ImageId`
    pattern, not the `RunId` one. A swap builds a new host whose first
    frame is a full repaint, so a WGSL edit under the same id cannot
    go stale. T83.8 prints that frame's `damaged_px` against the
    window area. The GPU's pipeline cache is keyed by a content hash
    of `(wgsl, uniform_size)`, never by `ShaderId`.
- **Shaders are data, delivered to the backend** (T20, A5).
  - **Registration.** `ShaderDesc { name: &'static str, wgsl: &'static
    str, uniform_size: u16 }` is a const constructor that asserts
    `uniform_size <= 256`. `Program::shaders(&'static [ShaderDesc])`
    installs the table in `Program::host()`, through
    `Host::set_shaders`, so every build and every headless host has
    it.
  - **The seam.** The new required `Session::shaders(&self) ->
    ShaderTable<'_>` hands it over. `ShaderTable` is an opaque type,
    empty without the shell's feature, which is `a11y_tree`'s
    unconditional-slot precedent (`session.rs`). The vtable is then
    one shape in every build. Its by-value layout still differs with
    the **shell's** `shader`, so the stamp carries a shell entry for
    it beside the core's (T83.3, "The reload stamp"; second review
    A7).
  - **Delivery.** The new required `Backend::set_shaders(&mut self,
    ShaderTable<'_>) -> usize` (count refused). The window calls it
    in `resume_backend`, which is every backend it makes, and after
    every successful swap. The backend compiles owned copies and keeps
    no borrow past the call. `Software::set_shaders` is a no-op,
    because its answer is the fallback already in the list. It is
    required so that a custom backend decides.
  - **The compiler is the forwarding gate.** `impl Session for Hot`
    (reload `lib.rs`:833) and `Local` (`session.rs`:556), the only
    implementors of the shell's `Session`, must each implement it. So
    must winit's test `Fake` backend, `Software` and `GpuBackend`.
    `fixtures/miri-reload`'s `Counter` is not one: it implements a
    local two-method stand-in `trait Session` (`main.rs`:21-35) and
    has no dependencies, so it is untouched (second review C8, G31,
    M24, R15). No stamp test and no drop-the-forward injection (S1;
    cross C4, G1, A15, M8).
- **The GPU pass split and its scissor** (T19, G13).
  - **The split.** `Split` becomes an ordered enum of glass and
    shader splits. The main pipeline runs up to the split's instance
    index. Then one pass (`LoadOp::Load`, into the target, not the
    swapchain) binds the user pipeline, sets the scissor and draws
    `0..3`: a full-target triangle, as `blit.wgsl`'s pass does. Then
    the main pipeline resumes.
  - **The scissor** is `snap_out(clipped(rect, clip))`. `collect`'s
    `clip` stack already holds the region's scope and every open
    `PushClip`, so it is `rect ∩ clip stack ∩ region`, once per region
    as `draw_regions_into` builds each region. Because the geometry
    covers the whole target, the scissor alone bounds the output, and
    dropping it paints the whole target, which the gates can see.
  - **What the user writes.** A framework prelude declares `struct
    LumenFrame { rect: vec4<f32>, viewport: vec2<f32>, scale: f32 }` at
    `@group(0) @binding(0)` and the vertex stage `lumen_vs`. The user
    writes `@group(0) @binding(1)` uniforms and `@fragment fn fs_main`.
  - **Pipeline state.** The blend state is the shapes pipeline's, and
    output is linear, as `linear(Color)` is. Frame globals and
    uniforms share one per-frame buffer at 256-byte-aligned slots with
    dynamic offsets. It grows by powers of two like `grads`, and no
    buffer or pipeline exists until the first registered shader.
  - **Layout, explicit per shader, never auto** (second review T9).
    Each pipeline gets its own `BindGroupLayout`: binding 1 has
    `has_dynamic_offset: true` and `min_binding_size =
    Some(uniform_size)`, and the bind group binds exactly
    `uniform_size` bytes of the slot, not the 256-byte slot. wgpu-core
    30.0.1 checks a shader's struct size against the layout at
    pipeline creation only when `min_binding_size` is set
    (`validation.rs`:580-620); without it the check is deferred to
    encoding (`LateMinBufferBindingSizeMismatch`, `binding_model.rs`
    :1464), outside the error scope below, where the default handler
    panics. A `uniform_size` of 0 declares no binding 1.
- **Invalid WGSL never takes the window down** (X6). `Gpu::set_shaders`
  compiles each module and pipeline under
  `device.push_error_scope(ErrorFilter::Validation)`, then
  `pollster::block_on(guard.pop())` (wgpu 30.0.1, `api/device.rs`).
  - **Why the scope is needed.** Unscoped, wgpu's
    `default_error_handler` panics (`backend/wgpu_core.rs`), winit has
    no `catch_unwind`, and M36's guard covers only application code
    inside the session (`session.rs`, "One guarded entry").
  - **What a refused shader does.** It prints one stderr line naming
    the shader and wgpu's message. Its brackets draw their fallback,
    and the count is returned. A uniform struct larger than
    `uniform_size` is one such refusal, because the explicit layout
    above moves its check into pipeline creation, inside the scope.
  - **The fault switch.** It mirrors `LUMEN_GPU_DROP_FIRST`: debug
    builds only, `LUMEN_GPU_BAD_SHADER=1` appends a syntax error to
    every source, so a live leg can exercise this.
  - **Changed from the review's fix.** The M36 screen was rejected
    for this case. The UI is correct with its fallback, and a panic
    screen would hide a working application over a cosmetic failure.
- **Feature placement: D3, with the variants gated at every use site**
  (owner decision, 2026-09-30; it reverses this plan's first draft,
  which left `Cmd::Shader`/`Cmd::ShaderEnd` unconditional).
  - **Behind the features.** The core feature is `shader = ["alloc"]`.
    It gates `Cmd::Shader`, `Cmd::ShaderEnd`, `ShaderId`,
    `Painter::shader`/`shader_end`, and `DisplayList`'s recording and
    uniform buffer. The shell's
    `shader = ["lumen-static/shader"]` gates `ShaderDesc`, the
    `ShaderTable` contents, `Program::shaders` and `Host::set_shaders`.
    wgpu has `shader = ["lumen-static-shell/shader"]`, which gates the
    pipeline code. winit has `shader = ["lumen-static-shell/shader",
    "lumen-static-wgpu?/shader"]`. The widgets have `shader =
    ["lumen-static/shader"]` for `ShaderView`. All are default-off and
    add no dependency.
  - **Every use site says so.** In `lumen-static`, each arm, method or
    impl that names a shader variant carries
    `#[cfg(feature = "shader")]`: `display.rs` (the enum, `DisplayList`,
    `Scaled`, `Cmd::hash_into`, `DisplayList::hash_from`), `wire.rs`
    (encode and decode), `retain.rs` (`Recording`), `overlay.rs`
    (`Deferring`), and the core's exhaustive test matches
    (`tests/overlay.rs`'s `cmds`, `tests/paint_containment.rs`). An
    integration test is compiled with its package's features, so the
    core's tests use the same `cfg`.
  - **The cross-crate problem.** Outside the core,
    `cfg(feature = "shader")` tests *that* crate's feature. Under
    Cargo's unification a crate whose own `shader` is off, in a graph
    where the widgets or wgpu turned the core's on, would drop its arm
    while the variant exists, and fail E0004. So:
    - `lumen-static` gains `links = "lumen_static"` and a `build.rs`
      that prints `cargo:shader=1` when `CARGO_FEATURE_SHADER` is set.
      A `links` package must have a build script, and Cargo then
      refuses two copies of `lumen-static` in one graph; each lock file
      in this repository has one (checked).
    - Each crate outside the core that matches `Cmd` gets a `build.rs`
      that reads `DEP_LUMEN_STATIC_SHADER` and, when it is set, emits
      `cargo:rustc-cfg=lumen_shader`; it always emits
      `cargo:rustc-check-cfg=cfg(lumen_shader)`, so the
      `unexpected_cfgs` lint knows the name when it is unset. A
      `rustc-cfg` applies to every target of the package, tests,
      benches and examples included.
    - Every use site there is `#[cfg(lumen_shader)] Cmd::Shader { .. }
      => …`. The crates and their sites, found by grep on `553fa5e`
      for exhaustive `match`es over `Cmd` and for the files T83.3 gives
      a shader arm:
      - `lumen-static-shell`: `src/software.rs` (the raster loop's
        exhaustive match; the clip pre-pass ends in `_ =>`),
        `src/svg.rs` (the exhaustive match; the first pass ends in
        `_ =>`), `src/contrast.rs` (`ground_at` ends in `_ =>`, but
        T83.3 adds the `Unknown("shader")` arm), `tests/svg.rs`
        (`kind`), `tests/clip_test.rs` (`render_cmds`), T83.0's
        `contrast_names_every_command`, and T83.3's
        `tests/alloc_gate.rs` arm;
      - `lumen-static-wgpu`: `src/lib.rs` (`collect`'s exhaustive
        match), `tests/parity.rs` (`kind`, and `KINDS`);
      - `benches`: `src/bin/retain.rs` (`ink`).
    - **`DEP_*` reaches direct dependents only**, and each of the three
      names `lumen-static` directly in `[dependencies]`
      (`crates/lumen-static-shell/Cargo.toml`,
      `crates/lumen-static-wgpu/Cargo.toml`, `benches/Cargo.toml`;
      checked). A future crate that matches `Cmd` through a re-export
      only must add the direct dependency, or its `build.rs` sees
      nothing.
    - **Not needing it.** `lumen-static-widgets` names `Painter::shader`
      only inside `ShaderView`, under its own `shader`, which turns the
      core's on, so its own feature is sound; it has no exhaustive
      `Cmd` match. `lumen-static-agent`, `lumen-static-winit`,
      `lumen-static-reload`, `fixtures/` and `examples/` match `Cmd`
      only with `if let`, `matches!` or a wildcard arm (checked).
    - **Where the stamp reads it.** Two entries, not one (second
      review A7): `cfg!(lumen_shader)` for the core's variant set, and
      the shell's own `cfg!(feature = "shader")` for `ShaderTable`'s
      layout across `Session::shaders`. The core can be on while the
      shell is off (an application that enables
      `lumen-static-widgets/shader` but not `lumen-static-winit/shader`),
      and the shell cannot be on while the core is off, so the two
      entries tell all three builds apart (T83.3).
    - **Cost.** Four build scripts (the core's and three dependents'),
      each compiled and run on the host. The core is `no_std`, but its
      `build.rs` runs on the host for the `thumbv7em-none-eabihf`
      firmware build too, and adds no dependency. The script count and
      the clean-build time of `just firmware-size` are printed before
      and after in `docs/results-shader.md`.
  - **What the firmware links.** Nothing of shader, by construction:
    with the feature off neither the variants nor the methods exist.
    K83b stays as a check with bound 0. The first draft's argument
    ("the firmware links no `Cmd` consumer") is no longer load-bearing.
  - **Why the methods are provided, not required** (A16, B6). A
    required method behind a feature would need its `cfg` on every
    implementor: six test painters, and `Checksum`
    (`fixtures/embedded`) and `Sink` (`benches --bin retain`) outside
    the core, where `fixtures/embedded` would need a `lumen_shader`
    build script of its own, or each fails to compile whenever
    anything in the graph enabled `shader`. The provided body is `push_clip(rect)`
    / `pop_clip()`. That is the documented degraded contract, the
    `gradient`/`glass` exception §2 allows, and it is *exactly
    correct* for a leaf that cannot run shaders, which draws the
    fallback clipped.
  - **The wrapper trap still applies.** A wrapper that inherits the
    default turns a shader into a clip. So `Scaled`, `Deferring` and
    `Recording` override both methods under the feature, and so does
    `CullTo` (M85b, `display.rs`) if M85/T85.8 has landed: a
    `ShaderView` inside a `ScrollView` paints through it, and an
    inherited default would draw only the fallback (second review
    A6/P6/R4). Whichever of M83/M85 lands second adds the override
    and its gate case (T83.3, "Deferring"). The injection
    below is the Deferring-inherits shape, which compiles only
    because the methods are provided.
- **Budgets** (D3, D4).
  - `Cmd` stays **60 B** (`tests/glass_cmd.rs`) in both builds.
    `Shader`'s payload is 16 + 2 + 8 = 26 B, inside `Gradient`'s 56 B
    bound, and exists only under the feature, so `size_of::<Cmd>() ==
    60` is asserted with the feature off (`just test`) and on
    (`dev-shape`'s `--features shader` line, which runs
    `tests/glass_cmd.rs` too and prints `Shader`'s payload beside 56).
  - `Style` and `NodeState` do not grow: unchanged from the parent
    commit's sizes, printed (M82/M86 may land first and move them
    within D4's 84/88 B; second review R16). `ShaderView` keeps
    its id and uniforms in its own widget struct, so no per-node table
    grows.
  - Firmware `.text` is **+0 B** on all four binaries with `shader`
    off, by construction, against the 28,616 B headline and
    `firmware_interactive`'s 61,922 B (19,998 B left under the 80 KB
    ceiling, D5; `docs/results-embedded.md`, "The headline re-measured
    at the M56 merge"). It is still measured (K83b).
  - Per host, with the feature on: one `Vec<u8>` on `DisplayList`
    (24 B). In every build: +1 vtable slot on each of `Session` and
    `Backend`.
- **Cross-milestone interactions, stated here so the assembler can
  carry them to §5.**
  - M82 builds no transform stack: its transform is a per-path `xf`
    that travels with each `Cmd::Path`, so a shader bracket is never
    under a transform and no arm is needed. If M82's stack reopens
    (its "Reopen" on a second transformed command kind), that task
    adds the bracket's fallback arm under a non-identity transform,
    because an axis-aligned scissor cannot bound a rotated rect.
  - M89: factory backends make their own device; the built-in GPU
    backend shares one through `Weak<GpuContext>`, in M89b (T89.10).
  - An animated shader rides `App::tick`, whose positive-`dt` `true`
    marks the whole tree under `Boundaries` (`event.rs`, `App::tick`'s
    rustdoc). The cost is recorded in T83.7. It is not redesigned
    here, and M84's K84c (`App::tick_marked`) is the reopen (A12).

**Tasks.**

*M83a — the backend seam.*

- **T83.0 — baselines and the gates that compile today** (S3).
  - **What changes.** New `docs/results-shader.md` (its
    `scripts/doc-paths.allow` entry, committed by M80/T80.0, removed
    in the same commit), holding the
    numbers every kill rule below compares against:
    - the four firmware binaries' `.text`/`.rodata`, by
      `just firmware-delta <parent> firmware=0 firmware_interactive=0
      firmware_bind=0 firmware_cascade=0` (M80/T80.0), Δ 0 on all
      four (`just firmware-size` prints only the paint-only
      `firmware`; second review B4);
    - the default `lumen-static-winit` normal-dependency count;
    - the shell-alone count;
    - the stripped release `examples/showcase` size;
    - the frame arm's `idle (floor)` and `+render (damage)` rows and
      `benches --bin cascade` `cascade_deep (whole table)`, eight runs
      a side under `just bench-lock`, spread printed;
    - `crates/lumen-static-wgpu/tests/alloc.rs`'s empty-frame floor;
    - the clean-build time of `just firmware-size`, the "before" for
      T83.3's four build scripts.
  - **Gate 1 — `scripts/check_shell_headless.sh`** (in `just static`,
    no compile). It runs `cargo tree -p lumen-static-shell -e normal
    --locked --offline` twice, with default features and with
    `--features truetype` (where T83.1's `backend` module lives,
    second review T6), and asserts none of `winit`, `softbuffer`,
    `wgpu` is in either. It prints each build's dependency count
    beside them. A `cargo tree` that fails (a stale lock, under
    `--locked`) is reported as that, never as a pass. This pins
    D1(b)'s premise before T83.1 relies on it.
    - Green today.
    - Injection: add `winit = "0.30"` to the shell's `[dependencies]`
      **and** run `cargo update --workspace --offline`, so the lock
      carries the edge (winit 0.30.13 is already in `Cargo.lock`) and
      `--locked` does not refuse first (second review G30). It goes
      red naming `winit`, not on cargo's lock error. Restore both
      files.
    - Accept direction: `raw-window-handle` (T83.1), with its lock
      update, passes; it appears only in the `truetype` run.
  - **Gate 2 — `contrast_names_every_command`** in
    `lumen-static-shell/tests/`.
    - **What it does.** It builds one of each `Cmd` through an
      exhaustive `match`, with no wildcard, puts a text run's centre
      on it, and asserts the ground `text_contrast` reports:
      `Opaque`, `Unknown(reason)`, or `Miss` for the non-filling
      kinds (`Text`, `Line`, `StrokedRect`, clips). `contrast.rs`'s
      `ground_at` ends in `_ => Ground::Miss`, so a new command is
      silently measured against whatever lies beneath it. This test
      forces T83.3 to state `Shader`'s row.
    - Setup landed: the report's measured+unmeasured count equals the
      number of variants, printed.
    - Injection: delete `ground_at`'s `Glass` arm, and glass reports
      `Miss`; the test names `Glass`.
  - **Doc and static obligations (S4).** None beyond the results doc,
    and the new script's name added to AGENT.md's `just static` list.
- **T83.1 — `Backend`/`Frame` into the shell; `BackendTarget`.**
  - **What changes.**
    - New module `lumen_static_shell::backend`, holding `Backend`,
      `Frame`, `BackendTarget` and `WindowHandles`.
    - winit re-exports them.
    - The module, `raw-window-handle` (optional) and, in T83.2,
      `Program::backend_with` are behind the shell's `truetype`
      (Decisions).
    - `fetch_rgba`, `set_images` and `frame_rgba` become required;
      `GpuBackend` and the test `Fake` write the bodies they inherited
      (Decisions).
    - `Software::new` and `GpuBackend::new` take `&BackendTarget`.
    - `Surface` holds `softbuffer::Surface<Arc<dyn WindowHandles>,
      Arc<dyn WindowHandles>>`.
    - `resumed_impl` builds the target from the window once.
    - No behaviour changes.
  - **Same commit.**
    - Rustdoc on the moved trait. Its "Why it takes a `FrameSource`"
      section stays, and it gains "why it lives in the shell".
    - `book/src/backends.md` anchors updated if they named
      `backend.rs`.
    - The handoff §2 item 4 paragraph gains a DECIDED line: "the
      backend seam is the shell's (M83/T83.1)".
    - `docs/results-shader.md` gets the crate-count rows.
    - `scripts/doc-identifiers.allow` loses `BackendTarget` and
      `WindowHandles`, which this task builds (second review M6; the
      block's header reports a built name left there as stale).
  - **Gates.**
    - `check_shell_headless.sh`, now printing `raw-window-handle` as
      the one addition, in the `truetype` run only.
    - `cargo check -p lumen-static-shell` (default features, no
      `truetype`) compiles, which is the build `just test-crates`
      makes of the shell alone (second review T6). Injection: drop the
      `cfg` on `pub mod backend`, and it fails naming `FrameSource`.
    - The winit graph: `cargo tree -p lumen-static-winit -e normal
      --prefix none | sed 's/ (\*)$//' | sort -u | wc -l` (K2's exact
      command) printed against T83.0's number (bound: equal; K83d).
    - `just resume-drive` green on all three legs. It proves the moved
      trait still presents and keeps images across a resume (its own
      M47/T47.7 injections stand); it does **not** observe constructor
      arithmetic.
    - M57's D5 parity pass in `just widget-drive` (full run; a filtered run
      skips it, `scripts/drive/run.py`:164-172) is the oracle for
      what `Software::new` reads from the target: the live canvas,
      built by the moved constructor, must equal a headless
      `showcase --demo` render byte for byte, entry by entry.
    - `just agent-drive` green.
  - **Injection.** `Software::new` ignores `target.font` (stores
    `None`). The live canvas then draws block-metric glyphs where the
    headless render draws the face, and M57's D5 fails naming the first
    entry with text, its differing region hash printed.
    - **Rejected: the swapped `height, width`** (second review C5,
      G12, T5). `Win::redraw` calls `backend.resize(w, h)` from
      `window.inner_size()` before every present (winit `lib.rs`
      :797-799), and `Software::resize` → `surface::sync_size`
      resizes the canvas in place (`backend.rs`:133-135), so a
      swapped size is corrected before the first frame. Both of
      `resume_drive.py`'s shots (`a = shot()`, `b = shot()`) come from
      the same constructor, so they would agree either way.
    - A headless render (`LUMEN_SHOT`, `showcase --demo`) never builds
      a `Backend`, so alone it cannot gate this move; `showcase
      --demo`'s is the reference M57's D5 compares against.
- **T83.2 — `Program::backend_with`.**
  - **What changes.**
    - `Program::backend_with`, `BackendFactory` and
      `Config::backend_with`.
    - `choose_backend(prefer, custom, gpu, software)`.
    - `pick_backend` builds the `BackendTarget` and passes the factory
      from **both** `cfg` branches; the `not(gpu)` branch calls
      `choose_backend` with a `gpu` constructor that returns `None`
      (Decisions, "Placement"; second review C14, T19).
    - `examples/glasswall` gains a `custom-backend` feature. Its
      factory wraps `Software` in a forwarding counter whose `name()`
      is `counting (software) #n`, and the window already prints it on
      every `resumed`. The counter forwards every method, the three
      newly required ones included (T83.1), because the compiler now
      asks for each.
  - **Same commit.**
    - The rustdoc on `Program::backend_with` states the table above:
      read once, not reloadable, `Prefer::Software` skips it.
    - The handoff §7 gets a DECIDED line.
    - `book/src/backends.md` gets a section including glasswall's
      factory by named anchor.
    - `docs/results-shader.md` gets the stripped showcase delta beside
      the 3 MB bound (B9).
    - `scripts/doc-identifiers.allow` loses `BackendFactory`.
  - **Gate 1: `crates/lumen-static-winit/tests/cpu_adapter.rs`'s table
    gains a `custom` column** (runs everywhere, no window). The rows,
    each printed:
    - `(Auto, custom Some)` → custom;
    - `(Gpu, custom None)` → `Gpu { accept_cpu: true }`;
    - `(Software, custom Some)` → software, custom not called;
    - `(Auto, custom None, gpu None)` → software;
    - `(Auto, custom Some, gpu absent)`, the `not(gpu)` branch's
      constructor → custom.
    - Injection: ask `gpu` before `custom`, and row 1 prints `Gpu`.
    - Injection: call `custom` under `Software`, and row 3's call
      count prints 1.
  - **Gate 2: `just resume-drive` gains a `custom-resume` leg**
    (glasswall `--features agent,custom-backend`, `LUMEN_TEST_RESUME`).
    `resume_drive.py` reads the leg's log for `counting (software)`
    lines. It prints the count against a bound of exactly 2 and names
    backend #2. The existing before/after pixel diff then proves the
    factory's backend honoured `take_images` (M47/T47.7), but only
    once the "before" frame is shown to hold the images (second review
    A16, M8, R3): a counter that drops the source loses the wallpaper
    from both captures, and the diff reads 0.
    - Setup landed: the first line is present before the resume
      (count ≥ 1 at 4 s).
    - Setup landed: in the first capture, a pixel the wallpaper covers
      and no panel does (glasswall's `Wallpaper` source, `main.rs`
      :194-201) differs from the page colour; both colours printed.
    - The leg reads `ui.screenshot` (the canvas source), so the
      counter's `frame_rgba` is exercised on every shot.
    - Injection: `Config::backend_with` is `take()`n on first use
      (FnOnce semantics, R15-F6's "taken once" shape). The count prints
      1, and backend #2's name is `software (softbuffer)`.
    - Injection: the counter's `set_images` drops the source (`let _ =
      images;`). The wallpaper-pixel setup prints the page colour and
      fails.
    - Injection: the counter's `frame_rgba` answers `None`.
      `ui.screenshot` reports "does not render" and `resume_drive.py`
      exits naming it.
    - Injection: the `not(gpu)` branch of `pick_backend` builds
      `Software::new` directly, as it does today. The count prints 0.
    - Accept: the unmodified leg prints 2, with the wallpaper pixel.
  - **Why no second strategy leg.** The factory runs in `resumed_impl`
    before the session's first frame. The invalidation strategy lives
    in the host and cannot reach it, so a leg per strategy would drive
    the same code twice.
- **M83a closes** (S5; second review M13). After T83.2, in its own
  commit:
  - `docs/results-shader.md` records K83d's two rows (winit graph,
    stripped showcase) with their bounds;
  - `just ci-verify <sha>` green on that commit, cited in a new
    AGENT.md queue entry "M83a — the backend seam: DONE", so M89b's
    dependency on T83.2 points at a closed half.

*M83b — custom shaders (after M83a).*

- **T83.3 — the core bracket.**
  - **What changes.**
    - Core feature `shader`, and under it `ShaderId`, `Cmd::Shader`,
      `Cmd::ShaderEnd`, `Painter::shader`/`shader_end` (provided),
      `DisplayList::uniforms`, and the arms in `hash_into`/`hash_from`
      (length/bytes), each carrying `#[cfg(feature = "shader")]`.
    - Overrides in `Recording`, `Scaled` (rect scaled, bytes untouched)
      and `Deferring`, under the feature.
    - `wire.rs`: the next free tags at landing, printed (append-only;
      expected 12/13, since 0–10 are taken, `PopClip` is 10, and M82's
      `Path` takes 11), encoded and decoded under the feature. A
      feature-off decoder meets those tags as unknown and refuses the
      buffer through the existing unknown-tag path.
    - **The cfg plumbing** (Decisions, "Feature placement"):
      `links = "lumen_static"` and `crates/lumen-static/build.rs`
      (`cargo:shader=1` when `CARGO_FEATURE_SHADER` is set); a
      `build.rs` in `lumen-static-shell`, `lumen-static-wgpu` and
      `benches` that turns `DEP_LUMEN_STATIC_SHADER` into
      `cargo:rustc-cfg=lumen_shader` and always declares it with
      `cargo:rustc-check-cfg=cfg(lumen_shader)`. Every arm outside the
      core is `#[cfg(lumen_shader)]`.
    - **The reload stamp.** `shader` changes `Cmd`'s variant set, so it
      joins `abi_fields`' feature list (`session.rs`;
      `docs/plan-iced-roadmap.md` §2.3) in this commit. The entry reads
      `cfg!(lumen_shader)`, not the shell's `cfg!(feature = "shader")`,
      because the core's feature can be on while the shell's is off,
      and the variant set is the core's. The shell's own `shader`
      feature is declared here too (`dev-shape`'s shell `--features
      shader` lines below need it), so it gets its own second entry,
      `cfg!(feature = "shader")`, in this commit: `ShaderTable`,
      which T83.4 puts behind it, crosses `Session::shaders` by value
      (second review A7). The core's `build.rs` is not
      under the reload crate's source hash, which hashes `src/` and the
      manifests; the manifest's `links` line is, and the feature entry
      is what tells two builds apart.
    - Consumers, each named in this commit (C24):
      - winit `hot::Barred::new` (`lib.rs`:1709, second review A13):
        the error bar's copy of the session's list is the workspace's
        only `DisplayList { .. }` literal (`grep -rn`, printed), and
        it copies `cmds`, `text`, `bounds`, so a `Cmd::Shader` under
        the bar would index an empty `uniforms`. `DisplayList` gains
        `#[derive(Clone)]` and `Barred` copies with `src.clone()`, so
        `uniforms` travels with the commands and winit needs no
        `lumen_shader` cfg of its own. Gate in T83.8. This is the
        design for whichever of M82/T82.5 and T83.3 lands first: if
        T82.5 has landed, `DisplayList` already derives `Clone` and
        `Barred` already clones, and T83.3 only adds `uniforms`.
      - `Canvas`: the bracket is a clip.
      - `Svg`: `svg.rs` wraps the contents in a `clipPath` group.
      - `contrast.rs`: text centred inside an open bracket is
        `Unknown("shader")`, and the fallback's fills are not ground
        for it.
      - `tests/svg.rs`'s exhaustive table, naming `Shader` and
        `ShaderEnd` under `#[cfg(lumen_shader)]`, so the table names
        every kind in either build.
      - `lumen-static-shell/tests/clip_test.rs`'s `render_cmds`.
      - The core's `tests/overlay.rs` (`cmds`) and
        `tests/paint_containment.rs`, under the core's own `cfg`.
      - T83.0's gate 2.
      - `benches/src/bin/retain.rs`'s `ink`.
      - `lumen-static-wgpu/tests/parity.rs`'s `kind` table, which gets
        `Shader` and `ShaderEnd` rows under `#[cfg(lumen_shader)]`,
        and its `KINDS`, which counts them only in that build.
      - wgpu `collect`: the bracket is a clip here, and becomes a real
        split in T83.5.
    - **`Painter` implementors, and what each does.** Leaf
      `DisplayList`, which records. Wrappers `Scaled`, `Deferring`,
      `Recording` and, if M85/T85.8 landed first, `CullTo`, which
      override. Leaves `Checksum`
      (`fixtures/embedded`), `Sink` (`benches --bin retain`) and the
      test painters (`Echo`, `Surfaced`, `Plain`, `Counting`, `Null`,
      `Flat`) inherit the clip default, which is correct for a leaf.
    - `dev-shape` gains `cargo test -p lumen-static --features shader
      --test shader_cmd --test glass_cmd` and the shell's `--features
      shader` test lines, because `--workspace` unification must not
      decide whether they run. It also gains the two exhaustiveness
      builds below.
  - **Same commit.** Rustdoc on both methods (the provided-default
    argument above, and "wrappers must override" as `glass`'s says);
    the `Cmd::Shader` rustdoc (the bracket, the length-not-offset
    rule, citing M27/T27.3); the `results-embedded.md` row;
    `docs/results-shader.md`; the handoff §2 item 4 DECIDED line;
    `scripts/doc-identifiers.allow` loses `ShaderId`, `ShaderEnd`,
    `CARGO_FEATURE_SHADER` and `DEP_LUMEN_STATIC_SHADER` (second review
    M6).
  - **Gates** (`crates/lumen-static/tests/shader_cmd.rs` unless named):
    - **Exhaustive both ways** (`dev-shape`). `cargo check --workspace
      --all-targets` (feature off, the default) and `cargo check
      --workspace --all-targets --features lumen-static/shader`
      (unified on). `lumen-static-wgpu` is included: a `check` needs no
      adapter. So every `match` over `Cmd` is compiler-checked with the
      variants present and absent. Setup landed: the first injection
      below is the proof that the "on" build really has the variants
      and the `lumen_shader` arms, since only then can deleting an arm
      fail it.
      - Injection: delete one use site's `#[cfg(lumen_shader)]` arm
        (`benches/src/bin/retain.rs`'s `ink`). The "on" build fails
        E0004 naming that match; the "off" build stays green.
      - Injection: remove `cargo:rustc-cfg=lumen_shader` from
        `lumen-static-shell/build.rs`. The "on" build fails in the
        shell, E0004 at `software.rs`'s and `svg.rs`'s matches.
      - Injection: write one shell arm as `#[cfg(feature = "shader")]`
        (the crate's own feature). The "on" build fails E0004 there,
        which is the unification defect this design exists to stop.
      - Accept: both builds green, in the same session.
      - Lint (second review X7): `dev-shape` also runs `cargo clippy
        --workspace --all-targets --exclude lumen-static-wgpu
        --features lumen-static/shader -- -D warnings`, beside its
        feature-off line (justfile:112), and `just gpu` runs `cargo
        clippy -p lumen-static-wgpu --all-targets --features
        lumen-static/shader -- -D warnings` beside justfile:371, so
        the `cfg(lumen_shader)` arms are linted, not only checked.
      - Doc (index §3.1): the feature adds public items (`ShaderId`,
        `ShaderDesc`, `ShaderView`), so `dev-shape` also runs
        `RUSTDOCFLAGS="-D warnings" cargo doc -p lumen-static -p
        lumen-static-shell -p lumen-static-widgets --no-deps --features
        lumen-static/shader,lumen-static-shell/shader,lumen-static-widgets/shader`,
        each crate's line added by the task that adds its items.
    - **Stamp** (`just abi-stamp`). The two entries are listed as
      `lumen_shader` (core) and `shader` (shell).
      - Pair 1: a library built with `--features lumen-static/shader`
        and the shell's feature off is refused by a loader built
        without either, and the refusal names `lumen_shader`.
        Injection: that entry reads `cfg!(feature = "shader")` (the
        shell's). That library is then accepted, which is red.
      - Pair 2 (second review A7): a library with core and shell on is
        refused by a loader with core on and shell off, and the
        refusal names `shader`. Injection: drop the shell entry, and
        the library is accepted while `ShaderTable` differs across the
        call, which is red.
      - Accept: a library and loader built alike are accepted, in all
        three builds.
    - **Shift** (S2). Nodes A and B each paint a bracket, B after A. In
      list 2, A's uniforms are 32 B instead of 16, so B's offset moves.
      Printed: B's offset in each list (setup: they differ), and B's
      `hash_from` in each (bound: equal).
      - Injection: `hash_into` folds `uniforms.0`, and the two
        signatures differ.
    - **Same slot, new bytes.** B's 16 bytes change at the same offset
      and length, and the signatures must differ.
      - Injection: `hash_from` skips uniform bytes, and they are equal.
    - **Retention.** Two frames through `Recording` into
      `retain::Incremental`, where only A's uniform bytes change. The
      damage's area over A's rect is printed, with a bound of 1.00, and
      B's rect is not in the regions.
      - Injection: remove `Recording`'s `shader` override. The
        inherited clip default hashes only the rect, and damage prints
        0.00× (`Damage::None`).
    - **Deferring.** `deferring_forwards_the_bracket`: direct vs
      `Deferring`-wrapped lists are equal and contain `Cmd::Shader`
      (setup: count 1).
      - Injection: remove the override, and the assertion names
        `Shader` (4 in, 4 out, but `PushClip` where `Shader` was).
      - The same check for `Scaled`, with the rect scaled and the
        bytes identical.
      - If M85/T85.8 landed first, a `CullTo` case (second review
        A6/P6/R4): a bracket inside the cull rect is forwarded as
        `Cmd::Shader`. Injection: delete `CullTo`'s `shader` override,
        and `PushClip` appears where `Shader` was. If M83 lands first,
        T85.8 adds this case.
    - **Software** (`lumen-static-shell/tests/shader_software.rs`). A
      bracket whose fallback fills a rect 20 px larger than the
      shader's is rendered byte-identical to `push_clip(rect)` + that
      fill + `pop_clip()`. Setup: fallback pixels inside the rect > 0,
      printed.
      - Injection: `Canvas` ignores `Shader`, and the pixels outside
        the rect are printed non-zero.
    - **Contrast.** T83.0's gate 2 gets `Shader`'s row,
      `Unknown("shader")`.
      - Injection: no arm, and it reports `Miss`/measured.
    - **Wire.** A round trip keeps the uniform bytes.
      - Injection: decode drops them, and the lists differ.
    - **Allocation.** `lumen-static-shell/tests/alloc_gate.rs` gets a
      `shader` arm: a warm frame with two brackets makes 0
      allocations. The recipe counts the arm present (≥ 1), as it does
      the truetype arms.
      - Injection: `DisplayList::shader` pushes into a fresh `Vec`, and
        the allocations are printed non-zero.
    - **Size.** `size_of::<Cmd>() == 60` (`tests/glass_cmd.rs`,
      existing), asserted in both builds, with `Shader`'s 26 B payload
      printed beside the 56 B bound in the "on" build.
    - **Firmware** (D3, K83b, K4). `just firmware-delta <parent>
      firmware=0 firmware_interactive=0 firmware_bind=0
      firmware_cascade=0` (`scripts/firmware_delta.sh`, K4's one
      comparator, M80/T80.0's positional form)
      prints the four binaries' `.text`/`.rodata` against the parent
      commit, bound Δ 0 B each (second review G13: `check_text.sh`
      keeps no baseline, only the ceiling and a 1% headline
      tolerance). `just test`'s `check_text.sh` ceiling gate still
      runs.
      - Injection: drop the `cfg` on `ShaderId` **and** on
        `Painter::shader`/`shader_end` together, so the type and both
        provided methods exist in every build (dropping the method's
        alone does not compile, because its signature names the gated
        `ShaderId`; second review T20). Each binary's Δ is printed
        above 0, which proves the comparator sees the two vtable
        slots.
- **T83.4 — the shader table, host to backend.**
  - **What changes.**
    - `ShaderDesc` and `ShaderTable`.
    - `Program::shaders`, installed by `Program::host()` through
      `Host::set_shaders`.
    - `Session::shaders`, required, and implemented by `Local` and
      `Hot`, the shell's only implementors (`fixtures/miri-reload`'s
      `Counter` implements a local stand-in trait and is untouched;
      second review C8, G31, M24, R15).
    - `Backend::set_shaders`, required. `Software` is a no-op, and
      `GpuBackend` stores the table until T83.5.
    - `resume_backend` calls `set_shaders` on every backend it makes.
    - The swap branch in `redraw` calls it after `swap` returns true.
      From M89 the swap branch becomes `swap_all`'s per-slot adopt
      step, which calls `set_shaders` on each slot's backend
      (`docs/plan-windows.md`, second review A8).
  - **Same commit.** Rustdoc: the table above, and "no borrow survives
    the call". The handoff §7 DECIDED line: "shaders are re-sent on
    every swap and resume". `session.rs`'s `Session` rustdoc lists the
    method. `scripts/doc-identifiers.allow` loses `ShaderDesc` and
    `ShaderTable`.
  - **Gates.**
    - **Headless.** `Program::shaders(&TWO).host()` and
      `Local::from_program(..).shaders()` both report 2 descs, printed
      by name.
      - Injection: `host()` omits `set_shaders`, and it prints 0.
    - **Winit unit test.** `a_second_resume_still_has_the_images`'s
      `Fake` records `set_shaders` calls. Both backends were given the
      table (printed `[1, 2]`).
      - Injection: `set_shaders` only when `made == 1`, and it prints
        `[1]`.
    - The `Hot` forward is a compile error if missing (S1). No runtime
      injection is claimed for it.
  - **Why no live gate here.** No backend acts on the table yet. The
    call sites are driven live by T83.7 (resume) and T83.8 (swap).
- **T83.5 — the GPU split.**
  - **What changes.** In `lumen-static-wgpu`, behind `shader`:
    - `Gpu::set_shaders`, with the content-keyed pipeline cache and
      the error scope;
    - the prelude;
    - `Split::Shader`, and `collect`'s bracket skip;
    - the uniform slots;
    - `LUMEN_GPU_BAD_SHADER`;
    - `GpuBackend::set_shaders` forwarding;
    - the uniform-length check at draw (a mismatch draws the fallback
      and is counted);
    - new `examples/shader_cost.rs` (the `glass_cost.rs` method, with
      `Gpu::wait`).
    - `just gpu` gains `cargo test -p lumen-static-wgpu --features
      shader --test shader`.
  - **Same commit.** The `Gpu::draw_regions` rustdoc: "No scissor is
    needed" gains "except a user shader's pass, which draws a
    full-target triangle and is bounded by its scissor alone".
    `docs/results-shader.md` gets the cost table.
    `docs/plan-gpu-fidelity.md` is not touched (nothing in it
    changes). `scripts/doc-identifiers.allow` loses
    `LUMEN_GPU_BAD_SHADER`.
  - **Gates** (`crates/lumen-static-wgpu/tests/shader.rs`, headless
    `Gpu`, 300×300, adapter required under `just gpu`):
    - **Scissor.** The shader writes its uniform colour, with a 100×100
      rect at (50, 50). Printed: inside = 10,000 (setup) and outside =
      0 (bound 0).
      - Injection: skip `set_scissor_rect`, and outside prints 80,000.
    - **Clip.** The same shader inside `PushClip([50, 50, 100, 150])`.
      Outside the clip prints 0.
      - Injection: scissor from `rect ∩ region` without the clip
        stack, and it prints 5,000.
    - **Region.** Frame 1 in colour A. Frame 2 is uniform colour B,
      drawn with `draw_regions` over a region covering the rect's left
      half. Printed: B inside the region = 5,000 (setup), and B outside
      the region = 0.
      - Injection: the scissor ignores the region, and it prints 5,000.
    - **Parity.** WGSL identical in effect to its fallback (a solid
      uniform fill). GPU vs `Canvas` is checked per region by
      `tests/parity.rs`, under the `Shader` row's bound, which is
      derived as the others are (`2 × b + 2` over a measured
      baseline). The measured difference is printed beside it.
      - Injection: `collect` skips the bracket and runs no pipeline,
        and the rect is page-coloured and out of bound.
    - **Refusal.** Invalid WGSL, and a WGSL whose uniform struct is
      larger than `uniform_size`: `set_shaders` returns 2, and each
      message names its shader. A draw then equals `Canvas`'s fallback
      pixels.
      - Injection: remove the error scope. The test process panics
        with `wgpu error:`, which is red.
      - Injection (second review T9): binding 1's `min_binding_size:
        None`. The oversize struct is no longer refused at pipeline
        creation: `set_shaders` prints 1, or the draw panics with
        `LateMinBufferBindingSizeMismatch`; either is red.
    - **Content key.** `set_shaders(red at id 0)` draws red, and
      `set_shaders(blue at id 0)` draws blue.
      - Injection: key the cache on `ShaderId`, and it stays red.
    - **Order.** A shader, then a `Glass` panel over it: the panel's
      centre differs from the page colour by more than the parity
      bound (distance printed).
      - Injection: shader splits run after all glass splits, and the
        panel blurs the page.
    - **Allocation.** `tests/alloc.rs` gets a warm frame with two
      shaders. Allocations are ≤ the empty-frame floor, both printed.
      - Injection: a per-frame `Vec` for uniform staging, and it
        prints above the floor.
    - Accept for every gate: the unmodified build prints within bound
      in the same session.
- **T83.6 — `ShaderView`.**
  - **What changes.** `lumen_static_widgets::ShaderView<const N:
    usize>`, behind the widgets' `shader` feature, holds `id`,
    `uniforms: [u8; N]` (N ≤ 256, const-asserted) and `fallback:
    fn(&mut dyn Painter, Rect, &[u8])`. `set_uniforms` writes the
    bytes, and `#[bind]` marks the node, as any write does. Its
    `paint` is the bracket. `size_of::<ShaderView<16>>()` is printed
    (it is not in `Style`/`NodeState`, so D4 is not triggered).
  - **Same commit (S4).**
    - `observe`: `Role::Image`, with the author's label as its name.
    - `scripts/drive/undriven.allow`: `ShaderView | M83 | takes no
      input; examples/shader's drive owns its pixels`.
    - An `examples/showcase` entry under a new showcase `shader`
      feature, which shows the fallback on the default software
      backend.
    - The rustdoc, book `backends.md` via a named anchor in
      `shader.rs`.
    - `scripts/doc-identifiers.allow` loses `ShaderView`. The first
      draft's "no M83 name is listed there (checked)" was false: the
      M80–M90 block pre-seeds twelve M83 names, and each task above
      deletes the ones it builds (second review M6). `RunId` stays: it
      is named only as the pattern `ShaderId` rejects and is never
      built.
    - **`a11y_roles.rs` row** (second review M4). `node_impls()`
      (`lumen-static-widgets/tests/a11y_roles.rs`:165-185) scans
      source text and ignores `cfg`, so the feature-gated `impl Node
      for ShaderView` is counted in every build, while `dev-shape`
      builds the test with `--features a11y` only (justfile:90). So
      `ROWS` gains a `#[cfg(feature = "shader")]` entry, and the test
      gains a `FEATURE_GATED: &[(&str, &str)] = &[("ShaderView",
      "shader")]` list: a name on it is excused from `missing` when
      its feature is off and must have a row when it is on.
      `dev-shape` gains `cargo test -p lumen-static-widgets --features
      a11y,shader --test a11y_roles`.
  - **Gates**, in `lumen-static-agent/tests/damage_shape.rs`, whose
    dev-dependency on the widgets gains `shader`:
    - **Uniforms.** `a_uniform_change_damages_only_its_shader`: two
      `ShaderView`s. A `set_uniforms` on the first reports the first
      and not the second.
      - Setup: the first is reported.
      - Injection: `ShaderView::set_uniforms` writes without marking.
        No change is reported, and the render stays stale.
    - **Shift.** `a_uniform_shift_does_not_report_later_shaders`: the
      first switches to a shader with a 32 B table size, and the
      second must not report. This is T83.3's shift, end to end
      through the journal.
      - Injection: T83.3's offset fold.
    - The existing `check_drive_coverage.sh` and `check_observe.sh`
      pass. Injection: drop the allow line or the `observe`, and each
      names `ShaderView`.
    - `a11y_roles`, both builds. Injection: drop the cfg'd `ROWS`
      entry, and the `a11y,shader` run names `ShaderView` missing;
      drop the `FEATURE_GATED` line, and the `a11y` run names it.
      Accept: both runs green in the same session.
- **T83.7 — `examples/shader`, driven live.**
  - **What changes.**
    - New `examples/shader`: an animated plasma `ShaderView` behind a
      `Glass` card, with a time uniform advanced in `App::tick`, and
      features `gpu`, `agent` and `everything`, the showcase's
      pattern.
    - New `scripts/shader_drive.sh`/`.py` and `just shader-drive`,
      through `scripts/live/launch.py`.
    - `scripts/live.sh`'s `ALL` gains
      `"shader-drive|./scripts/shader_drive.sh"` in this commit, so
      `just live`, and the `just ci-verify` that runs it, aggregates
      the drive; `check_live_list` (`live.sh`:50-58) fails any
      launcher user left out (second review G34, M9).
  - **Legs** (both strategies, S3):
    - **Software, `Boundaries` and `Everything`.** `ui.screenshot`:
      fallback pixels inside the rect, and 0 changed pixels outside
      `rect ∪ glass ink` across two animated frames. `ui.lastDamage`
      per animated frame against `rect ∪ glass ink` is printed as a
      ratio with bound 1.00.
    - **GPU, `Boundaries` and `Everything`** (`LUMEN_BACKEND=gpu`,
      read back with `xwd`, as `resume-drive` does).
      - Plasma pixels differ from the fallback's at the rect's centre
        (distance printed).
      - 0 changed pixels outside `rect ∪ glass ink` between captures.
      - A `LUMEN_TEST_RESUME` round trip keeps the plasma. This is
        `resume_backend`'s `set_shaders`, driven live.
    - **GPU with `LUMEN_GPU_BAD_SHADER=1`.** The window stays up, the
      agent answers `app.diagnostics` after 3 s, and the rect shows
      the fallback. The log names the shader.
  - **Injections.**
    - Software: `Canvas` ignores `Shader`, and the outside count
      prints non-zero.
    - GPU: skip `set_scissor_rect`, and the outside count prints
      non-zero.
    - Bad-shader leg: remove the error scope. The child dies and
      `launch.py` prints its exit and the log tail.
    - Resume: drop the `set_shaders` call in `resume_backend`, and
      after the resume the rect shows the fallback.
  - **Recorded, not gated (A12).** `app.perf`'s `layout_ns` median per
    animated frame against the same window still, with the tree size,
    in `docs/results-shader.md`. It is the whole-tree mark's price.
  - **Same commit.** The book `backends.md` shader section (the
    example by named anchor), and the handoff §2 item 4 DECIDED
    line.
- **T83.8 — a WGSL edit reaches a running window; M83 closes.**
  - **What changes.**
    - `fixtures/hot-window` gains a `ShaderView` under a `shader`
      feature.
    - `scripts/hot_window_drive.sh` gains a `--gpu` leg (`--features
      hot,agent,gpu,shader`, `LUMEN_BACKEND=gpu`, `xwd` readback).
      It skips with exit 77 without an adapter, and a skip is not a
      pass (M43).
  - **Gate.** Edit the copy's WGSL colour from red to blue.
    - After the swap the rect's centre is blue, with the distance
      printed.
    - The swap frame's `damaged_px` equals the window area (printed).
      That is the full-repaint premise the `ShaderId` decision rests
      on.
    - Setup: the reload count went up by 1.
    - Injection: skip `set_shaders` at the post-swap site (the swap
      branch in `redraw`; from M89, `swap_all`'s per-slot adopt step),
      and the centre stays red.
  - **Gate — the error bar keeps the shader** (second review A13).
    The `--gpu` leg's failed-build step (the drive's existing broken
    edit) reads the `ShaderView` region by `xwd` before and while the
    bar is up: unchanged, with the differing-pixel count printed
    (bound 0) and setup "the bar is up" (its region differs, printed).
    The `xwd` readback is what reads GPU pixels before M88/T88.2 lets
    the agent do it.
    - Injection: `Barred::new` rebuilds the list without `uniforms`
      (today's literal), and the region differs or the frame panics on
      the empty buffer, either of which is red.
  - **Same commit.**
    - The AGENT.md queue entry for M83 gains M83b's `just ci-verify
      <sha>` beside the M83a one already cited by "M83a closes" (S5).
    - `docs/results-shader.md`'s verdict table.
    - `book/src/gaps.md` drops the two iced items.

**Kill rules** (written before building; a rule that fires stops its
task and is recorded in `docs/results-shader.md` with the number).

- **K83a — pass-split cost.**
  - **Measure.** `cargo run --release -p lumen-static-wgpu --features
    shader --example shader_cost`, 1080p: 32 trivial 64×64 shaders,
    minus the same frame drawing their fallbacks, divided by 32.
  - **Bound.** 50 µs per node. The derivation: a 400×200 glass panel,
    blur included, is seven passes in 0.12 ms
    (`docs/results-glass.md` §0h), about 17 µs a pass, and a trivial
    shader pass may cost at most three times that.
  - **When it fires.** T83.5 batches consecutive brackets that share a
    pipeline into one pass, drawing instanced quads with the clip in
    vertex data, the shapes pipeline's way, and re-measures. If it
    still fires, shaders ship with the per-node figure in the
    rustdoc, and "many shader nodes per frame" is stated unsupported.
- **K83b — firmware.**
  - **Measure.** `just firmware-delta <parent> firmware=0
    firmware_interactive=0 firmware_bind=0 firmware_cascade=0`
    (`scripts/firmware_delta.sh`, K4's one comparator), all four
    binaries, `shader` off. `check_text.sh` keeps no baseline, so it
    is the ceiling check, not this rule's measure (second review G13).
  - **Bound.** Δ 0 B `.text` and `.rodata` against the parent commit,
    in every task that touches `lumen-static` (K4); T83.0's numbers
    are the milestone's record.
  - **True by construction, kept as a check.** With the variants,
    methods and table all behind the feature (owner, 2026-09-30),
    nothing of shader exists in a feature-off build. The check stays
    because a `cfg` missed on one item is exactly what it would see.
  - **When it fires.** T83.3 does not land until the leaked symbol is
    found by symbol diff and moved behind the feature.
    `firmware_interactive` has 19,998 B left under the 80 KB ceiling
    (D5), and this milestone spends none of it.
- **K83c — the frame with shaders unused.**
  - **Measure.** `benches-competitive/arms/frame` (package
    `lumen-static-frame`) rows `idle (floor)` and `+render (damage)`,
    `benches --bin cascade` `cascade_deep (whole table)`, and
    `benches --bin textarea`'s `keystroke + layout` as the typing
    proxy (no typing arm exists, B10). Eight runs a side under `just
    bench-lock`, `shader` off, against T83.0.
  - **Bound.** Inside the baseline's spread.
  - **When it fires.** The first draft's fallback, moving the variants
    behind the feature, is now the design, so there is no shader code
    in a feature-off build to blame. The cost is attributed by a symbol
    diff of the release frame binary, and code alignment is ruled out
    as M26 Phase D did (two arms that disagree are a fact about the
    arms). T83.3 does not land until it is attributed, and then one of
    two outcomes (second review B18):
    - **Alignment** (no new frame-path symbol in the diff): T83.3
      lands with both numbers and the symbol diff recorded in
      `docs/results-shader.md`.
    - **A shader symbol in the default build**: that is a missed
      `cfg`, the K83b case; T83.3 stops until it is gated, and K83c is
      re-measured.
- **K83d — default graph.**
  - **Measure.** `cargo tree -p lumen-static-winit -e normal --prefix
    none | sed 's/ (\*)$//' | sort -u | wc -l` (K2's exact command;
    without the `sed` it reads 117, not 96) with default features, and
    the stripped
    release showcase.
  - **Bound.** +0 crates, and a showcase delta under 3 MB (B9;
    `docs/results-a11y-platform.md`'s bound, not a11y's measured
    4.12 MiB).
  - **When it fires.** T83.1 stops. A new crate means
    `raw-window-handle` came in with a feature set that pulls more,
    and it is pinned to `std` only.

**What building found (M83).**

- T83.0: **an exhaustive match alone does not force a sample.**
  `contrast_names_every_command` (in
  `lumen-static-shell/tests/contrast_commands.rs`) states each command's
  ground in a match with no wildcard, so a new variant does not compile
  until it has a row; but a row for a variant no sample builds compiles
  and never runs. So the test also keeps a `KINDS` count (12 at landing:
  `Path` is M82's) asserted against the distinct kinds its samples built,
  the `tests/parity.rs` pattern; T83.3 adds two samples and moves it to
  14 under `cfg(lumen_shader)`. Dropping the `Image` sample reads "kinds
  11 built of 12", red by name. Opaque fills and a sampled gradient are
  one row shape, "measured against this colour"; a miss is told apart by
  measuring against a page colour no sample uses.
- T83.0: **`check_shell_headless.sh` meets a stale lock first, as the
  second review predicted.** With `winit = "0.30"` added and no lock
  update, `cargo tree --locked` refuses to run and the script reports
  "cargo tree failed for the default build -- not a pass", exit 1; only
  after `cargo update --workspace --offline` does it go red naming
  `winit v0.30.13` in both builds. The accept run (`raw-window-handle`
  optional under `truetype`, with its lock update) read 8 crates default
  and 16 → 17 under `truetype`: the +1 the Decisions predict, in the
  `truetype` run only.
- T83.0: **AGENT.md's `just static` list is not edited here.** The task
  says to add the script's name to it; AGENT.md says "the recipe is the
  list" and is the coordinator's, so the script is in the `static`
  recipe with its M83/T83.0 comment, and the coordinator is told. It has
  no `scripts/selftest/` plant, as `check_fill_paint.sh` (M82/T82.3) has
  none; its red and accept runs are in `docs/results-shader.md` T83.0.

- T83.1: **`check_shell_headless.sh` prints counts, not names**, so
  "printing `raw-window-handle` as the one addition" is read from the
  diff of the two sorted `cargo tree --features truetype` lists (16 -> 17,
  one line, `raw-window-handle v0.6.2`), recorded in
  `docs/results-shader.md` T83.1; the script is unchanged. The
  `widget-drive` D5 oracle ran filtered (`scripts/widget_drive.sh Label`):
  since M81/T81.6 a filtered run compares the entries it names, so the
  plan's "full run" is not needed for the injection to show (30,453
  bytes differ on all three legs), and the full catalogue drive stays
  the coordinator's `just ci`.

**What the review found (M83).**

- **C5** → changed. The trait moves to the shell (D1 b), so
  `Program::backend_with` is declarable, and the factory is a winit
  `Config` field read once. `accept_cpu` passes through
  `BackendTarget` and `Option` keeps the fallback.
- **C18** → changed. The fallback is recorded at paint time inside a
  `Shader`…`ShaderEnd` bracket. The GPU skips it and `Canvas` draws it
  clipped, so one list serves both backends and the hash is
  backend-independent.
- **C24** → adopted. T83.3 lists `Painter` implementors (leaf
  `DisplayList`; wrappers `Scaled`, `Deferring`, `Recording`; leaves
  `Checksum`, `Sink` and the test painters) separately from `Cmd`
  consumers (`Canvas`, `Gpu`, `Svg`, `wire.rs`, `contrast.rs`,
  `hash_into`/`hash_from`, `retain` bench ink).
- **G8** → adopted. Contents are folded through `hash_from` and
  `Recording`, the length through `hash_into`, and the two-node shift
  and same-slot gates are in T83.3/T83.6.
- **G13** → adopted. A full-target triangle, and the no-scissor
  outside count is printed (80,000); the region gate cuts the rect.
- **G14** → adopted. The live `custom-resume` leg counts 2 factory
  backends, and the injection is the take-once factory. The second
  review added a wallpaper-pixel setup, `set_images`/`frame_rgba`
  injections and the `not(gpu)` branch (second review A16, M8, R3,
  C14, T19).
- **G28** → adopted. Every gate in T83.0–T83.8 names a
  mechanism-breaking injection, including the example (old T83.3).
- **B6** → changed per D3. `shader` is gated and firmware is measured
  at Δ 0 B per binary (K83b). `Checksum`/`Sink` are listed, and need
  no change because the methods are provided.
- **B16** → adopted. See G8, and the injection folds the offset.
- **B19** → changed. See C5.
- **A4** → changed. The factory lives on `Program` (now legal) but is
  window-side, read once from the static binary, and not reloadable.
  `Prefer` stays on `Program`.
- **A5** → adopted. There is a host/window table. The fallback runs at
  record time in the library, and WGSL is re-sent via
  `Session::shaders` → `Backend::set_shaders` on resume and swap.
- **A8** → adopted. `Painter::shader` takes bytes, the list assigns
  offsets internally, and there is the shifted-index no-damage gate.
- **A12** → adopted as recorded cost. The per-animated-frame
  `layout_ns` goes into T83.7's results row. A narrow mark path is
  M84's K84c (`App::tick_marked`), with that as the reopen.
- **A16** → changed. The methods are provided (argued: required under
  a feature is non-additive), with the injection "Deferring inherits
  the default". `Checksum`/`Sink` are listed, and M82's per-path `xf`
  (no stack) is stated to need no bracket arm.
- **T6** → adopted. `shader(rect, id, &[u8])`, the length/bytes
  split, and the shift gate.
- **T19** → adopted. The scissor is `snap_out(clipped(rect, clip))`
  per region, with gates for a partial region and a clipped scroller.
- **T20** → changed. The factory is window-side and read once, and
  shaders go as data through a required `Backend::set_shaders`. The
  hot gate edits WGSL on the GPU (T83.8). The software-fallback edit
  is rejected: the fallback is ordinary paint code, which
  `just hot-window-drive` already swaps.
- **T21** → changed per D3. See B6, with K83b as the firmware rule.
- **M2** → changed per D3. See B6. `check_text.sh` stays, and no
  headline moves because Δ is 0.
- **M13** → adopted. The factory is a boxed `FnMut`, so a custom
  backend can capture a device. M89b (T89.10) shares the built-in
  backend's device through `Weak<GpuContext>`, and factory backends
  make their own; recorded for §5.
- **X6** → changed. The validation error scope is adopted. A refused
  shader draws its fallback and logs, rather than showing M36's
  screen. `LUMEN_GPU_BAD_SHADER` gives it a live leg.
- **Owner, 2026-09-30** → changed. The variants are feature-gated too,
  at every use site: `cfg(feature = "shader")` in the core,
  `cfg(lumen_shader)` from a `build.rs` elsewhere, and two workspace
  builds that check exhaustiveness both ways (T83.3). This reverses
  the first draft's "variants unconditional" refinement and removes
  K83c's fallback, which was that same move; K83b holds by
  construction.
- *Cross, applied here.*
  - **C4/G1/A15/M8.** New `Session` methods are required, and the
    compiler is the forwarding gate.
  - **M3.** No trailing docs task.
  - **G29.** Live legs under both strategies, or a stated reason.
  - **B9/B10.** Kill arms are named and the 3 MB bound is used.
  - **B23.** Figures cite their results doc.

**What the second review found (M83).**

- **C5, G12, T5** → changed: T83.1's swapped-size injection is
  rejected (the per-frame `resize` corrects it, winit `lib.rs`
  :797-799); the injection is `Software::new` ignoring `target.font`,
  seen by M57's D5 parity in `widget-drive`, and `resume-drive` is stated to
  gate presence and image retention only.
- **C8, G31, M24, R15** → adopted: `fixtures/miri-reload`'s `Counter`
  dropped from the `Session` implementor lists (it implements a local
  stand-in, `main.rs`:21-35); `Local` and `Hot` are named.
- **C14, T19** → adopted: both `cfg` branches of `pick_backend` ask the
  factory; a `not(gpu)` row in `cpu_adapter.rs`, and an injection on
  the live leg (count 0).
- **G13** → adopted: T83.3's firmware gate and K83b use `just
  firmware-delta <parent> firmware=0 firmware_interactive=0
  firmware_bind=0 firmware_cascade=0` (K4's comparator).
- **T20** → adopted (with G13's second half): the firmware injection
  ungates `ShaderId` with both methods, so it compiles.
- **G30** → adopted: the gate 1 injection and accept runs update the
  lock (`cargo update --workspace --offline`), so the red run names
  `winit`, not cargo's lock error.
- **G34, M9** → adopted: T83.7 adds `shader-drive` to `live.sh`'s
  `ALL`.
- **B18** → adopted: K83c states both outcomes (alignment lands with
  numbers; a leaked symbol stops T83.3).
- **A7** → adopted: a second stamp entry for the shell's own `shader`
  (`ShaderTable`'s layout), added in T83.3 with the feature, and an
  `abi-stamp` pair core-on+shell-on vs core-on+shell-off with a
  drop-the-entry injection.
- **A16, M8, R3** → adopted: `fetch_rgba`, `set_images`, `frame_rgba`
  become required in T83.1; T83.2's leg asserts a wallpaper pixel in
  the first capture, with `set_images`-drop and `frame_rgba`-`None`
  injections.
- **T6** → adopted: `lumen_static_shell::backend`,
  `Program::backend_with` and its field are behind the shell's
  `truetype`; `raw-window-handle` is optional under it (shell default
  +0 crates, `truetype` +1); `check_shell_headless.sh` runs both
  builds, and T83.1 gates `cargo check -p lumen-static-shell`.
  `Program::shaping` is M81's (request filed).
- **T9** → adopted: an explicit per-shader `BindGroupLayout` with
  `min_binding_size = uniform_size`, so the oversize struct fails
  inside the error scope; injection `min_binding_size: None`.
- **M4** → adopted: T83.6 gives `ShaderView` a cfg'd `a11y_roles.rs`
  row, a `FEATURE_GATED` list checked both ways, and a `dev-shape`
  line with `a11y,shader`.
- **M6** → adopted: T83.6's "(checked)" claim was false; T83.1–T83.6
  each delete the allowlist names they build (`RunId` stays, never
  built). §3.2's widget-only scope is the index's (request filed).
- **M13** → adopted: an "M83a closes" step after T83.2 with its own
  `just ci-verify`; T83.8 adds M83b's.
- **Owner D6** (2026-10-02, `text_spaced` and `fills` accepted
  firmware growth) → no change: this file attributes neither to D3,
  and `shader` stays at Δ 0 B. D7 and X5 do not touch M83.
- **B4, X7** (from a cross-file request) → adopted: T83.0 records
  the four binaries by `scripts/firmware_delta.sh <parent>`, Δ 0 on
  all four (`just firmware-size` prints only `firmware`; K83b already
  used the comparator); T83.3 adds `--features lumen-static/shader`
  clippy lines to `dev-shape` (justfile:112) and `just gpu`
  (justfile:371).
- **B16, G3 count** (from a cross-file request) → adopted: every
  `just firmware-delta` call takes M80/T80.0's positional form
  `<parent> firmware=0 firmware_interactive=0 firmware_bind=0
  firmware_cascade=0` (T83.3, K83b, G13's line). The count half does
  not apply: this file never counts `deferring_forwards_every_command`
  (its "4 in, 4 out" is `deferring_forwards_the_bracket`'s own list).
- **T13/R10** (from a cross-file request) → already satisfied:
  T83.3 and K83b used `firmware_delta.sh` since G13, and neither text
  the request quotes (`just firmware` against T83.0;
  `firmware-size` / `check_text.sh` in K83b) is in the file. Its
  `base=<parent> bound=0` syntax was not adopted: B16's positional
  form is M80's, which builds the script.
- **A13** (from a cross-file request) → changed: winit `Barred::new`
  (`lib.rs`:1709, the only `DisplayList` literal) joins T83.3's
  consumers; `DisplayList` derives `Clone` and `Barred` copies with
  `src.clone()`, so winit needs no `lumen_shader` cfg. The gate is in
  T83.8's `--gpu` leg (its `xwd` readback already reads GPU pixels),
  not a `just gpu` test; injection: the literal without `uniforms`.
- **A6/P6/R4** (from a cross-file request) → adopted: `CullTo` joins
  the wrapper-trap list and T83.3's implementor list if T85.8 landed
  first; `deferring_forwards_the_bracket` gains a `CullTo` case,
  injection "delete its override → `PushClip` where `Shader` was",
  added by whichever of M83/M85 lands second.
- **R16** (from a cross-file request) → adopted: Budgets asserts
  `Style`/`NodeState` unchanged from the parent commit's sizes,
  printed. No "16.0 KB" literal is in this file, so that half is moot.
- **A8** (from `plan-windows.md`) → adopted: T83.4's swap-branch line
  and T83.8's injection name `swap_all`'s per-slot adopt step from
  M89.
- consistency-3 item 6 → T83.1's and K83d's crate counts use K2's
  exact command, with the `sed` step.
- consistency-3 item 8 → T83.0's firmware row is `just firmware-delta
  <parent>` with all four bounds 0; the bare script bounds only
  `firmware_interactive`.
- consistency-3 item 11 → derive-`Clone` plus `src.clone()` is the
  design for whichever of T82.5/T83.3 lands first; T83.3 only adds
  `uniforms` if T82.5 landed.
- consistency-3 item 12 → T83.3's lint block adds a `cargo doc
  --features .../shader` line for `ShaderId`, `ShaderDesc`,
  `ShaderView`.
- consistency-3 item 19 → "D5 parity" reads "M57's D5" (four sites).
- consistency-3 item 20 → "seven test painters" → six.
- consistency-3 item 24 → T83.0 removes `docs/results-shader.md`'s
  `doc-paths.allow` entry.
