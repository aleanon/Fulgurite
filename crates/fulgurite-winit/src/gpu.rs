//! The GPU backend, wired to a window — `docs/plan-capabilities.md`
//! T15.12.
//!
//! Thin on purpose. Everything about *drawing* is in
//! `fulgurite-wgpu`, which knows nothing about winit; everything
//! about *windows* is here, and knows nothing about how a shader is
//! drawn (M83/T83.5: it hands the program's table to `Gpu::set_shaders`
//! and no more). The
//! dependency points one way — this crate names that one, never the
//! reverse — which is what lets the renderer be tested headlessly
//! against the software canvas with no display anywhere in it.

use std::time::Instant;

use fulgurite::Color;
use fulgurite_shell::text::FontStack;
use fulgurite_shell::session::FrameSource;
use fulgurite_wgpu::{Gpu, GpuContext};

use crate::backend::{Backend, BackendTarget, Frame};

/// A window drawn by `fulgurite-wgpu`.
///
/// **One surface on a shared device** since M89b/T89.10: the window layer
/// builds every GPU window through [`GpuBackend::shared`], so a second
/// window adds a surface to the first one's `GpuContext` rather than
/// opening a second device — +1.0 MB resident over the software canvas
/// instead of +17.5 (`docs/results-windows.md` T89.10). Each backend still
/// owns its target, its atlases and its images.
pub struct GpuBackend {
    gpu: Gpu,
    name: String,
    /// Whether K82d's line has been logged: the first frame that holds a
    /// path mask says what share of the glyph atlas's starting size path
    /// masks take (M82/T82.6, `docs/plan-paint-vocabulary.md`), once per
    /// window; `just gpu-parity`'s PaintBox leg reads it.
    masks_logged: bool,
    /// The target read back by the last [`Backend::frame_rgba`] call —
    /// M88/T88.2. Owned here so the call can lend it out; **re-read on
    /// every call**, never on a frame nobody asked about.
    shot: Vec<u8>,
    /// **No frame has been drawn into this renderer's target yet** —
    /// M89b/T89.9. Its first present asks the session for age 0 (the
    /// whole surface), every later one for age 1. A backend is made per
    /// window and per resume, and a resumed window's session is mid-life:
    /// asked for age 1, an unchanged `Boundaries` session answered no
    /// damage, and the new, empty target was presented as it was — both
    /// windows of `just resume-drive`'s `two-windows-gpu` leg came back
    /// one flat colour (100% of their pixels differing).
    fresh: bool,
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
    ///
    /// **A device of its own**, which the window no longer does for its
    /// windows ([`GpuBackend::shared`], M89b/T89.10); kept for a caller
    /// that wants one surface on one device and nothing shared.
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
            shot: Vec::new(),
            fresh: true,
        })
    }

    /// **A window's backend on the context its other windows share** —
    /// M89b/T89.10 (`docs/plan-windows.md`, "M89b: one wgpu device, a
    /// surface per window"). `share` is the window layer's `Weak` to it:
    /// while some window's backend keeps the context alive this adds a
    /// surface to it ([`Gpu::for_window_in`]), and when none does it makes
    /// one ([`Gpu::for_window`]) and points `share` at it. A suspend drops
    /// every backend and so the context; the first window resumed makes
    /// the next one.
    ///
    /// Its [`Backend::name`], the window's stderr line, says `shared`, how
    /// many surfaces the context holds counting this one, and the format
    /// this surface was configured with (second review P11): `gpu (NVIDIA
    /// GeForce RTX 4070 (Vulkan, DiscreteGpu)), shared context (2
    /// surfaces), surface Bgra8Unorm`. The image atlas and its
    /// `ImageSource` stay this surface's: two programs' `ImageId(0)` are
    /// different images (P25).
    pub fn shared(target: &BackendTarget, share: &mut std::rc::Weak<GpuContext>) -> Option<Self> {
        let (w, h) = (target.width.max(1), target.height.max(1));
        let mut gpu = match share.upgrade() {
            Some(ctx) => Gpu::for_window_in(&ctx, target.window.clone(), w, h, target.accept_cpu)?,
            None => Gpu::for_window(target.window.clone(), w, h, target.accept_cpu)?,
        };
        *share = std::rc::Rc::downgrade(gpu.context());
        if let Some(f) = target.font.clone() {
            gpu.set_font(f);
        }
        let surfaces = std::rc::Rc::strong_count(gpu.context());
        let name = format!(
            "gpu ({}), shared context ({surfaces} surface{}), surface {:?}",
            gpu.adapter(),
            if surfaces == 1 { "" } else { "s" },
            gpu.surface_format()
        );
        Some(GpuBackend {
            gpu,
            name,
            masks_logged: false,
            shot: Vec::new(),
            fresh: true,
        })
    }

    pub fn gpu_mut(&mut self) -> &mut Gpu {
        &mut self.gpu
    }
}

impl Backend for GpuBackend {
    fn set_images(&mut self, images: Box<dyn fulgurite::ImageSource>) {
        self.gpu.set_images(images);
    }

    fn take_images(&mut self) -> Option<Box<dyn fulgurite::ImageSource>> {
        self.gpu.take_images()
    }

    /// `Gpu::invalidate_images`: the atlas caches by `(id, frame)`, and a
    /// swapped-in build's store may answer an unchanged pair with new
    /// pixels (M84/T84.6, second review A14).
    fn invalidate_images(&mut self) {
        self.gpu.invalidate_images();
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
    /// (`fulgurite-wgpu`'s `custom.rs`). Without this crate's `shader`
    /// there is no table to compile and every bracket draws its fallback.
    fn set_shaders(&mut self, table: fulgurite_shell::shader::ShaderTable<'_>) -> usize {
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

        // **Age 1, after the first frame.** This is the one real difference from the
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
        //
        // **Except the first** (M89b/T89.9): a new target holds no frame at
        // all, which is age 0 — what softbuffer reports for a new buffer —
        // so the whole surface is drawn. Without it a backend made by a
        // resume presented an empty target under a session with nothing
        // new to draw.
        let age = if core::mem::take(&mut self.fresh) { 0 } else { 1 };
        let regions = crate::regions_for(host.take_damage_for_age(age), viewport);

        let t = Instant::now();
        let render_ns = if regions.is_empty() {
            0
        } else {
            // The session's store first, then the window's (M84/T84.6).
            self.gpu.draw_regions_from(host.list(), &regions, Some(page), host.images());
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
            let start = fulgurite_wgpu::GLYPH_ATLAS as u64 * fulgurite_wgpu::GLYPH_ATLAS as u64;
            eprintln!(
                "fulgurite: path masks {} texels in {} tile(s), {:.2}% of the glyph atlas's start ({}²) -- K82d, bound 25%",
                stats.path_texels,
                stats.path_masks,
                stats.path_texels as f64 / start as f64 * 100.0,
                fulgurite_wgpu::GLYPH_ATLAS
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
    /// not checkable. **What was rendered is [`Backend::frame_rgba`]'s**,
    /// below (M88/T88.2): the readback lives there, under the name that
    /// claims only "drawn".
    fn fetch_rgba(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        None
    }

    /// **The target, read back on this call** — M88/T88.2
    /// (`docs/plan-platform-requests.md`). What the renderer drew, the
    /// same RGBA8 layout as the software canvas, into the owned `shot`
    /// and lent out. Blocking, and never on a frame: a call costs a
    /// copy, a map and a wait (`docs/results-gpu.md`, "Readback"), paid
    /// only by whoever asked. Until T88.2 this answered `None`, and
    /// `ui.screenshot` reported "does not render" on every GPU window.
    fn frame_rgba(&mut self) -> Option<(u32, u32, &[u8])> {
        let (w, h) = self.gpu.size();
        self.shot = self.gpu.read_rgba();
        // A failed map hands back nothing: "not checkable", never a
        // short buffer a reader would index past.
        (self.shot.len() == w as usize * h as usize * 4).then_some((w, h, &self.shot[..]))
    }

    fn name(&self) -> &str {
        &self.name
    }
}

