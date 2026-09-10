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
/// short — and so **every case actually splits**, in the full render and
/// in the damaged one.
///
/// **The previous list did not, and said it did** (fixed 2026-09-10).
/// It was `(640,480), (641,481), (800,601), (1024,769), (1920,1080)`
/// with a comment claiming the surface was "large enough to cross the
/// splitter's threshold at every one of them". Measured: **six of the
/// ten cases never split** — three sizes, in both their full and damaged
/// variants — so the cross-build checksum compared a single-band render
/// with itself and would have agreed however badly the splitter was
/// broken. `AGENT.md`'s first vacuous shape; the seventh instance found
/// on this repository.
///
/// The claim was wrong by a factor of two. `Canvas::bands_for`'s
/// threshold is not the 250 000 px floor in `workers_for` — that is the
/// pixels-per-*worker* quantum, and `render_banded` needs **two**
/// workers before it splits at all. So the real bar is 500 000 px, and
/// for the damaged cases it is 500 000 px *of the damage rect*, which is
/// 66% of the surface here. Every size below clears both, and
/// `a_full_render_is_identical_however_it_is_banded` now asserts it
/// rather than trusting this comment.
const SIZES: &[(u32, u32)] = &[(1281, 601), (1023, 769), (897, 1001), (1537, 853), (1920, 1080)];

/// The damage rect the damaged cases use — one place, so the test and
/// the checksum printer cannot drift apart about what they measured.
fn damage_of(w: u32, h: u32) -> [f32; 4] {
    [w as f32 * 0.13, h as f32 * 0.17, w as f32 * 0.91, h as f32 * 0.83]
}

/// Assert the frame this case is about **actually splits**.
///
/// Without this the suite's whole subject can be absent and every
/// comparison still passes, which is exactly what happened above. Prints
/// the band count beside the bound, per `AGENT.md`.
#[cfg(feature = "threads")]
fn assert_splits(w: u32, h: u32, damage: Option<[f32; 4]>, tag: &str) {
    let c = Canvas::new(w, h);
    let bands = c.bands_for(damage);
    assert!(
        bands >= 2,
        "{w}x{h}/{tag}: bands_for = {bands}, so this case never splits and compares \
         a single-band render with itself"
    );
}

#[cfg(not(feature = "threads"))]
fn assert_splits(_: u32, _: u32, _: Option<[f32; 4]>, _: &str) {}

#[test]
fn a_full_render_is_identical_however_it_is_banded() {
    for &(w, h) in SIZES {
        assert_splits(w, h, None, "full");
        let got = render(w, h, None);
        // Within one binary both paths are the same build, so this half
        // checks determinism and ink; the *banded vs single-band*
        // comparison is the cross-build checksum below, which
        // `scripts/check_banded.sh` runs with and without `threads`.
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
        let d = damage_of(w, h);
        assert_splits(w, h, Some(d), "damaged");
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
/// **A list with a backdrop filter in it must not be banded** —
/// M19/T19.5.
///
/// A band owns disjoint rows; `Cmd::Glass` reads `3σ + 4` px past its
/// panel on every side, into rows another worker owns. The renderer
/// declines, and this asserts the decline rather than trusting it — a
/// splitter that silently started threading a glass frame would produce
/// a wrong picture only under load, which is the worst way to find out.
///
/// Only meaningful with `threads`: without it nothing bands, so a
/// decline is indistinguishable from the default and the control below
/// cannot hold.
#[cfg(feature = "threads")]
#[test]
fn a_list_with_glass_is_not_banded() {
    let (w, h) = (1920u32, 1080u32);
    let c = Canvas::new(w, h);
    // The same geometry without glass *does* band, so this is a
    // statement about the command and not about the size.
    assert!(c.bands_for(None) >= 2, "the control does not band; this proves nothing");

    let plain = scene(w, h);
    assert!(c.bands_for_list(&plain, None) >= 2, "the same scene without glass must band");

    let mut l = scene(w, h);
    l.glass([100.0, 100.0, 500.0, 200.0], 8.0, 12.0, Color(255, 255, 255, 40));
    let bands = c.bands_for_list(&l, None);
    assert_eq!(bands, 1, "a list with a backdrop filter was banded into {bands}");
}

#[test]
fn print_checksums_for_cross_build_comparison() {
    let mut out = String::new();
    for &(w, h) in SIZES {
        for (tag, d) in [
            ("full", None),
            ("damaged", Some(damage_of(w, h))),
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
