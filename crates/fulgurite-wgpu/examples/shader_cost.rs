//! What a custom shader's pass costs on the GPU — M83/T83.5, kill rule
//! K83a (`docs/plan-gpu-extension.md`; `docs/results-shader.md` owns the
//! numbers).
//!
//! `glass_cost.rs`'s method: the same display list in both arms, the
//! device waited for, best of N. At 1080p, 32 trivial 64 × 64 shader
//! brackets, each with a one-rect fallback; the shader arm compiles them,
//! the fallback arm hands the renderer an empty table, so the same
//! brackets draw their fallbacks. The difference ÷ 32 is one shader node's
//! price: its pass, its scissor and its slots. K83a's bound is 50 µs.
//!
//! `cargo run --release -p fulgurite-wgpu --features shader --example shader_cost`
use fulgurite::display::{Color, DisplayList, Renderer, ShaderId};
use fulgurite_shell::shader::{ShaderDesc, ShaderTable};
use fulgurite_wgpu::Gpu;
use std::time::Instant;

const W: u32 = 1920;
const H: u32 = 1080;
const N: usize = 32;
const PAGE: Color = Color(250, 250, 250, 255);

/// Trivial: its uniform colour, shaded by where the fragment is in its
/// rect, so the pass does a little arithmetic and cannot be folded away.
const TRIVIAL: &str = "
struct U { color: vec4<f32> };
@group(0) @binding(1) var<uniform> u: U;
@fragment fn fs_main(in: FulguriteVarying) -> @location(0) vec4<f32> {
    return vec4(u.color.rgb * (0.5 + 0.5 * in.local.x), 1.0);
}
";
const TABLE: [ShaderDesc; 1] = [ShaderDesc::new("trivial", TRIVIAL, 16)];

fn scene() -> DisplayList {
    let mut l = DisplayList::new();
    l.rect([0.0, 0.0, W as f32, H as f32], Color(0x20, 0x40, 0x90, 0xff));
    let mut bytes = [0u8; 16];
    for (k, v) in [0.9f32, 0.4, 0.2, 1.0].into_iter().enumerate() {
        bytes[k * 4..k * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    for i in 0..N {
        let (x, y) = (40.0 + (i % 8) as f32 * 220.0, 40.0 + (i / 8) as f32 * 220.0);
        let r = [x, y, x + 64.0, y + 64.0];
        l.shader(r, ShaderId(0), &bytes);
        l.rect(r, Color(230, 100, 50, 255));
        l.shader_end();
    }
    l
}

fn best_of(n: u32, mut f: impl FnMut()) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..n {
        let t = Instant::now();
        f();
        best = best.min(t.elapsed().as_secs_f64() * 1e6);
    }
    best
}

fn main() {
    let Some(mut g) = Gpu::headless(W, H) else {
        println!("no adapter — skipping");
        return;
    };
    println!("adapter: {}", g.adapter());
    let list = scene();
    let arm = |g: &mut Gpu, table: ShaderTable<'_>| {
        assert_eq!(g.set_shaders(table), 0);
        // Warm: buffers grown, pipelines compiled.
        for _ in 0..3 {
            g.render(&list, W, H);
            g.wait();
        }
        // **`wait`, or this times the queue submission** (`glass_cost.rs`).
        best_of(50, || {
            g.render(&list, W, H);
            g.wait();
        })
    };
    let before = g.shader_stats().passes;
    let shaders = arm(&mut g, ShaderTable::new(&TABLE));
    let passes = g.shader_stats().passes - before;
    let fallbacks = arm(&mut g, ShaderTable::EMPTY);
    let per = (shaders - fallbacks) / N as f64;
    println!("{N} shaders at {W}x{H}: {shaders:.1} us; their fallbacks: {fallbacks:.1} us; passes run {passes} (setup: {N} per frame)");
    println!("per shader node: {per:.2} us (K83a bound 50 us)");
    g.clear(PAGE);
}
