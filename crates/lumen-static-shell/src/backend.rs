//! What a backend must do to put a frame on screen —
//! `docs/plan-capabilities.md` T15.12, the second seam; moved here from
//! `lumen-static-winit` by M83/T83.1 (`docs/plan-gpu-extension.md`, D1 b).
//!
//! # Why this exists
//!
//! [`Renderer`](lumen_static::Renderer) was the seam M10a built, and it
//! is the right one: a renderer consumes a display list and never
//! appears in a widget signature. But it is only half of what a GPU
//! backend needs. The other half is the **surface** — acquiring
//! something to draw into, asking it how stale it is, and presenting it
//! — and until T15.12 that half was written directly against softbuffer
//! in the window crate's `Surface::present`.
//!
//! T15.12's survey found this and said so plainly: *"the work is two
//! seams, not one, and the second does not exist yet."* This is the
//! second one.
//!
//! # Why it takes a `FrameSource` and not a `Host<A>`
//!
//! Until M37 this trait was `Backend<A>`, because `present` took the
//! host itself — it needs two things from it at once, the display list
//! and `take_damage_for_age`. A window that reloads keeps its host in
//! the library, where the loader cannot name `A`
//! (`docs/plan-window-reload.md` §1), so the two things are now a trait,
//! [`FrameSource`](crate::session::FrameSource), which a `Host` and a session both are.
//!
//! # Why it lives in the shell
//!
//! Nothing in it names a window system: it needs [`FrameSource`](crate::session::FrameSource),
//! [`Color`](lumen_static::Color), `Instant`, [`ImageSource`](lumen_static::ImageSource),
//! [`FontStack`](crate::text::FontStack) and [`Frame`](crate::backend::Frame), all shell or core types. In the window
//! crate it could only be *implemented* by code that also linked winit,
//! and a program's description (`Program`, here) could not name it, so a
//! custom renderer had no door (M83, "custom renderer injection"). Here,
//! a backend is built from a [`BackendTarget`](crate::backend::BackendTarget) — raw window handles plus
//! a size — and the shell still depends on no window or GPU crate:
//! `scripts/check_shell_headless.sh` counts the edges in `just static`.
//! `lumen-static-winit` re-exports every item, so
//! `lumen_static_winit::Backend` still resolves; its `Software`,
//! `GpuBackend`, `AsScreen` and `pick_backend` stay there, because they
//! hold softbuffer, wgpu or the window.
//!
//! **Not merged with [`Screen`](crate::session::Screen).** `Screen` is
//! the narrow view that crosses the `Session` ABI, and `Offscreen`
//! implements it with no images, no resize and no name; merging would
//! widen what the ABI stamp covers for no user.
//!
//! Behind `truetype`, as `session` and `text` are, because
//! [`FrameSource`](crate::session::FrameSource) and [`FontStack`](crate::text::FontStack) are; `raw-window-handle` is optional
//! under the same feature, so the shell's default build gains no crate
//! (`docs/results-shader.md` T83.1).

use std::sync::Arc;
use std::time::Instant;

use lumen_static::Color;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::session::FrameSource;
use crate::text::FontStack;

/// What one presented frame cost, in nanoseconds.
///
/// The same fields `lumen_static_agent::FrameTiming` wants, without
/// depending on the agent crate — a window should not have to link the
/// agent to time itself, and the window crate maps one to the other
/// (`frame_times` there).
#[derive(Clone, Copy, Debug, Default)]
pub struct Frame {
    pub layout_ns: u64,
    pub paint_ns: u64,
    pub render_ns: u64,
    pub present_ns: u64,
    pub frame_ns: u64,
    /// Pixels the damage regions covered, or 0 for a frame that rendered
    /// nothing and was presented anyway (the window crate's
    /// `Surface::present`). **Physical** pixels, like the regions
    /// (M40/T40.1): before that the regions were logical and this counted
    /// them as physical, so it was short by the scale squared on a HiDPI
    /// display.
    pub damaged_px: u64,
    /// The frame was rendered and **did not reach the screen** — the
    /// surface was outdated, lost or timed out (M47/T47.4, R7-F7). The
    /// window asks for another frame on it, a bounded number of times in
    /// a row (the window crate's `present_retry`). Always `false` from
    /// softbuffer, which panics rather than dropping.
    pub dropped: bool,
    /// How many frames the GPU renderer has drawn with glyphs or images
    /// missing because an atlas could not hold them, since it was made
    /// (`lumen_static_wgpu::AtlasStats::give_ups`, M51/T51.1). A running
    /// count, not this frame's; `None` from the software canvas, which has
    /// no atlas.
    pub atlas_give_ups: Option<u64>,
}

/// A window, as a backend needs it: its raw handles, shareable across
/// threads — M83/T83.1.
///
/// Blanket-implemented, so a winit `Window` (or any other window type with
/// both handles) is one. `softbuffer::Context::new`/`Surface::new` and
/// `lumen_static_wgpu::Gpu::for_window` all accept an
/// `Arc<dyn WindowHandles>`, because `raw-window-handle` 0.6 implements
/// both handle traits for `Arc<H: ?Sized>`. `Send + Sync` because wgpu's
/// safe surface constructor wants it, and `crates/*/src` may contain no
/// `unsafe` to reach the other one.
pub trait WindowHandles: HasWindowHandle + HasDisplayHandle + Send + Sync {}

impl<T: HasWindowHandle + HasDisplayHandle + Send + Sync + ?Sized> WindowHandles for T {}

/// What a backend is built from — one path for the built-in backends and
/// a program's own (M83/T83.1, T83.2).
///
/// The size is carried because raw handles have none: it is the window's
/// physical inner size when the target was made, and a backend's
/// [`Backend::resize`] corrects it before every present anyway.
#[derive(Clone)]
pub struct BackendTarget {
    pub window: Arc<dyn WindowHandles>,
    /// Physical pixels.
    pub width: u32,
    pub height: u32,
    /// The face the window's session measures with, **in its mode**
    /// (`Session::shaping`, M81/T81.4). A backend that rasterises text
    /// with any other draws glyphs into boxes sized for a different face.
    /// `None` when the system has none, as the block stand-in measures.
    pub font: Option<FontStack>,
    /// Whether a CPU rasteriser such as llvmpipe counts as an adapter
    /// (M51/T51.2): `true` under `Prefer::Gpu` only.
    pub accept_cpu: bool,
}

/// One way of getting a frame onto a window.
///
/// Two implementations ship in `lumen-static-winit`: `Software` there,
/// and `GpuBackend` behind its `gpu` feature. The software one is the
/// default and the fallback, and it is not a lesser path — it is the
/// only path on a machine with no GPU, and the only path in the
/// embedded profile.
///
/// **Every method that acts on what the backend holds is required**
/// (M83/T83.1). Once code outside this workspace implements the trait, a
/// forwarding wrapper is the likely implementor, and a wrapper that
/// inherited a provided body would act on itself instead of what it wraps
/// — `set_images` would drop every image in silence (`AGENT.md`: a
/// provided method is right for a leaf and wrong for a wrapper).
pub trait Backend {
    /// Match the backend's buffers to the window. Cheap and idempotent
    /// when the size has not changed; **whether it changed**, because
    /// that is when the host must be told too.
    fn resize(&mut self, width: u32, height: u32) -> bool;

    /// Render the frame `src` just built, and present it.
    fn present(&mut self, src: &mut dyn FrameSource, page: Color, frame_start: Instant) -> Frame;

    /// The presented window read back from the display server, when
    /// that is possible here.
    ///
    /// `None` means "not checkable", not "nothing was drawn" — the
    /// distinction `ui.screenshot { source: "surface" }` depends on.
    /// **Required since M83/T83.1**; it was provided as `None`.
    fn fetch_rgba(&mut self) -> Option<(u32, u32, Vec<u8>)>;

    /// Hand the renderer the application's image pixels.
    ///
    /// A backend that cannot resolve `Cmd::Image` is a real configuration
    /// — the SVG writer is one — and the contract `ImageSource` already
    /// states for a missing asset is "draw nothing, do not fail"; such a
    /// backend writes the no-op itself. **Required since M83/T83.1**: it
    /// was provided as a no-op, which is exactly what a forwarding wrapper
    /// that forgot to forward would do.
    fn set_images(&mut self, images: Box<dyn lumen_static::ImageSource>);

    /// Hand the image source back — M47/T47.7 (R15-F6).
    ///
    /// A window that is suspended drops its backend and makes a new one on
    /// the next `resumed`. The source is a trait object the backend owns,
    /// not a thing that can be cloned, so it must be **taken back** before
    /// the drop, or every image vanishes on the second resume.
    ///
    /// **Required**: a backend that was handed a source and answers
    /// "nothing" here reproduces the bug exactly. `AGENT.md`'s rule — a
    /// provided method is right for a leaf and wrong for a wrapper — in
    /// its other form: a default that cannot know what the implementor
    /// holds is the defect.
    fn take_images(&mut self) -> Option<Box<dyn lumen_static::ImageSource>>;

    /// **Rasterise text with `font` from the next frame on** — M81/T81.4
    /// (`docs/plan-shaped-paint.md`, "Choosing is a `Program` setting").
    /// The window calls it after a swap whose session answers a different
    /// `Session::shaping` than the stack it rendered with, then repaints
    /// all: the new build measures with its program's mode, and a
    /// backend left on the old stack would draw `Complex` boxes with
    /// `Basic` glyphs.
    ///
    /// **Required, not provided**: a default that ignored the stack is
    /// exactly that defect, and it would compile.
    fn set_font(&mut self, font: FontStack);

    /// The last frame this backend rendered, as RGBA8.
    ///
    /// **Not the same as `fetch_rgba`**, and the difference is the whole
    /// reason both exist: this is what the renderer *drew*, that is what
    /// the display server is *showing*. A backend that cannot hand back
    /// its own pixels answers `None`, which `ui.screenshot` reports as
    /// an error rather than as a blank image. **Required since
    /// M83/T83.1**; it was provided as `None`, so a wrapper inheriting it
    /// made every screenshot of a working window an error.
    fn frame_rgba(&self) -> Option<(u32, u32, &[u8])>;

    /// What to print in a startup line. A user reporting "it is slow"
    /// should not have to guess which renderer they got.
    fn name(&self) -> &str;
}

/// A program's own backend constructor — M83/T83.2
/// (`Program::backend_with`, `docs/plan-gpu-extension.md`, "The factory").
///
/// Asked on every `resumed`, before the built-in GPU-then-software
/// choice, with the window's [`BackendTarget`]; `None` falls through to
/// that choice, so the software fallback stands. A closure, not a `fn`,
/// so a backend that wants to share a device can capture one. **Window
/// side and never reloadable**: the window takes it once, from the static
/// binary's program, into its `Config`; a hot library's is ignored.
pub type BackendFactory = Box<dyn FnMut(&BackendTarget) -> Option<Box<dyn Backend>>>;
