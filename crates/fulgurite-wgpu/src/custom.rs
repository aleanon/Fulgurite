//! Custom shaders on the GPU — M83/T83.5 (`docs/plan-gpu-extension.md`,
//! "The GPU pass split and its scissor", "Invalid WGSL never takes the
//! window down"), behind this crate's `shader` feature.
//!
//! **Compiling.** [`Gpu::set_shaders`] turns a program's
//! [`ShaderTable`] into one pipeline per entry: `src/prelude.wgsl`
//! followed by the entry's WGSL, vertex stage `fulgurite_vs`, fragment stage
//! `fs_main`, the shapes pipeline's blend. Each pipeline has its **own**
//! bind group layout, never an automatic one: binding 0 is the prelude's
//! `FulguriteFrame` and binding 1, when `uniform_size > 0`, the user's
//! uniforms with `min_binding_size = uniform_size`. That size is what
//! moves wgpu's struct-size check into pipeline creation — without it the
//! check is deferred to encoding (`LateMinBufferBindingSizeMismatch`),
//! outside any error scope, where wgpu's default handler panics.
//!
//! **Inside a validation error scope.** Every module, layout and pipeline
//! is created under `Device::push_error_scope`, and the scope is popped
//! before the next entry. Unscoped, an invalid WGSL source reaches wgpu's
//! uncaptured-error handler, which panics; the window has no
//! `catch_unwind` there, and M36's guard covers only application code
//! inside the session. A refused entry prints one stderr line naming the
//! shader and wgpu's message, is counted in the answer, and its brackets
//! draw their fallback — which is a correct UI, so no panic screen hides
//! a working application over a cosmetic failure.
//!
//! **Content-keyed.** A compiled pipeline is found again by a hash of
//! its full source and its uniform size, never by its `ShaderId`: a hot
//! swap re-sends the table with an edited source under the same id, and
//! a cache keyed by id would keep drawing the old shader. Entries no
//! table names any more are dropped on the next call.
//!
//! **Drawing** is `collect`'s and `submit`'s (`lib.rs`): a known bracket
//! with the right uniform length becomes a [`Split::Shader`](crate::Split)
//! and its fallback is skipped; anything else draws the fallback, clipped
//! to the rect, as the software canvas does.

use fulgurite::display::ShaderId;
use fulgurite_shell::shader::ShaderTable;

use crate::{clipped, snap_out, Gpu, Rect, Split, TARGET_FORMAT};

/// The prelude every custom shader is compiled after.
const PRELUDE: &str = include_str!("prelude.wgsl");

/// One uniform slot: the most a bracket records
/// ([`MAX_UNIFORMS`](fulgurite::MAX_UNIFORMS)), and the largest
/// minimum uniform-buffer offset alignment WebGPU allows, so every device
/// accepts a dynamic offset that is a multiple of it.
const SLOT: usize = 256;

/// The uniform buffer's first size, in bytes: four brackets' frame and
/// uniform slots. It doubles as a frame needs more, like `grads`.
const FIRST_CAP: u64 = 4 * 2 * SLOT as u64;

/// What `@group(0) @binding(0)` holds for one bracket. Mirrors
/// `FulguriteFrame` in `src/prelude.wgsl`, 32 B with its pad.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FulguriteFrame {
    rect: [f32; 4],
    viewport: [f32; 2],
    scale: f32,
    _pad: f32,
}

const FRAME_SIZE: u64 = core::mem::size_of::<FulguriteFrame>() as u64;

/// One compiled shader.
pub(crate) struct Pipe {
    /// [`content_key`] of its source and uniform size.
    key: u64,
    uniform_size: u16,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    /// Its group 0 over the current uniform buffer; rebuilt when the
    /// buffer grows.
    bind: wgpu::BindGroup,
}

/// What custom shaders have done since this renderer was made, read with
/// [`Gpu::shader_stats`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShaderStats {
    /// Shader passes submitted: one per drawn bracket per damage region.
    pub passes: u64,
    /// Brackets whose uniform bytes were not their shader's
    /// `uniform_size`, and drew their fallback instead (per bracket per
    /// region per build of the frame, so an atlas retry counts again).
    pub mismatches: u64,
}

/// [`Gpu`]'s custom-shader state. Empty, with no buffer and no pipeline,
/// until the first table that compiles anything.
#[derive(Default)]
pub(crate) struct Shaders {
    pipes: Vec<Pipe>,
    /// `ShaderId` → index into `pipes`; `None` for a refused entry.
    ids: Vec<Option<usize>>,
    /// Frame and uniform slots for every bracket of a frame, written once
    /// per frame.
    buf: Option<wgpu::Buffer>,
    cap: u64,
    /// The lines the last [`Gpu::set_shaders`] printed, one per refusal.
    refused: Vec<String>,
    stats: ShaderStats,
}

/// FNV-1a over the full source (prelude included, so a fault-switched
/// source is a different key) and the uniform size.
fn content_key(src: &str, uniform_size: u16) -> u64 {
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in src.bytes().chain(uniform_size.to_le_bytes()) {
        h = (h ^ b as u64).wrapping_mul(PRIME);
    }
    h
}

fn round_up(n: usize) -> usize {
    n.div_ceil(SLOT) * SLOT
}

impl Gpu {
    /// Compile a program's shader table, and answer how many entries were
    /// **refused** — M83/T83.5 (the module docs).
    ///
    /// Entry `n` is `ShaderId(n)`. A refused entry — WGSL that does not
    /// compile, a missing `fs_main`, a uniform struct larger than its
    /// `uniform_size` — prints one stderr line naming it and wgpu's
    /// message, and its brackets draw their fallback. Nothing panics: every
    /// creation runs inside a validation error scope.
    ///
    /// **No borrow survives the call** (`fulgurite_shell::shader`):
    /// each source is copied into the module it compiles, and the table is
    /// not kept. A pipeline already compiled from the same source and
    /// uniform size is reused, whatever id it had; one no entry names any
    /// more is dropped.
    ///
    /// **`FULGURITE_GPU_BAD_SHADER=1`, debug builds only**, appends a syntax
    /// error to every source, so a live leg can see a refusal — the
    /// `FULGURITE_GPU_DROP_FIRST` pattern.
    pub fn set_shaders(&mut self, table: ShaderTable<'_>) -> usize {
        let bad = cfg!(debug_assertions) && std::env::var("FULGURITE_GPU_BAD_SHADER").is_ok_and(|v| v == "1");
        let mut old: Vec<Option<Pipe>> = core::mem::take(&mut self.shaders.pipes).into_iter().map(Some).collect();
        self.shaders.ids.clear();
        self.shaders.refused.clear();
        let mut refused = 0;
        for d in table.descs() {
            let mut src = String::with_capacity(PRELUDE.len() + d.wgsl().len() + 64);
            src.push_str(PRELUDE);
            src.push('\n');
            src.push_str(d.wgsl());
            if bad {
                src.push_str("\n@@ FULGURITE_GPU_BAD_SHADER @@\n");
            }
            let key = content_key(&src, d.uniform_size());
            if let Some(i) = self.shaders.pipes.iter().position(|p| p.key == key) {
                self.shaders.ids.push(Some(i));
                continue;
            }
            if let Some(p) = old.iter_mut().find(|p| p.as_ref().is_some_and(|p| p.key == key)).and_then(Option::take) {
                self.shaders.pipes.push(p);
                self.shaders.ids.push(Some(self.shaders.pipes.len() - 1));
                continue;
            }
            match self.compile(d.name(), &src, d.uniform_size(), key) {
                Ok(p) => {
                    self.shaders.pipes.push(p);
                    self.shaders.ids.push(Some(self.shaders.pipes.len() - 1));
                }
                Err(msg) => {
                    // One line: wgpu's message spans several, with a code
                    // frame under a parse error; its text lines are kept,
                    // the frame's drawing is not.
                    let msg = msg
                        .lines()
                        .map(str::trim)
                        .filter(|l| !l.is_empty() && !l.contains('│') && !l.contains('┌'))
                        .collect::<Vec<_>>()
                        .join(" / ");
                    let line = format!(
                        "fulgurite: shader \"{}\" refused; its brackets draw their fallback: {msg}",
                        d.name()
                    );
                    eprintln!("{line}");
                    self.shaders.refused.push(line);
                    self.shaders.ids.push(None);
                    refused += 1;
                }
            }
        }
        refused
    }

    /// The lines the last [`Gpu::set_shaders`] printed, one per refused
    /// entry — for a test that must see each one name its shader.
    #[doc(hidden)]
    pub fn shader_refusals(&self) -> &[String] {
        &self.shaders.refused
    }

    /// Shader passes run and uniform-length mismatches so far.
    pub fn shader_stats(&self) -> ShaderStats {
        self.shaders.stats
    }

    /// One entry, under a validation error scope. The buffer is made here
    /// on the first entry that needs it, because its bind group names it.
    fn compile(&mut self, name: &str, src: &str, uniform_size: u16, key: u64) -> Result<Pipe, String> {
        let buf = self.shaders.buf.get_or_insert_with(|| {
            self.shaders.cap = FIRST_CAP;
            make_buffer(&self.device, FIRST_CAP)
        });
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(name),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });
        let mut entries = vec![dynamic_entry(0, FRAME_SIZE)];
        if uniform_size > 0 {
            entries.push(dynamic_entry(1, uniform_size as u64));
        }
        let layout = self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(name),
            entries: &entries,
        });
        let pipeline = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(name),
            layout: Some(&self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(name),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            })),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("fulgurite_vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: TARGET_FORMAT,
                    blend: Some(crate::SHAPES_BLEND),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let bind = make_bind(&self.device, &layout, buf, uniform_size);
        match pollster::block_on(scope.pop()) {
            None => Ok(Pipe { key, uniform_size, pipeline, layout, bind }),
            Some(e) => Err(e.to_string()),
        }
    }

    /// `collect`'s half of a bracket: whether `id` runs here. When it does,
    /// a [`Split::Shader`] is recorded at instance `at` with its scissor
    /// and slots, and the caller skips the fallback; when it does not, the
    /// caller draws the fallback clipped to `rect`.
    ///
    /// **The scissor is `snap_out(clipped(rect, clip))`**: `clip` is the
    /// damage region followed by every open `PushClip`, so it is rect ∩
    /// clips ∩ region, in whole pixels by the convention every region
    /// follows. A bracket the clips leave nothing of is still drawn by the
    /// pipeline — as nothing — and its fallback still skipped.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn shader_split(
        &mut self,
        list: &fulgurite::display::DisplayList,
        rect: Rect,
        id: ShaderId,
        range: (u32, u32),
        clip: &[Rect],
        at: u32,
        splits: &mut Vec<Split>,
        bytes: &mut Vec<u8>,
    ) -> bool {
        let Some(&Some(pipe)) = self.shaders.ids.get(id.0 as usize) else { return false };
        let size = self.shaders.pipes[pipe].uniform_size;
        if range.1 != size as u32 {
            self.shaders.stats.mismatches += 1;
            return false;
        }
        let Some(r) = clipped(rect, clip) else { return true };
        let s = snap_out(r);
        let (w, h) = (self.size.0 as f32, self.size.1 as f32);
        let s = [s[0].max(0.0), s[1].max(0.0), s[2].min(w), s[3].min(h)];
        if s[2] <= s[0] || s[3] <= s[1] {
            return true;
        }
        let frame = round_up(bytes.len());
        bytes.resize(frame, 0);
        bytes.extend_from_slice(bytemuck::bytes_of(&FulguriteFrame {
            rect,
            viewport: [w, h],
            scale: 1.0,
            _pad: 0.0,
        }));
        let mut uniforms = 0;
        if size > 0 {
            uniforms = round_up(bytes.len());
            bytes.resize(uniforms, 0);
            bytes.extend_from_slice(list.uniforms_of(range));
        }
        splits.push(Split::Shader {
            at,
            pipe: pipe as u32,
            frame: frame as u32,
            uniforms: uniforms as u32,
            scissor: [s[0] as u32, s[1] as u32, (s[2] - s[0]) as u32, (s[3] - s[1]) as u32],
        });
        true
    }

    /// `submit`'s upload: the frame's slots into the uniform buffer, grown
    /// to the next power of two first if they do not fit (every pipe's
    /// bind group names the buffer, so each is rebuilt then).
    pub(crate) fn upload_shader_slots(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let need = bytes.len() as u64;
        if need > self.shaders.cap || self.shaders.buf.is_none() {
            let cap = need.next_power_of_two().max(FIRST_CAP);
            let buf = make_buffer(&self.device, cap);
            for p in &mut self.shaders.pipes {
                p.bind = make_bind(&self.device, &p.layout, &buf, p.uniform_size);
            }
            self.shaders.buf = Some(buf);
            self.shaders.cap = cap;
        }
        if let Some(buf) = &self.shaders.buf {
            self.queue.write_buffer(buf, 0, bytes);
        }
    }

    /// The pass for one shader split: `LoadOp::Load` into the target, the
    /// user pipeline, the bracket's slots as dynamic offsets, its scissor,
    /// and a full-target triangle — so the scissor alone bounds what it
    /// writes.
    pub(crate) fn shader_pass(&self, enc: &mut wgpu::CommandEncoder, pipe: u32, frame: u32, uniforms: u32, scissor: [u32; 4]) {
        let p = &self.shaders.pipes[pipe as usize];
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("custom shader"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.target_view,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&p.pipeline);
        let offsets = [frame, uniforms];
        let n = if p.uniform_size > 0 { 2 } else { 1 };
        pass.set_bind_group(0, &p.bind, &offsets[..n]);
        pass.set_scissor_rect(scissor[0], scissor[1], scissor[2], scissor[3]);
        pass.draw(0..3, 0..1);
    }

    /// Count the passes a frame is about to submit.
    pub(crate) fn count_shader_passes(&mut self, n: u64) {
        self.shaders.stats.passes += n;
    }
}

fn make_buffer(device: &wgpu::Device, cap: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("fulgurite custom shader slots"),
        size: cap,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// A uniform binding read at a dynamic offset, `min_binding_size` set so
/// the shader's struct is checked against it at pipeline creation.
fn dynamic_entry(binding: u32, size: u64) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: true,
            min_binding_size: wgpu::BufferSize::new(size),
        },
        count: None,
    }
}

/// Group 0 over `buf`: the frame's 32 B, and exactly `uniform_size`
/// bytes of the uniform slot — not the 256-byte slot.
fn make_bind(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, buf: &wgpu::Buffer, uniform_size: u16) -> wgpu::BindGroup {
    let frame = wgpu::BindGroupEntry {
        binding: 0,
        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer: buf,
            offset: 0,
            size: wgpu::BufferSize::new(FRAME_SIZE),
        }),
    };
    let unif = wgpu::BindGroupEntry {
        binding: 1,
        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer: buf,
            offset: 0,
            size: wgpu::BufferSize::new(uniform_size as u64),
        }),
    };
    let both = [frame, unif];
    let n = if uniform_size > 0 { 2 } else { 1 };
    device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("fulgurite custom shader"), layout, entries: &both[..n] })
}
