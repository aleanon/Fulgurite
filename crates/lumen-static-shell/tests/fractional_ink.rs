//! M40/T40.0, R9-F5: a 1-logical-px rect inks the device pixels its
//! snapped geometry covers, and no more, at a fractional display scale.
//!
//! `Scaled` multiplied coordinates with no snapping, and the software
//! `fill` covers `floor(y0)..ceil(y1)` at full intensity. So at 1.25 a
//! divider at logical y = 22 became `27.5..28.75` and was drawn as two
//! whole rows for 1.25 rows of geometry, and the review measured the
//! frame's dividers at 1.61× their area (`docs/results-deep-review.md`
//! R9-F5).
//! The GPU ramps the same edge, so the two backends disagreed on every
//! hairline at every fractional scale, and nothing ran a scale other
//! than 1.
//!
//! The rule the gate holds `Scaled::rect` to is `docs/plan-physical-
//! damage.md` §2.2: each scaled edge rounds to the nearest device pixel,
//! independently, and an axis whose two edges round together keeps one
//! pixel (ties to even). So a divider inks `max(1, round(s·(y+1)) − round(s·y))` whole
//! rows — one or two at 1.5, depending on where it sits, and that is the
//! price of the next property: **two rects that share an edge still share
//! it**, with no gap row and no overlap row between them.
//!
//! Software only, so `just test` sees it. Its GPU twin is
//! `crates/lumen-static-wgpu/tests/parity.rs`'s fractional-scale arm.

use lumen_static::display::{Color, DisplayList, Painter, Renderer};
use lumen_static::Scaled;
use lumen_static_shell::software::Canvas;

const PAGE: Color = Color(255, 255, 255, 255);
const INK: Color = Color(0, 0, 0, 255);
/// Logical x extent of every divider.
const X0: f32 = 3.0;
const X1: f32 = 103.0;
/// Logical y of the six dividers: varied, so their scaled edges land at
/// every phase a fractional scale produces rather than one lucky one.
const YS: [f32; 6] = [10.0, 13.0, 17.0, 22.0, 28.0, 35.0];

/// One device pixel per edge, the rule under test: nearest, ties to
/// even (`Scaled::rect`'s rounding), and at least one pixel.
fn snapped(a: f32, b: f32, s: f32) -> (i64, i64) {
    let (p, q) = ((a * s).round_ties_even() as i64, (b * s).round_ties_even() as i64);
    (p, q.max(p + 1))
}

fn render(scale: f32, paint: impl Fn(&mut dyn Painter)) -> Canvas {
    let (w, h) = ((120.0 * scale).ceil() as u32, (50.0 * scale).ceil() as u32);
    let mut list = DisplayList::new();
    {
        let mut s = Scaled::new(&mut list, scale);
        paint(&mut s);
    }
    let mut c = Canvas::new(w, h);
    c.clear(PAGE);
    c.render(&list, w, h);
    c
}

/// Coverage summed over a band of rows, in pixels: 1.0 per fully inked
/// pixel.
fn ink(c: &Canvas, rows: core::ops::Range<i64>) -> f32 {
    let mut sum = 0.0;
    for y in rows.start.max(0)..rows.end.min(c.height as i64) {
        for x in 0..c.width {
            let i = ((y as u32 * c.width + x) * 4) as usize;
            sum += (255 - c.pixels[i]) as f32 / 255.0;
        }
    }
    sum
}

/// Per divider: its ink over what its snapped geometry covers. Returns
/// the worst ratio, having printed every one.
fn dividers(scale: f32) -> f32 {
    let c = render(scale, |p| {
        for y in YS {
            p.rect([X0, y, X1, y + 1.0], INK);
        }
    });
    let (cx0, cx1) = snapped(X0, X1, scale);
    let mut worst: f32 = 1.0;
    let (mut total, mut geometric) = (0.0, 0.0);
    for y in YS {
        let (r0, r1) = snapped(y, y + 1.0, scale);
        let expected = ((r1 - r0) * (cx1 - cx0)) as f32;
        // Wide enough to see any row the unsnapped fill adds either side.
        let got = ink(&c, (y * scale).floor() as i64 - 1..((y + 1.0) * scale).ceil() as i64 + 1);
        let ratio = got / expected;
        println!(
            "scale {scale:<4} divider y={y:<4} rows {r0}..{r1}: ink {got:.1} px / snapped {expected:.1} \
             = {ratio:.3} (bound 1.00)"
        );
        if (ratio - 1.0).abs() > (worst - 1.0).abs() {
            worst = ratio;
        }
        total += got;
        geometric += scale * scale * (X1 - X0);
    }
    println!("scale {scale:<4} all six: ink / geometric area = {:.3}", total / geometric);
    worst
}

/// Two translucent rects meeting at logical `edge`. Returns the rows
/// between them that are a gap (page) or an overlap (both blended).
fn shared_edge(scale: f32, edge: f32) -> (Vec<i64>, Vec<i64>) {
    let (a, b) = (Color(200, 0, 0, 128), Color(0, 0, 200, 128));
    let c = render(scale, |p| {
        p.rect([X0, edge - 5.0, X1, edge], a);
        p.rect([X0, edge, X1, edge + 5.0], b);
    });
    let x = ((X0 + X1) / 2.0 * scale) as u32;
    let (lo, hi) = (((edge - 5.0) * scale).round_ties_even() as i64, ((edge + 5.0) * scale).round_ties_even() as i64);
    let px = |y: i64| {
        let i = ((y as u32 * c.width + x) * 4) as usize;
        [c.pixels[i], c.pixels[i + 1], c.pixels[i + 2]]
    };
    // Each colour alone, read from a row well inside each rect, so the
    // blend arithmetic is the rasteriser's and not restated here.
    let (only_a, only_b) = (px(lo + 1), px(hi - 2));
    assert_ne!(only_a, only_b, "setup: the two rects must be told apart");
    let (mut gap, mut overlap) = (Vec::new(), Vec::new());
    for y in lo..hi {
        let p = px(y);
        if p == [255, 255, 255] {
            gap.push(y);
        } else if p != only_a && p != only_b {
            overlap.push(y);
        }
    }
    println!(
        "scale {scale:<4} edge {edge} -> physical {}: gap rows {gap:?}, overlap rows {overlap:?} (both must be empty)",
        edge * scale
    );
    (gap, overlap)
}

#[test]
fn whole_pixel_scales_ink_exactly_their_geometry() {
    // The accept direction: at 1.0 and 2.0 every edge is whole, and the
    // snap must change nothing.
    for scale in [1.0, 2.0] {
        let worst = dividers(scale);
        assert_eq!(worst, 1.0, "scale {scale}: a divider's ink is {worst:.3} of its geometry");
        for edge in [10.0, 11.0, 13.0] {
            assert_eq!(shared_edge(scale, edge), (vec![], vec![]), "scale {scale}, edge {edge}");
        }
    }
}

#[test]
fn a_one_logical_px_divider_inks_its_snapped_rows_at_fractional_scale() {
    for scale in [1.25, 1.5] {
        let worst = dividers(scale);
        assert!(
            (worst - 1.0).abs() < 1e-3,
            "scale {scale}: a divider inks {worst:.3}x its snapped geometry (bound 1.00)"
        );
    }
}

#[test]
fn two_rects_that_share_a_fractional_edge_still_share_it() {
    for scale in [1.25, 1.5] {
        for edge in [10.0, 11.0, 13.0] {
            let (gap, overlap) = shared_edge(scale, edge);
            assert!(gap.is_empty() && overlap.is_empty(), "scale {scale}, edge {edge}: gap {gap:?}, overlap {overlap:?}");
        }
    }
}
