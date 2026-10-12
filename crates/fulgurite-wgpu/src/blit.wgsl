// Target texture to swapchain, one triangle.
//
// The GPU backend renders into a texture it owns rather than straight
// into the surface, for the reason `retain.rs` spends a page on: a
// swapchain image handed back by the presentation engine may hold the
// frame from two presents ago, or nothing. A target this crate owns
// always holds exactly the previous frame — **buffer age is 1, by
// construction** — which is what lets damage mean something on this
// path at all.
//
// The cost is one full-screen textured triangle per frame.

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VsOut {
    // One oversized triangle, not two: no seam down the diagonal and
    // one fewer vertex.
    let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    var out: VsOut;
    out.uv = uv;
    out.pos = vec4(uv * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0);
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    return textureSampleLevel(src, samp, in.uv, 0.0);
}
