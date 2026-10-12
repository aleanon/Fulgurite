//! M83/T83.5's gates: a custom shader's pass on the GPU
//! (`docs/plan-gpu-extension.md`, T83.5; `docs/results-shader.md` owns
//! every number printed here).
//!
//! A headless `Gpu`, 300 × 300. Each test **skips** without an adapter
//! under a plain `cargo test`; `just gpu` sets FULGURITE_REQUIRE_GPU=1, under
//! which a missing adapter fails (M43/T43.10).
//!
//! The shader under test writes its uniform colour, and every bracket's
//! fallback is a fill of a **different** colour, so "the pipeline ran and
//! the fallback was skipped" is a pixel count, not an inference: the
//! scissor, clip and region gates each assert their setup that way before
//! they assert the bound.
#![cfg(feature = "shader")]

use fulgurite::display::{Color, DisplayList, Material, Renderer, ShaderId};
use fulgurite_shell::shader::{ShaderDesc, ShaderTable};
use fulgurite_shell::software::Canvas;
use fulgurite_wgpu::Gpu;

const W: u32 = 300;
const H: u32 = 300;
const PAGE: Color = Color(250, 250, 250, 255);
/// What every fallback here paints: never a shader's colour.
const FALLBACK: Color = Color(90, 90, 90, 255);
const RED: Color = Color(220, 30, 40, 255);
const BLUE: Color = Color(30, 60, 220, 255);
const RECT: [f32; 4] = [50.0, 50.0, 150.0, 150.0];

/// Writes its uniform colour: 16 bytes, one `vec4<f32>`.
const SOLID: &str = "
struct U { color: vec4<f32> };
@group(0) @binding(1) var<uniform> u: U;
@fragment fn fs_main(in: FulguriteVarying) -> @location(0) vec4<f32> { return u.color; }
";
const TABLE: [ShaderDesc; 1] = [ShaderDesc::new("solid", SOLID, 16)];

fn required() -> bool {
    std::env::var("FULGURITE_REQUIRE_GPU").is_ok_and(|v| v == "1")
}

fn gpu() -> Option<Gpu> {
    match Gpu::headless(W, H) {
        Some(g) => {
            eprintln!("gpu: {}", g.adapter());
            Some(g)
        }
        None => {
            assert!(!required(), "FULGURITE_REQUIRE_GPU=1 but no adapter");
            eprintln!("gpu: no adapter -- skipping");
            None
        }
    }
}

/// `c` as the shader's 16 uniform bytes: four `f32`s in 0..1, the
/// `linear(Color)` convention.
fn uniforms(c: Color) -> [u8; 16] {
    let mut b = [0u8; 16];
    for (k, v) in [c.0, c.1, c.2, c.3].into_iter().enumerate() {
        b[k * 4..k * 4 + 4].copy_from_slice(&(v as f32 / 255.0).to_le_bytes());
    }
    b
}

/// A bracket over `rect` running shader `id` with `bytes`, whose
/// fallback fills the rect with [`FALLBACK`].
fn bracket(l: &mut DisplayList, rect: [f32; 4], id: u16, bytes: &[u8]) {
    l.shader(rect, ShaderId(id), bytes);
    l.rect(rect, FALLBACK);
    l.shader_end();
}

fn is(px: &[u8], i: usize, c: Color) -> bool {
    (0..3).all(|k| px[i + k].abs_diff([c.0, c.1, c.2][k]) <= 1)
}

/// Pixels of colour `c` inside and outside `r` (whole pixels).
fn count(px: &[u8], r: [f32; 4], c: Color) -> (usize, usize) {
    let (mut inside, mut outside) = (0, 0);
    for y in 0..H as usize {
        for x in 0..W as usize {
            let i = (y * W as usize + x) * 4;
            if !is(px, i, c) {
                continue;
            }
            let within = x as f32 >= r[0] && (x as f32) < r[2] && y as f32 >= r[1] && (y as f32) < r[3];
            if within {
                inside += 1;
            } else {
                outside += 1;
            }
        }
    }
    (inside, outside)
}

/// Pixels outside `r` that are not the page.
fn ink_outside(px: &[u8], r: [f32; 4]) -> usize {
    let mut n = 0;
    for y in 0..H as usize {
        for x in 0..W as usize {
            let within = x as f32 >= r[0] && (x as f32) < r[2] && y as f32 >= r[1] && (y as f32) < r[3];
            if !within && !is(px, (y * W as usize + x) * 4, PAGE) {
                n += 1;
            }
        }
    }
    n
}

fn compiled(g: &mut Gpu, table: &[ShaderDesc]) {
    let refused = g.set_shaders(ShaderTable::new(table));
    assert_eq!(refused, 0, "the test's own shader was refused: {:?}", g.shader_refusals());
}

/// **Scissor.** The pass draws a full-target triangle, so the scissor
/// alone bounds it. Injection: skip `set_scissor_rect`, and the outside
/// count reads the whole target less the rect.
#[test]
fn the_scissor_alone_bounds_the_pass() {
    let Some(mut g) = gpu() else { return };
    compiled(&mut g, &TABLE);
    let mut l = DisplayList::new();
    bracket(&mut l, RECT, 0, &uniforms(RED));
    g.clear(PAGE);
    g.render(&l, W, H);
    let px = g.read_rgba();
    let (inside, _) = count(&px, RECT, RED);
    let (fallback, _) = count(&px, RECT, FALLBACK);
    let outside = ink_outside(&px, RECT);
    println!(
        "scissor: shader colour inside {inside} (setup 10000), fallback pixels {fallback} (setup 0), \
         ink outside the rect {outside} (bound 0); passes {}",
        g.shader_stats().passes
    );
    assert_eq!(inside, 10_000, "the pipeline did not cover its rect -- the setup did not happen");
    assert_eq!(fallback, 0, "the fallback was drawn under or instead of the pipeline");
    assert_eq!(outside, 0, "the shader wrote {outside} px outside its rect");
}

/// **Clip.** The bracket inside a narrower `PushClip`: the scissor is
/// rect ∩ the clip stack. Injection: the scissor from `rect ∩ region`
/// without the open clips, and the outside count reads the cut half.
#[test]
fn the_scissor_includes_the_clip_stack() {
    let Some(mut g) = gpu() else { return };
    compiled(&mut g, &TABLE);
    let clip = [50.0, 50.0, 100.0, 150.0];
    let mut l = DisplayList::new();
    l.push_clip(clip);
    bracket(&mut l, RECT, 0, &uniforms(RED));
    l.pop_clip();
    g.clear(PAGE);
    g.render(&l, W, H);
    let px = g.read_rgba();
    let (inside, _) = count(&px, clip, RED);
    let outside = ink_outside(&px, clip);
    println!("clip: shader colour inside the clip {inside} (setup 5000), ink outside the clip {outside} (bound 0)");
    assert_eq!(inside, 5_000, "the pipeline did not fill the clipped rect -- the setup did not happen");
    assert_eq!(outside, 0, "the shader wrote {outside} px outside its clip");
}

/// **Region.** Frame 1 in colour A over the whole target; frame 2 in
/// colour B, drawn only into a region covering the rect's left half. B
/// must land inside the region and nowhere else. Injection: the scissor
/// ignores the region, and B outside it reads the right half.
#[test]
fn the_scissor_includes_the_damage_region() {
    let Some(mut g) = gpu() else { return };
    compiled(&mut g, &TABLE);
    let mut a = DisplayList::new();
    bracket(&mut a, RECT, 0, &uniforms(RED));
    g.clear(PAGE);
    g.render(&a, W, H);
    let region = [50.0, 50.0, 100.0, 150.0];
    let mut b = DisplayList::new();
    bracket(&mut b, RECT, 0, &uniforms(BLUE));
    g.draw_regions(&b, &[region], Some(PAGE));
    let px = g.read_rgba();
    let (b_in, b_out) = count(&px, region, BLUE);
    let (_, a_kept) = count(&px, region, RED);
    println!(
        "region: B inside the region {b_in} (setup 5000), B outside it {b_out} (bound 0), \
         A kept outside it {a_kept} (5000)"
    );
    assert_eq!(b_in, 5_000, "frame 2 did not draw its region -- the setup did not happen");
    assert_eq!(b_out, 0, "frame 2's shader wrote {b_out} px outside its damage region");
    assert_eq!(a_kept, 5_000, "frame 2 disturbed frame 1 outside its region");
}

/// **Refusal.** WGSL that does not parse, and a uniform struct larger
/// than its `uniform_size`: both refused inside the error scope, each
/// message naming its shader, and the frame then equals the software
/// canvas's fallback pixels exactly.
///
/// Injections: remove the error scope, and the process panics through
/// wgpu's uncaptured-error handler ("wgpu error:"); give binding 1
/// `min_binding_size: None`, and the oversize struct is no longer refused
/// at pipeline creation (the count reads 1, or the draw panics with
/// `LateMinBufferBindingSizeMismatch`).
#[test]
fn a_refused_shader_draws_its_fallback() {
    let Some(mut g) = gpu() else { return };
    const OVERSIZE: &str = "
struct U { a: vec4<f32>, b: vec4<f32> };
@group(0) @binding(1) var<uniform> u: U;
@fragment fn fs_main(in: FulguriteVarying) -> @location(0) vec4<f32> { return u.a + u.b; }
";
    let table = [ShaderDesc::new("broken", "this is not wgsl", 0), ShaderDesc::new("oversize", OVERSIZE, 16)];
    let refused = g.set_shaders(ShaderTable::new(&table));
    let lines = g.shader_refusals().to_vec();
    println!("refusal: set_shaders answered {refused} (bound 2)");
    for line in &lines {
        println!("  {line}");
    }
    assert_eq!(refused, 2, "both shaders should have been refused: {lines:?}");
    for name in ["\"broken\"", "\"oversize\""] {
        assert!(lines.iter().any(|l| l.contains(name)), "no refusal names {name}: {lines:?}");
    }

    let mut l = DisplayList::new();
    bracket(&mut l, [20.0, 20.0, 120.0, 120.0], 0, &[]);
    bracket(&mut l, [160.0, 160.0, 260.0, 260.0], 1, &uniforms(RED));
    g.clear(PAGE);
    g.render(&l, W, H);
    let gp = g.read_rgba();
    let mut c = Canvas::new(W, H);
    c.clear(PAGE);
    c.render(&l, W, H);
    let differ = gp.chunks_exact(4).zip(c.pixels.chunks_exact(4)).filter(|(a, b)| (0..3).any(|k| a[k] != b[k])).count();
    let (fb, _) = count(&gp, [0.0, 0.0, W as f32, H as f32], FALLBACK);
    println!("refusal: fallback pixels {fb} (setup 20000), {differ} px differ from the canvas (bound 0)");
    assert_eq!(fb, 20_000, "the fallbacks were not drawn");
    assert_eq!(differ, 0, "a refused shader's bracket differs from the canvas's fallback");
}

/// **Uniform length.** A bracket whose bytes are not its shader's
/// `uniform_size` draws its fallback and is counted.
#[test]
fn a_uniform_length_mismatch_draws_the_fallback_and_is_counted() {
    let Some(mut g) = gpu() else { return };
    compiled(&mut g, &TABLE);
    let mut l = DisplayList::new();
    bracket(&mut l, RECT, 0, &uniforms(RED)[..8]);
    g.clear(PAGE);
    g.render(&l, W, H);
    let px = g.read_rgba();
    let (fb, _) = count(&px, RECT, FALLBACK);
    let (red, _) = count(&px, RECT, RED);
    let s = g.shader_stats();
    println!("mismatch: fallback {fb} (10000), shader colour {red} (0), mismatches {} (1), passes {} (0)", s.mismatches, s.passes);
    assert_eq!((fb, red, s.mismatches, s.passes), (10_000, 0, 1, 0));
    // Accept: the right length runs the pipeline on the same renderer.
    let mut ok = DisplayList::new();
    bracket(&mut ok, RECT, 0, &uniforms(RED));
    g.render(&ok, W, H);
    let (red, _) = count(&g.read_rgba(), RECT, RED);
    println!("mismatch accept: 16 bytes draw the shader colour on {red} px (10000)");
    assert_eq!(red, 10_000);
}

/// **Content key.** The same id, a new source: the pipeline cache is
/// keyed by the source, so the second table draws the second colour.
/// Injection: key the cache on `ShaderId`, and the second frame stays red.
#[test]
fn the_pipeline_cache_is_keyed_by_content() {
    let Some(mut g) = gpu() else { return };
    const R: &str = "@fragment fn fs_main(in: FulguriteVarying) -> @location(0) vec4<f32> { return vec4(220.0, 30.0, 40.0, 255.0) / 255.0; }";
    const B: &str = "@fragment fn fs_main(in: FulguriteVarying) -> @location(0) vec4<f32> { return vec4(30.0, 60.0, 220.0, 255.0) / 255.0; }";
    let mut l = DisplayList::new();
    bracket(&mut l, RECT, 0, &[]);
    let centre = |g: &mut Gpu, src: &'static str| {
        compiled(g, &[ShaderDesc::new("swapped", src, 0)]);
        g.clear(PAGE);
        g.render(&l, W, H);
        let px = g.read_rgba();
        let i = ((100 * W + 100) * 4) as usize;
        Color(px[i], px[i + 1], px[i + 2], px[i + 3])
    };
    let first = centre(&mut g, R);
    let second = centre(&mut g, B);
    println!("content key: id 0 red source -> {first:?}; id 0 blue source -> {second:?} (want {RED:?}, {BLUE:?})");
    assert_eq!(first, RED);
    assert_eq!(second, BLUE, "a new source under the same id drew the old pipeline");
}

/// **Order.** A shader, then a glass panel over it: the panel must blur
/// the shader, as the canvas's panel blurs the identical fallback.
/// Injection: shader splits run after every glass split, and the shader
/// is drawn over the panel instead of under it.
#[test]
fn a_panel_over_a_shader_blurs_the_shader() {
    let Some(mut g) = gpu() else { return };
    compiled(&mut g, &TABLE);
    let green = Color(20, 160, 60, 255);
    let panel = [100.0, 100.0, 200.0, 200.0];
    let scene = |with_shader: bool| {
        let mut l = DisplayList::new();
        if with_shader {
            // Identical in effect to its fallback: the canvas draws the
            // same colour the GPU's shader does.
            l.shader([50.0, 50.0, 250.0, 250.0], ShaderId(0), &uniforms(green));
            l.rect([50.0, 50.0, 250.0, 250.0], green);
            l.shader_end();
        }
        // A half-white tint, so a panel that tints the shader and a
        // shader drawn over the panel are far apart (LIQUID's 18/255
        // tint left them 9 levels apart, one over this bound).
        let half = Color(255, 255, 255, 128);
        l.glass(panel, 16.0, Material { tint: half, tint_over_light: half, ..Material::LIQUID });
        l
    };
    let at = |px: &[u8]| {
        let i = ((150 * W + 150) * 4) as usize;
        [px[i], px[i + 1], px[i + 2]]
    };
    let dist = |a: [u8; 3], b: [u8; 3]| (0..3).map(|k| a[k].abs_diff(b[k])).max().unwrap_or(0);
    let soft = |l: &DisplayList| {
        let mut c = Canvas::new(W, H);
        c.clear(PAGE);
        c.render(l, W, H);
        at(&c.pixels)
    };
    let truth = soft(&scene(true));
    let over_page = soft(&scene(false));
    g.clear(PAGE);
    g.render(&scene(true), W, H);
    let got = at(&g.read_rgba());
    const BOUND: u8 = 8;
    println!(
        "order: panel centre GPU {got:?}, canvas {truth:?} (distance {} , bound {BOUND}); \
         canvas panel over the page {over_page:?} (setup: distance {} > {BOUND}); passes {}",
        dist(got, truth),
        dist(truth, over_page),
        g.shader_stats().passes
    );
    assert!(dist(truth, over_page) > BOUND, "the panel over the shader looks like the panel over the page -- the scene shows nothing");
    assert!(dist(got, truth) <= BOUND, "the GPU panel did not blur the shader beneath it");
}
