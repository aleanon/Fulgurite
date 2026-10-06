//! The window crate's backends — `docs/plan-capabilities.md` T15.12.
//!
//! The trait they implement, [`Backend`], and what a backend is built
//! from, [`BackendTarget`], moved to `lumen_static_shell::backend` in
//! M83/T83.1 (`docs/plan-gpu-extension.md`, D1 b): nothing in either
//! names a window system, and a program's description can name a trait
//! that lives in the shell. They are re-exported here, so
//! `lumen_static_winit::Backend` still resolves. What stays is what holds
//! softbuffer, wgpu or the window: [`Software`], `GpuBackend` (`crate::gpu`),
//! [`AsScreen`] and the choice between them in `pick_backend`.

use std::time::Instant;

use lumen_static::Color;
use lumen_static_shell::session::{FrameSource, FrameTimes, Screen};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::FontStack;

pub use lumen_static_shell::backend::{Backend, BackendFactory, BackendTarget, Frame, WindowHandles};
/// M83/T83.4: what `Backend::set_shaders` is handed, re-exported beside the
/// trait; `ShaderDesc` under this crate's `shader`.
pub use lumen_static_shell::shader::ShaderTable;
#[cfg(feature = "shader")]
pub use lumen_static_shell::shader::ShaderDesc;

/// The software canvas, blitted through softbuffer — what every window
/// in this workspace did before T15.12, unchanged.
pub struct Software {
    surface: crate::Surface,
    canvas: Canvas,
    font: Option<FontStack>,
    name: String,
}

impl Software {
    /// Built from the window's raw handles and size (M83/T83.1): the
    /// canvas is `target.width × target.height` and rasterises with
    /// `target.font`, which must be the face the session measures with.
    /// `accept_cpu` is the GPU's question and is not read here.
    pub fn new(target: &BackendTarget) -> Self {
        let font = target.font.clone();
        let canvas = crate::surface::canvas(target.width.max(1), target.height.max(1), font.as_ref());
        Software {
            surface: crate::Surface::new(target.window.clone()),
            canvas,
            font,
            name: "software (softbuffer)".into(),
        }
    }

    /// The canvas, for an application that wants the pixels without the
    /// display server — the agent's `ui.screenshot` reads it through
    /// [`AsScreen`].
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

    /// **A no-op**: the canvas caches no image pixels — every command is
    /// resolved through the frame's store and the window's afresh
    /// (M84/T84.6).
    fn invalidate_images(&mut self) {}

    fn set_font(&mut self, font: FontStack) {
        self.canvas.set_font(font.clone());
        self.font = Some(font);
    }

    /// **A no-op that refuses nothing** — M83/T83.4. The canvas runs no
    /// shader: a `Cmd::Shader` bracket is a clip around its fallback, which
    /// the widget recorded in the list at paint time, so that fallback is
    /// this backend's whole answer and no shader is refused.
    fn set_shaders(&mut self, _table: lumen_static_shell::shader::ShaderTable<'_>) -> usize {
        0
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
    fn present(&mut self, src: &mut dyn FrameSource, page: Color) -> FrameTimes {
        crate::frame_times(&self.0.present(src, page, Instant::now()))
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
