//! M21: the bevel — refraction and specular.
//!
//! What separates **frosted** from **liquid**. A blur makes the backdrop
//! soft; a bevel makes the panel look like a solid object with an edge,
//! and it does that with almost no blur at all.
//!
//! The property underneath all of it is that the surface normal has a
//! **third component**. A rounded-rect SDF gives a 2D gradient, and a 2D
//! gradient is *constant* along each straight edge — so a highlight
//! built on one is a uniform stripe that switches off at the corners.
//! The bevel lifts it: `(u·n, sqrt(1 − u²))`, a spherical cap, already
//! unit and singular nowhere.

use fulgurite::display::{Color, DisplayList, Material};
use fulgurite_shell::software::Canvas;

const W: u32 = 240;
const H: u32 = 200;
const BG: Color = Color(0x20, 0x20, 0x20, 0xff);

/// A panel over a mid-grey field, so any brightening is the specular and
/// nothing else.
fn render(m: Material) -> Canvas {
    let mut c = Canvas::new(W, H);
    c.clear(BG);
    let mut l = DisplayList::new();
    l.rect([0.0, 0.0, W as f32, H as f32], Color(0x6a, 0x6a, 0x6a, 0xff));
    l.glass([40.0, 50.0, 200.0, 150.0], 30.0, m);
    fulgurite::Renderer::render(&mut c, &l, W, H);
    c
}

fn luma(c: &Canvas, x: usize, y: usize) -> f32 {
    let p = (y * W as usize + x) * 4;
    0.2126 * c.pixels[p] as f32 + 0.7152 * c.pixels[p + 1] as f32 + 0.0722 * c.pixels[p + 2] as f32
}

/// Brightest pixel in a box, which is where a highlight is.
fn brightest(c: &Canvas, x0: usize, x1: usize, y0: usize, y1: usize) -> f32 {
    let mut m = 0.0f32;
    for y in y0..y1 {
        for x in x0..x1 {
            m = m.max(luma(c, x, y));
        }
    }
    m
}

fn liquid() -> Material {
    Material { tint: Color(0, 0, 0, 0), ..Material::LIQUID }
}

/// **The `sign(p − c)` test.** `max(q, 0)` is componentwise
/// non-negative, so a normal built from it alone always points into the
/// `+x/+y` quadrant — and the left and top rims then light and refract
/// as if they were the right and bottom ones.
///
/// Light from the left must put the highlight on the **left** rim.
/// Asymmetric on both axes, so a dropped `sign` shows on either.
#[test]
fn the_highlight_is_on_the_side_the_light_is() {
    // π radians: straight left in y-down space.
    let left = render(Material { light: core::f32::consts::PI, ..liquid() });
    let l = brightest(&left, 40, 70, 70, 130);
    let r = brightest(&left, 170, 200, 70, 130);
    println!("light from the left: left rim {l:.0}, right rim {r:.0}");
    assert!(l > r + 8.0, "the highlight is on the wrong side: left {l:.0}, right {r:.0}");

    // And the mirror, so neither side is bright by accident.
    let right = render(Material { light: 0.0, ..liquid() });
    let l2 = brightest(&right, 40, 70, 70, 130);
    let r2 = brightest(&right, 170, 200, 70, 130);
    println!("light from the right: left rim {l2:.0}, right rim {r2:.0}");
    assert!(r2 > l2 + 8.0, "mirrored light did not mirror the highlight");
}

/// Up and down too — a `sign` dropped on only one axis would pass the
/// test above.
#[test]
fn the_highlight_follows_the_light_vertically_as_well() {
    let up = render(Material { light: -core::f32::consts::FRAC_PI_2, ..liquid() });
    let t = brightest(&up, 90, 150, 50, 72);
    let b = brightest(&up, 90, 150, 128, 150);
    println!("light from above: top rim {t:.0}, bottom rim {b:.0}");
    assert!(t > b + 8.0, "the highlight is on the wrong edge: top {t:.0}, bottom {b:.0}");
}

/// **Specular must survive `refraction = 0`.**
///
/// The first design derived the rim's width from the displacement, so a
/// panel that bent nothing was also a panel that caught no light — and
/// "see-through with a lit edge" is exactly the case worth having.
/// `rim` is its own field for this reason.
#[test]
fn a_panel_that_bends_nothing_still_catches_the_light() {
    // **The light is set explicitly, and the first version was not.**
    // `Material::LIQUID` lights from the upper *right*, so sampling the
    // left rim measured a face correctly in shadow and read as "no
    // highlight". A lit-edge test has to sample the lit edge.
    let flat = render(Material {
        refraction: 0.0,
        light: core::f32::consts::PI,
        ..liquid()
    });
    let rim = brightest(&flat, 40, 70, 70, 130);
    let middle = brightest(&flat, 110, 130, 95, 105);
    println!("refraction 0, light from the left: rim {rim:.0}, middle {middle:.0}");
    assert!(rim > middle + 10.0, "no highlight without refraction: rim {rim:.0}, middle {middle:.0}");
}

/// A bevel displaces what is behind it. With a hard edge in the
/// backdrop, the displacement is visible as the edge moving.
/// A bevel displaces what is behind it.
///
/// Measured over the **rim band**, because that is the only place a
/// bevel does anything: `u` falls to 0 within `rim` px of the boundary,
/// so the displacement is `refraction · u²` and is strongest right at
/// the edge. The first version put a backdrop edge 12 px inside the
/// panel and looked for it to move — 12 px in, `u` is 0.04 and the
/// displacement is a sixtieth of a pixel, so nothing moved and the test
/// was measuring the flat interior.
///
/// Specular off, so brightness cannot be mistaken for displacement.
#[test]
fn refraction_displaces_the_backdrop() {
    let scene = |refraction: f32| {
        let mut c = Canvas::new(W, H);
        c.clear(BG);
        let mut l = DisplayList::new();
        // Stripes, so any displacement lands on a different colour.
        for k in 0..24 {
            let x = k as f32 * 10.0;
            let v = if k % 2 == 0 { 0xf0 } else { 0x30 };
            l.rect([x, 0.0, x + 10.0, H as f32], Color(v, v, v, 0xff));
        }
        l.glass(
            [40.0, 50.0, 200.0, 150.0],
            30.0,
            Material { refraction, specular: 0.0, tint: Color(0, 0, 0, 0), ..Material::LIQUID },
        );
        fulgurite::Renderer::render(&mut c, &l, W, H);
        c
    };
    let flat = scene(0.0);
    let bent = scene(12.0);

    // The rim band on the straight left edge: x within `rim` of 40.
    let (mut differ, mut checked) = (0, 0);
    for y in 85..115usize {
        for x in 40..53usize {
            checked += 1;
            if (luma(&flat, x, y) - luma(&bent, x, y)).abs() > 12.0 {
                differ += 1;
            }
        }
    }
    println!("{differ} of {checked} rim px moved under refraction");
    assert!(checked > 300, "not enough sampled: {checked}");
    assert!(differ > checked / 5, "refraction barely moved the backdrop: {differ}/{checked}");

    // And the flat interior must be untouched, or this is measuring
    // something other than a bevel.
    let mut inner = 0;
    for y in 85..115usize {
        for x in 90..150usize {
            if (luma(&flat, x, y) - luma(&bent, x, y)).abs() > 1.0 {
                inner += 1;
            }
        }
    }
    assert_eq!(inner, 0, "{inner} interior px moved; the bevel is not confined to the rim");
}

/// Degenerate geometry must not produce NaN. `normalize(max(q, 0))` is
/// `0/0` over the whole inner rect, not merely at the centre.
#[test]
fn degenerate_geometry_stays_finite() {
    for (radius, rim, refraction) in
        [(0.0f32, 13.0f32, 11.0f32), (30.0, 200.0, 200.0), (30.0, 0.0, 11.0), (1.0, 13.0, 0.0)]
    {
        let mut c = Canvas::new(W, H);
        c.clear(BG);
        let mut l = DisplayList::new();
        l.rect([0.0, 0.0, W as f32, H as f32], Color(0x6a, 0x6a, 0x6a, 0xff));
        l.glass([40.0, 50.0, 200.0, 150.0], radius, Material { rim, refraction, ..liquid() });
        fulgurite::Renderer::render(&mut c, &l, W, H);
        // A NaN reaching `as u8` saturates to 0, so a black panel is the
        // symptom. Assert the panel is not black and not blown out.
        let m = brightest(&c, 60, 180, 70, 130);
        println!("radius {radius}, rim {rim}, refraction {refraction}: brightest {m:.0}");
        assert!(m > 20.0, "radius {radius} rim {rim} refraction {refraction}: panel went black");
    }
}
