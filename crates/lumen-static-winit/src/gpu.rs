//! The GPU backend, wired to a window — `docs/plan-capabilities.md`
//! T15.12.
//!
//! Thin on purpose. Everything about *drawing* is in
//! `lumen-static-wgpu`, which knows nothing about winit; everything
//! about *windows* is here, and knows nothing about how a shader is
//! drawn (M83/T83.5: it hands the program's table to `Gpu::set_shaders`
//! and no more). The
//! dependency points one way — this crate names that one, never the
//! reverse — which is what lets the renderer be tested headlessly
//! against the software canvas with no display anywhere in it.

use std::time::Instant;

use lumen_static::Color;
use lumen_static_shell::text::FontStack;
use lumen_static_shell::session::FrameSource;
use lumen_static_wgpu::Gpu;

use crate::backend::{Backend, BackendTarget, Frame};

/// A window drawn by `lumen-static-wgpu`.
pub struct GpuBackend {
    gpu: Gpu,
    name: String,
    /// Whether K82d's line has been logged: the first frame that holds a
    /// path mask says what share of the glyph atlas's starting size path
    /// masks take (M82/T82.6, `docs/plan-paint-vocabulary.md`), once per
    /// window; `just gpu-parity`'s PaintBox leg reads it.
    masks_logged: bool,
}

impl GpuBackend {
    /// `None` when there is no usable adapter — which is not an error
    /// and must not be treated as one. See
    /// [`Prefer`](crate::backend::Prefer).
    ///
    /// Built from the window's raw handles and size (M83/T83.1).
    /// `target.accept_cpu`: whether a CPU rasteriser such as llvmpipe
    /// counts as an adapter (M51/T51.2). [`crate::choose_backend`] passes
    /// it for `Prefer::Gpu` only.
    pub fn new(target: &BackendTarget) -> Option<Self> {
        let (w, h) = (target.width.max(1), target.height.max(1));
        let mut gpu = Gpu::for_window(target.window.clone(), w, h, target.accept_cpu)?;
        if let Some(f) = target.font.clone() {
            gpu.set_font(f);
        }
        let name = format!("gpu ({})", gpu.adapter());
        Some(GpuBackend {
            gpu,
            name,
            masks_logged: false,
        })
    }

    pub fn gpu_mut(&mut self) -> &mut Gpu {
        &mut self.gpu
    }
}

impl Backend for GpuBackend {
    fn set_images(&mut self, images: Box<dyn lumen_static::ImageSource>) {
        self.gpu.set_images(images);
    }

    fn take_images(&mut self) -> Option<Box<dyn lumen_static::ImageSource>> {
        self.gpu.take_images()
    }

    fn set_font(&mut self, font: FontStack) {
        // `Gpu::set_font` resets the glyph atlas: ids are per stack.
        self.gpu.set_font(font);
    }

    /// **Compiles the table** — M83/T83.5: `Gpu::set_shaders`, which
    /// copies each source into its module (no borrow survives the call),
    /// reuses a pipeline whose source and uniform size are unchanged, and
    /// answers how many entries it refused. A refused entry prints one
    /// stderr line naming it and its brackets draw their fallback; nothing
    /// panics, because every creation runs inside a validation error scope
    /// (`lumen-static-wgpu`'s `custom.rs`). Without this crate's `shader`
    /// there is no table to compile and every bracket draws its fallback.
    fn set_shaders(&mut self, table: lumen_static_shell::shader::ShaderTable<'_>) -> usize {
        #[cfg(feature = "shader")]
        {
            self.gpu.set_shaders(table)
        }
        #[cfg(not(feature = "shader"))]
        {
            let _ = table;
            0
        }
    }

    fn resize(&mut self, width: u32, height: u32) -> bool {
        if self.gpu.size() == (width, height) {
            return false;
        }
        self.gpu.resize(width, height);
        true
    }

    fn present(&mut self, host: &mut dyn FrameSource, page: Color, frame_start: Instant) -> Frame {
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
            self.gpu.draw_regions(host.list(), &regions, Some(page));
            t.elapsed().as_nanos() as u64
        };

        // Presented whether or not anything was rendered, for the reason
        // `Surface::present` records at length: `RedrawRequested` is
        // also an expose, and an expose means the display server lost
        // the content, which the application's own damage knows nothing
        // about.
        let t = Instant::now();
        let presented = self.gpu.present();
        let present_ns = t.elapsed().as_nanos() as u64;

        let stats = self.gpu.atlas_stats();
        if !self.masks_logged && stats.path_texels > 0 {
            self.masks_logged = true;
            let start = lumen_static_wgpu::GLYPH_ATLAS as u64 * lumen_static_wgpu::GLYPH_ATLAS as u64;
            eprintln!(
                "lumen-static: path masks {} texels in {} tile(s), {:.2}% of the glyph atlas's start ({}²) -- K82d, bound 25%",
                stats.path_texels,
                stats.path_masks,
                stats.path_texels as f64 / start as f64 * 100.0,
                lumen_static_wgpu::GLYPH_ATLAS
            );
        }

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
            // M47/T47.4: the window asks for another frame on this.
            dropped: !presented,
            // M51/T51.1: reported through `app.perf` as `atlas_give_ups`.
            atlas_give_ups: Some(stats.give_ups),
        }
    }

    /// **`None`, deliberately** — written out since M83/T83.1 made the
    /// method required; it was the inherited default.
    ///
    /// The renderer *can* read its target back, and that would be easy
    /// to return here — but the target is what was rendered, not what
    /// was presented, and `Backend::fetch_rgba` exists precisely to see
    /// the present. Answering with the pre-present texture would make
    /// `ui.screenshot { source: "surface" }` report "the frame reached
    /// the screen" on a frame that did not. `None` is the honest answer:
    /// not checkable.
    fn fetch_rgba(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        None
    }

    /// **`None`: this backend does not hand its pixels back** — written
    /// out since M83/T83.1, as today's inherited default. `ui.screenshot`
    /// reports "does not render", and `just resume-drive`'s GPU leg reads
    /// the window off the X server instead.
    fn frame_rgba(&self) -> Option<(u32, u32, &[u8])> {
        None
    }

    fn name(&self) -> &str {
        &self.name
    }
}

