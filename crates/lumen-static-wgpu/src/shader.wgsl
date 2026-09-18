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

// **The material, out of line, for the same reason `Grad` is** — M22/T22.6.
// It is 112 bytes and almost no frame has a glass panel in it, so paying
// for it per instance would make every rect more than twice the size to
// describe something it is not.
struct Mat {
    tint: vec4<f32>,
    tint2: vec4<f32>,
    // The primary blob's rect in px. For a panel with no second blob
    // this is the command's own rect.
    prim: vec4<f32>,
    // rim, refraction, specular, shadow (0..1)
    p0: vec4<f32>,
    // the light vector, and saturation as a multiplier
    p1: vec4<f32>,
    // dispersion (0..1), adapt (0..1), the morph offset
    p2: vec4<f32>,
    // kappa, morphing, the clamped corner radius, unused
    p3: vec4<f32>,
};
@group(0) @binding(5) var<storage, read> mats: array<Mat>;

// **Group 1 is the backdrop being read**, and it changes between passes
// while group 0 does not: the frame is split at every glass command so
// the captured target can be blurred, and each blur pass reads the
// texture the previous one wrote.
@group(1) @binding(0) var src_tex: texture_2d<f32>;
// **Linear**, unlike `samp`. `samp` is `Nearest` deliberately so glyphs
// stay crisp against the software arm; a refracted read lands at a
// fractional coordinate by definition, and nearest there is the
// staircase M22/T22.1 removed from the CPU.
@group(1) @binding(1) var src_samp: sampler;

const KIND_ROUNDED: u32 = 0u;
const KIND_SHADOW:  u32 = 1u;
const KIND_LINE:    u32 = 2u;
const KIND_GRADIENT:u32 = 3u;
const KIND_GLYPH:   u32 = 4u;
const KIND_IMAGE:   u32 = 5u;
// M22/T22.6.
const KIND_BLUR:    u32 = 6u;
const KIND_GLASS:   u32 = 7u;
const KIND_STROKE:  u32 = 8u;

const LIGHT_Z: f32 = 0.62;
const LIGHT_XY: f32 = 0.7845993;
/// `Material::MAX_DISPERSION`.
const MAX_DISPERSION: f32 = 0.25;

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
    if (inst.kind <= KIND_GRADIENT || inst.kind == KIND_GLASS || inst.kind == KIND_STROKE) {
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

/// `software::sign_of` — **not** WGSL's `sign`, which returns 0 at 0.
/// Zero there would collapse the normal on the axes through the panel's
/// centre, where `q` is exactly 0, and put a seam down the middle of
/// every rim.
fn sign_of(v: f32) -> f32 {
    if (v < 0.0) { return -1.0; }
    return 1.0;
}

/// Signed distance to one rounded blob and its outward unit normal —
/// `software.rs`'s `blob` closure, transcribed.
fn blob(p: vec2<f32>, ctr: vec2<f32>, h: vec2<f32>, rad: f32) -> vec3<f32> {
    let q = abs(p - ctr) - h;
    let mq = max(q, vec2(0.0));
    let outside = length(mq);
    let d = outside + min(max(q.x, q.y), 0.0) - rad;
    let sg = vec2(sign_of(p.x - ctr.x), sign_of(p.y - ctr.y));
    var n = vec2(0.0, sg.y);
    if (outside > 1.0e-4) {
        n = sg * mq / outside;
    } else if (q.x > q.y) {
        n = vec2(sg.x, 0.0);
    }
    return vec3(d, n.x, n.y);
}

/// `software::saturate`, including the per-channel headroom clamp that
/// stops a boost from rotating hue at either end.
fn saturate_c(c: vec3<f32>, s: f32) -> vec3<f32> {
    if (s == 1.0) { return c; }
    let lum = dot(c, vec3(0.299, 0.587, 0.114));
    var k = s;
    for (var i = 0u; i < 3u; i = i + 1u) {
        let d = c[i] - lum;
        if (d > 0.0) {
            if (lum + d * k > 1.0) { k = min(k, (1.0 - lum) / d); }
        } else if (d < 0.0 && lum + d * k < 0.0) {
            k = min(k, -lum / d);
        }
    }
    return vec3(lum) + (c - vec3(lum)) * k;
}

/// One backdrop tap, clamped to the captured rect the way the CPU's
/// sampler clamps to its capture — clamp-to-edge is the same rule the
/// box passes use at their own boundary.
fn tap(p: vec2<f32>, src: vec4<f32>) -> vec3<f32> {
    let q = clamp(p, src.xy + vec2(0.5), src.zw - vec2(0.5));
    return textureSampleLevel(src_tex, src_samp, q / globals.viewport, 0.0).rgb;
}

fn tap_ch(p: vec2<f32>, src: vec4<f32>, ch: u32) -> f32 {
    return tap(p, src)[ch];
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
        case KIND_STROKE: {
            // The ring between the box and the same box inset by the
            // border's width, as the difference of two coverages — which
            // antialiases the outer and inner curve in one expression and
            // keeps them concentric. `software::Canvas::rr_cov` is the
            // same subtraction over the same `sd_round_box`.
            let w = in.params.y;
            let inner = vec4<f32>(in.rect.x + w, in.rect.y + w, in.rect.z - w, in.rect.w - w);
            let ri = max(in.params.x - w, 0.0);
            let co = clamp(0.5 - sd_round_box(p, in.rect, in.params.x), 0.0, 1.0);
            let ci = clamp(0.5 - sd_round_box(p, inner, ri), 0.0, 1.0);
            cov = clamp(co - ci, 0.0, 1.0);
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
        case KIND_BLUR: {
            // One box pass — `software::box_rows` / `box_cols`,
            // normalised by the number of *contributing* taps so a box
            // hanging off the capture does not darken the rim.
            let dir = in.params.xy;
            let l = i32(in.params.z);
            let r = i32(in.params.w);
            var acc = vec3(0.0);
            var n = 0.0;
            for (var i = -l; i <= r; i = i + 1) {
                acc = acc + tap(p + dir * f32(i), in.uv);
                n = n + 1.0;
            }
            return vec4(acc / n, 1.0);
        }
        case KIND_GLASS: {
            let m = mats[u32(in.params.w)];
            let src = in.uv;
            // **The CPU's pixel range, exactly.** `glass_within` loops
            // `floor(bounds0) .. ceil(bounds2)` and inks a straight-edge
            // row whole; the quad's one-pixel skirt would otherwise
            // reach a pixel the software canvas never visits, and a hard
            // span would fill it.
            let ix = floor(p.x);
            let iy = floor(p.y);
            if (ix < floor(in.rect.x) || ix >= ceil(in.rect.z)
                || iy < floor(in.rect.y) || iy >= ceil(in.rect.w)) {
                discard;
            }

            let rr = m.p3.z;
            let ctr = (m.prim.xy + m.prim.zw) * 0.5;
            let hxy = (m.prim.zw - m.prim.xy) * 0.5 - vec2(rr);
            let rim = m.p0.x;
            let morphing = m.p3.y > 0.5;

            var d = 0.0;
            var nrm = vec2(0.0);
            var have = false;
            if (morphing) {
                let ba = blob(p, ctr, hxy, rr);
                let bb = blob(p, ctr + m.p2.zw, hxy, rr);
                let kappa = m.p3.x;
                let hb = clamp(0.5 + 0.5 * (bb.x - ba.x) / kappa, 0.0, 1.0);
                d = mix(bb.x, ba.x, hb);
                d = d - kappa * hb * (1.0 - hb);
                let g = mix(bb.yz, ba.yz, hb);
                let len = length(g);
                if (len > 1.0e-4) {
                    // The blend is exact but not unit, and a smooth
                    // minimum is not a distance field: normalise, and
                    // divide the distance by the same length.
                    d = d / len;
                    nrm = g / len;
                }
                have = true;
                cov = clamp(0.5 - d, 0.0, 1.0);
            } else {
                // **Hard spans, like `fill_rounded_within`.** M19 keeps
                // this structure so a panel is pixel-identical to a
                // rounded rect of the same geometry; an SDF ramp
                // disagrees by up to 254/255 on a fractional edge
                // (`docs/plan-glass.md` §4.1), and the software canvas
                // does not have one here either.
                let corner_row = !(iy >= ceil(m.prim.y + rr) && iy < floor(m.prim.w - rr)) && rr > 0.5;
                if (!corner_row || (ix >= ceil(m.prim.x + rr) && ix < floor(m.prim.z - rr))) {
                    cov = 1.0;
                } else {
                    let dx = max(max(m.prim.x + rr - p.x, p.x - (m.prim.z - rr)), 0.0);
                    let dy = max(max(m.prim.y + rr - p.y, p.y - (m.prim.w - rr)), 0.0);
                    cov = clamp(rr + 0.5 - sqrt(dx * dx + dy * dy), 0.0, 1.0);
                }
            }
            if (cov <= 0.0) { discard; }

            var at = p;
            var split = vec2(0.0);
            var spec = 0.0;
            var fres = 0.0;
            var dark = 0.0;
            let bevelled = rim > 0.0 && (m.p0.y > 0.0 || m.p0.z > 0.0 || m.p0.w > 0.0);
            if (bevelled) {
                if (!have) {
                    let b = blob(p, ctr, hxy, rr);
                    d = b.x;
                    nrm = b.yz;
                }
                let u = clamp(1.0 + d / rim, 0.0, 1.0);
                if (u > 0.0) {
                    let nz = sqrt(max(1.0 - u * u, 0.0));
                    if (m.p0.y > 0.0) {
                        let k = m.p0.y * u * u;
                        at = p + nrm * k;
                        if (m.p2.x > 0.0) {
                            split = nrm * (k * m.p2.x * MAX_DISPERSION);
                        }
                    }
                    if (m.p0.z > 0.0) {
                        let f = 1.0 - nz;
                        let f2 = f * f;
                        fres = f2 * f2 * f * m.p0.z;
                        let along = dot(nrm * u, m.p1.xy);
                        let dt = along + nz * m.p1.z;
                        if (dt > 0.0) {
                            let d2 = dt * dt;
                            let d8 = d2 * d2 * d2 * d2;
                            spec = d8 * d8 * d8 * m.p0.z;
                        }
                    }
                    if (m.p0.w > 0.0) {
                        let facing = -dot(nrm, m.p1.xy) / LIGHT_XY;
                        if (facing > 0.0) { dark = m.p0.w * u * u * facing; }
                    }
                }
            }

            var back = vec3(0.0);
            if (split.x != 0.0 || split.y != 0.0) {
                let lo = min(at - split, at + split);
                let hi = max(at - split, at + split);
                let edge = lo.x <= src.x + 0.5 || hi.x >= src.z - 0.5
                    || lo.y <= src.y + 0.5 || hi.y >= src.w - 0.5;
                if (edge) {
                    back = tap(at, src);
                } else {
                    back = vec3(
                        tap_ch(at - split, src, 0u),
                        tap_ch(at, src, 1u),
                        tap_ch(at + split, src, 2u),
                    );
                }
            } else {
                back = tap(at, src);
            }
            back = saturate_c(back, m.p1.w);

            var tint = m.tint;
            if (m.p2.y > 0.0) {
                // **The mean over the panel's own rect**, never the
                // clip: reducing over the clip makes a clipped panel
                // compute a different colour from an unclipped one.
                // Sampled on a grid rather than summed, because the
                // field it reduces has already been blurred — the CPU
                // sums every pixel of a buffer it owns, which a fragment
                // shader does not have.
                var acc = vec3(0.0);
                for (var gy = 0u; gy < 6u; gy = gy + 1u) {
                    for (var gx = 0u; gx < 6u; gx = gx + 1u) {
                        let f = (vec2(f32(gx), f32(gy)) + vec2(0.5)) / 6.0;
                        acc = acc + tap(mix(m.prim.xy, m.prim.zw, f), src);
                    }
                }
                let lum = dot(acc / 36.0, vec3(0.299, 0.587, 0.114));
                let l = clamp(lum, 0.0, 1.0);
                tint = mix(m.tint, m.tint2, l * l * (3.0 - 2.0 * l) * m.p2.y);
            }

            let tv = tint.a;
            let t = tv + (1.0 - tv) * fres;
            let shade = 1.0 - dark;
            // Shadow multiplies and specular adds — subtracting clips
            // each channel at a different point and rotates hue.
            col = vec4(clamp(mix(back, tint.rgb, t) * shade + vec3(spec), vec3(0.0), vec3(1.0)), 1.0);
        }
        default: {}
    }

    if (cov <= 0.0 || col.a <= 0.0) {
        discard;
    }
    return vec4(col.rgb, col.a * cov);
}
