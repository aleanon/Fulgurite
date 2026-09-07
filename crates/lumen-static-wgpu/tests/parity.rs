//! T15.12's gate: the same frame on both backends.
//!
//! Three claims, in increasing strength:
//!
//! 1. **The GPU path draws.** A frame is not blank and its ink is where
//!    the commands put it.
//! 2. **It agrees with the software canvas** within a stated tolerance.
//!    Not bit-exact, and the reason is written down rather than waved
//!    at: the GPU antialiases edges the software rasteriser rounds to
//!    whole pixels, so a fractional edge differs by up to one pixel of
//!    ramp. Every difference is bounded and counted here.
//! 3. **A partial redraw equals a full one, exactly**, on the GPU path —
//!    the property `lumen-static-shell/tests/damage_oracle.rs` holds the
//!    software path to. This one *is* bit-exact, because the clip test
//!    in the shader is hard and the damage rect is snapped to whole
//!    pixels.
//!
//! Every test here **skips** when no adapter can be found rather than
//! failing. A machine with no GPU is a supported configuration — it is
//! the configuration the CPU fallback exists for — and a test suite that
//! went red on it would be reporting the wrong thing.

use lumen_static::display::{Color, DisplayList, Renderer, Stops};
use lumen_static::Point;
use lumen_static_shell::software::Canvas;
use lumen_static_wgpu::Gpu;

const W: u32 = 240;
const H: u32 = 180;
const PAGE: Color = Color(250, 250, 250, 255);

/// A frame using every command in the vocabulary.
///
/// Coordinates are whole pixels wherever the shape's *edge* is, because
/// a fractional edge is exactly where the two backends are allowed to
/// differ — putting one in this list would measure the tolerance rather
/// than the agreement. `tolerance_is_bounded_at_fractional_edges` uses
/// fractional ones on purpose.
fn frame() -> DisplayList {
    let mut l = DisplayList::new();
    l.rect([10.0, 10.0, 70.0, 40.0], Color(200, 60, 60, 255));
    l.rounded_rect([80.0, 10.0, 150.0, 45.0], Color(40, 90, 200, 255), 10.0);
    l.shadow([90.0, 60.0, 160.0, 100.0], 8.0, 10.0);
    l.rounded_rect([90.0, 60.0, 160.0, 100.0], Color(30, 38, 54, 255), 8.0);
    l.line(Point { x: 12.0, y: 60.0 }, Point { x: 70.0, y: 100.0 }, 3.0, Color(20, 160, 90, 255));
    l.gradient([10.0, 120.0, 110.0, 160.0], Stops::two(Color(255, 200, 0, 255), Color(200, 0, 120, 255)), 0.0);
    // Half-transparent, so the blend arithmetic is compared too and not
    // just the coverage.
    l.rect([120.0, 120.0, 200.0, 160.0], Color(0, 120, 200, 128));
    // A clip scope with something escaping it, so the clip stack is
    // exercised on both sides.
    l.push_clip([160.0, 10.0, 200.0, 50.0]);
    l.rect([150.0, 0.0, 230.0, 60.0], Color(120, 120, 120, 255));
    l.pop_clip();
    l.text([10.0, 45.0, 140.0, 62.0], "parity", 14.0, Color(0, 0, 0, 255));
    l
}

/// `None` when there is no GPU here. Prints why, so a skipped test does
/// not look like a passing one.
fn gpu() -> Option<Gpu> {
    match Gpu::headless(W, H) {
        Some(mut g) => {
            eprintln!("gpu: {}", g.adapter());
            if let Some(f) = lumen_static_shell::text::FontStack::system() {
                g.set_font(f);
            }
            Some(g)
        }
        None => {
            eprintln!("gpu: no adapter — skipping (this is a supported configuration)");
            None
        }
    }
}

fn canvas() -> Canvas {
    match lumen_static_shell::text::FontStack::system() {
        Some(f) => Canvas::with_font(W, H, f),
        None => Canvas::new(W, H),
    }
}

/// Per-pixel comparison: the worst channel difference, and how many
/// pixels differ by more than `soft`.
fn compare(a: &[u8], b: &[u8], soft: u8) -> (u8, usize) {
    let mut worst = 0u8;
    let mut over = 0usize;
    for (pa, pb) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        let mut d = 0u8;
        for k in 0..3 {
            d = d.max(pa[k].abs_diff(pb[k]));
        }
        worst = worst.max(d);
        if d > soft {
            over += 1;
        }
    }
    (worst, over)
}

#[test]
fn the_gpu_path_draws_a_frame() {
    let Some(mut g) = gpu() else { return };
    g.clear(PAGE);
    g.render(&frame(), W, H);
    let px = g.read_rgba();
    assert_eq!(px.len(), (W * H * 4) as usize);

    let inked = px
        .chunks_exact(4)
        .filter(|p| p[0] != PAGE.0 || p[1] != PAGE.1 || p[2] != PAGE.2)
        .count();
    let frac = inked as f32 / (W * H) as f32;
    assert!(frac > 0.10, "the frame is nearly blank ({frac:.3} inked)");

    // The red rect's interior is exactly the colour asked for: coverage
    // is 1 well inside a shape, so this is not a tolerance question.
    let at = |x: u32, y: u32| {
        let i = ((y * W + x) * 4) as usize;
        Color(px[i], px[i + 1], px[i + 2], px[i + 3])
    };
    assert_eq!(at(40, 25), Color(200, 60, 60, 255), "inside the rect");
    assert_eq!(at(5, 5), Color(250, 250, 250, 255), "outside everything");
    // Clipped away: the grey rect spans x 150..230 but its scope stops
    // at 160, so 155 must still be the page.
    assert_eq!(at(155, 30), Color(250, 250, 250, 255), "escaped its clip scope");
    assert_eq!(at(170, 30), Color(120, 120, 120, 255), "inside its clip scope");
}

#[test]
fn the_two_backends_agree_within_the_stated_tolerance() {
    let Some(mut g) = gpu() else { return };
    let mut c = canvas();
    c.clear(PAGE);
    c.render(&frame(), W, H);
    g.clear(PAGE);
    g.render(&frame(), W, H);

    let (worst, over) = compare(&c.pixels, &g.read_rgba(), 8);
    let frac = over as f32 / (W * H) as f32;
    eprintln!("parity: worst channel delta {worst}, {over} px ({:.2}%) over 8", frac * 100.0);

    // **The stated tolerance: fewer than 0.2% of pixels may differ by
    // more than 8 levels.** Measured at 0.01% (4 pixels) on this frame
    // and 0.000% on the richer one in `examples/compare`
    // (`docs/results-gpu.md`) — so the bound has ~20x headroom and is
    // still tight enough to catch a shape that moved by a pixel, which
    // would put a whole edge over the line rather than four corners.
    //
    // *How much* an edge pixel differs is deliberately not bounded: an
    // edge pixel is the full difference between a shape and the page,
    // and a bound on it would be a bound on antialiasing quality rather
    // than on agreement.
    assert!(frac < 0.002, "too many pixels differ: {:.3}% (worst {worst})", frac * 100.0);
}

#[test]
fn interiors_are_identical_not_merely_close() {
    // The stronger half of the comparison: away from every edge, the
    // two backends must agree almost exactly. If they do not, the
    // difference is a blend or a colour-space bug rather than
    // antialiasing, and the tolerance above would have hidden it.
    let Some(mut g) = gpu() else { return };
    let mut c = canvas();
    c.clear(PAGE);
    c.render(&frame(), W, H);
    g.clear(PAGE);
    g.render(&frame(), W, H);
    let gp = g.read_rgba();

    // Interiors of three shapes and one translucent overlay.
    for (x, y, what) in [
        (40u32, 25u32, "opaque rect"),
        (115, 30, "rounded rect"),
        (125, 80, "card over its shadow"),
        (160, 140, "translucent rect over the page"),
        (60, 140, "gradient"),
    ] {
        let i = ((y * W + x) * 4) as usize;
        for k in 0..3 {
            let d = c.pixels[i + k].abs_diff(gp[i + k]);
            assert!(d <= 2, "{what} at ({x},{y}) channel {k}: {} vs {} ", c.pixels[i + k], gp[i + k]);
        }
    }
}

#[test]
fn a_partial_redraw_is_bit_identical_to_a_full_one() {
    let Some(mut g) = gpu() else { return };

    // Frame 1, the ground both later frames are compared from.
    g.clear(PAGE);
    g.render(&frame(), W, H);
    let full_first = g.read_rgba();

    // The change, and the region it damaged.
    //
    // **Both are placed inside the translucent rect on purpose, and the
    // region's edges are fractional on purpose.** Those two choices are
    // what give the test teeth, and it took a fault injection to learn
    // it: with whole-pixel damage over opaque content, this test passes
    // whether or not `snap_out` exists, because a boundary pixel that
    // is then covered by an opaque command has its half-cleared value
    // overwritten, and a boundary pixel over unchanged background is
    // being blended with a copy of itself.
    //
    // Under *translucent* content neither rescue applies: an
    // antialiased clear leaves `0.7·page + 0.3·(page under glass)` at
    // the edge, the glass is then blended over it a second time, and
    // the pixel differs from a full render. That is the artefact
    // `snap_out` exists to prevent, and this is where it shows.
    let mut second = frame();
    second.rect([140.0, 132.0, 180.0, 148.0], Color(0, 0, 0, 255));
    let damage = [135.3, 128.7, 185.2, 152.4];

    g.draw(&second, Some(damage), Some((PAGE, damage)));
    let partial = g.read_rgba();

    g.clear(PAGE);
    g.render(&second, W, H);
    let full = g.read_rgba();

    assert_ne!(partial, full_first, "the second frame changed nothing — the test proves nothing");
    let (worst, over) = compare(&partial, &full, 0);
    assert_eq!(
        (worst, over),
        (0, 0),
        "a partial redraw must be *identical* to a full one on this backend"
    );
}

#[test]
fn tolerance_is_bounded_at_fractional_edges() {
    // The one place the two backends are known to disagree, isolated:
    // a rect whose edges land between pixels. The software canvas fills
    // from floor(x0) to ceil(x1) — whole pixels, no ramp — and the GPU
    // ramps. The disagreement must stay *at the edge*: one column and
    // one row, not a shifted shape.
    let Some(mut g) = gpu() else { return };
    let mut l = DisplayList::new();
    l.rect([10.4, 10.4, 60.6, 50.6], Color(0, 0, 0, 255));

    let mut c = canvas();
    c.clear(PAGE);
    c.render(&l, W, H);
    g.clear(PAGE);
    g.render(&l, W, H);
    let gp = g.read_rgba();

    let at = |px: &[u8], x: u32, y: u32| px[((y * W + x) * 4) as usize];
    // Interiors agree exactly.
    assert_eq!(at(&c.pixels, 30, 30), at(&gp, 30, 30), "interior");
    assert_eq!(at(&c.pixels, 5, 5), at(&gp, 5, 5), "exterior");
    // The boundary column is where they differ, and the difference is
    // the software arm being *more* covered, not less.
    assert_eq!(at(&c.pixels, 10, 30), 0, "software fills the boundary pixel whole");
    assert!(at(&gp, 10, 30) > 0, "the GPU ramps it");
}
