//! M17/T17.1: the shadow must rasterise identically however it is
//! computed.
//!
//! The signed-distance field is evaluated scalar by default and eight
//! lanes wide under `feature = "simd"`. The vector form is meant to be
//! byte-identical, not merely close: the `as u8` cast and the blend stay
//! scalar, so only the *order* of exactly-specified `f32` operations
//! differs.
//!
//! That argument is worth exactly nothing without a check, so this file
//! pins the rendered pixels against a reference implementation written
//! out here — the loop `shadow_within` had before T17.1, per-pixel
//! `blend` and all. Run it both ways:
//!
//!   cargo test -p fulgurite-shell --test simd_shadow
//!   cargo test -p fulgurite-shell --test simd_shadow --features simd
//!
//! `just check` runs both.
//!
//! **And that the vector path ran** (M50/T50.5, R18-F10). The scalar path
//! produces exactly these pixels, so compiling the vector branch out could
//! not turn anything above red: the review gated it on AVX-512, which
//! the default x86-64 target lacks, and `just simd` stayed green. With
//! the test-only `raster-counters` feature, which `just simd` turns on,
//! `the_vector_shadow_path_ran` reads how many times the branch ran.

use fulgurite::display::{Color, DisplayList};
use fulgurite_shell::software::Canvas;

/// `shadow_within`'s loop as it stood before T17.1: per-pixel signed
/// distance, per-pixel `blend` with its own bounds check.
fn reference_shadow(
    w: u32,
    h: u32,
    shape: [f32; 4],
    c: Color,
    radius: f32,
    spread: f32,
    ground: Color,
) -> Vec<u8> {
    let mut px = vec![0u8; (w * h * 4) as usize];
    for p in px.chunks_exact_mut(4) {
        p.copy_from_slice(&[ground.0, ground.1, ground.2, ground.3]);
    }
    let bounds = [shape[0] - spread, shape[1] - spread, shape[2] + spread, shape[3] + spread];
    let rad = radius.clamp(0.0, (shape[2] - shape[0]).min(shape[3] - shape[1]) / 2.0);
    let (cx, cy) = ((shape[0] + shape[2]) / 2.0, (shape[1] + shape[3]) / 2.0);
    let (hx, hy) = ((shape[2] - shape[0]) / 2.0 - rad, (shape[3] - shape[1]) / 2.0 - rad);
    let (iw, ih) = (w as i64, h as i64);
    let (x0, y0) = ((bounds[0].floor() as i64).max(0), (bounds[1].floor() as i64).max(0));
    let (x1, y1) = ((bounds[2].ceil() as i64).min(iw), (bounds[3].ceil() as i64).min(ih));
    let skip = rad >= 1.0;
    let (ix0, iy0) = ((shape[0] + rad).ceil() as i64, (shape[1] + rad).ceil() as i64);
    let (ix1, iy1) = ((shape[2] - rad).floor() as i64, (shape[3] - rad).floor() as i64);

    let blend = |px: &mut Vec<u8>, x: i64, y: i64, c: Color| {
        if x < 0 || y < 0 || x >= iw || y >= ih || c.3 == 0 {
            return;
        }
        let i = ((y as u32 * w + x as u32) * 4) as usize;
        let a = c.3 as u32;
        let inv = 255 - a;
        for (k, src) in [c.0, c.1, c.2].into_iter().enumerate() {
            px[i + k] = ((src as u32 * a + px[i + k] as u32 * inv) / 255) as u8;
        }
        px[i + 3] = 255;
    };

    for y in y0..y1 {
        let qy = (y as f32 + 0.5 - cy).abs() - hy;
        let inner_row = skip && y >= iy0 && y < iy1;
        let mut x = x0;
        while x < x1 {
            if inner_row && x >= ix0 && x < ix1 {
                x = ix1;
                continue;
            }
            let qx = (x as f32 + 0.5 - cx).abs() - hx;
            let (ax, ay) = (qx.max(0.0), qy.max(0.0));
            let d = (ax * ax + ay * ay).sqrt() + qx.max(qy).min(0.0) - rad;
            if d >= -1.0 {
                let t = 1.0 - (d / spread).clamp(0.0, 1.0);
                let a = (c.3 as f32 * t * t) as u8;
                if a > 0 {
                    blend(&mut px, x, y, Color(c.0, c.1, c.2, a));
                }
            }
            x += 1;
        }
    }
    px
}

/// `DisplayList::shadow` offsets and colours the shadow itself, so the
/// reference has to be fed what the command actually carries.
fn shadow_cmd(rect: [f32; 4], spread: f32) -> ([f32; 4], Color) {
    let r = [rect[0], rect[1] + spread * 0.35, rect[2], rect[3] + spread * 0.35];
    (r, Color(0, 0, 0, 80))
}

const GROUND: Color = Color(250, 250, 250, 255);

/// Sizes and shapes chosen so the eight-lane stride lands differently
/// each time: runs shorter than a lane, runs with every possible tail,
/// shadows clipped by the surface on each edge, and a radius large
/// enough that the interior hole splits every row into two runs.
#[test]
fn every_shape_matches_the_reference_pixel_for_pixel() {
    let cases: &[(u32, u32, [f32; 4], f32, f32)] = &[
        // (w, h, rect, radius, spread)
        (40, 30, [10.0, 8.0, 30.0, 22.0], 4.0, 5.0),
        (41, 31, [10.0, 8.0, 30.0, 22.0], 4.0, 5.0),
        (42, 29, [10.5, 8.5, 30.5, 22.5], 4.0, 5.0),
        (43, 33, [11.0, 9.0, 29.0, 21.0], 8.0, 12.0),
        // clipped off the left and top edges
        (40, 30, [-8.0, -6.0, 20.0, 18.0], 6.0, 9.0),
        // clipped off the right and bottom edges
        (40, 30, [22.0, 16.0, 60.0, 50.0], 6.0, 9.0),
        // a run narrower than one lane
        (12, 12, [4.0, 4.0, 8.0, 8.0], 1.0, 2.0),
        // radius 0: no interior hole, one run per row
        (37, 21, [6.0, 4.0, 30.0, 17.0], 0.0, 6.0),
        // a big spread, so the reach dwarfs the shape
        (80, 60, [30.0, 24.0, 50.0, 36.0], 5.0, 24.0),
    ];

    for &(w, h, rect, radius, spread) in cases {
        let mut list = DisplayList::new();
        list.shadow(rect, radius, spread);
        assert_eq!(list.cmds.len(), 1, "the command must have been recorded");

        let mut got = Canvas::new(w, h);
        got.clear(GROUND);
        fulgurite::Renderer::render(&mut got, &list, w, h);

        let (r, c) = shadow_cmd(rect, spread);
        let want = reference_shadow(w, h, r, c, radius, spread, GROUND);

        let diff = got.pixels.iter().zip(&want).filter(|(a, b)| a != b).count();
        assert_eq!(
            diff, 0,
            "{w}x{h} rect={rect:?} r={radius} spread={spread}: {diff} bytes differ \
             (simd = {})",
            cfg!(feature = "simd")
        );
    }
}

#[test]
fn the_shadow_actually_draws_something() {
    // Without this the test above would pass on two blank canvases, which
    // is the shape of vacuous test AGENT.md's rule exists for.
    let (w, h) = (80u32, 60u32);
    let mut list = DisplayList::new();
    list.shadow([30.0, 24.0, 50.0, 36.0], 5.0, 24.0);
    let mut c = Canvas::new(w, h);
    c.clear(GROUND);
    fulgurite::Renderer::render(&mut c, &list, w, h);
    let inked = c
        .pixels
        .chunks_exact(4)
        .filter(|p| p[0] != GROUND.0 || p[1] != GROUND.1 || p[2] != GROUND.2)
        .count();
    let frac = inked as f32 / (w * h) as f32;
    assert!(frac > 0.15, "the shadow inked {frac:.3} of the canvas — nothing to compare");
}

/// The vector branch is compiled in and was invoked — M50/T50.5.
#[cfg(feature = "raster-counters")]
#[test]
fn the_vector_shadow_path_ran() {
    let before = fulgurite_shell::software::vector_path_counts()[2];
    let mut list = DisplayList::new();
    list.shadow([30.0, 24.0, 50.0, 36.0], 5.0, 24.0);
    let mut c = Canvas::new(80, 60);
    c.clear(GROUND);
    fulgurite::Renderer::render(&mut c, &list, 80, 60);
    let n = fulgurite_shell::software::vector_path_counts()[2] - before;
    let compiled = cfg!(feature = "simd");
    println!("shadow_run: vector path compiled: {compiled}, invoked {n} times");
    if compiled {
        assert!(n > 0, "vector path not compiled: shadow_run's simd branch was invoked 0 times");
    } else {
        assert_eq!(n, 0, "no simd feature, yet a vector branch counted {n}");
    }
}
