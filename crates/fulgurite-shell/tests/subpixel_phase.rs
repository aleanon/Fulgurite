//! M81/T81.7 and T81.8 — subpixel pens: can fontdue 0.9.4's **public** API
//! give a glyph drawn a third of a pixel to the right, and does the shipped
//! paint draw one? (`docs/plan-shaped-paint.md`, "Subpixel pens (M81b) are
//! thirds", K81d.)
//!
//! fontdue has no public pen offset: metrics_raw's offset is private and
//! always 0.0. Its public `rasterize_indexed_subpixel` draws the outline at
//! 3× horizontal resolution, so three summed sub-columns are one pixel's
//! coverage, and a shift by `k` sub-columns moves the outline `k/3` px. The
//! catch is that it draws the 3× outline with the 1× `offset_x` (the
//! fraction of the glyph's left bearing, `fract(bounds.xmin)`) **unscaled**,
//! so the 3× outline sits `offset_x` sub-columns, not `3·offset_x`, from its
//! column 0: `2/3·offset_x` px left of where the 1× bitmap has it. The
//! derivation shifts by `round(2·offset_x) + k` sub-columns, which corrects
//! that to within half a sub-column, 1/6 px.
//!
//! Since T81.8 the derivation is the shipped `FontStack::rasterize` for a
//! phase ≥ 1, and [`derive`] reaches it the way paint does: a shaped run's
//! placement through `text::pen_of`. K81d reads this test's output: its
//! bound is the absolute, relative and coverage checks below, for every
//! probed glyph. The relative check measures each shift against
//! [`reference`], not against `k/3`: an area-coverage centroid is not linear
//! in a sub-pixel shift, and `l` at 16 px moves 0.426 px for a third under
//! fontdue's own rasteriser (`docs/plan-shaped-paint.md`, "What building
//! found", T81.7; the numbers are `docs/results-text.md` M81/T81.7's and
//! M81/T81.8's).
//!
//! The other tests: where phases occur at all (only where the shaper runs,
//! since K81a's response); T81.8's canvas gates, phases keying the glyph
//! cache and moving the ink, by run origin and by shaped advances; and the
//! flipped T81.7 pin, a shaped glyph's bearing fraction drawn once.
#![cfg(all(feature = "truetype", feature = "shaping"))]

#[path = "fonts/pinned.rs"]
mod pinned;

use fontdue::{Font, FontSettings};
use fulgurite::display::{Color, DisplayList, Renderer};
use fulgurite_shell::software::Canvas;
use fulgurite_shell::text::{pen_of, FontStack, Shaping};

/// The shaper counters (`text_counters`) are process-wide and libtest runs
/// a file's tests on threads, so each test here holds this while it shapes.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

/// The plan's probe set: a stem, a bowl, a wide diagonal, and a Hebrew
/// letter (`ע`, ayin), at a small, a body and a large size.
const GLYPHS: [char; 4] = ['l', 'o', 'W', 'ע'];
const SIZES: [f32; 3] = [12.0, 16.0, 32.0];

/// Phase 1 against the 1× bitmap: the decision's residual after an integer
/// sub-column correction (half a sub-column, 1/6 px), plus 0.05 px.
const ABS_BOUND: f32 = 1.0 / 6.0 + 0.05;
/// Each later phase's shift from phase 1: `k/3` within this.
const REL_TOL: f32 = 0.05;
/// Total coverage against the 1× bitmap's, as a fraction.
const COV_TOL: f32 = 0.01;
/// The setup: some probed glyph's `offset_x` reaches this, so the
/// uncorrected error (`2/3·offset_x` ≥ 0.27 px) exceeds [`ABS_BOUND`].
const SETUP_OX: f32 = 0.4;

/// Coverage in 0..=1 per pixel, column 0 at absolute x `xmin` (the
/// glyph's floored left bearing, as fontdue's Metrics::xmin).
struct Bitmap {
    xmin: i32,
    w: usize,
    h: usize,
    cov: Vec<f32>,
}

impl Bitmap {
    /// The ink's horizontal centroid, in absolute pixels from the pen.
    fn centroid(&self) -> f32 {
        let (mut m, mut t) = (0.0f64, 0.0f64);
        for y in 0..self.h {
            for x in 0..self.w {
                let v = self.cov[y * self.w + x] as f64;
                m += (self.xmin as f64 + x as f64 + 0.5) * v;
                t += v;
            }
        }
        (m / t) as f32
    }

    fn total(&self) -> f32 {
        self.cov.iter().sum()
    }
}

/// fontdue's 1× bitmap, the reference every phase is checked against.
fn one_x(font: &Font, index: u16, px: f32) -> Bitmap {
    let (m, cov) = font.rasterize_indexed(index, px);
    Bitmap { xmin: m.xmin, w: m.width, h: m.height, cov: cov.iter().map(|&c| c as f32 / 255.0).collect() }
}

/// The bearing fraction fontdue draws both rasters with: `fract(xmin)` in
/// 0..1, as fontdue's private metrics_raw computes it (recomputed here from
/// the public bounds).
fn offset_x(font: &Font, index: u16, px: f32) -> f32 {
    let xmin = font.metrics_indexed(index, px).bounds.xmin;
    xmin - xmin.floor()
}

/// The glyph moved `k/3` px right (`k` in 0..3), **from the shipped
/// raster**: a `Complex` stack places the glyph alone (a Latin one forced
/// through the shaper by a zero-width joiner), [`pen_of`] puts it at
/// `k/3` past the placement, and `FontStack::rasterize` draws the phase it
/// chose. Column 0 is the 1× bitmap's (`floor(bounds.xmin)` from a whole
/// pen, taken from fontdue's metrics rather than from the placement, whose
/// column is what `a_shaped_glyph_draws_its_bearing_fraction_once`
/// checks), so the centroids compare in one frame.
fn derive(stack: &FontStack, font: &Font, ch: char, px: f32, k: i32) -> Bitmap {
    let (placed, _) = stack.place_run(&forced(ch), px, 400.0, f32::INFINITY);
    assert_eq!(placed.len(), 1, "setup: {ch:?} at {px} px placed {} glyphs", placed.len());
    let p = placed[0];
    let index = font.lookup_glyph_index(ch);
    let xmin = font.metrics_indexed(index, px).bounds.xmin.floor();
    let pen = pen_of(xmin + k as f32 / 3.0, &p);
    assert_eq!(pen.id.key().phase, 1 + k as u8, "setup: {ch:?} at {px} px + {k}/3 took phase {}", pen.id.key().phase);
    assert_eq!(pen.x, xmin, "setup: {ch:?} at {px} px + {k}/3 moved to column {}", pen.x);
    let (w, h, cov) = stack.rasterize(pen.id).expect("the stack rasterises its own id");
    assert_eq!(w, pen.w, "pen_of's width {} is not the bitmap's {w}", pen.w);
    Bitmap { xmin: pen.x as i32, w: w as usize, h: h as usize, cov: cov.iter().map(|&c| c as f32 / 255.0).collect() }
}

/// `ch` as a run the shaper lays out: Hebrew as it is, anything else with a
/// zero-width joiner after it (inks nothing, takes no width), since a run
/// with nothing to shape takes fontdue's whole-pixel path (K81a's response).
fn forced(ch: char) -> String {
    if ch.is_ascii() {
        format!("{ch}\u{200d}")
    } else {
        ch.to_string()
    }
}

/// The relative check's reference: the same outline moved `k/3` px, drawn
/// by fontdue's 1× rasteriser at `3·px` and binned 3:1. fontdue does not
/// hint, so the outline at `3·px` is the outline at `px` scaled by three,
/// and its column 0 lies on a whole high-resolution pixel: a `k`-column
/// shift is exact, with no bearing correction. Binned by columns only,
/// which is all an x centroid and a total need. `(centroid, total)` in
/// low-resolution pixels.
fn reference(font: &Font, index: u16, px: f32, k: i32) -> (f32, f32) {
    let (m, cov) = font.rasterize_indexed(index, 3.0 * px);
    let (mut c, mut t) = (0.0f64, 0.0f64);
    for q in 0..m.width {
        let mass: f64 = (0..m.height).map(|y| cov[y * m.width + q] as f64 / 255.0).sum();
        let pixel = (m.xmin + q as i32 + k).div_euclid(3);
        c += (pixel as f64 + 0.5) * mass;
        t += mass;
    }
    ((c / t) as f32, (t / 9.0) as f32)
}

#[test]
fn thirds_from_the_subpixel_raster_are_honest_phases() {
    let _serial = serial();
    let font = Font::from_bytes(pinned::bytes(), FontSettings::default()).expect("the pinned face parses");
    let stack = complex(pinned::bytes());
    let mut failures = Vec::new();
    let mut max_ox = (0.0f32, ' ', 0.0f32);
    let (mut worst_abs, mut worst_rel, mut worst_cov, mut worst_ref, mut worst_third) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
    println!(
        "glyph    px offset_x |   1x cx  ph1 cx |abs| (≤ {ABS_BOUND:.3}) | shift ph2, ph3: derived / reference (± {REL_TOL}) | coverage ph1 ph2 ph3 / 1x"
    );
    for &ch in &GLYPHS {
        let index = font.lookup_glyph_index(ch);
        assert_ne!(index, 0, "the pinned face has no glyph for {ch:?}: the probe would measure .notdef");
        for &px in &SIZES {
            let ox = offset_x(&font, index, px);
            if ox > max_ox.0 {
                max_ox = (ox, ch, px);
            }
            let one = one_x(&font, index, px);
            let phases: Vec<Bitmap> = (0..3).map(|k| derive(&stack, &font, ch, px, k)).collect();
            let refs: Vec<(f32, f32)> = (0..3).map(|k| reference(&font, index, px, k)).collect();
            let (c1x, c_one) = (phases[0].centroid(), one.centroid());
            let abs = (c1x - c_one).abs();
            // Setup: the reference is the 1× bitmap's outline (fontdue does
            // not hint), or the relative check compares against nothing.
            let ref_err = (refs[0].0 - c_one).abs().max((refs[0].1 / one.total() - 1.0).abs());
            worst_ref = worst_ref.max(ref_err);
            if ref_err > REL_TOL {
                failures.push(format!("setup: the 3·px reference for {ch:?} at {px} px is {ref_err:.3} from the 1× bitmap"));
            }
            let covs: Vec<f32> = phases.iter().map(|b| b.total() / one.total()).collect();
            let mut shifts = String::new();
            for k in 1..3 {
                let derived = phases[k].centroid() - c1x;
                let want = refs[k].0 - refs[0].0;
                shifts += &format!(" {derived:+.3}/{want:+.3}");
                worst_rel = worst_rel.max((derived - want).abs());
                worst_third = worst_third.max((derived - k as f32 / 3.0).abs());
                if (derived - want).abs() > REL_TOL {
                    failures.push(format!(
                        "relative: {ch:?} at {px} px, phase {}: shift {derived:.3} px, the reference's {want:.3} ± {REL_TOL}",
                        k + 1
                    ));
                }
            }
            println!(
                "{ch:?} {px:>5} {ox:>8.3} | {c_one:>7.3} {c1x:>7.3} {abs:.3}           |{shifts}                     | {:.4} {:.4} {:.4}",
                covs[0], covs[1], covs[2]
            );
            worst_abs = worst_abs.max(abs);
            if abs > ABS_BOUND {
                failures.push(format!("absolute: {ch:?} at {px} px (offset_x {ox:.3}): |phase 1 − 1×| = {abs:.3} px > {ABS_BOUND:.3}"));
            }
            for (i, c) in covs.iter().enumerate() {
                worst_cov = worst_cov.max((c - 1.0).abs());
                if (c - 1.0).abs() > COV_TOL {
                    failures.push(format!("coverage: {ch:?} at {px} px, phase {}: {c:.4} of the 1× bitmap's", i + 1));
                }
            }
        }
    }
    println!(
        "setup: max offset_x {:.3} ({:?} at {} px), bound ≥ {SETUP_OX}; reference vs 1× {worst_ref:.4} (bound {REL_TOL})\n\
         worst |abs| {worst_abs:.3} (bound {ABS_BOUND:.3}); worst |shift − reference| {worst_rel:.3} (bound {REL_TOL}); \
         worst |shift − k/3| {worst_third:.3} (unbounded: box-filter centroids are not linear in a shift); \
         worst |coverage − 1| {worst_cov:.4} (bound {COV_TOL})",
        max_ox.0, max_ox.1, max_ox.2
    );
    assert!(
        max_ox.0 >= SETUP_OX,
        "setup: no probed glyph has offset_x ≥ {SETUP_OX} (max {:.3}), so a missing 2/3·offset_x correction would pass",
        max_ox.0
    );
    assert!(failures.is_empty(), "K81d: {} of the probe's checks failed:\n{}", failures.len(), failures.join("\n"));
}

/// Where fractional pens exist: what K81b's arm (T81.8) can count.
///
/// Since K81a's response a `Complex` stack lays a run with nothing to shape
/// (`text::needs_shaping` false) out by fontdue's whole-pixel path, so the
/// grid's Latin and digits have **no** fractional pen and a K81b arm on them
/// counts nothing. Runs the shaper lays out land on every third.
#[test]
fn phases_occur_only_where_the_shaper_runs() {
    let _serial = serial();
    use fulgurite::text::GlyphKey;
    use fulgurite_shell::text::text_counters;

    let bytes = pinned::bytes();
    let font = Font::from_bytes(bytes, FontSettings::default()).expect("the pinned face parses");
    let stack = complex(bytes);
    // What the grid draws (labels, numbers, a formula), and the string
    // T81.8's second case names.
    let latin = ["Total 12345", "=SUM(A1:A9)", "Wolf 3.14", "llll"];
    // What reaches the shaper: Hebrew, Hebrew with digits, a mixed line,
    // and `llll` forced through the shaper by a zero-width joiner.
    let shaped = ["שלום עולם", "סה״כ 12345", "abc שלום", "llll\u{200d}"];
    let sizes = [12.0f32, 13.0, 16.0];

    // (runs, through the shaper, glyphs, fractional pens, glyphs per third)
    let survey = |corpus: &[&str]| {
        let (mut runs, mut through, mut glyphs, mut frac, mut thirds) = (0u32, 0u64, 0u32, 0u32, [0u32; 3]);
        for &px in &sizes {
            for &text in corpus {
                let before = text_counters().complex;
                let (placed, _) = stack.place_run(text, px, 400.0, f32::INFINITY);
                let via = text_counters().complex - before;
                runs += 1;
                through += via;
                for p in &placed {
                    let key: GlyphKey = p.id.into();
                    let xmin = font.metrics_indexed(key.index, px).bounds.xmin;
                    // The pen: since T81.8 a shaped placement is
                    // `pen + floor(xmin)` (`shaped_ink`), as a fontdue one
                    // is `floor(pen + xmin)` on a whole pen.
                    let pen = p.x - xmin.floor();
                    let f = pen - pen.floor();
                    glyphs += 1;
                    if f > 1e-3 && f < 1.0 - 1e-3 {
                        frac += 1;
                    }
                    thirds[((f * 3.0).round() as usize) % 3] += 1;
                }
            }
        }
        (runs, through, glyphs, frac, thirds)
    };

    let (lr, lt, lg, lf, lth) = survey(&latin);
    let (sr, st, sg, sf, sth) = survey(&shaped);
    let seen = sth.iter().filter(|&&n| n > 0).count();
    println!(
        "grid-like Latin: {lr} runs, {lt} through the shaper, {lg} glyphs, {lf} fractional pens, thirds {lth:?} \
         (bound: 0 through, 0 fractional)"
    );
    println!(
        "shaped: {sr} runs, {st} through the shaper, {sg} glyphs, {sf} fractional pens, thirds {sth:?}, {seen} distinct \
         (bound: all {sr} through, ≥ 2 distinct)"
    );
    assert_eq!(lt, 0, "K81a's response: grid-like Latin went through the shaper {lt} times");
    assert_eq!(lf, 0, "grid-like Latin has {lf} fractional pens: a K81b arm on the grid would count them");
    assert_eq!(st, u64::from(sr), "setup: {st} of {sr} shaped runs went through the shaper");
    assert!(seen >= 2, "shaped runs land on {seen} third(s) {sth:?}: no phase for K81b to count");
}

/// **A shaped glyph's bearing fraction is drawn once** — T81.7's pin,
/// which asserted it drawn twice, flipped and renamed by T81.8.
/// `shaped_ink` placed a bitmap at `pen + bounds.xmin` and the canvas
/// rounded that, but fontdue's bitmap already carries `fract(bounds.xmin)`
/// inside its column 0 (which is why fontdue's own layout *floors*
/// `pen + xmin`), so a glyph whose `offset_x` is 0.5 or more drew one pixel
/// right. It is placed at `pen + floor(bounds.xmin)` now, and the phase is
/// the pen's fraction alone.
///
/// **A column check**, because T81.3's gates compared ink *rows* with
/// `Basic`'s, never columns, and that is how this shipped. Per glyph alone
/// at the start of a run (a whole pen on both paths):
///
/// - the `Complex` render's ink centroid against the shipped phase-1
///   bitmap's centroid placed at `10 + floor(bounds.xmin)`, computed from
///   fontdue's metrics, not from the placement: **0.000** (bound 0.01);
/// - against the `Basic` render's: the derivation's own residual, ≤ 1/6 +
///   0.05 px (K81d's absolute bound).
#[test]
fn a_shaped_glyph_draws_its_bearing_fraction_once() {
    let _serial = serial();
    let bytes = pinned::bytes();
    let font = Font::from_bytes(bytes, FontSettings::default()).expect("the pinned face parses");
    let basic = FontStack::from_static(bytes, None).expect("the pinned face builds a stack");
    let complex = complex(bytes);
    let before = fulgurite_shell::text::text_counters().complex;
    let (mut cases, mut wrong, mut worst_exact, mut worst_basic) = (0u32, Vec::new(), 0.0f32, 0.0f32);
    for &ch in &GLYPHS {
        let index = font.lookup_glyph_index(ch);
        for &px in &SIZES {
            let ox = offset_x(&font, index, px);
            let shaped = ink_cx(&complex, &forced(ch), [10.0, 10.0], px);
            let plain = ink_cx(&basic, &ch.to_string(), [10.0, 10.0], px);
            // Where the bearing says column 0 goes: from fontdue, and the
            // phase-1 bitmap from the shipped raster.
            let mut bitmap = derive(&complex, &font, ch, px, 0);
            bitmap.xmin += 10;
            let exact = shaped - bitmap.centroid();
            let d = shaped - plain;
            println!(
                "{ch:?} {px:>4} px: offset_x {ox:.3}; Complex − phase-1 bitmap at 10 + floor(xmin) {exact:+.3} px (bound 0.01); \
                 Complex − Basic {d:+.3} px (bound {ABS_BOUND:.3})"
            );
            cases += 1;
            worst_exact = worst_exact.max(exact.abs());
            worst_basic = worst_basic.max(d.abs());
            if exact.abs() > 0.01 {
                wrong.push(format!("{ch:?} at {px} px (offset_x {ox:.3}): {exact:+.3} px from where its bearing puts it"));
            }
            if d.abs() > ABS_BOUND {
                wrong.push(format!("{ch:?} at {px} px (offset_x {ox:.3}): {d:+.3} px from Basic's ink"));
            }
        }
    }
    let shaped = fulgurite_shell::text::text_counters().complex - before;
    println!(
        "{cases} cases, {shaped} runs through the shaper (setup ≥ {cases}); worst |Complex − bitmap| {worst_exact:.3} px \
         (bound 0.01), worst |Complex − Basic| {worst_basic:.3} px (bound {ABS_BOUND:.3})"
    );
    assert!(shaped >= u64::from(cases), "setup: only {shaped} of {cases} Complex renders went through the shaper");
    assert!(wrong.is_empty(), "a shaped glyph's column is not its bearing's in {} check(s):\n{}", wrong.len(), wrong.join("\n"));
}

/// The ink's horizontal centroid of `text` drawn at `at` on a fresh
/// 200×100 canvas, white on black, in canvas pixels.
fn ink_cx(stack: &FontStack, text: &str, at: [f32; 2], px: f32) -> f32 {
    let mut list = DisplayList::new();
    list.text_weighted([at[0], at[1], at[0] + 180.0, at[1] + 80.0], text, px, 400.0, Color(255, 255, 255, 255));
    let mut c = Canvas::with_font(200, 100, stack.clone());
    c.clear(Color(0, 0, 0, 255));
    c.render(&list, 200, 100);
    let (mut m, mut t) = (0.0f64, 0.0f64);
    for y in 0..c.height {
        for x in 0..c.width {
            let v = c.pixels[((y * c.width + x) * 4) as usize] as f64;
            m += (x as f64 + 0.5) * v;
            t += v;
        }
    }
    assert!(t > 0.0, "setup: {text:?} at {px} px drew no ink");
    (m / t) as f32
}

/// **T81.8's first gate: one canvas, `l` at x 10, 10⅓ and 10⅔.** Three
/// runs on one `Complex` canvas, each on its own band of rows, the run
/// origin carrying the fraction (`pen_of(rect_x + p.x)` honours it on a raw
/// `Canvas`). Exactly three cache entries, one per phase; each band's ink
/// centroid shifted from the first by the [`reference`]'s shift ± 0.05 px
/// (not `k/3`: an area centroid is not linear in a shift; T81.7); and
/// K81d's absolute check on the phase-1 bitmap through the shipped raster.
#[test]
fn one_canvas_draws_three_phases_of_one_glyph() {
    let _serial = serial();
    let bytes = pinned::bytes();
    let font = Font::from_bytes(bytes, FontSettings::default()).expect("the pinned face parses");
    let stack = complex(bytes);
    let px = 16.0;
    let index = font.lookup_glyph_index('l');
    let mut list = DisplayList::new();
    for k in 0..3 {
        let x = 10.0 + k as f32 / 3.0;
        let y = 10.0 + 30.0 * k as f32;
        list.text_weighted([x, y, x + 100.0, y + 30.0], &forced('l'), px, 400.0, Color(255, 255, 255, 255));
    }
    let mut c = Canvas::with_font(200, 100, stack.clone());
    c.clear(Color(0, 0, 0, 255));
    let before = fulgurite_shell::text::text_counters().complex;
    c.render(&list, 200, 100);
    let shaped = fulgurite_shell::text::text_counters().complex - before;
    let stats = c.glyph_cache_stats();
    let band = |k: u32| {
        let (mut m, mut t) = (0.0f64, 0.0f64);
        for y in 30 * k..30 * (k + 1) {
            for x in 0..c.width {
                let v = c.pixels[((y * c.width + x) * 4) as usize] as f64;
                m += (x as f64 + 0.5) * v;
                t += v;
            }
        }
        assert!(t > 0.0, "setup: band {k} drew no ink");
        (m / t) as f32
    };
    let cx: Vec<f32> = (0..3).map(band).collect();
    let refs: Vec<(f32, f32)> = (0..3).map(|k| reference(&font, index, px, k)).collect();
    let one = one_x(&font, index, px);
    let phase1 = derive(&stack, &font, 'l', px, 0);
    let abs = (phase1.centroid() - one.centroid()).abs();
    println!(
        "'l' at {px} px, x 10, 10⅓, 10⅔: {shaped} runs through the shaper (setup 3); cache misses {} (bound == 3), \
         hits {}, resets {}",
        stats.misses, stats.hits, stats.resets
    );
    let mut failures = Vec::new();
    for k in 1..3 {
        let got = cx[k] - cx[0];
        let want = refs[k].0 - refs[0].0;
        println!("phase {}: centroid shift {got:+.3} px, the reference's {want:+.3} (bound ± {REL_TOL})", k + 1);
        if (got - want).abs() > REL_TOL {
            failures.push(format!("phase {}: shift {got:+.3}, reference {want:+.3}", k + 1));
        }
    }
    println!("phase 1 through FontStack::rasterize against the 1× bitmap: |abs| {abs:.3} px (bound {ABS_BOUND:.3})");
    assert_eq!(shaped, 3, "setup: {shaped} of 3 runs went through the shaper");
    assert_eq!(stats.misses, 3, "one glyph at three thirds is {} cache entries, not 3", stats.misses);
    assert!(abs <= ABS_BOUND, "K81d absolute, shipped: phase 1 is {abs:.3} px from the 1× bitmap");
    assert!(failures.is_empty(), "the thirds do not move the ink: {}", failures.join("; "));
}

/// **T81.8's second case: fractions the production way.** A whole-pixel
/// run origin and a run whose *shaped advances* are fractional: `llll` at
/// 13 px, forced through the shaper by a zero-width joiner (without it the
/// run takes fontdue's whole-pixel path and counts one phase; T81.7). The
/// placements land on at least two phases, and a fresh canvas misses once
/// per distinct (glyph, phase). Setup: the `Complex` corpus of
/// `phases_occur_only_where_the_shaper_runs` has glyphs on a fractional pen
/// (counted there, and here for this run).
#[test]
fn shaped_advances_land_on_more_than_one_phase() {
    let _serial = serial();
    use std::collections::HashSet;
    let bytes = pinned::bytes();
    let stack = complex(bytes);
    let (px, text, origin) = (13.0, "llll\u{200d}", 10.0);
    let (placed, _) = stack.place_run(text, px, 400.0, f32::INFINITY);
    let fractional = placed.iter().filter(|p| p.x.fract().abs() > 1e-3).count();
    let pens: Vec<_> = placed.iter().map(|p| pen_of(origin + p.x, p)).collect();
    let phases: HashSet<u8> = pens.iter().map(|p| p.id.key().phase).collect();
    let glyphs: HashSet<u16> = pens.iter().map(|p| p.id.key().index).collect();
    let keys: HashSet<(u16, u8)> = pens.iter().map(|p| (p.id.key().index, p.id.key().phase)).collect();
    let mut list = DisplayList::new();
    list.text_weighted([origin, 10.0, origin + 150.0, 40.0], text, px, 400.0, Color(255, 255, 255, 255));
    let mut c = Canvas::with_font(200, 60, stack);
    c.clear(Color(0, 0, 0, 255));
    c.render(&list, 200, 60);
    let misses = c.glyph_cache_stats().misses;
    println!(
        "{text:?} at {px} px from x {origin}: {} glyphs, {fractional} on a fractional pen (setup > 0); phases {phases:?} \
         ({} distinct, bound ≥ 2); misses {misses} (bound == {} phases × {} glyphs = {})",
        placed.len(),
        phases.len(),
        phases.len(),
        glyphs.len(),
        keys.len()
    );
    assert!(fractional > 0, "setup: no glyph of {text:?} at {px} px is on a fractional pen");
    assert!(phases.len() >= 2, "{text:?} at {px} px landed on {} phase(s): {phases:?}", phases.len());
    assert_eq!(keys.len(), phases.len() * glyphs.len(), "setup: the run is one glyph at several phases");
    assert_eq!(misses, keys.len() as u64, "a fresh canvas missed {misses} times for {} (glyph, phase) keys", keys.len());
}

fn complex(bytes: &'static [u8]) -> FontStack {
    FontStack::from_static(bytes, None).expect("the pinned face builds a stack").with_shaping(Shaping::Complex)
}
