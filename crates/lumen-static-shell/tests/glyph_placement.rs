//! Glyphs land where the metrics say they do. A text run drawn at a
//! rect puts its ink where `FontMetrics` measured it, so a box centred
//! on the measurement is centred on the ink. Found by
//! `scripts/ui_check.py centring`: every button label sat 2 px low
//! while every number in the placement arithmetic said it should not.
#![cfg(feature = "truetype")]

use lumen_static::display::{Color, DisplayList, Renderer};
use lumen_static::TextMeasure;
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::{FontMetrics, FontStack};

const BG: Color = Color(0, 0, 0, 255);
const FG: Color = Color(255, 255, 255, 255);

fn ink_rows(c: &Canvas) -> (u32, u32) {
    let (mut top, mut bottom) = (u32::MAX, 0);
    for y in 0..c.height {
        for x in 0..c.width {
            let i = ((y * c.width + x) * 4) as usize;
            if c.pixels[i] > 128 {
                top = top.min(y);
                bottom = bottom.max(y + 1);
            }
        }
    }
    (top, bottom)
}

#[test]
fn a_run_is_vertically_centred_on_its_measured_box() {
    let Some(font) = FontStack::system() else {
        eprintln!("no system font; skipping");
        return;
    };
    let mut metrics = FontMetrics::new(font.clone());
    let px = 16.0;
    let size = metrics.measure_weighted("1H", px, 700.0, f32::INFINITY);

    // Draw the run in a box exactly the measured size, at y = 100.
    let mut list = DisplayList::new();
    let top = 100.0;
    list.text_weighted([10.0, top, 10.0 + size.w, top + size.h], "1H", px, 700.0, FG);
    let mut canvas = Canvas::with_font(200, 200, font);
    canvas.clear(BG);
    canvas.render(&list, 200, 200);

    let (ink_top, ink_bottom) = ink_rows(&canvas);
    let box_mid = top + size.h / 2.0;
    let ink_mid = (ink_top + ink_bottom) as f32 / 2.0;
    eprintln!(
        "box {top}..{} mid {box_mid}; ink {ink_top}..{ink_bottom} mid {ink_mid}",
        top + size.h
    );
    assert!(
        (ink_mid - box_mid).abs() <= 1.0,
        "cap-height ink is {:+.1} px from the box's centre",
        ink_mid - box_mid
    );
}
