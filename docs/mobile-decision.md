# Mobile: deferred, and the goal corrected to say so

**Decided 2026-09-08 — `docs/plan-invalidation.md` T16.14.**

The project's stated goal is to be the fastest and most efficient GUI
framework **on desktop, mobile and embedded**. Two of those are
measured. This records what the third actually is today, and changes the
claim rather than leaving it standing.

## What exists

Nothing.

Searched across `crates/`, `examples/`, `fixtures/`, `benches/`,
`justfile`, `scripts/` and `.github/workflows/ci.yml`:

- no Android or iOS target named anywhere;
- no `cdylib` crate type except the hot-reload fixture, which is a
  desktop dylib for `dlopen`;
- no `winit::platform::*` use — the windowing crate uses only the
  portable surface;
- no NDK, Gradle, Xcode or `cargo-apk` configuration;
- CI is `ubuntu-latest` plus a non-blocking `macos-latest`, which is
  desktop Apple Silicon, not iOS.

The toolchain on the development machine has `aarch64-linux-android`,
`armv7-linux-androideabi` and `x86_64-linux-android` installed, and no
`.cargo/config.toml` names an NDK linker — so nothing here can **link**
an Android binary. `docs/competitive-feature-gaps.md` says it plainly:
desktop-only, plus a `no_std` core with no driver.

**But `cargo check` does not link, and it completes** (M54/T54.8,
R12-F6 — this paragraph said until then that it "cannot complete"). `just
mobile-check` type-checks for `aarch64-linux-android` (declared in
`rust-toolchain.toml`) and prints one line per configuration:

| configuration | checks |
|---|---|
| `lumen-static`, `lumen-static-widgets`, `lumen-static-shell` | yes |
| `lumen-static-winit`, `--no-default-features` + `winit/android-native-activity` | yes |
| … with `reload`, `hot` and `agent` | yes |
| … with its **default** features (`clipboard`) | yes, since T54.8: `arboard` has no Android backend, so it is a target-gated dependency and `clipboard` is a no-op there |
| … with `a11y` | yes (`accesskit_winit` has an Android backend) |
| … with `dialogs` | **no** — `rfd` has no Android file dialog; the feature is desktop-only and default-off |

`winit/android-native-activity` (or `-game-activity`) must come from the
application: winit refuses to build for Android without one, and which
activity is the application's choice. Checking is not running: no
lifecycle beyond the desktop's below has been exercised on a device.

### The lifecycle, on desktop (M47/T47.7, 2026-09-26)

One piece of a mobile lifecycle now holds on desktop: a window that is
suspended and resumed keeps its images. `resumed` makes a new window
and backend each time, and until T47.7 there was no `suspended` handler
and the image source was taken once, so a second `resumed` drew every
image as nothing (R15-F6). `Win::suspended` now takes the source back
through `Backend::take_images` (required on every backend) and drops the
backend and window. `just resume-drive` runs the round trip on a live
window through a debug-only `LUMEN_TEST_RESUME` hook and reads 0
differing pixels of 458,620 on the software and GPU backends
(`docs/plan-timers-wake.md` T47.7). Surface recreation on an Android
activity is still the deferred work below.

## Why this is not a small gap

The core is genuinely portable — it is `no_std` at two tiers and links a
22 458-byte `.text` for `thumbv7em-none-eabihf`
(2026-09-10)
(`docs/results-embedded.md`), which is a far harder target than a phone.
The gap is not the framework; it is everything around it.

The smallest honest measurement needs, in order:

1. an NDK or Xcode toolchain with a linker configured, so the core
   cross-compiles at all;
2. a `cdylib` entry point plus `android-activity` (or the iOS
   equivalent) glue — the `winit` support exists but nothing here uses
   it;
3. the same application packaged as an APK or `.app`, because a
   framework that cannot be shipped has not been ported;
4. a device, because an emulator's GPU and memory reporting are not the
   thing being claimed;
5. **a separate column, not a shared one.** `dumpsys meminfo` does not
   report the same quantity as `/proc/<pid>/status`, so a mobile RSS
   number cannot join the table in `docs/results-competitive.md` §3 —
   it would look comparable and would not be.

## The decision

**Deferred**, on a stated condition, in the shape `docs/plan-capabilities.md`
T15.12 used: not "someday", but *when this specific thing is true*.

**The condition: when someone intends to ship an application on a
phone.** Not when it would be interesting to know — the work is a
toolchain, a packaging story and a device lab, and none of it improves
the framework for the platforms it does run on.

**And the goal is corrected in the meantime.** A goal that names a
platform nobody can measure is a claim, not a target, and this project's
whole discipline is that numbers are measured and stated once. Until the
condition above is met, the claim is:

> the fastest and most efficient GUI framework on **desktop and
> embedded**, with a core that is portable to mobile and unproven there.

That is weaker and it is true, which is the trade this repository makes
everywhere else.

**What is not deferred: touch as a pointer.** The deferral is of
packaging, toolchains and devices, not of input. Since M44/T44.4
(`docs/plan-input-semantics.md` §2.4) a `Host` turns the primary touch
into pointer events — a tap presses a button, toggles a checkbox and
moves a slider on any touchscreen a desktop shell reaches, including
Wayland — and a touch `Cancel` is a `PointerCancel` that activates
nothing. Before it no stock widget handled `Event::Touch`, so a tap did
nothing at all (R15-F4). `crates/lumen-static-shell/tests/touch_as_pointer.rs`
is the gate.

## What would probably happen if it were done

Recorded as a prediction, so a future measurement can contradict it
rather than being shaped by it:

- **The core should port cleanly.** It is `no_std`, allocation-free in
  the steady state, and already runs on a Cortex-M4F.
- **The software rasteriser is the risk.** `docs/results-raster-scaling.md`
  measures ~2.7 ms per megapixel on an i9-13900KF. A phone at 1080p+ and
  a fraction of that single-core throughput is the case
  `docs/results-gpu.md`'s crossover was measured for, and the answer is
  likely "GPU by default on mobile" — the opposite of the desktop
  default.
- **The 174 MB GPU context** (`docs/results-gpu.md` §5) is a desktop
  number and would need re-measuring; it is dominated by driver
  allocations that differ entirely on a tile-based mobile GPU.

None of those are claims. They are what to check first.
