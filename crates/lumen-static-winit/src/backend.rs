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
//! # Why it is generic rather than dynamic in `A`
//!
//! `Box<dyn Backend<A>>` rather than `Box<dyn Backend>`, because
//! `present` takes a `&mut Host<A>` and needs it for two things at once
//! — the display list and `take_damage_for_age` — which a closure
//! cannot be handed without borrowing the host twice. Parameterising the
//! *trait* keeps it object-safe and keeps the signature the one
//! `Surface::present` already had.

use std::time::Instant;

use lumen_static::event::App;
use lumen_static::Color;
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::FontStack;
use lumen_static_shell::Host;

use crate::surface::Frame;

/// One way of getting a frame onto a window.
///
/// Two implementations ship: [`Software`] here, and `GpuBackend` in
/// `crate::gpu` behind the `gpu` feature. The software one is the
/// default and the fallback, and it is not a lesser path — it is the
/// only path on a machine with no GPU, and the only path in the
/// embedded profile.
pub trait Backend<A: App> {
    /// Match the backend's buffers to the window, and tell the host.
    /// Cheap and idempotent when the size has not changed.
    fn resize(&mut self, host: &mut Host<A>, width: u32, height: u32);

    /// Render what the host just pumped, and present it.
    fn present(&mut self, host: &mut Host<A>, page: Color, frame_start: Instant) -> Frame;

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

impl<A: App> Backend<A> for Software {
    fn resize(&mut self, host: &mut Host<A>, width: u32, height: u32) {
        crate::surface::sync_size(&mut self.canvas, host, self.font.as_ref(), width, height);
    }

    fn set_images(&mut self, images: Box<dyn lumen_static::ImageSource>) {
        self.canvas.images = Some(images);
    }

    fn present(&mut self, host: &mut Host<A>, page: Color, frame_start: Instant) -> Frame {
        self.surface.present(host, &mut self.canvas, page, frame_start)
    }

    fn fetch_rgba(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        self.surface.fetch_rgba(self.canvas.width, self.canvas.height)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Which backend a window should try for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Prefer {
    /// GPU when this crate was built with the `gpu` feature *and* an
    /// adapter exists; the software canvas otherwise. The default,
    /// because falling back is the whole point: a machine with no
    /// driver, a container with no `/dev/dri` and a remote session all
    /// have to keep working.
    #[default]
    Auto,
    /// The software canvas, whatever hardware is present.
    Software,
    /// The GPU, and **still fall back** if there is none. A refusal to
    /// open a window would be a worse answer than a slower one; a
    /// caller that needs to know which it got asks
    /// [`Backend::name`].
    Gpu,
}

impl Prefer {
    /// `LUMEN_BACKEND=software` / `=gpu`, overriding the application.
    ///
    /// An escape hatch for the person running the program rather than
    /// the person who wrote it: a driver bug on one machine should not
    /// need a rebuild to work around. An unrecognised value is ignored
    /// rather than fatal.
    pub fn from_env(default: Prefer) -> Prefer {
        match std::env::var("LUMEN_BACKEND").as_deref() {
            Ok("software") | Ok("cpu") => Prefer::Software,
            Ok("gpu") | Ok("wgpu") => Prefer::Gpu,
            _ => default,
        }
    }
}
