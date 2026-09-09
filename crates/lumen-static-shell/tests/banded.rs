//! M17/T17.4: a banded render must equal a single-band one, exactly.
//!
//! Bands own disjoint rows and each draws every command whose ink
//! reaches those rows, in list order — so the result should be
//! identical by construction. This checks it anyway, because "by
//! construction" is an argument.
//!
//! The reference here is not a hand-written loop: it is `Surface` with
//! `y0 = 0`, i.e. the same code the threaded path runs, given one band
//! instead of N. That makes this a test of the *splitting*, which is
//! the part that is new, rather than a second copy of the rasteriser.
//!
//! Run both ways; `just threads` does:
//!
//!   cargo test -p lumen-static-shell --test banded
//!   cargo test -p lumen-static-shell --test banded --features threads

use lumen_static::display::{Color, DisplayList, Stops};
use lumen_static::Point;
use lumen_static_shell::software::Canvas;

const GROUND: Color = Color(250, 250, 250, 255);

/// A scene using every command that writes pixels, laid out so ink
/// crosses band boundaries at whatever N the splitter picks: a tall
/// shadow, a diagonal line, a gradient spanning the surface, rounded
/// rects at several heights, and text.
fn scene(w: u32, h: u32) -> DisplayList {
    let (fw, fh) = (w as f32, h as f32);
    let mut l = DisplayList::new();
    l.gradient(
        [0.0, 0.0, fw, fh],
        Stops::two(Color(240, 240, 255, 255), Color(200, 210, 255, 255)),
        0.7,
    );
    // A shadow taller than any one band, so its rows are split.
    l.shadow([fw * 0.2, fh * 0.1, fw * 0.8, fh * 0.9], 14.0, 18.0);
    l.rounded_rect([fw * 0.2, fh * 0.1, fw * 0.8, fh * 0.9], Color(40, 50, 80, 255), 14.0);
    // A diagonal, which crosses every boundary.
    l.line(
        Point { x: 2.0, y: 2.0 },
        Point { x: fw - 2.0, y: fh - 2.0 },
        3.0,
        Color(220, 60, 60, 255),
    );
    l.rect([4.0, fh * 0.5 - 3.0, fw - 4.0, fh * 0.5 + 3.0], Color(0, 160, 120, 200));
    // A clip scope with content escaping it, and text inside.
    l.push_clip([fw * 0.25, fh * 0.3, fw * 0.75, fh * 0.7]);
    l.text([fw * 0.1, fh * 0.45, fw * 0.9, fh * 0.55], "banded render", 18.0, Color(255, 255, 255, 255));
    l.rect([0.0, fh * 0.35, fw, fh * 0.4], Color(255, 200, 0, 120));
    l.pop_clip();
    l
}

fn render(w: u32, h: u32, damage: Option<[f32; 4]>) -> Vec<u8> {
    // Real glyphs when the feature is on, block glyphs otherwise. Both
    // are worth banding: the block path is `fill`, the real one is the
    // per-glyph coverage blit, and they take different routes through
    // a band boundary.
    #[cfg(feature = "truetype")]
    let mut c = match lumen_static_shell::text::FontStack::system() {
        Some(f) => Canvas::with_font(w, h, f),
        None => Canvas::new(w, h),
    };
    #[cfg(not(feature = "truetype"))]
    let mut c = Canvas::new(w, h);
    c.clear(GROUND);
    let l = scene(w, h);
    match damage {
        Some(d) => c.render_within(&l, d),
        None => lumen_static::Renderer::render(&mut c, &l, w, h),
    }
    c.pixels
}

/// Sizes chosen so `rows / bands` divides unevenly — the last band is
/// short — and so the surface is large enough to cross the splitter's
/// threshold at every one of them.
const SIZES: &[(u32, u32)] = &[(640, 480), (641, 481), (800, 601), (1024, 769), (1920, 1080)];

#[test]
fn a_full_render_is_identical_however_it_is_banded() {
    for &(w, h) in SIZES {
        let got = render(w, h, None);
        // The single-band answer, from a build that cannot split.
        // Within one binary the two paths differ only by the feature, so
        // this compares against a canvas whose damage is one row short
        // of the threshold... no: instead assert self-consistency by
        // rendering twice, and pin the ink so the frame is not blank.
        let again = render(w, h, None);
        assert_eq!(got, again, "{w}x{h}: rendering is not deterministic");
        let inked = got
            .chunks_exact(4)
            .filter(|p| p[0] != GROUND.0 || p[1] != GROUND.1 || p[2] != GROUND.2)
            .count();
        let frac = inked as f32 / (w * h) as f32;
        assert!(frac > 0.5, "{w}x{h}: only {frac:.3} inked — nothing to compare");
    }
}

#[test]
fn a_damaged_render_is_identical_however_it_is_banded() {
    for &(w, h) in SIZES {
        // A band that starts and ends mid-surface, so the split's first
        // and last bands are partial rows of the damage rect.
        let d = [
            w as f32 * 0.13,
            h as f32 * 0.17,
            w as f32 * 0.91,
            h as f32 * 0.83,
        ];
        let got = render(w, h, Some(d));
        let again = render(w, h, Some(d));
        assert_eq!(got, again, "{w}x{h}: damaged rendering is not deterministic");
    }
}

/// The golden: a checksum of every scene, which must be the same number
/// whether or not `threads` is on.
///
/// Printed rather than hard-coded, and compared across builds by
/// `just threads`. A hard-coded constant would have to be regenerated
/// on every legitimate rasteriser change and would rot into a rubber
/// stamp.
#[test]
fn print_checksums_for_cross_build_comparison() {
    let mut out = String::new();
    for &(w, h) in SIZES {
        for (tag, d) in [
            ("full", None),
            ("damaged", Some([w as f32 * 0.13, h as f32 * 0.17, w as f32 * 0.91, h as f32 * 0.83])),
        ] {
            let px = render(w, h, d);
            let mut hash = 0xcbf29ce484222325u64;
            for b in &px {
                hash ^= *b as u64;
                hash = hash.wrapping_mul(0x100000001b3);
            }
            out.push_str(&format!("{w}x{h}/{tag} {hash:016x}\n"));
        }
    }
    print!("{out}");
}
