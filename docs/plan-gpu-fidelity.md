# M51 — the GPU path: what its parity gate can see, and what it costs

> **T51.3 DONE 2026-09-25 (early landing).** `glass_deps.rs` fires
> `dilate`'s `MAX_DEPS` escape: 4 panels go partial 5/5, 5 panels go
> `Damage::All` 5/5, 0 px differ; with the escape deleted (`ecaf67f`)
> 1,822 px differ. Software only, so llvmpipe is irrelevant to it: it
> proves the same thing on any adapter.
>
> **Written 2026-09-24.** Closes R9-F1, R9-F2, R9-F3,
> R9-F6, R9-F7, R9-F8, R9-F9 and R9-F10 from M38
> (`docs/results-deep-review.md`), as re-triaged against `main` at
> 1717808 on 2026-09-24.
>
> **Phase 4** of the fix order. It comes after M43, for two reasons.
> First, its gates run in M43's local CI runner. Second, **a missing
> adapter must not count as a pass**, and that rule is M43's R17-F6: the
> `gpu` recipe sets a require-an-adapter switch, and a skip becomes a
> refusal. M51 depends on it and does not re-implement it. It also comes
> after M40, whose R9-F5 adds a fractional-scale arm to `parity.rs`.
> T51.1 restructures that file with M40's arm already in it. T51.3 and
> T51.6 may run earlier, and each says why.
>
> **The machine this plan was written on has no usable GPU.** The
> nvidia driver and its library disagree, so wgpu opens **llvmpipe**,
> Mesa's CPU rasteriser, and the review ran every GPU measurement on it.
> So every gate below says two things: what it proves on llvmpipe, and
> what needs a real adapter. A claim about a discrete GPU that nobody
> could run is recorded as **owed**, not as passed.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and prints the measured quantity beside its bound.*

---

## 0. The rules for this milestone

1. **An adapter that is absent is a refusal, not a skip.** Each GPU gate
   prints the adapter's name and device type on its first line. Under
   M43's switch, it fails if no adapter was found. llvmpipe counts as an
   adapter for **parity**, because the atlas, the quads and the damage
   logic are CPU-side code that runs the same on it. It does not count
   as evidence about a discrete GPU's rasterisation baseline.
2. **A bound belongs to a region, not to a frame.** The whole-frame
   bound hid a 56 px defect under an 86 px allowance (R9-F7). Each named
   shape in the parity frame gets its own count and its own bound, set
   from its measured baseline with a stated headroom.
3. **"Every command in the vocabulary" is made mechanically true, or
   not said.** Three files say it (`parity.rs`, `docs/results-gpu.md`
   §2, `svg.rs`), and two of them are false (R9-F2, R9-F10). The fix is
   an exhaustive `match` over `Cmd` in the test that builds the frame,
   so a new command is a compile error there, not a sentence that went
   stale.

## 1. What is wrong, at 1717808

The re-triage checked that `crates/lumen-static-wgpu/src` has no commits
since the review's base. So the review's numbers stand, and they are
cited, not restated.

### 1.1 Parity cannot see three things it claims to hold (R9-F7, R9-F2, R9-F1)

`the_two_backends_agree_within_the_stated_tolerance`
(`crates/lumen-static-wgpu/tests/parity.rs:138`) applies one bound over
the whole 240×180 frame: fewer than 0.2% of pixels may differ by more
than 8 levels. On llvmpipe, the frame's baseline already uses most of
that allowance. Making the software border's ring coverage binary adds
fewer pixels than the headroom left. So the border comment
(`parity.rs:56-61`), which says this frame "holds them to" the shared
`sd_round_box` expression, is not enforced on the adapter the docs name
(R9-F7). The code comment's "~20× headroom" (`parity.rs:153`) is from a
discrete-GPU run that predates the border by eleven days. Nobody has
measured that adapter's baseline with the border in the frame.

`frame()` (`parity.rs:39-63`) draws no `Cmd::Image`. Deleting the GPU
image arm leaves the whole workspace green on a live llvmpipe adapter.
The software half has a coarse gate in `examples/glasswall`
(`the_wallpaper_is_a_picture`, `main.rs:893`), which catches an image
that is not drawn but not a wrong scale, tint or alpha. So: no GPU
pixel test, and no software-against-GPU parity for images (R9-F2, as
the verifier reworded it).

Parity's only text is "parity" at 14 px. That fits a 64² atlas, so
shrinking the atlas to 64 leaves parity green (R9-F1's inverse check).
The overflow path described in §1.2 is unreachable from every test.

### 1.2 The glyph atlas gives up silently, every frame, at 2× and 3× (R9-F1)

`GLYPH_ATLAS` is 1024 (`crates/lumen-static-wgpu/src/lib.rs:74`). When a
frame overflows, the atlas is reset and the frame is built once more.
If the frame still does not fit an empty atlas, the give-up branch
(`lib.rs:699-706`) submits what fits, and nothing counts or logs it.
Three places say this never happens:

- `docs/results-gpu.md` §4 says the frame is "built again rather than
  finishing with glyphs missing";
- `Overflow`'s rustdoc (`lib.rs:242-243`) says "not by dropping a
  glyph";
- `atlas.rs:87` says "not with a dropped glyph".

The re-triage re-ran the review's type-specimen corpus at 1717808 on
llvmpipe and got its numbers to the pixel. Both verifiers lowered the
finding to S2, for two reasons. `gpu` is a non-default feature. And the
corpus is every ASCII glyph in sixteen styles on one screen, where the
overflow comes from volume plus the shelf packer's waste, not from a
set larger than 1024².

### 1.3 The CPU fallback does not happen where Mesa is installed (R9-F6)

`Gpu::build` asks for any adapter with `force_fallback_adapter: false`
(`lib.rs:285-292`) and never checks the device type. On a machine with
no working driver and Mesa installed, and this machine is one, the only
adapter is llvmpipe. `Prefer::Auto` then takes it. The review measured
it at several times the software canvas's frame time, with the
resident memory and threads of a full GPU context. Three docs say that
exactly this machine gets the software canvas: `Prefer::Auto`'s rustdoc
(`crates/lumen-static-shell/src/program.rs:116-123`), `pick_backend`'s
rustdoc (`crates/lumen-static-winit/src/lib.rs:133-139`, and the crate
header at `:34`), and `docs/results-gpu.md` §6.

### 1.4 What the GPU path costs when nobody is looking (R9-F8, R9-F9)

`draw_regions` (`lib.rs:639`) builds fresh `quads`, `grads`, `mats` and
`splits` `Vec`s on every call (`:643`, `:654`, `:661-662`), and a clip
`Vec` (`:731`). Every run is also shaped through `FontStack::place_run`,
which allocates (M50 §1.1). `docs/results-allocation.md` never mentions
the GPU path, and `just test` excludes wgpu, so nothing gates it (R9-F8,
S3). Some of the per-frame allocations are wgpu's own command encoding.
Those are outside this crate's control, and the fix must separate them
from the rest.

`build` (`lib.rs:482`) and `resize` (`lib.rs:577`) call `make_scratch`
(`lib.rs:1396`) unconditionally. That creates two full-surface textures
for the backdrop blur, whether or not any `Cmd::Glass` is ever drawn.
The image atlas beside it is created on first use, and its rustdoc
(`lib.rs:75-76`) says so as a principle (R9-F9, S3). On llvmpipe, the
pages are lazily committed and RSS cannot show the cost. On a discrete
GPU it is committed VRAM, and that could not be measured here.

### 1.5 The dependency cap's `Damage::All` escape has never been shown to fire (R9-F3)

`MAX_DEPS` is 4 (`crates/lumen-static/src/retain.rs:487`). When a frame
has more backdrop-reading commands than that, `dilate` returns
`Damage::All` (`retain.rs:544-546`). The rustdoc directly above it
(`retain.rs:537-542`) says "there is no cap and no fallback", and says a
cap with a `Damage::All` escape "would be a branch that cannot fire".
The branch can fire, and nothing tests it. With it deleted (scratch
`ecaf67f`), five frosted panels produce wrong pixels, and every existing
test stays green.

**New at 1717808, from the re-triage:** M34's shadow oracle
(`--features shadow-oracle --test shadow`) is also green under
`ecaf67f`. It compares strategies, not a partial render with a full
one, so it cannot see this. `examples/glasswall` has 26 panels, so every
one of its frames takes this branch.

### 1.6 The SVG renderer's "whole vocabulary" test draws neither a border nor glass (R9-F10)

`svg.rs:22` says the SVG test "exercises the **whole** command
vocabulary". `the_whole_command_vocabulary_renders`
(`crates/lumen-static-shell/tests/svg.rs:32`) draws no `stroked_rect`
and no glass. With both SVG arms emitting nothing (scratch `c75d6c4`),
the shell's suite is green.

## 2. Decisions

### 2.1 The atlas grows before it gives up, and a give-up is reported

The options:

- **(a) Grow the one glyph texture** to 2048, then 4096, and so on up
  to `max_texture_dimension_2d` of the adapter's limits, rebuilding the
  frame once at each step.
- **(b) A multi-page atlas:** a bind group per page, with draws split
  by page. It has no ceiling, but it adds a draw split to every frame
  that uses a second page.
- **(c) A better packer** (skyline instead of shelf), which recovers
  the waste the verifiers measured but raises no ceiling.

**Recommendation: (a), plus a counter.** The review's failing corpus
fits after one growth step. The limit on llvmpipe is far above 4096, so
the growth path is testable on this machine. (b) is reopened only if a
real screen reaches the device limit, and (c) only if growth costs
measurable memory on a real screen. At the device limit, the give-up
still draws what fits. It now also increments a counter the renderer
exposes and logs once per session. The counter is on `Gpu`. The
agent's `app.perf` reports it once M48 has made `app.perf` truthful
(R13-F2). M48/T48.3 carries stage timings and damaged pixels only, so
the index (2026-09-25) assigns that wiring to T51.1, which depends on
M48/T48.3. Its gate is the same over-full glyph set: `app.perf` must
report a non-zero give-up count, printed beside the count the renderer
logged.

**What it breaks:** nothing public. A grown atlas uses more VRAM, and
only a frame that needed it pays. The atlas never shrinks within a
session. That is recorded, not fixed.

### 2.2 Parity becomes per-region, in one restructure

R9-F1, R9-F2 and R9-F7 each change `parity.rs`, so they are one task
(T51.1). The frame keeps its shapes. Each shape gets a named region, and
the comparison reports and bounds each region separately, as
`glass_parity.rs` already does for the rim band.

**Where the bounds come from:** the per-region baseline measured on
llvmpipe, times a stated headroom. That is the only adapter this
machine has. `docs/results-gpu.md` §2 records the baselines per adapter
class. The discrete-GPU row reads **owed** until someone runs it. If a
discrete baseline in some region turns out above the llvmpipe bound,
the region's bound becomes the larger baseline times the headroom. The
border's injection must still fire at that bound. If it does not, the
border gets its own tight case, and the region is split rather than
loosened.

**What it breaks:** the single printed "parity: N px" line. `just
gpu-parity` and the results doc switch to per-region lines.

### 2.3 A CPU adapter is not a GPU under `Auto`

The rule: under `Prefer::Auto`, an adapter whose device type is CPU is
treated as absent, and the window keeps the software canvas. Under
`Prefer::Gpu`, or with the backend forced to `gpu` from the
environment, it is accepted, because the person asked for it.
`Gpu::headless` keeps accepting any adapter. That is what makes parity
executable on a machine like this one, and it is what the tests use.

The rule is a pure function in `lumen-static-wgpu`: the adapter's device
type and whether a CPU adapter is acceptable, answering accept or
reject with a reason. `Gpu::for_window` takes the flag, and
`pick_backend` (`crates/lumen-static-winit/src/lib.rs:143`) passes
"accept a CPU adapter" only when `prefer` is `Prefer::Gpu`. The chosen
backend and the adapter's name are printed once, as they already are
for the fallback case.

**What it breaks:** a user on a Mesa-only machine who relied on `Auto`
picking llvmpipe now gets the software canvas. That is the documented
behaviour, and the review measured it faster.

### 2.4 The GPU allocation gate counts only what this crate owns

wgpu allocates on every frame to encode commands. That floor is not
this crate's to remove. The gate measures the floor with an empty-list
frame, then a caret-sized `draw_regions`, and asserts that the
difference is zero, printing total, floor and difference. On llvmpipe,
Mesa's own allocations go through C `malloc` and are invisible to the
Rust allocator on both sides of the subtraction. On a discrete driver,
wgpu's floor may differ, but the difference is still this crate's. The
gate therefore means the same on both, and says so.

## 3. Tasks

### T51.0 — the gates first, red at 1717808

Each finding becomes a check committed as expected to fail, with its
red output recorded in the commit body. T51.0 may add **read-only
probes** that change no behaviour: an atlas reset and give-up counter
on `Gpu`, and a flag for whether the blur scratch exists. Without them
two of these gates cannot be written. Each probe is `#[doc(hidden)]`
and costs one counter increment on a path that is already slow.

- **R9-F1:** a parity case with the review's corpus (8 sizes × 2
  weights × printable ASCII) at scale 2 in a 3840×2160 headless frame.
  It asserts zero give-ups on the steady frame and px over 8 within the
  text region's bound. **Red today** by the review's counts. *llvmpipe
  proves* the atlas logic, which is adapter-independent. *A real
  adapter adds* only its own rasterisation baseline.
- **R9-F7:** the border region's count with its own bound. **Green
  today.** The red state is scratch `c68c51c`/`d7dd9f4`, which is
  recorded. *llvmpipe proves* the bound catches the injection on this
  adapter. The discrete baseline is owed (§2.2).
- **R9-F2:** an `Image` region (1:1, 2×, tinted, with alpha, and one
  fractional placement) with an inked-pixel lower bound. **Green
  today.** The red state is scratch `7415a9d`'s GPU half, recorded.
- **R9-F6:** the pure acceptance function's unit test is written
  against today's behaviour, where CPU under Auto is accepted. It fails
  when its assertion is flipped to "rejected". **Red today** on this
  machine: `Prefer::Auto` picks llvmpipe.
- **R9-F8:** `crates/lumen-static-wgpu/tests/alloc.rs`, thread-local
  counting allocator, floor-subtracted (§2.4). **Red today:** the
  per-call `Vec`s. *llvmpipe proves* the framework-owned count.
- **R9-F9:** a frame with no glass leaves the scratch absent. **Red
  today:** it is always built.
- **R9-F3:** `crates/lumen-static-shell/tests/glass_deps.rs`, the
  review's `r9_deps.rs` under a real name. **Green today.** The red
  state is `ecaf67f`, recorded.
- **R9-F10:** `tests/svg.rs` asserts one emitted element per command.
  **Green today.** The red state is `c75d6c4`, recorded.

**Effort:** M. **Depends on:** M43's R17-F6 (the require-an-adapter
switch in `just gpu`), M43's CI runner, and M40 (its parity arm is in
the file T51.1 restructures).

### T51.1 — parity by region, with images and a glyph set larger than one atlas, and an atlas that grows

**Closes:** R9-F7, R9-F2, R9-F1.

**Change:**

- `parity.rs`'s comparison splits into named regions (rect, rounded
  rect, shadow, line, gradient, text, clip scope, border, image), each
  with its own over-8 count and bound (§2.2). M40's fractional-scale
  arm moves into the same structure.
- The frame is built from an exhaustive `match` over `Cmd`, so a new
  command does not compile until it is placed (§0 rule 3). It adds
  `Image` through an `ImageSource` fixture.
- The atlas-overflow corpus becomes its own case.
- In `lumen-static-wgpu`, the glyph atlas grows before giving up
  (§2.1), and the give-up counter from T51.0 stays as a reported
  quantity.

**Gate:** each region prints `region: N px over 8 (bound B, baseline
b on <adapter>)`. The atlas case prints resets, growths and give-ups
and asserts zero give-ups. The image region asserts interior pixels
identical (0 px over 8 strictly inside) and an inked count above half
the image area. `just gpu` runs all of it, and refuses to report a pass
with no adapter (M43). A software-only twin of the image check, exact
interior values from `blit_image`, runs in `just test`, so a machine
with no wgpu build still sees it.

**Injection:**

1. Binary ring coverage in the software border (`c68c51c`): the border
   region goes red on llvmpipe with its footprint beside the bound. The
   whole-frame bound could not see this.
2. Pin the atlas at 1024 with growth disabled: the atlas case reports
   a give-up per frame and fails.
3. The old inverse check, the atlas at 64: parity must now go red
   through the atlas case.
4. `7415a9d`'s GPU half: the image region's inked count reads 0
   against its lower bound. Its software half fails the twin.

Accept: the scale-1 corpus resets 0 times and matches software. The 1:1
image is identical across backends. Every region is under its bound at
base on llvmpipe.

*What llvmpipe proves, and what it does not:* all four injections are
proven on llvmpipe. The per-region baselines of a discrete adapter are
**owed**. `docs/results-gpu.md` §2 says so in its table, and the first
run on a working driver fills that row in.

**Docs in the same commit:** `docs/results-gpu.md` §2 (per-region
bounds and baselines, per adapter class; the "every command" sentence
now points at the `match`) and §4 (growth, and the give-up that
remains at the device limit); `GLYPH_ATLAS`'s rustdoc (the "far more
than a screen has" sentence); `Overflow`'s rustdoc; `atlas.rs:87`;
`parity.rs`'s comments at `:32` and `:56-61`, and the headroom comment
at `:153`.

**Effort:** L. **Depends on:** T51.0; M40 (its parity arm); M43's
R17-F6.

### T51.2 — a CPU adapter is not a GPU under `Auto`

**Closes:** R9-F6.

**Change:** §2.3. The pure acceptance function lives in
`lumen-static-wgpu`, `Gpu::for_window` takes the "accept a CPU adapter"
flag, and `pick_backend` passes it for `Prefer::Gpu` only.

**Gate:** the unit test on the acceptance function: CPU under Auto is
rejected, CPU under Gpu is accepted, discrete under Auto is accepted,
integrated under Auto is accepted. It prints each case. A
`gpu`-feature integration test in `lumen-static-winit` asserts that on
a machine whose only adapter is a CPU, `Auto` yields the software
backend's name. If a real GPU is present, the test prints why it did
not apply and reports itself as not run, never as passed.

*What llvmpipe proves:* the rejection. This machine is exactly the case
the finding describes. *A real adapter* proves the other half, that
`Auto` still takes a discrete GPU. The pure test covers that half by
construction, and the live half is owed until a driver works here.

**Injection:** drop the device-type check. `Auto` picks llvmpipe, and
the unit test and the integration test both fail by name. Accept:
`Prefer::Gpu` on the same machine still opens llvmpipe, and parity
(which uses `Gpu::headless`) still runs.

**Docs in the same commit:** `Prefer::Auto`'s rustdoc; `pick_backend`'s
rustdoc and the crate header in `lumen-static-winit`; the comment above
`request_adapter` in `lumen-static-wgpu`; `docs/results-gpu.md` §6.

**Effort:** S. **Depends on:** T51.0.

### T51.3 — the dependency cap's fallback is gated, and its rustdoc stops saying it does not exist

**Closes:** R9-F3.

**Change:** `glass_deps.rs` loses its expected-fail marker. It is green
today. `dilate`'s rustdoc separates two facts: the dilation loop has no
cap, which is provable, and the dependency *table* has one, with a
`Damage::All` escape that this test fires. `MAX_DEPS`'s rustdoc names
the test. Growing the table (glasswall's 26 panels all take the
escape) is **not** done here. It needs a measurement, and §4 records it.

**Gate:** n = `MAX_DEPS` and n = `MAX_DEPS` + 1 frosted panels, with a
bar moving under the last one. Each frame is compared partial against
full. It asserts that frames went partial at n = `MAX_DEPS` and went
`Damage::All` at `MAX_DEPS` + 1, which shows the setup happened, and
that 0 px differ. It prints the partial and All counts and the worst
px difference beside the bound 0. It runs in `dev-shape` through
`--workspace`.

**Injection:** `ecaf67f` (delete the `Damage::All` return). The
`MAX_DEPS` + 1 arm reports its differing pixels, red by name. Accept:
at base, the `MAX_DEPS` arm passes with partial frames and the
`MAX_DEPS` + 1 arm passes with All frames.

**Docs in the same commit:** `dilate`'s and `MAX_DEPS`'s rustdoc;
`docs/results-glass.md` where the dependency rule is described; the
handoff if it cites "no fallback" (to be checked when the task lands).

*What building found (2026-09-25).* Nothing in the task was wrong. One
more doc made the claim: `docs/plan-glass.md` item 4 of the dilation
rule ("No round cap, and no fallback"), which is true of the round loop
and was read as true of the table. It gains a dated correction rather
than a rewrite. The handoff does not cite "no fallback". A second
injection, `dilate` answering `All` whenever any dependency exists,
turns the `MAX_DEPS` arm's setup assertion red, so the arm cannot pass
by never going partial.

**Effort:** S. **Depends on:** T51.0. **May run in Phase 2 or 3,**
because it is a software-only test plus rustdoc and uses nothing from
M43's scripts. T51.0 as a whole waits for M43 and M40, so an early
T51.3 carries T51.0's R9-F3 row itself (`glass_deps.rs`, with
`ecaf67f`'s red run in its commit body). **M50/T50.9 depends on it,**
because T50.9 changes the rule in the same function.

### T51.4 — the frame path allocates nothing this crate owns

**Closes:** R9-F8.

**Change:** `quads`, `grads`, `mats`, `splits` and the clip `Vec`
become fields of `Gpu`, cleared per frame. Text shaping uses M50/T50.2's
scratch-taking variant of `place_run` instead of the allocating one.

**Gate:** `crates/lumen-static-wgpu/tests/alloc.rs` (§2.4): a
steady-state caret `draw_regions` minus the empty-list floor equals 0.
It prints total, floor and difference. *llvmpipe proves* the
framework's count. The floor on a discrete driver is recorded when one
is available, and the gate means the same there by construction.

**Injection:** recreate `quads` per call. The difference reads at least
1 per frame, red. Go back to the allocating `place_run`. The
difference reads one per run. Accept: the empty-list frame equals the
floor.

**Docs in the same commit:** `docs/results-allocation.md` (the GPU path,
what is gated and what is wgpu's); the handoff's §10 qualifier that M50
wrote gains the GPU sentence; `draw_regions`'s comment.

**Effort:** S. **Depends on:** T51.0; M50/T50.2.

### T51.5 — the blur scratch is made on the first glass command

**Closes:** R9-F9.

**Change:** the scratch becomes an `Option`, created the first time a
frame contains `Cmd::Glass`, and resized only if it exists. It is sized
to the surface. Sizing it to the capture rect is a later refinement,
recorded in §4.

**Gate:** a frame with no glass leaves the scratch absent. A resize
with no glass creates none. A frame with one panel creates it once.
Each check prints the scratch bytes beside its expected value.
`glass_parity.rs` stays green.

*What llvmpipe proves:* that the allocation is not made. What it saves
on a discrete GPU is VRAM this machine cannot measure. The results doc
states it as the arithmetic it is.

**Injection:** allocate eagerly again. The no-glass assertion fails.
Accept: `glass_parity.rs` is green with the lazy scratch.

**Docs in the same commit:** the scratch's rustdoc, mirroring the image
atlas's pay-for-use sentence at `lib.rs:75-76`; `docs/results-gpu.md`
§5.

**Effort:** S. **Depends on:** T51.0.

### T51.6 — the SVG test draws every command, by construction

**Closes:** R9-F10.

**Change:** `the_whole_command_vocabulary_renders` builds its list from
an exhaustive `match` over `Cmd`, as T51.1 does, and asserts one
emitted element per command: `stroke-width` on the border's rect, and
the blur attribute on glass.

**Gate:** `tests/svg.rs` prints the list of commands whose element is
missing, and asserts the list is empty.

**Injection:** `c75d6c4` (the SVG `StrokedRect` and `Glass` arms emit
nothing). The assertion names both. Accept: base passes.

**Docs in the same commit:** `svg.rs`'s module doc; `tests/svg.rs`'s
doc comment.

**Effort:** S. **Depends on:** T51.0. **May run in Phase 2 or 3,**
because it is a software-only test with no dependency on M43's scripts.
If it does, it carries T51.0's R9-F10 row itself, with `c75d6c4`'s red
run in its commit body.

## 4. Boundaries of this plan

- **R9-F5**, the fractional-scale disagreement, is M40's. T51.1 only
  moves M40's arm into the per-region structure.
- **R17-F6**, a missing adapter counting as a pass, is M43's. Every
  gate here depends on it.
- **R9-F4**, the golden-image gate running zero tests, is M43's.
- **R13-F2** (`app.perf` reports zero) is M48's. The give-up counter is
  exposed on `Gpu`. Reporting it through the agent is not planned by
  M48 or here (§2.1), and waits for a caller that needs it.
- **Not done, and recorded:** a multi-page atlas (§2.1); a skyline
  packer; shrinking a grown atlas; sizing the blur scratch to the
  capture rect; growing `MAX_DEPS`, or making the dependency table a
  `Vec` in `alloc` builds. The last would take glasswall off the
  `Damage::All` path, and it needs its own measurement, because the
  escape is also a cost bound.
- **The discrete-GPU baselines are owed, not skipped.** No task here can
  close them on this machine. `docs/results-gpu.md` §2 names them as
  owed, so the gap stays visible.
- **No won't-fix items.**

## 5. Kill rules and risks

This milestone makes no performance claim, so it has no kill rule on a
speed-up. It has one scope rule:

- **T51.1's atlas growth.** If growing to 2048 does not remove the
  review corpus's give-up at scale 2, the packing waste is larger than
  the verifiers measured. Then (c), the better packer, comes before
  (a), and §2.1 is revised in place with the number.

Risks, and what would reveal each:

- **Per-region bounds tuned to llvmpipe fail on a real adapter.**
  *Reveal:* the first `just gpu` on a working driver. §2.2 says what
  happens next: raise to the larger baseline, and split any region
  whose injection then stops firing.
- **The CPU-adapter rule rejects an adapter that is really a GPU.**
  Some virtualised GPUs report an odd device type. *Reveal:* the
  once-per-session backend line. T51.2's unit test covers every device
  type wgpu defines, with the expected answer written down.
- **The floor subtraction in T51.4 is unstable** in the first frames.
  The review saw 100, 95, 92, 92. *Reveal:* the gate warms up until two
  consecutive floors agree, and prints how many frames that took.
- **M40 and T51.1 both edit `parity.rs`.** *Reveal:* M40's arm must
  report the same numbers in the restructured file as before it moved.

## Appendix: finding → task

| ID | sev now | status | task |
|---|---|---|---|
| R9-F1 | S2 | OPEN | T51.1 |
| R9-F2 | S2 | OPEN | T51.1 |
| R9-F7 | S2 | OPEN | T51.1 |
| R9-F6 | S2 | OPEN | T51.2 |
| R9-F3 | S2 | OPEN | T51.3 |
| R9-F8 | S3 | OPEN | T51.4 |
| R9-F9 | S3 | OPEN | T51.5 |
| R9-F10 | S3 | OPEN | T51.6 |
