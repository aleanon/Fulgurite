//! Glyphs land where the metrics say they do. A text run drawn at a
//! rect puts its ink where `FontMetrics` measured it, so a box centred
//! on the measurement is centred on the ink. Found by
//! `scripts/ui_check.py centring`: every button label sat 2 px low
//! while every number in the placement arithmetic said it should not.
#![cfg(feature = "truetype")]

use fulgurite::display::{Color, DisplayList, Renderer};
use fulgurite::TextMeasure;
use fulgurite_shell::software::Canvas;
use fulgurite_shell::text::{FontMetrics, FontStack};

#[path = "fonts/pinned.rs"]
mod pinned;

const BG: Color = Color(0, 0, 0, 255);
const FG: Color = Color(255, 255, 255, 255);

/// The ink's first and last rows, and its first column (all past half
/// coverage).
fn ink_rows(c: &Canvas) -> (u32, u32, u32) {
    let (mut top, mut bottom, mut left) = (u32::MAX, 0, u32::MAX);
    for y in 0..c.height {
        for x in 0..c.width {
            let i = ((y * c.width + x) * 4) as usize;
            if c.pixels[i] > 128 {
                top = top.min(y);
                bottom = bottom.max(y + 1);
                left = left.min(x);
            }
        }
    }
    (top, bottom, left)
}

#[test]
fn a_run_is_vertically_centred_on_its_measured_box() {
    // The committed face (M43/T43.11), not the system's: this returned
    // early -- and passed -- on a machine with no system face.
    let font = FontStack::from_bytes(pinned::bytes(), None).expect("the pinned face parses");
    centred(font, "Basic", "1H", 16.0);
}

/// **The same under `Shaping::Complex`** — M81/T81.3
/// (`docs/plan-shaped-paint.md`). The shaped branch of `place_run_into`
/// puts each glyph on the baseline and cap shift the fontdue branch uses,
/// so a box centred on the measurement is centred on the ink either way,
/// and the same ink lands on the same rows as `Basic`'s, at 16–64 px.
/// Injection (T81.3's commit): drop the cap shift from the `Complex`
/// branch; the rows print one off at 64 px.
///
/// **And on the same first column** — M81/T81.8. This compared rows only,
/// and a shaped glyph drew its bearing fraction twice, one pixel right,
/// through all of M81a (T81.7's pin). The run's first glyph sits on a whole
/// pen in both modes, so its first inked column is the same; the setup
/// asserts some size where `1`'s bearing fraction is 0.5 or more, the
/// glyphs the double count moved. Injection (T81.8's commit): `shaped_ink`
/// back at `pen + xmin`; the column prints one off.
#[cfg(feature = "shaping")]
#[test]
fn a_shaped_run_is_vertically_centred_on_its_measured_box() {
    use fulgurite_shell::text::Shaping;
    let font = FontStack::from_static(pinned::bytes(), None)
        .expect("the pinned face parses")
        .with_shaping(Shaping::Complex);
    let basic = FontStack::from_static(pinned::bytes(), None).expect("the pinned face parses");
    let before = fulgurite_shell::text::text_counters().complex;
    let face = fontdue::Font::from_bytes(pinned::bytes(), fontdue::FontSettings::default()).expect("parses");
    let one = face.lookup_glyph_index('1');
    let mut moved = Vec::new();
    let mut max_ox = 0.0f32;
    for px in [16.0, 24.0, 32.0, 48.0, 64.0] {
        let xmin = face.metrics_indexed(one, px).bounds.xmin;
        max_ox = max_ox.max(xmin - xmin.floor());
        // A zero-width joiner between the two, so the run needs the shaper
        // (`needs_shaping`): plain Latin takes fontdue's path under Complex
        // since K81a's response, and would test the Basic placement twice.
        let c = centred(font.clone(), "Complex", "1\u{200d}H", px);
        let b = centred(basic.clone(), "Basic", "1H", px);
        println!(
            "{px} px: Complex ink rows {}..{} first column {}, Basic rows {}..{} first column {} (bound: equal); \
             '1' offset_x {:.3}",
            c.0,
            c.1,
            c.2,
            b.0,
            b.1,
            b.2,
            xmin - xmin.floor()
        );
        if c != b {
            moved.push(format!("{px} px: Complex {c:?}, Basic {b:?} (top, bottom, first column)"));
        }
    }
    let shaped = fulgurite_shell::text::text_counters().complex - before;
    println!("Complex: {shaped} runs through the shaper (setup > 0); largest '1' offset_x {max_ox:.3} (setup ≥ 0.5)");
    assert!(shaped > 0, "setup: the Complex stack laid nothing out through the shaper");
    assert!(max_ox >= 0.5, "setup: no size puts '1' on a bearing fraction ≥ 0.5, so a doubled fraction moves nothing");
    // The shaped branch sits glyphs on Basic's baseline and cap shift, so
    // the same ink lands on the same rows. The 1 px centring bound alone
    // cannot see a dropped cap shift: on this face it is 0 or 1 px.
    assert!(moved.is_empty(), "the shaped run's ink is not on Basic's rows and first column: {moved:?}");
}

fn centred(font: FontStack, mode: &str, text: &str, px: f32) -> (u32, u32, u32) {
    let mut metrics = FontMetrics::new(font.clone());
    let size = metrics.measure_weighted(text, px, 700.0, f32::INFINITY);

    // Draw the run in a box exactly the measured size, at y = 100.
    let mut list = DisplayList::new();
    let top = 100.0;
    list.text_weighted([10.0, top, 10.0 + size.w, top + size.h], text, px, 700.0, FG);
    let mut canvas = Canvas::with_font(200, 220, font);
    canvas.clear(BG);
    canvas.render(&list, 200, 220);

    let (ink_top, ink_bottom, left) = ink_rows(&canvas);
    let box_mid = top + size.h / 2.0;
    let ink_mid = (ink_top + ink_bottom) as f32 / 2.0;
    println!(
        "{mode}: box {top}..{} mid {box_mid}; ink {ink_top}..{ink_bottom} mid {ink_mid}; offset {:+.1} px (bound 1)",
        top + size.h,
        ink_mid - box_mid
    );
    assert!(
        (ink_mid - box_mid).abs() <= 1.0,
        "{mode}: cap-height ink is {:+.1} px from the box's centre",
        ink_mid - box_mid
    );
    (ink_top, ink_bottom, left)
}
