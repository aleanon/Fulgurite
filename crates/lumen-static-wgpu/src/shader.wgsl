// The whole command vocabulary, as one pipeline.
//
// Every `Cmd` becomes one instanced quad with a `kind` discriminant and
// a signed-distance field for its coverage. That is not a shortcut: the
// software canvas already draws its rounded rects and its shadows from
// the *same* distance functions (`software.rs`
// `fill_rounded_within`, `shadow_within`, `stroke_line`), so the two
// backends agree by construction rather than by coincidence, and
// `tests/parity.rs` can hold them to a stated tolerance.
//
// One pipeline and one draw call per frame is also what keeps paint
// order correct. Instances rasterise in submission order, so the
// blended result is the display list's order — the same property the
// software renderer gets from walking a `Vec`. Batching by kind would
// be faster and would put a popup's text under its own background.

struct Globals {
    viewport: vec2<f32>,
    _pad: vec2<f32>,
};

// Four colour stops, out of line: they are 96 bytes and almost no frame
// has a gradient in it, so paying for them per instance would make
// every rect 60% bigger to describe something it is not.
struct Grad {
    color: array<vec4<f32>, 4>,
    at: vec4<f32>,
    len: u32,
    _pad: vec3<u32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var glyph_tex: texture_2d<f32>;
@group(0) @binding(2) var image_tex: texture_2d<f32>;
@group(0) @binding(3) var samp: sampler;
@group(0) @binding(4) var<storage, read> grads: array<Grad>;

const KIND_ROUNDED: u32 = 0u;
const KIND_SHADOW:  u32 = 1u;
const KIND_LINE:    u32 = 2u;
const KIND_GRADIENT:u32 = 3u;
const KIND_GLYPH:   u32 = 4u;
const KIND_IMAGE:   u32 = 5u;

struct Instance {
    // The shape, in physical pixels. For a shadow this is the *reach*
    // (the box plus its spread) and `uv` carries the box itself.
    @location(0) rect: vec4<f32>,
    // The clip stack, already intersected on the CPU — the same rect
    // `software::clipped` computes, arrived at by the same walk.
    @location(1) clip: vec4<f32>,
    @location(2) color: vec4<f32>,
    // (corner radius, shadow spread / line half-width, unused, gradient index)
    @location(3) params: vec4<f32>,
    // Atlas uv rect for a glyph or an image; the shadow's box; the
    // line's two endpoints; the gradient's axis and span.
    @location(4) uv: vec4<f32>,
    @location(5) kind: u32,
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) p: vec2<f32>,
    @location(1) rect: vec4<f32>,
    @location(2) clip: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) params: vec4<f32>,
    @location(5) uv: vec4<f32>,
    @location(6) @interpolate(flat) kind: u32,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, inst: Instance) -> VsOut {
    // A one-pixel skirt, so an antialiased edge has somewhere to ramp.
    // Not for glyphs and images: their quad *is* their uv mapping, and
    // growing it would smear the atlas lookup by a pixel.
    var pad = 0.0;
    if (inst.kind <= KIND_GRADIENT) {
        pad = 1.0;
    }
    let lo = inst.rect.xy - vec2(pad);
    let hi = inst.rect.zw + vec2(pad);

    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let p = mix(lo, hi, corner);

    var out: VsOut;
    // Pixels to clip space: y down to y up.
    out.pos = vec4(p / globals.viewport * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0);
    out.p = p;
    out.rect = inst.rect;
    out.clip = inst.clip;
    out.color = inst.color;
    out.params = inst.params;
    out.uv = inst.uv;
    out.kind = inst.kind;
    return out;
}

/// Signed distance to a rounded box (Quilez) — the function
/// `software::Canvas::shadow_within` uses, transcribed.
fn sd_round_box(p: vec2<f32>, r: vec4<f32>, rad: f32) -> f32 {
    let c = (r.xy + r.zw) * 0.5;
    let h = (r.zw - r.xy) * 0.5 - vec2(rad);
    let q = abs(p - c) - h;
    return length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - rad;
}

/// Distance to a *segment*, not to the infinite line: a line must end
/// where it ends. `software::Canvas::stroke_line` again.
fn sd_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let d = b - a;
    let len2 = dot(d, d);
    var t = 0.0;
    if (len2 > 0.0) {
        t = clamp(dot(p - a, d) / len2, 0.0, 1.0);
    }
    return distance(p, a + d * t);
}

/// `Stops::sample`, in the shader. Kept literal rather than clever so a
/// reader can put the two side by side.
fn grad_sample(g: u32, t_in: f32) -> vec4<f32> {
    let n = min(grads[g].len, 4u);
    if (n == 0u) {
        return vec4(0.0);
    }
    let t = clamp(t_in, 0.0, 1.0);
    if (t <= grads[g].at[0]) {
        return grads[g].color[0];
    }
    for (var k = 1u; k < n; k = k + 1u) {
        if (t <= grads[g].at[k]) {
            let span = grads[g].at[k] - grads[g].at[k - 1u];
            var f = 0.0;
            if (span > 0.0) {
                f = (t - grads[g].at[k - 1u]) / span;
            }
            return mix(grads[g].color[k - 1u], grads[g].color[k], f);
        }
    }
    return grads[g].color[n - 1u];
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let p = in.p;

    // The clip is a **hard** test, never antialiased. That is what makes
    // a partial redraw pixel-identical to a full one: a pixel on the
    // damage boundary is either wholly redrawn or wholly left alone, and
    // a half-covered boundary pixel would blend new ink onto old and
    // differ from the full render. `tests/damage.rs`.
    if (p.x < in.clip.x || p.x > in.clip.z || p.y < in.clip.y || p.y > in.clip.w) {
        discard;
    }

    var col = in.color;
    var cov = 1.0;

    switch in.kind {
        case KIND_ROUNDED: {
            // `rad + 0.5 - distance`, which is `0.5 - signed distance`.
            cov = clamp(0.5 - sd_round_box(p, in.rect, in.params.x), 0.0, 1.0);
        }
        case KIND_SHADOW: {
            let d = sd_round_box(p, in.uv, in.params.x);
            // Pixels well inside the shape are skipped: whatever cast
            // the shadow paints over them next.
            if (d < -1.0) {
                discard;
            }
            let t = 1.0 - clamp(d / in.params.y, 0.0, 1.0);
            cov = t * t;
        }
        case KIND_LINE: {
            let hw = max(in.params.y, 0.5);
            cov = clamp(hw + 0.5 - sd_segment(p, in.uv.xy, in.uv.zw), 0.0, 1.0);
        }
        case KIND_GRADIENT: {
            // Hard-edged, like the software fill: a gradient is a fill,
            // not a shape.
            if (p.x < in.rect.x || p.x > in.rect.z || p.y < in.rect.y || p.y > in.rect.w) {
                discard;
            }
            let c = (in.rect.xy + in.rect.zw) * 0.5;
            let t = dot(p - c, in.uv.xy) / in.uv.z + 0.5;
            col = grad_sample(u32(in.params.w), t);
        }
        case KIND_GLYPH: {
            let uv = mix(in.uv.xy, in.uv.zw, (p - in.rect.xy) / (in.rect.zw - in.rect.xy));
            cov = textureSampleLevel(glyph_tex, samp, uv, 0.0).r;
        }
        case KIND_IMAGE: {
            let uv = mix(in.uv.xy, in.uv.zw, (p - in.rect.xy) / (in.rect.zw - in.rect.xy));
            // Tint multiplies, so white is untinted and one monochrome
            // asset can serve several colours.
            col = textureSampleLevel(image_tex, samp, uv, 0.0) * in.color;
        }
        default: {}
    }

    if (cov <= 0.0 || col.a <= 0.0) {
        discard;
    }
    return vec4(col.rgb, col.a * cov);
}
