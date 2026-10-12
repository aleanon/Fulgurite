//! M40/T40.0, R1-F8: a glyph redrawn inside a damage region whose far
//! edge is fractional reaches that edge.
//!
//! Every command in the software rasteriser covers `floor(x0)..ceil(x1)`
//! of its clipped rect, and `clear_within` rounds outward to match —
//! except the glyph blit, which took its far edge with a bare `as i64`.
//! A region ending at 173.2 was cleared through row 173 and the glyphs
//! were redrawn only through row 172, so row 173 stayed background until
//! something damaged it again. Once damage is physical (T40.1), a
//! fractional edge is the ordinary case: every edge of a layout at 1.25
//! or 1.5 lands between pixels.
//!
//! Pinned to DejaVu Sans rather than `FontStack::system()`, so the row
//! the glyphs cross is the same row on every machine that runs it.
#![cfg(feature = "truetype")]

use fulgurite::display::{Color, DisplayList, Renderer};
use fulgurite_shell::software::Canvas;
use fulgurite_shell::text::FontStack;

const W: u32 = 360;
const H: u32 = 260;
const BG: Color = Color(10, 12, 20, 255);
const FG: Color = Color(240, 240, 240, 255);

fn face() -> Option<FontStack> {
    let regular = std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf").ok()?;
    FontStack::from_bytes(&regular, None)
}

/// A run whose glyphs cross the region's far edge.
fn frame(text: &str) -> DisplayList {
    let mut l = DisplayList::new();
    l.text_weighted([10.0, 140.0, 350.0, 200.0], text, 44.0, 400.0, FG);
    l
}

/// Paint `before` fully, then redraw `damage` from `after` the way a
/// shell does, and compare with a full render of `after` over the
/// pixels the region covers — `floor..ceil`, which is what the clear
/// touched. Returns the differing count and their bounding box.
fn partial_vs_full(damage: [f32; 4]) -> (usize, Option<[u32; 4]>, usize) {
    let font = face().expect("DejaVu Sans is the pinned face; install fonts-dejavu-core");
    let (before, after) = (frame("MMMMMMMMM"), frame("Hamburgefonts"));

    let mut full = Canvas::with_font(W, H, font.clone());
    full.clear(BG);
    full.render(&after, W, H);

    let mut partial = Canvas::with_font(W, H, font);
    partial.clear(BG);
    partial.render(&before, W, H);
    partial.clear_within(damage, BG);
    partial.render_within(&after, damage);

    let (x0, y0) = (damage[0].floor() as u32, damage[1].floor() as u32);
    let (x1, y1) = ((damage[2].ceil() as u32).min(W), (damage[3].ceil() as u32).min(H));
    let (mut n, mut bb, mut ink) = (0, None::<[u32; 4]>, 0);
    for y in y0..y1 {
        for x in x0..x1 {
            let i = ((y * W + x) * 4) as usize;
            if full.pixels[i..i + 4] != [BG.0, BG.1, BG.2, BG.3] {
                ink += 1;
            }
            if full.pixels[i..i + 4] != partial.pixels[i..i + 4] {
                n += 1;
                bb = Some(match bb {
                    None => [x, y, x + 1, y + 1],
                    Some(r) => [r[0].min(x), r[1].min(y), r[2].max(x + 1), r[3].max(y + 1)],
                });
            }
        }
    }
    (n, bb, ink)
}

/// Ink in one row of a full render — the setup check that the glyphs
/// really cross the edge row, without which no truncation could show.
fn ink_in_row(y: u32) -> usize {
    let font = face().expect("pinned face");
    let mut full = Canvas::with_font(W, H, font);
    full.clear(BG);
    full.render(&frame("Hamburgefonts"), W, H);
    (0..W)
        .filter(|&x| {
            let i = ((y * W + x) * 4) as usize;
            full.pixels[i..i + 4] != [BG.0, BG.1, BG.2, BG.3]
        })
        .count()
}

#[test]
fn an_integer_edged_region_redraws_exactly() {
    // The accept direction: whole-pixel edges were never affected, before
    // or after the fix.
    let (n, bb, ink) = partial_vs_full([0.0, 87.0, 360.0, 173.0]);
    println!("integer edge [0,87,360,173]: ink px {ink}; px differing {n} / bound 0; bbox {bb:?}");
    assert!(ink > 0, "setup: the run must ink inside the region");
    assert_eq!(n, 0, "an integer-edged region differs from a full render");
}

#[test]
fn a_fractional_far_edge_is_redrawn_through_its_last_row() {
    let edge_ink = ink_in_row(173);
    let (n, bb, ink) = partial_vs_full([0.0, 87.2, 360.0, 173.2]);
    println!(
        "fractional edge [0,87.2,360,173.2]: ink px {ink}, of them in row 173: {edge_ink}; \
         px differing {n} / bound 0; bbox {bb:?}"
    );
    assert!(edge_ink > 0, "setup: the glyphs must cross row 173, or no truncation can show");
    assert_eq!(n, 0, "the glyph redraw stopped short of the region's far edge: bbox {bb:?}");
}
