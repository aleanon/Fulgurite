//! What a backend must do to put a frame on screen —
//! `docs/plan-capabilities.md` T15.12, the second seam.
//!
//! # Why this exists
//!
//! [`Renderer`](lumen_static::Renderer) was the seam M10a built, and it
//! is the right one: a renderer consumes a display list and never
//! appears in a widget signature. But it is only half of what a GPU
//! backend needs. The other half is the **surface** — acquiring
//! something to draw into, asking it how stale it is, and presenting it
//! — and until now that half was written directly against softbuffer in
//! [`Surface::present`](crate::Surface::present).
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
//! [`FrameSource`], which a `Host` and a session both are.

use std::time::Instant;

use lumen_static::Color;
use lumen_static_shell::session::{FrameSource, Screen};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::FontStack;

use crate::surface::Frame;

/// One way of getting a frame onto a window.
///
/// Two implementations ship: [`Software`] here, and `GpuBackend` in
/// `crate::gpu` behind the `gpu` feature. The software one is the
/// default and the fallback, and it is not a lesser path — it is the
/// only path on a machine with no GPU, and the only path in the
/// embedded profile.
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
    fn fetch_rgba(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        None
    }

    /// Hand the renderer the application's image pixels.
    ///
    /// **Defaulted to a no-op, and that is not a shrug**: a backend that
    /// cannot resolve `Cmd::Image` is a real configuration — the SVG
    /// writer is one — and the contract `ImageSource` already states for
    /// a missing asset is "draw nothing, do not fail". A backend that
    /// can should override it.
    fn set_images(&mut self, images: Box<dyn lumen_static::ImageSource>) {
        let _ = images;
    }

    /// Hand the image source back — M47/T47.7 (R15-F6).
    ///
    /// A window that is suspended drops its backend and makes a new one on
    /// the next `resumed`. The source is a trait object the backend owns,
    /// not a thing that can be cloned, so it must be **taken back** before
    /// the drop, or every image vanishes on the second resume.
    ///
    /// **Required, not provided**, where `set_images` is provided: a
    /// backend that cannot draw images is a real configuration, but one
    /// that was handed a source and answers "nothing" here reproduces the
    /// bug exactly. `AGENT.md`'s rule — a provided method is right for a
    /// leaf and wrong for a wrapper — in its other form: a default that
    /// cannot know what the implementor holds is the defect.
    fn take_images(&mut self) -> Option<Box<dyn lumen_static::ImageSource>>;

    /// The last frame this backend rendered, as RGBA8.
    ///
    /// **Not the same as `fetch_rgba`**, and the difference is the whole
    /// reason both exist: this is what the renderer *drew*, that is what
    /// the display server is *showing*. A backend that cannot hand back
    /// its own pixels answers `None`, which `ui.screenshot` reports as
    /// an error rather than as a blank image.
    fn frame_rgba(&self) -> Option<(u32, u32, &[u8])> {
        None
    }

    /// What to print in a startup line. A user reporting "it is slow"
    /// should not have to guess which renderer they got.
    fn name(&self) -> &str;
}

/// The software canvas, blitted through softbuffer — what every window
/// in this workspace did before T15.12, unchanged.
pub struct Software {
    surface: crate::Surface,
    canvas: Canvas,
    font: Option<FontStack>,
    name: String,
}

impl Software {
    pub fn new(window: std::sync::Arc<winit::window::Window>, font: Option<FontStack>) -> Self {
        let size = window.inner_size();
        let canvas = crate::surface::canvas(size.width.max(1), size.height.max(1), font.as_ref());
        Software {
            surface: crate::Surface::new(window),
            canvas,
            font,
            name: "software (softbuffer)".into(),
        }
    }

    /// The canvas, for an application that wants the pixels without the
    /// display server — the tally's agent endpoint answers
    /// `ui.screenshot` from here.
    pub fn canvas(&self) -> &Canvas {
        &self.canvas
    }
}

impl Backend for Software {
    fn resize(&mut self, width: u32, height: u32) -> bool {
        crate::surface::sync_size(&mut self.canvas, self.font.as_ref(), width, height)
    }

    fn set_images(&mut self, images: Box<dyn lumen_static::ImageSource>) {
        self.canvas.images = Some(images);
    }

    fn take_images(&mut self) -> Option<Box<dyn lumen_static::ImageSource>> {
        self.canvas.images.take()
    }

    fn frame_rgba(&self) -> Option<(u32, u32, &[u8])> {
        Some((self.canvas.width, self.canvas.height, &self.canvas.pixels))
    }

    fn present(&mut self, src: &mut dyn FrameSource, page: Color, frame_start: Instant) -> Frame {
        self.surface.present(src, &mut self.canvas, page, frame_start)
    }

    fn fetch_rgba(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        self.surface.fetch_rgba(self.canvas.width, self.canvas.height)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// A backend, as the [`Screen`] a session's agent presents to and reads
/// pixels back from (M37: the agent lives with the tree, in the session).
pub struct AsScreen<'a>(pub &'a mut dyn Backend);

impl Screen for AsScreen<'_> {
    fn present(&mut self, src: &mut dyn FrameSource, page: Color) {
        self.0.present(src, page, Instant::now());
    }
    fn frame_rgba(&self) -> Option<(u32, u32, &[u8])> {
        self.0.frame_rgba()
    }
    fn fetch_rgba(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        self.0.fetch_rgba()
    }
}

/// Which backend a window should try for. Part of the program's
/// description since M32, so it lives in the shell; re-exported here.
pub use lumen_static_shell::program::Prefer;
