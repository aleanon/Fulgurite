// The framework half of every custom shader -- M83/T83.5
// (docs/plan-gpu-extension.md, "What the user writes").
//
// `Gpu::set_shaders` compiles this text followed by the program's WGSL,
// which declares two things and nothing else is required of it:
//
//   @group(0) @binding(1) var<uniform> u: YourStruct;   // when uniform_size > 0
//   @fragment fn fs_main(in: LumenVarying) -> @location(0) vec4<f32> { ... }
//
// The pass draws one triangle over the **whole target** (`lumen_vs`, as
// `blit.wgsl` does) and is bounded by its scissor alone: the bracket's
// rect, intersected with every open clip and the damage region. So a
// fragment outside the rect never runs, and `local` outside 0..1 is never
// seen. Output is straight alpha, blended source-over like every shape,
// in the same unconverted 0..1 colour space as `linear(Color)`.

// Mirrors `LumenFrame` in `src/custom.rs`: 16 + 8 + 4 = 28 B of fields,
// 32 B with the struct's 16-byte alignment, which the Rust side's explicit
// pad matches. A WGSL struct whose size differs from its Rust mirror reads
// every later field from the wrong offset -- the `Grad` defect M82/T82.2
// found (`shader.wgsl`).
struct LumenFrame {
    // The bracket's rect, physical px: [x0, y0, x1, y1].
    rect: vec4<f32>,
    // The target's size, physical px.
    viewport: vec2<f32>,
    // The display scale the list was painted at. 1.0 on this backend:
    // the list arrives in device pixels and no backend is told the scale.
    scale: f32,
};

@group(0) @binding(0) var<uniform> lumen: LumenFrame;

struct LumenVarying {
    // The fragment's centre in target pixels, as `@builtin(position)` is.
    @builtin(position) position: vec4<f32>,
    // Where the fragment is across the bracket's rect, 0..1 on each axis.
    @location(0) local: vec2<f32>,
};

@vertex
fn lumen_vs(@builtin(vertex_index) vi: u32) -> LumenVarying {
    // One oversized triangle covering the target, `blit.wgsl`'s.
    let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    let p = uv * lumen.viewport;
    var out: LumenVarying;
    out.position = vec4(uv * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0);
    out.local = (p - lumen.rect.xy) / max(lumen.rect.zw - lumen.rect.xy, vec2(1.0e-6));
    return out;
}
