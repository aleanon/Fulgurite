//! A GPU renderer — `docs/plan-capabilities.md` T15.12.
//!
//! # Why a separate crate
//!
//! Because that is the gate. The M15 gating policy ranks a separate
//! crate above a type parameter above a cargo feature, and wgpu is the
//! most expensive dependency this workspace could acquire: nothing
//! enters the graph unless a manifest names it. The software canvas
//! stays the default on the desktop, and the embedded profile — which
//! has no GPU — never sees this code at all.
//!
//! # Why it exists at all
//!
//! It was deferred once, on the ground that the software rasteriser
//! renders a tally frame in 0.54 ms of a 16 ms budget. That number came
//! from a **0.23 megapixel** window. A CPU rasteriser's cost is per
//! *pixel*, and the sweep that was never run says ~2.7 ms per megapixel,
//! flat: 9.75 ms at 1440p and **22.88 ms at 4K, which is over budget**
//! (`docs/results-raster-scaling.md`). Damage tracking is what holds the
//! desktop together, and damage tracking is exactly what a chart
//! animating or a list scrolling does not give you.
//!
//! So this is architectural, not opportunistic — and still a choice.
//!
//! # The shape
//!
//! Two seams, not one. [`Renderer`] was the seam M10a built and the SVG
//! backend proved; the second is the *surface*, which
//! `lumen-static-winit` had wired to softbuffer. This crate owns its own
//! — [`Gpu::for_window`] takes a window handle and holds the swapchain —
//! so a windowing layer has something to plug in rather than something
//! to work around. Wiring it to a window is that crate's job and knows
//! nothing about shaders; this one knows nothing about winit.
//!
//! Everything is one pipeline and one draw call: the whole `Cmd`
//! vocabulary is signed-distance quads (`src/shader.wgsl`), which keeps
//! paint order — instances rasterise in submission order — and keeps the
//! coverage arithmetic literally the same functions the software canvas
//! uses. Text is a glyph atlas ([`atlas`]), shaped every frame through
//! the same `FontStack::place_run` the software path shapes with, and
//! rasterised only on a miss.
//!
//! # No `unsafe`
//!
//! Like the rest of `crates/*/src`, checked by `scripts/check_no_unsafe.sh`.
//! That is why [`Gpu::for_window`] wants a `Send + Sync` window handle
//! rather than reaching for `create_surface_unsafe`.
#![forbid(unsafe_code)]

pub mod atlas;

use lumen_static::display::{Cmd, Color, DisplayList, ImageId, ImageSource, Renderer, Stops};
use lumen_static_shell::text::{FontStack, GlyphId};

use atlas::Atlas;

/// `[x0, y0, x1, y1]`, absolute — the core's [`lumen_static::display::Rect`].
type Rect = [f32; 4];

const KIND_ROUNDED: u32 = 0;
const KIND_SHADOW: u32 = 1;
const KIND_LINE: u32 = 2;
const KIND_GRADIENT: u32 = 3;
const KIND_GLYPH: u32 = 4;
const KIND_IMAGE: u32 = 5;
const KIND_BLUR: u32 = 6;
const KIND_GLASS: u32 = 7;
const KIND_STROKE: u32 = 8;
/// `software::LIGHT_Z` / `LIGHT_XY`.
const LIGHT_Z: f32 = 0.62;
const LIGHT_XY: f32 = 0.784_599_3;

/// Glyph atlas edge, in texels. 1 MB of R8, which holds a few thousand
/// glyphs at UI sizes — far more than a screen has.
const GLYPH_ATLAS: u32 = 1024;
/// Image atlas edge. **Allocated on the first image command**, so an
/// application with no images pays nothing for the capability.
const IMAGE_ATLAS: u32 = 1024;

/// The target texture's format, and the readback's.
///
/// **Deliberately not sRGB.** An `*Srgb` surface blends in linear space,
/// which is the more correct thing to do and is *not* what
/// `software::Canvas` does — it blends 8-bit straight sRGB, like every
/// other UI rasteriser. Matching the software arm is worth more here
/// than being right in isolation: the two backends have to be
/// comparable, and `tests/parity.rs` is the reason.
const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// One instanced quad. Mirrors `Instance` in `shader.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Quad {
    rect: [f32; 4],
    clip: [f32; 4],
    color: [f32; 4],
    /// (corner radius, spread or half-width, unused, gradient index)
    params: [f32; 4],
    uv: [f32; 4],
    kind: u32,
    _pad: [u32; 3],
}

/// Four colour stops, out of line. Mirrors `Grad` in `shader.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Grad {
    color: [[f32; 4]; 4],
    at: [f32; 4],
    len: u32,
    _pad: [u32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    viewport: [f32; 2],
    _pad: [f32; 2],
}

fn linear(c: Color) -> [f32; 4] {
    [c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0, c.3 as f32 / 255.0]
}

/// Intersect a rect with every clip on the stack; `None` if nothing
/// survives.
///
/// The same function as `software::clipped`, including the NaN rejection
/// on entry — `f32::max`/`min` return the non-NaN operand, so without it
/// a NaN coordinate is silently *replaced by the clip bound* and a NaN
/// rect inside a clip scope fills the whole scope solid. A second
/// renderer inherits the bug unless it inherits the guard.
fn clipped(rect: Rect, stack: &[Rect]) -> Option<Rect> {
    if !rect.iter().all(|v| v.is_finite()) || rect[0] >= rect[2] || rect[1] >= rect[3] {
        return None;
    }
    let mut r = rect;
    for c in stack {
        r = [r[0].max(c[0]), r[1].max(c[1]), r[2].min(c[2]), r[3].min(c[3])];
        if r[0] >= r[2] || r[1] >= r[3] {
            return None;
        }
    }
    Some(r)
}

/// Round a region outward to whole pixels.
///
/// Both backends follow one convention: a region covers
/// `floor(x0)..ceil(x1)`. The software canvas does so in every command
/// and in its clear (its module docs; the glyph blit was the one
/// exception, fixed in M40/T40.2), so its partial redraw is exact too.
/// Snapping here is what makes a partial redraw on this backend
/// **exactly** equal to a full one rather than approximately: a boundary
/// pixel is wholly inside the damage or wholly outside it, never partly
/// covered by an antialiased clear. Regions arrive in physical pixels
/// (M40/T40.1), so at a fractional scale their edges are fractional as a
/// rule, not an exception.
fn snap_out(r: Rect) -> Rect {
    [r[0].floor(), r[1].floor(), r[2].ceil(), r[3].ceil()]
}

/// A GPU device, a target texture, and everything needed to draw a
/// display list into it.
pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter_name: String,

    /// The window's swapchain, when there is a window. Headless
    /// rendering leaves this `None` and reads the target back instead.
    surface: Option<wgpu::Surface<'static>>,
    config: wgpu::SurfaceConfiguration,

    /// The frame this crate owns. Never handed to a presentation
    /// engine, so it always holds exactly the previous frame — see
    /// `src/blit.wgsl` for why that matters.
    target: wgpu::Texture,
    target_view: wgpu::TextureView,
    size: (u32, u32),

    pipeline: wgpu::RenderPipeline,
    blit_pipeline: wgpu::RenderPipeline,
    bind_layout: wgpu::BindGroupLayout,
    blit_layout: wgpu::BindGroupLayout,
    bind: wgpu::BindGroup,
    blit_bind: wgpu::BindGroup,
    sampler: wgpu::Sampler,

    globals: wgpu::Buffer,
    quads: wgpu::Buffer,
    quad_cap: usize,
    grads: wgpu::Buffer,
    grad_cap: usize,
    /// Glass materials, out of line like the gradients — M22/T22.6.
    mats: wgpu::Buffer,
    mat_cap: usize,

    /// The two ping-ponged scratch targets a backdrop filter needs. A
    /// texture copy cannot happen inside a render pass, so every glass
    /// command splits the frame: the target is blurred into these and
    /// read back out.
    scratch: [wgpu::Texture; 2],
    scratch_view: [wgpu::TextureView; 2],
    /// Group 1 for each of the three things a pass can read: the
    /// target, and the two scratch textures.
    src_binds: [wgpu::BindGroup; 3],
    src_layout: wgpu::BindGroupLayout,
    /// **Linear**, where `sampler` is `Nearest`. A refracted read lands
    /// at a fractional coordinate by definition.
    linear: wgpu::Sampler,

    glyph_tex: wgpu::Texture,
    glyph_atlas: Atlas<GlyphId>,
    image_tex: wgpu::Texture,
    image_atlas: Atlas<ImageId>,
    /// Rebuild [`Gpu::bind`] before the next draw — a texture or the
    /// gradient buffer was reallocated and the old group names a
    /// resource that no longer exists.
    bind_dirty: bool,

    /// The face runs are shaped and rasterised with. Must be the same
    /// one layout measured with, or boxes are sized for one face and
    /// filled with another — the bug that made the first window wrap
    /// "TALLY" to "TAL / LY".
    font: Option<FontStack>,
    /// Where `Cmd::Image` is resolved to pixels.
    images: Option<Box<dyn ImageSource>>,
    /// **A fault switch, debug builds only** — M47/T47.0 (R7-F7).
    /// `LUMEN_GPU_DROP_FIRST=n` makes the first `n` presents answer as if
    /// the surface were Outdated, so a live gate can see what an idle
    /// window does with a dropped present.
    #[cfg(debug_assertions)]
    drop_first: u32,
}

/// Which texture group 1 names for a pass — M22/T22.6.
const SRC_TARGET: usize = 0;
const SRC_SCRATCH_A: usize = 1;
const SRC_SCRATCH_B: usize = 2;

/// Where a glass command splits the frame, and which instances blur its
/// backdrop first.
struct Split {
    /// Instance index of the glass quad itself.
    glass: u32,
    /// Instance indices of its blur passes, in order. Six of them: three
    /// box passes each way, at the widths SVG specifies and
    /// `software::box_windows` computes.
    blur: [u32; 6],
}

/// The atlas ran out of room mid-frame. Answered by resetting it and
/// building the frame again, not by dropping a glyph.
struct Overflow;

impl Gpu {
    /// A renderer with no window: draws into a texture, read back with
    /// [`Gpu::read_rgba`].
    ///
    /// This is what the tests use, and it is why the GPU path can be
    /// held to the software path's pixels without a display.
    pub fn headless(width: u32, height: u32) -> Option<Gpu> {
        Self::build(width, height, None)
    }

    /// A renderer that owns a swapchain on `window`.
    ///
    /// `window` must be `Send + Sync` and outlive the renderer, which is
    /// what wgpu's safe surface constructor asks for; `lumen-static-winit`
    /// holds its window in an `Arc` for exactly this.
    pub fn for_window<W>(window: W, width: u32, height: u32) -> Option<Gpu>
    where
        W: wgpu::DisplayAndWindowHandle + 'static,
    {
        Self::build(width, height, Some(Box::new(window)))
    }

    fn build(
        width: u32,
        height: u32,
        window: Option<Box<dyn wgpu::DisplayAndWindowHandle>>,
    ) -> Option<Gpu> {
        let (width, height) = (width.max(1), height.max(1));
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());

        let surface = match window {
            Some(w) => {
                Some(instance.create_surface(wgpu::SurfaceTarget::DisplayAndWindow(w)).ok()?)
            }
            None => None,
        };

        // `None` here is the whole CPU-fallback story: no adapter, no
        // GPU renderer, and the caller keeps its software canvas. A
        // machine with no driver, a container with no `/dev/dri`, a
        // remote session — all of them arrive here, and none of them is
        // an error worth taking a window down for.
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: surface.as_ref(),
            apply_limit_buckets: false,
        }))
        .ok()?;

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("lumen-static"),
            // The adapter's own limits, so the request cannot fail for
            // asking too much. Nothing here needs more than a texture
            // and a vertex buffer.
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .ok()?;

        let info = adapter.get_info();
        let adapter_name = format!("{} ({:?})", info.name, info.backend);

        // `get_default_config` and then override the format: a
        // configuration built by hand goes stale every time wgpu adds a
        // field, and this one has to be rebuilt on every resize.
        let mut config = match &surface {
            Some(s) => {
                let mut c = s.get_default_config(&adapter, width, height)?;
                let caps = s.get_capabilities(&adapter);
                // Prefer a **non-sRGB** format, matching the target: the
                // blit is a copy, and an sRGB surface would re-encode
                // colours the software arm leaves alone.
                if let Some(f) = caps.formats.iter().copied().find(|f| !f.is_srgb()) {
                    c.format = f;
                }
                // Vsync. A UI that renders faster than the display
                // refreshes is spending power on frames nobody sees, and
                // the frame loop is already "redraw when something
                // changed".
                c.present_mode = wgpu::PresentMode::AutoVsync;
                s.configure(&device, &c);
                c
            }
            None => {
                let mut c = default_config(width, height);
                c.format = TARGET_FORMAT;
                c
            }
        };
        config.width = width;
        config.height = height;
        // The blit pipeline is built against whatever the surface
        // accepted, which is why the configuration is settled first.
        let surface_format = config.format;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lumen-static shapes"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let blit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lumen-static blit"),
            source: wgpu::ShaderSource::Wgsl(include_str!("blit.wgsl").into()),
        });

        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lumen-static binds"),
            entries: &[
                uniform_entry(0),
                texture_entry(1),
                texture_entry(2),
                sampler_entry(3),
                storage_entry(4),
                storage_entry(5),
            ],
        });
        let src_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lumen-static glass source"),
            entries: &[texture_entry(0), sampler_entry(1)],
        });
        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lumen-static blit binds"),
            entries: &[texture_entry(0), sampler_entry(1)],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("lumen-static sampler"),
            // Nearest, not linear. A glyph quad is placed at integer
            // pixels and its uv rect covers whole texels, so nearest is
            // an exact lookup — the same bytes `draw_run` blits. Linear
            // would soften every glyph by a quarter pixel against the
            // software arm for nothing.
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let linear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("lumen-static linear sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lumen-static shapes"),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[Some(&bind_layout), Some(&src_layout)],
                immediate_size: 0,
            })),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: core::mem::size_of::<Quad>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x4, 1 => Float32x4, 2 => Float32x4,
                        3 => Float32x4, 4 => Float32x4, 5 => Uint32
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: TARGET_FORMAT,
                    // Straight (non-premultiplied) source over, and an
                    // alpha channel that converges to opaque — which is
                    // what `Canvas::blend` does when it writes 255 into
                    // the alpha byte unconditionally.
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lumen-static blit"),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[Some(&blit_layout)],
                immediate_size: 0,
            })),
            vertex: wgpu::VertexState {
                module: &blit_shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &blit_shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(surface_format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let (target, target_view) = make_target(&device, width, height);
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lumen-static globals"),
            size: core::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let quads = make_quad_buffer(&device, 256);
        let grads = make_grad_buffer(&device, 4);
        let mats = make_mat_buffer(&device, 2);
        let (scratch, scratch_view) = make_scratch(&device, width, height);
        let glyph_tex = make_atlas_texture(&device, GLYPH_ATLAS, wgpu::TextureFormat::R8Unorm);
        // A 1x1 placeholder until an image command asks for a real one.
        let image_tex = make_atlas_texture(&device, 1, wgpu::TextureFormat::Rgba8Unorm);

        let bind = make_bind(
            &device, &bind_layout, &globals, &glyph_tex, &image_tex, &sampler, &grads, &mats,
        );
        let blit_bind = make_blit_bind(&device, &blit_layout, &target_view, &sampler);
        let src_binds = make_src_binds(&device, &src_layout, &target_view, &scratch_view, &linear);

        Some(Gpu {
            device,
            queue,
            adapter_name,
            surface,
            config,
            target,
            target_view,
            size: (width, height),
            pipeline,
            blit_pipeline,
            bind_layout,
            blit_layout,
            bind,
            blit_bind,
            sampler,
            globals,
            quads,
            quad_cap: 256,
            grads,
            grad_cap: 4,
            mats,
            mat_cap: 2,
            scratch,
            scratch_view,
            src_binds,
            src_layout,
            linear,
            glyph_tex,
            glyph_atlas: Atlas::new(GLYPH_ATLAS),
            image_tex,
            image_atlas: Atlas::new(1),
            bind_dirty: false,
            font: None,
            images: None,
            #[cfg(debug_assertions)]
            drop_first: std::env::var("LUMEN_GPU_DROP_FIRST").ok().and_then(|v| v.parse().ok()).unwrap_or(0),
        })
    }

    /// Which device this ended up on — for a startup line, and for a
    /// benchmark that must say what it measured.
    pub fn adapter(&self) -> &str {
        &self.adapter_name
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Draw text with `font`. Without one, runs fall back to a filled
    /// block per character at the advance `BlockMetrics` assumes — the
    /// same stand-in the software canvas uses, so what is drawn still
    /// matches what was measured.
    pub fn set_font(&mut self, font: FontStack) {
        self.font = Some(font);
        // Glyph identities are per face, and nothing in the atlas is
        // valid for a face it was not rasterised from.
        self.glyph_atlas.reset();
    }

    /// Where `Cmd::Image` is resolved to pixels.
    pub fn set_images(&mut self, images: Box<dyn ImageSource>) {
        self.images = Some(images);
        self.image_atlas.reset();
    }

    /// Give the image source back, so a window that drops this renderer
    /// on suspend can hand it to the next one (M47/T47.7).
    pub fn take_images(&mut self) -> Option<Box<dyn ImageSource>> {
        self.image_atlas.reset();
        self.images.take()
    }

    /// Match the target and the swapchain to a new window size.
    ///
    /// The target's contents do not survive, which is correct: every
    /// laid-out position changed, so `Host::resize` has already reset
    /// retention and the next frame is a full redraw anyway.
    pub fn resize(&mut self, width: u32, height: u32) {
        let (width, height) = (width.max(1), height.max(1));
        if self.size == (width, height) {
            return;
        }
        self.size = (width, height);
        let (t, v) = make_target(&self.device, width, height);
        self.target = t;
        self.target_view = v;
        self.blit_bind =
            make_blit_bind(&self.device, &self.blit_layout, &self.target_view, &self.sampler);
        // The scratch targets are the target's size, and group 1 names
        // all three views — a resize that rebuilt one and not the others
        // would blur last frame's dimensions into this one's.
        let (st, sv) = make_scratch(&self.device, width, height);
        self.scratch = st;
        self.scratch_view = sv;
        self.src_binds = make_src_binds(
            &self.device,
            &self.src_layout,
            &self.target_view,
            &self.scratch_view,
            &self.linear,
        );
        if let Some(s) = &self.surface {
            self.config.width = width;
            self.config.height = height;
            s.configure(&self.device, &self.config);
        }
    }

    /// Fill the whole target with `c` — [`software::Canvas::clear`]'s
    /// counterpart, for a caller that wants a known ground before
    /// rendering.
    ///
    /// [`software::Canvas::clear`]: lumen_static_shell::software::Canvas::clear
    pub fn clear(&mut self, c: Color) {
        let (w, h) = self.size;
        self.draw(&DisplayList::new(), None, Some((c, [0.0, 0.0, w as f32, h as f32])));
    }

    /// Draw `list` into the target.
    ///
    /// `damage` limits every command to a region, exactly as
    /// `Canvas::render_within` does; `page` clears that region first.
    /// Both in one call because both are one render pass — the clear is
    /// an opaque quad at the head of the instance stream, not a separate
    /// submission.
    pub fn draw(&mut self, list: &DisplayList, damage: Option<Rect>, page: Option<(Color, Rect)>) {
        // The single-rect form, kept for callers that have one: a
        // `None` damage is the whole surface, which is a clip that
        // clips nothing.
        let (w, h) = self.size();
        let one = [damage.or(page.map(|(_, r)| r)).unwrap_or([0.0, 0.0, w as f32, h as f32])];
        self.draw_regions(list, &one, page.map(|(c, _)| c));
    }

    /// Draw, clipped to **each** damage region (M26/T26.12).
    ///
    /// `Prefer::Auto` picks the GPU first and this used to take one
    /// rect: the shell handed over `Damage::clip_to`, the **bounding
    /// box** of M18's region set. Two small changes at opposite corners
    /// of a window bound to the whole window, which is the merging cost
    /// M18 removed on the software path and re-introduced here.
    ///
    /// **No scissor is needed**, and that is a property of this
    /// backend: a quad carries its own `clip` in its vertex data, so a
    /// region is expressed by building the frame's quads once per
    /// region with that region's clip. Regions are **disjoint by
    /// construction** — `retain::Regions` merges overlapping ones as
    /// they are added — so nothing is drawn twice and a translucent
    /// command cannot be blended into the same pixel from two regions.
    ///
    /// The page clear follows the same rule: one opaque quad per
    /// region, at the head of that region's instances, exactly as the
    /// single-rect form put one at the head of the frame.
    pub fn draw_regions(&mut self, list: &DisplayList, regions: &[Rect], page: Option<Color>) {
        if regions.is_empty() {
            return;
        }
        let mut quads = Vec::new();
        // Twice at most: an atlas that fills is reset and the frame is
        // built again from an empty one. Rebuilding is what keeps the
        // frame correct — the alternative is a frame with glyphs
        // missing, which is a rendering bug rather than a slow frame.
        for attempt in 0..2 {
            if attempt == 1 {
                self.glyph_atlas.reset();
                self.image_atlas.reset();
            }
            quads.clear();
            let mut grads = Vec::new();
            // **Cleared with the quads, not declared outside the loop.**
            // A retry rebuilds the frame from an empty atlas, and a
            // split names an *instance index*: carrying the first
            // attempt's splits into the second would blur at whatever
            // the rebuilt list happens to have at that position, which
            // on an atlas reset is a different command every time.
            let mut mats: Vec<Mat> = Vec::new();
            let mut splits: Vec<Split> = Vec::new();
            // **Once per region.** A quad carries its own clip, so a
            // region is expressed by building the frame's quads again
            // with that region's clip — no scissor, and no second pass.
            // The regions are disjoint (`retain::Regions` merges
            // overlapping ones as they are added), so no pixel is
            // written twice and a translucent command cannot blend into
            // the same pixel from two of them.
            let mut outcome = Ok(());
            for region in regions {
                let r = snap_out(*region);
                if let Some(c) = page {
                    // One clear per region, at the head of that
                    // region's instances — opaque, like
                    // `Canvas::clear_within`: a page colour with alpha
                    // is a page colour that lets the last frame through.
                    quads.push(Quad {
                        rect: r,
                        clip: r,
                        color: linear(Color(c.0, c.1, c.2, 255)),
                        params: [0.0; 4],
                        uv: [0.0; 4],
                        kind: KIND_ROUNDED,
                        _pad: [0; 3],
                    });
                }
                outcome = self.collect(list, Some(r), &mut quads, &mut grads, &mut mats, &mut splits);
                if outcome.is_err() {
                    break;
                }
            }
            match outcome {
                Ok(()) => {
                    self.submit(&quads, &grads, &mats, &splits);
                    return;
                }
                Err(Overflow) if attempt == 0 => continue,
                // A single frame that does not fit an empty atlas. Draw
                // what fits rather than nothing: a frame missing its
                // last glyphs is worse than one that is whole, and far
                // better than a black window.
                Err(Overflow) => {
                    self.submit(&quads, &grads, &mats, &splits);
                    return;
                }
            }
        }
    }

    /// Walk the list into instances, in paint order.
    ///
    /// The structure is `software::Canvas::render_impl`'s, arm for arm,
    /// including the clip-stack rule that the damage scope is never
    /// popped. Where they differ is only that this one emits a quad
    /// where that one writes pixels.
    fn collect(
        &mut self,
        list: &DisplayList,
        damage: Option<Rect>,
        out: &mut Vec<Quad>,
        grads: &mut Vec<Grad>,
        mats: &mut Vec<Mat>,
        splits: &mut Vec<Split>,
    ) -> Result<(), Overflow> {
        // Taken for the pass and put back at the end, like the software
        // canvas does: the source is a field and the atlas needs
        // `&mut self`.
        let images = self.images.take();
        let font = self.font.clone();
        let mut clip: Vec<Rect> = Vec::new();
        if let Some(d) = damage {
            clip.push(d);
        }
        let mut result = Ok(());

        for c in &list.cmds {
            match c {
                Cmd::Rect { rect, color } => {
                    if let Some(r) = clipped(*rect, &clip) {
                        out.push(shape(*rect, r, *color, 0.0, KIND_ROUNDED));
                    }
                }
                Cmd::RoundedRect { rect, color, radius } => {
                    if let Some(r) = clipped(*rect, &clip) {
                        // The radius is clamped the way the software
                        // canvas clamps it, so a radius larger than the
                        // box gives a stadium on both backends rather
                        // than a different artefact on each.
                        let rad = radius.min((rect[2] - rect[0]) / 2.0).min((rect[3] - rect[1]) / 2.0);
                        out.push(shape(*rect, r, *color, rad, KIND_ROUNDED));
                    }
                }
                Cmd::StrokedRect { rect, color, radius, width } => {
                    if let Some(r) = clipped(*rect, &clip) {
                        let half_w = (rect[2] - rect[0]) / 2.0;
                        let half_h = (rect[3] - rect[1]) / 2.0;
                        // Clamped exactly as the software canvas clamps
                        // it, so an over-wide border is a filled box on
                        // both backends rather than a different artefact
                        // on each (`just gpu-parity`).
                        let bw = width.min(half_w).min(half_h);
                        let rad = radius.min(half_w).min(half_h).max(0.0);
                        let mut q = shape(*rect, r, *color, rad, KIND_STROKE);
                        q.params[1] = bw;
                        out.push(q);
                    }
                }
                Cmd::Glass { rect, radius, material } => {
                    // **The real filter, since M22/T22.6.** Until then
                    // this drew a flat rounded rect, because a texture
                    // copy cannot happen inside a render pass
                    // (`docs/plan-glass.md` §1.3). It still cannot: the
                    // frame is split instead, and the six blur
                    // instances below run in their own passes between
                    // the backdrop and the panel.
                    let Some(r) = clipped(*rect, &clip) else { continue };
                    let m = *material;
                    let prim = lumen_static::display::glass_primary(*rect, &m);
                    let rad = radius
                        .min((prim[2] - prim[0]) / 2.0)
                        .min((prim[3] - prim[1]) / 2.0);
                    // The capture, from the **shared** expression the
                    // damage recorder and the software canvas both use.
                    let src = lumen_static::display::glass_source(*rect, &m);
                    let (sw, sh) = (self.size.0 as f32, self.size.1 as f32);
                    let src = [
                        src[0].floor().max(0.0),
                        src[1].floor().max(0.0),
                        src[2].ceil().min(sw),
                        src[3].ceil().min(sh),
                    ];
                    if src[2] <= src[0] || src[3] <= src[1] {
                        continue;
                    }

                    // The blur, at the widths `software::box_windows`
                    // computes for the same sigma. The CPU also
                    // downsamples by `k` and blurs the small image; this
                    // blurs at full resolution with the windows scaled
                    // by `k`, which delivers the same sigma and is
                    // identical when `k` is 1 — which it is at the
                    // liquid preset, and so at the parity scene.
                    let mut blur = [0u32; 6];
                    if m.blur > 0.0 {
                        let (k, d, _) = lumen_static_shell::software::blur_plan(m.blur);
                        let win = lumen_static_shell::software::box_windows(d);
                        for (i, slot) in blur.iter_mut().enumerate() {
                            let (l, rgt) = win[i / 2];
                            let dir = if i % 2 == 0 { [1.0, 0.0] } else { [0.0, 1.0] };
                            *slot = out.len() as u32;
                            out.push(Quad {
                                rect: src,
                                clip: src,
                                color: [0.0; 4],
                                params: [
                                    dir[0],
                                    dir[1],
                                    (l * k) as f32,
                                    (rgt * k) as f32,
                                ],
                                uv: src,
                                kind: KIND_BLUR,
                                _pad: [0; 3],
                            });
                        }
                    } else {
                        // No blur: one zero-width pass still has to run,
                        // or the panel would sample a scratch nothing
                        // wrote this frame.
                        for (i, slot) in blur.iter_mut().enumerate() {
                            *slot = out.len() as u32;
                            out.push(Quad {
                                rect: src,
                                clip: src,
                                color: [0.0; 4],
                                params: [
                                    if i % 2 == 0 { 1.0 } else { 0.0 },
                                    if i % 2 == 0 { 0.0 } else { 1.0 },
                                    0.0,
                                    0.0,
                                ],
                                uv: src,
                                kind: KIND_BLUR,
                                _pad: [0; 3],
                            });
                        }
                    }

                    let (odx, ody) = lumen_static::display::morph_offset(&m);
                    let morphing = lumen_static::display::morphing(&m);
                    let t = m.tint;
                    let t2 = m.tint_over_light;
                    let rim = m.rim.max(0.0);
                    mats.push(Mat {
                        tint: linear(t),
                        tint2: linear(t2),
                        prim,
                        p0: [rim, m.refraction, m.specular, m.shadow as f32 / 255.0],
                        p1: [
                            m.light.cos() * LIGHT_XY,
                            m.light.sin() * LIGHT_XY,
                            LIGHT_Z,
                            m.saturation as f32
                                / lumen_static::display::Material::NEUTRAL_SATURATION as f32,
                        ],
                        p2: [m.dispersion as f32 / 255.0, m.adapt as f32 / 255.0, odx, ody],
                        p3: [(rim * 3.0).max(4.0), morphing as u32 as f32, rad, 0.0],
                    });
                    let glass = out.len() as u32;
                    out.push(Quad {
                        rect: *rect,
                        clip: r,
                        color: [0.0; 4],
                        params: [rad, 0.0, 0.0, (mats.len() - 1) as f32],
                        uv: src,
                        kind: KIND_GLASS,
                        _pad: [0; 3],
                    });
                    splits.push(Split { glass, blur });
                }
                Cmd::Shadow { rect, color, radius, spread } => {
                    let reach =
                        [rect[0] - spread, rect[1] - spread, rect[2] + spread, rect[3] + spread];
                    if let Some(r) = clipped(reach, &clip) {
                        let rad = radius
                            .clamp(0.0, (rect[2] - rect[0]).min(rect[3] - rect[1]) / 2.0);
                        out.push(Quad {
                            rect: reach,
                            clip: r,
                            color: linear(*color),
                            params: [rad, *spread, 0.0, 0.0],
                            uv: *rect,
                            kind: KIND_SHADOW,
                            _pad: [0; 3],
                        });
                    }
                }
                Cmd::Line { a, b, width, color } => {
                    let hw = (width / 2.0).max(0.5);
                    let bounds = [
                        a.x.min(b.x) - hw,
                        a.y.min(b.y) - hw,
                        a.x.max(b.x) + hw,
                        a.y.max(b.y) + hw,
                    ];
                    if let Some(r) = clipped(bounds, &clip) {
                        out.push(Quad {
                            rect: bounds,
                            clip: r,
                            color: linear(*color),
                            params: [0.0, hw, 0.0, 0.0],
                            uv: [a.x, a.y, b.x, b.y],
                            kind: KIND_LINE,
                            _pad: [0; 3],
                        });
                    }
                }
                Cmd::Gradient { rect, stops, angle } => {
                    let Some(r) = clipped(*rect, &clip) else { continue };
                    // Axis and span computed here, once, exactly as
                    // `fill_gradient` computes them — the shader gets
                    // the answer rather than the trigonometry.
                    let (ux, uy) = (angle.cos(), angle.sin());
                    let half = ((rect[2] - rect[0]) * ux).abs() / 2.0
                        + ((rect[3] - rect[1]) * uy).abs() / 2.0;
                    let span = if half > 0.0 { half * 2.0 } else { 1.0 };
                    let idx = grads.len() as f32;
                    grads.push(grad_of(*stops));
                    out.push(Quad {
                        rect: *rect,
                        clip: r,
                        color: [0.0; 4],
                        params: [0.0, 0.0, 0.0, idx],
                        uv: [ux, uy, span, 0.0],
                        kind: KIND_GRADIENT,
                        _pad: [0; 3],
                    });
                }
                Cmd::Text { rect, text, size, weight, color } => {
                    // Cull the run before shaping it — the check that
                    // took a 3000-row frame from 7.3 ms to 0.16 ms on
                    // the software path (`docs/results-competitive.md`).
                    // Padded by one em, because a glyph may overhang its
                    // run's box.
                    let pad = *size;
                    let reach = [rect[0] - pad, rect[1] - pad, rect[2] + pad, rect[3] + pad];
                    if clipped(reach, &clip).is_none() {
                        continue;
                    }
                    let s = list.str_of(*text);
                    match &font {
                        Some(f) => {
                            if self.glyphs(f, rect, s, *size, *weight, *color, &clip, out).is_err()
                            {
                                result = Err(Overflow);
                            }
                        }
                        None => blocks(rect, s, *size, *color, &clip, out),
                    }
                }
                Cmd::Image { rect, id, tint } => {
                    let Some(r) = clipped(*rect, &clip) else { continue };
                    let Some(src) = images.as_ref() else { continue };
                    match self.image_slot(src.as_ref(), *id) {
                        Ok(Some(uv)) => out.push(Quad {
                            rect: *rect,
                            clip: r,
                            color: linear(*tint),
                            params: [0.0; 4],
                            uv,
                            kind: KIND_IMAGE,
                            _pad: [0; 3],
                        }),
                        // A missing asset draws nothing rather than
                        // taking the frame down.
                        Ok(None) => {}
                        Err(Overflow) => result = Err(Overflow),
                    }
                }
                Cmd::PushClip { rect } => clip.push(*rect),
                Cmd::PopClip => {
                    // Never pop the damage scope; an unbalanced list
                    // must not let a later command escape the region.
                    if clip.len() > damage.is_some() as usize {
                        clip.pop();
                    }
                }
            }
        }

        self.images = images;
        result
    }

    /// One text run's glyphs, atlased and placed.
    #[allow(clippy::too_many_arguments)]
    fn glyphs(
        &mut self,
        font: &FontStack,
        rect: &Rect,
        text: &str,
        size: f32,
        weight: f32,
        color: Color,
        clip: &[Rect],
        out: &mut Vec<Quad>,
    ) -> Result<(), Overflow> {
        let (placed, _) = font.place_run(text, size, weight, rect[2] - rect[0]);
        let col = linear(color);
        for g in placed {
            // Rounded to whole pixels, like `draw_run` — a glyph bitmap
            // has no subpixel phase, so placing it at a fraction would
            // resample what the rasteriser already decided.
            let (gx, gy) = ((rect[0] + g.x).round(), (rect[1] + g.y).round());
            let dst = [gx, gy, gx + g.w as f32, gy + g.h as f32];
            let Some(r) = clipped(dst, clip) else { continue };
            let uv = match self.glyph_atlas.get(&g.id) {
                Some(slot) => slot.uv(self.glyph_atlas.size()),
                None => {
                    let Some((w, h, cov)) = font.rasterize(g.id) else { continue };
                    let Some(slot) = self.glyph_atlas.insert(g.id, w, h) else {
                        return Err(Overflow);
                    };
                    write_texture(&self.queue, &self.glyph_tex, slot, &cov, 1);
                    slot.uv(self.glyph_atlas.size())
                }
            };
            out.push(Quad {
                rect: dst,
                clip: r,
                color: col,
                params: [0.0; 4],
                uv,
                kind: KIND_GLYPH,
                _pad: [0; 3],
            });
        }
        Ok(())
    }

    /// The atlas uv for an image, uploading it on a miss.
    fn image_slot(
        &mut self,
        src: &dyn ImageSource,
        id: ImageId,
    ) -> Result<Option<[f32; 4]>, Overflow> {
        if let Some(slot) = self.image_atlas.get(&id) {
            return Ok(Some(slot.uv(self.image_atlas.size())));
        }
        let Some((iw, ih, px)) = src.image(id) else { return Ok(None) };
        if iw == 0 || ih == 0 || px.len() < iw as usize * ih as usize * 4 {
            return Ok(None);
        }
        if self.image_atlas.size() == 1 {
            // First image: allocate the atlas now rather than at
            // startup, so an application with none pays nothing.
            self.image_tex =
                make_atlas_texture(&self.device, IMAGE_ATLAS, wgpu::TextureFormat::Rgba8Unorm);
            self.image_atlas = Atlas::new(IMAGE_ATLAS);
            self.bind_dirty = true;
        }
        let Some(slot) = self.image_atlas.insert(id, iw, ih) else {
            // An image too large for the atlas is refused, not scaled:
            // silently resampling someone's asset is a worse surprise
            // than a missing one, and `ImageSource` already says a
            // missing image draws nothing.
            return if iw > self.image_atlas.size() || ih > self.image_atlas.size() {
                Ok(None)
            } else {
                Err(Overflow)
            };
        };
        write_texture(&self.queue, &self.image_tex, slot, px, 4);
        Ok(Some(slot.uv(self.image_atlas.size())))
    }

    /// Upload the instances and run the passes.
    ///
    /// **Passes, plural, since M22/T22.6.** One pass is enough while
    /// nothing reads what has already been drawn; a backdrop filter
    /// does, and a texture cannot be sampled by the same pass that is
    /// writing it. So `splits` names the instance index of each glass
    /// command, and the frame becomes: everything before the panel,
    /// then six blur passes ping-ponging between the two scratch
    /// targets, then the panel itself — repeated per panel, then the
    /// remainder. `G + 1` main passes and `6G` blur passes.
    fn submit(&mut self, quads: &[Quad], grads: &[Grad], mats: &[Mat], splits: &[Split]) {
        if quads.is_empty() {
            return;
        }
        if quads.len() > self.quad_cap {
            self.quad_cap = quads.len().next_power_of_two();
            self.quads = make_quad_buffer(&self.device, self.quad_cap);
        }
        if grads.len() > self.grad_cap {
            self.grad_cap = grads.len().next_power_of_two();
            self.grads = make_grad_buffer(&self.device, self.grad_cap);
            self.bind_dirty = true;
        }
        if mats.len() > self.mat_cap {
            self.mat_cap = mats.len().next_power_of_two();
            self.mats = make_mat_buffer(&self.device, self.mat_cap);
            self.bind_dirty = true;
        }
        if self.bind_dirty {
            self.bind = make_bind(
                &self.device,
                &self.bind_layout,
                &self.globals,
                &self.glyph_tex,
                &self.image_tex,
                &self.sampler,
                &self.grads,
                &self.mats,
            );
            self.bind_dirty = false;
        }

        self.queue.write_buffer(
            &self.globals,
            0,
            bytemuck::bytes_of(&Globals {
                viewport: [self.size.0 as f32, self.size.1 as f32],
                _pad: [0.0; 2],
            }),
        );
        self.queue.write_buffer(&self.quads, 0, bytemuck::cast_slice(quads));
        if !grads.is_empty() {
            self.queue.write_buffer(&self.grads, 0, bytemuck::cast_slice(grads));
        }
        if !mats.is_empty() {
            self.queue.write_buffer(&self.mats, 0, bytemuck::cast_slice(mats));
        }

        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });

        // One pass over `range`, reading `src` and writing `dst`.
        //
        // **Load, never clear.** The target holds the previous frame,
        // and a partial redraw is exactly the statement that most of it
        // is still correct; clearing would make damage a lie. The
        // scratch targets are fully overwritten by their blur pass, so
        // loading them costs nothing and needs no special case.
        let run = |enc: &mut wgpu::CommandEncoder,
                       dst: &wgpu::TextureView,
                       src: usize,
                       range: core::ops::Range<u32>| {
            if range.is_empty() {
                return;
            }
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shapes"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: dst,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind, &[]);
            pass.set_bind_group(1, &self.src_binds[src], &[]);
            pass.set_vertex_buffer(0, self.quads.slice(..));
            pass.draw(0..4, range);
        };

        let mut at = 0u32;
        for sp in splits {
            // Everything under the panel, into the target. **Up to the
            // first blur instance, not up to the glass**: the six blur
            // quads are pushed immediately before the panel and belong
            // to their own passes, so a range that ran to `glass` would
            // draw them into the target and paint the frame with the
            // backdrop filter's intermediates.
            run(&mut enc, &self.target_view, SRC_SCRATCH_A, at..sp.blur[0]);
            // Capture and blur. The first pass reads the target itself,
            // so no copy is needed: a render pass may sample a texture
            // it is not writing.
            let mut src = SRC_TARGET;
            for (i, b) in sp.blur.iter().enumerate() {
                let dst = if i % 2 == 0 { 1usize } else { 0usize };
                run(&mut enc, &self.scratch_view[dst], src, *b..*b + 1);
                src = if dst == 1 { SRC_SCRATCH_B } else { SRC_SCRATCH_A };
            }
            // The panel, reading whichever scratch the last pass wrote.
            run(&mut enc, &self.target_view, src, sp.glass..sp.glass + 1);
            at = sp.glass + 1;
        }
        run(&mut enc, &self.target_view, SRC_SCRATCH_A, at..quads.len() as u32);
        self.queue.submit([enc.finish()]);
    }

    /// Block until the device has finished everything submitted.
    ///
    /// **For benchmarks and readbacks, not for frames.** A frame loop
    /// that waits here has thrown away the one thing a GPU is for:
    /// `draw` returns as soon as the work is *queued*, and the CPU is
    /// meant to go and do the next frame's layout while the device
    /// draws. A benchmark that did not wait would be timing the queue
    /// submission and reporting it as a render.
    pub fn wait(&self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
    }

    /// Put the target on screen. A no-op when there is no window.
    pub fn present(&mut self) {
        let Some(surface) = &self.surface else { return };
        #[cfg(debug_assertions)]
        if self.drop_first > 0 {
            // The Outdated arm below, taken on purpose (LUMEN_GPU_DROP_FIRST).
            self.drop_first -= 1;
            surface.configure(&self.device, &self.config);
            return;
        }
        use wgpu::CurrentSurfaceTexture as Cst;
        let frame = match surface.get_current_texture() {
            // Suboptimal is still a frame, and refusing to draw it is
            // how a window goes black after a monitor change.
            Cst::Success(f) | Cst::Suboptimal(f) => f,
            Cst::Outdated | Cst::Lost => {
                // The window changed under us. Reconfigure and let the
                // next redraw present; dropping one frame is what every
                // other backend does here too.
                surface.configure(&self.device, &self.config);
                return;
            }
            _ => return,
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("present") });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.blit_pipeline);
            pass.set_bind_group(0, &self.blit_bind, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([enc.finish()]);
        self.queue.present(frame);
    }

    /// The target, as RGBA8 — the same layout as
    /// [`software::Canvas::pixels`], so a test can subtract one from the
    /// other.
    ///
    /// [`software::Canvas::pixels`]: lumen_static_shell::software::Canvas::pixels
    pub fn read_rgba(&mut self) -> Vec<u8> {
        let (w, h) = self.size;
        // Copies out of a texture are row-aligned to 256 bytes, so the
        // staging buffer is padded and unpadded here rather than at
        // every call site.
        let unpadded = w as usize * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
        let padded = unpadded.div_ceil(align) * align;

        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded * h as usize) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("readback") });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded as u32),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.queue.submit([enc.finish()]);

        let slice = staging.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        // Blocking is the point: a readback is a test and a screenshot,
        // never a frame.
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        let mut out = Vec::with_capacity(unpadded * h as usize);
        if let Ok(data) = slice.get_mapped_range() {
            for row in 0..h as usize {
                out.extend_from_slice(&data[row * padded..row * padded + unpadded]);
            }
        }
        staging.unmap();
        out
    }
}

/// A full redraw of `list` into the target, leaving whatever was there
/// underneath — the same contract [`software::Canvas`] gives, so the two
/// are interchangeable at a `&mut dyn Renderer`.
///
/// [`software::Canvas`]: lumen_static_shell::software::Canvas
impl Renderer for Gpu {
    fn render(&mut self, list: &DisplayList, width: u32, height: u32) {
        self.resize(width, height);
        self.draw(list, None, None);
    }
}

/// A plain shape instance.
fn shape(rect: Rect, clip: Rect, color: Color, radius: f32, kind: u32) -> Quad {
    Quad {
        rect,
        clip,
        color: linear(color),
        params: [radius, 0.0, 0.0, 0.0],
        uv: [0.0; 4],
        kind,
        _pad: [0; 3],
    }
}

/// The fontless stand-in: one filled block per glyph at the advance
/// `BlockMetrics` assumes, so what is drawn matches what was measured.
/// Transcribed from the software canvas so a fontless comparison of the
/// two backends is still a comparison.
fn blocks(rect: &Rect, s: &str, size: f32, color: Color, clip: &[Rect], out: &mut Vec<Quad>) {
    let advance = size * 0.5;
    for (k, ch) in s.chars().enumerate() {
        if ch == ' ' {
            continue;
        }
        let x = rect[0] + k as f32 * advance;
        if x + advance > rect[2] {
            break; // clipped by the node's own box
        }
        let g = [x + 0.5, rect[1] + 2.0, x + advance - 0.5, rect[1] + size - 2.0];
        if let Some(r) = clipped(g, clip) {
            out.push(shape(g, r, color, 0.0, KIND_ROUNDED));
        }
    }
}

fn grad_of(s: Stops) -> Grad {
    let mut color = [[0.0f32; 4]; 4];
    for (k, c) in s.color.iter().enumerate() {
        color[k] = linear(*c);
    }
    Grad { color, at: s.at, len: s.len as u32, _pad: [0; 3] }
}

/// One glass material, out of line. Mirrors `Mat` in `shader.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Mat {
    tint: [f32; 4],
    tint2: [f32; 4],
    prim: [f32; 4],
    p0: [f32; 4],
    p1: [f32; 4],
    p2: [f32; 4],
    p3: [f32; 4],
}

fn make_mat_buffer(device: &wgpu::Device, cap: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("lumen-static materials"),
        size: (cap * core::mem::size_of::<Mat>()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// The two ping-ponged scratch targets the blur passes write.
fn make_scratch(
    device: &wgpu::Device,
    w: u32,
    h: u32,
) -> ([wgpu::Texture; 2], [wgpu::TextureView; 2]) {
    let mk = |n: &'static str| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(n),
            size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TARGET_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    };
    let a = mk("lumen-static scratch a");
    let b = mk("lumen-static scratch b");
    let av = a.create_view(&wgpu::TextureViewDescriptor::default());
    let bv = b.create_view(&wgpu::TextureViewDescriptor::default());
    ([a, b], [av, bv])
}

/// Group 1, for each thing a pass can read: the target, then the two
/// scratch textures.
fn make_src_binds(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    target: &wgpu::TextureView,
    scratch: &[wgpu::TextureView; 2],
    samp: &wgpu::Sampler,
) -> [wgpu::BindGroup; 3] {
    let one = |v: &wgpu::TextureView| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lumen-static glass source"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(v) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(samp) },
            ],
        })
    };
    [one(target), one(&scratch[0]), one(&scratch[1])]
}

fn make_target(device: &wgpu::Device, w: u32, h: u32) -> (wgpu::Texture, wgpu::TextureView) {
    let t = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("lumen-static target"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TARGET_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let v = t.create_view(&wgpu::TextureViewDescriptor::default());
    (t, v)
}

fn make_atlas_texture(
    device: &wgpu::Device,
    size: u32,
    format: wgpu::TextureFormat,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("lumen-static atlas"),
        size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn write_texture(
    queue: &wgpu::Queue,
    tex: &wgpu::Texture,
    slot: atlas::Slot,
    data: &[u8],
    bpp: u32,
) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: 0,
            origin: wgpu::Origin3d { x: slot.x, y: slot.y, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        &data[..(slot.w * slot.h * bpp) as usize],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(slot.w * bpp),
            rows_per_image: Some(slot.h),
        },
        wgpu::Extent3d { width: slot.w, height: slot.h, depth_or_array_layers: 1 },
    );
}

fn make_quad_buffer(device: &wgpu::Device, cap: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("lumen-static quads"),
        size: (cap * core::mem::size_of::<Quad>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn make_grad_buffer(device: &wgpu::Device, cap: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("lumen-static gradients"),
        size: (cap.max(1) * core::mem::size_of::<Grad>()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// A configuration for the headless case, where there is no surface to
/// ask. Only its `format` is ever read (by the blit pipeline), but the
/// field exists so `present` has nothing to special-case.
fn default_config(width: u32, height: u32) -> wgpu::SurfaceConfiguration {
    wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: TARGET_FORMAT,
        color_space: wgpu::SurfaceColorSpace::Auto,
        width,
        height,
        present_mode: wgpu::PresentMode::AutoVsync,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    }
}

fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn storage_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn make_bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    globals: &wgpu::Buffer,
    glyph: &wgpu::Texture,
    image: &wgpu::Texture,
    sampler: &wgpu::Sampler,
    grads: &wgpu::Buffer,
    mats: &wgpu::Buffer,
) -> wgpu::BindGroup {
    let gv = glyph.create_view(&wgpu::TextureViewDescriptor::default());
    let iv = image.create_view(&wgpu::TextureViewDescriptor::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("lumen-static binds"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&gv) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&iv) },
            wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(sampler) },
            wgpu::BindGroupEntry { binding: 4, resource: grads.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 5, resource: mats.as_entire_binding() },
        ],
    })
}

fn make_blit_bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("lumen-static blit binds"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    })
}
