//! **A damage region that ends mid-pixel repaints its last row whole**
//! (M57/D6).
//!
//! Found by the scale-2 drive: opening a picker's pad left one row under
//! the trigger's text stale — "19" read "!9" — because the popup's edge
//! sat at 304.2 logical px, which is 608.4 physical. `clear_within`
//! cleared row 608 (a fill rounds outward), and the glyph blit, clipped
//! to 608.4, truncated it away (the blit truncates), so the partial frame
//! kept the page colour where a full one drew ink. The canvas now snaps
//! the damage outward before either sees it, as the GPU backend's
//! `snap_out` does.
//!
//! This is the mechanism alone, on a bare display list: the showcase test
//! `a_popup_repaints_partially_to_the_same_pixels_at_scale_2` is the
//! assembled case.
#![cfg(feature = "truetype")]

use fulgurite::display::DisplayList;
use fulgurite::Color;
use fulgurite_shell::software::Canvas;
use fulgurite_shell::text::FontStack;

const BG: Color = Color(240, 242, 246, 255);
const INK: Color = Color(20, 20, 30, 255);

#[test]
fn a_fractional_damage_edge_through_text_repaints_like_a_full_frame() {
    let Some(font) = FontStack::system() else {
        eprintln!("no system font; skipping");
        return;
    };
    let (w, h) = (200u32, 80u32);
    let mut list = DisplayList::new();
    list.rect([0.0, 0.0, w as f32, h as f32], BG);
    list.text([10.0, 20.0, 190.0, 60.0], "19 Sept 2026", 28.0, INK);
    let (mut full, mut part) = (Canvas::with_font(w, h, font.clone()), Canvas::with_font(w, h, font));
    full.clear(BG);
    fulgurite::Renderer::render(&mut full, &list, w, h);
    // Every row the text inks, as the damage's fractional bottom edge:
    // the defect needs that row to carry glyph coverage.
    let inked: Vec<u32> = (0..h)
        .filter(|&y| (0..w).any(|x| {
            let i = ((y * w + x) * 4) as usize;
            full.pixels[i..i + 3] != [BG.0, BG.1, BG.2]
        }))
        .collect();
    assert!(inked.len() >= 10, "setup: the text inked only {} rows", inked.len());
    let mut worst = (0usize, 0u32);
    for &y in &inked {
        // A stale frame underneath: what the last frame left.
        part.clear(Color(255, 0, 0, 255));
        let r = [0.0, 0.0, w as f32, y as f32 + 0.4];
        part.clear_within(r, BG);
        part.render_within(&list, r);
        let rows = (y + 1) as usize * w as usize * 4;
        let differ = full.pixels[..rows].chunks(4).zip(part.pixels[..rows].chunks(4)).filter(|(a, b)| a != b).count();
        if differ > worst.0 {
            worst = (differ, y);
        }
    }
    println!("{} inked rows tried as a damage edge at +0.4 px; worst {} px differ (edge row {})",
             inked.len(), worst.0, worst.1);
    assert_eq!(worst.0, 0, "a damage edge at {}.4 left {} px stale above it", worst.1, worst.0);
}
