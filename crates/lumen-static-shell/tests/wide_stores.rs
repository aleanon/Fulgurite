//! M17/T17.0: the wide-store paths must be **byte-identical** to the
//! naive loops they replaced.
//!
//! `fill_px` writes sixteen bytes at a time and mops up a remainder four
//! at a time; `blit_0rgb` reads four pixels at a time and mops up the
//! same way. Both are pure speed rewrites of code whose output the whole
//! rendering gate depends on — `damage_oracle.rs` compares pixels,
//! `clip_test.rs` pins them, and the live gate reads them back off the X
//! server. A remainder bug here would be a stripe of wrong pixels down
//! the right edge of every frame at some widths and not others.
//!
//! So the reference implementations are written out here and compared
//! exhaustively over widths that exercise every remainder case: a canvas
//! row is `w * 4` bytes, so `w % 4` decides what is left over after the
//! sixteen-byte stride.

use lumen_static::display::Color;
use lumen_static_shell::software::Canvas;

/// The loop `clear` and `span` used before T17.0.
fn reference_fill(dst: &mut [u8], p: [u8; 4]) {
    for px in dst.chunks_exact_mut(4) {
        px.copy_from_slice(&p);
    }
}

/// The loop `blit_0rgb` used before T17.0.
fn reference_blit(pixels: &[u8], dst: &mut [u32]) {
    for (d, px) in dst.iter_mut().zip(pixels.chunks_exact(4)) {
        *d = (px[0] as u32) << 16 | (px[1] as u32) << 8 | px[2] as u32;
    }
}

/// Widths chosen so `w % 4` covers 0..3 — the four remainder shapes —
/// at both a size that fits in cache and one that does not.
const WIDTHS: &[u32] = &[1, 2, 3, 4, 5, 7, 8, 15, 16, 17, 31, 33, 63, 65, 440, 441, 442, 443];

#[test]
fn clear_is_byte_identical_at_every_remainder() {
    for &w in WIDTHS {
        for h in [1u32, 3, 7] {
            let mut got = Canvas::new(w, h);
            got.clear(Color(11, 22, 33, 44));

            let mut want = vec![0u8; (w * h * 4) as usize];
            reference_fill(&mut want, [11, 22, 33, 44]);

            assert_eq!(got.pixels, want, "clear differs at {w}x{h} (w%4 = {})", w % 4);
        }
    }
}

#[test]
fn a_partial_span_is_byte_identical_at_every_offset() {
    // `span` fills a sub-slice, so the sixteen-byte stride starts at an
    // offset the caller chose rather than at zero. Every start and end
    // within a row is exercised.
    for &w in &[1u32, 4, 7, 16, 17, 33] {
        for x0 in 0..w {
            for x1 in x0..=w {
                let mut got = Canvas::new(w, 1);
                got.clear(Color(0, 0, 0, 255));
                let mut want = got.pixels.clone();

                // An opaque fill of [x0, x1) on row 0, through the
                // public path that uses `span`.
                got.clear_within([x0 as f32, 0.0, x1 as f32, 1.0], Color(9, 8, 7, 255));
                reference_fill(
                    &mut want[(x0 * 4) as usize..(x1 * 4) as usize],
                    [9, 8, 7, 255],
                );

                assert_eq!(got.pixels, want, "span differs at w={w} [{x0},{x1})");
            }
        }
    }
}

#[test]
fn blit_is_byte_identical_at_every_remainder() {
    for &w in WIDTHS {
        for h in [1u32, 3] {
            let mut c = Canvas::new(w, h);
            for (i, px) in c.pixels.chunks_exact_mut(4).enumerate() {
                px.copy_from_slice(&[(i % 251) as u8, (i % 241) as u8, (i % 239) as u8, 255]);
            }
            let n = (w * h) as usize;

            let mut got = vec![0u32; n];
            c.blit_0rgb(&mut got);
            let mut want = vec![0u32; n];
            reference_blit(&c.pixels, &mut want);

            assert_eq!(got, want, "blit differs at {w}x{h} (px%4 = {})", n % 4);
        }
    }
}

#[test]
fn a_rendered_frame_is_byte_identical_to_the_reference_paths() {
    // The end-to-end check: a frame with opaque fills, rounded corners,
    // a shadow and a gradient, rasterised through the wide paths, must
    // equal the same frame with `clear` and the blit done naively. This
    // is the one that would catch a `span` remainder bug showing up only
    // through `fill_rounded_within`'s straight rows.
    let (w, h) = (203u32, 97u32); // deliberately not a multiple of 4
    let mut list = lumen_static::DisplayList::new();
    list.rect([3.0, 5.0, 190.0, 20.0], Color(200, 60, 60, 255));
    list.rounded_rect([7.0, 25.0, 180.0, 70.0], Color(40, 90, 200, 255), 9.0);
    list.shadow([20.0, 30.0, 150.0, 60.0], 6.0, 5.0);
    list.gradient(
        [5.0, 72.0, 195.0, 92.0],
        lumen_static::display::Stops::two(Color(255, 200, 0, 255), Color(0, 120, 255, 255)),
        0.0,
    );

    let mut a = Canvas::new(w, h);
    a.clear(Color(250, 250, 250, 255));
    lumen_static::Renderer::render(&mut a, &list, w, h);

    let mut b = Canvas::new(w, h);
    reference_fill(&mut b.pixels, [250, 250, 250, 255]);
    lumen_static::Renderer::render(&mut b, &list, w, h);

    assert_eq!(a.pixels, b.pixels, "the rasterised frame differs");

    let n = (w * h) as usize;
    let mut ga = vec![0u32; n];
    a.blit_0rgb(&mut ga);
    let mut gb = vec![0u32; n];
    reference_blit(&b.pixels, &mut gb);
    assert_eq!(ga, gb, "the blitted frame differs");
}

/// The loop `blit_0rgb_within` used before T17.3.
fn reference_blit_within(
    pixels: &[u8],
    w: u32,
    h: u32,
    dst: &mut [u32],
    r: [f32; 4],
) {
    let (iw, ih) = (w as i64, h as i64);
    let x0 = (r[0].floor() as i64).clamp(0, iw) as usize;
    let y0 = (r[1].floor() as i64).clamp(0, ih) as usize;
    let x1 = (r[2].ceil() as i64).clamp(0, iw) as usize;
    let y1 = (r[3].ceil() as i64).clamp(0, ih) as usize;
    let stride = w as usize;
    for y in y0..y1 {
        let row = y * stride;
        for x in x0..x1 {
            let i = (row + x) * 4;
            let px = &pixels[i..i + 4];
            dst[row + x] = (px[0] as u32) << 16 | (px[1] as u32) << 8 | px[2] as u32;
        }
    }
}

#[test]
fn the_damaged_blit_is_byte_identical_at_every_run_length() {
    // The path a real frame actually calls — `blit_0rgb_within` runs
    // every damaged frame while the whole-surface blit runs on a resize.
    // T17.2 vectorised the wrong one of the two, which is why this test
    // exists (T17.3).
    for &w in &[7u32, 8, 16, 17, 33, 64, 71] {
        for h in [1u32, 5] {
            let mut c = Canvas::new(w, h);
            for (i, px) in c.pixels.chunks_exact_mut(4).enumerate() {
                px.copy_from_slice(&[(i % 251) as u8, (i % 241) as u8, (i % 239) as u8, 255]);
            }
            let n = (w * h) as usize;
            // Every start and end column, so every eight-lane tail and
            // every sub-lane run is covered.
            for x0 in 0..w {
                for x1 in x0..=w {
                    let rect = [x0 as f32, 0.0, x1 as f32, h as f32];
                    let mut got = vec![0u32; n];
                    c.blit_0rgb_within(&mut got, rect);
                    let mut want = vec![0u32; n];
                    reference_blit_within(&c.pixels, w, h, &mut want, rect);
                    assert_eq!(
                        got, want,
                        "damaged blit differs at {w}x{h} cols [{x0},{x1}) \
                         (run = {}, simd = {})",
                        x1 - x0,
                        cfg!(feature = "simd")
                    );
                }
            }
        }
    }
}

/// Both vector blit branches are compiled in and were invoked —
/// M50/T50.5, R18-F10. The pixels above are identical either way, so
/// only a count can say which path ran (`software::vector_path_counts`).
#[cfg(feature = "raster-counters")]
#[test]
fn the_vector_blit_paths_ran() {
    use lumen_static_shell::software::vector_path_counts;
    let before = vector_path_counts();
    let c = Canvas::new(33, 5);
    let mut dst = vec![0u32; 33 * 5];
    c.blit_0rgb(&mut dst);
    c.blit_0rgb_within(&mut dst, [3.0, 0.0, 30.0, 5.0]);
    let after = vector_path_counts();
    let (whole, run) = (after[0] - before[0], after[1] - before[1]);
    let compiled = cfg!(feature = "simd");
    println!("blit_0rgb: vector path compiled: {compiled}, invoked {whole} times; blit_run: invoked {run} times");
    if compiled {
        assert!(whole > 0, "vector path not compiled: blit_0rgb's simd branch was invoked 0 times");
        assert!(run > 0, "vector path not compiled: blit_run's simd branch was invoked 0 times");
    } else {
        assert_eq!((whole, run), (0, 0), "no simd feature, yet a vector branch counted");
    }
}
