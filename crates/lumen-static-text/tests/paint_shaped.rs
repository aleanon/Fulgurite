//! **What T15.8 claimed: an Arabic string measures *and paints*
//! right-to-left** — M45/T45.0 item 2, for R10-F2
//! (`docs/plan-text-correctness.md` §1.1, §2.1).
//!
//! `ComplexText` shapes, orders and reports clusters. The painter then
//! draws the original string with fontdue: cmap glyphs in logical order.
//! So Arabic is painted unjoined and left to right. This asserts the
//! claim as T15.8 made it — the painted glyph ids are the shaper's, in
//! visual order, and the ink stays in the shaper's box — and prints
//! whether what was painted is simply the isolated forms.

use lumen_static::display::{Color, DisplayList, Renderer};
use lumen_static::TextMeasure;
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::FontStack;
use lumen_static_text::ComplexText;

#[path = "../../lumen-static-shell/tests/fonts/pinned.rs"]
mod pinned;

const WORD: &str = "\u{645}\u{631}\u{62d}\u{628}\u{627}";
const PX: f32 = 32.0;

#[test]
#[ignore = "red until shaped paint (§2.1 A): R10-F2, the painter draws cmap glyphs in logical order"]
fn arabic_paints_the_shapers_glyphs_right_to_left_inside_its_box() {
    let bytes = pinned::bytes();
    let mut shaper = ComplexText::new(bytes, PX).expect("the pinned face parses");
    let face = rustybuzz::Face::from_slice(bytes, 0).expect("the pinned face parses");
    let boxed = shaper.measure(WORD, PX, f32::INFINITY);
    let shaped: Vec<u16> = shaper.shaped_glyphs(WORD, PX).into_iter().map(|(g, _)| g).collect();
    let isolated: Vec<u16> = WORD.chars().map(|c| face.glyph_index(c).map_or(0, |g| g.0)).collect();
    assert!(isolated.iter().all(|&g| g != 0), "setup: the pinned face lacks an Arabic letter: {isolated:?}");

    // The window's painter, at the shaper's box.
    let font = FontStack::from_bytes(bytes, None).expect("the pinned face parses");
    let (mut placed, _) = font.place_run(WORD, PX, 400.0, boxed.w);
    placed.sort_by(|a, b| a.x.total_cmp(&b.x));
    let painted: Vec<u16> = placed.iter().map(|p| p.id.index()).collect();
    // The same run with no width limit, in the order it is painted: what
    // the painter draws when nothing wraps it.
    let (free, _) = font.place_run(WORD, PX, 400.0, f32::INFINITY);
    let free: Vec<u16> = free.iter().map(|p| p.id.index()).collect();

    let mut list = DisplayList::new();
    let r = [10.0, 10.0, 10.0 + boxed.w, 10.0 + boxed.h];
    list.text_weighted(r, WORD, PX, 400.0, Color(0, 0, 0, 255));
    let mut canvas = Canvas::with_font(300, 100, font);
    canvas.clear(Color(255, 255, 255, 255));
    canvas.render(&list, 300, 100);
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for y in 0..100u32 {
        for x in 0..300u32 {
            let i = ((y * 300 + x) * 4) as usize;
            if canvas.pixels[i..i + 3].iter().any(|&c| c != 255) {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
            }
        }
    }
    let inside = x0 as f32 >= r[0] - 1.0 && x1 as f32 <= r[2] + 1.0 && y0 as f32 >= r[1] - 1.0 && y1 as f32 <= r[3] + 1.0;
    println!("shaper, visual order:   {shaped:?}");
    println!("painted, left to right: {painted:?}");
    println!("painted, unwrapped:     {free:?}");
    println!("isolated forms (cmap, logical order): {isolated:?} -> unwrapped painted == isolated: {}", free == isolated);
    println!("box {r:?}, ink {x0}..{x1} x {y0}..{y1}, inside (1 px): {inside}");
    assert_eq!(painted, shaped, "the painter drew other glyphs than the shaper produced");
    assert!(inside, "the painted ink left the shaper's box");
}
