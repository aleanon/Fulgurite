//! M82/T82.5: the path rasteriser's coverage, against a supersampled
//! reference, and its round joins (`docs/plan-paint-vocabulary.md`).
//!
//! **Coverage is judged on edge pixels only** (the plan's G12): a pixel
//! wholly inside or outside a shape is 255 or 0 under any rasteriser that
//! fills at all, so averaging over them would dilute an edge error until
//! a binary (stencil-without-MSAA) rasteriser passed. The population is
//! the pixels whose reference coverage is strictly between 0 and 1,
//! counted and printed beside its floor.
//!
//! The reference is non-zero winding at 32 × 32 samples a pixel, of the
//! **true** shape: the star's polygon, and the circle itself rather than
//! its four cubics, so the circle's error includes the flattening's.

use lumen_static::display::PathEl;
use lumen_static::Point;
use lumen_static_shell::software::{rasterize_path, PathCoverage, PathScratch, FLATTEN_TOLERANCE};

const N: usize = 128;
const SS: usize = 32;

fn pt(x: f32, y: f32) -> Point {
    Point { x, y }
}

/// A five-pointed star as its ten-vertex outline — concave, never
/// self-intersecting, so its winding is 0 or 1 everywhere.
fn star() -> Vec<Point> {
    let (cx, cy) = (64.3, 63.6);
    (0..10)
        .map(|k| {
            let r = if k % 2 == 0 { 52.0 } else { 21.0 };
            let a = -core::f32::consts::FRAC_PI_2 + k as f32 * core::f32::consts::PI / 5.0;
            pt(cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

fn polygon_els(p: &[Point]) -> Vec<PathEl> {
    let mut v = vec![PathEl::Move(p[0])];
    v.extend(p[1..].iter().map(|q| PathEl::Line(*q)));
    v.push(PathEl::Close);
    v
}

const CIRCLE: (f32, f32, f32) = (64.3, 63.7, 40.0);

/// The circle as four cubics, the usual `k = 0.5523` construction.
fn circle_els() -> Vec<PathEl> {
    let (cx, cy, r) = CIRCLE;
    let k = 0.552_284_8 * r;
    vec![
        PathEl::Move(pt(cx + r, cy)),
        PathEl::Cubic(pt(cx + r, cy + k), pt(cx + k, cy + r), pt(cx, cy + r)),
        PathEl::Cubic(pt(cx - k, cy + r), pt(cx - r, cy + k), pt(cx - r, cy)),
        PathEl::Cubic(pt(cx - r, cy - k), pt(cx - k, cy - r), pt(cx, cy - r)),
        PathEl::Cubic(pt(cx + k, cy - r), pt(cx + r, cy - k), pt(cx + r, cy)),
        PathEl::Close,
    ]
}

fn winding(p: &[Point], x: f32, y: f32) -> i32 {
    let mut w = 0;
    for k in 0..p.len() {
        let (a, b) = (p[k], p[(k + 1) % p.len()]);
        if (a.y <= y) != (b.y <= y) {
            let t = (y - a.y) / (b.y - a.y);
            if x < a.x + t * (b.x - a.x) {
                w += if b.y > a.y { 1 } else { -1 };
            }
        }
    }
    w
}

/// Fraction of each pixel's 32 × 32 samples inside the shape.
fn reference(inside: impl Fn(f32, f32) -> bool) -> Vec<f32> {
    let mut out = vec![0.0; N * N];
    for py in 0..N {
        for px in 0..N {
            let mut n = 0;
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = px as f32 + (sx as f32 + 0.5) / SS as f32;
                    let y = py as f32 + (sy as f32 + 0.5) / SS as f32;
                    n += inside(x, y) as u32;
                }
            }
            out[py * N + px] = n as f32 / (SS * SS) as f32;
        }
    }
    out
}

struct Judged {
    edge: usize,
    mean: f32,
    max: f32,
    interior: usize,
    interior_wrong: usize,
    exterior: usize,
    exterior_wrong: usize,
}

/// Compare a mask with the reference: the edge population's mean and max
/// error in 1/255, and the interior (exterior) pixels — reference 1 (0)
/// with all eight neighbours 1 (0) — that are not exactly 255 (0).
fn judge(mask: &[u8], r: &[f32]) -> Judged {
    let mut j = Judged { edge: 0, mean: 0.0, max: 0.0, interior: 0, interior_wrong: 0, exterior: 0, exterior_wrong: 0 };
    let all = |px: usize, py: usize, v: f32| {
        (py.saturating_sub(1)..(py + 2).min(N)).all(|y| (px.saturating_sub(1)..(px + 2).min(N)).all(|x| r[y * N + x] == v))
    };
    let mut sum = 0.0;
    for py in 0..N {
        for px in 0..N {
            let (m, f) = (mask[py * N + px] as f32 / 255.0, r[py * N + px]);
            if f > 0.0 && f < 1.0 {
                let e = (m - f).abs() * 255.0;
                j.edge += 1;
                sum += e;
                j.max = j.max.max(e);
            } else if f == 1.0 && all(px, py, 1.0) {
                j.interior += 1;
                j.interior_wrong += (mask[py * N + px] != 255) as usize;
            } else if f == 0.0 && all(px, py, 0.0) {
                j.exterior += 1;
                j.exterior_wrong += (mask[py * N + px] != 0) as usize;
            }
        }
    }
    j.mean = sum / j.edge.max(1) as f32;
    j
}

fn check(name: &str, els: &[PathEl], r: &[f32], mean_bound: f32, max_bound: f32) {
    let mut scratch = PathScratch::default();
    let mask = rasterize_path(els, PathCoverage::Fill, [0, 0], N, N, &mut scratch).to_vec();
    let j = judge(&mask, r);
    println!(
        "{name}: {} edge px (floor 200); mean |err| {:.3}/255 (bound {mean_bound:.1}/255), max {:.2}/255 \
         (bound {max_bound:.1}/255); interior {} px, {} not 255; exterior {} px, {} not 0",
        j.edge, j.mean, j.max, j.interior, j.interior_wrong, j.exterior, j.exterior_wrong
    );
    assert!(j.edge >= 200, "{name}: setup — only {} edge pixels, the population is too small to judge", j.edge);
    assert!(j.interior > 0 && j.exterior > 0, "{name}: setup — the shape must have an inside and an outside");
    assert!(j.mean <= mean_bound, "{name}: mean edge error {:.3}/255 over {mean_bound}/255", j.mean);
    assert!(j.max <= max_bound, "{name}: max edge error {:.2}/255 over {max_bound}/255", j.max);
    assert_eq!(j.interior_wrong, 0, "{name}: interior pixels not fully covered");
    assert_eq!(j.exterior_wrong, 0, "{name}: exterior pixels touched");
}

/// Straight edges: the accumulation is exact area, so the only error is
/// the reference's own sampling.
#[test]
fn a_star_of_lines_covers_its_edge_pixels_analytically() {
    let p = star();
    let r = reference(|x, y| winding(&p, x, y) != 0);
    check("star", &polygon_els(&p), &r, 1.0, 8.0);
}

/// Cubics: the flattening at [`FLATTEN_TOLERANCE`] is the error, and the
/// bounds are computed from that tolerance, not chosen.
///
/// A chord whose sagitta is `s` lies on average `2s/3` inside its arc, so
/// an edge pixel's mean error is at most `2/3 · tol · 255` ≈ 17/255; its
/// worst is a pixel the chord crosses diagonally at the sagitta, `√2 · tol
/// · 255` ≈ 36/255. The plan wrote 3/255 and 26/255 for these and called
/// them "the 0.1 px flattening"; 3/255 is a sagitta of about 0.02 px, and
/// 26/255 ignores the diagonal (`docs/plan-paint-vocabulary.md`, "What
/// building found").
#[test]
fn a_circle_of_cubics_covers_its_edge_pixels_within_the_flattening() {
    let (cx, cy, rr) = CIRCLE;
    let r = reference(|x, y| (x - cx) * (x - cx) + (y - cy) * (y - cy) < rr * rr);
    let mean = 2.0 / 3.0 * FLATTEN_TOLERANCE * 255.0;
    let max = core::f32::consts::SQRT_2 * FLATTEN_TOLERANCE * 255.0;
    check("circle", &circle_els(), &r, mean, max);
}

/// **A round join covers the outside of a corner.** A 90° turn stroked
/// 16 px wide: the pixel on the join's outer bisector, `w / 4` from the
/// vertex, is outside both segments' quads and inside the join's disc.
/// Not the geometric outer corner, which lies `w / √2` out — outside a
/// round join, and uncovered on correct code.
#[test]
fn a_round_join_fills_the_outside_of_a_corner() {
    let w = 16.0;
    let v = pt(64.0, 64.0);
    let els = [PathEl::Move(pt(20.0, 64.0)), PathEl::Line(v), PathEl::Line(pt(64.0, 20.0))];
    let mut scratch = PathScratch::default();
    let mask = rasterize_path(&els, PathCoverage::Stroke(w), [0, 0], N, N, &mut scratch).to_vec();
    // The turn's outside is down-right of the vertex.
    let d = w / 4.0 / core::f32::consts::SQRT_2;
    let (px, py) = ((v.x + d) as usize, (v.y + d) as usize);
    let (cx, cy) = (px as f32 + 0.5, py as f32 + 0.5);
    // Setup: the probe is outside both quads and inside the disc.
    assert!(cx > v.x && cy > v.y, "setup — the probe must be past both segments' ends");
    let dist = ((cx - v.x).powi(2) + (cy - v.y).powi(2)).sqrt();
    assert!(dist + 0.71 < w / 2.0 * (core::f32::consts::PI / 16.0).cos(), "setup — the probe pixel must lie in the join");
    let cov = mask[py * N + px] as f32 / 255.0;
    let mid = mask[64 * N + 40] as f32 / 255.0;
    println!("join: pixel ({px}, {py}), {dist:.2} px from the vertex, coverage {cov:.3} (bound > 0.5); segment interior {mid:.3}");
    assert_eq!(mid, 1.0, "setup — the segment itself must be stroked");
    assert!(cov > 0.5, "the round join did not cover the corner's outside: coverage {cov:.3}");
}
