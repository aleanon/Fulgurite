//! M19/T19.3: the blur's shape, not its speed.
//!
//! The speed is `benches --bin glass-cost` and `docs/results-glass.md`. What
//! is checked here is the property a table cannot show: **a blur must
//! not move the image**.

use lumen_static::display::{Color, DisplayList, Material};
use lumen_static_shell::software::{blur_plan, Canvas};

const W: u32 = 200;
const H: u32 = 120;

fn render(blur: f32) -> Canvas {
    let mut c = Canvas::new(W, H);
    c.clear(Color(10, 10, 10, 255));
    let mut l = DisplayList::new();
    // A single bright bar, centred exactly. Whatever the blur does to
    // it must stay centred.
    l.rect([0.0, 58.0, W as f32, 62.0], Color(250, 250, 250, 255));
    // A fully transparent tint, so what the panel shows is the blurred
    // backdrop and nothing else.
    l.glass(
        [0.0, 0.0, W as f32, H as f32],
        0.0,
        Material { blur, tint: Color(0, 0, 0, 0), ..Material::FROSTED },
    );
    lumen_static::Renderer::render(&mut c, &l, W, H);
    c
}

/// Mean luminance of a row, down the middle of the canvas.
fn row_weight(c: &Canvas, y: usize) -> f64 {
    let mut sum = 0.0;
    for x in 60..140 {
        let p = (y * W as usize + x) * 4;
        sum += c.pixels[p] as f64;
    }
    sum / 80.0
}

/// **The odd/even test.** Three equal boxes of an *even* width have no
/// centre pixel, so each pass shifts the image half a pixel and three
/// shift it 1.5 — a visible slide of the backdrop under the panel. SVG's
/// rule replaces one of them with a box of `d + 1`; this is what says
/// that rule is implemented.
///
/// Measured as the blur's centre of mass: symmetric input, so a correct
/// blur leaves it where it was.
#[test]
fn a_blur_does_not_move_what_it_blurs() {
    for sigma in [2.0f32, 4.0, 6.0, 8.0, 12.0] {
        let (k, d, _) = blur_plan(sigma);
        let c = render(sigma);
        let (mut num, mut den) = (0.0f64, 0.0f64);
        for y in 0..H as usize {
            let wgt = row_weight(&c, y) - 10.0;
            if wgt > 0.0 {
                num += wgt * y as f64;
                den += wgt;
            }
        }
        assert!(den > 0.0, "sigma {sigma}: the bar vanished");
        let centre = num / den;
        // The bar spans rows 58..62, so its centre of mass is 59.5.
        println!("sigma {sigma} (k={k}, d={d}): centre of mass {centre:.3}, want 59.5");
        assert!(
            (centre - 59.5).abs() < 0.35,
            "sigma {sigma} (d={d}) moved the image to {centre:.3}; a 1.5 px slide is what \
             three equal even-width boxes do"
        );
    }
}

/// A wider blur spreads further. Trivial to state and the thing that
/// breaks first if the downsample or the box width is wrong.
#[test]
fn a_wider_blur_spreads_further() {
    let spread = |sigma: f32| {
        let c = render(sigma);
        (0..H as usize).filter(|y| row_weight(&c, *y) > 14.0).count()
    };
    let (a, b, d) = (spread(2.0), spread(8.0), spread(24.0));
    println!("rows lit: sigma 2 -> {a}, 8 -> {b}, 24 -> {d}");
    assert!(a < b && b < d, "spread is not monotone in sigma: {a}, {b}, {d}");
}

/// The plan the renderer follows is the plan the results doc publishes,
/// and it stays inside the error the doc claims.
///
/// **Swept finely, not sampled.** The first version checked eight round
/// sigmas and passed even with the search replaced by `k = ceil(σ/2)` —
/// the rule the search exists to beat, whose worst case is near σ = 9.3
/// and which no round number lands on. A bound that only holds where
/// you looked is not a bound.
#[test]
fn the_delivered_sigma_is_within_ten_percent_everywhere() {
    let mut worst = (0.0f32, 0.0f32, 0.0f32);
    let mut sigma = 2.0f32;
    while sigma <= 32.0 {
        let (_, _, got) = blur_plan(sigma);
        let err = (got - sigma).abs() / sigma * 100.0;
        if err > worst.2 {
            worst = (sigma, got, err);
        }
        sigma += 0.05;
    }
    // Bound 12% over a measured worst of **10.0% at sigma 3.85**, which
    // is where an integer box width is least able to hit a small sigma.
    // The plan predicted "about 10%" for this search and that is what it
    // is; the bound sits just above so the number, not the bound, is
    // what a reader takes away.
    println!("worst over 2..32: sigma {:.2} -> {:.2} ({:.1}%), bound 12%", worst.0, worst.1, worst.2);
    assert!(
        worst.2 < 12.0,
        "sigma {:.2} delivered {:.2}, {:.1}% off — the bound is 12%",
        worst.0,
        worst.1,
        worst.2
    );
}

/// **The downsample grid is anchored to the surface, not to the panel.**
///
/// Anchored to the capture, a panel at a different position samples a
/// different phase of the backdrop, so translating the whole scene by
/// exactly `k` pixels would change the blurred result — a shimmer as a
/// panel scrolls, invisible to every single-frame test and to the
/// incremental-vs-rebuild oracle alike, because the capture rect is
/// identical in the two renders those compare.
///
/// Rendered twice, the second shifted by `k` on both axes, and the
/// overlapping interiors must match exactly.
#[test]
fn translating_the_scene_by_k_translates_the_blur_exactly() {
    const SIGMA: f32 = 12.0;
    let (k, _, _) = blur_plan(SIGMA);
    let shift = k as f32;

    // A canvas big enough that the panel's *source region* is nowhere
    // near an edge in either placement. Clamping the capture at the
    // surface boundary is a legitimate edge rule, and the first version
    // of this test put the panel at (0, 0) where it applied — 746 of
    // 7 200 px differed for that reason and not because of the grid.
    const BW: u32 = 320;
    const BH: u32 = 240;
    let render_at = |dx: f32, dy: f32| {
        let mut c = Canvas::new(BW, BH);
        c.clear(Color(10, 10, 10, 255));
        let mut l = DisplayList::new();
        for n in 0..6 {
            let y = dy + 70.0 + n as f32 * 14.0;
            l.rect([dx + 55.0, y, dx + 205.0, y + 5.0], Color(240, 240, 240, 255));
        }
        l.glass(
            [dx + 60.0, dy + 60.0, dx + 200.0, dy + 160.0],
            0.0,
            Material { blur: SIGMA, tint: Color(0, 0, 0, 0), ..Material::FROSTED },
        );
        lumen_static::Renderer::render(&mut c, &l, BW, BH);
        c
    };

    let a = render_at(0.0, 0.0);
    let b = render_at(shift, shift);

    // Compare a's (x, y) against b's (x + k, y + k), well inside both
    // panels so no edge case is involved.
    let mut differ = 0;
    let mut checked = 0;
    for y in 70..150usize {
        for x in 70..190usize {
            let pa = (y * BW as usize + x) * 4;
            let pb = ((y + k) * BW as usize + x + k) * 4;
            checked += 1;
            if a.pixels[pa..pa + 3] != b.pixels[pb..pb + 3] {
                differ += 1;
            }
        }
    }
    println!("k = {k}: {differ} of {checked} px differ after translating by k");
    assert!(checked > 5000, "not enough sampled: {checked}");
    assert_eq!(differ, 0, "the blur is not translation-invariant at its own grid step");
}

/// **Moving the panel by one pixel must not re-bucket the backdrop.**
///
/// This is what "anchored to the surface" actually buys, and the
/// translation test above cannot see it: shifting by exactly `k`
/// preserves the grid phase whether the grid is anchored to the surface
/// or to the capture, because both origins move by `k` too. Shifting by
/// **one** is what separates them — an unanchored grid re-buckets the
/// same backdrop pixels into different cells, so the blurred result
/// changes under a panel that merely slid.
///
/// The backdrop is held still and only the panel moves, so any
/// difference over their common interior is the grid and nothing else.
#[test]
fn sliding_the_panel_by_one_pixel_does_not_change_the_blur() {
    const SIGMA: f32 = 12.0;
    const BW: u32 = 320;
    const BH: u32 = 240;
    let (k, _, _) = blur_plan(SIGMA);
    assert!(k > 1, "this test needs a downsample to exist; k = {k}");

    let render_at = |dx: f32| {
        let mut c = Canvas::new(BW, BH);
        c.clear(Color(10, 10, 10, 255));
        let mut l = DisplayList::new();
        // The backdrop does **not** move.
        for n in 0..6 {
            let y = 70.0 + n as f32 * 14.0;
            l.rect([55.0, y, 205.0, y + 5.0], Color(240, 240, 240, 255));
        }
        l.glass(
            [60.0 + dx, 60.0, 200.0 + dx, 160.0],
            0.0,
            Material { blur: SIGMA, tint: Color(0, 0, 0, 0), ..Material::FROSTED },
        );
        lumen_static::Renderer::render(&mut c, &l, BW, BH);
        c
    };

    let a = render_at(0.0);
    let b = render_at(1.0);
    let (mut differ, mut checked) = (0, 0);
    for y in 70..150usize {
        for x in 70..190usize {
            let p = (y * BW as usize + x) * 4;
            checked += 1;
            if a.pixels[p..p + 3] != b.pixels[p..p + 3] {
                differ += 1;
            }
        }
    }
    println!("k = {k}: {differ} of {checked} px differ after sliding the panel 1 px");
    assert!(checked > 5000, "not enough sampled: {checked}");
    assert_eq!(differ, 0, "the downsample grid moved with the panel instead of the surface");
}
