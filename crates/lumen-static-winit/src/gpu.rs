//! The GPU backend, wired to a window — `docs/plan-capabilities.md`
//! T15.12.
//!
//! Thin on purpose. Everything about *drawing* is in
//! `lumen-static-wgpu`, which knows nothing about winit; everything
//! about *windows* is here, and knows nothing about shaders. The
//! dependency points one way — this crate names that one, never the
//! reverse — which is what lets the renderer be tested headlessly
//! against the software canvas with no display anywhere in it.

use std::time::Instant;

use lumen_static::event::App;
use lumen_static::Color;
use lumen_static_shell::text::FontStack;
use lumen_static_shell::Host;
use lumen_static_wgpu::Gpu;

use crate::backend::Backend;
use crate::surface::Frame;

/// A window drawn by `lumen-static-wgpu`.
pub struct GpuBackend {
    gpu: Gpu,
    name: String,
}

impl GpuBackend {
    /// `None` when there is no usable adapter — which is not an error
    /// and must not be treated as one. See
    /// [`Prefer`](crate::backend::Prefer).
    pub fn new(
        window: std::sync::Arc<winit::window::Window>,
        font: Option<FontStack>,
    ) -> Option<Self> {
        let size = window.inner_size();
        let mut gpu = Gpu::for_window(window, size.width.max(1), size.height.max(1))?;
        if let Some(f) = font {
            gpu.set_font(f);
        }
        let name = format!("gpu ({})", gpu.adapter());
        Some(GpuBackend { gpu, name })
    }

    pub fn gpu_mut(&mut self) -> &mut Gpu {
        &mut self.gpu
    }
}

impl<A: App> Backend<A> for GpuBackend {
    fn set_images(&mut self, images: Box<dyn lumen_static::ImageSource>) {
        self.gpu.set_images(images);
    }

    fn resize(&mut self, host: &mut Host<A, crate::Ret, crate::Inval>, width: u32, height: u32) {
        if self.gpu.size() == (width, height) {
            return;
        }
        self.gpu.resize(width, height);
        host.resize(width, height);
    }

    fn present(&mut self, host: &mut Host<A, crate::Ret, crate::Inval>, page: Color, frame_start: Instant) -> Frame {
        let (layout_ns, paint_ns) = host.timings();
        let (width, height) = self.gpu.size();
        let viewport = [0.0, 0.0, width as f32, height as f32];

        // **Age 1, always.** This is the one real difference from the
        // softbuffer path, and it is a simplification rather than an
        // assumption: the renderer draws into a texture it owns and
        // blits *that* to the swapchain, so the surface it accumulates
        // into is never handed to a presentation engine and never comes
        // back holding a frame from two presents ago. `src/blit.wgsl`
        // in that crate spends a paragraph on why the extra blit is
        // worth it, and this is the paragraph's payoff.
        // **The region set, not its bounding box** (M26/T26.12). This
        // was `Damage::clip_to`, whose own doc says it is "the bounding
        // box of every region" and is "kept for callers that can only
        // take one rect". The GPU can take several: two small changes
        // at opposite corners of a window bound to the whole window,
        // which is the merging cost M18 removed on the software path
        // and which this quietly re-introduced.
        let regions = crate::regions_for(host.take_damage_for_age(1), viewport);

        let t = Instant::now();
        let render_ns = if regions.is_empty() {
            0
        } else {
            self.gpu.draw_regions(&host.list, &regions, Some(page));
            t.elapsed().as_nanos() as u64
        };

        // Presented whether or not anything was rendered, for the reason
        // `Surface::present` records at length: `RedrawRequested` is
        // also an expose, and an expose means the display server lost
        // the content, which the application's own damage knows nothing
        // about.
        let t = Instant::now();
        self.gpu.present();
        let present_ns = t.elapsed().as_nanos() as u64;

        Frame {
            layout_ns,
            paint_ns,
            render_ns,
            present_ns,
            frame_ns: frame_start.elapsed().as_nanos() as u64,
            // **The regions' own area, not their bounding box's**
            // (M26/T26.12). This read the single rect the backend used
            // to be handed, so the number a profile printed was the
            // merged one — the quantity the change is about, reported
            // as the quantity it replaced.
            damaged_px: regions
                .iter()
                .map(|a| ((a[2] - a[0]) as f64 * (a[3] - a[1]) as f64).max(0.0) as u64)
                .sum(),
        }
    }

    // `fetch_rgba` is deliberately left at its default `None`.
    //
    // The renderer *can* read its target back, and that would be easy
    // to return here — but the target is what was rendered, not what
    // was presented, and `Backend::fetch_rgba` exists precisely to see
    // the present. Answering with the pre-present texture would make
    // `ui.screenshot { source: "surface" }` report "the frame reached
    // the screen" on a frame that did not. `None` is the honest answer:
    // not checkable.

    fn name(&self) -> &str {
        &self.name
    }
}

