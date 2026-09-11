//! M22/T22.6: the backdrop filter on both backends.
//!
//! `tests/parity.rs` cannot absorb this and `docs/plan-liquid.md` §8
//! says why. As it stood the gate would have passed with the feature
//! absent: its frame has no glass in it at all; the frosted preset has
//! no rim, so skipping the shader's dispersion would be a no-op on both
//! sides; its comparison ignores per-pixel deltas under 8 levels, which
//! a fringe over a smooth backdrop does not exceed; and a bound "stated
//! with the measurement beside it" is a bound chosen after seeing the
//! number.
//!
//! So the scene uses the **liquid** preset with dispersion and adapt
//! explicitly non-zero over a **hard, saturated colour edge** placed
//! where the refracted reads actually land; the CPU side asserts the
//! fringe exists *before* the backends are compared; and there are two
//! bounds, because one frame-wide count cannot both admit the blur's
//! disagreement and refuse a missing fringe.
//!
//! # The bounds, stated before the measurement
//!
//! **B1, the whole panel: at most 10% of its pixels may differ by more
//! than 8 levels.** Loose on purpose. The GPU blurs at full resolution
//! where the CPU downsamples, and each of the six box passes lands in an
//! `Rgba8Unorm` attachment where the CPU carries `u16` intermediates, so
//! a few levels of rounding accumulate across a panel-sized area.
//!
//! **B2, the rim band: the two backends' colour fringe may differ by at
//! most 8 levels on average.** A shader that skipped dispersion must not
//! sit under this. That is the whole point of a second bound: B1 absorbs
//! a missing fringe without noticing, because the rim band is a small
//! fraction of the panel — measured, deleting dispersion from the shader
//! moves B1 to 1.2% against its 10%, comfortably green.
//!
//! **What the fringe is measured *as* took two attempts, and the first
//! was of the wrong quantity.** Blue-minus-red across the rim band is
//! dominated by the backdrop's own colour — over this scene's blue and
//! yellow it reads +110 levels whether or not the renderer disperses
//! anything, and deleting dispersion moved it by 3.3, under the bound.
//! The bound was not the problem and has not moved. The quantity has:
//! the fringe is now the *difference* the same panel makes with
//! `dispersion: 255` against `dispersion: 0`, on each backend
//! separately, which is dispersion's contribution and nothing else.

use lumen_static::display::{Color, DisplayList, Material, Renderer};
use lumen_static_shell::software::Canvas;
use lumen_static_wgpu::Gpu;

const W: u32 = 320;
const H: u32 = 240;
const PAGE: Color = Color(250, 250, 250, 255);
const PANEL: [f32; 4] = [60.0, 60.0, 260.0, 180.0];

/// At most this fraction of the panel's pixels may differ by more than
/// [`SOFT`] levels.
const B1: f32 = 0.10;
const SOFT: u8 = 8;
/// Mean absolute difference between the backends' fringes, in levels.
const B2: f32 = 8.0;
/// What the CPU's own fringe must reach before the comparison means
/// anything.
const FRINGE_FLOOR: f32 = 15.0;

/// The rim band on the panel's left edge, where dispersion lives.
///
/// **Four columns, and which four is measured rather than assumed.**
/// The split is `refraction × u² × 0.25`, largest where `u` is 1 and
/// gone a few pixels in: profiled across this scene it is 32, 21, 5 and
/// 15 levels in columns 60–63 and **exactly zero** from 65 onwards. The
/// first version of this band ran 61..67, so half of it was dead
/// columns pulling the mean down — the same mistake T22.0's Fresnel test
/// made, and for the same reason.
const RIM_X: core::ops::Range<usize> = 60..64;
const RIM_Y: core::ops::Range<usize> = 90..150;

fn material() -> Material {
    let m = Material { dispersion: 255, ..Material::LIQUID };
    assert!(m.rim > 0.0, "a material with no rim has no bevel and no fringe");
    assert!(m.dispersion > 0, "dispersion is off; the rim-band bound would gate nothing");
    assert!(m.adapt > 0, "adapt is off; the panel's tint would not depend on the backdrop");
    m
}

fn frame(dispersion: u8) -> DisplayList {
    let mut l = DisplayList::new();
    // **A hard, saturated colour edge where the reads land.** Refraction
    // displaces the left rim's read *outward* by up to `refraction` px,
    // so a pixel at x = 61 reads near x = 50: an edge under the middle
    // of the panel would never be sampled by the rim, and the fringe
    // would measure zero on a correct renderer.
    //
    // **Bright under the panel, dark outside it.** The panel's mean
    // luminance is what drives the adaptive tint, and over a dark
    // backdrop `adapt` sits on the flat end of its curve where deleting
    // it from the shader changes 4 pixels. Here it is engaged, and
    // deleting it moves the whole panel.
    l.rect([0.0, 0.0, 52.0, H as f32], Color(0x10, 0x30, 0xa0, 0xff));
    l.rect([52.0, 0.0, W as f32, H as f32], Color(0xff, 0xf0, 0x30, 0xff));
    l.glass(PANEL, 28.0, Material { dispersion, ..material() });
    l
}

fn gpu() -> Option<Gpu> {
    match Gpu::headless(W, H) {
        Some(g) => {
            eprintln!("gpu: {}", g.adapter());
            Some(g)
        }
        None => {
            eprintln!("gpu: no adapter — skipping (a supported configuration)");
            None
        }
    }
}

fn at(px: &[u8], x: usize, y: usize) -> [f32; 3] {
    let i = (y * W as usize + x) * 4;
    [px[i] as f32, px[i + 1] as f32, px[i + 2] as f32]
}

/// Blue-over-red at every pixel of the rim band — the backdrop's own
/// colour included, which is why this is only ever used as a
/// *difference*.
fn split(px: &[u8]) -> Vec<f32> {
    let mut v = Vec::new();
    for y in RIM_Y {
        for x in RIM_X {
            let c = at(px, x, y);
            v.push(c[2] - c[0]);
        }
    }
    v
}

/// **What dispersion contributed**, per pixel: the same panel with the
/// field at 255 against the same panel with it at 0.
///
/// Per pixel and not as a mean, because the mean of a *signed* fringe
/// cancels — the first version of this measure read −1.2 levels over a
/// fringe that is plainly there, since the fringe is one sign on one
/// side of an edge and the other sign on the other.
fn contribution(on: &[u8], off: &[u8]) -> Vec<f32> {
    split(on).iter().zip(split(off).iter()).map(|(a, b)| a - b).collect()
}

fn mean_abs(v: &[f32]) -> f32 {
    v.iter().map(|x| x.abs()).sum::<f32>() / v.len() as f32
}

#[test]
fn the_backdrop_filter_agrees_on_both_backends() {
    let Some(mut g) = gpu() else { return };
    let soft = |d: u8| {
        let mut c = Canvas::new(W, H);
        c.clear(PAGE);
        Renderer::render(&mut c, &frame(d), W, H);
        c
    };
    let cpu_off = soft(0);
    let c = soft(255);
    let mut hard = |d: u8| {
        g.clear(PAGE);
        g.render(&frame(d), W, H);
        g.read_rgba()
    };
    let gpu_off = hard(0);
    let px = hard(255);

    // **The fringe exists before anything is compared**, and it is
    // dispersion's own contribution: the same panel with the field at
    // 255 against the same panel with it at 0.
    let cpu_c = contribution(&c.pixels, &cpu_off.pixels);
    let cpu_f = mean_abs(&cpu_c);
    println!("cpu fringe {cpu_f:.1} levels mean |chroma shift| (floor {FRINGE_FLOOR})");
    assert!(
        cpu_f > FRINGE_FLOOR,
        "the software arm has no fringe to compare ({cpu_f:.1}); the scene gates nothing"
    );

    // B1: the panel as a whole.
    let (mut over, mut n, mut worst) = (0usize, 0usize, 0u8);
    for y in PANEL[1] as usize..PANEL[3] as usize {
        for x in PANEL[0] as usize..PANEL[2] as usize {
            let i = (y * W as usize + x) * 4;
            let d = (0..3).map(|k| px[i + k].abs_diff(c.pixels[i + k])).max().unwrap();
            worst = worst.max(d);
            if d > SOFT {
                over += 1;
            }
            n += 1;
        }
    }
    let frac = over as f32 / n as f32;
    println!("B1: {over} of {n} panel px over {SOFT} ({:.1}%, bound {:.0}%), worst {worst}",
        frac * 100.0, B1 * 100.0);

    // B2: the two backends' dispersion contributions, pixel by pixel.
    //
    // **Not a comparison of magnitudes.** Two fringes of the same size
    // on opposite sides of the edge would agree on any summary
    // statistic, and the ordering is what makes dispersion a lens rather
    // than colour noise. A shader that skips dispersion contributes
    // zero, so this reads back the software arm's own fringe — which is
    // asserted above to exceed the bound.
    let gpu_c = contribution(&px, &gpu_off);
    let gap =
        mean_abs(&gpu_c.iter().zip(cpu_c.iter()).map(|(a, b)| a - b).collect::<Vec<_>>());
    println!("B2: gpu fringe {:.1}, gap from cpu {gap:.1} (bound {B2})", mean_abs(&gpu_c));

    assert!(frac <= B1, "the panel disagrees on {:.1}% of its pixels", frac * 100.0);
    assert!(gap <= B2, "the fringes differ by {gap:.1} levels");
}
